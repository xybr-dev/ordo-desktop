//! The Arrangement editor frame through the shell that hosts it: task tabs,
//! save state, the navigation guard and the Editing keys.

use domain::{
    identity::{
        ArrangementId, EntityId, HistorySessionId, OccurrenceId, SectionId, SongId, Timestamp,
    },
    revisions::EditPoint,
    song_library::{
        ArrangementDraft, OccurrenceDraft, PaginationState, SectionDraft, SongDraft, SongLibrary,
    },
};
use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use super::{ArrangementEditor, TaskTab};
use crate::{
    commands::registry::{self, CommandId, Group, Scope},
    edit_session::SaveState,
    shell::{ShellView, rail::Destination},
    songs::{
        LibraryStore,
        arrangement_details::tests::write,
        choirs::tests::{click as click_now, open_store, present, wait},
        new_song::tests::{click, finish, parish, settle},
        sequence::SequenceSelection,
    },
};

const SAVE_DIALOG: &str = "Couldn\u{2019}t save";

pub(crate) fn entity(n: u8) -> EntityId {
    EntityId::from_random_bytes([n; 16])
}

pub(crate) fn at() -> Timestamp {
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
pub(crate) fn in_memory(
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

pub(crate) fn sections_of(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Vec<String> {
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

pub(crate) fn open_editor(
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

pub(crate) fn save_state(
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

pub(crate) fn dialog(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) -> Option<String> {
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
pub(crate) fn parish_editor<'a>(
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
    parish_with(cx, tag, false)
}

/// The same Arrangement as [`library`]: Verse 1 and Chorus, a Sequence of
/// Verse, Chorus, Verse and Approved pagination, written through the real
/// store.
pub(crate) fn parish_full<'a>(
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
    parish_with(cx, tag, true)
}

fn parish_with<'a>(
    cx: &'a mut TestAppContext,
    tag: &str,
    full: bool,
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
    let verse = SectionId::from_entity(entity(77));
    if full {
        let chorus = SectionId::from_entity(entity(78));
        let occurrence =
            |n: u8, section| OccurrenceDraft::new(OccurrenceId::from_entity(entity(n)), section);
        lyrics.sections.clear();
        lyrics.sections.extend([
            SectionDraft::labeled(verse, "Verse 1", "Ave Maria"),
            SectionDraft::labeled(chorus, "Chorus", "Refrain line"),
        ]);
        lyrics.sequence = vec![
            occurrence(81, verse),
            occurrence(82, chorus),
            occurrence(83, verse),
        ];
        for section in &mut lyrics.sections {
            section.pagination = PaginationState::Approved;
        }
    } else {
        lyrics
            .sections
            .push(SectionDraft::labeled(verse, "Verse 1", "First words"));
    }
    let session = HistorySessionId::from_entity(entity(9));
    write(&store, vcx, |store, cx| {
        store.save_arrangement_lyrics(id, lyrics, session, cx)
    });
    (dir, slot, store, shell, id, vcx)
}

/// An Arrangement with no sections at all, written through the real store,
/// and the shell over it: the start of the E2E.
pub(crate) fn parish_blank<'a>(
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
    if !lyrics.sections.is_empty() || !lyrics.sequence.is_empty() {
        lyrics.sections.clear();
        lyrics.sequence.clear();
        let session = HistorySessionId::from_entity(entity(9));
        write(&store, vcx, |store, cx| {
            store.save_arrangement_lyrics(id, lyrics, session, cx)
        });
    }
    (dir, slot, store, shell, id, vcx)
}

/// The section whose list row has keyboard focus, if any.
pub(crate) fn focused_row(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Option<SectionId> {
    let rows: Vec<_> = editor.read_with(vcx, |editor, _| {
        editor
            .lyrics
            .rows
            .iter()
            .map(|(id, row)| (*id, row.clone()))
            .collect()
    });
    vcx.update(|window, _| {
        rows.iter()
            .find(|(_, row)| row.is_focused(window))
            .map(|(id, _)| *id)
    })
}

/// The serialized frame of every thumbnail of the selected section, once each
/// is composed: what the operator sees in the navigator.
pub(crate) fn thumbnail_frames(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> Vec<String> {
    let text = area_value(editor, vcx);
    let label = editor.read_with(vcx, |editor, cx| {
        let content = editor.content(cx)?;
        let chosen = editor.selected_section()?;
        content
            .sections
            .iter()
            .find(|section| section.id == chosen)
            .and_then(|section| section.label.clone())
    });
    let bodies: Vec<String> = domain::pagination::slides(&text)
        .iter()
        .map(|slide| domain::pagination::slide_body(&text, slide))
        .collect();
    wait(vcx, |vcx| {
        settle(vcx);
        editor.read_with(vcx, |editor, _| {
            bodies.iter().all(|body| {
                editor
                    .lyrics
                    .frames
                    .peek(&(label.clone(), body.clone()))
                    .is_some()
            })
        })
    });
    editor.read_with(vcx, |editor, _| {
        bodies
            .iter()
            .map(|body| {
                match editor
                    .lyrics
                    .frames
                    .peek(&(label.clone(), body.clone()))
                    .expect("composed")
                {
                    rendering::FrameEntry::Ready(frame) => frame.serialize(),
                    rendering::FrameEntry::Unavailable(reason) => {
                        panic!("a thumbnail could not be composed: {reason}")
                    }
                }
            })
            .collect()
    })
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
fn an_archived_song_makes_the_editor_read_only(cx: &mut TestAppContext) {
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
    for hidden in [
        "button-Split here",
        "button-Join with next slide",
        "button-Join with previous slide",
        "button-Remove section",
        "button-Move up",
        "button-Move down",
        "button-Add section",
    ] {
        assert!(
            !present(vcx, hidden),
            "{hidden} is not offered when read-only"
        );
    }
    assert!(
        editor.read_with(vcx, |editor, _| editor.lyric_area().is_some()),
        "the text is still shown"
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
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-add-unique");
    let editor = open_editor(&shell, id, vcx);
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
        Err("Another section is already called Verse 1.".to_owned())
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
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    finish(vcx, &slot, &dir);
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

pub(crate) fn area_value(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> String {
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("the lyric editor exists");
    vcx.update(|_, cx| area.value(cx).to_string())
}

/// Gives the lyric editor focus and replaces its text with `text` as typing
/// would (reports `Changed`).
pub(crate) fn type_into_area(
    editor: &Entity<ArrangementEditor>,
    text: &str,
    vcx: &mut VisualTestContext,
) {
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
    present(vcx, "arrangement-section-sung")
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
fn ac4_blank_line_starts_a_slide_and_deleting_it_joins(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-typing-sets-the-lyri");
    let editor = open_editor(&shell, id, vcx);
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
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_split_and_join_labels_name_exact_targets(cx: &mut TestAppContext) {
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
        present(vcx, "button-Join with previous slide"),
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
    assert_eq!(
        slide_counts(&editor, vcx),
        (2, 1),
        "after a split the caret is on the new slide"
    );
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
fn ac5_join_is_disabled_on_the_first_slide(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-split-is-unavailable");
    let editor = open_editor(&shell, id, vcx);
    seed_lyrics(&editor, "Ginoó", vcx);
    assert!(present(vcx, "button-Split slide"));
    let refused = editor.update(vcx, |editor, cx| editor.split_selected_slide(cx));
    assert_eq!(refused, Err("A slide needs two lines to split.".to_owned()));
    assert_eq!(
        editor.update(vcx, |editor, cx| editor.join_selected_slide(cx)),
        Err("The first slide has no slide before it.".to_owned())
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn the_selected_slide_follows_the_caret(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-the-selected-slide-f");
    let editor = open_editor(&shell, id, vcx);
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
    finish(vcx, &slot, &dir);
}

fn thumbnail_size(vcx: &mut VisualTestContext, selector: &'static str) -> (f32, f32) {
    let bounds = vcx.debug_bounds(selector).expect(selector);
    (bounds.size.width.into(), bounds.size.height.into())
}

pub(crate) fn wait_for_frames(
    editor: &Entity<ArrangementEditor>,
    count: usize,
    vcx: &mut VisualTestContext,
) {
    wait(vcx, |vcx| {
        settle(vcx);
        editor.read_with(vcx, |editor, _| editor.lyrics.frames.len()) >= count
    });
}

#[gpui::test]
fn ac7_thumbnails_keep_one_size_alone_and_in_a_full_row(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-thumbnails-have-one-");
    let editor = open_editor(&shell, id, vcx);
    seed_lyrics(&editor, "Ginoó", vcx);
    let alone = thumbnail_size(vcx, "thumb-0");
    seed_lyrics(
        &editor,
        "Ginoó

Niño

Paz

Vida

Amén

Ave",
        vcx,
    );
    for slide in ["thumb-0", "thumb-3", "thumb-5"] {
        assert_eq!(thumbnail_size(vcx, slide), alone, "{slide}");
    }
    assert!(
        (alone.0 * 9.0 / 16.0 - alone.1).abs() < 1.0,
        "16:9: {alone:?}"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn slides_are_composed_off_thread_cached_and_shown_in_the_inspector(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-slides-are-composed-");
    let editor = open_editor(&shell, id, vcx);
    seed_lyrics(
        &editor,
        "Ginoó
Niño

Paz
Vida",
        vcx,
    );
    wait_for_frames(&editor, 2, vcx);
    assert!(present(vcx, "inspector-preview") && present(vcx, "inspector-meta"));
    let ready = editor.read_with(vcx, |editor, _| editor.lyrics.frames.len());
    assert!(ready >= 2, "a frame per slide, cached by (header, body)");
    // Re-rendering composes nothing new.
    settle(vcx);
    assert_eq!(
        editor.read_with(vcx, |editor, _| editor.lyrics.frames.len()),
        ready
    );
    assert!(editor.read_with(vcx, |editor, _| editor.lyrics.pending.is_empty()));
    // Another section drops what the first still waited for.
    click(vcx, "arrangement-section-1");
    settle(vcx);
    assert!(editor.read_with(vcx, |editor, _| editor.selected_section().is_some()));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac7_selecting_a_thumbnail_selects_its_lines_and_drives_the_inspector(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-clicking-a-thumbnail");
    let editor = open_editor(&shell, id, vcx);
    seed_lyrics(
        &editor,
        "Ginoó
Niño

Paz
Vida",
        vcx,
    );
    click(vcx, "thumb-1");
    settle(vcx);
    assert_eq!(
        editor.read_with(vcx, |editor, _| editor.selected_slide()),
        1
    );
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    let (range, value) = vcx.update(|_, cx| (area.selected_range(cx), area.value(cx).to_string()));
    assert_eq!(
        &value[range],
        "Paz
Vida",
        "the slide's source lines are selected"
    );
    assert!(present(vcx, "inspector-slide"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn below_the_pane_minima_only_the_inspector_moves_to_a_slideover(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-below-the-pane-minim");
    let _editor = open_editor(&shell, id, vcx);
    vcx.simulate_resize(gpui::size(gpui::px(800.0), gpui::px(600.0)));
    settle(vcx);
    assert!(present(vcx, "arrangement-sections"), "the sections stay");
    assert!(present(vcx, "arrangement-lyrics"));
    assert!(!present(vcx, "arrangement-inspector"), "no Inspector pane");
    assert!(present(vcx, "button-Inspector"));
    click(vcx, "button-Inspector");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Inspector"));
    assert!(present(vcx, "inspector-meta"));
    finish(vcx, &slot, &dir);
}

pub(crate) fn toasts(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(vcx, |shell, cx| {
        shell
            .overlays()
            .toasts(cx)
            .iter()
            .map(|toast| toast.to_string())
            .collect()
    })
}

pub(crate) fn sequence_len(
    editor: &Entity<ArrangementEditor>,
    vcx: &mut VisualTestContext,
) -> usize {
    editor.read_with(vcx, |editor, cx| {
        editor.content(cx).expect("content").sequence.len()
    })
}

fn type_in_rename(editor: &Entity<ArrangementEditor>, text: &str, vcx: &mut VisualTestContext) {
    let field = editor
        .read_with(vcx, |editor, _| editor.rename_field())
        .expect("rename field");
    vcx.update(|window, cx| {
        field.focus(window, cx);
        field.set_value(text.to_owned(), window, cx);
    });
}

#[gpui::test]
fn move_buttons_reorder_the_list_only_and_edges_are_unavailable(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-move-buttons");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    // Verse 1 is first: Move up does nothing and writes nothing.
    click(vcx, "button-Move up");
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    assert!(toasts(&shell, vcx).is_empty(), "an edge move is a no-op");
    let sequence = sequence_len(&editor, vcx);
    click(vcx, "button-Move down");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Chorus", "Verse 1"]);
    assert_eq!(
        sequence_len(&editor, vcx),
        sequence,
        "the Sequence is unchanged"
    );
    assert_eq!(
        toasts(&shell, vcx),
        ["Moved Verse 1 in Lyrics. The Sequence is unchanged."]
    );
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_alt_up_moves_the_focused_section_and_undo_returns_it(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t27-alt-up");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "arrangement-section-1");
    let chorus = editor
        .read_with(vcx, |editor, _| editor.selected_section())
        .expect("selected");
    let row = editor.read_with(vcx, |editor, _| editor.lyrics.rows[&chorus].clone());
    vcx.update(|window, cx| window.focus(&row, cx));
    vcx.simulate_keystrokes("alt-up");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Chorus", "Verse 1"]);
    let focused = vcx.update(|window, _| row.is_focused(window));
    assert!(focused, "focus stays on the moved row");
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    let stored = |vcx: &mut VisualTestContext| -> Vec<String> {
        store.read_with(vcx, |store, _| {
            store
                .library()
                .arrangement(id)
                .expect("stored")
                .lyrics()
                .sections
                .iter()
                .map(|section| section.label.clone().unwrap_or_default())
                .collect()
        })
    };
    assert_eq!(stored(vcx), ["Chorus", "Verse 1"], "saved in the new order");
    // Undo outside a text field is the editor's own.
    vcx.simulate_keystrokes("ctrl-z");
    wait(vcx, |vcx| {
        settle(vcx);
        sections_of(&editor, vcx) == ["Verse 1", "Chorus"]
    });
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    assert_eq!(stored(vcx), ["Verse 1", "Chorus"]);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn rename_commits_on_enter_restores_on_escape_and_explains_a_taken_name(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-rename-enter");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    type_in_rename(&editor, "chorus", vcx);
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert!(
        present(vcx, "arrangement-rename-error"),
        "a taken name is explained"
    );
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    vcx.simulate_keystrokes("escape");
    settle(vcx);
    let field = editor
        .read_with(vcx, |editor, _| editor.rename_field())
        .expect("field");
    assert_eq!(vcx.update(|_, cx| field.value(cx).to_string()), "Verse 1");
    assert!(!present(vcx, "arrangement-rename-error"));
    type_in_rename(&editor, "Verse One", vcx);
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse One", "Chorus"]);
    assert_eq!(
        toasts(&shell, vcx),
        ["Renamed Verse 1 to Verse One. Every Sequence occurrence shows the new name."]
    );
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac9_removing_a_used_section_confirms_with_the_count(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-remove-used");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "arrangement-section-1");
    click(vcx, "button-Remove section");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Remove Chorus?"));
    click(vcx, "button-Cancel");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    click(vcx, "button-Remove section");
    settle(vcx);
    click(vcx, "button-Remove section");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1"]);
    assert_eq!(
        sequence_len(&editor, vcx),
        2,
        "the Chorus occurrence went with it"
    );
    assert_eq!(toasts(&shell, vcx), ["Removed Chorus."]);
    // Selection falls back onto an existing section.
    assert!(editor.read_with(vcx, |editor, _| editor.selected_section().is_some()));
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn undo_after_a_structural_section_edit_restores_the_list_and_selection_stays_valid(
    cx: &mut TestAppContext,
) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-section-undo");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let base = sections_of(&editor, vcx);
    let mut with_bridge = base.clone();
    with_bridge.push("Bridge".to_owned());
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    editor.update(vcx, |editor, cx| {
        let id = editor.selected_section().expect("selected");
        editor.remove_section(id, cx).expect("removed");
    });
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), base);
    editor.update(vcx, |editor, cx| editor.undo(cx));
    wait(vcx, |vcx| {
        settle(vcx);
        sections_of(&editor, vcx) == with_bridge
    });
    assert!(editor.read_with(vcx, |editor, _| editor.selected_section().is_some()));
    finish(vcx, &slot, &dir);
}

pub(crate) fn region_now(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) -> String {
    vcx.update(|window, cx| {
        shell
            .read(cx)
            .focused_region(window, cx)
            .map(|name| name.to_string())
            .unwrap_or_default()
    })
}

#[gpui::test]
fn f6_cycles_actions_sections_lyrics_inspector_and_wraps(cx: &mut TestAppContext) {
    let (shell, _editor, _id, vcx) = in_memory(cx, false);
    settle(vcx);
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
        .position(|n| *n == visited[0])
        .expect("a known region");
    let expected: Vec<_> = (0..4).map(|k| order[(start + k) % 4]).collect();
    assert_eq!(visited, expected, "reading order, all four");
    vcx.simulate_keystrokes("f6");
    settle(vcx);
    assert_eq!(region_now(&shell, vcx), visited[0], "it wraps");
    vcx.simulate_keystrokes("shift-f6");
    settle(vcx);
    assert_eq!(region_now(&shell, vcx), visited[3], "and goes back");

    // Compact: the Inspector is a button, not a pane, so the cycle has three.
    vcx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(900.0)));
    vcx.run_until_parked();
    settle(vcx);
    let mut visited = Vec::new();
    for _ in 0..4 {
        vcx.simulate_keystrokes("f6");
        settle(vcx);
        visited.push(region_now(&shell, vcx));
    }
    assert!(
        !visited.iter().any(|n| n == "Inspector"),
        "no Inspector pane"
    );
    assert_eq!(visited[3], visited[0], "three regions, then it wraps");
}

#[gpui::test]
fn rail_navigation_flushes_pending_lyrics_then_leaves(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-rail-flush");
    let editor = open_editor(&shell, id, vcx);
    type_lyrics(&editor, "Words changed", vcx);
    assert_eq!(save_state(&editor, vcx), Some(SaveState::Saving));
    shell.update_in(vcx, |shell, window, cx| {
        shell.open(Destination::Masses, window, cx);
    });
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.destination() == Destination::Masses)
    });
    settle(vcx);
    assert_eq!(stored_lyrics(&store, id, vcx), "Words changed");
    assert_eq!(dialog(&shell, vcx), None, "a successful flush asks nothing");
    assert!(shell.read_with(vcx, |shell, _| shell.arrangement_editor().is_none()));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_failed_flush_blocks_rail_navigation(cx: &mut TestAppContext) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    type_lyrics(&editor, "Unsaved words", vcx);
    shell.update_in(vcx, |shell, window, cx| {
        shell.open(Destination::Masses, window, cx);
    });
    wait_for_dialog(&shell, vcx);
    assert_eq!(
        shell.read_with(vcx, |shell, _| shell.destination()),
        Destination::Library,
        "the destination did not change"
    );
    assert!(shell.read_with(vcx, |shell, _| shell.arrangement_editor().is_some()));
    click(vcx, "button-Leave without saving");
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.destination() == Destination::Masses)
    });
    settle(vcx);
    assert_eq!(dialog(&shell, vcx), None);
    assert!(shell.read_with(vcx, |shell, _| shell.arrangement_editor().is_none()));
}

/// Fails the save, so the session is in `CouldntSave`.
pub(crate) fn fail_the_save(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) {
    type_lyrics(editor, "Unsaved words", vcx);
    editor.update(vcx, |editor, cx| editor.request_navigation(cx).detach());
    vcx.run_until_parked();
    settle(vcx);
    assert_eq!(save_state(editor, vcx), Some(SaveState::CouldntSave));
}

#[gpui::test]
fn after_a_failed_save_structural_edits_say_so_and_the_header_offers_retry_save(
    cx: &mut TestAppContext,
) {
    let (shell, editor, _id, vcx) = in_memory(cx, false);
    settle(vcx);
    assert!(!present(vcx, "button-Retry save"), "nothing failed yet");
    fail_the_save(&editor, vcx);
    assert!(
        present(vcx, "button-Retry save"),
        "the header offers Retry save"
    );

    click(vcx, "button-Move down");
    assert_eq!(
        sections_of(&editor, vcx),
        ["Verse 1", "Chorus"],
        "nothing moved"
    );
    assert_eq!(
        toasts(&shell, vcx),
        ["Couldn't save. Retry to keep editing."]
    );

    // The Add section popover stays open and says why.
    click(vcx, "button-Add section");
    settle(vcx);
    assert!(editor.read_with(vcx, |editor, _| editor.add_section_open()));
    assert!(present(vcx, "add-section-error"));

    // Retry save runs the save again (it still fails here, so the button stays).
    click(vcx, "button-Retry save");
    vcx.run_until_parked();
    assert_eq!(save_state(&editor, vcx), Some(SaveState::CouldntSave));
}

#[gpui::test]
fn a_rename_blurred_after_a_failed_save_keeps_the_typed_name(cx: &mut TestAppContext) {
    let (_shell, editor, _id, vcx) = in_memory(cx, false);
    settle(vcx);
    fail_the_save(&editor, vcx);
    type_in_rename(&editor, "Verse One", vcx);
    // Focus leaves the field for the lyric editor.
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    vcx.update(|window, cx| area.focus(window, cx));
    settle(vcx);
    let field = editor
        .read_with(vcx, |editor, _| editor.rename_field())
        .expect("field");
    assert_eq!(vcx.update(|_, cx| field.value(cx).to_string()), "Verse One");
    assert!(present(vcx, "arrangement-rename-error"));
}

#[gpui::test]
fn a_rename_typed_just_before_choosing_another_section_is_committed(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-rename-switch");
    let editor = open_editor(&shell, id, vcx);
    editor.update(vcx, |editor, cx| {
        editor.add_section("Chorus", cx).expect("added");
    });
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    click(vcx, "arrangement-section-0");
    type_in_rename(&editor, "Verse One", vcx);
    editor.update(vcx, |editor, cx| {
        let other = editor.content(cx).expect("content").sections[1].id;
        editor.select_section(other, cx);
    });
    vcx.run_until_parked();
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    let stored = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("stored").lyrics()
    });
    let names: Vec<_> = stored.sections.iter().map(|s| s.label.clone()).collect();
    assert_eq!(names[0].as_deref(), Some("Verse One"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn the_undo_toast_is_taken_back_by_the_next_edit(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-toast-undo");
    let editor = open_editor(&shell, id, vcx);
    editor.update(vcx, |editor, cx| {
        editor.add_section("Bridge", cx).expect("added");
    });
    settle(vcx);
    assert_eq!(toasts(&shell, vcx), ["Added section Bridge."]);
    // The lyric editor took focus; typing is the next edit.
    type_into_area(&editor, "x", vcx);
    settle(vcx);
    assert!(
        toasts(&shell, vcx).is_empty(),
        "its Undo would undo the typing instead"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ctrl_z_in_a_text_field_is_the_fields_own_and_outside_it_is_the_editors(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_editor(cx, "t27-ctrl-z-routing");
    let editor = open_editor(&shell, id, vcx);
    editor.update(vcx, |editor, cx| {
        editor.add_section("Bridge", cx).expect("added");
    });
    wait(vcx, |vcx| {
        save_state(&editor, vcx) == Some(SaveState::Saved)
    });
    settle(vcx);
    let with_bridge = sections_of(&editor, vcx);
    assert_eq!(with_bridge.last().map(String::as_str), Some("Bridge"));
    // The lyric editor has focus (Add section gave it): its own Undo wins,
    // and with nothing typed it does nothing, so the section stays.
    vcx.simulate_keystrokes("ctrl-z");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), with_bridge);
    // Outside a text field the editor's Undo takes the section back.
    click(vcx, "arrangement-section-0");
    let row = editor.read_with(vcx, |editor, cx| {
        let first = editor.content(cx).expect("content").sections[0].id;
        editor.lyrics.rows[&first].clone()
    });
    vcx.update(|window, cx| window.focus(&row, cx));
    vcx.simulate_keystrokes("ctrl-z");
    wait(vcx, |vcx| {
        settle(vcx);
        sections_of(&editor, vcx).len() < with_bridge.len()
    });
    finish(vcx, &slot, &dir);
}

pub(crate) fn stored_pagination(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> Vec<PaginationState> {
    store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("stored")
            .lyrics()
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect()
    })
}

/// Lets a debounced text edit come due, then waits for the write.
pub(crate) fn saved(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) {
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(10));
    wait(vcx, |vcx| save_state(editor, vcx) == Some(SaveState::Saved));
    settle(vcx);
}

#[gpui::test]
fn removing_an_unused_section_is_immediate_with_an_undo_toast(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-remove-unused");
    let editor = open_editor(&shell, id, vcx);
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    saved(&editor, vcx);
    click(vcx, "button-Remove section");
    settle(vcx);
    assert_eq!(
        dialog(&shell, vcx),
        None,
        "an unused section needs no question"
    );
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    assert_eq!(toasts(&shell, vcx), ["Removed Bridge."]);
    assert!(present(vcx, "button-Undo"));
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac9_undo_restores_the_removed_section_and_its_occurrences(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t27-undo-restores");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "arrangement-section-1");
    click(vcx, "button-Remove section");
    settle(vcx);
    click(vcx, "button-Remove section");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1"]);
    assert_eq!(sequence_len(&editor, vcx), 2);
    click(vcx, "button-Undo");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    assert_eq!(sequence_len(&editor, vcx), 3, "the occurrence came back");
    saved(&editor, vcx);
    let stored = store.read_with(vcx, |store, _| {
        store.library().arrangement(id).expect("stored").lyrics()
    });
    assert_eq!(stored.sections.len(), 2);
    assert_eq!(stored.sequence.len(), 3);
    assert!(
        stored
            .sections
            .iter()
            .all(|section| section.pagination == PaginationState::Approved),
        "Undo restored every section's approval"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn a_toast_undo_is_dismissed_by_a_later_edit_through_the_real_store(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-toast-later");
    let editor = open_editor(&shell, id, vcx);
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    saved(&editor, vcx);
    type_into_area(&editor, "Bridge words", vcx);
    saved(&editor, vcx);
    assert!(
        toasts(&shell, vcx).is_empty(),
        "the Undo toast would undo the typing"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn typing_on_an_approved_arrangement_needs_review_and_undo_restores_approved(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t27-needs-review");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    assert_eq!(
        stored_pagination(&store, id, vcx),
        vec![PaginationState::Approved, PaginationState::Approved]
    );
    type_into_area(&editor, "Ave Maria, gratia plena", vcx);
    saved(&editor, vcx);
    assert_eq!(
        stored_pagination(&store, id, vcx),
        vec![PaginationState::NeedsReview, PaginationState::Approved],
        "only the edited section needs review; the other keeps its approval"
    );
    // Focus outside a text field: Undo is the editor's own.
    click(vcx, "arrangement-section-0");
    vcx.simulate_keystrokes("ctrl-z");
    settle(vcx);
    saved(&editor, vcx);
    assert_eq!(
        stored_pagination(&store, id, vcx),
        vec![PaginationState::Approved, PaginationState::Approved]
    );
    assert_eq!(stored_lyrics(&store, id, vcx), "Ave Maria");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn rename_commits_on_blur_and_on_section_switch(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-rename-blur");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    // A blank name is refused on blur and the old one is kept.
    type_in_rename(&editor, "  ", vcx);
    vcx.update(|window, cx| area.focus(window, cx));
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse 1", "Chorus"]);
    assert_eq!(
        toasts(&shell, vcx),
        ["A section needs a name. Kept Verse 1."]
    );
    // A good name is committed by leaving the field.
    type_in_rename(&editor, "Verse One", vcx);
    vcx.update(|window, cx| area.focus(window, cx));
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse One", "Chorus"]);
    saved(&editor, vcx);
    // And by choosing another section.
    type_in_rename(&editor, "Verse Uno", vcx);
    click(vcx, "arrangement-section-1");
    settle(vcx);
    assert_eq!(sections_of(&editor, vcx), ["Verse Uno", "Chorus"]);
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn add_section_popover_enter_adds_and_focuses_the_lyrics(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-popover-enter");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    click(vcx, "button-Add section");
    settle(vcx);
    assert_eq!(dialog(&shell, vcx).as_deref(), Some("Add section"));
    vcx.simulate_keystrokes("enter");
    settle(vcx);
    let names = sections_of(&editor, vcx);
    assert_eq!(names.len(), 3, "the suggested name was added: {names:?}");
    assert_eq!(names[..2], ["Verse 1", "Chorus"]);
    assert!(!editor.read_with(vcx, |editor, _| editor.add_section_open()));
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    assert!(
        vcx.update(|window, cx| area.focus_handle(cx).is_focused(window)),
        "the lyric editor has focus"
    );
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn crlf_paste_splits_slides_and_keeps_bytes(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t27-crlf-paste");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    vcx.update(|window, cx| {
        area.set_value("", window, cx);
        area.focus(window, cx);
    });
    vcx.write_to_clipboard(gpui::ClipboardItem::new_string(
        "Ave\r\nMaria\r\n\r\nGratia".into(),
    ));
    vcx.simulate_keystrokes("ctrl-v");
    settle(vcx);
    saved(&editor, vcx);
    let shown = area_value(&editor, vcx);
    let kept = stored_lyrics(&store, id, vcx);
    assert_eq!(kept, shown, "what is stored is what the field holds");
    assert_eq!(
        domain::pagination::slides(&kept).len(),
        2,
        "a CRLF blank line starts a slide: {kept:?}"
    );
    assert!(kept.contains("Ave") && kept.contains("Gratia"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn panes_switch_to_compact_by_scaled_minima(cx: &mut TestAppContext) {
    use gpui::{px, size};
    use ui::environment::{EnvironmentChange, InterfaceScale};
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t27-compact-scale");
    let _editor = open_editor(&shell, id, vcx);
    vcx.simulate_resize(size(px(1400.0), px(900.0)));
    settle(vcx);
    assert!(
        present(vcx, "arrangement-inspector"),
        "100% fits all three panes"
    );
    assert!(!present(vcx, "button-Inspector"));
    shell.update(vcx, |shell, cx| {
        shell.change_from_settings(EnvironmentChange::SetScale(InterfaceScale::Percent125), cx);
    });
    settle(vcx);
    assert!(
        !present(vcx, "arrangement-inspector"),
        "the same window is too narrow once the minima are scaled"
    );
    assert!(present(vcx, "button-Inspector"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn entering_presenter_mode_flushes_pending_lyrics(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_editor(cx, "t27-presenter-flush");
    let editor = open_editor(&shell, id, vcx);
    type_lyrics(&editor, "Typed before going live", vcx);
    vcx.dispatch_action(crate::shell::EnterPresenter);
    wait(vcx, |vcx| {
        stored_lyrics(&store, id, vcx) == "Typed before going live"
    });
    finish(vcx, &slot, &dir);
}

/// The shown occurrences' ids in sung order.
fn sequence_ids(
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

/// The section names in sung order.
fn sung_names(editor: &Entity<ArrangementEditor>, vcx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(vcx, |editor, cx| {
        let content = editor.content(cx).expect("content");
        content
            .sequence
            .iter()
            .filter_map(|occurrence| {
                content
                    .sections
                    .iter()
                    .find(|section| section.id == occurrence.section)
                    .and_then(|section| section.label.clone())
            })
            .collect()
    })
}

fn stored_sequence_ids(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
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

#[gpui::test]
fn ac2_add_to_sequence_from_lyrics_uses_the_same_insertion_rule_and_opens_sequence(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-lyrics-add");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    assert!(
        !present(vcx, "arrangement-add-to-sequence"),
        "Verse 1 is sung, so it offers no Add to Sequence"
    );
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    saved(&editor, vcx);
    assert_eq!(
        sequence_len(&editor, vcx),
        3,
        "adding a section adds no occurrence"
    );
    // The shared rule: after the selected occurrence.
    editor.update(vcx, |editor, _| {
        editor.sequence_selection = Some(SequenceSelection::occurrence(OccurrenceId::from_entity(
            entity(81),
        )));
    });
    settle(vcx);
    click(vcx, "arrangement-add-to-sequence");
    settle(vcx);
    assert_eq!(
        sung_names(&editor, vcx),
        ["Verse 1", "Bridge", "Chorus", "Verse 1"]
    );
    let ids = sequence_ids(&editor, vcx);
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Sequence,
        "the Sequence tab opens"
    );
    assert_eq!(
        editor.read_with(vcx, |editor, _| editor.sequence_selection),
        Some(SequenceSelection::occurrence(ids[1])),
        "the new occurrence is selected"
    );
    assert_eq!(toasts(&shell, vcx), ["Added Bridge at 02 in the Sequence."]);
    saved(&editor, vcx);
    assert_eq!(stored_sequence_ids(&store, id, vcx), ids);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac2_add_to_sequence_from_lyrics_appends_when_nothing_is_selected(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-lyrics-append");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    saved(&editor, vcx);
    click(vcx, "arrangement-add-to-sequence");
    settle(vcx);
    assert_eq!(
        sung_names(&editor, vcx),
        ["Verse 1", "Chorus", "Verse 1", "Bridge"]
    );
    assert_eq!(toasts(&shell, vcx), ["Added Bridge at 04 in the Sequence."]);
    saved(&editor, vcx);
    assert_eq!(
        stored_sequence_ids(&store, id, vcx),
        sequence_ids(&editor, vcx)
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn adding_a_section_shows_no_add_to_sequence_side_effect(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-add-section-pure");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    let before = sequence_ids(&editor, vcx);
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    saved(&editor, vcx);
    settle(vcx);
    assert_eq!(sequence_ids(&editor, vcx), before, "no occurrence appeared");
    assert_eq!(
        editor.read_with(vcx, |editor, cx| editor.tab(cx)),
        TaskTab::Lyrics
    );
    assert!(
        present(vcx, "arrangement-add-to-sequence"),
        "the new section offers Add to Sequence and nothing more"
    );
    assert!(
        toasts(&shell, vcx)
            .iter()
            .all(|toast| !toast.starts_with("Added")),
        "{:?}",
        toasts(&shell, vcx)
    );
    assert_eq!(stored_sequence_ids(&store, id, vcx), before);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn editing_an_approved_section_shows_needs_review_on_that_section_only(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-review-status");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    assert!(!present(vcx, "arrangement-section-status-0"));
    assert!(!present(vcx, "arrangement-section-status-1"));
    assert!(!present(vcx, "arrangement-review-notice"));
    type_into_area(&editor, "Ave Maria, gratia plena", vcx);
    saved(&editor, vcx);
    settle(vcx);
    assert_eq!(
        stored_pagination(&store, id, vcx),
        vec![PaginationState::NeedsReview, PaginationState::Approved]
    );
    assert!(
        present(vcx, "arrangement-section-status-0"),
        "the edited row"
    );
    assert!(
        !present(vcx, "arrangement-section-status-1"),
        "the other row"
    );
    assert!(present(vcx, "arrangement-review-notice"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn approve_pagination_clears_the_notice_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let (dir, slot, store, shell, id, vcx) = parish_full(cx, "t28-approve");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    type_into_area(&editor, "Ave Maria, gratia plena", vcx);
    saved(&editor, vcx);
    settle(vcx);
    assert!(present(vcx, "arrangement-review-notice"));
    click(vcx, "button-Approve pagination");
    settle(vcx);
    assert_eq!(toasts(&shell, vcx), ["Approved Verse 1 pagination."]);
    assert!(!present(vcx, "arrangement-review-notice"));
    assert!(!present(vcx, "arrangement-section-status-0"));
    saved(&editor, vcx);
    assert_eq!(
        stored_pagination(&store, id, vcx),
        vec![PaginationState::Approved, PaginationState::Approved]
    );
    assert_eq!(stored_lyrics(&store, id, vcx), "Ave Maria, gratia plena");
    // Focus outside a text field: Undo is the editor's own.
    click(vcx, "arrangement-section-0");
    vcx.simulate_keystrokes("ctrl-z");
    settle(vcx);
    saved(&editor, vcx);
    settle(vcx);
    assert_eq!(
        stored_pagination(&store, id, vcx),
        vec![PaginationState::NeedsReview, PaginationState::Approved]
    );
    assert_eq!(stored_lyrics(&store, id, vcx), "Ave Maria, gratia plena");
    assert!(present(vcx, "arrangement-review-notice"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn approve_writes_one_revision_and_the_notice_words_are_the_plan(cx: &mut TestAppContext) {
    assert_eq!(super::review::DRAFT_LEAD, "Pagination not approved yet.");
    assert_eq!(
        super::review::DRAFT_BODY,
        "Check each slide in the Inspector, then approve."
    );
    assert_eq!(super::review::REVIEW_LEAD, "Pagination needs review.");
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-approve-revision");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    type_into_area(&editor, "Ave Maria, gratia plena", vcx);
    saved(&editor, vcx);
    settle(vcx);
    assert!(present(vcx, "arrangement-review-notice"));
    let count = |vcx: &mut VisualTestContext| {
        let future = slot
            .with(|storage| storage.revisions(domain::revisions::RevisionEntity::Arrangement(id)))
            .expect("storage is open");
        vcx.foreground_executor()
            .block_test(future)
            .expect("revisions")
            .len()
    };
    let before = count(vcx);
    click(vcx, "button-Approve pagination");
    saved(&editor, vcx);
    assert_eq!(count(vcx), before + 1, "one Approve is one revision");
    // Focus returns to the lyric area, not to the vanished button.
    let area = editor
        .read_with(vcx, |editor, _| editor.lyric_area())
        .expect("area");
    let focused = vcx.update(|window, cx| area.focus_handle(cx).is_focused(window));
    assert!(focused, "the lyric area has focus after Approve");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn an_archived_arrangement_shows_no_review_notice(cx: &mut TestAppContext) {
    let (_shell, _editor, _id, vcx) = in_memory(cx, false);
    assert!(
        present(vcx, "arrangement-review-notice"),
        "editable shows it"
    );
    let (_shell, _editor, _id, vcx) = in_memory(cx, true);
    assert!(
        !present(vcx, "arrangement-review-notice"),
        "an archived Arrangement cannot approve"
    );
}

#[gpui::test]
fn add_to_sequence_from_lyrics_lands_focus_on_the_new_row(cx: &mut TestAppContext) {
    let (dir, slot, _store, shell, id, vcx) = parish_full(cx, "t28-lyrics-add-focus");
    let editor = open_editor(&shell, id, vcx);
    settle(vcx);
    editor
        .update(vcx, |editor, cx| editor.add_section("Bridge", cx))
        .expect("added");
    saved(&editor, vcx);
    click(vcx, "arrangement-add-to-sequence");
    settle(vcx);
    let ids = sequence_ids(&editor, vcx);
    let new = *ids.last().expect("new occurrence");
    let handle = editor
        .read_with(vcx, |editor, _| editor.sequence.rows.get(&new).cloned())
        .expect("row handle");
    assert!(vcx.update(|window, _| handle.is_focused(window)));
    saved(&editor, vcx);
    finish(vcx, &slot, &dir);
}
