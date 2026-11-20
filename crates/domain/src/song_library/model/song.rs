//! Song works: the identity of one underlying musical composition or setting.
//!
//! A Song work is independent of Choir, Arrangement, language version, and
//! liturgical use. Numbered settings such as Kyrie 8th and Kyrie 13th are
//! separate works, because a shared title or shared liturgical text never
//! unites two musical settings. A translated rendition retains the work and
//! becomes a separate Arrangement instead.

use std::collections::BTreeSet;

use crate::identity::{SongId, Timestamp};
use crate::song_library::interface::{Field, SongLibraryError};

use super::ArchiveState;
use super::text;

/// The authored content of a Song work.
///
/// The primary title is the only required value. Callers may reuse this type
/// to replace a Song work's whole content; identity, creation time, and archive
/// state are owned by [`SongWork`] itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SongDraft {
    /// The primary parish title, exactly as authored; the only required value.
    pub title: String,
    /// Alternate titles in authored order, exactly as authored; must not repeat
    /// or equal the primary title.
    pub aliases: Vec<String>,
    /// Who the work is originally attributed to, exactly as authored; when
    /// present, must not be blank.
    pub original_attribution: Option<String>,
    /// Where the work came from, exactly as authored; when present, must not be
    /// blank.
    pub provenance: Option<String>,
    /// Rights notes, exactly as authored; when present, must not be blank.
    pub rights_notes: Option<String>,
    /// Liturgical moments the work belongs to, such as Entrance.
    pub liturgical_moments: Vec<String>,
    /// Occasion applicability tags, advisory rather than restrictive.
    pub occasions: Vec<String>,
}

impl SongDraft {
    /// A draft whose only content is the required primary title.
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// One Song work and the content it owns.
///
/// Every value is private: a Song work is created or changed only through
/// [`crate::song_library::SongLibrary`], which enforces the library's
/// invariants before the value exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongWork {
    id: SongId,
    content: SongContent,
    created_at: Timestamp,
    archive_state: ArchiveState,
}

impl SongWork {
    /// Builds a new active Song work after validating its content.
    pub(crate) fn new(
        id: SongId,
        created_at: Timestamp,
        draft: SongDraft,
    ) -> Result<Self, SongLibraryError> {
        Ok(Self {
            id,
            content: SongContent::validate(draft)?,
            created_at,
            archive_state: ArchiveState::Active,
        })
    }

    /// Sets the archive state of a Song work being rebuilt from storage.
    pub(crate) fn with_archive_state(mut self, archive_state: ArchiveState) -> Self {
        self.archive_state = archive_state;
        self
    }

    /// Replaces the authored content, keeping identity, creation time, and
    /// archive state unchanged.
    pub(crate) fn replace_content(&mut self, draft: SongDraft) -> Result<(), SongLibraryError> {
        self.content = SongContent::validate(draft)?;
        Ok(())
    }

    /// Archives the Song work; an already archived work keeps its original time.
    pub(crate) fn archive(&mut self, at: Timestamp) {
        self.archive_state.archive(at);
    }

    /// Restores the Song work to active; an active work is unchanged.
    pub(crate) fn restore(&mut self) {
        self.archive_state.restore();
    }

    /// The canonical identity of this Song work.
    pub fn id(&self) -> SongId {
        self.id
    }

    /// The primary parish title, exactly as authored.
    pub fn title(&self) -> &str {
        &self.content.title
    }

    /// Alternate titles in authored order, exactly as authored.
    pub fn aliases(&self) -> &[String] {
        &self.content.aliases
    }

    /// Who the work is originally attributed to, exactly as authored.
    pub fn original_attribution(&self) -> Option<&str> {
        self.content.original_attribution.as_deref()
    }

    /// Where the work came from, exactly as authored.
    pub fn provenance(&self) -> Option<&str> {
        self.content.provenance.as_deref()
    }

    /// Rights notes, exactly as authored.
    pub fn rights_notes(&self) -> Option<&str> {
        self.content.rights_notes.as_deref()
    }

    /// Liturgical moments in sorted byte order.
    pub fn liturgical_moments(&self) -> &BTreeSet<String> {
        &self.content.liturgical_moments
    }

    /// Occasion applicability tags in sorted byte order.
    pub fn occasions(&self) -> &BTreeSet<String> {
        &self.content.occasions
    }

    /// When the Song work was created.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Whether the Song work is active or archived.
    pub fn archive_state(&self) -> ArchiveState {
        self.archive_state
    }

    /// Whether the Song work is archived.
    pub fn is_archived(&self) -> bool {
        self.archive_state.is_archived()
    }
}

/// Validated authored Song content.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SongContent {
    title: String,
    aliases: Vec<String>,
    original_attribution: Option<String>,
    provenance: Option<String>,
    rights_notes: Option<String>,
    liturgical_moments: BTreeSet<String>,
    occasions: BTreeSet<String>,
}

impl SongContent {
    /// Checks every field rule before any value is stored.
    fn validate(draft: SongDraft) -> Result<Self, SongLibraryError> {
        text::require_present(Field::SongTitle, &draft.title)?;
        text::require_present_when_given(
            Field::OriginalAttribution,
            draft.original_attribution.as_deref(),
        )?;
        text::require_present_when_given(Field::Provenance, draft.provenance.as_deref())?;
        text::require_present_when_given(Field::RightsNotes, draft.rights_notes.as_deref())?;
        for alias in &draft.aliases {
            text::require_present(Field::Aliases, alias)?;
        }
        text::require_distinct(Field::Aliases, &draft.aliases)?;
        if draft.aliases.iter().any(|alias| alias == &draft.title) {
            return Err(SongLibraryError::DuplicateField(Field::Aliases));
        }
        for moment in &draft.liturgical_moments {
            text::require_present(Field::LiturgicalMoment, moment)?;
        }
        text::require_distinct(Field::LiturgicalMoment, &draft.liturgical_moments)?;
        for occasion in &draft.occasions {
            text::require_present(Field::Occasion, occasion)?;
        }
        text::require_distinct(Field::Occasion, &draft.occasions)?;
        Ok(Self {
            title: draft.title,
            aliases: draft.aliases,
            original_attribution: draft.original_attribution,
            provenance: draft.provenance,
            rights_notes: draft.rights_notes,
            liturgical_moments: draft.liturgical_moments.into_iter().collect(),
            occasions: draft.occasions.into_iter().collect(),
        })
    }
}
