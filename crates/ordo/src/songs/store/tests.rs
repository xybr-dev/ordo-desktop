//! Behavior of `LibraryStore::create_song` over a real store in a temporary
//! parish folder: what reaches disk, what reaches memory, and in what order.

use std::{cell::RefCell, path::Path, rc::Rc};

use domain::{
    identity::SongId,
    song_library::{ChoirDraft, SongDraft, SongLibrary},
};
use gpui::{Entity, SharedString, TestAppContext};
use storage::{ShutdownOutcome, Storage, StorageConfig};

use crate::songs::{
    LibraryEvent, LibraryStore, StorageSlot,
    choirs::tests::{open_parish, temp_dir},
};

/// Counts the `Changed` events the store emits from now on.
fn count_changes(cx: &mut TestAppContext, store: &Entity<LibraryStore>) -> Rc<RefCell<usize>> {
    let seen = Rc::new(RefCell::new(0));
    let counter = seen.clone();
    cx.update(|cx| {
        cx.subscribe(store, move |_, _: &LibraryEvent, _| {
            *counter.borrow_mut() += 1;
        })
        .detach();
    });
    seen
}

fn create(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    draft: SongDraft,
) -> Result<SongId, SharedString> {
    let task = store.update(cx, |store, cx| store.create_song(draft, cx));
    cx.foreground_executor().block_test(task)
}

/// Stops the slot's worker cleanly, then opens the same folder again.
fn reopen(
    cx: &mut TestAppContext,
    slot: &StorageSlot,
    dir: &Path,
) -> (StorageSlot, Entity<LibraryStore>) {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    open_parish(cx, dir)
}

fn finish(cx: &mut TestAppContext, slot: &StorageSlot, dir: &Path) {
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    let _ = std::fs::remove_dir_all(dir);
}

fn song_count(cx: &mut TestAppContext, store: &Entity<LibraryStore>) -> usize {
    store.read_with(cx, |store, _| store.library().songs().count())
}

#[gpui::test]
fn ac6_store_a_title_only_song_is_created_and_survives_reopen(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-song-title-only");
    let (slot, store) = open_parish(cx, &dir);
    let changes = count_changes(cx, &store);

    let id = create(cx, &store, SongDraft::titled("Panis Angelicus")).expect("created");
    store.read_with(cx, |store, _| {
        let song = store.library().song(id).expect("in memory");
        assert_eq!(song.title(), "Panis Angelicus");
        assert!(
            store
                .library()
                .arrangements_for_song(id)
                .expect("song")
                .is_empty()
        );
    });
    assert_eq!(*changes.borrow(), 1, "one Changed for one create");

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        let song = store.library().song(id).expect("same id after reopen");
        assert_eq!(song.title(), "Panis Angelicus");
        assert!(
            store
                .library()
                .arrangements_for_song(id)
                .expect("song")
                .is_empty()
        );
        assert_eq!(store.library().songs().count(), 1);
    });
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_full_optional_metadata_round_trips_through_reopen(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-song-metadata");
    let (slot, store) = open_parish(cx, &dir);
    let draft = SongDraft {
        title: "Panis Angelicus".into(),
        aliases: vec!["Panis Angelicus (Franck)".into(), "O Lord Most Holy".into()],
        original_attribution: Some("Cesar Franck".into()),
        provenance: Some("Messe a trois voix".into()),
        rights_notes: Some("Public domain".into()),
        liturgical_moments: vec!["Communion".into(), "Entrance".into()],
        occasions: vec!["Corpus Christi".into()],
    };
    let id = create(cx, &store, draft).expect("created");

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        store.read_with(cx, |store, _| {
            let song = store.library().song(id).expect("song");
            assert_eq!(song.title(), "Panis Angelicus");
            assert_eq!(
                song.aliases(),
                ["Panis Angelicus (Franck)", "O Lord Most Holy"]
            );
            assert_eq!(song.original_attribution(), Some("Cesar Franck"));
            assert_eq!(song.provenance(), Some("Messe a trois voix"));
            assert_eq!(song.rights_notes(), Some("Public domain"));
            let moments: Vec<&str> = song
                .liturgical_moments()
                .iter()
                .map(String::as_str)
                .collect();
            assert_eq!(moments, ["Communion", "Entrance"]);
            let occasions: Vec<&str> = song.occasions().iter().map(String::as_str).collect();
            assert_eq!(occasions, ["Corpus Christi"]);
        });
    };
    check(cx, &store);
    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_invalid_drafts_are_refused_with_nothing_written(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-song-refused");
    let (slot, store) = open_parish(cx, &dir);
    let changes = count_changes(cx, &store);

    let with_alias = |title: &str, alias: &str| SongDraft {
        aliases: vec![alias.into()],
        ..SongDraft::titled(title)
    };
    let cases = [
        (SongDraft::titled("   "), "Song title is blank"),
        (SongDraft::titled(""), "Song title is blank"),
        (with_alias("Kyrie", "Kyrie"), "alias repeats a value"),
        (with_alias("Kyrie", "  "), "alias is blank"),
        (
            SongDraft {
                aliases: vec!["Lord".into(), "Lord".into()],
                ..SongDraft::titled("Kyrie")
            },
            "alias repeats a value",
        ),
    ];
    for (draft, message) in cases {
        let outcome = create(cx, &store, draft);
        assert_eq!(outcome, Err(SharedString::from(message)));
    }
    assert_eq!(song_count(cx, &store), 0);
    assert_eq!(*changes.borrow(), 0, "a refusal emits nothing");

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(song_count(cx, &store), 0, "nothing reached disk");
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_an_exact_duplicate_title_creates_a_second_song(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-song-duplicate");
    let (slot, store) = open_parish(cx, &dir);

    let first = create(cx, &store, SongDraft::titled("Ave Maria")).expect("first");
    let second = create(cx, &store, SongDraft::titled("Ave Maria")).expect("second");
    assert_ne!(first, second);
    assert_eq!(song_count(cx, &store), 2);

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        for id in [first, second] {
            assert_eq!(store.library().song(id).expect("song").title(), "Ave Maria");
        }
    });
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_a_failed_storage_write_leaves_memory_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-song-write-fails");
    let (slot, store) = open_parish(cx, &dir);
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);

    // Swap in a storage whose folder is a plain file: the real worker answers
    // every request with an error.
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    let file = dir.join("not-a-folder");
    std::fs::write(&file, b"x").expect("file");
    slot.put(Storage::spawn(StorageConfig::in_directory(&file)));

    let message = create(cx, &store, SongDraft::titled("Ave Maria")).expect_err("write fails");
    assert_ne!(message.as_ref(), "The parish store is not open.");
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    assert_eq!(*changes.borrow(), 0, "a failed write emits nothing");

    // An empty slot is refused with the store's own text.
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    assert_eq!(
        create(cx, &store, SongDraft::titled("Ave Maria")),
        Err(SharedString::from("The parish store is not open."))
    );
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac6_store_choir_and_song_writes_complete_in_submission_order(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-song-order");
    let (slot, store) = open_parish(cx, &dir);

    // After each Changed, the (Choirs, Songs) counts the library then held.
    let seen: Rc<RefCell<Vec<(usize, usize)>>> = Rc::default();
    let log = seen.clone();
    cx.update(|cx| {
        cx.subscribe(&store, move |store, _: &LibraryEvent, cx| {
            let library = store.read(cx).library();
            log.borrow_mut()
                .push((library.choirs().count(), library.songs().count()));
        })
        .detach();
    });

    // Choir first, Song second, both submitted before either is answered.
    let (choir, song) = store.update(cx, |store, cx| {
        (
            store.create_choir(ChoirDraft::named("Youth Choir"), cx),
            store.create_song(SongDraft::titled("Ave Maria"), cx),
        )
    });
    cx.foreground_executor().block_test(choir).expect("choir");
    cx.foreground_executor().block_test(song).expect("song");
    assert_eq!(*seen.borrow(), [(1, 0), (1, 1)]);

    // Song first, Choir second.
    seen.borrow_mut().clear();
    let (song, choir) = store.update(cx, |store, cx| {
        (
            store.create_song(SongDraft::titled("Kyrie"), cx),
            store.create_choir(ChoirDraft::named("Old Choir"), cx),
        )
    });
    cx.foreground_executor().block_test(song).expect("song");
    cx.foreground_executor().block_test(choir).expect("choir");
    assert_eq!(*seen.borrow(), [(1, 2), (2, 2)]);
    finish(cx, &slot, &dir);
}
