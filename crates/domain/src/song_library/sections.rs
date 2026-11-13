//! Section operations and the Lyrics editor's content type.
//!
//! [`ArrangementLyrics`] is the working copy the Arrangement editor changes and
//! hands to
//! [`crate::song_library::SongLibrary::edit_arrangement_lyrics`]. Its mutators
//! enforce the rules that can be decided inside the working copy and never
//! touch storage; the review rule runs separately in
//! [`ArrangementLyrics::reviewed_against`], so an edit and its Undo can both be
//! committed with the state the editor intends.

use crate::identity::SectionId;
use crate::pagination::{join_with_previous, slides, split_after, split_point};
use crate::song_library::interface::{Field, SongLibraryError, match_key};
use crate::song_library::model::text;
use crate::song_library::model::{OccurrenceDraft, PaginationState, SectionDraft};

/// The direction one section moves in the Lyrics list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Toward the start of the list.
    Up,
    /// Toward the end of the list.
    Down,
}

/// What the Lyrics editor edits: sections, Sequence, review state.
///
/// Sections are in authoring order, occurrences in performance order. The
/// pagination state is carried here because the revision payload does not
/// store it: a forward edit applies the review rule with
/// [`Self::reviewed_against`], and Undo commits the earlier snapshot with its
/// state intact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrangementLyrics {
    /// Reusable sections in authored order.
    pub sections: Vec<SectionDraft>,
    /// Section placements in performance order.
    pub sequence: Vec<OccurrenceDraft>,
    /// The pagination review state this content is committed with.
    pub pagination: PaginationState,
}

impl ArrangementLyrics {
    /// Appends a named section with no lyrics and returns its position.
    ///
    /// The name is trimmed, must not be blank, and must be unique by
    /// [`match_key`] within the Arrangement. The identity must not already name
    /// a section here. Nothing changes on refusal.
    pub fn add_section(&mut self, id: SectionId, name: &str) -> Result<usize, SongLibraryError> {
        let name = name.trim();
        text::require_present(Field::SectionLabel, name)?;
        require_unique_name(&self.sections, name, None)?;
        if self.sections.iter().any(|section| section.id == id) {
            return Err(SongLibraryError::DuplicateSection(id));
        }
        self.sections.push(SectionDraft::labeled(id, name, ""));
        Ok(self.sections.len() - 1)
    }

    /// Renames one section.
    ///
    /// The name is trimmed and follows the same rule as [`Self::add_section`],
    /// so it may not repeat another section's name. A name whose match key
    /// equals the section's own current key is allowed without that check (it
    /// only changes the spelling), so a section that shares a stored duplicate
    /// name can still be respelled. Nothing changes on refusal.
    pub fn rename_section(&mut self, id: SectionId, name: &str) -> Result<(), SongLibraryError> {
        let index = self.position(id)?;
        let name = name.trim();
        text::require_present(Field::SectionLabel, name)?;
        let own_key = self.sections[index].label.as_deref().map(match_key);
        if own_key != Some(match_key(name)) {
            require_unique_name(&self.sections, name, Some(id))?;
        }
        self.sections[index].label = Some(name.to_owned());
        Ok(())
    }

    /// Replaces one section's lyrics exactly as given.
    ///
    /// Empty and whitespace-only text is valid: a section may have no words.
    /// Nothing changes when the identity names no section here.
    pub fn set_section_lyrics(
        &mut self,
        id: SectionId,
        lyrics: String,
    ) -> Result<(), SongLibraryError> {
        let index = self.position(id)?;
        self.sections[index].lyrics = lyrics;
        Ok(())
    }

    /// Moves one section one place up or down in the Lyrics list.
    ///
    /// Returns whether the section moved; a move past either edge changes
    /// nothing. The Sequence and the pagination state are never touched, and
    /// order alone never affects pagination.
    pub fn move_section(&mut self, id: SectionId, step: Step) -> Result<bool, SongLibraryError> {
        let index = self.position(id)?;
        match step {
            Step::Up if index == 0 => Ok(false),
            Step::Down if index + 1 == self.sections.len() => Ok(false),
            Step::Up => {
                self.sections.swap(index, index - 1);
                Ok(true)
            }
            Step::Down => {
                self.sections.swap(index, index + 1);
                Ok(true)
            }
        }
    }

    /// Removes one section and every occurrence that places it.
    ///
    /// Returns how many occurrences were removed. The remaining occurrences
    /// keep their relative order. Nothing changes when the identity names no
    /// section here.
    pub fn remove_section(&mut self, id: SectionId) -> Result<usize, SongLibraryError> {
        let index = self.position(id)?;
        self.sections.remove(index);
        let before = self.sequence.len();
        self.sequence.retain(|occurrence| occurrence.section != id);
        Ok(before - self.sequence.len())
    }

    /// How many Sequence occurrences place one section.
    pub fn occurrences_of(&self, id: SectionId) -> usize {
        self.sequence
            .iter()
            .filter(|occurrence| occurrence.section == id)
            .count()
    }

    /// Splits one section's slide at its middle line.
    ///
    /// The slide is split after [`split_point`], so each side keeps at least
    /// one line. Returns whether the lyrics changed; a missing slide, a
    /// one-line slide, or an unknown section changes nothing.
    pub fn split_slide(&mut self, id: SectionId, slide: usize) -> Result<bool, SongLibraryError> {
        let index = self.position(id)?;
        let lyrics = &self.sections[index].lyrics;
        let Some(target) = slides(lyrics).get(slide).cloned() else {
            return Ok(false);
        };
        let Some(line) = split_point(&target) else {
            return Ok(false);
        };
        let Some(split) = split_after(lyrics, slide, line) else {
            return Ok(false);
        };
        self.sections[index].lyrics = split;
        Ok(true)
    }

    /// Joins one section's slide with the slide before it.
    ///
    /// The blank run between them becomes one newline. Returns whether the
    /// lyrics changed; the first slide, a missing slide, or an unknown section
    /// changes nothing.
    pub fn join_slide(&mut self, id: SectionId, slide: usize) -> Result<bool, SongLibraryError> {
        let index = self.position(id)?;
        let lyrics = &self.sections[index].lyrics;
        let Some(joined) = join_with_previous(lyrics, slide) else {
            return Ok(false);
        };
        self.sections[index].lyrics = joined;
        Ok(true)
    }

    /// Applies the review rule against `baseline`.
    ///
    /// An approved Arrangement returns to [`PaginationState::NeedsReview`]
    /// when any section's lyrics differ by identity, when the set of section
    /// identities differs, or when the Sequence differs. Renaming a section
    /// and reordering the Lyrics list alone do not count. Draft and
    /// already-needs-review states pass through unchanged.
    #[must_use]
    pub fn reviewed_against(mut self, baseline: &Self) -> Self {
        if self.pagination == PaginationState::Approved && lyrics_content_changed(baseline, &self) {
            self.pagination = PaginationState::NeedsReview;
        }
        self
    }

    /// The position of one section, or [`SongLibraryError::UnknownSection`].
    fn position(&self, id: SectionId) -> Result<usize, SongLibraryError> {
        self.sections
            .iter()
            .position(|section| section.id == id)
            .ok_or(SongLibraryError::UnknownSection(id))
    }
}

/// The next numbered verse and the still-free name suggestions for a new
/// section.
///
/// The first language decides the wording. Cebuano offers `Bersikulo N`,
/// `Koro` and `Katapusan`; every other language, including none, offers
/// `Verse N`, `Chorus`, `Refrain`, `Bridge` and `Ending`. `N` is one past the
/// highest number already used with that word, so the first section gets
/// `Verse 1` or `Bersikulo 1`. The prefill is always the numbered verse; the
/// suggestion list starts with it and leaves out every name an existing
/// section already holds, compared with [`match_key`].
pub fn section_name_suggestions(
    languages: &[String],
    lyrics: &ArrangementLyrics,
) -> (String, Vec<String>) {
    let cebuano = languages
        .first()
        .is_some_and(|language| match_key(language) == "cebuano");
    let (word, fixed): (&str, &[&str]) = if cebuano {
        ("Bersikulo", &["Koro", "Katapusan"])
    } else {
        ("Verse", &["Chorus", "Refrain", "Bridge", "Ending"])
    };
    let prefill = format!("{word} {}", next_number(word, &lyrics.sections));
    let mut taken: std::collections::BTreeSet<String> = lyrics
        .sections
        .iter()
        .filter_map(|section| section.label.as_deref())
        .map(match_key)
        .collect();
    let mut suggestions = Vec::new();
    for suggestion in std::iter::once(prefill.clone())
        .chain(fixed.iter().map(|suggestion| (*suggestion).to_owned()))
    {
        if taken.insert(match_key(&suggestion)) {
            suggestions.push(suggestion);
        }
    }
    (prefill, suggestions)
}

/// Checks that every present section name is non-blank.
///
/// Uniqueness is not checked here: it is enforced where a name is introduced
/// (add and rename), so a snapshot committed once can always be committed
/// again and duplicate names already stored keep loading, editing and undoing.
pub(crate) fn require_present_labels(sections: &[SectionDraft]) -> Result<(), SongLibraryError> {
    for section in sections {
        if let Some(label) = section.label.as_deref() {
            text::require_present(Field::SectionLabel, label)?;
        }
    }
    Ok(())
}

/// Whether `next` changes content that affects slide pagination, compared by
/// section identity.
fn lyrics_content_changed(baseline: &ArrangementLyrics, next: &ArrangementLyrics) -> bool {
    if baseline.sequence != next.sequence {
        return true;
    }
    lyrics_by_section(baseline) != lyrics_by_section(next)
}

/// The lyrics of each section keyed by identity.
fn lyrics_by_section(lyrics: &ArrangementLyrics) -> std::collections::BTreeMap<SectionId, &str> {
    lyrics
        .sections
        .iter()
        .map(|section| (section.id, section.lyrics.as_str()))
        .collect()
}

/// Rejects a name another section already holds, ignoring `except`.
fn require_unique_name(
    sections: &[SectionDraft],
    name: &str,
    except: Option<SectionId>,
) -> Result<(), SongLibraryError> {
    let key = match_key(name);
    let repeated = sections.iter().any(|section| {
        Some(section.id) != except
            && section
                .label
                .as_deref()
                .is_some_and(|label| match_key(label) == key)
    });
    if repeated {
        Err(SongLibraryError::DuplicateField(Field::SectionLabel))
    } else {
        Ok(())
    }
}

/// One past the highest number already used as `{word} {number}`, or 1.
fn next_number(word: &str, sections: &[SectionDraft]) -> u32 {
    let prefix = format!("{} ", match_key(word));
    sections
        .iter()
        .filter_map(|section| section.label.as_deref())
        .filter_map(|label| {
            let key = match_key(label);
            let number = key.strip_prefix(&prefix)?;
            if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            number.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}
