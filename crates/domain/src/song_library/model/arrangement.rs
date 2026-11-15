//! Arrangements: independent renditions of one Song work.
//!
//! An Arrangement belongs to exactly one Song work and independently owns its
//! title, languages, Choir associations, rendition attribution, occasion
//! applicability, reusable sections (each with its own pagination review
//! state), performance Sequence, and archive state. A translated or
//! choir-specific rendition is a separate Arrangement even when it retains the
//! underlying composition, and several Choirs may share one exact Arrangement.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{ArrangementId, ChoirId, OccurrenceId, SectionId, SongId, Timestamp};
use crate::song_library::interface::{Field, SongLibraryError};
use crate::song_library::languages;
use crate::song_library::sections::ArrangementLyrics;

use super::ArchiveState;
use super::text;

/// The review state of one section's slide pagination.
///
/// A new section starts in [`PaginationState::Draft`]. Approving it moves it to
/// [`PaginationState::Approved`]. While it is approved, changing its lyrics
/// (Split and Join included) moves it to [`PaginationState::NeedsReview`];
/// renaming it, reordering the Lyrics list, and every Sequence edit do not.
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
    /// The review state of this section's slide pagination.
    pub pagination: PaginationState,
}

impl SectionDraft {
    /// An unlabeled section with lyrics and a draft pagination state.
    pub fn new(id: SectionId, lyrics: impl Into<String>) -> Self {
        Self {
            id,
            label: None,
            lyrics: lyrics.into(),
            pagination: PaginationState::Draft,
        }
    }

    /// A labeled section with lyrics and a draft pagination state.
    pub fn labeled(id: SectionId, label: impl Into<String>, lyrics: impl Into<String>) -> Self {
        Self {
            id,
            label: Some(label.into()),
            lyrics: lyrics.into(),
            pagination: PaginationState::Draft,
        }
    }

    /// The same section with the pagination state given.
    ///
    /// Storage rehydration uses this to take a stored state exactly; forward
    /// edits go through the review rule instead.
    #[must_use]
    pub fn with_pagination(mut self, pagination: PaginationState) -> Self {
        self.pagination = pagination;
        self
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
    pagination: PaginationState,
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

    /// The review state of this section's slide pagination.
    pub fn pagination(&self) -> PaginationState {
        self.pagination
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
/// The owning Song, identity, Choir associations, creation time, and archive
/// state are owned by [`Arrangement`] itself. Each section carries its own
/// pagination review state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArrangementDraft {
    /// The display title exactly as authored; `None` when there is none, in
    /// which case the label is derived. When present, must not be blank.
    pub title: Option<String>,
    /// Languages in authored order; duplicates are invalid. Creation and edits
    /// need at least one, from the built-in list.
    pub languages: Vec<String>,
    /// Who this rendition is attributed to, exactly as authored; when present,
    /// must not be blank.
    pub rendition_attribution: Option<String>,
    /// Occasion applicability tags, advisory rather than restrictive.
    pub occasions: Vec<String>,
    /// Source and rights notes exactly as authored; when present, must not be
    /// blank.
    pub notes: Option<String>,
    /// Reusable sections in authored order.
    pub sections: Vec<SectionDraft>,
    /// Section placements in performance order; a section may repeat.
    pub sequence: Vec<OccurrenceDraft>,
}

impl ArrangementDraft {
    /// A draft whose only content is the display title.
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            ..Self::default()
        }
    }

    /// A draft with no title whose only content is one language.
    pub fn in_language(language: impl Into<String>) -> Self {
        Self {
            languages: vec![language.into()],
            ..Self::default()
        }
    }
}

/// The metadata of an Arrangement: everything a details edit replaces.
///
/// A details edit replaces these values and the Choir associations as one
/// unit and leaves sections, the Sequence, and the pagination review state
/// untouched. Callers get the current value from [`Arrangement::details`],
/// change it, and hand it to
/// [`crate::song_library::SongLibrary::edit_arrangement_details`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArrangementDetails {
    /// The display title exactly as authored; `None` derives the label. When
    /// present, must not be blank.
    pub title: Option<String>,
    /// Languages in authored order; at least one, each from the built-in
    /// list, without repeats.
    pub languages: Vec<String>,
    /// The Choirs that perform this exact rendition; active Choirs only
    /// unless already associated, without repeats.
    pub choirs: Vec<ChoirId>,
    /// Who this rendition is attributed to; when present, must not be blank.
    pub rendition_attribution: Option<String>,
    /// Occasion applicability tags, advisory rather than restrictive.
    pub occasions: Vec<String>,
    /// Source and rights notes; when present, must not be blank.
    pub notes: Option<String>,
}

impl ArrangementDetails {
    /// Whether `other` holds the same details, ignoring the order of Choirs
    /// and occasions (an Arrangement keeps both as sets). Languages are an
    /// ordered list, so their order counts.
    pub fn same_as(&self, other: &Self) -> bool {
        let sorted = |details: &Self| {
            let mut details = details.clone();
            details.choirs.sort_unstable();
            details.occasions.sort_unstable();
            details
        };
        sorted(self) == sorted(other)
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
    created_at: Timestamp,
    archive_state: ArchiveState,
}

impl Arrangement {
    /// Builds an Arrangement, with each section's pagination state taken as
    /// given, after validating its content.
    ///
    /// Creation through the Interface normalizes every section to Draft;
    /// hydration from storage takes the stored states.
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
            created_at,
            archive_state: ArchiveState::Active,
        })
    }

    /// Sets the archive state of an Arrangement being rebuilt from storage.
    pub(crate) fn with_archive_state(mut self, archive_state: ArchiveState) -> Self {
        self.archive_state = archive_state;
        self
    }

    /// Replaces the authored content and applies the pagination review rule per
    /// section.
    ///
    /// A section whose identity is new starts Draft. A stored section keeps its
    /// state unless it was Approved and its lyrics change, in which case it
    /// becomes [`PaginationState::NeedsReview`]. Adding or removing other
    /// sections, renaming, reordering the Lyrics list, and changing the
    /// Sequence never move a stored section's state. Identity, owning Song,
    /// Choir associations, creation time, and archive state are preserved.
    pub(crate) fn replace_content(
        &mut self,
        draft: ArrangementDraft,
    ) -> Result<(), SongLibraryError> {
        let previous = section_states(&self.content);
        let mut next = ArrangementContent::validate(self.id, draft)?;
        apply_review_rule(&mut next.sections, &previous);
        self.content = next;
        Ok(())
    }

    /// Validates replacement sections and Sequence against this Arrangement's
    /// metadata without changing anything.
    pub(crate) fn check_lyrics(
        &self,
        sections: &[SectionDraft],
        sequence: &[OccurrenceDraft],
    ) -> Result<(), SongLibraryError> {
        self.lyrics_content(sections.to_vec(), sequence.to_vec())
            .map(|_| ())
    }

    /// Replaces sections and Sequence as given, keeping the metadata, Choir
    /// associations, identity, creation time, and archive state. Applies no
    /// review rule: each section's pagination state is the one supplied.
    ///
    /// Nothing changes on refusal.
    pub(crate) fn replace_lyrics(
        &mut self,
        sections: Vec<SectionDraft>,
        sequence: Vec<OccurrenceDraft>,
    ) -> Result<(), SongLibraryError> {
        self.content = self.lyrics_content(sections, sequence)?;
        Ok(())
    }

    /// The validated content with the given sections and Sequence and this
    /// Arrangement's metadata.
    fn lyrics_content(
        &self,
        sections: Vec<SectionDraft>,
        sequence: Vec<OccurrenceDraft>,
    ) -> Result<ArrangementContent, SongLibraryError> {
        let draft = ArrangementDraft {
            title: self.content.title.clone(),
            languages: self.content.languages.clone(),
            rendition_attribution: self.content.rendition_attribution.clone(),
            occasions: self.content.occasions.iter().cloned().collect(),
            notes: self.content.notes.clone(),
            sections,
            sequence,
        };
        ArrangementContent::validate(self.id, draft)
    }

    /// Replaces the authored content and the Choir associations with values
    /// that storage already holds, applying no review rule: each section's
    /// pagination state is the one supplied.
    ///
    /// Nothing changes on refusal.
    pub(crate) fn refresh(
        &mut self,
        draft: ArrangementDraft,
        choirs: Vec<ChoirId>,
    ) -> Result<(), SongLibraryError> {
        let next = ArrangementContent::validate(self.id, draft)?;
        self.content = next;
        self.choirs = choirs.into_iter().collect();
        Ok(())
    }

    /// Replaces the metadata and keeps sections, Sequence, pagination state,
    /// Choir associations, and archive state; Choirs are the caller's concern.
    ///
    /// Languages must already be canonical. Nothing changes on refusal.
    pub(crate) fn replace_metadata(
        &mut self,
        details: ArrangementDetails,
    ) -> Result<(), SongLibraryError> {
        let metadata = Metadata::validate(
            details.title,
            details.languages,
            details.rendition_attribution,
            details.occasions,
            details.notes,
        )?;
        self.content.title = metadata.title;
        self.content.languages = metadata.languages;
        self.content.rendition_attribution = metadata.rendition_attribution;
        self.content.occasions = metadata.occasions;
        self.content.notes = metadata.notes;
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

    /// Archives the Arrangement; an already archived one keeps its original
    /// time.
    pub(crate) fn archive(&mut self, at: Timestamp) {
        self.archive_state.archive(at);
    }

    /// Restores the Arrangement to active; an active one is unchanged.
    pub(crate) fn restore(&mut self) {
        self.archive_state.restore();
    }

    /// The canonical identity of this Arrangement.
    pub fn id(&self) -> ArrangementId {
        self.id
    }

    /// The Song work this Arrangement belongs to.
    pub fn song(&self) -> SongId {
        self.song
    }

    /// The display title exactly as authored, when it has one.
    ///
    /// Use [`crate::song_library::SongLibrary::arrangement_label`] for a name
    /// to show: it falls back to a label derived from languages and Choirs.
    pub fn title(&self) -> Option<&str> {
        self.content.title.as_deref()
    }

    /// Source and rights notes exactly as authored.
    pub fn notes(&self) -> Option<&str> {
        self.content.notes.as_deref()
    }

    /// The current metadata, ready to edit and hand back to
    /// [`crate::song_library::SongLibrary::edit_arrangement_details`].
    ///
    /// Choirs are in identity order and occasions in sorted byte order.
    pub fn details(&self) -> ArrangementDetails {
        ArrangementDetails {
            title: self.content.title.clone(),
            languages: self.content.languages.clone(),
            choirs: self.choirs.iter().copied().collect(),
            rendition_attribution: self.content.rendition_attribution.clone(),
            occasions: self.content.occasions.iter().cloned().collect(),
            notes: self.content.notes.clone(),
        }
    }

    /// The current authored content, ready for the Lyrics editor to change and
    /// hand back to
    /// [`crate::song_library::SongLibrary::edit_arrangement_lyrics`].
    ///
    /// Sections are in authored order, Sequence occurrences in performance
    /// order, and each section carries its own pagination review state so an
    /// edit can commit the state it computed.
    pub fn lyrics(&self) -> ArrangementLyrics {
        ArrangementLyrics {
            sections: self
                .content
                .sections
                .iter()
                .map(|section| SectionDraft {
                    id: section.id(),
                    label: section.label().map(str::to_owned),
                    lyrics: section.lyrics().to_owned(),
                    pagination: section.pagination(),
                })
                .collect(),
            sequence: self
                .content
                .sequence
                .iter()
                .map(|occurrence| OccurrenceDraft::new(occurrence.id(), occurrence.section()))
                .collect(),
        }
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

/// Validated Arrangement metadata.
struct Metadata {
    title: Option<String>,
    languages: Vec<String>,
    rendition_attribution: Option<String>,
    occasions: BTreeSet<String>,
    notes: Option<String>,
}

impl Metadata {
    /// Checks the field rules shared by creation, content edits, details edits
    /// and hydration. It does not require a language: that rule belongs to
    /// [`require_listed_languages`], so stored records without one still load.
    fn validate(
        title: Option<String>,
        languages: Vec<String>,
        rendition_attribution: Option<String>,
        occasions: Vec<String>,
        notes: Option<String>,
    ) -> Result<Self, SongLibraryError> {
        text::require_present_when_given(Field::ArrangementTitle, title.as_deref())?;
        for language in &languages {
            text::require_present(Field::Language, language)?;
        }
        text::require_distinct(Field::Language, &languages)?;
        text::require_present_when_given(
            Field::RenditionAttribution,
            rendition_attribution.as_deref(),
        )?;
        for occasion in &occasions {
            text::require_present(Field::Occasion, occasion)?;
        }
        text::require_distinct(Field::Occasion, &occasions)?;
        text::require_present_when_given(Field::ArrangementNotes, notes.as_deref())?;
        Ok(Self {
            title,
            languages,
            rendition_attribution,
            occasions: occasions.into_iter().collect(),
            notes,
        })
    }
}

/// The rule for languages a caller supplies on creation or an edit: at least
/// one, each the display name of a built-in language (matched ignoring case
/// and diacritics) and stored under that display name. A language the
/// Arrangement already holds (`current`, matched exactly) may be kept even when
/// it is not built in; only a language being added must be listed.
///
/// Hydration does not call this, so stored records keep whatever they hold.
pub(crate) fn require_listed_languages(
    languages: Vec<String>,
    current: &[String],
) -> Result<Vec<String>, SongLibraryError> {
    if languages.is_empty() {
        return Err(SongLibraryError::BlankField(Field::Language));
    }
    languages
        .into_iter()
        .map(|language| {
            text::require_present(Field::Language, &language)?;
            if current.contains(&language) {
                return Ok(language);
            }
            languages::canonical(&language)
                .map(|listed| listed.name().to_owned())
                .ok_or(SongLibraryError::UnlistedLanguage(language))
        })
        .collect()
}

/// Validated authored Arrangement content.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ArrangementContent {
    title: Option<String>,
    languages: Vec<String>,
    rendition_attribution: Option<String>,
    occasions: BTreeSet<String>,
    notes: Option<String>,
    sections: Vec<Section>,
    sequence: Vec<Occurrence>,
}

impl ArrangementContent {
    /// Checks every field rule before any value is stored.
    fn validate(
        arrangement: ArrangementId,
        draft: ArrangementDraft,
    ) -> Result<Self, SongLibraryError> {
        let metadata = Metadata::validate(
            draft.title,
            draft.languages,
            draft.rendition_attribution,
            draft.occasions,
            draft.notes,
        )?;
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
            title: metadata.title,
            languages: metadata.languages,
            rendition_attribution: metadata.rendition_attribution,
            occasions: metadata.occasions,
            notes: metadata.notes,
            sections: draft
                .sections
                .into_iter()
                .map(|section| Section {
                    id: section.id,
                    label: section.label,
                    lyrics: section.lyrics,
                    pagination: section.pagination,
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

/// The pagination state each stored section holds, keyed by identity.
fn section_states(content: &ArrangementContent) -> BTreeMap<SectionId, (String, PaginationState)> {
    content
        .sections
        .iter()
        .map(|section| {
            (
                section.id(),
                (section.lyrics().to_owned(), section.pagination()),
            )
        })
        .collect()
}

/// Applies Decision 4 to replacement sections.
///
/// A section identity that was not stored before starts Draft. A stored
/// section keeps its state unless it was Approved and its lyrics changed.
fn apply_review_rule(
    sections: &mut [Section],
    previous: &BTreeMap<SectionId, (String, PaginationState)>,
) {
    for section in sections {
        section.pagination = match previous.get(&section.id) {
            None => PaginationState::Draft,
            Some((lyrics, PaginationState::Approved)) if *lyrics != section.lyrics => {
                PaginationState::NeedsReview
            }
            Some((_, state)) => *state,
        };
    }
}
