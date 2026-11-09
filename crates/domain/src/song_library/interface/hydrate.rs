//! Rebuilding a [`SongLibrary`] from stored records.
//!
//! Storage hands the app plain records. [`SongLibrary::hydrate`] turns them
//! back into the aggregate and re-checks every invariant the create and edit
//! operations enforce, so a damaged or hand-edited database becomes a typed
//! [`HydrationError`] naming the offending record, never a panic and never a
//! half-built library. The domain knows nothing of storage: the mapping from a
//! storage record to these input types lives in the storage-facing caller.

use std::fmt;

use crate::identity::{ArrangementId, ChoirId, SongId, Timestamp};

use super::super::model::{
    ArchiveState, Arrangement, ArrangementDraft, Choir, ChoirDraft, PaginationState, SongDraft,
    SongWork,
};
use super::{RecordRef, SongLibrary, SongLibraryError, text};

/// One stored Song work to rebuild.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydrateSong {
    /// The stored identity.
    pub id: SongId,
    /// The stored creation time.
    pub created_at: Timestamp,
    /// The stored authored content.
    pub draft: SongDraft,
    /// The stored archive state.
    pub archive_state: ArchiveState,
}

/// One stored Arrangement to rebuild.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydrateArrangement {
    /// The stored identity.
    pub id: ArrangementId,
    /// The Song work it belongs to.
    pub song: SongId,
    /// The stored creation time.
    pub created_at: Timestamp,
    /// The stored authored content, including section and occurrence identities.
    pub draft: ArrangementDraft,
    /// The Choirs that perform it.
    pub choirs: Vec<ChoirId>,
    /// The stored pagination review state.
    pub pagination_state: PaginationState,
    /// The stored archive state.
    pub archive_state: ArchiveState,
}

/// One stored Choir to rebuild.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydrateChoir {
    /// The stored identity.
    pub id: ChoirId,
    /// The stored creation time.
    pub created_at: Timestamp,
    /// The stored details.
    pub draft: ChoirDraft,
    /// The stored archive state.
    pub archive_state: ArchiveState,
}

/// A stored record that breaks a Song Library invariant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydrationError {
    /// The offending record.
    pub record: RecordRef,
    /// The invariant it breaks.
    pub cause: SongLibraryError,
}

impl fmt::Display for HydrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stored {} is invalid: {}", self.record, self.cause)
    }
}

impl std::error::Error for HydrationError {}

impl SongLibrary {
    /// Rebuilds a library from stored records.
    ///
    /// Choirs and Songs are rebuilt first, then each Arrangement in the given
    /// order. Every rule `create_choir`, `create_song` and `create_arrangement`
    /// enforce is re-checked: unique identities, text rules, the Song exists,
    /// every Choir exists and is distinct, and section and occurrence
    /// identities are free. Stored archive and pagination states are taken as
    /// they are, so an archived Choir may stay associated and an Arrangement
    /// of an archived Song stays as stored. The first breach is returned with
    /// the offending record.
    ///
    /// Section and occurrence identities an Arrangement dropped in an earlier
    /// edit are not stored with it and are therefore not reserved again.
    pub fn hydrate(
        songs: Vec<HydrateSong>,
        arrangements: Vec<HydrateArrangement>,
        choirs: Vec<HydrateChoir>,
    ) -> Result<Self, HydrationError> {
        let mut library = Self::new();
        for stored in choirs {
            let record = RecordRef::Choir(stored.id);
            let breach = |cause| HydrationError { record, cause };
            if library.choirs.contains_key(&stored.id) {
                return Err(breach(SongLibraryError::DuplicateChoir(stored.id)));
            }
            let choir = Choir::new(stored.id, stored.created_at, stored.draft)
                .map_err(breach)?
                .with_archive_state(stored.archive_state);
            library.choirs.insert(stored.id, choir);
        }
        for stored in songs {
            let record = RecordRef::Song(stored.id);
            let breach = |cause| HydrationError { record, cause };
            if library.songs.contains_key(&stored.id) {
                return Err(breach(SongLibraryError::DuplicateSong(stored.id)));
            }
            let song = SongWork::new(stored.id, stored.created_at, stored.draft)
                .map_err(breach)?
                .with_archive_state(stored.archive_state);
            library.songs.insert(stored.id, song);
        }
        for stored in arrangements {
            library.hydrate_arrangement(stored)?;
        }
        Ok(library)
    }

    /// Rebuilds one stored Arrangement into the library.
    fn hydrate_arrangement(&mut self, stored: HydrateArrangement) -> Result<(), HydrationError> {
        let id = stored.id;
        let record = RecordRef::Arrangement(id);
        let breach = |cause| HydrationError { record, cause };
        if self.arrangements.contains_key(&id) {
            return Err(breach(SongLibraryError::DuplicateArrangement(id)));
        }
        if !self.songs.contains_key(&stored.song) {
            return Err(breach(SongLibraryError::UnknownSong(stored.song)));
        }
        text::require_distinct(super::Field::Choir, &stored.choirs).map_err(breach)?;
        self.require_choirs_exist(&stored.choirs).map_err(breach)?;
        Self::require_free_section_and_occurrence_ids(
            &self.section_ids,
            &self.occurrence_ids,
            &stored.draft,
            id,
        )
        .map_err(breach)?;
        let arrangement = Arrangement::new(
            id,
            stored.song,
            stored.created_at,
            stored.draft,
            stored.choirs,
        )
        .map_err(breach)?
        .with_states(stored.pagination_state, stored.archive_state);
        self.section_ids
            .extend(arrangement.sections().iter().map(|s| (s.id(), id)));
        self.occurrence_ids
            .extend(arrangement.sequence().iter().map(|o| (o.id(), id)));
        self.arrangements.insert(id, arrangement);
        Ok(())
    }
}
