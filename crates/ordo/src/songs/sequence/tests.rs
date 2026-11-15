//! The Sequence's registry rows, editor state and header readiness.

use std::collections::BTreeSet;

use domain::{
    identity::OccurrenceId,
    pagination::{slide_body, slides},
    revisions::EditPoint,
    song_library::{ArrangementLyrics, PaginationState, ReadinessState, readiness as rate},
};
use gpui::{Entity, KeyBindingContextPredicate, KeyContext, TestAppContext, VisualTestContext};

use super::{SequenceSelection, readiness};
use crate::{
    commands::registry::{self, CommandId, Group, Scope},
    edit_session::SaveState,
    songs::{
        choirs::tests::{present, wait},
        lyrics::{
            ArrangementEditor,
            tests::{entity, in_memory, open_editor, parish_editor, parish_full, save_state},
        },
        new_song::tests::{finish, settle},
    },
};

/// Whether the binding of `keystroke` fires with `stack` (outermost first) as
/// the focus path.
fn fires(keystroke: &str, stack: &[&str]) -> bool {
    let contexts: Vec<KeyContext> = stack
        .iter()
        .map(|name| KeyContext::parse(name).expect("context"))
        .collect();
    let rows: Vec<_> = registry::ENTRIES
        .iter()
        .filter(|entry| entry.keys.contains(&keystroke))
        .collect();
    assert_eq!(rows.len(), 1, "{keystroke} has exactly one row");
    let predicate =
        KeyBindingContextPredicate::parse(&registry::editing_predicate(rows[0].id)).expect("parse");
    predicate.eval(&contexts)
}

fn rows_with(keystroke: &str) -> Vec<CommandId> {
    registry::ENTRIES
        .iter()
        .filter(|entry| entry.keys.contains(&keystroke))
        .map(|entry| entry.id)
        .collect()
}

const GUARDED: [&str; 8] = [
    "TextEntry",
    "Input",
    "NumberInput",
    "Combobox",
    "TimeField",
    "Dialog",
    "Sheet",
    "Modal",
];

#[test]
fn ac4_alt_up_and_down_are_one_registry_pair_for_sections_and_occurrences() {
    for (keystroke, id, label) in [
        ("alt-up", CommandId::MoveUp, "Move up"),
        ("alt-down", CommandId::MoveDown, "Move down"),
    ] {
        assert_eq!(rows_with(keystroke), [id], "one row for {keystroke}");
        let row = registry::entry_for(id);
        assert_eq!(row.label, label);
        assert_eq!(row.description, "Move the focused section or occurrence");
        assert_eq!((row.group, row.scope), (Group::Editing, Scope::Editing));
        for list in ["LyricsSections", "SequenceOccurrences"] {
            assert!(fires(keystroke, &["EditSession", list]), "{list}");
            assert!(fires(keystroke, &[list]), "{list}");
            for guard in GUARDED {
                assert!(!fires(keystroke, &["EditSession", list, guard]), "{guard}");
            }
        }
        for stack in [
            &["Presenter"][..],
            &["Presenter", "Timeline"],
            &["EditSession"],
            &[],
        ] {
            assert!(!fires(keystroke, stack), "{stack:?}");
        }
    }
}

#[test]
fn ac5_delete_is_bound_only_in_the_occurrence_list() {
    assert_eq!(rows_with("delete"), [CommandId::RemoveOccurrence]);
    let row = registry::entry_for(CommandId::RemoveOccurrence);
    assert_eq!(row.keycaps, "Delete");
    assert_eq!(row.label, "Remove from Sequence");
    assert_eq!(
        row.description,
        "Remove the focused occurrence from the Sequence"
    );
    assert_eq!((row.group, row.scope), (Group::Editing, Scope::Editing));
    assert!(fires("delete", &["EditSession", "SequenceOccurrences"]));
    for stack in [
        &["EditSession", "LyricsSections"][..],
        &["EditSession"],
        &["Presenter"],
        &[],
    ] {
        assert!(!fires("delete", stack), "{stack:?}");
    }
    let predicate = registry::editing_predicate(CommandId::RemoveOccurrence);
    assert!(
        predicate.starts_with("SequenceOccurrences && !"),
        "{predicate}"
    );
}

#[gpui::test]
fn ac5_delete_in_a_text_field_or_dialog_does_nothing(cx: &mut TestAppContext) {
    for guard in GUARDED {
        assert!(
            !fires("delete", &["EditSession", "SequenceOccurrences", guard]),
            "{guard}"
        );
    }
    // Through the real keys: Delete with focus in the lyric editor leaves the
    // Sequence alone.
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    let before = sequence_of(&editor, vcx);
    assert_eq!(before.len(), 3);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), before);
}

fn sequence_of(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Vec<OccurrenceId> {
    editor.read_with(vcx, |editor, cx| {
        editor
            .content(cx)
            .expect("content")
            .sequence
            .iter()
            .map(|occurrence| occurrence.id)
            .collect()
    })
}

fn content_of(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> ArrangementLyrics {
    editor.read_with(vcx, |editor, cx| editor.content(cx).expect("content"))
}

#[gpui::test]
fn ac6_header_shows_incomplete_and_the_tab_marker_for_an_empty_sequence(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t28-wp5-empty-sequence");
    let editor = open_editor(&shell, id, vcx);
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| content.sequence.clear(),
            cx,
        )
    });
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    let verdict = editor
        .read_with(vcx, |editor, cx| editor.readiness(cx))
        .expect("verdict");
    assert_eq!(verdict.state, ReadinessState::Incomplete);
    assert_eq!(readiness::state_label(verdict.state), "Incomplete");
    assert_eq!(
        readiness::reason(&content_of(&editor, vcx), &verdict).as_deref(),
        Some("No performance sequence yet")
    );
    assert!(present(vcx, "editor-readiness"));
    assert!(
        present(vcx, "sequence-tab-marker"),
        "the Sequence tab is marked"
    );
    let label = vcx.debug_bounds("editor-readiness").expect("label");
    let saved = vcx.debug_bounds("editor-save-state").expect("save state");
    assert!(label.right() <= saved.left(), "{label:?} before {saved:?}");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn readiness_follows_the_content_and_uses_the_planned_copy(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    let content = content_of(&editor, vcx);
    let verdict = editor
        .read_with(vcx, |editor, cx| editor.readiness(cx))
        .expect("verdict");
    assert_eq!(verdict.state, ReadinessState::NeedsReview);
    assert_eq!(
        readiness::reason(&content, &verdict).as_deref(),
        Some("Verse 1, Chorus need pagination review")
    );
    assert!(present(vcx, "editor-readiness"));
    assert!(!present(vcx, "sequence-tab-marker"));
    let none = BTreeSet::new();
    let mut one = content.clone();
    one.sections[1].pagination = PaginationState::Approved;
    assert_eq!(
        readiness::reason(&one, &rate(&one, &none)).as_deref(),
        Some("Verse 1 needs pagination review")
    );
    let mut empty = one.clone();
    empty.sections[0].lyrics.clear();
    let verdict = rate(&empty, &none);
    assert_eq!(verdict.state, ReadinessState::Incomplete);
    assert_eq!(
        readiness::reason(&empty, &verdict).as_deref(),
        Some("Verse 1 has no lyrics and is sung 2 times")
    );
    empty.sequence.pop();
    assert_eq!(
        readiness::reason(&empty, &rate(&empty, &none)).as_deref(),
        Some("Verse 1 has no lyrics and is sung once")
    );
    let mut ready = content.clone();
    for section in &mut ready.sections {
        section.pagination = PaginationState::Approved;
    }
    let verdict = rate(&ready, &none);
    assert_eq!(readiness::state_label(verdict.state), "Ready");
    assert_eq!(readiness::reason(&ready, &verdict), None);
    let blank = ArrangementLyrics {
        sections: Vec::new(),
        sequence: Vec::new(),
    };
    let verdict = rate(&blank, &none);
    assert_eq!(readiness::state_label(verdict.state), "No sections yet");
    assert_eq!(readiness::reason(&blank, &verdict), None);
}

#[gpui::test]
fn a_compose_failure_makes_the_section_unrenderable(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    let content = content_of(&editor, vcx);
    let verse = &content.sections[0];
    let source = slides(&verse.lyrics).remove(0);
    let key = (verse.label.clone(), slide_body(&verse.lyrics, &source));
    editor.update(vcx, |editor, cx| {
        editor.fail_frame(key, &content);
        cx.notify();
    });
    let verdict = editor
        .read_with(vcx, |editor, cx| editor.readiness(cx))
        .expect("verdict");
    assert_eq!(verdict.state, ReadinessState::Incomplete);
    assert_eq!(
        readiness::reason(&content, &verdict).as_deref(),
        Some("Verse 1 can\u{2019}t be shown on the projector; Chorus needs pagination review")
    );
}

#[gpui::test]
fn the_selection_follows_undo_and_expanded_occurrences_are_forgotten(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-wp5-selection");
    let editor = open_editor(&shell, id, vcx);
    let first = OccurrenceId::from_entity(entity(81));
    let second = OccurrenceId::from_entity(entity(82));
    let third = OccurrenceId::from_entity(entity(83));
    editor.update(vcx, |editor, _| {
        assert!(editor.sequence_selection.is_none());
        editor.sequence_selection = Some(SequenceSelection::occurrence(second));
        editor.expanded.insert(second);
        editor.expanded.insert(first);
    });
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    let removed = session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| {
                let _ = content.remove_occurrence(second);
            },
            cx,
        )
    });
    assert!(removed);
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    editor.update(vcx, |editor, cx| editor.undo(cx));
    wait(vcx, |vcx| sequence_of(&editor, vcx).len() == 3);
    assert_eq!(
        editor.read_with(vcx, |editor, _| editor.sequence_selection),
        Some(SequenceSelection::occurrence(second))
    );
    editor.update(vcx, |editor, cx| editor.redo(cx));
    wait(vcx, |vcx| sequence_of(&editor, vcx).len() == 2);
    assert_eq!(
        editor.read_with(vcx, |editor, _| editor.sequence_selection),
        Some(SequenceSelection::occurrence(third))
    );
    editor.read_with(vcx, |editor, _| {
        assert!(editor.expanded.contains(&first));
        assert!(!editor.expanded.contains(&second));
    });
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    finish(vcx, &slot, &dir);
}
