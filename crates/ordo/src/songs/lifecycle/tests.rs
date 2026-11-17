//! Lifecycle UI tests: archive, restore and deletion through the real
//! parish store, driven by clicks on the real screens.

use gpui::{Entity, TestAppContext, VisualTestContext};

use domain::{
    identity::{ArrangementId, EntityId, HistorySessionId, SongId},
    song_library::{
        DurableReference, RecordRef, arrangement_lifecycle_actions, song_lifecycle_actions,
    },
};

use crate::{
    history,
    shell::ShellView,
    songs::{
        LibraryStore, SongsWorkspace, StorageSlot,
        choirs::tests::{open_store, present, wait},
        library::detail::arrangement_badge,
        lifecycle::{
            LifecycleTarget, action_label, archive_impact, blocked_description, erase_summary,
            menu::trigger_label,
        },
        lyrics::tests::{dialog, toasts},
        new_song::tests::{
            Seeded, click, find_title, finish, parish, parish_with, settle, songs_of,
        },
    },
};

type Rig<'a> = (
    std::path::PathBuf,
    StorageSlot,
    Entity<LibraryStore>,
    Seeded,
    Entity<ShellView>,
    &'a mut VisualTestContext,
);

/// The seeded parish with the shell over it, on Library.
fn rig<'a>(cx: &'a mut TestAppContext, tag: &str) -> Rig<'a> {
    let (dir, slot, store, ids) = parish(cx, tag);
    let (shell, vcx) = open_store(cx, store.clone());
    (dir, slot, store, ids, shell, vcx)
}

fn select(shell: &Entity<ShellView>, song: SongId, vcx: &mut VisualTestContext) {
    let songs = songs_of(shell, vcx);
    songs.update(vcx, |songs, cx| songs.select(song, cx));
    settle(vcx);
}

fn arrangements(
    store: &Entity<LibraryStore>,
    song: SongId,
    vcx: &mut VisualTestContext,
) -> Vec<ArrangementId> {
    store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangements_for_song(song)
            .expect("arrangements")
            .into_iter()
            .map(|arrangement| arrangement.id())
            .collect()
    })
}

fn song_archived(store: &Entity<LibraryStore>, song: SongId, vcx: &mut VisualTestContext) -> bool {
    store.read_with(vcx, |store, _| {
        store.library().song(song).expect("song").is_archived()
    })
}

fn arrangement_archived(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> bool {
    store.read_with(vcx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("arrangement")
            .is_archived()
    })
}

/// Archives a Song straight through the store, as another part of the app
/// would.
fn archive_directly(store: &Entity<LibraryStore>, song: SongId, vcx: &mut VisualTestContext) {
    let task = store.update(vcx, |store, cx| store.archive_song(song, cx));
    vcx.foreground_executor()
        .block_test(task)
        .expect("archived");
    settle(vcx);
}

/// Opens the overflow Menu and chooses its row `at`.
fn choose(trigger: &'static str, at: usize, vcx: &mut VisualTestContext) {
    click(vcx, trigger);
    let row: &'static str = Box::leak(format!("choice-{at}").into_boxed_str());
    click(vcx, row);
}

fn wait_toast(shell: &Entity<ShellView>, text: &str, vcx: &mut VisualTestContext) {
    wait(vcx, |vcx| {
        toasts(shell, vcx).iter().any(|toast| toast == text)
    });
    settle(vcx);
}

fn focused(handle: &gpui::FocusHandle, vcx: &mut VisualTestContext) -> bool {
    vcx.update(|window, _| handle.is_focused(window))
}

fn workspace_focus(
    songs: &Entity<SongsWorkspace>,
    pick: impl Fn(&SongsWorkspace) -> gpui::FocusHandle,
    vcx: &mut VisualTestContext,
) -> bool {
    let handle = songs.read_with(vcx, |songs, _| pick(songs));
    focused(&handle, vcx)
}

const ARCHIVED_TINAPAY: &str = "Archived Tinapay sa Langit. Mass plans that use it keep it.";

/// Archives the selected Song through its overflow and the dialog.
fn archive_selected_song(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) {
    choose("button-More Song actions", 0, vcx);
    click(vcx, "button-Archive");
    wait(vcx, |vcx| dialog(shell, vcx).is_none());
    settle(vcx);
}

#[gpui::test]
fn ac4_archiving_a_song_from_its_overflow_keeps_child_states(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-song");
    select(&shell, ids.exact, vcx);
    let children = arrangements(&store, ids.exact, vcx);
    assert_eq!(children.len(), 2);
    archive_selected_song(&shell, vcx);

    assert!(song_archived(&store, ids.exact, vcx));
    for child in children {
        assert!(
            !arrangement_archived(&store, child, vcx),
            "child state is kept"
        );
    }
    assert_eq!(toasts(&shell, vcx), vec![ARCHIVED_TINAPAY.to_owned()]);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac4_archiving_an_arrangement_leaves_its_song_active(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-arr");
    let id = arrangements(&store, ids.exact, vcx)[0];
    shell.update(vcx, |shell, cx| shell.open_arrangement(id, cx));
    settle(vcx);
    choose("button-More Arrangement actions", 0, vcx);
    click(vcx, "button-Archive");
    wait(vcx, |vcx| dialog(&shell, vcx).is_none());
    settle(vcx);

    assert!(arrangement_archived(&store, id, vcx));
    assert!(!song_archived(&store, ids.exact, vcx));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac7_archive_dialog_names_the_impact_and_has_only_cancel_and_archive(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-dialog");
    select(&shell, ids.exact, vcx);
    choose("button-More Song actions", 0, vcx);
    assert_eq!(
        dialog(&shell, vcx).as_deref(),
        Some("Archive Tinapay sa Langit?")
    );
    assert!(present(vcx, "button-Cancel"));
    assert!(present(vcx, "button-Archive"));
    assert!(!present(vcx, "button-Delete permanently"));
    let impact = vcx.update(|_, cx| {
        archive_impact(
            &store,
            crate::songs::lifecycle::LifecycleTarget::Song(ids.exact),
            cx,
        )
    });
    assert_eq!(
        impact,
        "Its 2 Arrangements leave planning with it and keep their own archive state. \
         Existing Mass plans and past Presentation snapshots don\u{2019}t change."
    );
    click(vcx, "button-Cancel");
    assert!(
        !song_archived(&store, ids.exact, vcx),
        "Cancel changes nothing"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac7_archive_toast_offers_undo_that_restores(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-undo");
    select(&shell, ids.exact, vcx);
    archive_selected_song(&shell, vcx);
    assert!(song_archived(&store, ids.exact, vcx));
    click(vcx, "button-Undo");
    wait(vcx, |vcx| !song_archived(&store, ids.exact, vcx));
    wait_toast(&shell, "Restored Tinapay sa Langit.", vcx);
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_archiving_the_selected_song_selects_the_next_visible_active_result(cx: &mut TestAppContext) {
    let (dir, slot, _store, _ids, shell, vcx) = rig(cx, "life-next");
    let songs = songs_of(&shell, vcx);
    let hits: Vec<SongId> = songs.read_with(vcx, |songs, _| {
        songs.hits().iter().map(|h| h.song).collect()
    });
    select(&shell, hits[0], vcx);
    archive_selected_song(&shell, vcx);
    assert_eq!(
        songs.read_with(vcx, |songs, _| songs.selected()),
        Some(hits[1])
    );
    assert_eq!(
        songs.read_with(vcx, |songs, _| songs.focus_song()),
        Some(hits[1])
    );
    assert!(workspace_focus(&songs, |s| s.row_focus().clone(), vcx));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_archiving_the_last_listed_song_selects_the_previous(cx: &mut TestAppContext) {
    let (dir, slot, _store, _ids, shell, vcx) = rig(cx, "life-prev");
    let songs = songs_of(&shell, vcx);
    let hits: Vec<SongId> = songs.read_with(vcx, |songs, _| {
        songs.hits().iter().map(|h| h.song).collect()
    });
    let last = hits.len() - 1;
    select(&shell, hits[last], vcx);
    archive_selected_song(&shell, vcx);
    assert_eq!(
        songs.read_with(vcx, |songs, _| songs.selected()),
        Some(hits[last - 1])
    );
    assert!(workspace_focus(&songs, |s| s.row_focus().clone(), vcx));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_archiving_the_only_active_song_focuses_the_empty_state_and_keeps_undo(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-only");
    archive_directly(&store, ids.near, vcx);
    archive_directly(&store, ids.unrelated, vcx);
    select(&shell, ids.exact, vcx);
    archive_selected_song(&shell, vcx);
    let songs = songs_of(&shell, vcx);
    assert_eq!(songs.read_with(vcx, |songs, _| songs.selected()), None);
    assert!(present(vcx, "songs-archived-only"));
    assert!(workspace_focus(&songs, |s| s.empty_focus().clone(), vcx));
    assert!(toasts(&shell, vcx).contains(&ARCHIVED_TINAPAY.to_owned()));
    assert!(present(vcx, "button-Undo"), "Undo stays offered");
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_with_include_archived_the_archived_song_stays_selected_without_an_active_neighbour(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-include");
    archive_directly(&store, ids.near, vcx);
    archive_directly(&store, ids.unrelated, vcx);
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    select(&shell, ids.exact, vcx);
    archive_selected_song(&shell, vcx);
    assert_eq!(
        songs.read_with(vcx, |songs, _| songs.selected()),
        Some(ids.exact)
    );
    assert!(song_archived(&store, ids.exact, vcx));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_archiving_an_arrangement_returns_to_its_song(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-return");
    select(&shell, ids.exact, vcx);
    let id = arrangements(&store, ids.exact, vcx)[0];
    shell.update(vcx, |shell, cx| shell.open_arrangement(id, cx));
    settle(vcx);
    choose("button-More Arrangement actions", 0, vcx);
    click(vcx, "button-Archive");
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.editing().is_none())
    });
    settle(vcx);
    let songs = songs_of(&shell, vcx);
    assert_eq!(
        songs.read_with(vcx, |songs, _| songs.selected()),
        Some(ids.exact)
    );
    assert!(workspace_focus(
        &songs,
        |s| s.arrangement_focus().clone(),
        vcx
    ));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn archive_refused_by_storage_shows_operator_copy_and_changes_nothing(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-refused");
    select(&shell, ids.exact, vcx);
    choose("button-More Song actions", 0, vcx);
    archive_directly(&store, ids.exact, vcx);
    click(vcx, "button-Archive");
    wait_toast(&shell, "It\u{2019}s already archived.", vcx);
    assert_eq!(
        toasts(&shell, vcx),
        vec!["It\u{2019}s already archived.".to_owned()]
    );
    assert!(song_archived(&store, ids.exact, vcx));
    finish(vcx, &slot, &dir);
}

fn archive_arrangement_directly(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) {
    let task = store.update(vcx, |store, cx| store.archive_arrangement(id, cx));
    vcx.foreground_executor()
        .block_test(task)
        .expect("archived");
    settle(vcx);
}

fn editor_read_only(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) -> bool {
    shell.read_with(vcx, |shell, cx| {
        shell
            .arrangement_editor()
            .expect("editor")
            .read(cx)
            .is_read_only()
    })
}

fn open(shell: &Entity<ShellView>, id: ArrangementId, vcx: &mut VisualTestContext) {
    shell.update(vcx, |shell, cx| shell.open_arrangement(id, cx));
    settle(vcx);
}

#[gpui::test]
fn ac5_archived_arrangement_editor_is_inspect_only_with_restore(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-banner");
    let id = arrangements(&store, ids.exact, vcx)[1];
    archive_arrangement_directly(&store, id, vcx);
    open(&shell, id, vcx);
    assert!(editor_read_only(&shell, vcx));
    assert!(present(vcx, "arrangement-archived-notice"));
    assert!(present(vcx, "button-Restore"));
    assert!(
        !present(vcx, "button-Arrangement details"),
        "no editing controls"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_restore_from_the_banner_makes_the_editor_editable(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-restore");
    let id = arrangements(&store, ids.exact, vcx)[1];
    archive_arrangement_directly(&store, id, vcx);
    open(&shell, id, vcx);
    click(vcx, "button-Restore");
    wait(vcx, |vcx| {
        !arrangement_archived(&store, id, vcx) && !editor_read_only(&shell, vcx)
    });
    settle(vcx);
    assert!(!present(vcx, "arrangement-archived-notice"));
    assert!(present(vcx, "button-Arrangement details"));
    let label = store.read_with(vcx, |store, _| {
        store.library().arrangement_label(id).expect("label")
    });
    assert!(toasts(&shell, vcx).contains(&format!("Restored {label}.")));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_archived_song_detail_hides_new_arrangement_and_offers_restore(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-song-banner");
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    select(&shell, ids.archived, vcx);
    assert!(present(vcx, "songs-archived-notice"));
    assert!(!present(vcx, "button-Add Arrangement"));
    click(vcx, "button-Restore");
    wait(vcx, |vcx| !song_archived(&store, ids.archived, vcx));
    settle(vcx);
    assert!(!present(vcx, "songs-archived-notice"));
    assert!(present(vcx, "button-Add Arrangement"));
    assert_eq!(
        toasts(&shell, vcx),
        vec!["Restored Tinapay-sa-langit.".to_owned()]
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_include_archived_lists_archived_songs_and_arrangements(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-include-list");
    let songs = songs_of(&shell, vcx);
    let listed = |songs: &Entity<SongsWorkspace>, vcx: &mut VisualTestContext| {
        songs.read_with(vcx, |songs, _| {
            songs.hits().iter().any(|hit| hit.song == ids.archived)
        })
    };
    assert!(!listed(&songs, vcx), "hidden until Include archived");
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    settle(vcx);
    assert!(listed(&songs, vcx));
    let id = arrangements(&store, ids.exact, vcx)[1];
    archive_arrangement_directly(&store, id, vcx);
    select(&shell, ids.exact, vcx);
    assert!(
        present(vcx, "button-Inspect"),
        "the archived Arrangement is still listed"
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_history_of_an_archived_arrangement_is_viewable(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-history");
    let id = arrangements(&store, ids.exact, vcx)[1];
    archive_arrangement_directly(&store, id, vcx);
    open(&shell, id, vcx);
    click(vcx, "button-History");
    wait(vcx, |vcx| {
        dialog(&shell, vcx).as_deref() == Some(history::TITLE)
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac5_suppressed_arrangement_shows_available_when_song_is_restored(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-suppressed");
    assert_eq!(
        arrangement_badge(false, true),
        Some("Available when Song is restored")
    );
    assert_eq!(arrangement_badge(true, true), Some("Archived"));
    assert_eq!(arrangement_badge(true, false), Some("Archived"));
    assert_eq!(arrangement_badge(false, false), None);
    // Restoring the Song from the banner of its suppressed Arrangement makes
    // the Arrangement editable again.
    let id = arrangements(&store, ids.archived, vcx)[0];
    open(&shell, id, vcx);
    assert!(editor_read_only(&shell, vcx));
    click(vcx, "button-Restore");
    wait(vcx, |vcx| {
        !song_archived(&store, ids.archived, vcx) && !editor_read_only(&shell, vcx)
    });
    assert!(!arrangement_archived(&store, id, vcx));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_overflow_offers_delete_permanently_only_when_archived(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-offers");
    let songs = songs_of(&shell, vcx);
    select(&shell, ids.exact, vcx);
    click(vcx, "button-More Song actions");
    assert!(present(vcx, "choice-0"));
    assert!(
        !present(vcx, "choice-1"),
        "an active Song offers Archive only"
    );
    click(vcx, "button-More Song actions");
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    select(&shell, ids.archived, vcx);
    click(vcx, "button-More Song actions");
    assert!(
        present(vcx, "choice-1"),
        "an archived Song offers Restore and Delete permanently"
    );
    assert!(!present(vcx, "choice-2"));
    let _ = store;
    finish(vcx, &slot, &dir);
}

fn plant(slot: &StorageSlot, target: RecordRef, byte: u8, vcx: &mut VisualTestContext) {
    let reference = DurableReference::MassPlan(EntityId::from_random_bytes([byte; 16]));
    let future = slot
        .with(|storage| storage.plant_reference(target, reference))
        .expect("storage is open");
    vcx.foreground_executor()
        .block_test(future)
        .expect("planted");
}

fn title_of(dialog: Option<String>) -> String {
    dialog.expect("a dialog is open")
}

/// Waits for the deletion dialog to show something other than its loading
/// line, then lets it settle.
fn wait_reviewed(shell: &Entity<ShellView>, vcx: &mut VisualTestContext) {
    wait(vcx, |vcx| {
        dialog(shell, vcx).is_some() && !present(vcx, "lifecycle-checking")
    });
    settle(vcx);
}

fn label_of(
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) -> String {
    store.read_with(vcx, |store, _| {
        store.library().arrangement_label(id).expect("label")
    })
}

/// An Arrangement archived and open in the editor, with Delete permanently
/// chosen from its overflow.
fn delete_from_editor(
    store: &Entity<LibraryStore>,
    shell: &Entity<ShellView>,
    id: ArrangementId,
    vcx: &mut VisualTestContext,
) {
    archive_arrangement_directly(store, id, vcx);
    open(shell, id, vcx);
    choose("button-More Arrangement actions", 1, vcx);
    wait_reviewed(shell, vcx);
}

#[gpui::test]
fn ac6_delete_dialog_lists_blockers_and_offers_no_delete(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-blocked");
    let id = arrangements(&store, ids.exact, vcx)[1];
    plant(&slot, RecordRef::Arrangement(id), 0x11, vcx);
    let label = label_of(&store, id, vcx);
    delete_from_editor(&store, &shell, id, vcx);
    assert_eq!(
        title_of(dialog(&shell, vcx)),
        format!("Delete {label} permanently?")
    );
    assert!(present(vcx, "lifecycle-group-Mass plans"));
    assert!(!present(vcx, "button-Delete permanently"));
    assert!(!present(vcx, "button-Cancel"));
    assert_eq!(
        blocked_description("Cebuano SATB"),
        "Cebuano SATB can\u{2019}t be deleted while these still use it. Archived, it stays out of planning."
    );
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_song_delete_lists_each_child_arrangement_with_inspect(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-children");
    archive_directly(&store, ids.exact, vcx);
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    select(&shell, ids.exact, vcx);
    choose("button-More Song actions", 1, vcx);
    wait_reviewed(&shell, vcx);
    assert!(present(vcx, "lifecycle-group-Arrangements"));
    assert!(present(vcx, "lifecycle-inspect-0"));
    assert!(present(vcx, "lifecycle-inspect-1"));
    assert!(!present(vcx, "lifecycle-inspect-2"), "one row per child");
    assert!(!present(vcx, "button-Delete permanently"));
    click(vcx, "lifecycle-inspect-0");
    wait(vcx, |vcx| {
        shell.read_with(vcx, |shell, _| shell.editing().is_some())
    });
    assert!(
        dialog(&shell, vcx).is_none(),
        "Inspect closes the dialog by id"
    );
    assert!(song_archived(&store, ids.exact, vcx));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_safe_delete_dialog_states_what_is_erased_and_deletes(cx: &mut TestAppContext) {
    let (dir, slot, store, _ids) = parish_with(cx, "life-safe", 0, true);
    let (hollow, _) = find_title(&store, "Hollow", cx).expect("hollow");
    let (shell, vcx) = open_store(cx, store.clone());
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    select(&shell, hollow, vcx);
    choose("button-More Song actions", 1, vcx);
    wait_reviewed(&shell, vcx);
    assert_eq!(title_of(dialog(&shell, vcx)), "Delete Hollow permanently?");
    assert!(present(vcx, "button-Cancel"));
    assert!(present(vcx, "button-Delete permanently"));
    assert!(!present(vcx, "lifecycle-blockers"));
    assert_eq!(
        erase_summary(LifecycleTarget::Song(hollow), "Hollow", 0, 3),
        "This erases Hollow and all 3 revisions of its History. This can\u{2019}t be undone."
    );
    click(vcx, "button-Delete permanently");
    wait_toast(&shell, "Deleted Hollow permanently.", vcx);
    assert!(store.read_with(vcx, |store, _| store.library().song(hollow).is_err()));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac6_a_reference_planted_after_review_blocks_the_commit(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-late");
    let id = arrangements(&store, ids.exact, vcx)[1];
    delete_from_editor(&store, &shell, id, vcx);
    assert!(
        present(vcx, "button-Delete permanently"),
        "safe at review time"
    );
    plant(&slot, RecordRef::Arrangement(id), 0x22, vcx);
    click(vcx, "button-Delete permanently");
    wait(vcx, |vcx| present(vcx, "lifecycle-group-Mass plans"));
    assert!(!present(vcx, "button-Delete permanently"));
    assert!(store.read_with(vcx, |store, _| store.library().arrangement(id).is_ok()));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac7_delete_toast_has_no_undo(cx: &mut TestAppContext) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-delete-toast");
    let id = arrangements(&store, ids.exact, vcx)[1];
    let label = label_of(&store, id, vcx);
    delete_from_editor(&store, &shell, id, vcx);
    click(vcx, "button-Delete permanently");
    wait_toast(&shell, &format!("Deleted {label} permanently."), vcx);
    assert_eq!(
        toasts(&shell, vcx),
        vec![format!("Deleted {label} permanently.")]
    );
    assert!(!present(vcx, "button-Undo"));
    assert!(
        shell.read_with(vcx, |shell, _| shell.editing().is_none()),
        "back on the Song"
    );
    finish(vcx, &slot, &dir);
}

/// The words nothing in the library offers (T2.9 AC9, Decision 167).
const FORBIDDEN_WORDS: [&str; 4] = ["merge", "survivor", "redirect", "revers"];

fn assert_no_forbidden_words(names: &[String], surface: &str) {
    assert!(!names.is_empty(), "{surface} offered nothing to check");
    for name in names {
        let lowered = name.to_lowercase();
        for word in FORBIDDEN_WORDS {
            assert!(
                !lowered.contains(word),
                "{surface} offers {name:?}, which carries {word:?}"
            );
        }
    }
}

/// Opens the overflow Menu on screen, asserts it holds exactly the rows the
/// domain offers `target` (no more, no fewer) and returns every name it
/// shows: the trigger's and each row's. The pinned test platform exposes no
/// accessibility tree, so the rows are counted on screen and named from the
/// same function the Menu is built with.
fn overflow_names(
    store: &Entity<LibraryStore>,
    target: LifecycleTarget,
    trigger: &'static str,
    vcx: &mut VisualTestContext,
) -> Vec<String> {
    let actions = store.read_with(vcx, |store, _| match target {
        LifecycleTarget::Song(id) => {
            song_lifecycle_actions(store.library().song(id).expect("song"))
        }
        LifecycleTarget::Arrangement(id) => {
            arrangement_lifecycle_actions(store.library().arrangement(id).expect("arrangement"))
        }
    });
    click(vcx, trigger);
    let row = |at: usize| -> &'static str { Box::leak(format!("choice-{at}").into_boxed_str()) };
    for at in 0..actions.len() {
        assert!(present(vcx, row(at)), "{trigger}: row {at} is on screen");
    }
    assert!(
        !present(vcx, row(actions.len())),
        "{trigger}: no row beyond the domain's {} actions",
        actions.len()
    );
    click(vcx, trigger);
    let mut names = vec![trigger_label(target).to_owned()];
    names.extend(
        actions
            .iter()
            .map(|action| action_label(target, *action).to_owned()),
    );
    names
}

/// Two edits in two History sessions, so History has a revision older than
/// the head to compare.
fn two_sessions(store: &Entity<LibraryStore>, id: ArrangementId, vcx: &mut VisualTestContext) {
    for (byte, words) in [(0xa1_u8, "First edit"), (0xb2, "Second edit")] {
        let mut lyrics = store.read_with(vcx, |store, _| {
            store
                .library()
                .arrangement(id)
                .expect("arrangement")
                .lyrics()
        });
        lyrics.sections[0].lyrics = words.to_owned();
        let session = HistorySessionId::from_entity(EntityId::from_random_bytes([byte; 16]));
        let task = store.update(vcx, |store, cx| {
            store.save_arrangement_lyrics(id, lyrics, session, cx)
        });
        assert_eq!(vcx.foreground_executor().block_test(task), Ok(id));
    }
    settle(vcx);
}

#[gpui::test]
fn ac9_overflow_menus_and_history_offer_no_merge_survivor_redirect_or_reversal(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, ids, shell, vcx) = rig(cx, "life-ac9");
    let mut names: Vec<String> = Vec::new();

    // The Song overflow: active, then archived (Include archived).
    select(&shell, ids.exact, vcx);
    let song = overflow_names(
        &store,
        LifecycleTarget::Song(ids.exact),
        "button-More Song actions",
        vcx,
    );
    assert_eq!(song, ["More Song actions", "Archive song"]);
    names.extend(song);
    let songs = songs_of(&shell, vcx);
    songs.update(vcx, |songs, cx| songs.set_include_archived(true, cx));
    select(&shell, ids.archived, vcx);
    let archived = overflow_names(
        &store,
        LifecycleTarget::Song(ids.archived),
        "button-More Song actions",
        vcx,
    );
    assert_eq!(
        archived,
        ["More Song actions", "Restore", "Delete permanently\u{2026}"]
    );
    names.extend(archived);

    // The Arrangement overflow: active, archived, and suppressed by its Song.
    let active = arrangements(&store, ids.exact, vcx)[0];
    let retired = arrangements(&store, ids.exact, vcx)[1];
    open(&shell, active, vcx);
    let row = overflow_names(
        &store,
        LifecycleTarget::Arrangement(active),
        "button-More Arrangement actions",
        vcx,
    );
    assert_eq!(row, ["More Arrangement actions", "Archive arrangement"]);
    names.extend(row);
    two_sessions(&store, active, vcx);
    archive_arrangement_directly(&store, retired, vcx);
    open(&shell, retired, vcx);
    let row = overflow_names(
        &store,
        LifecycleTarget::Arrangement(retired),
        "button-More Arrangement actions",
        vcx,
    );
    assert_eq!(
        row,
        [
            "More Arrangement actions",
            "Restore",
            "Delete permanently\u{2026}"
        ]
    );
    names.extend(row);
    let suppressed = arrangements(&store, ids.archived, vcx)[0];
    open(&shell, suppressed, vcx);
    names.extend(overflow_names(
        &store,
        LifecycleTarget::Arrangement(suppressed),
        "button-More Arrangement actions",
        vcx,
    ));

    // History, a comparison and its confirmation, on the edited Arrangement.
    open(&shell, active, vcx);
    for control in ["button-History", "button-Arrangement details"] {
        assert!(present(vcx, control), "{control} is the header's");
        names.push(control.trim_start_matches("button-").to_owned());
    }
    click(vcx, "button-History");
    wait(vcx, |vcx| {
        dialog(&shell, vcx).as_deref() == Some(history::TITLE) && present(vcx, "history-session-1")
    });
    settle(vcx);
    names.push(history::TITLE.to_owned());
    names.push("Close History".to_owned());
    names.push("Current".to_owned());
    names.push(
        "Every saved change is kept. Select a revision to compare it with the current Arrangement."
            .to_owned(),
    );
    click(vcx, "history-session-1");
    click(vcx, "history-revision-1-0");
    assert_eq!(dialog(&shell, vcx).as_deref(), Some(history::COMPARE_TITLE));
    names.push(history::COMPARE_TITLE.to_owned());
    // The comparison's four tabs and its footer.
    for (at, tab) in ["Overview", "Lyrics", "Sequence", "Slides"]
        .into_iter()
        .enumerate()
    {
        let selector: &'static str = Box::leak(format!("tab-{at}").into_boxed_str());
        assert!(present(vcx, selector), "tab {tab}");
        names.push(tab.to_owned());
    }
    assert!(!present(vcx, "tab-4"), "exactly four tabs");
    for control in [history::BACK_LABEL, history::RESTORE_THIS] {
        let selector: &'static str = Box::leak(format!("button-{control}").into_boxed_str());
        assert!(present(vcx, selector), "{control}");
        names.push(control.to_owned());
    }
    click(vcx, "button-Restore this revision");
    for control in [history::KEEP, "Restore entire Arrangement"] {
        let selector: &'static str = Box::leak(format!("button-{control}").into_boxed_str());
        assert!(present(vcx, selector), "{control}");
        names.push(control.to_owned());
    }
    assert_no_forbidden_words(&names, "the overflow menus and History");
    finish(vcx, &slot, &dir);
}
