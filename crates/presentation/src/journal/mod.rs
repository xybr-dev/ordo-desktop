//! The compact recovery journal record (`recovery-journal` v2), its codec and
//! its validation against a loaded snapshot. See
//! [Decision 314](../../../../docs/decisions/0314-compact-recovery-journal-v2.md).
//!
//! This module is pure: it turns a [`RecoveryPoint`] into bytes and back, and
//! checks a decoded point against a [`LoadedSnapshot`]. It never touches a
//! file; the writer worker and the startup read live in the `ordo` crate.
//!
//! # Canonical form and `record_sha256`
//!
//! The canonical record is this codec's own serialization: members in schema
//! order, no whitespace, `serde_json` string escaping. `record_sha256` is the
//! SHA-256 of that canonical text with the `record_sha256` member removed, and
//! it is written as the last member. [`decode`] re-serializes what it parsed
//! and compares, so insignificant whitespace in a file does not matter but any
//! changed value does.

use std::fmt;

use domain::presentation::{ContentHash, CueId, EntityId, SnapshotId};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::snapshot_reader::LoadedSnapshot;

mod recorder;
pub use recorder::{JournalRecorder, SessionIdentity};

/// The only `schema_version` this build reads and writes.
pub const SCHEMA_VERSION: u32 = 2;

/// Why a journal record cannot be used. Each reason is typed so Diagnostics
/// can name it; none of them is guessed around (Decision 314).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalRejection {
    /// Unparseable, wrong-shaped, or `record_sha256` does not match.
    Torn,
    /// A well-formed `schema_version` other than 2.
    UnknownVersion(u32),
    /// `ended` is true: the session ended cleanly. Age is never a reason.
    Stale,
    /// The snapshot id or manifest hash differs from the loaded snapshot.
    SnapshotMismatch,
    /// A Preview or Program `cue_id` is not in the snapshot.
    UnknownCue,
}

impl fmt::Display for JournalRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Torn => f.write_str("the recovery journal is torn or unreadable"),
            Self::UnknownVersion(n) => {
                write!(f, "the recovery journal has unknown schema version {n}")
            }
            Self::Stale => f.write_str("the recovery journal records a session that ended"),
            Self::SnapshotMismatch => {
                f.write_str("the recovery journal belongs to a different snapshot")
            }
            Self::UnknownCue => f.write_str("the recovery journal names a cue the snapshot lacks"),
        }
    }
}

impl std::error::Error for JournalRejection {}

/// A lowercase hyphenated UUID v4 naming one Presentation session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionId(String);

impl SessionId {
    /// Accept exactly the canonical 36-character lowercase UUID v4 form.
    pub fn parse(text: &str) -> Result<Self, JournalRejection> {
        EntityId::parse(text).map_err(|_| JournalRejection::Torn)?;
        Ok(Self(text.to_owned()))
    }

    /// The canonical text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A UTC timestamp, `YYYY-MM-DDTHH:MM:SSZ`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrittenAt(String);

impl WrittenAt {
    /// Accept exactly the contract's timestamp shape.
    pub fn parse(text: &str) -> Result<Self, JournalRejection> {
        const SHAPE: &[u8; 20] = b"dddd-dd-ddTdd:dd:ddZ";
        let ok = text.len() == SHAPE.len()
            && text.bytes().zip(SHAPE).all(|(byte, shape)| match shape {
                b'd' => byte.is_ascii_digit(),
                other => byte == *other,
            });
        if ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(JournalRejection::Torn)
        }
    }

    /// The timestamp text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// UTC `YYYY-MM-DDTHH:MM:SSZ` for seconds since the Unix epoch
    /// (civil-from-days, proleptic Gregorian, so no date crate is needed).
    /// Seconds past 9999-12-31T23:59:59Z are clamped there, so the result
    /// always has the contract's four-digit year and always parses.
    pub fn from_unix_seconds(seconds: u64) -> Self {
        /// 9999-12-31T23:59:59Z, the last second the timestamp shape can spell.
        const LAST_REPRESENTABLE: u64 = 253_402_300_799;
        let seconds = seconds.min(LAST_REPRESENTABLE);
        let days = seconds / 86_400;
        let hour = (seconds % 86_400) / 3_600;
        let minute = (seconds % 3_600) / 60;
        let second = seconds % 60;

        // Howard Hinnant's civil_from_days, with days shifted so that era 0
        // starts on 0000-03-01.
        let shifted = days + 719_468;
        let era = shifted / 146_097;
        let day_of_era = shifted % 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let march_based_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * march_based_month + 2) / 5 + 1;
        let month = if march_based_month < 10 {
            march_based_month + 3
        } else {
            march_based_month - 9
        };
        let year = year_of_era + era * 400 + u64::from(month <= 2);

        Self(format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
        ))
    }
}

/// The live state the journal records (`state`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveIntent {
    OutputDisabled,
    EnabledBlack,
    Live,
    Frozen,
    Blackout,
    Disconnected,
}

impl LiveIntent {
    /// The contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OutputDisabled => "output-disabled",
            Self::EnabledBlack => "enabled-black",
            Self::Live => "live",
            Self::Frozen => "frozen",
            Self::Blackout => "blackout",
            Self::Disconnected => "disconnected",
        }
    }

    /// The journal's spelling of an Engine live state. Both `Disconnected`
    /// values journal as `disconnected`: `display_back` is not recorded,
    /// because the output stays black until the operator acts (Decision 272).
    pub fn of(live: crate::state::LiveState) -> Self {
        use crate::state::LiveState;
        match live {
            LiveState::OutputDisabled => Self::OutputDisabled,
            LiveState::EnabledBlack => Self::EnabledBlack,
            LiveState::Live => Self::Live,
            LiveState::Frozen => Self::Frozen,
            LiveState::Blackout => Self::Blackout,
            LiveState::Disconnected { .. } => Self::Disconnected,
        }
    }

    fn from_wire(text: &str) -> Option<Self> {
        Some(match text {
            "output-disabled" => Self::OutputDisabled,
            "enabled-black" => Self::EnabledBlack,
            "live" => Self::Live,
            "frozen" => Self::Frozen,
            "blackout" => Self::Blackout,
            "disconnected" => Self::Disconnected,
            _ => return None,
        })
    }
}

/// Display bounds in the display adapter's coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundsRecord {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Evidence for the display the congregation output was assigned to. It
/// mirrors the platform's display evidence without depending on `platform`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayRecord {
    pub name: Option<String>,
    pub uuid: Option<String>,
    pub bounds: BoundsRecord,
}

/// Output enablement and the assigned display, if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputEvidence {
    pub enabled: bool,
    pub display: Option<DisplayRecord>,
}

/// One compact recovery record: the whole state needed to reopen a session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryPoint {
    pub session_id: SessionId,
    /// Strictly increasing within a session.
    pub seq: u64,
    pub written_at: WrittenAt,
    pub snapshot_id: SnapshotId,
    pub manifest_hash: ContentHash,
    /// Directory the snapshot was loaded from; must not be empty.
    pub source_dir: String,
    pub preview: CueId,
    /// The last valid Program Cue, or `None` when nothing was ever live.
    pub program: Option<CueId>,
    pub state: LiveIntent,
    pub output: OutputEvidence,
    /// True only on a clean end.
    pub ended: bool,
}

/// A point that matches its snapshot, with flat indexes in the Engine's Cue
/// order (`PresentationSnapshot::positions`).
///
/// Only [`validate`] constructs one, so holding this type is the proof that
/// the record was checked against the snapshot it will reopen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedRecovery {
    point: RecoveryPoint,
    preview: usize,
    program: Option<usize>,
}

impl ValidatedRecovery {
    /// The validated journal record.
    pub fn point(&self) -> &RecoveryPoint {
        &self.point
    }

    /// Flat index of the Preview Cue.
    pub fn preview(&self) -> usize {
        self.preview
    }

    /// Flat index of the last valid Program Cue, if anything had been live.
    pub fn program(&self) -> Option<usize> {
        self.program
    }
}

#[cfg(test)]
impl ValidatedRecovery {
    /// Builds a validated recovery by hand for Engine tests that have no
    /// [`LoadedSnapshot`]. Production code can only get one from [`validate`].
    pub(crate) fn for_test(point: RecoveryPoint, preview: usize, program: Option<usize>) -> Self {
        Self {
            point,
            preview,
            program,
        }
    }
}

// ---- wire form: field order is the schema order and the canonical order ----

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    session_id: String,
    seq: u64,
    written_at: String,
    snapshot: WireSnapshot,
    preview: WireCue,
    #[serde(deserialize_with = "present")]
    program: Option<WireCue>,
    state: String,
    output: WireOutput,
    ended: bool,
    /// Never serialized: it is computed over the serialization of the rest.
    #[serde(skip_serializing)]
    record_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireSnapshot {
    id: String,
    manifest_hash: String,
    source_dir: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireCue {
    cue_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireOutput {
    enabled: bool,
    #[serde(deserialize_with = "present")]
    display: Option<WireDisplay>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireDisplay {
    #[serde(deserialize_with = "present")]
    name: Option<String>,
    #[serde(deserialize_with = "present")]
    uuid: Option<String>,
    bounds: WireBounds,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Makes a nullable member required: serde otherwise treats a missing
/// `Option` member as `None`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn digest_hex(bytes: &[u8]) -> String {
    ContentHash::from_bytes(Sha256::digest(bytes).into()).to_string()
}

fn to_wire(point: &RecoveryPoint) -> Wire {
    Wire {
        schema_version: SCHEMA_VERSION,
        session_id: point.session_id.0.clone(),
        seq: point.seq,
        written_at: point.written_at.0.clone(),
        snapshot: WireSnapshot {
            id: point.snapshot_id.to_string(),
            manifest_hash: point.manifest_hash.to_string(),
            source_dir: point.source_dir.clone(),
        },
        preview: WireCue {
            cue_id: point.preview.to_string(),
        },
        program: point.program.map(|cue| WireCue {
            cue_id: cue.to_string(),
        }),
        state: point.state.as_str().to_owned(),
        output: WireOutput {
            enabled: point.output.enabled,
            display: point.output.display.as_ref().map(|display| WireDisplay {
                name: display.name.clone(),
                uuid: display.uuid.clone(),
                bounds: WireBounds {
                    x: display.bounds.x,
                    y: display.bounds.y,
                    width: display.bounds.width,
                    height: display.bounds.height,
                },
            }),
        },
        ended: point.ended,
        record_sha256: String::new(),
    }
}

/// The canonical text without `record_sha256`, which closes with `}`.
fn canonical_body(wire: &Wire) -> Vec<u8> {
    // Plain structs of strings and integers always serialize.
    serde_json::to_vec(wire).expect("journal wire values serialize")
}

/// Encode a point as its canonical record, with `record_sha256` embedded.
pub fn encode(point: &RecoveryPoint) -> Vec<u8> {
    let mut bytes = canonical_body(&to_wire(point));
    let hash = digest_hex(&bytes);
    bytes.pop(); // the closing brace
    bytes.extend_from_slice(b",\"record_sha256\":\"");
    bytes.extend_from_slice(hash.as_bytes());
    bytes.extend_from_slice(b"\"}");
    bytes
}

/// Decode a record. The version is checked before the shape, so a v1 or v3
/// file reports [`JournalRejection::UnknownVersion`] and not `Torn`.
pub fn decode(bytes: &[u8]) -> Result<RecoveryPoint, JournalRejection> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| JournalRejection::Torn)?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(JournalRejection::Torn)?;
    if version != u64::from(SCHEMA_VERSION) {
        return Err(match u32::try_from(version) {
            Ok(n) => JournalRejection::UnknownVersion(n),
            Err(_) => JournalRejection::Torn,
        });
    }
    let wire: Wire = serde_json::from_value(value).map_err(|_| JournalRejection::Torn)?;
    if digest_hex(&canonical_body(&wire)) != wire.record_sha256 {
        return Err(JournalRejection::Torn);
    }
    from_wire(wire)
}

fn from_wire(wire: Wire) -> Result<RecoveryPoint, JournalRejection> {
    fn torn<E>(_: E) -> JournalRejection {
        JournalRejection::Torn
    }
    if wire.snapshot.source_dir.is_empty() {
        return Err(JournalRejection::Torn);
    }
    let display = wire.output.display.map(|display| DisplayRecord {
        name: display.name,
        uuid: display.uuid,
        bounds: BoundsRecord {
            x: display.bounds.x,
            y: display.bounds.y,
            width: display.bounds.width,
            height: display.bounds.height,
        },
    });
    Ok(RecoveryPoint {
        session_id: SessionId::parse(&wire.session_id)?,
        seq: wire.seq,
        written_at: WrittenAt::parse(&wire.written_at)?,
        snapshot_id: SnapshotId::parse(&wire.snapshot.id).map_err(torn)?,
        manifest_hash: ContentHash::parse(&wire.snapshot.manifest_hash).map_err(torn)?,
        source_dir: wire.snapshot.source_dir,
        preview: CueId::parse(&wire.preview.cue_id).map_err(torn)?,
        program: wire
            .program
            .map(|cue| CueId::parse(&cue.cue_id).map_err(torn))
            .transpose()?,
        state: LiveIntent::from_wire(&wire.state).ok_or(JournalRejection::Torn)?,
        output: OutputEvidence {
            enabled: wire.output.enabled,
            display,
        },
        ended: wire.ended,
    })
}

/// Check a decoded point against the snapshot it will reopen.
///
/// Flat indexes come from `PresentationSnapshot::positions`, the same
/// flattened order the Engine uses for Preview and Program.
pub fn validate(
    point: RecoveryPoint,
    snapshot: &LoadedSnapshot,
) -> Result<ValidatedRecovery, JournalRejection> {
    if point.ended {
        return Err(JournalRejection::Stale);
    }
    if point.snapshot_id != snapshot.snapshot().id()
        || point.manifest_hash != snapshot.manifest_hash()
    {
        return Err(JournalRejection::SnapshotMismatch);
    }
    let positions = snapshot.snapshot().positions();
    let flat_of = |cue: CueId| positions.iter().position(|p| p.cue_id == cue);
    let preview = flat_of(point.preview).ok_or(JournalRejection::UnknownCue)?;
    let program = point
        .program
        .map(|cue| flat_of(cue).ok_or(JournalRejection::UnknownCue))
        .transpose()?;
    Ok(ValidatedRecovery {
        point,
        preview,
        program,
    })
}

#[cfg(test)]
mod tests;
