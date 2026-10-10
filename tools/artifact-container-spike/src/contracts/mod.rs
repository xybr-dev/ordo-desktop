//! Payload contract validation.
//!
//! WP1 ships only this stub: every contract checks that the document is a JSON
//! object with `schema_version == 1`. WP2 replaces it with a typed validator per
//! schema that mirrors `contracts/<kind>/v1.schema.json`. The signature is
//! final, so callers do not change when the checks grow.

use serde_json::Value;

/// Every versioned contract in the T0.8 set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contract {
    SongImport,
    LiturgyImport,
    SnapshotManifest,
    CelebrationPackage,
    ParishBackup,
    DiagnosticReport,
    RecoveryJournal,
}

impl Contract {
    /// Human-readable name used in error messages.
    pub fn name(self) -> &'static str {
        match self {
            Contract::SongImport => "song-import",
            Contract::LiturgyImport => "liturgy-import",
            Contract::SnapshotManifest => "snapshot-manifest",
            Contract::CelebrationPackage => "celebration-package",
            Contract::ParishBackup => "parish-backup",
            Contract::DiagnosticReport => "diagnostic-report",
            Contract::RecoveryJournal => "recovery-journal",
        }
    }
}

/// One validation finding. `code` is one of the stable codes the fixtures
/// name: `schema`, `unsupported-version`, `missing-reference`, `hash-mismatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    pub code: &'static str,
    /// JSON pointer to the offending value, or empty for the document itself.
    pub path: String,
    pub message: String,
}

/// Validates `doc` against `contract`. Stub: object shape and version only.
pub fn validate_contract(contract: Contract, doc: &Value) -> Result<(), Vec<ContractError>> {
    let Some(object) = doc.as_object() else {
        return Err(vec![ContractError {
            code: "schema",
            path: String::new(),
            message: format!("{} document must be a JSON object", contract.name()),
        }]);
    };
    match object.get("schema_version") {
        Some(version) if version.as_u64() == Some(1) => Ok(()),
        Some(version) if version.as_u64().is_some() => Err(vec![ContractError {
            code: "unsupported-version",
            path: "/schema_version".to_owned(),
            message: format!(
                "{} schema_version {version} is not supported by this build (v1)",
                contract.name()
            ),
        }]),
        _ => Err(vec![ContractError {
            code: "schema",
            path: "/schema_version".to_owned(),
            message: format!("{} requires an integer schema_version", contract.name()),
        }]),
    }
}
