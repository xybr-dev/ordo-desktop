//! Plain input and output data for the parish store.
//!
//! Inputs describe what a caller wants created; records are what the store
//! returns and rehydrates. Neither type carries SQL, a connection, or any
//! driver type, so callers never depend on the storage implementation.

use domain::identity::{
    ArrangementId, ChoirId, ManagedAssetId, OccurrenceId, ParishId, SectionId, SongId, ThemeId,
    Timestamp,
};
use domain::presentation::ContentHash;
use domain::song_library::{
    ArchiveState, ArrangementDraft, ChoirDraft, HydrateArrangement, HydrateChoir, HydrateReserved,
    HydrateSong, HydrationError, OccurrenceDraft, PaginationState, SectionDraft, SongDraft,
    SongLibrary,
};

/// Description of the Parish to create.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewParish {
    /// The Parish name exactly as authored.
    pub name: String,
}

/// The stored Parish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParishRecord {
    /// Canonical UUIDv4 identity.
    pub id: ParishId,
    /// The name exactly as authored.
    pub name: String,
    /// Creation time.
    pub created_at: Timestamp,
}

/// Description of the Choir to create.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewChoir {
    /// The Choir name exactly as authored.
    pub name: String,
    /// A short label for tight layouts; when present, not blank.
    pub short_label: Option<String>,
    /// Preferred languages in authored order; no blanks, no duplicates.
    pub languages: Vec<String>,
    /// Private notes exactly as authored; when present, not blank.
    pub notes: Option<String>,
}

/// The replacement details of an existing Choir.
///
/// The same rules as [`NewChoir`] apply. Identity, creation time and archive
/// state are never changed by an edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoirDetails {
    /// The Choir name exactly as authored.
    pub name: String,
    /// A short label for tight layouts; when present, not blank.
    pub short_label: Option<String>,
    /// Preferred languages in authored order; no blanks, no duplicates.
    pub languages: Vec<String>,
    /// Private notes exactly as authored; when present, not blank.
    pub notes: Option<String>,
}

/// The stored Choir.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoirRecord {
    /// Canonical UUIDv4 identity.
    pub id: ChoirId,
    /// The name exactly as authored.
    pub name: String,
    /// The short label, if any.
    pub short_label: Option<String>,
    /// Preferred languages in authored order.
    pub languages: Vec<String>,
    /// Private notes, if any.
    pub notes: Option<String>,
    /// Creation time.
    pub created_at: Timestamp,
    /// When the Choir was archived, if it has been.
    pub archived_at: Option<Timestamp>,
}

/// Description of the Song work to create.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSong {
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
    /// Liturgical moments the work belongs to; duplicates are invalid.
    pub liturgical_moments: Vec<String>,
    /// Occasions the work belongs to; duplicates are invalid.
    pub occasions: Vec<String>,
}

/// The stored Song work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongRecord {
    /// Canonical UUIDv4 identity.
    pub id: SongId,
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
    /// Liturgical moments, sorted ascending by byte order.
    pub liturgical_moments: Vec<String>,
    /// Occasions, sorted ascending by byte order.
    pub occasions: Vec<String>,
    /// Creation time.
    pub created_at: Timestamp,
    /// When the Song was archived, if it has been.
    pub archived_at: Option<Timestamp>,
}

/// One section of an Arrangement to create.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSection {
    /// Optional label exactly as authored.
    pub label: Option<String>,
    /// Lyrics exactly as authored.
    pub lyrics: String,
}

/// Description of the Arrangement to create.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewArrangement {
    /// The Song work this Arrangement belongs to.
    pub song: SongId,
    /// The title exactly as authored.
    pub title: String,
    /// Languages in authored order; must be unique.
    pub languages: Vec<String>,
    /// Choirs that perform this Arrangement; duplicates are invalid.
    pub choirs: Vec<ChoirId>,
    /// Who this rendition is attributed to, exactly as authored.
    pub rendition_attribution: Option<String>,
    /// Occasions this Arrangement belongs to; duplicates are invalid.
    pub occasions: Vec<String>,
    /// Reusable sections in authored order.
    pub sections: Vec<NewSection>,
    /// Section indexes in performance order; an index may repeat.
    pub sequence: Vec<usize>,
}

/// One stored section of an Arrangement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionRecord {
    /// Canonical UUIDv4 identity.
    pub id: SectionId,
    /// Optional label exactly as authored.
    pub label: Option<String>,
    /// Lyrics exactly as authored.
    pub lyrics: String,
    /// Creation time.
    pub created_at: Timestamp,
}

/// One stored placement of a section in an Arrangement's Sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OccurrenceRecord {
    /// Canonical UUIDv4 identity.
    pub id: OccurrenceId,
    /// The section this occurrence places.
    pub section: SectionId,
    /// Creation time.
    pub created_at: Timestamp,
}

/// The stored Arrangement with its sections and Sequence occurrences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrangementRecord {
    /// Canonical UUIDv4 identity.
    pub id: ArrangementId,
    /// The Song work this Arrangement belongs to.
    pub song: SongId,
    /// The title exactly as authored.
    pub title: String,
    /// Languages in authored order.
    pub languages: Vec<String>,
    /// Choirs that perform this Arrangement, sorted ascending.
    pub choirs: Vec<ChoirId>,
    /// Who this rendition is attributed to, exactly as authored.
    pub rendition_attribution: Option<String>,
    /// Occasions, sorted ascending by byte order.
    pub occasions: Vec<String>,
    /// Sections by position.
    pub sections: Vec<SectionRecord>,
    /// Sequence occurrences by performance position.
    pub sequence: Vec<OccurrenceRecord>,
    /// Whether the pagination is a draft, approved, or needs review.
    pub pagination: PaginationState,
    /// Creation time.
    pub created_at: Timestamp,
    /// When the Arrangement was archived, if it has been.
    pub archived_at: Option<Timestamp>,
}

/// Every Song Library record in one consistent read.
///
/// Each list is ordered by creation time, then identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryRecords {
    /// Every Song work, archived or not.
    pub songs: Vec<SongRecord>,
    /// Every Arrangement, archived or not, with its sections and Sequence.
    pub arrangements: Vec<ArrangementRecord>,
    /// Every Choir, archived or not.
    pub choirs: Vec<ChoirRecord>,
    /// The section and occurrence identities each Arrangement has ever used,
    /// one entry per Arrangement, in the same order as `arrangements`.
    pub reserved: Vec<ReservedIds>,
}

/// Every section and occurrence identity one Arrangement has ever used.
///
/// The union of its current rows and every stored revision, so identities an
/// edit dropped stay reserved to it across restarts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReservedIds {
    /// The Arrangement that owns the identities.
    pub arrangement: ArrangementId,
    /// Section identities, ascending.
    pub sections: Vec<SectionId>,
    /// Occurrence identities, ascending.
    pub occurrences: Vec<OccurrenceId>,
}

/// The archive state a stored `archived_at` stands for.
fn archive_state(archived_at: Option<Timestamp>) -> ArchiveState {
    archived_at.map_or(ArchiveState::Active, ArchiveState::Archived)
}

impl From<SongRecord> for HydrateSong {
    fn from(record: SongRecord) -> Self {
        Self {
            id: record.id,
            created_at: record.created_at,
            archive_state: archive_state(record.archived_at),
            draft: SongDraft {
                title: record.title,
                aliases: record.aliases,
                original_attribution: record.original_attribution,
                provenance: record.provenance,
                rights_notes: record.rights_notes,
                liturgical_moments: record.liturgical_moments,
                occasions: record.occasions,
            },
        }
    }
}

impl From<ArrangementRecord> for HydrateArrangement {
    fn from(record: ArrangementRecord) -> Self {
        Self {
            id: record.id,
            song: record.song,
            created_at: record.created_at,
            pagination_state: record.pagination,
            archive_state: archive_state(record.archived_at),
            choirs: record.choirs,
            draft: ArrangementDraft {
                title: record.title,
                languages: record.languages,
                rendition_attribution: record.rendition_attribution,
                occasions: record.occasions,
                sections: record
                    .sections
                    .into_iter()
                    .map(|section| SectionDraft {
                        id: section.id,
                        label: section.label,
                        lyrics: section.lyrics,
                    })
                    .collect(),
                sequence: record
                    .sequence
                    .into_iter()
                    .map(|occurrence| OccurrenceDraft::new(occurrence.id, occurrence.section))
                    .collect(),
            },
        }
    }
}

impl From<ChoirRecord> for HydrateChoir {
    fn from(record: ChoirRecord) -> Self {
        Self {
            id: record.id,
            created_at: record.created_at,
            archive_state: archive_state(record.archived_at),
            draft: ChoirDraft {
                name: record.name,
                short_label: record.short_label,
                languages: record.languages,
                notes: record.notes,
            },
        }
    }
}

impl From<ReservedIds> for HydrateReserved {
    fn from(reserved: ReservedIds) -> Self {
        Self {
            arrangement: reserved.arrangement,
            sections: reserved.sections,
            occurrences: reserved.occurrences,
        }
    }
}

impl LibraryRecords {
    /// Rebuilds the domain [`SongLibrary`] from these records, re-checking
    /// every library rule. A stored record that breaks one is a
    /// [`HydrationError`] naming it.
    pub fn hydrate(self) -> Result<SongLibrary, HydrationError> {
        SongLibrary::hydrate(
            self.songs.into_iter().map(Into::into).collect(),
            self.arrangements.into_iter().map(Into::into).collect(),
            self.choirs.into_iter().map(Into::into).collect(),
            self.reserved.into_iter().map(Into::into).collect(),
        )
    }
}

/// Image bytes to store as a managed asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetUpload {
    /// The original file bytes; never empty.
    pub bytes: Vec<u8>,
    /// `image/png` or `image/jpeg`.
    pub media_type: String,
}

/// The stored metadata of one managed asset file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedAssetRecord {
    /// Canonical UUIDv4 identity; also the file name under `assets/`.
    pub id: ManagedAssetId,
    /// SHA-256 of the stored bytes.
    pub sha256: ContentHash,
    /// Length of the stored bytes.
    pub byte_len: u64,
    /// The media type exactly as stored.
    pub media_type: String,
    /// Creation time.
    pub created_at: Timestamp,
}

/// Description of the Theme to create.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewTheme {
    /// The Theme name exactly as authored.
    pub name: String,
    /// An optional background image, stored as a managed asset.
    pub background: Option<AssetUpload>,
}

/// The stored Theme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeRecord {
    /// Canonical UUIDv4 identity.
    pub id: ThemeId,
    /// The name exactly as authored.
    pub name: String,
    /// The background image asset, if any.
    pub background: Option<ManagedAssetRecord>,
    /// Creation time.
    pub created_at: Timestamp,
    /// When the Theme was archived, if it has been.
    pub archived_at: Option<Timestamp>,
}

/// Outcome of re-hashing every managed asset file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetReport {
    /// How many asset rows were checked.
    pub checked: usize,
    /// Assets whose file is absent or unreadable, sorted by id.
    pub missing: Vec<ManagedAssetId>,
    /// Assets whose bytes do not match the recorded length or hash, sorted by id.
    pub corrupt: Vec<ManagedAssetId>,
}
