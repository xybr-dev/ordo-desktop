//! Payload contract validation (T0.8 WP2).
//!
//! Each contract has a typed validator that mirrors `contracts/<kind>/v1.schema.json`.
//! Checks run in two phases: shape first (JSON pointers for every missing, unknown,
//! or mistyped member, collected rather than stopping at the first), then semantic
//! rules (internal references and hashes) once the shape is clean. The rules are
//! documented in `docs/contracts/README.md`.

mod backup;
mod celebration;
mod diagnostic;
mod journal;
mod liturgy;
mod snapshot;
mod song;
mod support;
#[cfg(test)]
mod tests;

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

/// Validates `doc` against `contract`, collecting every finding.
pub fn validate_contract(contract: Contract, doc: &Value) -> Result<(), Vec<ContractError>> {
    let mut errors = Vec::new();
    match contract {
        Contract::SongImport => song::check(doc, &mut errors),
        Contract::LiturgyImport => liturgy::check(doc, &mut errors),
        Contract::SnapshotManifest => snapshot::check(doc, &mut errors),
        Contract::CelebrationPackage => celebration::check(doc, &mut errors),
        Contract::ParishBackup => backup::check(doc, &mut errors),
        Contract::DiagnosticReport => diagnostic::check(doc, &mut errors),
        Contract::RecoveryJournal => journal::check(doc, &mut errors),
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The top-level (required, optional) member names each validator declares.
/// The schema drift guard compares these with the schema files.
#[cfg(test)]
fn declared_keys(contract: Contract) -> (&'static [&'static str], &'static [&'static str]) {
    match contract {
        Contract::SongImport => (song::REQUIRED, song::OPTIONAL),
        Contract::LiturgyImport => (liturgy::REQUIRED, liturgy::OPTIONAL),
        Contract::SnapshotManifest => (snapshot::REQUIRED, snapshot::OPTIONAL),
        Contract::CelebrationPackage => (celebration::REQUIRED, celebration::OPTIONAL),
        Contract::ParishBackup => (backup::REQUIRED, backup::OPTIONAL),
        Contract::DiagnosticReport => (diagnostic::REQUIRED, diagnostic::OPTIONAL),
        Contract::RecoveryJournal => (journal::REQUIRED, journal::OPTIONAL),
    }
}
