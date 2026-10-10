//! Atomic container creation.
//!
//! The archive is built in a temporary sibling of the destination, synced to
//! disk, reopened through the same [`inspect`](crate::inspect) path a reader
//! uses, and only then renamed over the destination. Any failure removes the
//! temporary file and leaves an existing destination byte-for-byte unchanged.

use crate::{
    ASSETS_PREFIX, ArtifactKind, FORMAT, FORMAT_VERSION, FileRef, InspectLimits, Inspection,
    MANIFEST_PATH, Manifest, PAYLOAD_PATH, ScratchDir,
    contracts::{ContractError, validate_contract},
    inspect,
    inspect::{Failure, check_entry_name},
    next_unique, rfc3339_utc, sha256_hex,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    fs::{self, File},
    io::{self, Seek, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Schema version written into every v1 manifest.
const SCHEMA_VERSION: u64 = 1;

/// Why [`create`] refused to publish.
#[derive(Debug)]
pub enum CreateError {
    /// The payload fails its contract. Nothing was written.
    Payload(Vec<ContractError>),
    /// An asset path is unsafe or duplicated, or the written archive failed
    /// reopen verification. Nothing was published.
    Rejected(Vec<Failure>),
    /// A filesystem or archive-writing error. Nothing was published.
    Io(io::Error),
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreateError::Payload(errors) => write!(f, "payload rejected: {errors:?}"),
            CreateError::Rejected(failures) => write!(f, "container rejected: {failures:?}"),
            CreateError::Io(error) => write!(f, "container not written: {error}"),
        }
    }
}

impl Error for CreateError {}

impl From<io::Error> for CreateError {
    fn from(error: io::Error) -> Self {
        CreateError::Io(error)
    }
}

/// Writes a `.ordo` container of `kind` to `dest`, atomically.
///
/// `assets` are `(archive path, bytes)` pairs whose paths must live under
/// `assets/`. The returned manifest is the one written and verified.
///
/// Durability rests on two steps: the temporary archive is `sync_all`ed before
/// it is reopened, and it is published by a rename over `dest`. On Windows that
/// rename is `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`. std offers no
/// directory fsync on Windows, so the rename itself is not made durable by this
/// code; only the temporary file's contents are.
///
/// Only a temporary this call created is removed on failure. If the temporary
/// name already exists, the call fails and that file is left alone.
pub fn create(
    dest: &Path,
    kind: ArtifactKind,
    payload: &Value,
    assets: &[(String, Vec<u8>)],
) -> Result<Manifest, CreateError> {
    validate_contract(kind.contract(), payload).map_err(CreateError::Payload)?;
    let payload_bytes = serde_json::to_vec_pretty(payload).map_err(|e| {
        CreateError::Payload(vec![ContractError {
            code: "schema",
            path: String::new(),
            message: e.to_string(),
        }])
    })?;
    check_assets(assets)?;

    let manifest = Manifest {
        format: FORMAT.to_owned(),
        format_version: FORMAT_VERSION,
        kind,
        schema_version: SCHEMA_VERSION,
        artifact_id: uuid::Uuid::new_v4().to_string(),
        created_at: rfc3339_utc(now_unix_seconds()),
        payload: FileRef {
            path: PAYLOAD_PATH.to_owned(),
            sha256: sha256_hex(&payload_bytes),
            size: payload_bytes.len() as u64,
        },
        assets: assets
            .iter()
            .map(|(path, bytes)| FileRef {
                path: path.clone(),
                sha256: sha256_hex(bytes),
                size: bytes.len() as u64,
            })
            .collect(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;

    let file_name = dest
        .file_name()
        .ok_or_else(|| io::Error::other("destination has no file name"))?;
    // A bare file name such as `out.ordo` has an empty parent; use the current
    // directory so the temporary sibling lands beside the destination.
    let parent = match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let temp = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name.to_string_lossy(),
        std::process::id(),
        next_unique()
    ));

    // `create_new` refuses to overwrite. If it fails, the path is not ours to
    // remove, so this early return must not reach the cleanup below.
    let file = File::create_new(&temp)?;

    let published = write_and_publish(
        file,
        &temp,
        dest,
        &manifest,
        &manifest_bytes,
        &payload_bytes,
        assets,
    );
    if published.is_err() {
        // Best effort: the temp was created by this call, and a failure to
        // remove it must not mask the error that caused the cleanup.
        let _ = fs::remove_file(&temp);
    }
    published.map(|()| manifest)
}

fn write_and_publish(
    file: File,
    temp: &Path,
    dest: &Path,
    manifest: &Manifest,
    manifest_bytes: &[u8],
    payload_bytes: &[u8],
    assets: &[(String, Vec<u8>)],
) -> Result<(), CreateError> {
    write_archive(file, manifest_bytes, payload_bytes, assets)?;
    verify_reopened(temp, manifest)?;
    // On Windows, std::fs::rename replaces an existing file via MoveFileEx.
    fs::rename(temp, dest)?;
    Ok(())
}

/// Writes and syncs the archive through the handle this call created.
fn write_archive(
    file: File,
    manifest_bytes: &[u8],
    payload_bytes: &[u8],
    assets: &[(String, Vec<u8>)],
) -> io::Result<()> {
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    write_entry(&mut zip, MANIFEST_PATH, manifest_bytes, options)?;
    write_entry(&mut zip, PAYLOAD_PATH, payload_bytes, options)?;
    for (name, bytes) in assets {
        write_entry(&mut zip, name, bytes, options)?;
    }
    let file = zip.finish().map_err(io::Error::other)?;
    file.sync_all()
}

fn write_entry<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> io::Result<()> {
    zip.start_file(name, options).map_err(io::Error::other)?;
    zip.write_all(bytes)
}

/// Reopens the temporary archive through the reader's own inspection path and
/// requires it to round-trip to the manifest that was written.
fn verify_reopened(path: &Path, expected: &Manifest) -> Result<(), CreateError> {
    let staging = ScratchDir::new("create-verify")?;
    match inspect(path, staging.path(), &InspectLimits::default()) {
        Inspection::Valid(routed) if routed.manifest == *expected => Ok(()),
        Inspection::Valid(_) => Err(CreateError::Rejected(vec![Failure::BadManifest {
            message: "reopened manifest differs from the one written".to_owned(),
        }])),
        Inspection::Invalid(failures) => Err(CreateError::Rejected(failures)),
        other => Err(CreateError::Rejected(vec![Failure::BadManifest {
            message: format!("reopened container was not valid: {other:?}"),
        }])),
    }
}

/// Refuses asset names that inspection would refuse, before anything is written.
fn check_assets(assets: &[(String, Vec<u8>)]) -> Result<(), CreateError> {
    let mut seen: HashMap<String, &str> = HashMap::new();
    for (path, _) in assets {
        if !path.starts_with(ASSETS_PREFIX) {
            return Err(CreateError::Rejected(vec![Failure::BadManifest {
                message: format!("asset {path} must live under assets/"),
            }]));
        }
        if let Err(reason) = check_entry_name(path) {
            return Err(CreateError::Rejected(vec![Failure::UnsafePath {
                path: path.clone(),
                reason,
            }]));
        }
        if let Some(first) = seen.insert(path.to_lowercase(), path.as_str()) {
            return Err(CreateError::Rejected(vec![Failure::DuplicatePath {
                path: path.clone(),
                first: first.to_owned(),
            }]));
        }
    }
    Ok(())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
