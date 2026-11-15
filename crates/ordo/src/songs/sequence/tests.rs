//! The Sequence's registry rows, editor state and header readiness.

use std::collections::BTreeSet;

use domain::{
    identity::OccurrenceId,
    pagination::{slide_body, slides},
    revisions::EditPoint,
    song_library::{ArrangementLyrics, PaginationState, ReadinessState, readiness as rate},
};
use domain::{
    identity::SectionId,
    revisions::RevisionEntity,
    song_library::{SectionDraft, flatten},
};
use gpui::{Entity, KeyBindingContextPredicate, KeyContext, TestAppContext, VisualTestContext};

use super::{SequenceSelection, available, occurrence_name, occurrences, readiness};
use crate::{
    commands::registry::{self, CommandId, Group, Scope},
    edit_session::SaveState,
    songs::{
        choirs::tests::{click, present, wait},
        lyrics::{
            ArrangementEditor,
            tests::{
                entity, fail_the_save, in_memory, open_editor, parish_editor, parish_full,
                region_now, save_state, sections_of, type_into_area,
            },
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
    assert!(present(vcx, "choice-marker"), "the Sequence tab is marked");
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
    assert!(!present(vcx, "choice-marker"));
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
        editor.fail_frame(key.clone(), &content);
        cx.notify();
    });
    assert!(
        editor.read_with(vcx, |editor, _| editor.is_failed(&key)),
        "a failure is remembered apart from the cache"
    );
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

/// The section names of the Sequence, in sung order.
fn names_of(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(vcx, |editor, cx| {
        let content = editor.content(cx).expect("content");
        content
            .sequence
            .iter()
            .map(|occurrence| occurrence_name(&content, occurrence.section))
            .collect()
    })
}

fn selection_of(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Option<SequenceSelection> {
    editor.read_with(vcx, |editor, _| editor.sequence_selection)
}

/// The Sequence as the store holds it.
fn stored_sequence(
    store: &Entity<crate::songs::LibraryStore>,
    id: domain::identity::ArrangementId,
    vcx: &mut VisualTestContext,
) -> Vec<OccurrenceId> {
    store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("stored")
            .lyrics()
            .sequence
            .iter()
            .map(|occurrence| occurrence.id)
            .collect()
    })
}

fn saved(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) {
    wait(vcx, |vcx| save_state(editor, vcx) == Some(SaveState::Saved));
}

fn occurrence(n: u8) -> OccurrenceId {
    OccurrenceId::from_entity(entity(n))
}

fn toasts_of(shell: &Entity<crate::shell::ShellView>, vcx: &mut VisualTestContext) -> Vec<String> {
    crate::songs::lyrics::tests::toasts(shell, vcx)
}

#[gpui::test]
fn ac1_sequence_shows_available_sections_occurrences_and_inspector_panes(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    click(vcx, "tab-1");
    for selector in [
        "sequence-available",
        "sequence-available-0",
        "sequence-available-1",
        "sequence-add-0",
        "sequence-add-1",
        "sequence-pane",
        "arrangement-sequence-0",
        "arrangement-sequence-1",
        "arrangement-sequence-2",
        "arrangement-inspector",
        "sequence-inspector-empty",
    ] {
        assert!(present(vcx, selector), "{selector}");
    }
    assert!(!present(vcx, "arrangement-sections"));
    assert!(!present(vcx, "sequence-empty"));
    // The words of the panes.
    let content = editor.read_with(vcx, |editor, cx| editor.content(cx).expect("content"));
    assert_eq!(
        occurrences::summary(&content),
        "3 occurrences \u{b7} 3 slides in sung order"
    );
    let (verse, chorus) = (content.sections[0].clone(), content.sections[1].clone());
    assert_eq!(
        available::meta(&content, &verse.lyrics, verse.id),
        "1 slide \u{b7} used 2\u{d7}"
    );
    assert_eq!(
        available::meta(&content, &chorus.lyrics, chorus.id),
        "1 slide \u{b7} used 1\u{d7}"
    );
    assert_eq!(names_of(&editor, vcx), ["Verse 1", "Chorus", "Verse 1"]);
}

#[gpui::test]
fn ac1_selecting_an_available_section_changes_nothing(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    click(vcx, "tab-1");
    let before = content_of(&editor, vcx);
    click(vcx, "sequence-available-1");
    click(vcx, "sequence-available-0");
    assert_eq!(content_of(&editor, vcx), before, "no edit");
    assert_eq!(selection_of(&editor, vcx), None, "no occurrence selected");
    assert!(toasts_of(&shell, vcx).is_empty());
}

#[gpui::test]
fn ac2_add_inserts_after_the_selection_and_selects_it(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-add-after");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    click(vcx, "sequence-name-0");
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(81)))
    );
    click(vcx, "sequence-add-1");
    settle(vcx);
    assert_eq!(
        names_of(&editor, vcx),
        ["Verse 1", "Chorus", "Chorus", "Verse 1"]
    );
    let ids = sequence_of(&editor, vcx);
    assert_eq!(ids[0], occurrence(81));
    assert_eq!([ids[2], ids[3]], [occurrence(82), occurrence(83)]);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(ids[1])),
        "the new occurrence is selected on its first slide"
    );
    assert_eq!(
        toasts_of(&shell, vcx),
        ["Added Chorus at 02 in the Sequence."]
    );
    saved(&editor, vcx);
    assert_eq!(stored_sequence(&store, id, vcx), ids);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac2_add_appends_when_nothing_is_selected(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-add-append");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    assert_eq!(selection_of(&editor, vcx), None);
    click(vcx, "sequence-add-0");
    settle(vcx);
    assert_eq!(
        names_of(&editor, vcx),
        ["Verse 1", "Chorus", "Verse 1", "Verse 1"]
    );
    let ids = sequence_of(&editor, vcx);
    assert_eq!(&ids[..3], [occurrence(81), occurrence(82), occurrence(83)]);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(ids[3]))
    );
    assert_eq!(
        toasts_of(&shell, vcx),
        ["Added Verse 1 at 04 in the Sequence."]
    );
    saved(&editor, vcx);
    assert_eq!(stored_sequence(&store, id, vcx), ids);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_empty_sequence_focuses_add_first_occurrence_and_survives_reload(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-empty");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    let cleared = session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| content.sequence.clear(),
            cx,
        )
    });
    assert!(cleared);
    settle(vcx);
    saved(&editor, vcx);
    click(vcx, "tab-1");
    settle(vcx);
    for selector in [
        "sequence-empty",
        "sequence-add-first",
        "button-Add first occurrence \u{b7} Verse 1",
    ] {
        assert!(present(vcx, selector), "{selector}");
    }
    let focus = editor.read_with(vcx, |editor, _| editor.sequence.add_first.clone());
    assert!(
        vcx.update(|window, _| focus.is_focused(window)),
        "the action has focus"
    );
    assert_eq!(
        occurrences::summary(&content_of(&editor, vcx)),
        "Incomplete"
    );
    // Enter adds the first occurrence, selects and focuses it.
    let keystroke = gpui::Keystroke::parse("enter").expect("key");
    vcx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    vcx.simulate_event(gpui::KeyUpEvent { keystroke });
    settle(vcx);
    assert_eq!(names_of(&editor, vcx), ["Verse 1"]);
    saved(&editor, vcx);
    let added = stored_sequence(&store, id, vcx);
    assert_eq!(added.len(), 1);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(added[0]))
    );
    // Undo empties it again; the empty Sequence is saved as it is and shows
    // the same state after a reload.
    vcx.simulate_keystrokes("ctrl-z");
    wait(vcx, |vcx| {
        settle(vcx);
        sequence_of(&editor, vcx).is_empty()
    });
    saved(&editor, vcx);
    assert!(stored_sequence(&store, id, vcx).is_empty());
    click(vcx, "button-Back to Songs");
    settle(vcx);
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    settle(vcx);
    assert!(present(vcx, "sequence-empty"));
    let focus = editor.read_with(vcx, |editor, _| editor.sequence.add_first.clone());
    assert!(
        vcx.update(|window, _| focus.is_focused(window)),
        "focused again after reload"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_unused_sections_show_no_warning(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-unused");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    // The Chorus is no longer sung: the Sequence keeps Verse 1 only.
    session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| content.sequence.retain(|o| o.id != occurrence(82)),
            cx,
        )
    });
    settle(vcx);
    click(vcx, "tab-1");
    settle(vcx);
    let content = content_of(&editor, vcx);
    assert_eq!(content.occurrences_of(content.sections[1].id), 0);
    assert_eq!(
        available::meta(
            &content,
            &content.sections[1].lyrics,
            content.sections[1].id
        ),
        "1 slide \u{b7} unused"
    );
    let verdict = editor
        .read_with(vcx, |editor, cx| editor.readiness(cx))
        .expect("readiness");
    assert_eq!(
        verdict.state,
        ReadinessState::Ready,
        "an unused section raises nothing"
    );
    assert!(verdict.findings.is_empty());
    assert_eq!(readiness::state_label(verdict.state), "Ready");
    assert!(present(vcx, "editor-readiness"));
    assert!(!present(vcx, "choice-marker"), "no tab marker");
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn archived_arrangement_offers_no_sequence_actions(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, true);
    click(vcx, "tab-1");
    assert!(present(vcx, "sequence-available-0"));
    assert!(present(vcx, "arrangement-sequence-0"));
    for selector in [
        "sequence-add-0",
        "sequence-add-1",
        "sequence-add-first",
        "sequence-row-buttons",
        "sequence-grip-0",
    ] {
        assert!(!present(vcx, selector), "{selector}");
    }
    click(vcx, "sequence-name-0");
    assert!(
        selection_of(&editor, vcx).is_some(),
        "an archived row can still be selected"
    );
    assert!(!present(vcx, "sequence-row-buttons"), "no row actions");
    assert!(!present(vcx, "button-Repeat"));
    let before = content_of(&editor, vcx);
    // The keys do nothing either.
    focus_row(&editor, occurrence(23), vcx);
    vcx.simulate_keystrokes("alt-down");
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(content_of(&editor, vcx), before);
}

/// Gives one occurrence row keyboard focus.
fn focus_row(editor: &Entity<ArrangementEditor>, id: OccurrenceId, vcx: &mut VisualTestContext) {
    let handle = editor
        .read_with(vcx, |editor, _| editor.sequence.rows.get(&id).cloned())
        .expect("the row has a focus handle");
    vcx.update(|window, cx| window.focus(&handle, cx));
    settle(vcx);
}

/// Which stop has focus: an occurrence row, or one of its slide tiles.
fn focused_stop(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Option<(OccurrenceId, Option<usize>)> {
    let (rows, kids): (Vec<_>, Vec<_>) = editor.read_with(vcx, |editor, _| {
        (
            editor
                .sequence
                .rows
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
            editor
                .sequence
                .kids
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
        )
    });
    vcx.update(|window, _| {
        rows.iter()
            .find(|(_, handle)| handle.is_focused(window))
            .map(|(id, _)| (*id, None))
            .or_else(|| {
                kids.iter()
                    .find(|(_, handle)| handle.is_focused(window))
                    .map(|((id, slide), _)| (*id, Some(*slide)))
            })
    })
}

#[gpui::test]
fn ac3_repeat_inserts_the_same_section_after_the_selection(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-repeat");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    click(vcx, "sequence-name-0");
    click(vcx, "button-Repeat");
    settle(vcx);
    assert_eq!(
        names_of(&editor, vcx),
        ["Verse 1", "Verse 1", "Chorus", "Verse 1"]
    );
    let ids = sequence_of(&editor, vcx);
    assert_eq!(ids[0], occurrence(81));
    assert_eq!([ids[2], ids[3]], [occurrence(82), occurrence(83)]);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(ids[1]))
    );
    assert_eq!(
        toasts_of(&shell, vcx),
        ["Added Verse 1 at 02 in the Sequence."]
    );
    saved(&editor, vcx);
    assert_eq!(stored_sequence(&store, id, vcx), ids);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac3_both_occurrences_show_the_edited_lyrics(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-edited");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    let section = content_of(&editor, vcx).sections[0].id;
    session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| {
                if let Some(draft) = content.sections.iter_mut().find(|s| s.id == section) {
                    draft.lyrics = "Edited line".to_owned();
                }
            },
            cx,
        )
    });
    settle(vcx);
    click(vcx, "tab-1");
    click(vcx, "button-Expand all");
    let key = (Some("Verse 1".to_owned()), "Edited line".to_owned());
    wait(vcx, |vcx| {
        settle(vcx);
        editor.update(vcx, |editor, _| editor.has_frame(&key))
    });
    // The first and the third occurrence are both Verse 1: both show it.
    assert!(present(vcx, "sequence-child-0-0"));
    assert!(present(vcx, "sequence-child-2-0"));
    assert_eq!(names_of(&editor, vcx), ["Verse 1", "Chorus", "Verse 1"]);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac4_alt_down_moves_the_focused_occurrence_with_its_slides(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-move");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    focus_row(&editor, occurrence(81), vcx);
    vcx.simulate_keystrokes("alt-down");
    settle(vcx);
    let moved = [occurrence(82), occurrence(81), occurrence(83)];
    assert_eq!(sequence_of(&editor, vcx), moved);
    assert_eq!(names_of(&editor, vcx), ["Chorus", "Verse 1", "Verse 1"]);
    assert_eq!(
        focused_stop(&editor, vcx),
        Some((occurrence(81), None)),
        "focus stays on the row"
    );
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(81)))
    );
    assert_eq!(toasts_of(&shell, vcx), ["Moved Verse 1 to 02."]);
    saved(&editor, vcx);
    assert_eq!(stored_sequence(&store, id, vcx), moved);
    // Past the edge: nothing is written and nothing is said.
    focus_row(&editor, occurrence(83), vcx);
    vcx.simulate_keystrokes("alt-down");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), moved);
    assert_eq!(toasts_of(&shell, vcx), ["Moved Verse 1 to 02."]);
    // Alt+Up undoes the move by hand.
    focus_row(&editor, occurrence(81), vcx);
    vcx.simulate_keystrokes("alt-up");
    settle(vcx);
    assert_eq!(
        sequence_of(&editor, vcx),
        [occurrence(81), occurrence(82), occurrence(83)]
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_delete_removes_the_occurrence_keeps_the_section_and_selects_next_then_previous(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-delete");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    focus_row(&editor, occurrence(82), vcx);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [occurrence(81), occurrence(83)]);
    assert_eq!(sections_of(&editor, vcx).len(), 2, "the section stays");
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(83)))
    );
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(83), None)));
    assert_eq!(
        toasts_of(&shell, vcx),
        ["Removed 02 Chorus from the Sequence. The section stays in Lyrics."]
    );
    // The last one has no next: the previous is selected.
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [occurrence(81)]);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(81)))
    );
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(81), None)));
    // The only one: Add first occurrence takes focus.
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert!(sequence_of(&editor, vcx).is_empty());
    assert_eq!(selection_of(&editor, vcx), None);
    let focus = editor.read_with(vcx, |editor, _| editor.sequence.add_first.clone());
    assert!(vcx.update(|window, _| focus.is_focused(window)));
    assert_eq!(sections_of(&editor, vcx).len(), 2);
    saved(&editor, vcx);
    assert!(stored_sequence(&store, id, vcx).is_empty());
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_ctrl_z_restores_the_removed_occurrence_and_selects_it(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-delete-undo");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    focus_row(&editor, occurrence(82), vcx);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    saved(&editor, vcx);
    vcx.simulate_keystrokes("ctrl-z");
    let all = [occurrence(81), occurrence(82), occurrence(83)];
    wait(vcx, |vcx| {
        settle(vcx);
        sequence_of(&editor, vcx) == all
    });
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(82)))
    );
    assert_eq!(
        focused_stop(&editor, vcx),
        Some((occurrence(82), None)),
        "focus returns with the restored selection"
    );
    saved(&editor, vcx);
    assert_eq!(stored_sequence(&store, id, vcx), all);
    // Delete now acts on the restored, selected and focused row.
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [occurrence(81), occurrence(83)]);
    finish(vcx, &slot, &dir);
}

/// Gives Verse 1 two slides so an occurrence has something to expand.
fn two_slide_verse(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) {
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    let section = content_of(editor, vcx).sections[0].id;
    session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| {
                if let Some(draft) = content.sections.iter_mut().find(|s| s.id == section) {
                    draft.lyrics = "Line one\n\nLine two".to_owned();
                }
            },
            cx,
        )
    });
    settle(vcx);
}

#[gpui::test]
fn ac8_expand_all_and_collapse_all_change_disclosure_only(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    click(vcx, "tab-1");
    click(vcx, "sequence-name-1");
    let (content, selection) = (content_of(&editor, vcx), selection_of(&editor, vcx));
    assert!(present(vcx, "button-Expand all"));
    assert!(!present(vcx, "sequence-children-0"));
    click(vcx, "button-Expand all");
    for selector in [
        "sequence-children-0",
        "sequence-children-1",
        "sequence-children-2",
    ] {
        assert!(present(vcx, selector), "{selector}");
    }
    assert!(present(vcx, "button-Collapse all"));
    assert_eq!(content_of(&editor, vcx), content, "content untouched");
    assert_eq!(selection_of(&editor, vcx), selection, "selection untouched");
    click(vcx, "button-Collapse all");
    for selector in [
        "sequence-children-0",
        "sequence-children-1",
        "sequence-children-2",
    ] {
        assert!(!present(vcx, selector), "{selector}");
    }
    assert!(present(vcx, "button-Expand all"));
    assert_eq!(content_of(&editor, vcx), content);
    assert_eq!(selection_of(&editor, vcx), selection);
}

#[gpui::test]
fn ac8_inspector_keeps_the_selected_slide_when_its_occurrence_collapses(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-inspector");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    two_slide_verse(&editor, vcx);
    click(vcx, "tab-1");
    assert!(
        present(vcx, "sequence-inspector-empty"),
        "nothing selected yet"
    );
    click(vcx, "sequence-expander-0");
    click(vcx, "sequence-child-0-1");
    let chosen = SequenceSelection {
        occurrence: occurrence(81),
        slide: 1,
    };
    assert_eq!(selection_of(&editor, vcx), Some(chosen));
    assert!(present(vcx, "inspector-slide"));
    assert!(present(vcx, "inspector-preview"));
    click(vcx, "sequence-expander-0");
    assert!(!present(vcx, "sequence-children-0"), "collapsed");
    assert_eq!(
        selection_of(&editor, vcx),
        Some(chosen),
        "the slide stays selected"
    );
    assert!(
        present(vcx, "inspector-slide"),
        "the Inspector still shows it"
    );
    assert!(present(vcx, "inspector-preview"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_arrow_keys_walk_the_flattened_order(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-walk");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    two_slide_verse(&editor, vcx);
    click(vcx, "tab-1");
    focus_row(&editor, occurrence(81), vcx);
    let step = |key: &str, vcx: &mut VisualTestContext| {
        vcx.simulate_keystrokes(key);
        settle(vcx);
    };
    // Right expands the focused row, and focus stays on it.
    step("right", vcx);
    assert!(present(vcx, "sequence-children-0"));
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(81), None)));
    // Right again steps into the first slide.
    step("right", vcx);
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(81), Some(0))));
    for (key, stop, slide) in [
        ("down", 81, Some(1)),
        ("down", 82, None),
        ("up", 81, Some(1)),
        ("up", 81, Some(0)),
        ("up", 81, None),
    ] {
        step(key, vcx);
        assert_eq!(
            focused_stop(&editor, vcx),
            Some((occurrence(stop), slide)),
            "{key}"
        );
        assert_eq!(
            selection_of(&editor, vcx),
            Some(SequenceSelection {
                occurrence: occurrence(stop),
                slide: slide.unwrap_or(0)
            }),
            "selection follows focus after {key}"
        );
    }
    // Left collapses the row; from a slide it goes to the row first.
    step("left", vcx);
    assert!(!present(vcx, "sequence-children-0"));
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(81), None)));
    // Down from a collapsed row skips its slides.
    step("down", vcx);
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(82), None)));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac4_dropping_a_dragged_occurrence_moves_it_once(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-drag");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    let (first, middle, last) = (occurrence(81), occurrence(82), occurrence(83));
    // The mouse: grab the first row's grip and let go on the lower half of
    // the last row.
    let grip = vcx.debug_bounds("sequence-grip-0").expect("grip").center();
    let target = vcx.debug_bounds("arrangement-sequence-2").expect("row");
    let drop_at = gpui::point(target.center().x, target.bottom() - gpui::px(2.0));
    vcx.simulate_mouse_down(grip, gpui::MouseButton::Left, gpui::Modifiers::default());
    for step in 1..=4 {
        let t = step as f32 / 4.0;
        let at = gpui::point(
            grip.x + (drop_at.x - grip.x) * t,
            grip.y + (drop_at.y - grip.y) * t,
        );
        vcx.simulate_mouse_move(at, gpui::MouseButton::Left, gpui::Modifiers::default());
    }
    vcx.simulate_mouse_up(drop_at, gpui::MouseButton::Left, gpui::Modifiers::default());
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [middle, last, first]);
    assert_eq!(toasts_of(&shell, vcx), ["Moved Verse 1 to 03."]);
    saved(&editor, vcx);
    assert_eq!(stored_sequence(&store, id, vcx), [middle, last, first]);
    // One drop is one edit: a single Undo restores the order.
    vcx.simulate_keystrokes("ctrl-z");
    wait(vcx, |vcx| {
        settle(vcx);
        sequence_of(&editor, vcx) == [first, middle, last]
    });
    // The halves: the upper half of the last row puts the row before it.
    editor.update(vcx, |editor, cx| {
        editor.drop_occurrence(first, last, false, cx)
    });
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [middle, first, last]);
    // A drop on itself writes nothing.
    let toasts = toasts_of(&shell, vcx);
    editor.update(vcx, |editor, cx| {
        editor.drop_occurrence(first, first, true, cx)
    });
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [middle, first, last]);
    assert_eq!(toasts_of(&shell, vcx), toasts);
    finish(vcx, &slot, &dir);
}

/// How many revisions the store holds for the Arrangement.
fn revision_count(
    slot: &crate::songs::StorageSlot,
    id: domain::identity::ArrangementId,
    vcx: &mut VisualTestContext,
) -> usize {
    let future = slot
        .with(|storage| storage.revisions(RevisionEntity::Arrangement(id)))
        .expect("storage is open");
    vcx.foreground_executor()
        .block_test(future)
        .expect("revisions")
        .len()
}

#[gpui::test]
fn expand_all_on_a_long_sequence_composes_every_slide_and_settles(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    // Forty distinct one-slide sections, each sung once: more slides than
    // the old 32-frame cache could hold.
    let keys: Vec<_> = (0..40)
        .map(|n| (Some(format!("Part {n}")), format!("Words of part {n}")))
        .collect();
    session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| {
                content.sequence.clear();
                for (label, body) in &keys {
                    let section =
                        SectionId::from_entity(domain::identity::EntityId::from_random_bytes(
                            *uuid::Uuid::new_v4().as_bytes(),
                        ));
                    content.sections.push(SectionDraft::labeled(
                        section,
                        label.as_deref().unwrap_or_default(),
                        body,
                    ));
                    let _ = content.insert_occurrence(super::fresh_occurrence(), section, None);
                }
            },
            cx,
        )
    });
    settle(vcx);
    click(vcx, "tab-1");
    click(vcx, "button-Expand all");
    wait(vcx, |vcx| {
        settle(vcx);
        editor.update(vcx, |editor, _| {
            keys.iter().all(|key| editor.has_frame(key)) && !editor.composing_or_failed().0
        })
    });
    // Nothing is asked for again: the cache holds all forty and stays quiet.
    for _ in 0..5 {
        settle(vcx);
    }
    editor.update(vcx, |editor, _| {
        assert_eq!(
            editor.composing_or_failed(),
            (false, false),
            "quiet, nothing failed"
        );
        assert!(keys.iter().all(|key| editor.has_frame(key)));
    });
}

#[gpui::test]
fn focus_on_an_unselected_row_selects_it_and_delete_acts_on_the_focused_row(
    cx: &mut TestAppContext,
) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-focus-select");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    assert_eq!(selection_of(&editor, vcx), None);
    // Tab or F6 lands focus without a click: the selection follows it.
    focus_row(&editor, occurrence(82), vcx);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(82)))
    );
    focus_row(&editor, occurrence(83), vcx);
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(occurrence(83)))
    );
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), [occurrence(81), occurrence(82)]);
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn repeat_by_keyboard_keeps_focus_on_the_new_row(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-repeat-focus");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    focus_row(&editor, occurrence(81), vcx);
    let repeat = vcx.debug_bounds("sequence-row-buttons").is_some();
    assert!(repeat, "the focused row shows its actions");
    // Tab walks from the row to its first action, Repeat; Enter presses it.
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    assert_eq!(focused_stop(&editor, vcx), None, "focus is on the action");
    let keystroke = gpui::Keystroke::parse("enter").expect("key");
    vcx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    vcx.simulate_event(gpui::KeyUpEvent { keystroke });
    settle(vcx);
    let ids = sequence_of(&editor, vcx);
    assert_eq!(ids.len(), 4);
    assert_eq!(names_of(&editor, vcx)[..2], ["Verse 1", "Verse 1"]);
    assert_eq!(focused_stop(&editor, vcx), Some((ids[1], None)));
    assert_eq!(
        selection_of(&editor, vcx),
        Some(SequenceSelection::occurrence(ids[1]))
    );
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn collapsing_with_a_focused_tile_focuses_its_row(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-collapse-focus");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    two_slide_verse(&editor, vcx);
    click(vcx, "tab-1");
    editor.update(vcx, |editor, cx| editor.toggle_expanded(occurrence(81), cx));
    settle(vcx);
    let tile = editor
        .read_with(vcx, |editor, _| {
            editor.sequence.kids.get(&(occurrence(81), 1)).cloned()
        })
        .expect("tile handle");
    vcx.update(|window, cx| window.focus(&tile, cx));
    settle(vcx);
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(81), Some(1))));
    // The keyboard collapses without moving focus first.
    editor.update(vcx, |editor, cx| editor.toggle_expanded(occurrence(81), cx));
    settle(vcx);
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(81), None)));
    // The same for Collapse all.
    editor.update(vcx, |editor, cx| editor.toggle_all_expanded(cx));
    settle(vcx);
    let tile = editor
        .read_with(vcx, |editor, _| {
            editor.sequence.kids.get(&(occurrence(82), 0)).cloned()
        })
        .expect("tile handle");
    vcx.update(|window, cx| window.focus(&tile, cx));
    settle(vcx);
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(82), Some(0))));
    editor.update(vcx, |editor, cx| editor.toggle_all_expanded(cx));
    settle(vcx);
    assert_eq!(focused_stop(&editor, vcx), Some((occurrence(82), None)));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn after_a_failed_save_every_sequence_edit_says_retry_and_changes_nothing(cx: &mut TestAppContext) {
    let retry = "Couldn't save. Retry to keep editing.";
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    settle(vcx);
    fail_the_save(&editor, vcx);
    click(vcx, "tab-1");
    let before = sequence_of(&editor, vcx);
    let refused = |vcx: &mut VisualTestContext| {
        assert_eq!(sequence_of(&editor, vcx), before, "nothing changed");
        let toasts = toasts_of(&shell, vcx);
        assert!(
            !toasts.is_empty() && toasts.iter().all(|toast| toast == retry),
            "{toasts:?}"
        );
    };
    click(vcx, "sequence-add-1");
    refused(vcx);
    focus_row(&editor, occurrence(23), vcx);
    vcx.simulate_keystrokes("alt-down");
    settle(vcx);
    refused(vcx);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    refused(vcx);
    editor.update(vcx, |editor, cx| {
        editor.drop_occurrence(occurrence(23), occurrence(25), true, cx)
    });
    settle(vcx);
    refused(vcx);
    click(vcx, "button-Repeat");
    refused(vcx);
}

#[gpui::test]
fn alt_down_stays_with_the_focused_list_on_both_tabs(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-both-tabs");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    // Lyrics tab, Sections list: the sections move, the Sequence does not.
    let sequence = sequence_of(&editor, vcx);
    click(vcx, "arrangement-section-0");
    vcx.simulate_keystrokes("alt-down");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Chorus", "Verse 1"]);
    assert_eq!(sequence_of(&editor, vcx), sequence);
    saved(&editor, vcx);
    // Sequence tab, a row: the Sequence moves, the sections do not.
    click(vcx, "tab-1");
    focus_row(&editor, occurrence(81), vcx);
    vcx.simulate_keystrokes("alt-down");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Chorus", "Verse 1"]);
    assert_eq!(
        sequence_of(&editor, vcx),
        [occurrence(82), occurrence(81), occurrence(83)]
    );
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn delete_over_a_dialog_a_tile_or_a_row_action_leaves_the_sequence_alone(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-delete-guards");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    two_slide_verse(&editor, vcx);
    click(vcx, "tab-1");
    let all = sequence_of(&editor, vcx);
    // A slide tile: Delete is not an occurrence removal.
    editor.update(vcx, |editor, cx| editor.toggle_expanded(occurrence(81), cx));
    settle(vcx);
    let tile = editor
        .read_with(vcx, |editor, _| {
            editor.sequence.kids.get(&(occurrence(81), 0)).cloned()
        })
        .expect("tile handle");
    vcx.update(|window, cx| window.focus(&tile, cx));
    settle(vcx);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), all, "tile");
    // The row's own Repeat button shares the key context.
    focus_row(&editor, occurrence(82), vcx);
    vcx.simulate_keystrokes("tab");
    settle(vcx);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), all, "row action");
    // A dialog opened from the editor header.
    focus_row(&editor, occurrence(82), vcx);
    click(vcx, "button-Arrangement details");
    settle(vcx);
    vcx.simulate_keystrokes("delete");
    settle(vcx);
    assert_eq!(sequence_of(&editor, vcx), all, "dialog");
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn alt_down_moves_an_occurrence_with_all_its_slides(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-move-slides");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    type_into_area(&editor, "Line one\n\nLine two", vcx);
    crate::songs::lyrics::tests::saved(&editor, vcx);
    click(vcx, "tab-1");
    let order = |vcx: &mut VisualTestContext| -> Vec<(OccurrenceId, usize)> {
        flatten(&content_of(&editor, vcx))
            .iter()
            .map(|slide| (slide.occurrence, slide.slide))
            .collect()
    };
    assert_eq!(
        order(vcx),
        [
            (occurrence(81), 0),
            (occurrence(81), 1),
            (occurrence(82), 0),
            (occurrence(83), 0),
            (occurrence(83), 1),
        ]
    );
    focus_row(&editor, occurrence(81), vcx);
    vcx.simulate_keystrokes("alt-down");
    settle(vcx);
    assert_eq!(
        order(vcx),
        [
            (occurrence(82), 0),
            (occurrence(81), 0),
            (occurrence(81), 1),
            (occurrence(83), 0),
            (occurrence(83), 1),
        ],
        "both slides travel with their occurrence"
    );
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn an_edit_in_lyrics_shows_one_shared_frame_in_both_occurrences(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-shared-frame");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    type_into_area(&editor, "Typed line", vcx);
    crate::songs::lyrics::tests::saved(&editor, vcx);
    click(vcx, "tab-1");
    click(vcx, "button-Expand all");
    let key = (Some("Verse 1".to_owned()), "Typed line".to_owned());
    wait(vcx, |vcx| {
        settle(vcx);
        editor.update(vcx, |editor, _| editor.has_frame(&key))
    });
    let content = content_of(&editor, vcx);
    let keys: Vec<_> = [occurrence(81), occurrence(83)]
        .iter()
        .map(|id| {
            let at = content.occurrence_index(*id).expect("occurrence");
            let section = content
                .sections
                .iter()
                .find(|section| section.id == content.sequence[at].section)
                .expect("section");
            super::slide_keys(section).remove(0)
        })
        .collect();
    assert_eq!(keys[0], keys[1], "both occurrences use the one key");
    assert_eq!(keys[0], key);
    assert!(present(vcx, "sequence-child-0-0"));
    assert!(present(vcx, "sequence-child-2-0"));
    let frame = editor
        .update(vcx, |editor, _| editor.frame_of(&key))
        .expect("composed");
    let again = editor
        .update(vcx, |editor, _| editor.frame_of(&key))
        .expect("composed");
    assert!(
        std::sync::Arc::ptr_eq(&frame, &again),
        "one frame is shared"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_drop_that_leaves_the_order_alone_writes_nothing(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-drop-noop");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "tab-1");
    saved(&editor, vcx);
    let (first, second) = (occurrence(81), occurrence(82));
    let (revisions, toasts, order) = (
        revision_count(&slot, id, vcx),
        toasts_of(&shell, vcx),
        sequence_of(&editor, vcx),
    );
    // The upper half of the next row is where the row already is.
    editor.update(vcx, |editor, cx| {
        editor.drop_occurrence(first, second, false, cx)
    });
    settle(vcx);
    saved(&editor, vcx);
    assert_eq!(sequence_of(&editor, vcx), order);
    assert_eq!(toasts_of(&shell, vcx), toasts, "no toast");
    assert_eq!(revision_count(&slot, id, vcx), revisions, "no new revision");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn f6_on_the_sequence_tab_visits_available_sequence_and_inspector(cx: &mut TestAppContext) {
    let (shell, _editor, _id, vcx) = in_memory(cx, false);
    click(vcx, "tab-1");
    let mut visited = Vec::new();
    for _ in 0..4 {
        vcx.simulate_keystrokes("f6");
        settle(vcx);
        visited.push(region_now(&shell, vcx));
    }
    let order = [
        "Workspace actions",
        "Primary list",
        "Main editor",
        "Inspector",
    ];
    let start = order
        .iter()
        .position(|name| *name == visited[0])
        .expect("a known region");
    let expected: Vec<_> = (0..4).map(|step| order[(start + step) % 4]).collect();
    assert_eq!(visited, expected);
    assert!(
        present(vcx, "sequence-available"),
        "Primary list is Available"
    );
    assert!(present(vcx, "sequence-pane"), "Main editor is the Sequence");
    assert!(present(vcx, "arrangement-inspector"));
}
