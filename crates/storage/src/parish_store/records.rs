//! Plain input and output data for the parish store.
//!
//! Inputs describe what a caller wants created; records are what the store
//! returns and rehydrates. Neither type carries SQL, a connection, or any
//! driver type, so callers never depend on the storage implementation.

use domain::identity::{
    ArrangementId, ChoirId, OccurrenceId, ParishId, SectionId, SongId, Timestamp,
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
}

/// The stored Choir.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoirRecord {
    /// Canonical UUIDv4 identity.
    pub id: ChoirId,
    /// The name exactly as authored.
    pub name: String,
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
    /// Creation time.
    pub created_at: Timestamp,
    /// When the Arrangement was archived, if it has been.
    pub archived_at: Option<Timestamp>,
}
