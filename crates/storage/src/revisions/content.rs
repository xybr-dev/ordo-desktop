//! Revision content and its canonical JSON form.
//!
//! The stored `content` of a revision is one JSON object with a fixed key
//! order and no whitespace, produced from private serde mirrors so the domain
//! types need no serde. `content_sha256` is the SHA-256 of exactly the stored
//! bytes. Decoding is strict: an unknown `format`, an unknown key or a bad
//! identity is reported as damaged content, never repaired.

use domain::identity::{ChoirId, OccurrenceId, RevisionId, SectionId};
use domain::presentation::ContentHash;
use serde::{Deserialize, Serialize};

use crate::StorageError;
use crate::assets;
use crate::parish_store::{ArrangementRecord, SongRecord};

/// The canonical form this build writes and reads for Song revisions.
const SONG_FORMAT: u32 = 1;

/// The legacy Arrangement form, read but never written again: title required,
/// no notes.
const ARRANGEMENT_FORMAT_V1: u32 = 1;

/// The canonical form this build writes for Arrangement revisions.
const ARRANGEMENT_FORMAT: u32 = 2;

/// The editable content of a Song work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongContent {
    /// The title exactly as authored.
    pub title: String,
    /// Alternate titles in authored order.
    pub aliases: Vec<String>,
    /// Who the work is originally attributed to, exactly as authored.
    pub original_attribution: Option<String>,
    /// Where the work came from, exactly as authored.
    pub provenance: Option<String>,
    /// Rights notes, exactly as authored.
    pub rights_notes: Option<String>,
    /// Liturgical moments; stored sorted ascending by byte order.
    pub liturgical_moments: Vec<String>,
    /// Occasions; stored sorted ascending by byte order.
    pub occasions: Vec<String>,
}

/// The editable content of one Arrangement section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionContent {
    /// Identity chosen by the caller; kept across edits.
    pub id: SectionId,
    /// Optional label exactly as authored.
    pub label: Option<String>,
    /// Lyrics exactly as authored.
    pub lyrics: String,
}

/// The editable content of one Sequence occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OccurrenceContent {
    /// Identity chosen by the caller; kept across edits.
    pub id: OccurrenceId,
    /// The section this occurrence places.
    pub section: SectionId,
}

/// The editable content of an Arrangement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrangementContent {
    /// The display title exactly as authored; `None` derives the label.
    pub title: Option<String>,
    /// Languages in authored order.
    pub languages: Vec<String>,
    /// Choirs that perform this Arrangement; stored sorted ascending.
    pub choirs: Vec<ChoirId>,
    /// Who this rendition is attributed to, exactly as authored.
    pub rendition_attribution: Option<String>,
    /// Occasions; stored sorted ascending by byte order.
    pub occasions: Vec<String>,
    /// Source and rights notes exactly as authored.
    pub notes: Option<String>,
    /// Sections by position.
    pub sections: Vec<SectionContent>,
    /// Sequence occurrences by performance position.
    pub sequence: Vec<OccurrenceContent>,
}

/// The content of one revision, by entity kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevisionContent {
    /// Song work content.
    Song(SongContent),
    /// Arrangement content.
    Arrangement(ArrangementContent),
}

impl SongContent {
    /// The editable content of a stored Song.
    pub fn of(record: &SongRecord) -> Self {
        Self {
            title: record.title.clone(),
            aliases: record.aliases.clone(),
            original_attribution: record.original_attribution.clone(),
            provenance: record.provenance.clone(),
            rights_notes: record.rights_notes.clone(),
            liturgical_moments: record.liturgical_moments.clone(),
            occasions: record.occasions.clone(),
        }
    }
}

impl ArrangementContent {
    /// The editable content of a stored Arrangement.
    pub fn of(record: &ArrangementRecord) -> Self {
        Self {
            title: record.title.clone(),
            languages: record.languages.clone(),
            choirs: record.choirs.clone(),
            rendition_attribution: record.rendition_attribution.clone(),
            occasions: record.occasions.clone(),
            notes: record.notes.clone(),
            sections: record
                .sections
                .iter()
                .map(|section| SectionContent {
                    id: section.id,
                    label: section.label.clone(),
                    lyrics: section.lyrics.clone(),
                })
                .collect(),
            sequence: record
                .sequence
                .iter()
                .map(|occurrence| OccurrenceContent {
                    id: occurrence.id,
                    section: occurrence.section,
                })
                .collect(),
        }
    }
}

/// Canonical JSON mirror of [`SongContent`]; field order is the key order.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SongDoc {
    format: u32,
    entity: String,
    title: String,
    aliases: Vec<String>,
    original_attribution: Option<String>,
    provenance: Option<String>,
    rights_notes: Option<String>,
    liturgical_moments: Vec<String>,
    occasions: Vec<String>,
}

/// Canonical JSON mirror of [`SectionContent`].
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SectionDoc {
    id: String,
    label: Option<String>,
    lyrics: String,
}

/// Canonical JSON mirror of [`OccurrenceContent`].
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OccurrenceDoc {
    id: String,
    section: String,
}

/// Canonical JSON mirror of [`ArrangementContent`]; field order is the key
/// order. Optional title and notes are omitted when absent so equal content
/// always hashes to equal bytes.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArrangementDoc {
    format: u32,
    entity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    languages: Vec<String>,
    choirs: Vec<String>,
    rendition_attribution: Option<String>,
    occasions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    sections: Vec<SectionDoc>,
    sequence: Vec<OccurrenceDoc>,
}

/// The format-1 Arrangement payload: a required title and no notes. It is read
/// only; this build never writes it again.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArrangementDocV1 {
    format: u32,
    entity: String,
    title: String,
    languages: Vec<String>,
    choirs: Vec<String>,
    rendition_attribution: Option<String>,
    occasions: Vec<String>,
    sections: Vec<SectionDoc>,
    sequence: Vec<OccurrenceDoc>,
}

/// The format and entity of a stored Arrangement document, read before the
/// full payload so each format gets its own strict mirror.
#[derive(Deserialize)]
struct ArrangementProbe {
    format: u32,
    entity: String,
}

/// Decoded Arrangement content and whether it is in this build's current
/// format. Older formats verify against their stored bytes and are never
/// re-serialized.
pub(crate) struct DecodedArrangement {
    /// The decoded content.
    pub(crate) content: ArrangementContent,
    /// True for the format this build writes.
    pub(crate) current: bool,
}

/// Canonical text of one revision's content and its hash.
pub(crate) struct Encoded {
    /// The exact bytes stored in `content`.
    pub(crate) text: String,
    /// SHA-256 of `text`.
    pub(crate) sha256: ContentHash,
}

fn sorted(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort_unstable();
    values
}

fn encoding_failed() -> StorageError {
    StorageError::Sqlite("revision content could not be encoded".to_owned())
}

/// Encodes content to its canonical JSON text and hash.
pub(crate) fn encode(content: &RevisionContent) -> Result<Encoded, StorageError> {
    let bytes = match content {
        RevisionContent::Song(song) => serde_json::to_vec(&SongDoc {
            format: SONG_FORMAT,
            entity: "song".to_owned(),
            title: song.title.clone(),
            aliases: song.aliases.clone(),
            original_attribution: song.original_attribution.clone(),
            provenance: song.provenance.clone(),
            rights_notes: song.rights_notes.clone(),
            liturgical_moments: sorted(&song.liturgical_moments),
            occasions: sorted(&song.occasions),
        }),
        RevisionContent::Arrangement(arrangement) => {
            let mut choirs = arrangement.choirs.clone();
            choirs.sort_unstable();
            serde_json::to_vec(&ArrangementDoc {
                format: ARRANGEMENT_FORMAT,
                entity: "arrangement".to_owned(),
                title: arrangement.title.clone(),
                languages: arrangement.languages.clone(),
                choirs: choirs.iter().map(ToString::to_string).collect(),
                rendition_attribution: arrangement.rendition_attribution.clone(),
                occasions: sorted(&arrangement.occasions),
                notes: arrangement.notes.clone(),
                sections: arrangement
                    .sections
                    .iter()
                    .map(|section| SectionDoc {
                        id: section.id.to_string(),
                        label: section.label.clone(),
                        lyrics: section.lyrics.clone(),
                    })
                    .collect(),
                sequence: arrangement
                    .sequence
                    .iter()
                    .map(|occurrence| OccurrenceDoc {
                        id: occurrence.id.to_string(),
                        section: occurrence.section.to_string(),
                    })
                    .collect(),
            })
        }
    };
    let bytes = bytes.map_err(|_| encoding_failed())?;
    let sha256 = assets::hash(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| encoding_failed())?;
    Ok(Encoded { text, sha256 })
}

/// SHA-256 of the stored text bytes.
pub(crate) fn hash_of(text: &str) -> ContentHash {
    assets::hash(text.as_bytes())
}

/// Decodes the stored text of a Song revision; any defect is `RevisionIntegrity`.
pub(crate) fn decode_song(text: &str, id: RevisionId) -> Result<SongContent, StorageError> {
    let damaged = || StorageError::RevisionIntegrity { id };
    let doc: SongDoc = serde_json::from_str(text).map_err(|_| damaged())?;
    if doc.format != SONG_FORMAT || doc.entity != "song" {
        return Err(damaged());
    }
    Ok(SongContent {
        title: doc.title,
        aliases: doc.aliases,
        original_attribution: doc.original_attribution,
        provenance: doc.provenance,
        rights_notes: doc.rights_notes,
        liturgical_moments: doc.liturgical_moments,
        occasions: doc.occasions,
    })
}

/// The parsed fields shared by every Arrangement payload format.
struct ArrangementParts {
    title: Option<String>,
    languages: Vec<String>,
    choirs: Vec<String>,
    rendition_attribution: Option<String>,
    occasions: Vec<String>,
    notes: Option<String>,
    sections: Vec<SectionDoc>,
    sequence: Vec<OccurrenceDoc>,
}

/// Converts the parsed mirror fields into content, rejecting a bad identity.
fn content_from_docs(parts: ArrangementParts) -> Result<ArrangementContent, ()> {
    let mut parsed_choirs = Vec::with_capacity(parts.choirs.len());
    for choir in &parts.choirs {
        parsed_choirs.push(choir.parse::<ChoirId>().map_err(|_| ())?);
    }
    let mut parsed_sections = Vec::with_capacity(parts.sections.len());
    for section in parts.sections {
        parsed_sections.push(SectionContent {
            id: section.id.parse().map_err(|_| ())?,
            label: section.label,
            lyrics: section.lyrics,
        });
    }
    let mut parsed_sequence = Vec::with_capacity(parts.sequence.len());
    for occurrence in parts.sequence {
        parsed_sequence.push(OccurrenceContent {
            id: occurrence.id.parse().map_err(|_| ())?,
            section: occurrence.section.parse().map_err(|_| ())?,
        });
    }
    Ok(ArrangementContent {
        title: parts.title,
        languages: parts.languages,
        choirs: parsed_choirs,
        rendition_attribution: parts.rendition_attribution,
        occasions: parts.occasions,
        notes: parts.notes,
        sections: parsed_sections,
        sequence: parsed_sequence,
    })
}

/// Decodes the stored text of an Arrangement revision; any defect is
/// `RevisionIntegrity`. Format 1 has a required title and no notes; format 2
/// carries both as optional fields.
pub(crate) fn decode_arrangement(
    text: &str,
    id: RevisionId,
) -> Result<DecodedArrangement, StorageError> {
    let damaged = || StorageError::RevisionIntegrity { id };
    let probe: ArrangementProbe = serde_json::from_str(text).map_err(|_| damaged())?;
    if probe.entity != "arrangement" {
        return Err(damaged());
    }
    match probe.format {
        ARRANGEMENT_FORMAT => {
            let doc: ArrangementDoc = serde_json::from_str(text).map_err(|_| damaged())?;
            let content = content_from_docs(ArrangementParts {
                title: doc.title,
                languages: doc.languages,
                choirs: doc.choirs,
                rendition_attribution: doc.rendition_attribution,
                occasions: doc.occasions,
                notes: doc.notes,
                sections: doc.sections,
                sequence: doc.sequence,
            })
            .map_err(|()| damaged())?;
            Ok(DecodedArrangement {
                content,
                current: true,
            })
        }
        ARRANGEMENT_FORMAT_V1 => {
            let doc: ArrangementDocV1 = serde_json::from_str(text).map_err(|_| damaged())?;
            if doc.format != ARRANGEMENT_FORMAT_V1 || doc.entity != "arrangement" {
                return Err(damaged());
            }
            let content = content_from_docs(ArrangementParts {
                title: Some(doc.title),
                languages: doc.languages,
                choirs: doc.choirs,
                rendition_attribution: doc.rendition_attribution,
                occasions: doc.occasions,
                notes: None,
                sections: doc.sections,
                sequence: doc.sequence,
            })
            .map_err(|()| damaged())?;
            Ok(DecodedArrangement {
                content,
                current: false,
            })
        }
        _ => Err(damaged()),
    }
}
