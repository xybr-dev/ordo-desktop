//! The manifest grammar: `snapshot.json` (`snapshot-manifest` v2) parsed into
//! declarations, and the declarations assembled into the domain snapshot.
//!
//! Parsing is two-stage. The wire structs mirror the JSON exactly, keep every
//! identity and digest as the raw string, and refuse unknown fields (and, by
//! serde's derive, a field that appears twice). The conversion stage then
//! checks each value against its domain rule and names the JSON Pointer of the
//! first one that fails, so a message points at the spot to repair.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use domain::presentation::{
    AssetId, AutoPlay, ContentHash, Cue, CueBehavior, CueId, EntityId, FrameId, IdError, MassItem,
    MassItemId, MassItemKind, MassSection, MassSectionId, PresentationSnapshot, SnapshotError,
    SnapshotId, ThemeId, ThemeKind, ThemeRef,
};
use serde::{Deserialize, Deserializer};

use super::{
    ASSET_DIRECTORY, FRAME_DIRECTORY, MANIFEST_SCHEMA_VERSION, ManifestReason, is_snapshot_relative,
};

/// One manifest as parsed, before any file is opened.
pub(super) struct Manifest {
    pub(super) snapshot: SnapshotId,
    pub(super) title: String,
    pub(super) fingerprint: ContentHash,
    pub(super) themes: Vec<ThemeRef>,
    pub(super) mass_theme: ThemeId,
    pub(super) frames: Vec<FrameDecl>,
    pub(super) assets: Vec<AssetDecl>,
    pub(super) sections: Vec<SectionDecl>,
}

/// One declared dependency. `S` is what only some kinds declare beside the
/// digest: nothing for a frame, the byte length for an asset.
pub(super) struct DependencyDecl<T, S> {
    pub(super) id: T,
    pub(super) path: String,
    pub(super) size: S,
    pub(super) hash: ContentHash,
}

/// A frame declares no length: its bytes are checked by digest and format.
pub(super) type FrameDecl = DependencyDecl<FrameId, ()>;

/// An asset declares its exact byte length beside its digest.
pub(super) type AssetDecl = DependencyDecl<AssetId, u32>;

pub(super) struct SectionDecl {
    id: MassSectionId,
    title: String,
    items: Vec<ItemDecl>,
}

struct ItemDecl {
    id: MassItemId,
    title: String,
    kind: MassItemKind,
    theme: Option<ThemeId>,
    auto_play: Option<u32>,
    cues: Vec<CueDecl>,
}

struct CueDecl {
    id: CueId,
    label: String,
    private_context: Option<String>,
    behavior: CueBehavior,
}

// ---------------------------------------------------------------------------
// Wire shape. Every struct denies unknown fields; a duplicated key is a serde
// "duplicate field" error for the same reason. Optional members are absent or
// present with a value, never `null`, so one fact has one spelling.
// ---------------------------------------------------------------------------

/// Reads only `schema_version`, ignoring every other member, so a manifest
/// from another version is refused as such before its other fields (which may
/// be unknown to this build) are judged.
#[derive(Deserialize)]
struct VersionProbe {
    #[serde(default, deserialize_with = "present")]
    schema_version: Option<u64>,
}

/// A member that is either absent or has a value; `null` is a type error.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    #[serde(rename = "schema_version")]
    _schema_version: u64,
    id: String,
    mass_plan_id: String,
    #[serde(default, deserialize_with = "present")]
    celebration_id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    captured_at: Option<String>,
    title: String,
    themes: Vec<ThemeWire>,
    mass_theme_id: String,
    sections: Vec<SectionWire>,
    frames: Vec<FrameWire>,
    assets: Vec<AssetWire>,
    dependencies_fingerprint: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeWire {
    id: String,
    name: String,
    kind: ThemeKindWire,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ThemeKindWire {
    Custom,
    Fallback,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionWire {
    id: String,
    title: String,
    items: Vec<ItemWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemWire {
    id: String,
    title: String,
    kind: ItemKindWire,
    #[serde(default, deserialize_with = "present")]
    theme_id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    auto_play_seconds: Option<u32>,
    cues: Vec<CueWire>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ItemKindWire {
    Song,
    Reading,
    Psalm,
    ResponseSequence,
    GeneralPresentation,
    Rite,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CueWire {
    id: String,
    label: String,
    #[serde(default, deserialize_with = "present")]
    private_context: Option<String>,
    behavior: BehaviorWire,
}

/// Every variant is a struct variant, even the empty ones: serde ignores
/// extra members on a unit variant of an internally tagged enum, but it
/// enforces `deny_unknown_fields` on a struct variant.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum BehaviorWire {
    Project {
        frame_id: String,
        #[serde(default, deserialize_with = "present")]
        duration_seconds: Option<u32>,
    },
    OperatorTrigger {},
    Hold {},
    Wait {},
    Unavailable {
        reason: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameWire {
    id: String,
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetWire {
    id: String,
    path: String,
    size: u32,
    sha256: String,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse one `snapshot.json`. The caller has already enforced the size cap
/// and UTF-8; this checks the byte-level rules that keep one snapshot to one
/// byte sequence, then the version, then the structure.
pub(super) fn parse_manifest(text: &str) -> Result<Manifest, ManifestReason> {
    if text.starts_with('\u{feff}') {
        return Err(ManifestReason::ByteOrderMark);
    }
    if let Some(offset) = text.find('\r') {
        return Err(ManifestReason::CarriageReturn {
            line: text[..offset].matches('\n').count() + 1,
        });
    }
    if !text.ends_with('\n') {
        return Err(ManifestReason::MissingFinalNewline);
    }

    let probe: VersionProbe = serde_json::from_str(text).map_err(malformed)?;
    if let Some(found) = probe.schema_version
        && found != MANIFEST_SCHEMA_VERSION
    {
        return Err(ManifestReason::UnsupportedVersion { found });
    }

    let wire: ManifestWire = serde_json::from_str(text).map_err(malformed)?;
    convert(wire)
}

fn malformed(error: serde_json::Error) -> ManifestReason {
    // serde_json appends " at line L column C"; the position is carried as
    // fields instead, so the message is only the cause.
    let full = error.to_string();
    let suffix = format!(" at line {} column {}", error.line(), error.column());
    let message = full.strip_suffix(&suffix).unwrap_or(&full).to_owned();
    ManifestReason::Malformed {
        line: error.line(),
        column: error.column(),
        message,
    }
}

fn convert(wire: ManifestWire) -> Result<Manifest, ManifestReason> {
    let snapshot = id::<SnapshotId>(&wire.id, || "/id".to_owned())?;
    // The Mass plan and Celebration identities and the capture time describe
    // where the snapshot came from. The domain snapshot does not carry them, so
    // they are validated for shape and then dropped.
    id::<EntityId>(&wire.mass_plan_id, || "/mass_plan_id".to_owned())?;
    if let Some(celebration) = &wire.celebration_id {
        id::<EntityId>(celebration, || "/celebration_id".to_owned())?;
    }
    if let Some(captured) = wire.captured_at
        && !is_utc_timestamp(&captured)
    {
        return Err(ManifestReason::InvalidTimestamp {
            at: "/captured_at".to_owned(),
            value: captured,
        });
    }
    let title = text(wire.title, || "/title".to_owned())?;
    let fingerprint = hash(&wire.dependencies_fingerprint, || {
        "/dependencies_fingerprint".to_owned()
    })?;

    let mut themes = Vec::with_capacity(wire.themes.len());
    for (index, theme) in wire.themes.into_iter().enumerate() {
        themes.push(ThemeRef {
            id: id(&theme.id, || format!("/themes/{index}/id"))?,
            name: text(theme.name, || format!("/themes/{index}/name"))?,
            kind: match theme.kind {
                ThemeKindWire::Custom => ThemeKind::Custom,
                ThemeKindWire::Fallback => ThemeKind::Fallback,
            },
        });
    }
    let mass_theme = id::<ThemeId>(&wire.mass_theme_id, || "/mass_theme_id".to_owned())?;

    let mut paths = BTreeSet::new();
    let mut frame_ids = BTreeSet::new();
    let mut frames = Vec::with_capacity(wire.frames.len());
    for (index, frame) in wire.frames.into_iter().enumerate() {
        frames.push(dependency(
            DependencyWire {
                id: &frame.id,
                path: frame.path,
                sha256: &frame.sha256,
                size: (),
            },
            &DependencyKind {
                list: "frames",
                directory: FRAME_DIRECTORY,
                index,
            },
            &mut frame_ids,
            &mut paths,
        )?);
    }
    let mut asset_ids = BTreeSet::new();
    let mut assets = Vec::with_capacity(wire.assets.len());
    for (index, asset) in wire.assets.into_iter().enumerate() {
        assets.push(dependency(
            DependencyWire {
                id: &asset.id,
                path: asset.path,
                sha256: &asset.sha256,
                size: asset.size,
            },
            &DependencyKind {
                list: "assets",
                directory: ASSET_DIRECTORY,
                index,
            },
            &mut asset_ids,
            &mut paths,
        )?);
    }

    let mut sections = Vec::with_capacity(wire.sections.len());
    for (s, section) in wire.sections.into_iter().enumerate() {
        let mut items = Vec::with_capacity(section.items.len());
        for (i, item) in section.items.into_iter().enumerate() {
            let at = |tail: &str| format!("/sections/{s}/items/{i}/{tail}");
            let mut cues = Vec::with_capacity(item.cues.len());
            for (c, cue) in item.cues.into_iter().enumerate() {
                cues.push(convert_cue(cue, |tail| {
                    format!("/sections/{s}/items/{i}/cues/{c}/{tail}")
                })?);
            }
            items.push(ItemDecl {
                id: id(&item.id, || at("id"))?,
                title: text(item.title, || at("title"))?,
                kind: match item.kind {
                    ItemKindWire::Song => MassItemKind::Song,
                    ItemKindWire::Reading => MassItemKind::Reading,
                    ItemKindWire::Psalm => MassItemKind::Psalm,
                    ItemKindWire::ResponseSequence => MassItemKind::ResponseSequence,
                    ItemKindWire::GeneralPresentation => MassItemKind::GeneralPresentation,
                    ItemKindWire::Rite => MassItemKind::Rite,
                },
                theme: item
                    .theme_id
                    .as_deref()
                    .map(|theme| id(theme, || at("theme_id")))
                    .transpose()?,
                auto_play: item.auto_play_seconds,
                cues,
            });
        }
        sections.push(SectionDecl {
            id: id(&section.id, || format!("/sections/{s}/id"))?,
            title: text(section.title, || format!("/sections/{s}/title"))?,
            items,
        });
    }

    Ok(Manifest {
        snapshot,
        title,
        fingerprint,
        themes,
        mass_theme,
        frames,
        assets,
        sections,
    })
}

fn convert_cue(cue: CueWire, at: impl Fn(&str) -> String) -> Result<CueDecl, ManifestReason> {
    let behavior = match cue.behavior {
        BehaviorWire::Project {
            frame_id,
            duration_seconds,
        } => CueBehavior::Project {
            frame: id(&frame_id, || at("behavior/frame_id"))?,
            duration_seconds,
        },
        BehaviorWire::OperatorTrigger {} => CueBehavior::OperatorTrigger,
        BehaviorWire::Hold {} => CueBehavior::Hold,
        BehaviorWire::Wait {} => CueBehavior::Wait,
        BehaviorWire::Unavailable { reason } => CueBehavior::Unavailable {
            reason: text(reason, || at("behavior/reason"))?,
        },
    };
    Ok(CueDecl {
        id: id(&cue.id, || at("id"))?,
        label: text(cue.label, || at("label"))?,
        private_context: cue
            .private_context
            .map(|context| text(context, || at("private_context")))
            .transpose()?,
        behavior,
    })
}

/// A wire dependency with its identity and digest still raw.
struct DependencyWire<'a, S> {
    id: &'a str,
    path: String,
    sha256: &'a str,
    size: S,
}

/// Where one dependency sits: its list name, required directory, and index.
struct DependencyKind {
    list: &'static str,
    directory: &'static str,
    index: usize,
}

/// Convert one dependency, rejecting a repeated identity or a path any earlier
/// dependency (of either kind) already names. Paths are compared
/// ASCII-lowercased, because the snapshot must also open on a case-insensitive
/// filesystem where `Frames/A.txt` and `frames/a.txt` are one file.
fn dependency<T, S>(
    wire: DependencyWire<'_, S>,
    kind: &DependencyKind,
    ids: &mut BTreeSet<T>,
    paths: &mut BTreeSet<String>,
) -> Result<DependencyDecl<T, S>, ManifestReason>
where
    T: FromStr<Err = IdError> + fmt::Display + Ord + Copy,
{
    let DependencyKind {
        list,
        directory,
        index,
    } = *kind;
    let id = id::<T>(wire.id, || format!("/{list}/{index}/id"))?;
    let under_directory = wire
        .path
        .strip_prefix(directory)
        .is_some_and(|rest| rest.starts_with('/'));
    if !under_directory || !is_snapshot_relative(&wire.path) {
        return Err(ManifestReason::UnsafePath { path: wire.path });
    }
    let hash = hash(wire.sha256, || format!("/{list}/{index}/sha256"))?;
    if !ids.insert(id) {
        return Err(ManifestReason::Duplicate {
            field: list,
            value: id.to_string(),
        });
    }
    if !paths.insert(wire.path.to_ascii_lowercase()) {
        return Err(ManifestReason::DuplicatePath { path: wire.path });
    }
    Ok(DependencyDecl {
        id,
        path: wire.path,
        size: wire.size,
        hash,
    })
}

fn id<T>(value: &str, at: impl FnOnce() -> String) -> Result<T, ManifestReason>
where
    T: FromStr<Err = IdError>,
{
    T::from_str(value).map_err(|error| ManifestReason::InvalidId {
        at: at(),
        value: value.to_owned(),
        error,
    })
}

fn hash(value: &str, at: impl FnOnce() -> String) -> Result<ContentHash, ManifestReason> {
    ContentHash::parse(value).map_err(|error| ManifestReason::InvalidHash {
        at: at(),
        value: value.to_owned(),
        error,
    })
}

/// A non-blank text value on one line, with no trailing whitespace: JSON could
/// carry a trailing space or a line break, but two texts that look identical
/// would hash differently, so the format refuses both as v1's line format did.
fn text(value: String, at: impl FnOnce() -> String) -> Result<String, ManifestReason> {
    if value.trim().is_empty() {
        return Err(ManifestReason::Blank { at: at() });
    }
    if value.contains(['\n', '\r']) {
        return Err(ManifestReason::LineBreak { at: at() });
    }
    if value.ends_with(char::is_whitespace) {
        return Err(ManifestReason::TrailingWhitespace { at: at() });
    }
    Ok(value)
}

/// The shape `YYYY-MM-DDTHH:MM:SSZ` the contract schema pins; calendar ranges
/// are the container layer's concern, as they are for v1.
fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

/// Assemble the domain snapshot, moving every string out of the declarations
/// instead of cloning it.
pub(super) fn build_snapshot(
    id: SnapshotId,
    title: String,
    mass_theme: ThemeId,
    themes: Vec<ThemeRef>,
    sections: Vec<SectionDecl>,
) -> Result<PresentationSnapshot, SnapshotError> {
    let mut built_sections = Vec::with_capacity(sections.len());
    for section in sections {
        let mut items = Vec::with_capacity(section.items.len());
        for item in section.items {
            let mut cues = Vec::with_capacity(item.cues.len());
            for cue in item.cues {
                cues.push(Cue::new(
                    cue.id,
                    cue.label,
                    cue.private_context,
                    cue.behavior,
                )?);
            }
            items.push(MassItem::new(
                item.id,
                item.title,
                item.kind,
                item.theme,
                item.auto_play.map(|seconds| AutoPlay {
                    default_seconds: seconds,
                }),
                cues,
            )?);
        }
        built_sections.push(MassSection::new(section.id, section.title, items)?);
    }
    PresentationSnapshot::new(id, title, mass_theme, themes, built_sections)
}
