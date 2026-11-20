//! History change labels and the read-only comparison model.
//!
//! Every stored change is classified from two [`ArrangementState`]s, and the
//! comparison marks what differs between any two states (the History
//! comparison and, later, import review). The rules are pure: they read
//! authored content and recorded pagination states only, and never decide a
//! restore or a write.
//!
//! A format 1 or 2 Arrangement document records no section states
//! (`states_recorded == false`). Such a side never shows a state, never marks a
//! state difference, and never produces a `Pagination*` change: a legacy
//! payload never claims an approval it did not record (Decisions 5 and 6).

use std::collections::BTreeSet;

use crate::identity::{OccurrenceId, SectionId};
use crate::pagination::slides;
use crate::song_library::model::{ArrangementDetails, OccurrenceDraft, PaginationState, SongDraft};

use super::sections::ArrangementLyrics;

/// One stored Arrangement state as History sees it.
pub struct ArrangementState<'a> {
    /// Title, languages, Choirs, attribution, occasions and notes.
    pub details: &'a ArrangementDetails,
    /// Sections, Sequence and per-section pagination states.
    pub lyrics: &'a ArrangementLyrics,
    /// Whether this document recorded section states (format 3 or later).
    pub states_recorded: bool,
}

/// What one revision changed, as far as History can tell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrangementChange {
    /// Metadata only.
    Details,
    /// One section's words changed.
    Wording {
        /// The section whose words changed.
        section: SectionId,
    },
    /// Only the blank-line grouping of one section's words changed.
    SlideBoundary {
        /// The section whose slide breaks moved.
        section: SectionId,
    },
    /// A section was added.
    SectionAdded {
        /// The added section.
        section: SectionId,
    },
    /// A section was removed.
    SectionRemoved {
        /// The removed section.
        section: SectionId,
    },
    /// A section's label changed.
    SectionRenamed {
        /// The renamed section.
        section: SectionId,
    },
    /// The Lyrics list order changed.
    SectionsReordered,
    /// A new occurrence of an already-used section entered the Sequence.
    Repeated {
        /// The repeated section.
        section: SectionId,
        /// Whether the occurrence was appended at the end.
        at_end: bool,
    },
    /// An occurrence of a section new to the Sequence was inserted.
    OccurrenceAdded {
        /// The section the occurrence places.
        section: SectionId,
        /// Its zero-based position in the new Sequence.
        position: usize,
    },
    /// One occurrence moved to another position.
    OccurrenceMoved {
        /// The section the occurrence places.
        section: SectionId,
        /// Its zero-based position in the new Sequence.
        to: usize,
    },
    /// An occurrence left the Sequence.
    OccurrenceRemoved {
        /// The section the occurrence placed.
        section: SectionId,
        /// Its zero-based position in the old Sequence.
        position: usize,
    },
    /// Pagination was approved; words are equal.
    PaginationApproved {
        /// One approved section.
        section: SectionId,
    },
    /// An approval was undone; words are equal.
    PaginationReturned {
        /// One section that is no longer approved.
        section: SectionId,
    },
    /// More than one distinct change.
    Several,
    /// Equal content, or only a difference one side did not record.
    Unchanged,
}

/// Classifies one Edit revision against its parent.
///
/// In order: metadata, section structure and labels, wording (and
/// blank-line-only slide-boundary changes), then the Sequence. A Sequence
/// change explained by a section addition or removal is suppressed. A
/// state-only difference is reported only when both sides recorded states and
/// no content changed. Anything that does not reduce to one change is
/// [`ArrangementChange::Several`].
pub fn arrangement_change(
    before: &ArrangementState<'_>,
    after: &ArrangementState<'_>,
) -> ArrangementChange {
    let mut changes = Vec::new();

    if !before.details.same_as(after.details) {
        changes.push(ArrangementChange::Details);
    }

    let before_ids: BTreeSet<SectionId> = before
        .lyrics
        .sections
        .iter()
        .map(|section| section.id)
        .collect();
    let after_ids: BTreeSet<SectionId> = after
        .lyrics
        .sections
        .iter()
        .map(|section| section.id)
        .collect();
    let removed: Vec<SectionId> = before
        .lyrics
        .sections
        .iter()
        .map(|section| section.id)
        .filter(|id| !after_ids.contains(id))
        .collect();
    let added: Vec<SectionId> = after
        .lyrics
        .sections
        .iter()
        .map(|section| section.id)
        .filter(|id| !before_ids.contains(id))
        .collect();
    for section in &removed {
        changes.push(ArrangementChange::SectionRemoved { section: *section });
    }
    for section in &added {
        changes.push(ArrangementChange::SectionAdded { section: *section });
    }

    let before_common: Vec<SectionId> = before
        .lyrics
        .sections
        .iter()
        .map(|section| section.id)
        .filter(|id| after_ids.contains(id))
        .collect();
    let after_common: Vec<SectionId> = after
        .lyrics
        .sections
        .iter()
        .map(|section| section.id)
        .filter(|id| before_ids.contains(id))
        .collect();
    if before_common != after_common {
        changes.push(ArrangementChange::SectionsReordered);
    }

    for old in &before.lyrics.sections {
        let Some(new) = after
            .lyrics
            .sections
            .iter()
            .find(|section| section.id == old.id)
        else {
            continue;
        };
        if old.label != new.label {
            changes.push(ArrangementChange::SectionRenamed { section: old.id });
        }
        if old.lyrics != new.lyrics {
            if same_non_blank_lines(&old.lyrics, &new.lyrics) {
                // A trailing newline or a whitespace-only line moves no slide
                // boundary, so it is no change worth naming.
                if slide_breaks(&old.lyrics) != slide_breaks(&new.lyrics) {
                    changes.push(ArrangementChange::SlideBoundary { section: old.id });
                }
            } else {
                changes.push(ArrangementChange::Wording { section: old.id });
            }
        }
    }

    // Occurrences of a section this revision added or removed are explained by
    // that section change; only the rest describes a Sequence edit.
    let before_sequence: Vec<_> = before
        .lyrics
        .sequence
        .iter()
        .filter(|occurrence| !removed.contains(&occurrence.section))
        .collect();
    let after_sequence: Vec<_> = after
        .lyrics
        .sequence
        .iter()
        .filter(|occurrence| !added.contains(&occurrence.section))
        .collect();
    let before_occurrences: BTreeSet<_> = before_sequence
        .iter()
        .map(|occurrence| occurrence.id)
        .collect();
    let after_occurrences: BTreeSet<_> = after_sequence
        .iter()
        .map(|occurrence| occurrence.id)
        .collect();
    let inserted: Vec<_> = after_sequence
        .iter()
        .filter(|occurrence| !before_occurrences.contains(&occurrence.id))
        .copied()
        .collect();
    let gone: Vec<_> = before_sequence
        .iter()
        .filter(|occurrence| !after_occurrences.contains(&occurrence.id))
        .copied()
        .collect();

    let kept_ids =
        |sequence: &[&OccurrenceDraft], skip: &[&OccurrenceDraft]| -> Vec<OccurrenceId> {
            sequence
                .iter()
                .filter(|occurrence| !skip.iter().any(|other| other.id == occurrence.id))
                .map(|occurrence| occurrence.id)
                .collect()
        };
    let moved_to_another_section = before_sequence.iter().any(|old| {
        after_sequence
            .iter()
            .any(|new| new.id == old.id && new.section != old.section)
    });
    if moved_to_another_section {
        changes.push(ArrangementChange::Several);
    } else if inserted.is_empty() && gone.is_empty() {
        if before_sequence
            .iter()
            .map(|occurrence| occurrence.id)
            .ne(after_sequence.iter().map(|occurrence| occurrence.id))
        {
            match single_move(&before_sequence, &after_sequence) {
                Some((section, to)) => {
                    changes.push(ArrangementChange::OccurrenceMoved { section, to });
                }
                None => changes.push(ArrangementChange::Several),
            }
        }
    } else if inserted.len() == 1
        && gone.is_empty()
        && kept_ids(&after_sequence, &inserted) == kept_ids(&before_sequence, &[])
    {
        let occurrence = inserted[0];
        let position = after_sequence
            .iter()
            .position(|candidate| candidate.id == occurrence.id)
            .unwrap_or(0);
        let already_used = before_sequence
            .iter()
            .any(|candidate| candidate.section == occurrence.section);
        if already_used {
            changes.push(ArrangementChange::Repeated {
                section: occurrence.section,
                at_end: position + 1 == after_sequence.len(),
            });
        } else {
            changes.push(ArrangementChange::OccurrenceAdded {
                section: occurrence.section,
                position,
            });
        }
    } else if gone.len() == 1
        && inserted.is_empty()
        && kept_ids(&before_sequence, &gone) == kept_ids(&after_sequence, &[])
    {
        let occurrence = gone[0];
        let position = before_sequence
            .iter()
            .position(|candidate| candidate.id == occurrence.id)
            .unwrap_or(0);
        changes.push(ArrangementChange::OccurrenceRemoved {
            section: occurrence.section,
            position,
        });
    } else {
        changes.push(ArrangementChange::Several);
    }

    // A state difference is meaningful only when both documents recorded
    // states. With no other change it is the whole revision; beside another
    // change, only a state change that edit did not cause by itself counts.
    if before.states_recorded && after.states_recorded {
        if changes.is_empty() {
            if let Some(change) = recorded_state_change(before.lyrics, after.lyrics) {
                changes.push(change);
            }
        } else if has_unexplained_state_change(before.lyrics, after.lyrics) {
            changes.push(ArrangementChange::Several);
        }
    }

    match changes.len() {
        0 => ArrangementChange::Unchanged,
        1 => changes.pop().unwrap_or(ArrangementChange::Unchanged),
        _ => ArrangementChange::Several,
    }
}

/// Which topic a session's changes cover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionTopic {
    /// At least one change was not metadata.
    Lyrics,
    /// Every change was metadata.
    Details,
}

/// The topic a session's History entry names: Details when at least one
/// change is metadata and every other change is metadata or unchanged;
/// otherwise Lyrics (an empty list included).
pub fn session_topic(changes: &[ArrangementChange]) -> SessionTopic {
    let details = |change: &ArrangementChange| matches!(change, ArrangementChange::Details);
    if changes.iter().any(details)
        && changes
            .iter()
            .all(|change| details(change) || matches!(change, ArrangementChange::Unchanged))
    {
        SessionTopic::Details
    } else {
        SessionTopic::Lyrics
    }
}

/// One authored Song field a Song revision changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SongField {
    /// The primary title.
    Title,
    /// Alternate titles.
    Aliases,
    /// Original attribution.
    Attribution,
    /// Provenance.
    Provenance,
    /// Rights notes.
    RightsNotes,
    /// Liturgical moments.
    Moments,
    /// Occasion applicability tags.
    Occasions,
}

/// The Song fields that differ between two drafts, in declaration order.
///
/// Moments and occasions are sets, so reordering them is not a change. Aliases
/// are an authored order, so their order counts.
pub fn song_changed_fields(before: &SongDraft, after: &SongDraft) -> Vec<SongField> {
    let mut fields = Vec::new();
    if before.title != after.title {
        fields.push(SongField::Title);
    }
    if before.aliases != after.aliases {
        fields.push(SongField::Aliases);
    }
    if before.original_attribution != after.original_attribution {
        fields.push(SongField::Attribution);
    }
    if before.provenance != after.provenance {
        fields.push(SongField::Provenance);
    }
    if before.rights_notes != after.rights_notes {
        fields.push(SongField::RightsNotes);
    }
    let mut before_moments = before.liturgical_moments.clone();
    before_moments.sort_unstable();
    let mut after_moments = after.liturgical_moments.clone();
    after_moments.sort_unstable();
    if before_moments != after_moments {
        fields.push(SongField::Moments);
    }
    let mut before_occasions = before.occasions.clone();
    before_occasions.sort_unstable();
    let mut after_occasions = after.occasions.clone();
    after_occasions.sort_unstable();
    if before_occasions != after_occasions {
        fields.push(SongField::Occasions);
    }
    fields
}

/// The Overview tab's marks for one side of a comparison.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverviewMarks {
    /// Languages differ.
    pub languages: bool,
    /// Choir associations differ.
    pub choirs: bool,
    /// The section count differs.
    pub sections: bool,
    /// The occurrence count or total slide count differs.
    pub sequence: bool,
    /// The display title differs.
    pub title: bool,
    /// The rendition attribution differs.
    pub attribution: bool,
    /// Occasion tags differ.
    pub occasions: bool,
    /// Source and rights notes differ.
    pub notes: bool,
}

/// One side of a read-only Arrangement comparison.
pub struct ArrangementComparison {
    /// The Overview tab's marks for this side.
    pub overview: OverviewMarks,
    /// One entry per section of this side, in its authored order.
    pub sections: Vec<SectionMarks>,
    /// One mark per occurrence of this side: whether the section at that
    /// position differs.
    pub sequence: Vec<bool>,
}

/// One section's marks on one side of a comparison.
pub struct SectionMarks {
    /// The section this entry describes.
    pub section: SectionId,
    /// One mark per raw line of this side, in order.
    pub lines: Vec<LineMark>,
    /// This side's state, or `None` when it recorded none.
    pub state: Option<PaginationState>,
    /// Whether both sides recorded a state and they differ.
    pub state_differs: bool,
}

/// How one line of this side compares with the other side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineMark {
    /// The same line at the same position among this section's non-blank
    /// lines, or a slide break the other side has at the same position.
    Same,
    /// A different line, or any line of a section the other side lacks.
    Changed,
    /// A slide break the other side does not have at this position.
    SlideBreak,
}

/// Marks what differs between `this` and `other` for one side of the
/// comparison (mockup 808-815).
///
/// `sections` describes `this`'s sections only: non-blank lines are compared by
/// position with `other`'s non-blank lines for the same section identity, and a
/// section `other` no longer has is wholly `Changed`. Blank lines are
/// [`LineMark::SlideBreak`]. The Sequence compares the section placed at each
/// position. When either side recorded no states, that side's `state` is
/// `None`, `state_differs` is false, and no difference is claimed.
pub fn compare_arrangements(
    this: &ArrangementState<'_>,
    other: &ArrangementState<'_>,
) -> ArrangementComparison {
    let overview = OverviewMarks {
        languages: this.details.languages != other.details.languages,
        choirs: sorted(&this.details.choirs) != sorted(&other.details.choirs),
        sections: this.lyrics.sections.len() != other.lyrics.sections.len(),
        sequence: this.lyrics.sequence.len() != other.lyrics.sequence.len()
            || slide_count(this.lyrics) != slide_count(other.lyrics),
        title: this.details.title != other.details.title,
        attribution: this.details.rendition_attribution != other.details.rendition_attribution,
        occasions: sorted(&this.details.occasions) != sorted(&other.details.occasions),
        notes: this.details.notes != other.details.notes,
    };

    let sections = this
        .lyrics
        .sections
        .iter()
        .map(|section| {
            let counterpart = other
                .lyrics
                .sections
                .iter()
                .find(|candidate| candidate.id == section.id);
            let lines = match counterpart {
                None => section.lyrics.lines().map(|_| LineMark::Changed).collect(),
                Some(candidate) => {
                    let other_lines = non_blank_lines(&candidate.lyrics);
                    let other_breaks = slide_breaks(&candidate.lyrics);
                    let mut position = 0;
                    section
                        .lyrics
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                if other_breaks.contains(&position) {
                                    LineMark::Same
                                } else {
                                    LineMark::SlideBreak
                                }
                            } else {
                                let mark = if other_lines.get(position) == Some(&line) {
                                    LineMark::Same
                                } else {
                                    LineMark::Changed
                                };
                                position += 1;
                                mark
                            }
                        })
                        .collect()
                }
            };
            let state = this.states_recorded.then_some(section.pagination);
            let state_differs = this.states_recorded
                && other.states_recorded
                && counterpart.is_some_and(|candidate| candidate.pagination != section.pagination);
            SectionMarks {
                section: section.id,
                lines,
                state,
                state_differs,
            }
        })
        .collect();

    let sequence = this
        .lyrics
        .sequence
        .iter()
        .enumerate()
        .map(|(position, occurrence)| {
            other
                .lyrics
                .sequence
                .get(position)
                .is_none_or(|candidate| candidate.section != occurrence.section)
        })
        .collect();

    ArrangementComparison {
        overview,
        sections,
        sequence,
    }
}

/// The one moved occurrence, if exactly one identity explains the reordering.
///
/// Moving one occurrence by one place is also explained by moving its
/// neighbour the other way; the occurrence that travelled furthest wins, and
/// on a tie the one that moved toward the start (a "Move up" of it).
fn single_move(
    before: &[&OccurrenceDraft],
    after: &[&OccurrenceDraft],
) -> Option<(SectionId, usize)> {
    if before.len() != after.len() {
        return None;
    }
    let mut best: Option<(usize, bool, SectionId, usize)> = None;
    for (position, occurrence) in after.iter().enumerate() {
        let from = before
            .iter()
            .position(|candidate| candidate.id == occurrence.id)?;
        if from == position {
            continue;
        }
        let rest = |sequence: &[&OccurrenceDraft]| -> Vec<OccurrenceId> {
            sequence
                .iter()
                .filter(|candidate| candidate.id != occurrence.id)
                .map(|candidate| candidate.id)
                .collect()
        };
        if rest(before) == rest(after) {
            let key = (from.abs_diff(position), position < from, occurrence.section);
            if best.is_none_or(|current| (key.0, key.1) > (current.0, current.1)) {
                best = Some((key.0, key.1, key.2, position));
            }
        }
    }
    best.map(|(_, _, section, to)| (section, to))
}

/// One recorded state change, or `None` when no state differs.
///
/// Exactly one section approved is `PaginationApproved`; exactly one approval
/// withdrawn is `PaginationReturned`; any other mix is `Several`. The caller
/// has already established that the content is equal.
fn recorded_state_change(
    before: &ArrangementLyrics,
    after: &ArrangementLyrics,
) -> Option<ArrangementChange> {
    let mut approved = Vec::new();
    let mut returned = Vec::new();
    let mut other = 0_usize;
    for old in &before.sections {
        let Some(new) = after.sections.iter().find(|section| section.id == old.id) else {
            continue;
        };
        if old.pagination == new.pagination {
            continue;
        }
        if old.pagination == PaginationState::Approved {
            returned.push(old.id);
        } else if new.pagination == PaginationState::Approved {
            approved.push(old.id);
        } else {
            other += 1;
        }
    }
    match (approved.as_slice(), returned.as_slice(), other) {
        ([], [], 0) => None,
        ([section], [], 0) => Some(ArrangementChange::PaginationApproved { section: *section }),
        ([], [section], 0) => Some(ArrangementChange::PaginationReturned { section: *section }),
        _ => Some(ArrangementChange::Several),
    }
}

/// Whether a section's state changed in a way no edit explains. Any state
/// change on a section whose own words changed, and any state on an added
/// section, belongs to that edit (an Undo writes the old words with the old
/// approval); only state changes on untouched sections count.
fn has_unexplained_state_change(before: &ArrangementLyrics, after: &ArrangementLyrics) -> bool {
    after.sections.iter().any(
        |new| match before.sections.iter().find(|old| old.id == new.id) {
            Some(old) => old.pagination != new.pagination && old.lyrics == new.lyrics,
            None => false,
        },
    )
}

/// The positions of slide breaks: each is the number of non-blank lines
/// before a blank line.
fn slide_breaks(text: &str) -> BTreeSet<usize> {
    let mut breaks = BTreeSet::new();
    let mut position = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            breaks.insert(position);
        } else {
            position += 1;
        }
    }
    breaks
}

/// The non-blank lines of `text`, in order.
fn non_blank_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

/// Whether two lyrics values differ only in blank-line grouping.
fn same_non_blank_lines(before: &str, after: &str) -> bool {
    non_blank_lines(before) == non_blank_lines(after)
}

/// Total generated slides across the Sequence.
fn slide_count(lyrics: &ArrangementLyrics) -> usize {
    lyrics
        .sequence
        .iter()
        .filter_map(|occurrence| {
            lyrics
                .sections
                .iter()
                .find(|section| section.id == occurrence.section)
        })
        .map(|section| slides(&section.lyrics).len())
        .sum()
}

/// A sorted copy, for comparing set-valued details.
fn sorted<T: Clone + Ord>(values: &[T]) -> Vec<T> {
    let mut values = values.to_vec();
    values.sort_unstable();
    values
}
