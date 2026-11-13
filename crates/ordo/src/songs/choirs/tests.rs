//! Behavior of the Choirs workspace through the shell that hosts it. Tests
//! that write run over a real store in a temporary directory; the rest read a
//! `LibraryStore::with_library` store, whose empty slot also stands in for a
//! storage that cannot be reached.

use std::{path::PathBuf, time::Duration};

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, SongId, Timestamp},
    song_library::{ArrangementDraft, ChoirDraft, Evidence, SectionDraft, SongDraft, SongLibrary},
};
use gpui::{
    AppContext as _, Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, px, size,
};
use storage::{NewArrangement, NewChoir, NewSection, NewSong, Storage, StorageConfig};

use super::{ChoirsWorkspace, Offer, offer};
use crate::{
    BUILD_IDENTITY,
    edit_session::SaveState,
    shell::{ShellView, rail::Destination},
    songs::{LibraryState, LibraryStore, StorageSlot},
};

pub(crate) fn entity(n: u16) -> EntityId {
    let [low, high] = n.to_le_bytes();
    let mut bytes = [high; 16];
    bytes[0] = low;
    EntityId::from_random_bytes(bytes)
}

pub(crate) fn at() -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000)
}

/// What the in-memory library below holds.
pub(crate) struct Memory {
    pub library: SongLibrary,
    pub koro: ChoirId,
    pub old: ChoirId,
}

/// Three Choirs (one archived) and three Arrangements over two Songs, two of
/// them naming "Koro Sto. Niño" alone and one shared with "Youth Choir".
pub(crate) fn memory() -> Memory {
    let mut library = SongLibrary::new();
    let koro = ChoirId::from_entity(entity(1));
    let youth = ChoirId::from_entity(entity(2));
    let old = ChoirId::from_entity(entity(3));
    for (id, name) in [
        (koro, "Koro Sto. Niño"),
        (youth, "Youth Choir"),
        (old, "Old Choir"),
    ] {
        library
            .create_choir(id, at(), ChoirDraft::named(name))
            .expect("choir");
    }
    library.archive_choir(old, at()).expect("archive");
    let mut song = |n: u16, title: &str| {
        let id = SongId::from_entity(entity(n));
        library
            .create_song(id, at(), SongDraft::titled(title))
            .expect("song");
        id
    };
    let panis = song(10, "Panis Angelicus");
    let ave = song(11, "Ave Maria");
    let mut arrangement = |n: u16, song, title: &str, language: &str, choirs: Vec<ChoirId>| {
        let section = domain::identity::SectionId::from_entity(entity(n + 1));
        library
            .create_arrangement(
                ArrangementId::from_entity(entity(n)),
                song,
                at(),
                ArrangementDraft {
                    title: Some(title.to_owned()),
                    languages: vec![language.to_owned()],
                    rendition_attribution: None,
                    notes: None,
                    occasions: Vec::new(),
                    sections: vec![SectionDraft::labeled(section, "Verse 1", "Lyric line")],
                    sequence: vec![domain::song_library::OccurrenceDraft::new(
                        domain::identity::OccurrenceId::from_entity(entity(n + 2)),
                        section,
                    )],
                },
                choirs,
            )
            .expect("arrangement");
    };
    arrangement(20, panis, "Latin SATB", "Latin", vec![koro]);
    arrangement(30, panis, "Cebuano", "Cebuano", vec![koro, youth]);
    arrangement(40, ave, "Ave Maria SATB", "Latin", vec![koro]);
    Memory { library, koro, old }
}

/// A shell on Library over `store`, sized like a projector-side laptop.
pub(crate) fn open_store(
    cx: &mut TestAppContext,
    store: Entity<LibraryStore>,
) -> (Entity<ShellView>, &mut VisualTestContext) {
    cx.update(crate::init_ui).expect("ui");
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = ShellView::new(BUILD_IDENTITY, window, cx);
        shell.set_library_store(store, window, cx);
        shell.open(Destination::Library, cx);
        window.activate_window();
        let focus = shell.focus_handle(cx);
        window.focus(&focus, cx);
        shell
    });
    cx.simulate_resize(size(px(1920.0), px(1080.0)));
    cx.run_until_parked();
    (shell, cx)
}

/// A shell over `library` with the Choirs view showing.
fn open_choirs(
    cx: &mut TestAppContext,
    library: SongLibrary,
) -> (Entity<ShellView>, &mut VisualTestContext) {
    let store = cx.new(|cx| LibraryStore::with_library(library, cx));
    let (shell, cx) = open_store(cx, store);
    click(cx, "tab-3");
    (shell, cx)
}

pub(crate) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be on screen"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

pub(crate) fn present(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

fn workspace(shell: &Entity<ShellView>, cx: &mut VisualTestContext) -> Entity<ChoirsWorkspace> {
    shell.read_with(cx, |shell, _| shell.choirs().clone())
}

/// Lets the storage worker thread and the foreground catch up until `done`.
pub(crate) fn wait(
    cx: &mut VisualTestContext,
    mut done: impl FnMut(&mut VisualTestContext) -> bool,
) {
    for _ in 0..400 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("the condition never became true");
}

/// A fresh, empty parish folder for one test.
pub(crate) fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ordo-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Opens the real store over `dir` and waits for it to finish loading.
pub(crate) fn open_parish(
    cx: &mut TestAppContext,
    dir: &std::path::Path,
) -> (StorageSlot, Entity<LibraryStore>) {
    let slot = StorageSlot::new();
    let store = cx.new(|cx| LibraryStore::open(dir.to_path_buf(), slot.clone(), cx));
    for _ in 0..400 {
        cx.run_until_parked();
        if store.read_with(cx, |store, _| *store.state() != LibraryState::Loading) {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    store.read_with(cx, |store, _| {
        assert_eq!(store.state(), &LibraryState::Ready)
    });
    (slot, store)
}

/// The Choirs and Song a seeded temporary parish holds.
pub(crate) struct Seeded {
    pub koro: ChoirId,
    pub youth: ChoirId,
    pub old: ChoirId,
}

/// Seeds "Koro Sto. Niño", "Youth Choir" and an archived "Old Choir", plus a
/// Panis Angelicus Arrangement naming the first two.
pub(crate) async fn seed_choirs(storage: &Storage) -> Seeded {
    let named = |name: &str| NewChoir {
        name: name.to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    };
    let koro = storage
        .create_choir(named("Koro Sto. Niño"))
        .await
        .expect("choir")
        .id;
    let youth = storage
        .create_choir(named("Youth Choir"))
        .await
        .expect("choir")
        .id;
    let old = storage
        .create_choir(named("Old Choir"))
        .await
        .expect("choir")
        .id;
    storage.archive_choir(old).await.expect("archive");
    let song = storage
        .create_song(NewSong {
            title: "Panis Angelicus".to_owned(),
            aliases: Vec::new(),
            original_attribution: None,
            provenance: None,
            rights_notes: None,
            liturgical_moments: Vec::new(),
            occasions: Vec::new(),
        })
        .await
        .expect("song")
        .id;
    storage
        .create_arrangement(NewArrangement {
            song,
            title: "Cebuano".to_owned(),
            languages: vec!["Cebuano".to_owned()],
            choirs: vec![koro, youth],
            rendition_attribution: None,
            occasions: Vec::new(),
            sections: vec![NewSection {
                label: Some("Verse 1".to_owned()),
                lyrics: "Tinapay sa langit".to_owned(),
            }],
            sequence: vec![0],
        })
        .await
        .expect("arrangement");
    Seeded { koro, youth, old }
}

/// A seeded temporary parish, opened for a test.
fn seeded_parish(
    cx: &mut TestAppContext,
    tag: &str,
) -> (PathBuf, StorageSlot, Entity<LibraryStore>, Seeded) {
    cx.executor().allow_parking();
    let dir = temp_dir(tag);
    let seed = Storage::spawn(StorageConfig::in_directory(&dir));
    let ids = cx.foreground_executor().block_test(seed_choirs(&seed));
    cx.foreground_executor().block_test(seed.shutdown());
    let (slot, store) = open_parish(cx, &dir);
    (dir, slot, store, ids)
}

fn finish(cx: &mut VisualTestContext, slot: &StorageSlot, dir: &std::path::Path) {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn ac8_enter_adds_a_name_only_choir_selects_it_and_it_persists(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("choirs-create");
    let (slot, store) = open_parish(cx, &dir);
    let (shell, vcx) = open_store(cx, store.clone());
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);
    // No Choirs yet: the empty list text and the optional-choirs points.
    assert!(present(vcx, "choirs-empty") && present(vcx, "choirs-detail-intro"));
    click(vcx, "choirs-add");
    assert!(present(vcx, "choir-quick-create"));
    vcx.simulate_input("Koro Sto. Niño");
    vcx.run_until_parked();
    vcx.simulate_keystrokes("enter");
    wait(vcx, |cx| ws.read_with(cx, |ws, _| ws.selected().is_some()));
    let id = ws.read_with(vcx, |ws, _| ws.selected()).expect("selected");
    store.read_with(vcx, |store, _| {
        let choirs: Vec<_> = store.library().choirs().collect();
        assert_eq!(choirs.len(), 1, "exactly one Choir was created");
        let choir = store.library().choir(id).expect("the new Choir");
        assert_eq!(choir.name(), "Koro Sto. Niño");
        assert_eq!(choir.short_label(), None);
        assert!(choir.languages().is_empty() && choir.notes().is_none());
    });
    ws.read_with(vcx, |ws, _| assert!(ws.quick_create().is_none()));
    assert!(present(vcx, "choir-name"), "the detail names the new Choir");

    // It is on disk: a second handle over the same folder reads it back.
    let outcome = vcx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    let again = Storage::spawn(StorageConfig::in_directory(&dir));
    let stored = vcx
        .foreground_executor()
        .block_test(again.choirs())
        .expect("choirs");
    assert_eq!(stored.len(), 1);
    assert_eq!(
        (stored[0].id, stored[0].name.as_str()),
        (id, "Koro Sto. Niño")
    );
    vcx.foreground_executor().block_test(again.shutdown());
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac8_escape_cancels_the_inline_add_without_creating(cx: &mut TestAppContext) {
    let (_shell, vcx) = open_choirs(cx, memory().library);
    click(vcx, "choirs-add");
    vcx.simulate_input("Nobody");
    vcx.simulate_keystrokes("escape");
    vcx.run_until_parked();
    assert!(!present(vcx, "choir-quick-create"));
}

#[gpui::test]
fn ac8_details_save_after_one_idle_second_and_on_blur(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-details");
    let (shell, vcx) = open_store(cx, store.clone());
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);
    click(vcx, "choirs-row-0");
    ws.read_with(vcx, |ws, _| assert_eq!(ws.selected(), Some(ids.koro)));

    click(vcx, "choir-short-label");
    vcx.simulate_input("KSR");
    vcx.run_until_parked();
    ws.read_with(vcx, |ws, _| {
        assert_eq!(ws.save_state(), SaveState::Saving);
        assert_eq!(ws.save_state().label(), "Saving\u{2026}");
    });
    assert!(present(vcx, "choirs-save-state"));
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store
                .library()
                .choir(ids.koro)
                .expect("choir")
                .short_label(),
            None
        );
    });
    // One idle second makes it due.
    vcx.executor().advance_clock(Duration::from_millis(1100));
    wait(vcx, |cx| {
        ws.read_with(cx, |ws, _| ws.save_state() == SaveState::Saved)
    });
    store.read_with(vcx, |store, _| {
        assert_eq!(
            store
                .library()
                .choir(ids.koro)
                .expect("choir")
                .short_label(),
            Some("KSR")
        );
    });
    assert_eq!(ws.read_with(vcx, |ws, _| ws.save_state().label()), "Saved");

    // Leaving the field writes at once, without waiting a second.
    click(vcx, "choir-notes");
    vcx.simulate_input("Bring folders");
    vcx.run_until_parked();
    click(vcx, "choir-short-label");
    wait(vcx, |cx| {
        store.read_with(cx, |store, _| {
            store.library().choir(ids.koro).expect("choir").notes() == Some("Bring folders")
        })
    });
    wait(vcx, |cx| {
        ws.read_with(cx, |ws, _| ws.save_state() == SaveState::Saved)
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_rename_reaches_the_songs_evidence(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-rename");
    let (shell, vcx) = open_store(cx, store.clone());
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);
    click(vcx, "choirs-row-0");
    click(vcx, "choir-name");
    ws.read_with(vcx, |ws, cx| {
        assert_eq!(ws.name_field().value(cx).as_ref(), "Koro Sto. Niño")
    });
    vcx.simulate_keystrokes("ctrl-a");
    vcx.simulate_input("Koro Santo Niño");
    vcx.simulate_keystrokes("enter");
    wait(vcx, |cx| {
        store.read_with(cx, |store, _| {
            store.library().choir(ids.koro).expect("choir").name() == "Koro Santo Niño"
        })
    });
    // The Songs search finds the Arrangement by the new name, and says so.
    let songs = shell.read_with(vcx, |shell, _| shell.songs().clone());
    vcx.update(|window, cx| {
        songs.update(cx, |songs, cx| {
            songs.search_field().set_value("Santo", window, cx);
            songs.refresh(cx);
        });
    });
    vcx.run_until_parked();
    songs.read_with(vcx, |songs, _| {
        let hit = songs.hits().first().expect("a hit by the new Choir name");
        assert!(matches!(
            hit.evidence.first(),
            Some(Evidence::Choir { name, .. }) if name == "Koro Santo Niño"
        ));
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_show_archived_hides_shows_and_restore_brings_a_choir_back(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-archived");
    let (shell, vcx) = open_store(cx, store.clone());
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);
    ws.read_with(vcx, |ws, _| assert!(!ws.show_archived()));
    assert!(present(vcx, "choirs-row-1") && !present(vcx, "choirs-row-2"));
    assert!(!present(vcx, "row-Old Choir"));

    click(vcx, "button-Show archived");
    assert!(present(vcx, "row-Old Choir") && present(vcx, "choirs-row-2"));
    click(vcx, "row-Old Choir");
    ws.read_with(vcx, |ws, _| assert_eq!(ws.selected(), Some(ids.old)));
    assert!(present(vcx, "choirs-archived-notice") && present(vcx, "button-Restore"));
    assert!(!present(vcx, "button-Archive"));

    click(vcx, "button-Restore");
    wait(vcx, |cx| {
        store.read_with(cx, |store, _| {
            !store.library().choir(ids.old).expect("choir").is_archived()
        })
    });
    assert!(present(vcx, "button-Archive") && !present(vcx, "choirs-archived-notice"));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_quick_create_offers_select_for_a_matching_name_even_when_archived(cx: &mut TestAppContext) {
    let memory = memory();
    let library = memory.library.clone();
    // The comparison alone: case, spacing and archived state.
    assert_eq!(
        offer(&library, "koro  STO. niño"),
        Some(Offer::Select {
            id: memory.koro,
            name: "Koro Sto. Niño".to_owned(),
            archived: false
        })
    );
    assert_eq!(
        offer(&library, "  OLD   choir "),
        Some(Offer::Select {
            id: memory.old,
            name: "Old Choir".to_owned(),
            archived: true
        })
    );
    assert_eq!(
        offer(&library, "New Choir"),
        Some(Offer::Add {
            name: "New Choir".to_owned()
        })
    );
    assert_eq!(offer(&library, "   "), None);

    // Through the list's field: Select, never a second Choir.
    let (shell, vcx) = open_choirs(cx, library);
    let ws = workspace(&shell, vcx);
    click(vcx, "choirs-add");
    vcx.simulate_input("koro  STO. niño");
    vcx.run_until_parked();
    assert!(present(vcx, "button-Select Koro Sto. Niño"));
    assert!(!present(vcx, "button-Add koro  STO. niño"));
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    ws.read_with(vcx, |ws, cx| {
        assert_eq!(ws.selected(), Some(memory.koro));
        let count = ws.store().read(cx).library().choirs().count();
        assert_eq!(count, 3, "nothing was created");
    });

    // An archived match offers Select too, and says it is archived.
    click(vcx, "choirs-add");
    vcx.simulate_input("old choir");
    vcx.run_until_parked();
    assert!(present(vcx, "button-Select Old Choir"));
    let quick = ws
        .read_with(vcx, |ws, _| ws.quick_create().cloned())
        .expect("open");
    quick.read_with(vcx, |quick, cx| {
        assert!(matches!(
            quick.current_offer(cx),
            Some(Offer::Select { archived: true, .. })
        ));
    });
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    ws.read_with(vcx, |ws, cx| {
        assert_eq!(ws.selected(), Some(memory.old));
        assert_eq!(ws.store().read(cx).library().choirs().count(), 3);
    });
}

#[gpui::test]
fn ac8_no_choirs_state_explains_that_choirs_are_optional(cx: &mut TestAppContext) {
    let (_shell, vcx) = open_choirs(cx, SongLibrary::new());
    assert!(present(vcx, "choirs-empty"), "the list says there are none");
    assert!(
        present(vcx, "choirs-detail-intro"),
        "the detail gives the three points"
    );
    assert!(
        present(vcx, "button-Add choir"),
        "Add choir is the primary action"
    );
    assert!(
        !present(vcx, "choirs-toolbar"),
        "no archived toggle without Choirs"
    );
}

#[gpui::test]
fn ac8_repertoire_rows_open_the_overview_and_back_keeps_the_selection(cx: &mut TestAppContext) {
    let memory = memory();
    let (shell, vcx) = open_choirs(cx, memory.library);
    let ws = workspace(&shell, vcx);
    click(vcx, "choirs-row-0");
    ws.read_with(vcx, |ws, _| assert_eq!(ws.selected(), Some(memory.koro)));
    // Three active Arrangements across two Songs, in Song title order.
    assert!(present(vcx, "choir-repertoire-row-0"));
    assert!(present(vcx, "choir-repertoire-row-2"));
    assert!(!present(vcx, "choir-repertoire-row-3"));
    assert!(present(vcx, "choir-repertoire-table"));

    click(vcx, "arrangement-open-0");
    assert!(present(vcx, "focused-editor") && present(vcx, "arrangement-overview"));
    click(vcx, "button-Back to Choirs");
    assert!(!present(vcx, "focused-editor") && present(vcx, "choirs-detail"));
    ws.read_with(vcx, |ws, _| assert_eq!(ws.selected(), Some(memory.koro)));
    assert!(present(vcx, "choir-repertoire-row-2"));
}

#[gpui::test]
fn ac8_a_narrow_pane_groups_arrangements_into_song_cards(cx: &mut TestAppContext) {
    let (_shell, vcx) = open_choirs(cx, memory().library);
    click(vcx, "choirs-row-0");
    assert!(present(vcx, "choir-repertoire-table") && !present(vcx, "choir-repertoire-cards"));
    vcx.simulate_resize(size(px(1100.0), px(800.0)));
    vcx.update(|window, _| window.refresh());
    vcx.run_until_parked();
    assert!(present(vcx, "choir-repertoire-cards"));
    assert!(
        present(vcx, "choir-repertoire-card-1"),
        "two Songs, two cards"
    );
    assert!(!present(vcx, "choir-repertoire-card-2"));
    assert!(
        present(vcx, "choir-repertoire-row-2"),
        "every Arrangement stays reachable"
    );
}

#[gpui::test]
fn ac8_a_storage_failure_leaves_memory_unchanged_and_says_couldnt_save(cx: &mut TestAppContext) {
    let memory = memory();
    let before = memory.library.clone();
    let (shell, vcx) = open_choirs(cx, memory.library);
    let ws = workspace(&shell, vcx);
    let store = ws.read_with(vcx, |ws, _| ws.store().clone());

    // Create: the quick-create stays open and nothing was added.
    click(vcx, "choirs-add");
    vcx.simulate_input("Brand New");
    vcx.run_until_parked();
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    ws.read_with(vcx, |ws, _| assert!(ws.quick_create().is_some()));
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &before));
    // The refusal names its reason.
    let task = store.update(vcx, |store, cx| {
        store.create_choir(ChoirDraft::named("Brand New"), cx)
    });
    let outcome = vcx.foreground_executor().block_test(task);
    assert_eq!(outcome, Err("The parish store is not open.".into()));

    // Edit: typing a label then leaving the field reports it and keeps the
    // library as it was.
    click(vcx, "button-Cancel");
    click(vcx, "choirs-row-0");
    click(vcx, "choir-short-label");
    vcx.simulate_input("KSR");
    click(vcx, "choir-notes");
    ws.read_with(vcx, |ws, _| {
        assert_eq!(ws.save_state(), SaveState::CouldntSave);
        assert_eq!(ws.save_state().label(), "Couldn't save");
    });
    assert!(present(vcx, "choirs-save-state"));
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &before));
}

/// Swaps the storage the slot holds: the old worker is stopped and a new one
/// is spawned over `config`.
fn swap_storage(cx: &mut VisualTestContext, slot: &StorageSlot, config: StorageConfig) {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    slot.put(Storage::spawn(config));
}

/// A storage whose folder is a plain file, so it opens nothing and every
/// request is answered with an error by the real storage worker.
fn broken_storage(dir: &std::path::Path) -> StorageConfig {
    let file = dir.join("not-a-folder");
    std::fs::write(&file, b"x").expect("file");
    StorageConfig::in_directory(&file)
}

/// Stops the slot's worker and reads the Choirs back through a second handle.
fn on_disk(
    cx: &mut VisualTestContext,
    slot: &StorageSlot,
    dir: &std::path::Path,
) -> Vec<storage::ChoirRecord> {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(storage::ShutdownOutcome::Clean));
    let again = Storage::spawn(StorageConfig::in_directory(dir));
    let stored = cx
        .foreground_executor()
        .block_test(again.choirs())
        .expect("choirs");
    cx.foreground_executor().block_test(again.shutdown());
    let _ = std::fs::remove_dir_all(dir);
    stored
}

/// Opens Koro's short label, types `KS`, submits that save and, while it is
/// still unanswered, types `R`. Returns the workspace.
fn type_during_a_save(
    shell: &Entity<ShellView>,
    vcx: &mut VisualTestContext,
) -> Entity<ChoirsWorkspace> {
    click(vcx, "tab-3");
    let ws = workspace(shell, vcx);
    click(vcx, "choirs-row-0");
    click(vcx, "choir-short-label");
    vcx.simulate_input("KS");
    ws.update(vcx, |ws, cx| ws.flush(cx));
    // Typed without letting the executor run, so the save stays unanswered.
    let field = ws.read_with(vcx, |ws, _| ws.short_label_field().clone());
    vcx.update(|window, cx| field.set_value("KSR", window, cx));
    ws.update(vcx, |ws, cx| ws.edited(cx));
    ws.read_with(vcx, |ws, _| {
        assert_eq!(ws.outstanding, 1, "the first save is still unanswered");
        assert!(ws.coalescer.is_pending(), "the newer text is waiting");
    });
    ws
}

#[gpui::test]
fn ac8_text_typed_during_a_save_survives_a_selection_change(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-depart-select");
    let (shell, vcx) = open_store(cx, store.clone());
    let ws = type_during_a_save(&shell, vcx);
    ws.update(vcx, |ws, cx| ws.select(ids.youth, cx));
    wait(vcx, |cx| {
        ws.read_with(cx, |ws, _| ws.outstanding == 0)
            && store.read_with(cx, |store, _| {
                store
                    .library()
                    .choir(ids.koro)
                    .expect("choir")
                    .short_label()
                    == Some("KSR")
            })
    });
    ws.read_with(vcx, |ws, _| {
        assert_eq!(ws.selected(), Some(ids.youth));
        assert_eq!(ws.save_state(), SaveState::Saved);
    });
    let stored = on_disk(vcx, &slot, &dir);
    let koro = stored
        .iter()
        .find(|choir| choir.id == ids.koro)
        .expect("koro");
    assert_eq!(koro.short_label.as_deref(), Some("KSR"));
}

#[gpui::test]
fn ac8_text_typed_during_a_save_is_written_before_an_archive(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-depart-archive");
    let (shell, vcx) = open_store(cx, store.clone());
    let ws = type_during_a_save(&shell, vcx);
    ws.update(vcx, |ws, cx| ws.archive_selected(cx));
    wait(vcx, |cx| {
        ws.read_with(cx, |ws, _| ws.outstanding == 0)
            && store.read_with(cx, |store, _| {
                store
                    .library()
                    .choir(ids.koro)
                    .expect("choir")
                    .is_archived()
            })
    });
    store.read_with(vcx, |store, _| {
        let koro = store.library().choir(ids.koro).expect("choir");
        assert_eq!(koro.short_label(), Some("KSR"));
    });
    ws.read_with(vcx, |ws, _| assert!(ws.message.is_none(), "no refusal"));
    let stored = on_disk(vcx, &slot, &dir);
    let koro = stored
        .iter()
        .find(|choir| choir.id == ids.koro)
        .expect("koro");
    assert_eq!(koro.short_label.as_deref(), Some("KSR"));
    assert!(koro.archived_at.is_some());
}

#[gpui::test]
fn ac8_a_failed_edit_is_retried_when_the_selection_changes(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-retry");
    let (shell, vcx) = open_store(cx, store.clone());
    swap_storage(vcx, &slot, broken_storage(&dir));
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);
    click(vcx, "choirs-row-0");
    click(vcx, "choir-short-label");
    vcx.simulate_input("KSR");
    click(vcx, "choir-notes");
    wait(vcx, |cx| {
        ws.read_with(cx, |ws, _| ws.save_state() == SaveState::CouldntSave)
    });

    // The storage comes back; leaving the Choir retries the failed edit.
    swap_storage(vcx, &slot, StorageConfig::in_directory(&dir));
    ws.update(vcx, |ws, cx| ws.select(ids.youth, cx));
    wait(vcx, |cx| {
        store.read_with(cx, |store, _| {
            store
                .library()
                .choir(ids.koro)
                .expect("choir")
                .short_label()
                == Some("KSR")
        })
    });
    ws.read_with(vcx, |ws, _| assert_eq!(ws.save_state(), SaveState::Saved));
    let stored = on_disk(vcx, &slot, &dir);
    let koro = stored
        .iter()
        .find(|choir| choir.id == ids.koro)
        .expect("koro");
    assert_eq!(koro.short_label.as_deref(), Some("KSR"));
}

#[gpui::test]
fn ac8_a_failing_storage_write_leaves_memory_unchanged_and_says_couldnt_save(
    cx: &mut TestAppContext,
) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-write-fails");
    let (shell, vcx) = open_store(cx, store.clone());
    let before = store.read_with(vcx, |store, _| store.library().clone());
    swap_storage(vcx, &slot, broken_storage(&dir));
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);

    // The storage worker itself answers with an error, not an empty slot.
    let task = store.update(vcx, |store, cx| {
        store.create_choir(ChoirDraft::named("Brand New"), cx)
    });
    let outcome = vcx.foreground_executor().block_test(task);
    let message = outcome.expect_err("the write fails");
    assert_ne!(message.as_ref(), "The parish store is not open.");
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &before));

    // A typed edit fails the same way, and leaving the Choir retries it and
    // says Couldn't save again.
    click(vcx, "choirs-row-0");
    click(vcx, "choir-short-label");
    vcx.simulate_input("KSR");
    click(vcx, "choir-notes");
    wait(vcx, |cx| {
        ws.read_with(cx, |ws, _| ws.save_state() == SaveState::CouldntSave)
    });
    ws.update(vcx, |ws, cx| ws.select(ids.youth, cx));
    wait(vcx, |cx| ws.read_with(cx, |ws, _| ws.outstanding == 0));
    ws.read_with(vcx, |ws, _| {
        assert_eq!(ws.save_state(), SaveState::CouldntSave);
        assert_eq!(ws.save_state().label(), "Couldn't save");
    });
    store.read_with(vcx, |store, _| assert_eq!(store.library(), &before));
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_the_store_refuses_a_duplicate_name_even_for_an_archived_choir(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-duplicate");
    let (_shell, vcx) = open_store(cx, store.clone());
    for typed in ["koro  sto. niño", "OLD CHOIR"] {
        let task = store.update(vcx, |store, cx| {
            store.create_choir(ChoirDraft::named(typed), cx)
        });
        let outcome = vcx.foreground_executor().block_test(task);
        let message = outcome.expect_err("a duplicate is refused");
        assert!(message.contains("already exists"), "{message}");
    }
    store.read_with(vcx, |store, _| {
        assert_eq!(store.library().choirs().count(), 3);
        assert!(store.library().choir(ids.old).expect("old").is_archived());
    });
    finish(vcx, &slot, &dir);
}

#[gpui::test]
fn ac8_selecting_an_archived_match_turns_show_archived_on(cx: &mut TestAppContext) {
    let (dir, slot, store, ids) = seeded_parish(cx, "choirs-archived-match");
    let (shell, vcx) = open_store(cx, store);
    click(vcx, "tab-3");
    let ws = workspace(&shell, vcx);
    ws.read_with(vcx, |ws, _| assert!(!ws.show_archived()));
    click(vcx, "choirs-add");
    vcx.simulate_input("old choir");
    vcx.run_until_parked();
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    ws.read_with(vcx, |ws, _| {
        assert_eq!(ws.selected(), Some(ids.old));
        assert!(
            ws.show_archived(),
            "the selected Choir is visible in the list"
        );
    });
    assert!(present(vcx, "choirs-row-2"));
    finish(vcx, &slot, &dir);
}
