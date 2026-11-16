//! History rows: the sessions and revisions of one entity in the operator's
//! words. Pure functions of the stored revisions, the current moment and the
//! local offset, so they run on a background task and test against a fixed
//! clock.

use std::collections::HashMap;

use domain::{
    identity::{RevisionId, SectionId},
    revisions::{HistoryEntry, RevisionKind, RevisionSummary, group_history},
    song_library::{
        ArrangementChange, ArrangementDetails, ArrangementLyrics, ArrangementState, SessionTopic,
        SongField, arrangement_change, position_label, session_topic, song_changed_fields,
    },
};
use storage::{HistoryRevision, RevisionContent};
use time::{OffsetDateTime, UtcOffset};

use super::time::{clock, session_when, when};

/// One History row: a session of edits, or a standalone event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRow {
    /// What happened, in bold: `Edited lyrics \u{b7} 3 changes`, `Created arrangement`.
    pub what: String,
    /// When: `Today \u{b7} 10:37\u{2013}10:44`.
    pub when: String,
    /// How: `Manual edit`, `Creation`, `Import \u{b7} whole-Arrangement replacement`, `Restore`.
    pub how: &'static str,
    /// The session's revisions, newest first.
    pub revisions: Vec<RevisionRow>,
}

/// One revision inside a [`SessionRow`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionRow {
    /// The revision this row opens.
    pub id: RevisionId,
    /// Its time of day: `10:44`.
    pub at: String,
    /// What it changed: `Corrected Chorus wording`.
    pub what: String,
    /// Whether this is the entity's head, which is not a revision to restore.
    pub current: bool,
}

/// Every row of `history` (oldest first, as storage returns it), newest first.
pub fn rows(
    history: &[HistoryRevision],
    now: OffsetDateTime,
    offset: UtcOffset,
) -> Vec<SessionRow> {
    let by_id: HashMap<RevisionId, usize> = history
        .iter()
        .enumerate()
        .map(|(at, revision)| (revision.record.id, at))
        .collect();
    let summaries: Vec<RevisionSummary> = history
        .iter()
        .map(|revision| RevisionSummary {
            id: revision.record.id,
            entity: revision.record.entity,
            kind: revision.record.kind,
            session: revision.record.session,
            created_at: revision.record.created_at,
        })
        .collect();
    let head = history.last().map(|revision| revision.record.id);
    let describe_one = |summary: &RevisionSummary| -> (String, Option<ArrangementChange>) {
        let Some(&at) = by_id.get(&summary.id) else {
            return ("Several changes".to_owned(), None);
        };
        describe(history, &by_id, at)
    };
    let revision_row = |summary: &RevisionSummary, what: String| RevisionRow {
        id: summary.id,
        at: clock(summary.created_at, offset),
        what,
        current: Some(summary.id) == head,
    };
    let mut rows: Vec<SessionRow> = group_history(&summaries)
        .into_iter()
        .map(|entry| match entry {
            HistoryEntry::Session { revisions, .. } => {
                let described: Vec<_> = revisions.iter().map(&describe_one).collect();
                let song = matches!(
                    history.get(by_id.get(&revisions[0].id).copied().unwrap_or(0)),
                    Some(HistoryRevision {
                        content: RevisionContent::Song(_),
                        ..
                    })
                );
                let changes: Vec<ArrangementChange> = described
                    .iter()
                    .filter_map(|(_, change)| change.clone())
                    .collect();
                let topic = if song {
                    SessionTopic::Details
                } else {
                    session_topic(&changes)
                };
                let count = revisions.len();
                let what = format!(
                    "Edited {} \u{b7} {count} {}",
                    match topic {
                        SessionTopic::Lyrics => "lyrics",
                        SessionTopic::Details => "details",
                    },
                    if count == 1 { "change" } else { "changes" }
                );
                let first = revisions[0].created_at;
                let last = revisions[count - 1].created_at;
                SessionRow {
                    what,
                    when: session_when(first, last, now, offset),
                    how: "Manual edit",
                    revisions: revisions
                        .iter()
                        .zip(described)
                        .rev()
                        .map(|(summary, (what, _))| revision_row(summary, what))
                        .collect(),
                }
            }
            HistoryEntry::Event(summary) => {
                let at = by_id.get(&summary.id).copied().unwrap_or(0);
                let (what, how, revision_what) = event(history, &by_id, at, now, offset);
                SessionRow {
                    what,
                    when: session_when(summary.created_at, summary.created_at, now, offset),
                    how,
                    revisions: vec![revision_row(&summary, revision_what)],
                }
            }
        })
        .collect();
    rows.reverse();
    rows
}

/// The row of a standalone event: its bold line, its `how`, and its one
/// revision's line.
fn event(
    history: &[HistoryRevision],
    by_id: &HashMap<RevisionId, usize>,
    at: usize,
    now: OffsetDateTime,
    offset: UtcOffset,
) -> (String, &'static str, String) {
    let revision = &history[at];
    match revision.record.kind {
        RevisionKind::Creation => match &revision.content {
            RevisionContent::Arrangement(content) => (
                "Created arrangement".to_owned(),
                "Creation",
                format!("Created with {}", content.details().languages.join(", ")),
            ),
            RevisionContent::Song(content) => (
                "Created song".to_owned(),
                "Creation",
                format!("Created {}", content.draft().title),
            ),
        },
        RevisionKind::Import | RevisionKind::Replacement => (
            "Imported".to_owned(),
            "Import \u{b7} whole-Arrangement replacement",
            "Imported".to_owned(),
        ),
        RevisionKind::Restoration => {
            let from = revision
                .record
                .restored_from
                .and_then(|id| by_id.get(&id))
                .map(|&from| when(history[from].record.created_at, now, offset));
            (
                "Restored a revision".to_owned(),
                "Restore",
                from.map_or_else(
                    || "Restored an earlier revision".to_owned(),
                    |when| format!("Restored the revision from {when}"),
                ),
            )
        }
        // An Edit with no session is its own row.
        RevisionKind::Edit => {
            let (what, _) = describe(history, by_id, at);
            (what.clone(), "Manual edit", what)
        }
    }
}

/// What one revision changed against its parent, in words, and the
/// Arrangement change behind it (for the session's topic).
fn describe(
    history: &[HistoryRevision],
    by_id: &HashMap<RevisionId, usize>,
    at: usize,
) -> (String, Option<ArrangementChange>) {
    let revision = &history[at];
    let parent = revision
        .record
        .parent
        .and_then(|id| by_id.get(&id))
        .map(|&parent| &history[parent]);
    match (
        &revision.content,
        parent.map(|parent| (&parent.content, parent)),
    ) {
        (
            RevisionContent::Arrangement(after),
            Some((RevisionContent::Arrangement(before), parent)),
        ) => {
            let (before_details, before_lyrics) = (before.details(), before.lyrics());
            let (after_details, after_lyrics) = (after.details(), after.lyrics());
            let change = arrangement_change(
                &state(&before_details, &before_lyrics, parent.pagination_recorded),
                &state(&after_details, &after_lyrics, revision.pagination_recorded),
            );
            let what = arrangement_words(&change, &before_lyrics, &after_lyrics);
            (what, Some(change))
        }
        (RevisionContent::Song(after), Some((RevisionContent::Song(before), _))) => {
            let fields = song_changed_fields(&before.draft(), &after.draft());
            let what = match fields.as_slice() {
                [] => "Saved without changes".to_owned(),
                [field] => format!("Edited {}", field_words(*field)),
                _ => "Edited details".to_owned(),
            };
            (what, None)
        }
        _ => (
            "Several changes".to_owned(),
            Some(ArrangementChange::Several),
        ),
    }
}

fn state<'a>(
    details: &'a ArrangementDetails,
    lyrics: &'a ArrangementLyrics,
    states_recorded: bool,
) -> ArrangementState<'a> {
    ArrangementState {
        details,
        lyrics,
        states_recorded,
    }
}

fn field_words(field: SongField) -> &'static str {
    match field {
        SongField::Title => "title",
        SongField::Aliases => "alternate titles",
        SongField::Attribution => "attribution",
        SongField::Provenance => "provenance",
        SongField::RightsNotes => "rights notes",
        SongField::Moments => "liturgical moments",
        SongField::Occasions => "occasions",
    }
}

/// A section's name in whichever side has it.
fn name_of(id: SectionId, first: &ArrangementLyrics, second: &ArrangementLyrics) -> String {
    [first, second]
        .into_iter()
        .find_map(|lyrics| lyrics.sections.iter().find(|section| section.id == id))
        .and_then(|section| section.label.clone())
        .unwrap_or_else(|| "Unnamed section".to_owned())
}

/// The one line for one change (the WP2 table).
fn arrangement_words(
    change: &ArrangementChange,
    before: &ArrangementLyrics,
    after: &ArrangementLyrics,
) -> String {
    match change {
        ArrangementChange::Details => "Edited details".to_owned(),
        ArrangementChange::Wording { section } => {
            format!("Corrected {} wording", name_of(*section, after, before))
        }
        ArrangementChange::SlideBoundary { .. } => "Adjusted a slide boundary".to_owned(),
        ArrangementChange::SectionAdded { section } => {
            format!("Added {}", name_of(*section, after, before))
        }
        ArrangementChange::SectionRemoved { section } => {
            format!("Removed {}", name_of(*section, before, after))
        }
        ArrangementChange::SectionRenamed { section } => {
            format!("Renamed {}", name_of(*section, after, before))
        }
        ArrangementChange::SectionsReordered => "Reordered sections".to_owned(),
        ArrangementChange::Repeated { section, at_end } => {
            let name = name_of(*section, after, before);
            if *at_end {
                format!("Repeated {name} at the end")
            } else {
                format!("Repeated {name}")
            }
        }
        ArrangementChange::OccurrenceAdded { section, position } => format!(
            "Added {} at {}",
            name_of(*section, after, before),
            position_label(*position)
        ),
        ArrangementChange::OccurrenceMoved { section, to } => format!(
            "Moved {} to {}",
            name_of(*section, after, before),
            position_label(*to)
        ),
        ArrangementChange::OccurrenceRemoved { section, position } => format!(
            "Removed {} {} from the Sequence",
            position_label(*position),
            name_of(*section, before, after)
        ),
        ArrangementChange::PaginationApproved { .. } => "Approved pagination".to_owned(),
        ArrangementChange::PaginationReturned { .. } => "Undid pagination approval".to_owned(),
        ArrangementChange::Several => "Several changes".to_owned(),
        ArrangementChange::Unchanged => "Saved without changes".to_owned(),
    }
}
