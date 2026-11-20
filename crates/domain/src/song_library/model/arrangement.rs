//! Arrangements: independent renditions of one Song work.
//!
//! An Arrangement belongs to exactly one Song work and independently owns its
//! title, languages, Choir associations, rendition attribution, occasion
//! applicability, reusable sections, performance Sequence, pagination review
//! state, and archive state. A translated or choir-specific rendition is a
//! separate Arrangement even when it retains the underlying composition, and
//! several Choirs may share one exact Arrangement.

use std::collections::BTreeSet;

use crate::identity::{ArrangementId, ChoirId, OccurrenceId, SectionId, SongId, Timestamp};
use crate::song_library::interface::{Field, SongLibraryError};

use super::ArchiveState;
use super::text;

/// The review state of one Arrangement's slide pagination.
///
/// A new Arrangement starts in [`PaginationState::Draft`]. Approving it moves
/// it to [`PaginationState::Approved`]. While it is approved, adding or
/// removing a section, changing a section's lyrics or label, or changing the
/// Sequence moves it to [`PaginationState::NeedsReview`]; title, language,
/// Choir, attribution, occasion, and section-order changes do not affect
/// pagination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaginationState {
    /// Pagination has not been reviewed yet.
    Draft,
    /// The operator approved the current slide pagination.
    Approved,
    /// Content that affects pagination changed after approval.
    NeedsReview,
}

/// One reusable section of an Arrangement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionDraft {
    /// The canonical identity the section will keep.
    pub id: SectionId,
    /// Optional label exactly as authored, such as Verse 1 or Chorus.
    pub label: Option<String>,
    /// Lyrics exactly as authored; an empty value is a section without words.
    pub lyrics: String,
}

impl SectionDraft {
    /// An unlabeled section with lyrics.
    pub fn new(id: SectionId, lyrics: impl Into<String>) -> Self {
        Self {
            id,
            label: None,
            lyrics: lyrics.into(),
        }
    }

    /// A labeled section with lyrics.
    pub fn labeled(id: SectionId, label: impl Into<String>, lyrics: impl Into<String>) -> Self {
        Self {
            id,
            label: Some(label.into()),
            lyrics: lyrics.into(),
        }
    }
}

/// One ordered placement of a section in an Arrangement's Sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OccurrenceDraft {
    /// The canonical identity the occurrence will keep.
    pub id: OccurrenceId,
    /// The section this occurrence places; it must be one of the Arrangement's
    /// own sections.
    pub section: SectionId,
}

impl OccurrenceDraft {
    /// An occurrence that places `section`.
    pub fn new(id: OccurrenceId, section: SectionId) -> Self {
        Self { id, section }
    }
}

/// One stored reusable section of an Arrangement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    id: SectionId,
    label: Option<String>,
    lyrics: String,
}

impl Section {
    /// The canonical identity of this section.
    pub fn id(&self) -> SectionId {
        self.id
    }

    /// The label exactly as authored.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The lyrics exactly as authored.
    pub fn lyrics(&self) -> &str {
        &self.lyrics
    }
}

/// One stored placement of a section in an Arrangement's Sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Occurrence {
    id: OccurrenceId,
    section: SectionId,
}

impl Occurrence {
    /// The canonical identity of this occurrence.
    pub fn id(&self) -> OccurrenceId {
        self.id
    }

    /// The section this occurrence places.
    pub fn section(&self) -> SectionId {
        self.section
    }
}

/// The authored content of an Arrangement.
///
/// Callers reuse this type for creation and for whole-content replacement.
/// The owning Song, identity, Choir associations, creation time, pagination
/// review state, and archive state are owned by [`Arrangement`] itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArrangementDraft {
    /// The title exactly as authored; required.
    pub title: String,
    /// Languages in authored order; duplicates are invalid.
    pub languages: Vec<String>,
    /// Who this rendition is attributed to, exactly as authored; when present,
    /// must not be blank.
    pub rendition_attribution: Option<String>,
    /// Occasion applicability tags, advisory rather than restrictive.
    pub occasions: Vec<String>,
    /// Reusable sections in authored order.
    pub sections: Vec<SectionDraft>,
    /// Section placements in performance order; a section may repeat.
    pub sequence: Vec<OccurrenceDraft>,
}

impl ArrangementDraft {
    /// A draft whose only content is the required title.
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// One Arrangement and the content it owns.
///
/// Every value is private: an Arrangement is created or changed only through
/// [`crate::song_library::SongLibrary`], which enforces the library's
/// invariants before the value exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arrangement {
    id: ArrangementId,
    song: SongId,
    content: ArrangementContent,
    choirs: BTreeSet<ChoirId>,
    pagination_state: PaginationState,
    created_at: Timestamp,
    archive_state: ArchiveState,
}

impl Arrangement {
    /// Builds a new draft-pagination Arrangement after validating its content.
    pub(crate) fn new(
        id: ArrangementId,
        song: SongId,
        created_at: Timestamp,
        draft: ArrangementDraft,
        choirs: Vec<ChoirId>,
    ) -> Result<Self, SongLibraryError> {
        Ok(Self {
            id,
            song,
            content: ArrangementContent::validate(id, draft)?,
            choirs: choirs.into_iter().collect(),
            pagination_state: PaginationState::Draft,
            created_at,
            archive_state: ArchiveState::Active,
        })
    }

    /// Replaces the authored content and applies the pagination review rule.
    ///
    /// Adding or removing a section, changing a section's lyrics or label, or
    /// changing the Sequence moves an approved Arrangement to
    /// [`PaginationState::NeedsReview`]. Draft and already-needs-review states
    /// stay as they are. Identity, owning Song, Choir associations, creation
    /// time, and archive state are preserved.
    pub(crate) fn replace_content(
        &mut self,
        draft: ArrangementDraft,
    ) -> Result<(), SongLibraryError> {
        let next = ArrangementContent::validate(self.id, draft)?;
        let pagination_changed = pagination_content_changed(&self.content, &next);
        if pagination_changed && self.pagination_state == PaginationState::Approved {
            self.pagination_state = PaginationState::NeedsReview;
        }
        self.content = next;
        Ok(())
    }

    /// Replaces the Choir associations.
    pub(crate) fn replace_choirs(&mut self, choirs: Vec<ChoirId>) {
        self.choirs = choirs.into_iter().collect();
    }

    /// Associates one more Choir; an existing association is unchanged.
    pub(crate) fn add_choir(&mut self, choir: ChoirId) {
        self.choirs.insert(choir);
    }

    /// Drops one Choir association; a missing association is unchanged.
    pub(crate) fn remove_choir(&mut self, choir: ChoirId) {
        self.choirs.remove(&choir);
    }

    /// Marks the current pagination as reviewed and approved.
    pub(crate) fn approve_pagination(&mut self) {
        self.pagination_state = PaginationState::Approved;
    }

    /// The canonical identity of this Arrangement.
    pub fn id(&self) -> ArrangementId {
        self.id
    }

    /// The Song work this Arrangement belongs to.
    pub fn song(&self) -> SongId {
        self.song
    }

    /// The title exactly as authored.
    pub fn title(&self) -> &str {
        &self.content.title
    }

    /// Languages in authored order.
    pub fn languages(&self) -> &[String] {
        &self.content.languages
    }

    /// Choirs that perform this exact rendition, in identity order.
    pub fn choirs(&self) -> &BTreeSet<ChoirId> {
        &self.choirs
    }

    /// Who this rendition is attributed to, exactly as authored.
    pub fn rendition_attribution(&self) -> Option<&str> {
        self.content.rendition_attribution.as_deref()
    }

    /// Occasion applicability tags in sorted byte order.
    pub fn occasions(&self) -> &BTreeSet<String> {
        &self.content.occasions
    }

    /// Reusable sections in authored order.
    pub fn sections(&self) -> &[Section] {
        &self.content.sections
    }

    /// Section placements in performance order.
    pub fn sequence(&self) -> &[Occurrence] {
        &self.content.sequence
    }

    /// The pagination review state.
    pub fn pagination_state(&self) -> PaginationState {
        self.pagination_state
    }

    /// When the Arrangement was created.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Whether the Arrangement is active or archived.
    pub fn archive_state(&self) -> ArchiveState {
        self.archive_state
    }

    /// Whether the Arrangement is archived.
    pub fn is_archived(&self) -> bool {
        self.archive_state.is_archived()
    }
}

/// Validated authored Arrangement content.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ArrangementContent {
    title: String,
    languages: Vec<String>,
    rendition_attribution: Option<String>,
    occasions: BTreeSet<String>,
    sections: Vec<Section>,
    sequence: Vec<Occurrence>,
}

impl ArrangementContent {
    /// Checks every field rule before any value is stored.
    fn validate(
        arrangement: ArrangementId,
        draft: ArrangementDraft,
    ) -> Result<Self, SongLibraryError> {
        text::require_present(Field::ArrangementTitle, &draft.title)?;
        for language in &draft.languages {
            text::require_present(Field::Language, language)?;
        }
        text::require_distinct(Field::Language, &draft.languages)?;
        text::require_present_when_given(
            Field::RenditionAttribution,
            draft.rendition_attribution.as_deref(),
        )?;
        for occasion in &draft.occasions {
            text::require_present(Field::Occasion, occasion)?;
        }
        text::require_distinct(Field::Occasion, &draft.occasions)?;
        for section in &draft.sections {
            text::require_present_when_given(Field::SectionLabel, section.label.as_deref())?;
        }

        let mut section_ids = BTreeSet::new();
        for section in &draft.sections {
            if !section_ids.insert(section.id) {
                return Err(SongLibraryError::DuplicateSection(section.id));
            }
        }
        let mut occurrence_ids = BTreeSet::new();
        for occurrence in &draft.sequence {
            if !occurrence_ids.insert(occurrence.id) {
                return Err(SongLibraryError::DuplicateOccurrence(occurrence.id));
            }
            if !section_ids.contains(&occurrence.section) {
                return Err(SongLibraryError::SectionNotInArrangement {
                    arrangement,
                    section: occurrence.section,
                });
            }
        }

        Ok(Self {
            title: draft.title,
            languages: draft.languages,
            rendition_attribution: draft.rendition_attribution,
            occasions: draft.occasions.into_iter().collect(),
            sections: draft
                .sections
                .into_iter()
                .map(|section| Section {
                    id: section.id,
                    label: section.label,
                    lyrics: section.lyrics,
                })
                .collect(),
            sequence: draft
                .sequence
                .into_iter()
                .map(|occurrence| Occurrence {
                    id: occurrence.id,
                    section: occurrence.section,
                })
                .collect(),
        })
    }
}

/// Whether an edit changed content that affects slide pagination.
///
/// Section order alone does not affect pagination, so sections are compared by
/// identity rather than by position.
fn pagination_content_changed(previous: &ArrangementContent, next: &ArrangementContent) -> bool {
    if previous.sequence != next.sequence {
        return true;
    }
    let mut previous_sections: Vec<&Section> = previous.sections.iter().collect();
    let mut next_sections: Vec<&Section> = next.sections.iter().collect();
    previous_sections.sort_by_key(|section| section.id());
    next_sections.sort_by_key(|section| section.id());
    previous_sections != next_sections
}
