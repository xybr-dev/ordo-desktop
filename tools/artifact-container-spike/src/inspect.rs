//! Container inspection: every check that can run before extraction runs first,
//! then listed files are extracted into the caller's staging directory and
//! verified against the manifest.
//!
//! Order matters. Archive size, entry count, declared sizes, the compression
//! ratio, path safety, duplicates, and link or non-regular entries are judged
//! from the central directory alone. Only then is `manifest.json` read, and
//! only after the manifest's own format, version, and kind checks pass are
//! payload and assets extracted, each through a reader that stops one byte
//! past its declared size.

use crate::{
    ASSETS_PREFIX, ArtifactKind, FORMAT, FORMAT_VERSION, FileRef, MANIFEST_PATH, Manifest,
    PAYLOAD_PATH, SUPPORTED_VERSIONS,
    contracts::{ContractError, validate_contract},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File},
    io::{Read, Write},
    iter,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Size and shape ceilings applied before extraction. Defaults are the T0.8
/// plan's constants; tests override them to exercise each limit cheaply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectLimits {
    /// Largest archive file accepted, in bytes. Import ceiling is 64 MiB; the
    /// 4 GiB backup ceiling is a caller choice passed in here.
    pub max_archive: u64,
    /// Largest number of central-directory entries.
    pub max_entries: usize,
    /// Largest declared uncompressed size for one entry.
    pub max_entry: u64,
    /// Largest sum of declared uncompressed sizes across the archive.
    pub max_total: u64,
    /// Largest declared uncompressed/compressed ratio for one entry.
    pub max_ratio: u64,
}

impl Default for InspectLimits {
    fn default() -> Self {
        Self {
            max_archive: 64 * MIB,
            max_entries: 4096,
            max_entry: GIB,
            max_total: 4 * GIB,
            max_ratio: 200,
        }
    }
}

/// One reason an archive is refused. Each variant is detected before, or
/// while, extracting, and is never a guess about intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The archive could not be read at all (I/O error, not a ZIP, bad staging).
    Unreadable {
        message: String,
    },
    ArchiveTooLarge {
        size: u64,
        limit: u64,
    },
    TooManyEntries {
        count: usize,
        limit: usize,
    },
    EntryTooLarge {
        path: String,
        size: u64,
        limit: u64,
    },
    TotalTooLarge {
        total: u64,
        limit: u64,
    },
    RatioExceeded {
        path: String,
        ratio: u64,
        limit: u64,
    },
    /// Absolute, `..`, `.`, backslash, drive prefix, NUL, colon, or empty segment.
    UnsafePath {
        path: String,
        reason: &'static str,
    },
    /// Two entries whose names match case-insensitively.
    DuplicatePath {
        path: String,
        first: String,
    },
    /// A symbolic link (`S_IFLNK` in the Unix mode bits).
    Link {
        path: String,
    },
    /// A device, FIFO, socket, or directory entry without the plain marker.
    NonRegular {
        path: String,
        mode: u32,
    },
    MissingManifest,
    BadManifest {
        message: String,
    },
    /// A regular file present in the archive but not listed in the manifest.
    Unlisted {
        path: String,
    },
    /// A file listed in the manifest but absent from the archive.
    Missing {
        path: String,
    },
    /// Extracted bytes do not hash to the manifest's SHA-256.
    Checksum {
        path: String,
        expected: String,
        actual: String,
    },
    /// Declared, extracted, and manifest sizes disagree.
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    /// The payload failed JSON parsing or its payload contract.
    Payload(Vec<ContractError>),
}

/// A fully verified container, ready to route by kind.
#[derive(Debug, Clone, PartialEq)]
pub struct Routed {
    pub kind: ArtifactKind,
    pub manifest: Manifest,
    pub payload: Value,
    /// Verified asset files, absolute, inside the staging directory.
    pub staged_assets: Vec<PathBuf>,
}

/// The outcome of inspecting one archive.
// `Valid(Routed)` is large, but the Interface fixes it unboxed and an outcome
// is produced once per inspection, so the size difference costs nothing here.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Inspection {
    /// Every check passed; the container is verified and staged.
    Valid(Routed),
    /// The container is malformed, unsafe, unlisted, or tampered with.
    Invalid(Vec<Failure>),
    /// The container is well formed but names a kind or schema version this
    /// build does not support. It is not guessed at; the message says which
    /// Ordo version is needed.
    Unsupported {
        kind: String,
        version: u64,
        supported: &'static [u64],
    },
    /// Reserved for the reconciliation layer, which decides what to do with a
    /// valid container against existing Parish data. The container layer never
    /// produces it, and it is distinct from `Invalid` by type.
    NeedsDecision { reasons: Vec<String> },
}

/// A regular-file entry that passed the central-directory checks.
struct Entry {
    name: String,
    index: usize,
    size: u64,
}

/// Inspects `archive`, staging verified files into `staging`.
///
/// `staging` must be an existing, empty directory owned by the caller. Nothing
/// is written anywhere else. The function takes no Parish handle.
pub fn inspect(archive: &Path, staging: &Path, limits: &InspectLimits) -> Inspection {
    match run(archive, staging, limits) {
        Ok(outcome) => outcome,
        Err(failures) => Inspection::Invalid(failures),
    }
}

fn run(archive: &Path, staging: &Path, limits: &InspectLimits) -> Result<Inspection, Vec<Failure>> {
    let size = fs::metadata(archive)
        .map_err(|e| vec![unreadable(format!("cannot read archive metadata: {e}"))])?
        .len();
    if size > limits.max_archive {
        return Err(vec![Failure::ArchiveTooLarge {
            size,
            limit: limits.max_archive,
        }]);
    }
    let staging = fs::canonicalize(staging)
        .map_err(|e| vec![unreadable(format!("staging directory unavailable: {e}"))])?;
    let file =
        File::open(archive).map_err(|e| vec![unreadable(format!("cannot open archive: {e}"))])?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| vec![unreadable(format!("not a readable ZIP archive: {e}"))])?;

    if zip.len() > limits.max_entries {
        return Err(vec![Failure::TooManyEntries {
            count: zip.len(),
            limit: limits.max_entries,
        }]);
    }
    let entries = scan_entries(&mut zip, limits)?;

    // Manifest: the single root document that lists everything else.
    let manifest_entry = entries
        .iter()
        .find(|entry| entry.name == MANIFEST_PATH)
        .ok_or(vec![Failure::MissingManifest])?;
    let manifest_bytes = read_manifest_bytes(&mut zip, manifest_entry)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes).map_err(|e| {
        vec![bad_manifest(format!(
            "manifest.json is not valid JSON: {e}"
        ))]
    })?;
    let manifest = match parse_manifest(&manifest_value)? {
        ManifestCheck::Ready(manifest) => manifest,
        ManifestCheck::Refused(outcome) => return Ok(outcome),
    };

    // Listing: every regular file except the manifest must be listed, and every
    // listed file must be present. Both directions are checked.
    let expected: BTreeSet<&str> = iter::once(manifest.payload.path.as_str())
        .chain(manifest.assets.iter().map(|asset| asset.path.as_str()))
        .collect();
    let actual: BTreeSet<&str> = entries
        .iter()
        .filter(|entry| entry.name != MANIFEST_PATH)
        .map(|entry| entry.name.as_str())
        .collect();
    let mut listing: Vec<Failure> = actual
        .difference(&expected)
        .map(|path| Failure::Unlisted {
            path: (*path).to_owned(),
        })
        .collect();
    listing.extend(expected.difference(&actual).map(|path| Failure::Missing {
        path: (*path).to_owned(),
    }));
    if !listing.is_empty() {
        return Err(listing);
    }

    // Extraction: only now is any listed content read. Each file is verified
    // against its manifest entry; failures are collected per file.
    let by_name: HashMap<&str, &Entry> = entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry))
        .collect();
    let mut failures = Vec::new();
    let payload_bytes = match read_listed(&mut zip, &by_name, &manifest.payload) {
        Ok(bytes) => bytes,
        Err(failure) => {
            failures.push(failure);
            Vec::new()
        }
    };
    let mut staged_assets = Vec::new();
    for asset in &manifest.assets {
        match stage_listed(&mut zip, &by_name, asset, &staging) {
            Ok(path) => staged_assets.push(path),
            Err(failure) => failures.push(failure),
        }
    }
    if !failures.is_empty() {
        return Err(failures);
    }

    // Payload: JSON parse, then the kind's payload contract.
    let payload: Value = serde_json::from_slice(&payload_bytes).map_err(|e| {
        vec![Failure::Payload(vec![ContractError {
            code: "schema",
            path: String::new(),
            message: format!("payload.json is not valid JSON: {e}"),
        }])]
    })?;
    validate_contract(manifest.kind.contract(), &payload)
        .map_err(|errors| vec![Failure::Payload(errors)])?;
    write_staged(&staging, PAYLOAD_PATH, &payload_bytes)?;

    Ok(Inspection::Valid(Routed {
        kind: manifest.kind,
        manifest,
        payload,
        staged_assets,
    }))
}

/// Walks the central directory without decompressing anything. Collects every
/// failure it can find rather than stopping at the first.
fn scan_entries(
    zip: &mut ZipArchive<File>,
    limits: &InspectLimits,
) -> Result<Vec<Entry>, Vec<Failure>> {
    const S_IFMT: u32 = 0o170_000;
    const S_IFREG: u32 = 0o100_000;
    const S_IFDIR: u32 = 0o040_000;
    const S_IFLNK: u32 = 0o120_000;

    let mut failures = Vec::new();
    let mut entries = Vec::new();
    // Folded name to the first spelling seen, for case-insensitive duplicates.
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut total: u64 = 0;

    for index in 0..zip.len() {
        let raw = match zip.by_index_raw(index) {
            Ok(raw) => raw,
            Err(e) => {
                failures.push(unreadable(format!("central directory entry {index}: {e}")));
                continue;
            }
        };
        let name = raw.name().to_owned();
        let size = raw.size();
        let compressed = raw.compressed_size();
        let mode = raw.unix_mode();

        // A trailing slash is a directory marker. The name is judged without it.
        let is_dir = name.ends_with('/');
        let bare: &str = name.strip_suffix('/').unwrap_or(name.as_str());
        if let Err(reason) = check_entry_name(bare) {
            failures.push(Failure::UnsafePath { path: name, reason });
            continue;
        }

        // Unix mode bits, when the archiver recorded them, decide the file type.
        let file_type = mode.map_or(0, |mode| mode & S_IFMT);
        if file_type == S_IFLNK {
            failures.push(Failure::Link { path: name });
            continue;
        }
        let allowed_type = if is_dir { S_IFDIR } else { S_IFREG };
        if file_type != 0 && file_type != allowed_type {
            failures.push(Failure::NonRegular {
                path: name,
                mode: mode.unwrap_or(0),
            });
            continue;
        }

        // Duplicates are judged for directories too, so `assets/` and `Assets`
        // cannot both appear.
        let folded = bare.to_lowercase();
        if let Some(first) = seen.get(&folded) {
            failures.push(Failure::DuplicatePath {
                path: name.clone(),
                first: first.clone(),
            });
            continue;
        }
        seen.insert(folded, name.clone());

        if is_dir {
            // A plain directory marker carries no content; nothing to list.
            continue;
        }
        if size > limits.max_entry {
            failures.push(Failure::EntryTooLarge {
                path: name,
                size,
                limit: limits.max_entry,
            });
            continue;
        }
        // A zero-byte compressed stream with content, or a ratio above the
        // ceiling, is a decompression bomb. Compressed zero is treated as an
        // infinite ratio.
        if size > 0 && (compressed == 0 || size > limits.max_ratio.saturating_mul(compressed)) {
            failures.push(Failure::RatioExceeded {
                path: name,
                ratio: size.checked_div(compressed).unwrap_or(u64::MAX),
                limit: limits.max_ratio,
            });
            continue;
        }
        total = total.saturating_add(size);
        entries.push(Entry { name, index, size });
    }

    if total > limits.max_total {
        failures.push(Failure::TotalTooLarge {
            total,
            limit: limits.max_total,
        });
    }
    if failures.is_empty() {
        Ok(entries)
    } else {
        Err(failures)
    }
}

/// Path rules for every archive entry and every manifest-listed path. Applied
/// to the name with any directory marker already removed.
///
/// NFC normalisation is not applied: the workspace has no normalisation crate,
/// so duplicates are caught by case folding alone.
pub(crate) fn check_entry_name(name: &str) -> Result<(), &'static str> {
    let bytes = name.as_bytes();
    if name.is_empty() {
        return Err("empty name");
    }
    if name.contains('\0') {
        return Err("NUL byte");
    }
    if name.contains('\\') {
        return Err("backslash separator");
    }
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err("drive letter");
    }
    if name.contains(':') {
        return Err("colon");
    }
    if name.starts_with('/') {
        return Err("absolute path");
    }
    for segment in name.split('/') {
        match segment {
            "" => return Err("empty segment"),
            ".." => return Err("parent traversal"),
            "." => return Err("current-directory segment"),
            _ => {}
        }
    }
    Ok(())
}

/// The manifest after its own checks, or the outcome to return instead of
/// continuing (unsupported kind or version).
enum ManifestCheck {
    Ready(Manifest),
    Refused(Inspection),
}

fn parse_manifest(value: &Value) -> Result<ManifestCheck, Vec<Failure>> {
    let object = value
        .as_object()
        .ok_or_else(|| vec![bad_manifest("manifest must be a JSON object")])?;
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(vec![bad_manifest("format must be \"ordo-container\"")]);
    }
    if object.get("format_version").and_then(Value::as_u64) != Some(FORMAT_VERSION) {
        return Err(vec![bad_manifest("format_version must be 1")]);
    }
    let kind_text = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| vec![bad_manifest("kind must be a string")])?;
    let version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| vec![bad_manifest("schema_version must be an integer")])?;

    // An unknown kind or a schema version this build lacks is refused, not
    // guessed at: the container may be newer than this Ordo.
    if kind_text.parse::<ArtifactKind>().is_err() || !SUPPORTED_VERSIONS.contains(&version) {
        return Ok(ManifestCheck::Refused(Inspection::Unsupported {
            kind: kind_text.to_owned(),
            version,
            supported: SUPPORTED_VERSIONS,
        }));
    }

    let manifest: Manifest = serde_json::from_value(value.clone())
        .map_err(|e| vec![bad_manifest(format!("manifest does not match v1: {e}"))])?;
    if manifest.payload.path != PAYLOAD_PATH {
        return Err(vec![bad_manifest("payload.path must be \"payload.json\"")]);
    }
    let mut listed = BTreeSet::new();
    for file in iter::once(&manifest.payload).chain(manifest.assets.iter()) {
        check_listed_path(&file.path)?;
        if !listed.insert(file.path.to_lowercase()) {
            return Err(vec![bad_manifest(format!(
                "manifest lists {} more than once",
                file.path
            ))]);
        }
    }
    for asset in &manifest.assets {
        if !asset.path.starts_with(ASSETS_PREFIX) {
            return Err(vec![bad_manifest(format!(
                "asset {} must live under assets/",
                asset.path
            ))]);
        }
    }
    Ok(ManifestCheck::Ready(manifest))
}

fn check_listed_path(path: &str) -> Result<(), Vec<Failure>> {
    check_entry_name(path)
        .map_err(|reason| vec![bad_manifest(format!("listed path {path}: {reason}"))])
}

/// Reads the manifest through a reader bounded by its declared size.
fn read_manifest_bytes(zip: &mut ZipArchive<File>, entry: &Entry) -> Result<Vec<u8>, Vec<Failure>> {
    let mut reader = zip
        .by_index(entry.index)
        .map_err(|e| vec![unreadable(format!("reading {MANIFEST_PATH}: {e}"))])?;
    let mut bytes = Vec::new();
    let read = (&mut reader)
        .take(entry.size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| vec![unreadable(format!("reading {MANIFEST_PATH}: {e}"))])?;
    if read as u64 != entry.size {
        return Err(vec![Failure::SizeMismatch {
            path: MANIFEST_PATH.to_owned(),
            expected: entry.size,
            actual: read as u64,
        }]);
    }
    Ok(bytes)
}

/// Looks up a listed file's entry. The listing check has already run, so a miss
/// here is reported rather than assumed impossible.
fn listed_entry<'a>(by_name: &HashMap<&str, &'a Entry>, path: &str) -> Result<&'a Entry, Failure> {
    by_name.get(path).copied().ok_or_else(|| Failure::Missing {
        path: path.to_owned(),
    })
}

fn read_listed(
    zip: &mut ZipArchive<File>,
    by_name: &HashMap<&str, &Entry>,
    expected: &FileRef,
) -> Result<Vec<u8>, Failure> {
    let entry = listed_entry(by_name, &expected.path)?;
    let mut reader = zip
        .by_index(entry.index)
        .map_err(|e| unreadable(format!("reading {}: {e}", expected.path)))?;
    let mut bytes = Vec::new();
    verify_into(&mut reader, entry, expected, &mut bytes)?;
    Ok(bytes)
}

/// Extracts one listed asset into staging, verifying it as it streams.
fn stage_listed(
    zip: &mut ZipArchive<File>,
    by_name: &HashMap<&str, &Entry>,
    expected: &FileRef,
    staging: &Path,
) -> Result<PathBuf, Failure> {
    let entry = listed_entry(by_name, &expected.path)?;
    let target = staged_target(staging, &expected.path)?;
    let mut out = File::create_new(&target)
        .map_err(|e| unreadable(format!("creating {}: {e}", expected.path)))?;
    let mut reader = zip
        .by_index(entry.index)
        .map_err(|e| unreadable(format!("reading {}: {e}", expected.path)))?;
    verify_into(&mut reader, entry, expected, &mut out)?;
    Ok(target)
}

/// Streams one entry into `out`, hashing as it goes. The reader stops one byte
/// past the declared size, so a header that lies about its size cannot make us
/// read unbounded data.
fn verify_into<R: Read, W: Write>(
    reader: R,
    entry: &Entry,
    expected: &FileRef,
    out: &mut W,
) -> Result<(), Failure> {
    let path = expected.path.as_str();
    let mut limited = reader.take(entry.size.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let read = limited
            .read(&mut buffer)
            .map_err(|e| unreadable(format!("reading {path}: {e}")))?;
        if read == 0 {
            break;
        }
        written += read as u64;
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])
            .map_err(|e| unreadable(format!("writing {path}: {e}")))?;
    }
    if written != entry.size {
        return Err(Failure::SizeMismatch {
            path: path.to_owned(),
            expected: entry.size,
            actual: written,
        });
    }
    if written != expected.size {
        return Err(Failure::SizeMismatch {
            path: path.to_owned(),
            expected: expected.size,
            actual: written,
        });
    }
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected.sha256.to_ascii_lowercase() {
        return Err(Failure::Checksum {
            path: path.to_owned(),
            expected: expected.sha256.clone(),
            actual,
        });
    }
    Ok(())
}

/// Resolves a staged file path and asserts it stays inside `staging`. The name
/// was already vetted by [`check_entry_name`]; the canonical check is a second
/// guard against anything that resolves outside the directory.
fn staged_target(staging: &Path, relative: &str) -> Result<PathBuf, Failure> {
    let target = staging.join(relative);
    let (Some(parent), Some(file_name)) = (target.parent(), target.file_name()) else {
        return Err(Failure::UnsafePath {
            path: relative.to_owned(),
            reason: "no parent directory",
        });
    };
    fs::create_dir_all(parent).map_err(|e| unreadable(format!("creating {relative}: {e}")))?;
    let parent =
        fs::canonicalize(parent).map_err(|e| unreadable(format!("resolving {relative}: {e}")))?;
    if !parent.starts_with(staging) {
        return Err(Failure::UnsafePath {
            path: relative.to_owned(),
            reason: "resolves outside staging",
        });
    }
    Ok(parent.join(file_name))
}

fn write_staged(staging: &Path, relative: &str, bytes: &[u8]) -> Result<(), Vec<Failure>> {
    let target = staged_target(staging, relative).map_err(|f| vec![f])?;
    let mut file = File::create_new(&target)
        .map_err(|e| vec![unreadable(format!("creating {relative}: {e}"))])?;
    file.write_all(bytes)
        .map_err(|e| vec![unreadable(format!("writing {relative}: {e}"))])
}

fn unreadable(message: impl Into<String>) -> Failure {
    Failure::Unreadable {
        message: message.into(),
    }
}

fn bad_manifest(message: impl Into<String>) -> Failure {
    Failure::BadManifest {
        message: message.into(),
    }
}
