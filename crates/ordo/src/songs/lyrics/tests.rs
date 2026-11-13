//! The Arrangement editor frame through the shell that hosts it: task tabs,
//! save state, the navigation guard and the Editing keys.

use domain::{
    identity::{
        ArrangementId, EntityId, HistorySessionId, OccurrenceId, SectionId, SongId, Timestamp,
    },
    revisions::EditPoint,
    song_library::{ArrangementDraft, OccurrenceDraft, SectionDraft, SongDraft, SongLibrary},
};
use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use super::{ArrangementEditor, TaskTab};
use crate::{
    commands::registry::{self, CommandId, Group, Scope},
    edit_session::SaveState,
    shell::ShellView,
    songs::{
        LibraryStore,
        arrangement_details::tests::write,
        choirs::tests::{click as click_now, open_store, present, wait},
        new_song::tests::{click, finish, parish, settle},
    },
};

const SAVE_DIALOG: &str = "Couldn't save.";

fn entity(n: u8) -> EntityId {
    EntityId::from_random_bytes([n; 16])
}

fn at() -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000)
}

/// An in-memory library with one Song "Ave Maria" and one Arrangement of two
/// sections and a three-place Sequence. Its store has no storage, so every
/// commit fails: the way to test the failed-flush paths.
fn library(archived: bool) -> (SongLibrary, ArrangementId) {
    build(archived, false)
}

fn build(archived: bool, empty: bool) -> (SongLibrary, ArrangementId) {
    let mut library = SongLibrary::new();
    let song = SongId::from_entity(entity(10));
    library
        .create_song(
            song,
            at(),
            SongDraft {
                title: "Ave Maria".to_owned(),
                ..SongDraft::default()
            },
        )
        .expect("song");
    let id = ArrangementId::from_entity(entity(20));
    let verse = SectionId::from_entity(entity(21));
    let chorus = SectionId::from_entity(entity(22));
    let occurrence =
        |n: u8, section| OccurrenceDraft::new(OccurrenceId::from_entity(entity(n)), section);
    library
        .create_arrangement(
            id,
            song,
            at(),
            ArrangementDraft {
                title: Some("Latin SATB".to_owned()),
                languages: vec!["Latin".to_owned()],
                rendition_attribution: None,
                notes: None,
                occasions: Vec::new(),
                sections: if empty {
                    Vec::new()
                } else {
                    vec![
                        SectionDraft::labeled(verse, "Verse 1", "Ave Maria"),
                        SectionDraft::labeled(chorus, "Chorus", "Refrain line"),
                    ]
                },
                sequence: if empty {
                    Vec::new()
                } else {
                    vec![
                        occurrence(23, verse),
                        occurrence(24, chorus),
                        occurrence(25, verse),
                    ]
                },
            },
            Vec::new(),
        )
        .expect("arrangement");
    if archived {
        library.archive_arrangement(id, at()).expect("archive");
    }
    (library, id)
}

/// The shell over the in-memory library with the editor open.
fn in_memory(
    cx: &mut TestAppContext,
    archived: bool,
) -> (
    Entity<ShellView>,
    Entity<ArrangementEditor>,
    ArrangementId,
    &mut VisualTestContext,
) {
    let (library, id) = library(archived);
    let store = cx.new(|cx| LibraryStore::with_library(library, cx));
    let (shell, vcx) = open_store(cx, store);
    let editor = open_editor(&shell, id, vcx);
    (shell, editor, id, vcx)
}

/// The shell over an in-memory Arrangement that has no sections yet.
fn in_memory_empty(
    cx: &mut TestAppContext,
) -> (
    Entity<ShellView>,
    Entity<ArrangementEditor>,
    &mut VisualTestContext,
) {
    let (library, id) = build(false, true);
    let store = cx.new(|cx| LibraryStore::with_library(library, cx));
    let (shell, vcx) = open_store(cx, store);
    let editor = open_editor(&shell, id, vcx);
    (shell, editor, vcx)
}

fn sections_of(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(vcx, |editor, cx| {
        editor
            .content(cx)
            .expect("content")
            .sections
            .iter()
            .map(|section| section.label.clone().unwrap_or_default())
            .collect()
    })
}

fn open_editor(
    shell: &Entity<ShellView>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> Entity<ArrangementEditor> {
    shell.update(vcx, |shell, cx| shell.open_arrangement(id, cx));
    settle(vcx);
    shell.read_with(vcx, |shell, _| {
        shell
            .arrangement_editor()
            .expect("the editor is open")
            .clone()
    })
}

/// Changes the first section's lyrics through the session, as the lyric
/// editor will.
fn type_lyrics(editor: &Entity<ArrangementEditor>, text: &str, vcx: &mut VisualTestContext) {
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("an editable Arrangement has a session");
    session.update(vcx, |session, cx| {
        session.text_edited(|content| content.sections[0].lyrics = text.to_owned(), cx);
    });
    vcx.run_until_parked();
}

fn save_state(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Option<SaveState> {
    editor.read_with(vcx, |editor, cx| editor.save_state(cx))
}

fn stored_lyrics(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> String {
    store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("stored")
            .lyrics()
            .sections[0]
            .lyrics
            .clone()
    })
}

fn dialog(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) -> Option<String> {
    shell.read_with(vcx, |shell, cx| {
        shell.overlays().top(cx).map(|title| title.to_string())
    })
}

fn editing(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) -> bool {
    shell.read_with(vcx, |shell, _| shell.editing().is_some())
}

/// Waits until the save dialog is the topmost surface.
fn wait_for_dialog(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) {
    wait(vcx, |vcx| {
        dialog(shell, vcx).as_deref() == Some(SAVE_DIALOG)
    });
    settle(vcx);
}

/// An Arrangement with one section, written through the real store, and the
/// shell over it.
fn parish_editor<'a>(
    cx: &'a mut TestAppContext,
    tag: &str,
) -> (
    std::path::PathBuf,
    crate::songs::StorageSlot,
    Entity<LibraryStore>,
    Entity<ShellView>,
    ArrangementId,
    &'a mut VisualTestContext,
) {
    let (dir, slot, store, seeded) = parish(cx, tag);
    let (shell, vcx) = open_store(cx, store.clone());
    let id = store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangements_for_song(seeded.exact)
            .expect("song")[0]
            .id()
    });
    let mut lyrics = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("stored").lyrics()
    });
    lyrics.sections.push(SectionDraft::labeled(
        SectionId::from_entity(entity(77)),
        "Verse 1",
        "First words",
    ));
    let session = HistorySessionId::from_entity(entity(9));
    write(&store, vcx, |store, cx| {
        store.save_arrangement_lyrics(id, lyrics, session, cx)
    });
    (dir, slot, store, shell, id, vcx)
}

#[gpui::test]
fn ac1_lyrics_and_sequence_are_the_only_task_tabs(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    assert!(present(vcx, "tab-0") && present(vcx, "tab-1"));
    assert!(!present(vcx, "tab-2"), "exactly two tabs");
    assert_eq!(TaskTab::ALL.map(TaskTab::label), ["Lyrics", "Sequence"]);
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Lyrics
    );
    assert!(
        present(vcx, "arrangement-sections"),
        "Lyrics is the default"
    );
    assert!(!present(vcx, "arrangement-sequence"));

    click(vcx, "tab-1");
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Sequence
    );
    for at in 0..3 {
        let selector: &'static str = [
            "arrangement-sequence-0",
            "arrangement-sequence-1",
            "arrangement-sequence-2",
        ][at];
        assert!(present(vcx, selector), "{selector}");
    }
    assert!(present(vcx, "arrangement-sequence-note"));
    assert!(!present(vcx, "arrangement-sections"));
    click(vcx, "tab-0");
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Lyrics
    );
}

#[gpui::test]
fn ac1_metadata_is_only_in_arrangement_details(cx: &mut TestAppContext) {
    let (shell, _editor, _id, vcx) = in_memory(cx, false);
    for wrapper in [
        "ad-languages",
        "ad-choirs",
        "ad-translator",
        "ad-occasions",
        "ad-notes",
    ] {
        assert!(
            !present(vcx, wrapper),
            "{wrapper} is not in the editor frame"
        );
    }
    assert!(!present(vcx, "arrangement-facts"));
    click(vcx, "button-Arrangement details");
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.arrangement_details().is_some())
    });
    settle(vcx);
    assert!(present(vcx, "ad-languages") && present(vcx, "ad-notes"));
}

#[gpui::test]
fn back_flushes_pending_lyrics_then_leaves(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-back-flush");
    let editor = open_editor(&shell, id, vcx);
    type_lyrics(&editor, "Words changed", vcx);
    assert_eq!(save_state(&editor, vcx), Some(SaveState::Saving));
    click(vcx, "button-Back to Songs");
    wait(vcx, |vcx| !editing(&shell, vcx));
    settle(vcx);
    assert_eq!(stored_lyrics(&store, id, vcx), "Words changed");
    assert_eq!(dialog(&shell, vcx), None, "a successful flush asks nothing");
    assert!(shell.read_with(vcx, |shell, _| shell.arrangement_editor().is_none()));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_failed_flush_blocks_back_and_offers_three_choices(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_lyrics(&editor, "Unsaved words", vcx);
    click(vcx, "button-Back to Songs");
    wait_for_dialog(&shell, vcx);
    assert!(editing(&shell, vcx), "Back is blocked");
    for button in [
        "button-Retry save",
        "button-Keep editing",
        "button-Leave without saving",
    ] {
        assert!(present(vcx, button), "{button}");
    }
    assert_eq!(save_state(&editor, vcx), Some(SaveState::CouldntSave));
}

#[gpui::test]
fn keep_editing_closes_the_dialog_and_keeps_the_text(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_lyrics(&editor, "Unsaved words", vcx);
    click(vcx, "button-Back to Songs");
    wait_for_dialog(&shell, vcx);
    click(vcx, "button-Keep editing");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx), None);
    assert!(editing(&shell, vcx));
    let kept = editor.read_with(vcx, |editor, cx| editor.content(cx).expect("content"));
    assert_eq!(kept.sections[0].lyrics, "Unsaved words");
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.save_state(cx)),
        Some(SaveState::CouldntSave)
    );
    assert!(present(vcx, "editor-save-state"));
}

#[gpui::test]
fn retry_save_runs_the_flush_again_and_blocks_while_it_still_fails(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_lyrics(&editor, "Unsaved words", vcx);
    click(vcx, "button-Back to Songs");
    wait_for_dialog(&shell, vcx);
    click(vcx, "button-Retry save");
    settle(vcx);
    wait_for_dialog(&shell, vcx);
    assert!(editing(&shell, vcx), "still blocked");
    assert_eq!(save_state(&editor, vcx), Some(SaveState::CouldntSave));
}

#[gpui::test]
fn leave_without_saving_drops_the_text_and_leaves(cx: &mut TestAppContext) {
    let (shell, editor, id, vcx) = in_memory(cx, false);
    let store = shell.read_with(vcx, |shell, _| shell.arrangement_editor().is_some());
    assert!(store);
    type_lyrics(&editor, "Unsaved words", vcx);
    click(vcx, "button-Back to Songs");
    wait_for_dialog(&shell, vcx);
    click(vcx, "button-Leave without saving");
    wait(vcx, |vcx| !editing(&shell, vcx));
    settle(vcx);
    assert_eq!(dialog(&shell, vcx), None);
    assert!(shell.read_with(vcx, |shell, _| shell.arrangement_editor().is_none()));
    // Nothing was written: the stored content is the original.
    let unchanged = shell.read_with(vcx, |shell, cx| {
        shell
            .library_store()
            .read(cx)
            .library()
            .arrangement(id)
            .expect("stored")
            .lyrics()
            .sections[0]
            .lyrics
            .clone()
    });
    assert_eq!(unchanged, "Ave Maria");
}

#[gpui::test]
fn a_blocked_quit_keeps_the_window_open(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_lyrics(&editor, "Unsaved words", vcx);
    let allowed = shell.update_in(vcx, |shell, window, cx| shell.allow_close(window, cx));
    assert!(!allowed, "the close always asks first");
    vcx.run_until_parked();
    settle(vcx);
    click(vcx, "button-Quit");
    wait_for_dialog(&shell, vcx);
    assert!(!shell.read_with(vcx, |shell, _| shell.quit_confirmed()));
    assert_eq!(vcx.cx.windows().len(), 1, "the window stays open");
    click(vcx, "button-Keep editing");
    settle(vcx);
    assert!(editing(&shell, vcx));
    assert!(!shell.read_with(vcx, |shell, _| shell.quit_confirmed()));
}

#[gpui::test]
fn a_clean_quit_flushes_and_closes_without_a_second_dialog(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-quit-flush");
    let editor = open_editor(&shell, id, vcx);
    type_lyrics(&editor, "Flushed on quit", vcx);
    let allowed = shell.update_in(vcx, |shell, window, cx| shell.allow_close(window, cx));
    assert!(!allowed);
    vcx.run_until_parked();
    settle(vcx);
    click_now(vcx, "button-Quit");
    wait(vcx, |vcx| vcx.cx.windows().is_empty());
    let stored = store.read_with(&vcx.cx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("stored")
            .lyrics()
            .sections[0]
            .lyrics
            .clone()
    });
    assert_eq!(stored, "Flushed on quit");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = &slot;
}

#[gpui::test]
fn opening_arrangement_details_flushes_pending_lyrics_first(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-details-flush");
    let editor = open_editor(&shell, id, vcx);
    type_lyrics(&editor, "Saved before details", vcx);
    click(vcx, "button-Arrangement details");
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.arrangement_details().is_some())
    });
    settle(vcx);
    assert_eq!(stored_lyrics(&store, id, vcx), "Saved before details");
    assert_eq!(save_state(&editor, vcx), Some(SaveState::Saved));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_failed_flush_keeps_arrangement_details_closed_until_the_operator_decides(
    cx: &mut TestAppContext,
) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_lyrics(&editor, "Unsaved words", vcx);
    click(vcx, "button-Arrangement details");
    wait_for_dialog(&shell, vcx);
    assert!(shell.read_with(vcx, |shell, _| shell.arrangement_details().is_none()));
    click(vcx, "button-Leave without saving");
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.arrangement_details().is_some())
    });
}

#[gpui::test]
fn an_archived_arrangement_is_a_read_only_frame_without_a_session(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, true);
    assert!(editor.read_with(vcx, |editor, _| editor.is_read_only()
        && editor.session().is_none()));
    assert_eq!(save_state(&editor, vcx), None);
    assert!(present(vcx, "arrangement-archived-notice"));
    assert!(!present(vcx, "editor-save-state"));
    assert!(!present(vcx, "button-Arrangement details"));
    assert!(
        present(vcx, "arrangement-section-0"),
        "the content is still inspectable"
    );
    click(vcx, "button-Back to Songs");
    settle(vcx);
    assert!(
        !editing(&shell, vcx),
        "a read-only frame leaves without a dialog"
    );
    assert_eq!(dialog(&shell, vcx), None);
}

#[gpui::test]
fn one_edit_session_serves_the_whole_visit_and_a_new_visit_starts_afresh(cx: &mut TestAppContext) {
    let (shell, editor, id, vcx) = in_memory(cx, false);
    let first = editor
        .read_with(vcx, |editor, _| editor.session().map(|s| s.entity_id()))
        .expect("session");
    click(vcx, "tab-1");
    click(vcx, "tab-0");
    let same = editor
        .read_with(vcx, |editor, _| editor.session().map(|s| s.entity_id()))
        .expect("session");
    assert_eq!(first, same, "tabs share the visit's session");
    click(vcx, "button-Back to Songs");
    settle(vcx);
    let editor = open_editor(&shell, id, vcx);
    let second = editor
        .read_with(vcx, |editor, _| editor.session().map(|s| s.entity_id()))
        .expect("session");
    assert_ne!(first, second, "closing dropped the first session");
}

#[gpui::test]
fn the_regions_are_the_workspace_actions_the_list_the_editor_and_the_inspector(
    cx: &mut TestAppContext,
) {
    let (_shell, _editor, _id, vcx) = in_memory(cx, false);
    // Each pane's content sits in its own region: header and tabs, sections,
    // lyrics, inspector.
    for selector in [
        "editor-tabs-row",
        "editor-title",
        "arrangement-sections",
        "arrangement-overview",
        "arrangement-inspector",
    ] {
        assert!(present(vcx, selector), "{selector}");
    }
}

#[gpui::test]
fn undo_and_redo_keys_are_registry_rows(cx: &mut TestAppContext) {
    for (id, keys, keycaps) in [
        (CommandId::Undo, "ctrl-z", "Ctrl+Z"),
        (CommandId::Redo, "ctrl-y", "Ctrl+Y"),
        (CommandId::MoveUp, "alt-up", "Alt+Up"),
        (CommandId::MoveDown, "alt-down", "Alt+Down"),
    ] {
        let row = registry::entry_for(id);
        assert_eq!(row.group, Group::Editing);
        assert_eq!(row.scope, Scope::Editing);
        assert_eq!(row.keys, &[keys]);
        assert_eq!(row.keycaps, keycaps);
    }
    // The registered keys act on the open editor's session.
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-undo-keys");
    let editor = open_editor(&shell, id, vcx);
    let original = stored_lyrics(&store, id, vcx);
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    let changed = session.update(vcx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |content| content.sections[0].lyrics = "Second words".to_owned(),
            cx,
        )
    });
    assert!(changed);
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    assert_eq!(stored_lyrics(&store, id, vcx), "Second words");
    // Focus sits on a control inside the editor, as it does for the operator.
    click(vcx, "tab-0");
    vcx.simulate_keystrokes("ctrl-z");
    wait(vcx, |vcx| stored_lyrics(&store, id, vcx) == original);
    vcx.simulate_keystrokes("ctrl-y");
    wait(vcx, |vcx| stored_lyrics(&store, id, vcx) == "Second words");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac2_empty_arrangement_offers_add_first_section_focused(cx: &mut TestAppContext) {
    let (shell, editor, vcx) = in_memory_empty(cx);
    assert!(present(vcx, "arrangement-no-sections"));
    assert!(present(vcx, "button-Add first section"));
    // Focus arrives on the action: Enter opens the popover.
    let keystroke = gpui::Keystroke::parse("enter").expect("key");
    vcx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    vcx.simulate_event(gpui::KeyUpEvent { keystroke });
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Add section"));
    assert!(editor.read_with(vcx, |editor, _| editor.add_section_open()));
}

#[gpui::test]
fn ac2_leaving_an_empty_arrangement_keeps_it_empty(cx: &mut TestAppContext) {
    let (shell, editor, vcx) = in_memory_empty(cx);
    assert!(sections_of(&editor, vcx).is_empty());
    click(vcx, "button-Back to Songs");
    wait(vcx, |vcx| !editing(&shell, vcx));
    settle(vcx);
    let store = shell.read_with(vcx, |shell, _| shell.library_store().clone());
    let stored = store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangement(ArrangementId::from_entity(entity(20)))
            .expect("stored")
            .lyrics()
            .sections
            .len()
    });
    assert_eq!(stored, 0, "opening and leaving writes no section");
    assert_eq!(dialog(&shell, vcx), None);
}

#[gpui::test]
fn ac3_add_section_needs_a_unique_name_then_focuses_the_lyrics(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    let attempt =
        |editor: &Entity<ArrangementEditor>, name: &'static str, vcx: &mut VisualTestContext| {
            editor.update(vcx, |editor, cx| editor.add_section(name, cx))
        };
    assert_eq!(
        attempt(&editor, "  ", vcx),
        Err("A section needs a name.".to_owned())
    );
    assert_eq!(
        attempt(&editor, "verse 1", vcx),
        Err("Another section is already called verse 1.".to_owned())
    );
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    // A double Enter inside one update adds once.
    let results = editor.update(vcx, |editor, cx| {
        (
            editor.add_section("Bridge", cx),
            editor.add_section("Bridge", cx),
        )
    });
    assert!(results.0.is_ok() && results.1.is_err(), "{results:?}");
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus", "Bridge"]);
    let (selected, last) = editor.read_with(vcx, |editor, cx| {
        (
            editor.selected_section(),
            editor.content(cx).expect("content").sections[2].id,
        )
    });
    assert_eq!(selected, Some(last), "the new section is selected");
}

#[gpui::test]
fn ac3_suggestions_are_editable_chips(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    click(vcx, "button-Add section");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Add section"));
    let field = editor
        .read_with(vcx, |editor, _| editor.add_section_field())
        .expect("popover open");
    assert_eq!(vcx.update(|_, cx| field.value(cx).to_string()), "Verse 2");
    assert!(present(vcx, "add-section-chips"));
    // A chip fills the field for editing; Chorus is taken so it is not offered.
    click_now(vcx, "button-Refrain");
    settle(vcx);
    assert_eq!(vcx.update(|_, cx| field.value(cx).to_string()), "Refrain");
}

fn area_value(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) -> String {
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("the lyric editor exists");
    vcx.update(|_, cx| area.value(cx).to_string())
}

/// Gives the lyric editor focus and replaces its text with `text` as typing
/// would (reports `Changed`).
fn type_into_area(editor: &Entity<ArrangementEditor>, text: &str, vcx: &mut VisualTestContext) {
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("the lyric editor exists");
    vcx.update(|window, cx| {
        area.set_value("", window, cx);
        area.focus(window, cx);
    });
    for (at, part) in text.split('\n').enumerate() {
        if at > 0 {
            vcx.simulate_keystrokes("enter");
        }
        if !part.is_empty() {
            vcx.simulate_input(part);
        }
    }
    vcx.run_until_parked();
}

fn caption(vcx: &mut VisualTestContext) -> bool {
    present(vcx, "arrangement-slide-caption")
}

/// Replaces the first section's lyrics in the session and reloads the field.
fn seed_lyrics(editor: &Entity<ArrangementEditor>, text: &str, vcx: &mut VisualTestContext) {
    type_lyrics(editor, text, vcx);
    editor.update(vcx, |editor, cx| {
        editor.lyrics.reload = true;
        cx.notify();
    });
    settle(vcx);
}

fn slide_counts(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) -> (usize, usize) {
    let text = area_value(editor, vcx);
    let selected = editor.read_with(vcx, |editor, _| editor.selected_slide());
    (domain::pagination::slides(&text).len(), selected)
}

#[gpui::test]
fn typing_sets_the_lyrics_and_a_blank_line_makes_two_slides(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    let _ = shell;
    type_into_area(
        &editor,
        "Ginoó

Niño",
        vcx,
    );
    assert_eq!(
        area_value(&editor, vcx),
        "Ginoó

Niño"
    );
    let session = editor
        .read_with(vcx, |editor, _| editor.session().cloned())
        .expect("session");
    let stored = session.read_with(vcx, |session, _| {
        session.content().sections[0].lyrics.clone()
    });
    assert_eq!(
        stored,
        "Ginoó

Niño",
        "Changed reaches the session"
    );
    assert_eq!(slide_counts(&editor, vcx).0, 2);
    assert!(caption(vcx));
    // Deleting the blank line joins the slides again.
    vcx.simulate_keystrokes("home backspace");
    settle(vcx);
    assert_eq!(
        area_value(&editor, vcx),
        "Ginoó
Niño"
    );
    assert_eq!(slide_counts(&editor, vcx).0, 1);
}

#[gpui::test]
fn split_and_join_are_structural_edits_with_exact_labels(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-split-join");
    let editor = open_editor(&shell, id, vcx);
    seed_lyrics(
        &editor,
        "Ginoó
Niño
Paz
Vida",
        vcx,
    );
    assert!(
        present(vcx, "button-Split after line 2"),
        "labelled by its line"
    );
    assert!(
        present(vcx, "button-Join with previous"),
        "slide 1 cannot join"
    );
    click(vcx, "button-Split after line 2");
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    assert_eq!(
        stored_lyrics(&store, id, vcx),
        "Ginoó
Niño

Paz
Vida"
    );
    assert_eq!(
        area_value(&editor, vcx),
        "Ginoó
Niño

Paz
Vida",
        "reloaded"
    );
    assert_eq!(slide_counts(&editor, vcx), (2, 0));
    // A native Ctrl+Z in the field cannot bring back the pre-split text.
    editor.update_in(vcx, |editor, window, cx| {
        editor.select_slide(1, window, cx);
    });
    settle(vcx);
    assert!(present(vcx, "button-Join slide 2 with slide 1"));
    vcx.simulate_keystrokes("ctrl-z");
    settle(vcx);
    assert_eq!(
        area_value(&editor, vcx),
        "Ginoó
Niño

Paz
Vida"
    );
    click(vcx, "button-Join slide 2 with slide 1");
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    assert_eq!(
        stored_lyrics(&store, id, vcx),
        "Ginoó
Niño
Paz
Vida"
    );
    assert_eq!(slide_counts(&editor, vcx), (1, 0));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn split_is_unavailable_on_a_one_line_slide(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    seed_lyrics(&editor, "Ginoó", vcx);
    assert!(present(vcx, "button-Split slide"));
    let refused = editor.update(vcx, |editor, cx| editor.split_selected_slide(cx));
    assert_eq!(refused, Err("A slide needs two lines to split.".to_owned()));
    assert_eq!(
        editor.update(vcx, |editor, cx| editor.join_selected_slide(cx)),
        Err("The first slide has no slide before it.".to_owned())
    );
}

#[gpui::test]
fn the_selected_slide_follows_the_caret(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    type_into_area(
        &editor,
        "Ginoó

Niño",
        vcx,
    );
    settle(vcx);
    assert_eq!(slide_counts(&editor, vcx), (2, 1));
    vcx.simulate_keystrokes("ctrl-home");
    settle(vcx);
    assert_eq!(slide_counts(&editor, vcx), (2, 0));
}

fn thumbnail_size(vcx: &mut VisualTestContext, selector: &'static str) -> (f32, f32) {
    let bounds = vcx.debug_bounds(selector).expect(selector);
    (bounds.size.width.into(), bounds.size.height.into())
}

fn wait_for_frames(editor: &Entity<ArrangementEditor>, count: usize, vcx: &mut VisualTestContext) {
    wait(vcx, |vcx| {
        settle(vcx);
        editor.read_with(vcx, |editor, _| editor.lyrics.frames.len()) >= count
    });
}

#[gpui::test]
fn thumbnails_have_one_fixed_16_9_size_whether_alone_or_in_a_full_row(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    seed_lyrics(&editor, "Ginoó", vcx);
    let alone = thumbnail_size(vcx, "thumb-0");
    seed_lyrics(&editor, "Ginoó

Niño

Paz

Vida

Amén

Ave", vcx);
    for slide in ["thumb-0", "thumb-3", "thumb-5"] {
        assert_eq!(thumbnail_size(vcx, slide), alone, "{slide}");
    }
    assert!((alone.0 * 9.0 / 16.0 - alone.1).abs() < 1.0, "16:9: {alone:?}");
}

#[gpui::test]
fn slides_are_composed_off_thread_cached_and_shown_in_the_inspector(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    seed_lyrics(&editor, "Ginoó
Niño

Paz
Vida", vcx);
    wait_for_frames(&editor, 2, vcx);
    assert!(present(vcx, "inspector-preview") && present(vcx, "inspector-meta"));
    let ready = editor.read_with(vcx, |editor, _| {
        editor.lyrics.frames.len()
    });
    assert!(ready >= 2, "a frame per slide, cached by (header, body)");
    // Re-rendering composes nothing new.
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, _| editor.lyrics.frames.len()), ready);
    assert!(editor.read_with(vcx, |editor, _| editor.lyrics.pending.is_empty()));
    // Another section drops what the first still waited for.
    click(vcx, "arrangement-section-1");
    settle(vcx);
    assert_eq!(
        editor.read_with(vcx, |editor, _| editor.selected_section().is_some()),
        true
    );
}

#[gpui::test]
fn clicking_a_thumbnail_selects_its_lines_and_the_inspector_follows(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    seed_lyrics(&editor, "Ginoó
Niño

Paz
Vida", vcx);
    click(vcx, "thumb-1");
    settle(vcx);
    assert_eq!(editor.read_with(vcx, |editor, _| editor.selected_slide()), 1);
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    let (range, value) = vcx.update(|_, cx| (area.selected_range(cx), area.value(cx).to_string()));
    assert_eq!(&value[range], "Paz
Vida", "the slide's source lines are selected");
    assert!(present(vcx, "inspector-slide"));
}

#[gpui::test]
fn below_the_pane_minima_only_the_inspector_moves_to_a_slideover(cx: &mut TestAppContext) {
    let (shell, _editor, _id, vcx) = in_memory(cx, false);
    vcx.simulate_resize(gpui::size(gpui::px(800.0), gpui::px(600.0)));
    settle(vcx);
    assert!(present(vcx, "arrangement-sections"), "the sections stay");
    assert!(present(vcx, "arrangement-overview"));
    assert!(!present(vcx, "arrangement-inspector"), "no Inspector pane");
    assert!(present(vcx, "button-Inspector"));
    click(vcx, "button-Inspector");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Inspector"));
    assert!(present(vcx, "inspector-slide"));
}
