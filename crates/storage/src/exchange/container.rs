//! Container inspection, ported from the T0.8 spike.
//!
//! Order matters. Archive size, entry count, declared sizes, the compression
//! ratio, path safety, duplicates and link or non-regular entries are judged
//! from the central directory alone. Only then is `manifest.json` read, and
//! only after its format, kind and version checks pass is the payload read,
//! through a reader that stops one byte past its declared size. Everything is
//! held in memory: nothing is extracted to disk.

use std::collections::{BTreeSet, HashMap};
use std::io::{Cursor, Read};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use super::findings::{Finding, FindingCode};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

const FORMAT: &str = "ordo-container";
const FORMAT_VERSION: u64 = 1;
const MANIFEST_PATH: &str = "manifest.json";
const PAYLOAD_PATH: &str = "payload.json";
const ASSETS_PREFIX: &str = "assets/";

/// The kinds of Ordo file this build knows by name.
const KNOWN_KINDS: [&str; 5] = [
    "song-import",
    "liturgy-import",
    "celebration-package",
    "parish-backup",
    "diagnostic-report",
];

/// End Of Central Directory record: fixed part, signature and offsets.
const EOCD_SIG: &[u8; 4] = b"PK\x05\x06";
const EOCD_LEN: usize = 22;
/// A comment can be up to 0xFFFF bytes, so the record can start this far back.
const EOCD_SEARCH: usize = 0xFFFF + EOCD_LEN;
/// ZIP64 End Of Central Directory locator, just before the EOCD.
const ZIP64_LOCATOR_SIG: &[u8; 4] = b"PK\x06\x07";
const ZIP64_LOCATOR_LEN: usize = 20;
/// ZIP64 End Of Central Directory record, read at the locator's offset.
const ZIP64_EOCD_SIG: &[u8; 4] = b"PK\x06\x06";
const ZIP64_EOCD_LEN: usize = 56;

/// Size and shape ceilings applied before anything is read. The defaults are
/// the T0.8 constants (README section 8); tests override them to exercise
/// each limit cheaply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectLimits {
    /// Largest archive accepted, in bytes (64 MiB for every container kind).
    pub max_archive: u64,
    /// Largest number of central-directory records, counted raw: duplicate
    /// names count once per record.
    pub max_entries: usize,
    /// Largest declared uncompressed size for one entry. The manifest and the
    /// payload are read into memory, so this also bounds what a read holds.
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
            max_entry: 64 * MIB,
            max_total: 4 * GIB,
            max_ratio: 200,
        }
    }
}

/// A verified payload, ready for its contract.
pub(super) struct Opened {
    /// The payload's exact bytes.
    pub payload: Vec<u8>,
}

/// Why an archive did not open.
pub(super) enum Refusal {
    /// Malformed, unsafe, unlisted, tampered with or of another kind.
    Invalid(Vec<Finding>),
    /// Well formed, but from a newer Ordo or of an unknown kind.
    Unsupported(String),
}

impl From<Finding> for Refusal {
    fn from(finding: Finding) -> Self {
        Self::Invalid(vec![finding])
    }
}

impl From<Vec<Finding>> for Refusal {
    fn from(findings: Vec<Finding>) -> Self {
        Self::Invalid(findings)
    }
}

/// A regular-file entry that passed the central-directory checks.
struct Entry {
    name: String,
    index: usize,
    size: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRef {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // strictness only: the creation fields are checked, not used
struct Manifest {
    format: String,
    format_version: u64,
    kind: String,
    schema_version: u64,
    artifact_id: String,
    created_at: String,
    payload: FileRef,
    assets: Vec<FileRef>,
}

/// The "requires a newer Ordo" refusal text.
fn unsupported_message(kind: &str, version: u64, supported: u64) -> String {
    format!(
        "requires an Ordo version supporting {kind} v{version}; this build supports v{supported}"
    )
}

fn not_ordo(detail: impl std::fmt::Display) -> Finding {
    Finding::container(format!("It isn't an Ordo file. ({detail})"))
}

/// Opens the archive in `bytes`, requiring kind `kind` at schema version
/// `version`, and returns its verified payload bytes.
pub(super) fn open(
    bytes: &[u8],
    limits: &InspectLimits,
    kind: &str,
    version: u64,
) -> Result<Opened, Refusal> {
    let size = bytes.len() as u64;
    if size > limits.max_archive {
        return Err(Finding::container(format!(
            "The file is {size} bytes; the limit is {} bytes.",
            limits.max_archive
        ))
        .into());
    }
    let declared = declared_entry_count(bytes)?;
    if declared > limits.max_entries as u64 {
        return Err(Finding::container(format!(
            "The archive has {declared} entries; the limit is {}.",
            limits.max_entries
        ))
        .into());
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| not_ordo(format!("not a readable ZIP archive: {error}")))?;
    if declared != zip.len() as u64 {
        return Err(Finding::container(format!(
            "The archive declares {declared} entries but {} have distinct names.",
            zip.len()
        ))
        .into());
    }
    let entries = scan_entries(&mut zip, limits)?;

    let manifest_entry = entries
        .iter()
        .find(|entry| entry.name == MANIFEST_PATH)
        .ok_or_else(|| not_ordo("no manifest.json"))?;
    let manifest_bytes = read_bounded(&mut zip, manifest_entry, MANIFEST_PATH)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| Finding::container(format!("manifest.json is not valid JSON: {error}")))?;
    let manifest = match parse_manifest(&manifest_value, kind, version)? {
        Parsed::Ready(manifest) => manifest,
        Parsed::Unsupported(message) => return Err(Refusal::Unsupported(message)),
    };
    if !manifest.assets.is_empty() {
        return Err(
            Finding::new(FindingCode::Assets, "", "song-import v2 carries no assets").into(),
        );
    }

    // Every regular file except the manifest must be listed and every listed
    // file present.
    let expected: BTreeSet<&str> = std::iter::once(manifest.payload.path.as_str()).collect();
    let actual: BTreeSet<&str> = entries
        .iter()
        .filter(|entry| entry.name != MANIFEST_PATH)
        .map(|entry| entry.name.as_str())
        .collect();
    let mut listing: Vec<Finding> = actual
        .difference(&expected)
        .map(|path| {
            Finding::container(format!("{path} is in the archive but not in the manifest."))
        })
        .collect();
    listing.extend(expected.difference(&actual).map(|path| {
        Finding::container(format!("{path} is in the manifest but not in the archive."))
    }));
    if !listing.is_empty() {
        return Err(listing.into());
    }

    let by_name: HashMap<&str, &Entry> = entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry))
        .collect();
    let entry = by_name
        .get(manifest.payload.path.as_str())
        .copied()
        .ok_or_else(|| Finding::container("payload.json is missing"))?;
    let payload = read_bounded(&mut zip, entry, PAYLOAD_PATH)?;
    if payload.len() as u64 != manifest.payload.size {
        return Err(Finding::container(format!(
            "payload.json is {} bytes; the manifest says {}.",
            payload.len(),
            manifest.payload.size
        ))
        .into());
    }
    let actual_hash = sha256_hex(&payload);
    if actual_hash != manifest.payload.sha256.to_ascii_lowercase() {
        return Err(Finding::container(format!(
            "payload.json hashes to {actual_hash}; the manifest says {}.",
            manifest.payload.sha256
        ))
        .into());
    }
    Ok(Opened { payload })
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The total record count the archive declares, read from the End Of Central
/// Directory record before the zip reader sees the bytes.
///
/// The zip crate keys entries by name, so two records with the same name
/// collapse into one and its count is short; this count is the raw one. A
/// ZIP64 record is followed when a ZIP64 locator precedes the EOCD.
fn declared_entry_count(bytes: &[u8]) -> Result<u64, Finding> {
    let tail_start = bytes.len().saturating_sub(EOCD_SEARCH);
    let tail = &bytes[tail_start..];
    let eocd = find_eocd(tail).ok_or_else(|| not_ordo("no end of central directory record"))?;
    let count = u64::from(u16::from_le_bytes([tail[eocd + 10], tail[eocd + 11]]));
    let has_locator = eocd >= ZIP64_LOCATOR_LEN
        && &tail[eocd - ZIP64_LOCATOR_LEN..eocd - ZIP64_LOCATOR_LEN + 4] == ZIP64_LOCATOR_SIG;
    if !has_locator {
        // Without a locator a 0xFFFF count is the field's true value.
        return Ok(count);
    }
    let locator = eocd - ZIP64_LOCATOR_LEN;
    let zip64_offset = u64::from_le_bytes(
        tail[locator + 8..locator + 16]
            .try_into()
            .map_err(|_| not_ordo("ZIP64 locator truncated"))?,
    );
    let start = usize::try_from(zip64_offset)
        .map_err(|_| not_ordo("ZIP64 locator points outside the file"))?;
    let record = bytes
        .get(start..start.saturating_add(ZIP64_EOCD_LEN))
        .ok_or_else(|| not_ordo("ZIP64 end record is truncated"))?;
    if &record[0..4] != ZIP64_EOCD_SIG {
        return Err(not_ordo("ZIP64 locator does not point at a ZIP64 record"));
    }
    Ok(u64::from_le_bytes(
        record[32..40]
            .try_into()
            .map_err(|_| not_ordo("ZIP64 record truncated"))?,
    ))
}

/// Offset of the EOCD record inside `tail`. The record is followed by its
/// comment, so the true one ends exactly at the end of `tail`; that match is
/// preferred. Otherwise the last signature found is used.
fn find_eocd(tail: &[u8]) -> Option<usize> {
    let last_start = tail.len().checked_sub(EOCD_LEN)?;
    let mut fallback = None;
    for start in (0..=last_start).rev() {
        if &tail[start..start + 4] != EOCD_SIG {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(start);
        }
        let comment = usize::from(u16::from_le_bytes([tail[start + 20], tail[start + 21]]));
        if start + EOCD_LEN + comment == tail.len() {
            return Some(start);
        }
    }
    fallback
}

/// Walks the central directory without decompressing anything, collecting
/// every failure it can find.
fn scan_entries(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    limits: &InspectLimits,
) -> Result<Vec<Entry>, Vec<Finding>> {
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
            Err(error) => {
                failures.push(not_ordo(format!(
                    "central directory entry {index}: {error}"
                )));
                continue;
            }
        };
        let name = match raw.name() {
            Ok(name) => name.into_owned(),
            Err(error) => {
                failures.push(Finding::container(format!(
                    "Entry {index} has an undecodable name: {error}"
                )));
                continue;
            }
        };
        let size = raw.size();
        let compressed = raw.compressed_size();
        let mode = raw.unix_mode();

        // A trailing slash is a directory marker; the name is judged without it.
        let is_dir = name.ends_with('/');
        let bare: &str = name.strip_suffix('/').unwrap_or(name.as_str());
        if let Err(reason) = check_entry_name(bare) {
            failures.push(Finding::container(format!(
                "Unsafe entry name {name:?}: {reason}."
            )));
            continue;
        }

        let file_type = mode.map_or(0, |mode| mode & S_IFMT);
        if file_type == S_IFLNK {
            failures.push(Finding::container(format!("{name} is a symbolic link.")));
            continue;
        }
        let allowed_type = if is_dir { S_IFDIR } else { S_IFREG };
        if file_type != 0 && file_type != allowed_type {
            failures.push(Finding::container(format!(
                "{name} is not a regular file (mode {:o}).",
                mode.unwrap_or(0)
            )));
            continue;
        }

        // Duplicates are judged for directories too, so `assets/` and
        // `Assets` cannot both appear.
        let folded = bare.to_lowercase();
        if let Some(first) = seen.get(&folded) {
            failures.push(Finding::container(format!(
                "{name} repeats the entry {first} (names are compared without case)."
            )));
            continue;
        }
        seen.insert(folded, name.clone());

        if is_dir {
            continue;
        }
        if size > limits.max_entry {
            failures.push(Finding::container(format!(
                "{name} is {size} bytes uncompressed; the limit is {}.",
                limits.max_entry
            )));
            continue;
        }
        // A zero-byte compressed stream with content, or a ratio above the
        // ceiling, is a decompression bomb.
        if size > 0 && (compressed == 0 || size > limits.max_ratio.saturating_mul(compressed)) {
            failures.push(Finding::container(format!(
                "{name} expands by more than {} times.",
                limits.max_ratio
            )));
            continue;
        }
        total = total.saturating_add(size);
        entries.push(Entry { name, index, size });
    }

    if total > limits.max_total {
        failures.push(Finding::container(format!(
            "The entries expand to {total} bytes; the limit is {}.",
            limits.max_total
        )));
    }
    if failures.is_empty() {
        Ok(entries)
    } else {
        Err(failures)
    }
}

/// Windows device names. A segment whose stem (text before its first dot) is
/// one of these, in any case, names a device on Windows whatever its extension.
const WINDOWS_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Path rules for every archive entry and every manifest-listed path,
/// applied to the name with any directory marker already removed.
fn check_entry_name(name: &str) -> Result<(), &'static str> {
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
        if segment.ends_with('.') || segment.ends_with(' ') {
            return Err("segment ends in a dot or space");
        }
        let stem = segment.split('.').next().unwrap_or(segment);
        if WINDOWS_DEVICE_NAMES
            .iter()
            .any(|device| stem.eq_ignore_ascii_case(device))
        {
            return Err("Windows device name");
        }
    }
    Ok(())
}

enum Parsed {
    Ready(Manifest),
    Unsupported(String),
}

fn bad_manifest(message: impl std::fmt::Display) -> Finding {
    Finding::container(format!("manifest.json: {message}"))
}

fn parse_manifest(value: &Value, kind: &str, version: u64) -> Result<Parsed, Refusal> {
    let object = value
        .as_object()
        .ok_or_else(|| bad_manifest("must be a JSON object"))?;
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(bad_manifest("format must be \"ordo-container\"").into());
    }
    match object.get("format_version").and_then(Value::as_u64) {
        Some(FORMAT_VERSION) => {}
        Some(newer) if newer > FORMAT_VERSION => {
            return Ok(Parsed::Unsupported(unsupported_message(
                "container",
                newer,
                FORMAT_VERSION,
            )));
        }
        _ => return Err(bad_manifest("format_version must be the integer 1").into()),
    }
    let kind_text = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_manifest("kind must be a string"))?;
    let found = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| bad_manifest("schema_version must be an integer"))?;
    if !KNOWN_KINDS.contains(&kind_text) {
        return Ok(Parsed::Unsupported(unsupported_message(
            kind_text, found, version,
        )));
    }
    if kind_text != kind {
        return Err(Finding::new(
            FindingCode::WrongKind,
            "",
            format!("This is a {kind_text} file, not a {kind} file."),
        )
        .into());
    }
    if found != version {
        return Ok(Parsed::Unsupported(unsupported_message(
            kind_text, found, version,
        )));
    }
    let manifest: Manifest = serde_json::from_value(value.clone())
        .map_err(|error| bad_manifest(format!("does not match the container format: {error}")))?;
    if manifest.payload.path != PAYLOAD_PATH {
        return Err(bad_manifest("payload.path must be \"payload.json\"").into());
    }
    let mut listed = BTreeSet::new();
    for file in std::iter::once(&manifest.payload).chain(manifest.assets.iter()) {
        check_entry_name(&file.path)
            .map_err(|reason| bad_manifest(format!("listed path {}: {reason}", file.path)))?;
        if !listed.insert(file.path.to_lowercase()) {
            return Err(bad_manifest(format!("lists {} more than once", file.path)).into());
        }
    }
    if let Some(asset) = manifest
        .assets
        .iter()
        .find(|asset| !asset.path.starts_with(ASSETS_PREFIX))
    {
        return Err(bad_manifest(format!("asset {} must live under assets/", asset.path)).into());
    }
    Ok(Parsed::Ready(manifest))
}

/// Reads one entry through a reader bounded by its declared size: it stops
/// one byte past, so a header that lies about its size cannot make the read
/// unbounded.
fn read_bounded(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    entry: &Entry,
    label: &str,
) -> Result<Vec<u8>, Finding> {
    let reader = zip
        .by_index(entry.index)
        .map_err(|error| Finding::container(format!("Reading {label}: {error}")))?;
    let mut bytes = Vec::new();
    let read = reader
        .take(entry.size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| Finding::container(format!("Reading {label}: {error}")))?;
    if read as u64 != entry.size {
        return Err(Finding::container(format!(
            "{label} is {read} bytes; its header says {}.",
            entry.size
        )));
    }
    Ok(bytes)
}

/// What a test overrides in a manifest it packs.
#[cfg(feature = "fault-injection")]
#[derive(Clone, Debug, Default)]
pub struct ManifestOverrides {
    /// The artifact kind; `song-import` when absent.
    pub kind: Option<String>,
    /// The schema version; `2` when absent.
    pub schema_version: Option<u64>,
    /// The container format version; `1` when absent.
    pub format_version: Option<u64>,
    /// A wrong payload hash to record.
    pub payload_sha256: Option<String>,
}

/// One entry of a hand-built test archive.
#[cfg(feature = "fault-injection")]
#[derive(Clone, Debug)]
pub struct RawEntry {
    /// The entry name, written as given (traversal names included).
    pub name: String,
    /// The entry's bytes; for a symbolic link, its target text.
    pub bytes: Vec<u8>,
    /// Write the entry as a symbolic link.
    pub symlink: bool,
}

#[cfg(feature = "fault-injection")]
impl RawEntry {
    /// A regular file entry.
    pub fn file(name: &str, bytes: &[u8]) -> Self {
        Self {
            name: name.to_owned(),
            bytes: bytes.to_vec(),
            symlink: false,
        }
    }
}

/// Builds a well-formed `.ordo` archive around `payload` for tests: the
/// manifest records the payload's size and hash (or the override), and every
/// listed asset is written.
#[cfg(feature = "fault-injection")]
pub fn pack_for_tests(
    manifest: &ManifestOverrides,
    payload: &[u8],
    assets: &[(&str, &[u8])],
) -> Vec<u8> {
    let file_ref = |path: &str, bytes: &[u8], sha: Option<&String>| {
        serde_json::json!({
            "path": path,
            "sha256": sha.cloned().unwrap_or_else(|| sha256_hex(bytes)),
            "size": bytes.len(),
        })
    };
    let manifest_value = serde_json::json!({
        "format": FORMAT,
        "format_version": manifest.format_version.unwrap_or(FORMAT_VERSION),
        "kind": manifest.kind.clone().unwrap_or_else(|| "song-import".to_owned()),
        "schema_version": manifest.schema_version.unwrap_or(2),
        "artifact_id": "test-artifact",
        "created_at": "2026-10-10T00:00:00Z",
        "payload": file_ref(PAYLOAD_PATH, payload, manifest.payload_sha256.as_ref()),
        "assets": assets
            .iter()
            .map(|(path, bytes)| file_ref(path, bytes, None))
            .collect::<Vec<_>>(),
    });
    let manifest_bytes = serde_json::to_vec(&manifest_value).unwrap_or_default();
    let mut entries = vec![
        RawEntry::file(MANIFEST_PATH, &manifest_bytes),
        RawEntry::file(PAYLOAD_PATH, payload),
    ];
    entries.extend(
        assets
            .iter()
            .map(|(path, bytes)| RawEntry::file(path, bytes)),
    );
    pack_raw_for_tests(&entries)
}

/// Builds a ZIP archive from exactly `entries`, with no manifest added, so a
/// test can write traversal names, case duplicates, links and bombs.
#[cfg(feature = "fault-injection")]
pub fn pack_raw_for_tests(entries: &[RawEntry]) -> Vec<u8> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for entry in entries {
        if entry.symlink {
            let target = String::from_utf8_lossy(&entry.bytes).into_owned();
            let _ = zip.add_symlink(entry.name.as_str(), target, options);
        } else {
            let _ = zip.start_file(entry.name.as_str(), options);
            let _ = zip.write_all(&entry.bytes);
        }
    }
    zip.finish().map(Cursor::into_inner).unwrap_or_default()
}
