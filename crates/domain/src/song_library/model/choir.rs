//! Choirs: lightweight parish singing groups.
//!
//! A Choir owns its identity, name, optional short label, preferred languages,
//! private notes, creation time, and archive state. Its repertoire is derived
//! from the Arrangements that name it; membership is not copied onto the
//! Choir, so several Choirs may share one exact Arrangement.

use crate::identity::{ChoirId, Timestamp};
use crate::song_library::interface::{Field, SongLibraryError};

use super::ArchiveState;
use super::text;

/// The authored details of a Choir.
///
/// Callers reuse this type for creation and for whole-details replacement.
/// Only the name is required. Identity, creation time, and archive state are
/// owned by [`Choir`] itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChoirDraft {
    /// The Choir name exactly as authored; required.
    pub name: String,
    /// A short label for tight layouts, exactly as authored; when present,
    /// must not be blank.
    pub short_label: Option<String>,
    /// Preferred languages in authored order; no blanks, no duplicates.
    pub languages: Vec<String>,
    /// Private notes exactly as authored; when present, must not be blank.
    pub notes: Option<String>,
}

impl ChoirDraft {
    /// A draft whose only content is the required name.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Checks every field rule.
    fn validate(&self) -> Result<(), SongLibraryError> {
        text::require_present(Field::ChoirName, &self.name)?;
        text::require_present_when_given(Field::ChoirShortLabel, self.short_label.as_deref())?;
        for language in &self.languages {
            text::require_present(Field::ChoirLanguage, language)?;
        }
        text::require_distinct(Field::ChoirLanguage, &self.languages)?;
        text::require_present_when_given(Field::ChoirNotes, self.notes.as_deref())
    }
}

/// One parish Choir.
///
/// Every value is private: a Choir is created or edited only through
/// [`crate::song_library::SongLibrary`], which enforces the library's
/// invariants before the value exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choir {
    id: ChoirId,
    name: String,
    short_label: Option<String>,
    languages: Vec<String>,
    notes: Option<String>,
    created_at: Timestamp,
    archive_state: ArchiveState,
}

impl Choir {
    /// Builds a new active Choir after validating its details.
    pub(crate) fn new(
        id: ChoirId,
        created_at: Timestamp,
        draft: ChoirDraft,
    ) -> Result<Self, SongLibraryError> {
        draft.validate()?;
        Ok(Self {
            id,
            name: draft.name,
            short_label: draft.short_label,
            languages: draft.languages,
            notes: draft.notes,
            created_at,
            archive_state: ArchiveState::Active,
        })
    }

    /// Replaces the details, keeping identity, creation time, and archive
    /// state unchanged.
    pub(crate) fn replace_details(&mut self, draft: ChoirDraft) -> Result<(), SongLibraryError> {
        draft.validate()?;
        self.name = draft.name;
        self.short_label = draft.short_label;
        self.languages = draft.languages;
        self.notes = draft.notes;
        Ok(())
    }

    /// Sets the archive state of a Choir being rebuilt from storage.
    pub(crate) fn with_archive_state(mut self, archive_state: ArchiveState) -> Self {
        self.archive_state = archive_state;
        self
    }

    /// Archives the Choir; an already archived Choir keeps its original time.
    pub(crate) fn archive(&mut self, at: Timestamp) {
        self.archive_state.archive(at);
    }

    /// Restores the Choir to active; an active Choir is unchanged.
    pub(crate) fn restore(&mut self) {
        self.archive_state.restore();
    }

    /// The canonical identity of this Choir.
    pub fn id(&self) -> ChoirId {
        self.id
    }

    /// The name, exactly as authored.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The short label, exactly as authored.
    pub fn short_label(&self) -> Option<&str> {
        self.short_label.as_deref()
    }

    /// Preferred languages in authored order.
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// The private notes, exactly as authored.
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
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
