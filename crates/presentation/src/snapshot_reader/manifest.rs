//! The manifest grammar: `snapshot.txt` parsed into declarations, and the
//! declarations assembled into the domain snapshot.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use domain::presentation::{
    AssetId, AutoPlay, ContentHash, Cue, CueBehavior, CueId, FrameId, IdError, MassItem,
    MassItemId, MassItemKind, MassSection, MassSectionId, PresentationSnapshot, SnapshotError,
    SnapshotId, ThemeId, ThemeKind, ThemeRef,
};

use super::lines::{
    Cursor, LexError, LineError, ManifestError, key_of, strict_u32, take_prefixed, take_raw,
};
use super::{
    ASSET_DIRECTORY, FRAME_DIRECTORY, MANIFEST_FORMAT, ManifestReason, is_snapshot_relative,
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
    behavior: BehaviorDecl,
}

enum BehaviorDecl {
    Project {
        frame: FrameId,
        duration: Option<u32>,
    },
    OperatorTrigger,
    Hold,
    Wait,
    Unavailable {
        reason: String,
    },
}

pub(super) fn parse_manifest(text: &str) -> Result<Manifest, ManifestError> {
    let mut cursor = Cursor::new(text).map_err(LexError::into_manifest)?;
    let (line, format) =
        take_raw(&mut cursor, MANIFEST_FORMAT).map_err(LineError::into_manifest)?;
    if format != MANIFEST_FORMAT {
        return Err(ManifestError {
            line,
            reason: ManifestReason::UnknownFormat {
                found: format.to_owned(),
            },
        });
    }

    let (_, snapshot) = take_id::<SnapshotId>(&mut cursor, "snapshot")?;
    let (_, title) = take_text(&mut cursor, "title", "title")?;
    let (_, fingerprint) = take_hash(&mut cursor, "fingerprint")?;

    let mut themes = Vec::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "theme")
    {
        themes.push(parse_theme(&mut cursor)?);
    }
    let (_, mass_theme) = take_id::<ThemeId>(&mut cursor, "mass-theme")?;

    let mut paths = BTreeSet::new();
    let frames = parse_dependencies::<FrameId, ()>(
        &mut cursor,
        &mut paths,
        &DependencyKeys {
            record: "frame",
            path: "frame-path",
            hash: "frame-sha256",
            directory: FRAME_DIRECTORY,
        },
        |_| Ok(()),
    )?;
    let assets = parse_dependencies::<AssetId, u32>(
        &mut cursor,
        &mut paths,
        &DependencyKeys {
            record: "asset",
            path: "asset-path",
            hash: "asset-sha256",
            directory: ASSET_DIRECTORY,
        },
        |cursor| Ok(take_number(cursor, "asset-size")?.1),
    )?;

    let mut sections = Vec::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "section")
    {
        sections.push(parse_section(&mut cursor)?);
    }
    if let Some((at, line)) = cursor.peek() {
        return Err(ManifestError {
            line: at,
            reason: ManifestReason::Unexpected {
                expected: "end of manifest",
                found: line.to_owned(),
            },
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

fn parse_theme(cursor: &mut Cursor<'_>) -> Result<ThemeRef, ManifestError> {
    let (_, id) = take_id::<ThemeId>(cursor, "theme")?;
    let (_, name) = take_text(cursor, "theme-name", "Theme name")?;
    let (at, kind) = take_text(cursor, "theme-kind", "Theme kind")?;
    let kind = match kind.as_str() {
        "Custom" => ThemeKind::Custom,
        "Fallback" => ThemeKind::Fallback,
        _ => {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::InvalidChoice {
                    field: "Theme kind",
                    value: kind,
                },
            });
        }
    };
    Ok(ThemeRef { id, name, kind })
}

/// The record keys and path directory of one dependency kind.
struct DependencyKeys {
    record: &'static str,
    path: &'static str,
    hash: &'static str,
    directory: &'static str,
}

/// Parse every consecutive record of one dependency kind, rejecting a repeated
/// identity or a path any earlier dependency (of either kind) already names.
/// Paths are compared ASCII-lowercased, because the snapshot must also open on
/// a case-insensitive filesystem where `Frames/A.txt` and `frames/a.txt` are
/// one file. `read_size` parses the record some kinds carry between the path
/// and the digest.
fn parse_dependencies<T, S>(
    cursor: &mut Cursor<'_>,
    paths: &mut BTreeSet<String>,
    keys: &DependencyKeys,
    read_size: impl Fn(&mut Cursor<'_>) -> Result<S, ManifestError>,
) -> Result<Vec<DependencyDecl<T, S>>, ManifestError>
where
    T: FromStr<Err = IdError> + fmt::Display + Ord + Copy,
{
    let mut declared = Vec::new();
    let mut ids = BTreeSet::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == keys.record)
    {
        let (at, id) = take_id::<T>(cursor, keys.record)?;
        let (path_at, path) = take_text(cursor, keys.path, "Dependency path")?;
        let under_directory = path
            .strip_prefix(keys.directory)
            .is_some_and(|rest| rest.starts_with('/'));
        if !under_directory || !is_snapshot_relative(&path) {
            return Err(ManifestError {
                line: path_at,
                reason: ManifestReason::UnsafePath { path },
            });
        }
        let size = read_size(cursor)?;
        let (_, hash) = take_hash(cursor, keys.hash)?;
        if !ids.insert(id) {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::Duplicate {
                    field: keys.record,
                    value: id.to_string(),
                },
            });
        }
        if !paths.insert(path.to_ascii_lowercase()) {
            return Err(ManifestError {
                line: path_at,
                reason: ManifestReason::DuplicatePath { path },
            });
        }
        declared.push(DependencyDecl {
            id,
            path,
            size,
            hash,
        });
    }
    Ok(declared)
}

fn parse_section(cursor: &mut Cursor<'_>) -> Result<SectionDecl, ManifestError> {
    let (_, id) = take_id::<MassSectionId>(cursor, "section")?;
    let (_, title) = take_text(cursor, "section-title", "Section title")?;
    let mut items = Vec::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "item")
    {
        items.push(parse_item(cursor)?);
    }
    if items.is_empty() {
        // The domain rejects an empty section too, but naming the missing
        // record here points at the line the author must add.
        return Err(ManifestError {
            line: cursor.peek().map_or(cursor.eof_line(), |(line, _)| line),
            reason: ManifestReason::Missing { expected: "item" },
        });
    }
    Ok(SectionDecl { id, title, items })
}

fn parse_item(cursor: &mut Cursor<'_>) -> Result<ItemDecl, ManifestError> {
    let (_, id) = take_id::<MassItemId>(cursor, "item")?;
    let (_, title) = take_text(cursor, "item-title", "Mass item title")?;
    let (at, kind) = take_text(cursor, "item-kind", "Mass item kind")?;
    let kind = match kind.as_str() {
        "Song" => MassItemKind::Song,
        "Reading" => MassItemKind::Reading,
        "Psalm" => MassItemKind::Psalm,
        "ResponseSequence" => MassItemKind::ResponseSequence,
        "GeneralPresentation" => MassItemKind::GeneralPresentation,
        "Rite" => MassItemKind::Rite,
        _ => {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::InvalidChoice {
                    field: "Mass item kind",
                    value: kind,
                },
            });
        }
    };
    let theme = if cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "item-theme")
    {
        Some(take_id::<ThemeId>(cursor, "item-theme")?.1)
    } else {
        None
    };
    let auto_play = if cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "item-auto-play")
    {
        Some(take_number(cursor, "item-auto-play")?.1)
    } else {
        None
    };
    let mut cues = Vec::new();
    while cursor.peek().is_some_and(|(_, line)| key_of(line) == "cue") {
        cues.push(parse_cue(cursor)?);
    }
    if cues.is_empty() {
        return Err(ManifestError {
            line: cursor.peek().map_or(cursor.eof_line(), |(line, _)| line),
            reason: ManifestReason::Missing { expected: "cue" },
        });
    }
    Ok(ItemDecl {
        id,
        title,
        kind,
        theme,
        auto_play,
        cues,
    })
}

fn parse_cue(cursor: &mut Cursor<'_>) -> Result<CueDecl, ManifestError> {
    let (_, id) = take_id::<CueId>(cursor, "cue")?;
    let (_, label) = take_text(cursor, "cue-label", "Cue label")?;
    let private_context = if cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "cue-private-context")
    {
        Some(take_text(cursor, "cue-private-context", "Cue private context")?.1)
    } else {
        None
    };
    let (at, behavior) = take_text(cursor, "cue-behavior", "Cue behavior")?;
    let behavior = match behavior.as_str() {
        "Project" => {
            let frame = take_id::<FrameId>(cursor, "cue-frame")?.1;
            let duration = if cursor
                .peek()
                .is_some_and(|(_, line)| key_of(line) == "cue-duration")
            {
                Some(take_number(cursor, "cue-duration")?.1)
            } else {
                None
            };
            BehaviorDecl::Project { frame, duration }
        }
        "OperatorTrigger" => BehaviorDecl::OperatorTrigger,
        "Hold" => BehaviorDecl::Hold,
        "Wait" => BehaviorDecl::Wait,
        "Unavailable" => BehaviorDecl::Unavailable {
            reason: take_text(cursor, "cue-reason", "Unavailable reason")?.1,
        },
        _ => {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::InvalidChoice {
                    field: "Cue behavior",
                    value: behavior,
                },
            });
        }
    };
    Ok(CueDecl {
        id,
        label,
        private_context,
        behavior,
    })
}

fn take_id<T>(cursor: &mut Cursor<'_>, key: &'static str) -> Result<(usize, T), ManifestError>
where
    T: FromStr<Err = IdError>,
{
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    let id = T::from_str(value).map_err(|error| ManifestError {
        line: at,
        reason: ManifestReason::InvalidId {
            value: value.to_owned(),
            error,
        },
    })?;
    Ok((at, id))
}

fn take_hash(
    cursor: &mut Cursor<'_>,
    key: &'static str,
) -> Result<(usize, ContentHash), ManifestError> {
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    let hash = ContentHash::parse(value).map_err(|error| ManifestError {
        line: at,
        reason: ManifestReason::InvalidHash {
            value: value.to_owned(),
            error,
        },
    })?;
    Ok((at, hash))
}

fn take_text(
    cursor: &mut Cursor<'_>,
    key: &'static str,
    field: &'static str,
) -> Result<(usize, String), ManifestError> {
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    if value.trim().is_empty() {
        return Err(ManifestError {
            line: at,
            reason: ManifestReason::Blank { field },
        });
    }
    // An LF-only line format cannot show trailing whitespace, so two texts that
    // look identical would hash differently; refuse it rather than keep it.
    if value.ends_with(char::is_whitespace) {
        return Err(ManifestError {
            line: at,
            reason: ManifestReason::TrailingWhitespace { field },
        });
    }
    Ok((at, value.to_owned()))
}

fn take_number(cursor: &mut Cursor<'_>, key: &'static str) -> Result<(usize, u32), ManifestError> {
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    strict_u32(value)
        .map(|number| (at, number))
        .ok_or_else(|| ManifestError {
            line: at,
            reason: ManifestReason::InvalidNumber {
                value: value.to_owned(),
            },
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
                let behavior = match cue.behavior {
                    BehaviorDecl::Project { frame, duration } => CueBehavior::Project {
                        frame,
                        duration_seconds: duration,
                    },
                    BehaviorDecl::OperatorTrigger => CueBehavior::OperatorTrigger,
                    BehaviorDecl::Hold => CueBehavior::Hold,
                    BehaviorDecl::Wait => CueBehavior::Wait,
                    BehaviorDecl::Unavailable { reason } => CueBehavior::Unavailable { reason },
                };
                cues.push(Cue::new(cue.id, cue.label, cue.private_context, behavior)?);
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
