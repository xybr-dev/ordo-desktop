//! Sequence operations: the sung order of an Arrangement's sections.
//!
//! [`ArrangementLyrics`] keeps reusable sections in authored order and
//! occurrences in performance order. The mutators here change only the
//! Sequence; they never copy lyrics or slides, touch a section's approval, or
//! remove a section. [`flatten`] turns the Sequence and the generated slide
//! groups into the one order the operator sings and the Inspector walks.
//!
//! Selection is view state, but its maintenance after a content replacement is
//! a pure content question, so [`selection_after_replace`] lives here too.

use std::collections::BTreeMap;

use crate::identity::{OccurrenceId, SectionId};
use crate::pagination::slides;
use crate::song_library::interface::SongLibraryError;
use crate::song_library::model::{OccurrenceDraft, SectionDraft};

use super::sections::{ArrangementLyrics, Step};

/// One generated slide in sung order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlatSlide {
    /// The occurrence that carries the slide.
    pub occurrence: OccurrenceId,
    /// The occurrence's position in the Sequence, counted from 0.
    pub position: usize,
    /// The section the occurrence places.
    pub section: SectionId,
    /// The slide's position inside its section, counted from 0.
    pub slide: usize,
    /// How many slides the section generates.
    pub slides_in_section: usize,
}

impl ArrangementLyrics {
    /// Inserts an occurrence of `section` after `after`, or appends it when
    /// `after` is `None`, and returns its position.
    ///
    /// No lyrics or slides are copied: the occurrence references the one
    /// section. The section must be one of this Arrangement's own sections and
    /// `after` must be an occurrence of this Sequence; the identity must be
    /// free. Nothing changes on refusal, and no section's pagination state is
    /// touched.
    pub fn insert_occurrence(
        &mut self,
        id: OccurrenceId,
        section: SectionId,
        after: Option<OccurrenceId>,
    ) -> Result<usize, SongLibraryError> {
        if !self.sections.iter().any(|candidate| candidate.id == section) {
            return Err(SongLibraryError::UnknownSection(section));
        }
        let after = match after {
            Some(after) => Some(
                self.occurrence_index(after)
                    .ok_or(SongLibraryError::UnknownOccurrence(after))?,
            ),
            None => None,
        };
        if self.occurrence_index(id).is_some() {
            return Err(SongLibraryError::DuplicateOccurrence(id));
        }
        let at = after.map_or(self.sequence.len(), |index| index + 1);
        self.sequence.insert(at, OccurrenceDraft::new(id, section));
        Ok(at)
    }

    /// Inserts another occurrence of `source`'s section right after `source`
    /// and returns its position.
    ///
    /// The new occurrence references the same section, so it shares its lyrics
    /// and slides; nothing is copied. Both identities are checked and nothing
    /// changes on refusal. No section's pagination state is touched.
    pub fn repeat_occurrence(
        &mut self,
        id: OccurrenceId,
        source: OccurrenceId,
    ) -> Result<usize, SongLibraryError> {
        let index = self
            .occurrence_index(source)
            .ok_or(SongLibraryError::UnknownOccurrence(source))?;
        if self.occurrence_index(id).is_some() {
            return Err(SongLibraryError::DuplicateOccurrence(id));
        }
        let section = self.sequence[index].section;
        let at = index + 1;
        self.sequence.insert(at, OccurrenceDraft::new(id, section));
        Ok(at)
    }

    /// Moves one occurrence one place up or down, whole and with every
    /// generated slide.
    ///
    /// Returns whether the occurrence moved; a move past either edge changes
    /// nothing. Sections and approvals are never touched.
    pub fn move_occurrence(
        &mut self,
        id: OccurrenceId,
        step: Step,
    ) -> Result<bool, SongLibraryError> {
        let index = self
            .occurrence_index(id)
            .ok_or(SongLibraryError::UnknownOccurrence(id))?;
        match step {
            Step::Up if index == 0 => Ok(false),
            Step::Down if index + 1 == self.sequence.len() => Ok(false),
            Step::Up => {
                self.sequence.swap(index, index - 1);
                Ok(true)
            }
            Step::Down => {
                self.sequence.swap(index, index + 1);
                Ok(true)
            }
        }
    }

    /// Moves one occurrence to `index` in the Sequence without it, clamping an
    /// index past the end, and returns whether the order changed.
    ///
    /// This is the drag path: one call is one atomic move of the occurrence
    /// and all of its slides. Sections and approvals are never touched.
    pub fn move_occurrence_to(
        &mut self,
        id: OccurrenceId,
        index: usize,
    ) -> Result<bool, SongLibraryError> {
        let current = self
            .occurrence_index(id)
            .ok_or(SongLibraryError::UnknownOccurrence(id))?;
        let last = self.sequence.len().saturating_sub(1);
        let target = index.min(last);
        if target == current {
            return Ok(false);
        }
        let occurrence = self.sequence.remove(current);
        self.sequence.insert(target, occurrence);
        Ok(true)
    }

    /// Removes one occurrence, never its section, and returns the next
    /// selection.
    ///
    /// The next selection is the occurrence now at the removed one's position,
    /// otherwise the one before it, otherwise `None`. Nothing changes when the
    /// identity names no occurrence here, and no section's pagination state is
    /// touched.
    pub fn remove_occurrence(
        &mut self,
        id: OccurrenceId,
    ) -> Result<Option<OccurrenceId>, SongLibraryError> {
        let index = self
            .occurrence_index(id)
            .ok_or(SongLibraryError::UnknownOccurrence(id))?;
        self.sequence.remove(index);
        let next = self.sequence.get(index).map(|occurrence| occurrence.id);
        Ok(next.or_else(|| {
            index
                .checked_sub(1)
                .and_then(|previous| self.sequence.get(previous))
                .map(|occurrence| occurrence.id)
        }))
    }

    /// The position of one occurrence in the Sequence, counted from 0.
    pub fn occurrence_index(&self, id: OccurrenceId) -> Option<usize> {
        self.sequence
            .iter()
            .position(|occurrence| occurrence.id == id)
    }
}

/// Every generated slide in sung order.
///
/// Each occurrence contributes its section's slides in order, and a repeated
/// section contributes them again. An occurrence of a section without slides
/// contributes nothing but still counts in [`FlatSlide::position`]. A section
/// identity that is not one of the Arrangement's own contributes nothing.
pub fn flatten(lyrics: &ArrangementLyrics) -> Vec<FlatSlide> {
    let sections: BTreeMap<SectionId, &SectionDraft> = lyrics
        .sections
        .iter()
        .map(|section| (section.id, section))
        .collect();
    let mut flat = Vec::new();
    for (position, occurrence) in lyrics.sequence.iter().enumerate() {
        let Some(section) = sections.get(&occurrence.section) else {
            continue;
        };
        let generated = slides(&section.lyrics);
        let count = generated.len();
        for slide in 0..count {
            flat.push(FlatSlide {
                occurrence: occurrence.id,
                position,
                section: occurrence.section,
                slide,
                slides_in_section: count,
            });
        }
    }
    flat
}

/// The operator-facing position of one occurrence: `01`, `02`, … `10`, `100`.
///
/// [`FlatSlide::position`] and occurrence indexes are zero-based; this label is
/// one-based and always at least two digits wide.
pub fn position_label(index: usize) -> String {
    format!("{:02}", index + 1)
}

/// The occurrence to select after content is replaced, such as on Undo.
///
/// An occurrence that reappears is selected, which restores an undone removal;
/// otherwise the current selection is kept while it still exists, then the
/// occurrence now at its old position, then the one before it, then none.
pub fn selection_after_replace(
    before: &ArrangementLyrics,
    after: &ArrangementLyrics,
    selected: Option<OccurrenceId>,
) -> Option<OccurrenceId> {
    let selected = selected?;
    if after.occurrence_index(selected).is_some() {
        return Some(selected);
    }
    let index = before.occurrence_index(selected)?;
    after
        .sequence
        .get(index)
        .map(|occurrence| occurrence.id)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|previous| after.sequence.get(previous))
                .map(|occurrence| occurrence.id)
        })
}
