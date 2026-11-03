//! Development-only spike for the universal `.ordo` ZIP container (T0.8 WP1).
//!
//! A `.ordo` file is a ZIP archive with a required root `manifest.json`, one
//! structured JSON payload, and optional `assets/` files. [`create`] writes one
//! atomically; [`inspect`] checks an archive before anything is extracted, then
//! stages verified files into a caller-provided directory and routes it by the
//! manifest's artifact kind. Nothing here touches a Parish: the API has no
//! Parish argument, so "no Parish mutation" is structural.

pub mod contracts;
mod create;
mod inspect;
#[cfg(test)]
mod tests;

pub use contracts::{Contract, ContractError, validate_contract};
pub use create::{CreateError, create};
pub use inspect::{Failure, InspectLimits, Inspection, Routed, inspect};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

/// The `format` string every root manifest must carry.
pub const FORMAT: &str = "ordo-container";
/// The only container format version this build reads and writes.
pub const FORMAT_VERSION: u64 = 1;
/// The required root manifest entry.
pub const MANIFEST_PATH: &str = "manifest.json";
/// The one payload entry a manifest may list.
pub const PAYLOAD_PATH: &str = "payload.json";
/// Prefix every listed asset must carry.
pub const ASSETS_PREFIX: &str = "assets/";
/// Schema versions this build supports for every kind.
pub const SUPPORTED_VERSIONS: &[u64] = &[1];

/// Container kinds. Serialized as kebab-case strings, which is what appears in
/// `manifest.json` and in CLI arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    SongImport,
    LiturgyImport,
    CelebrationPackage,
    ParishBackup,
    DiagnosticReport,
}

impl ArtifactKind {
    /// Every kind this build knows, in a stable order.
    pub const ALL: [ArtifactKind; 5] = [
        ArtifactKind::SongImport,
        ArtifactKind::LiturgyImport,
        ArtifactKind::CelebrationPackage,
        ArtifactKind::ParishBackup,
        ArtifactKind::DiagnosticReport,
    ];

    /// The kebab-case name used in manifests and on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::SongImport => "song-import",
            ArtifactKind::LiturgyImport => "liturgy-import",
            ArtifactKind::CelebrationPackage => "celebration-package",
            ArtifactKind::ParishBackup => "parish-backup",
            ArtifactKind::DiagnosticReport => "diagnostic-report",
        }
    }

    /// The payload contract that validates this kind's `payload.json`.
    pub fn contract(self) -> Contract {
        match self {
            ArtifactKind::SongImport => Contract::SongImport,
            ArtifactKind::LiturgyImport => Contract::LiturgyImport,
            ArtifactKind::CelebrationPackage => Contract::CelebrationPackage,
            ArtifactKind::ParishBackup => Contract::ParishBackup,
            ArtifactKind::DiagnosticReport => Contract::DiagnosticReport,
        }
    }
}

impl FromStr for ArtifactKind {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        ArtifactKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| format!("unknown artifact kind `{text}`"))
    }
}

/// A file listed in the manifest: its archive path, lowercase hex SHA-256, and
/// exact uncompressed size in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// The root `manifest.json`. Every file in the archive must be listed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub format_version: u64,
    pub kind: ArtifactKind,
    pub schema_version: u64,
    /// Stable UUID string, assigned at creation.
    pub artifact_id: String,
    /// RFC 3339 UTC timestamp, assigned at creation.
    pub created_at: String,
    pub payload: FileRef,
    pub assets: Vec<FileRef>,
}

/// Lowercase hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Process-wide counter that keeps temp names unique within one process.
static NEXT_UNIQUE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn next_unique() -> u64 {
    NEXT_UNIQUE.fetch_add(1, Ordering::Relaxed)
}

/// A uniquely named directory under the OS temp dir, removed on drop. Used for
/// inspection staging and for the reopen check inside [`create`].
#[derive(Debug)]
pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Creates `<temp>/ordo-artifact-<label>-<pid>-<n>`.
    pub fn new(label: &str) -> io::Result<Self> {
        let path = env::temp_dir().join(format!(
            "ordo-artifact-{label}-{}-{}",
            std::process::id(),
            next_unique()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Cleanup is best effort; a leftover scratch dir is not a correctness
        // problem and must not panic during unwinding.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Human-readable refusal for a container from a newer or unknown kind.
pub fn unsupported_message(kind: &str, version: u64, supported: &[u64]) -> String {
    let supported: Vec<String> = supported.iter().map(|v| format!("v{v}")).collect();
    format!(
        "requires an Ordo version supporting {kind} v{version}; this build supports {}",
        supported.join(", ")
    )
}

/// Compact RFC 3339 UTC timestamp for `unix_seconds`, using the civil-calendar
/// conversion from Howard Hinnant's date algorithms (no clock or date crate).
pub(crate) fn rfc3339_utc(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let seconds_of_day = unix_seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3_600,
        (seconds_of_day / 60) % 60,
        seconds_of_day % 60
    )
}
