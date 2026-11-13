//! Choirs: lightweight parish singing groups.
//!
//! A Choir owns only its identity, name, creation time, and archive state. Its
//! repertoire is derived from the Arrangements that name it; membership is not
//! copied onto the Choir, so several Choirs may share one exact Arrangement.

use crate::identity::{ChoirId, Timestamp};
use crate::song_library::interface::{Field, SongLibraryError};

use super::ArchiveState;
use super::text;

/// One parish Choir.
///
/// Every value is private: a Choir is created or renamed only through
/// [`crate::song_library::SongLibrary`], which enforces the library's
/// invariants before the value exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choir {
    id: ChoirId,
    name: String,
    created_at: Timestamp,
    archive_state: ArchiveState,
}

impl Choir {
    /// Builds a new active Choir after validating its name.
    pub(crate) fn new(
        id: ChoirId,
        created_at: Timestamp,
        name: String,
    ) -> Result<Self, SongLibraryError> {
        text::require_present(Field::ChoirName, &name)?;
        Ok(Self {
            id,
            name,
            created_at,
            archive_state: ArchiveState::Active,
        })
    }

    /// Replaces the name, keeping identity, creation time, and archive state
    /// unchanged.
    pub(crate) fn rename(&mut self, name: String) -> Result<(), SongLibraryError> {
        text::require_present(Field::ChoirName, &name)?;
        self.name = name;
        Ok(())
    }

    /// The canonical identity of this Choir.
    pub fn id(&self) -> ChoirId {
        self.id
    }

    /// The name, exactly as authored.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// When the Choir was created.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Whether the Choir is active or archived.
    pub fn archive_state(&self) -> ArchiveState {
        self.archive_state
    }

    /// Whether the Choir is archived.
    pub fn is_archived(&self) -> bool {
        self.archive_state.is_archived()
    }
}
