//! Behavior of `LibraryStore` writes (Songs and Arrangements) over a real store in a temporary
//! parish folder: what reaches disk, what reaches memory, and in what order.

use std::{cell::RefCell, path::Path, rc::Rc};

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, SongId},
    revisions::{RevisionEntity, RevisionKind},
    song_library::{
        Arrangement, ArrangementDetails, ChoirDraft, PaginationState, SongDraft, SongLibrary,
    },
};
use gpui::{Entity, SharedString, Task, TestAppContext};
use storage::{
    NewArrangement, NewSection, RevisionRecord, ShutdownOutcome, Storage, StorageConfig,
};

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
    assert_ne!(message.as_ref(), "The change was abandoned.");
    assert_eq!(message.as_ref(), "storage is not open");
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
    assert_eq!(*changes.borrow(), 0, "an unreachable store emits nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac6_store_the_store_serializes_choir_and_song_writes_in_submission_order(
    cx: &mut TestAppContext,
) {
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
    assert_eq!(
        store.read_with(cx, |store, _| store.write_state()),
        (false, 0)
    );

    // A valid Choir, a refused Song and a valid Song, back to back, with the
    // executor not yet run: the first is in flight and the others wait behind
    // it in the store's own queue.
    let (choir, refused, song) = store.update(cx, |store, cx| {
        let choir = store.create_choir(ChoirDraft::named("Youth Choir"), cx);
        let refused = store.create_song(SongDraft::titled("  "), cx);
        let song = store.create_song(SongDraft::titled("Ave Maria"), cx);
        (choir, refused, song)
    });
    assert_eq!(
        store.read_with(cx, |store, _| store.write_state()),
        (true, 2),
        "one write in flight, two queued behind it"
    );
    assert!(seen.borrow().is_empty(), "nothing has landed yet");

    cx.foreground_executor().block_test(choir).expect("choir");
    assert_eq!(
        cx.foreground_executor().block_test(refused),
        Err(SharedString::from("Song title is blank")),
        "the refused write fails without I/O"
    );
    let song = cx.foreground_executor().block_test(song).expect("song");
    assert_eq!(
        store.read_with(cx, |store, _| store.write_state()),
        (false, 0)
    );
    assert_eq!(
        *seen.borrow(),
        [(1, 0), (1, 1)],
        "only the valid writes land"
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        assert_eq!(
            store.library().songs().count(),
            1,
            "nothing else was written"
        );
        assert_eq!(store.library().choirs().count(), 1);
        assert_eq!(
            store.library().song(song).expect("song").title(),
            "Ave Maria"
        );
    });
    finish(cx, &slot, &dir);
}

// ---- Arrangement writes (T2.6) ----

fn details(language: &str) -> ArrangementDetails {
    ArrangementDetails {
        languages: vec![language.to_owned()],
        ..ArrangementDetails::default()
    }
}

fn run<T: 'static>(cx: &mut TestAppContext, task: Task<T>) -> T {
    cx.foreground_executor().block_test(task)
}

fn create_arr(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    song: SongId,
    details: ArrangementDetails,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| store.create_arrangement(song, details, cx));
    run(cx, task)
}

fn save_arr(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    details: ArrangementDetails,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| {
        store.save_arrangement_details(id, details, cx)
    });
    run(cx, task)
}

fn duplicate_arr(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| store.duplicate_arrangement(id, cx));
    run(cx, task)
}

fn make_choir(cx: &mut TestAppContext, store: &Entity<LibraryStore>, name: &str) -> ChoirId {
    let draft = ChoirDraft::named(name);
    let task = store.update(cx, |store, cx| store.create_choir(draft, cx));
    run(cx, task).expect("choir")
}

fn archive_choir(cx: &mut TestAppContext, store: &Entity<LibraryStore>, id: ChoirId) {
    let task = store.update(cx, |store, cx| store.archive_choir(id, cx));
    run(cx, task).expect("archived");
}

fn history(cx: &mut TestAppContext, slot: &StorageSlot, id: ArrangementId) -> Vec<RevisionRecord> {
    let future = slot
        .with(|storage| storage.revisions(RevisionEntity::Arrangement(id)))
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("revisions")
}

fn arrangement_of(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
) -> Arrangement {
    store.read_with(cx, |store, _| {
        store
            .library()
            .arrangement(id)
            .expect("arrangement")
            .clone()
    })
}

/// A Song with a two-section, three-occurrence Arrangement approved for
/// pagination, written straight through storage, then the parish reopened.
fn seeded(
    cx: &mut TestAppContext,
    tag: &str,
) -> (
    std::path::PathBuf,
    StorageSlot,
    Entity<LibraryStore>,
    SongId,
    ArrangementId,
) {
    let dir = temp_dir(tag);
    let (slot, store) = open_parish(cx, &dir);
    let song = create(cx, &store, SongDraft::titled("Panis Angelicus")).expect("song");
    let future = slot
        .with(|storage| {
            storage.create_arrangement(NewArrangement {
                song,
                title: Some("Latin SATB".into()),
                languages: vec!["Latin".into()],
                choirs: Vec::new(),
                rendition_attribution: None,
                occasions: vec!["Corpus Christi".into()],
                notes: None,
                sections: vec![
                    NewSection {
                        label: Some("Verse".into()),
                        lyrics: "Panis angelicus".into(),
                    },
                    NewSection {
                        label: None,
                        lyrics: "Fit panis hominum".into(),
                    },
                ],
                sequence: vec![0, 1, 0],
            })
        })
        .expect("open");
    let record = cx.foreground_executor().block_test(future).expect("seeded");
    let future = slot
        .with(|storage| storage.set_pagination_state(record.id, PaginationState::Approved))
        .expect("open");
    cx.foreground_executor().block_test(future).expect("state");
    let (slot, store) = reopen(cx, &slot, &dir);
    (dir, slot, store, song, record.id)
}

#[gpui::test]
fn ac6_store_a_created_arrangement_survives_reopen(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-arr-create");
    let (slot, store) = open_parish(cx, &dir);
    let song = create(cx, &store, SongDraft::titled("Ave Maria")).expect("song");
    let changes = count_changes(cx, &store);

    let wanted = ArrangementDetails {
        title: None,
        languages: vec!["latin".into()],
        rendition_attribution: Some("Schubert".into()),
        occasions: vec!["Marian feasts".into()],
        notes: Some("From the 1998 hymnal".into()),
        choirs: Vec::new(),
    };
    let id = create_arr(cx, &store, song, wanted).expect("created");
    assert_eq!(*changes.borrow(), 1);
    // The language is stored under its display name.
    let expected = ArrangementDetails {
        languages: vec!["Latin".into()],
        rendition_attribution: Some("Schubert".into()),
        occasions: vec!["Marian feasts".into()],
        notes: Some("From the 1998 hymnal".into()),
        ..ArrangementDetails::default()
    };
    let arrangement = arrangement_of(cx, &store, id);
    assert_eq!(arrangement.details(), expected);
    assert_eq!(arrangement.title(), None, "a blank title stays None");
    assert_eq!(arrangement.pagination_state(), PaginationState::Draft);
    assert_eq!(history(cx, &slot, id).len(), 1, "one Creation revision");

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).details(), expected);
    assert_eq!(arrangement_of(cx, &store, id).title(), None);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_an_arrangement_the_domain_refuses_writes_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-arr-refused");
    let (slot, store) = open_parish(cx, &dir);
    let song = create(cx, &store, SongDraft::titled("Ave Maria")).expect("song");
    let changes = count_changes(cx, &store);

    let stranger = ChoirId::from_entity(EntityId::from_random_bytes([9; 16]));
    let no_song = SongId::from_entity(EntityId::from_random_bytes([8; 16]));
    let cases = [
        (
            song,
            ArrangementDetails::default(),
            "language is blank".to_owned(),
        ),
        (
            song,
            ArrangementDetails {
                title: Some("  ".into()),
                ..details("Latin")
            },
            "Arrangement title is blank".to_owned(),
        ),
        (
            song,
            details("Klingon"),
            "\"Klingon\" is not in Ordo's list of languages".to_owned(),
        ),
        (
            song,
            ArrangementDetails {
                choirs: vec![stranger],
                ..details("Latin")
            },
            format!("no Choir has identity {stranger}"),
        ),
        (
            no_song,
            details("Latin"),
            format!("no Song work has identity {no_song}"),
        ),
    ];
    for (song, details, message) in cases {
        let outcome = create_arr(cx, &store, song, details);
        assert_eq!(outcome, Err(SharedString::from(message)));
    }
    assert_eq!(*changes.borrow(), 0, "a refusal emits nothing");

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        assert_eq!(store.library().arrangements().count(), 0);
    });
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_an_archived_choir_cannot_be_newly_associated(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-arr-archived-choir");
    let (slot, store) = open_parish(cx, &dir);
    let song = create(cx, &store, SongDraft::titled("Ave Maria")).expect("song");
    let old = make_choir(cx, &store, "Old Choir");
    archive_choir(cx, &store, old);
    let message = create_arr(
        cx,
        &store,
        song,
        ArrangementDetails {
            choirs: vec![old],
            ..details("Latin")
        },
    )
    .expect_err("archived Choir");
    assert_eq!(
        message.as_ref(),
        format!("Choir {old} is archived and inspect-only until restored")
    );
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_save_details_writes_one_revision_and_keeps_the_content(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-arr-save");
    let before = arrangement_of(cx, &store, id);
    assert_eq!(before.pagination_state(), PaginationState::Approved);
    let history_before = history(cx, &slot, id);
    let changes = count_changes(cx, &store);

    let edited = ArrangementDetails {
        title: Some("Panis Angelicus (Franck)".into()),
        languages: vec!["Latin".into(), "English".into()],
        notes: Some("Rights cleared".into()),
        ..before.details()
    };
    assert_eq!(save_arr(cx, &store, id, edited.clone()), Ok(id));
    assert_eq!(*changes.borrow(), 1);

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(after.details(), edited);
        assert_eq!(after.sections(), before.sections());
        assert_eq!(after.sequence(), before.sequence());
        assert_eq!(after.pagination_state(), PaginationState::Approved);
    };
    check(cx, &store);
    let history_after = history(cx, &slot, id);
    assert_eq!(history_after.len(), history_before.len() + 1);
    let last = history_after.last().expect("revision");
    assert_eq!(last.kind, RevisionKind::Edit);
    assert_eq!(last.parent, history_before.last().map(|r| r.id));
    assert!(last.session.is_some());

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_two_saves_in_a_row_both_succeed_in_their_own_sessions(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-arr-two-saves");
    let base = arrangement_of(cx, &store, id).details();
    let start = history(cx, &slot, id).len();

    let first = ArrangementDetails {
        notes: Some("First".into()),
        ..base.clone()
    };
    let second = ArrangementDetails {
        notes: Some("Second".into()),
        ..base
    };
    assert_eq!(save_arr(cx, &store, id, first), Ok(id));
    assert_eq!(save_arr(cx, &store, id, second.clone()), Ok(id));

    let revisions = history(cx, &slot, id);
    assert_eq!(revisions.len(), start + 2);
    let [.., one, two] = revisions.as_slice() else {
        panic!("two new revisions");
    };
    assert_eq!(two.parent, Some(one.id), "the parent chain advances");
    assert_ne!(one.session, two.session, "each Save is its own session");
    assert_eq!(arrangement_of(cx, &store, id).details(), second);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_a_refused_save_writes_no_revision(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-arr-save-refused");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();

    let outcome = save_arr(
        cx,
        &store,
        id,
        ArrangementDetails {
            languages: Vec::new(),
            ..before.details()
        },
    );
    assert_eq!(outcome, Err(SharedString::from("language is blank")));
    assert_eq!(history(cx, &slot, id).len(), start);
    assert_eq!(arrangement_of(cx, &store, id), before);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_duplicate_copies_the_stored_arrangement_under_new_identities(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-arr-duplicate");
    let source = arrangement_of(cx, &store, id);
    let source_history = history(cx, &slot, id);
    let changes = count_changes(cx, &store);

    let copy_id = duplicate_arr(cx, &store, id).expect("duplicated");
    assert_ne!(copy_id, id);
    assert_eq!(*changes.borrow(), 1);

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let copy = arrangement_of(cx, store, copy_id);
        assert_eq!(copy.song(), song);
        assert_eq!(copy.details(), source.details());
        assert_eq!(copy.pagination_state(), PaginationState::Draft);
        assert_eq!(copy.sections().len(), source.sections().len());
        for (new, old) in copy.sections().iter().zip(source.sections()) {
            assert_ne!(new.id(), old.id(), "section ids are disjoint");
            assert_eq!(new.label(), old.label());
            assert_eq!(new.lyrics(), old.lyrics());
        }
        // The Sequence places the copy's own sections in the same order.
        let places = |a: &Arrangement| -> Vec<usize> {
            a.sequence()
                .iter()
                .map(|o| {
                    a.sections()
                        .iter()
                        .position(|s| s.id() == o.section())
                        .expect("own section")
                })
                .collect()
        };
        assert_eq!(places(&copy), places(&source));
        assert_eq!(copy.sequence().len(), 3);
        for new in copy.sequence() {
            assert!(
                source.sequence().iter().all(|old| old.id() != new.id()),
                "occurrence ids are disjoint"
            );
        }
        // The source is untouched.
        assert_eq!(arrangement_of(cx, store, id), source);
    };
    check(cx, &store);
    let copy_history = history(cx, &slot, copy_id);
    assert_eq!(copy_history.len(), 1, "one Creation revision");
    assert_eq!(copy_history[0].kind, RevisionKind::Creation);
    assert_eq!(history(cx, &slot, id), source_history);

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_duplicate_drops_archived_choirs_and_refuses_an_archived_song(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-arr-duplicate-choirs");
    let active = make_choir(cx, &store, "Koro");
    let old = make_choir(cx, &store, "Old");
    let with_both = ArrangementDetails {
        choirs: vec![active, old],
        ..arrangement_of(cx, &store, id).details()
    };
    save_arr(cx, &store, id, with_both).expect("saved");
    archive_choir(cx, &store, old);

    let copy = duplicate_arr(cx, &store, id).expect("duplicated");
    let choirs = arrangement_of(cx, &store, copy).details().choirs;
    assert_eq!(choirs, vec![active], "the archived Choir is dropped");
    assert_eq!(arrangement_of(cx, &store, id).details().choirs.len(), 2);

    // An archived Song is refused with no write.
    let future = slot
        .with(|storage| storage.archive_song(song))
        .expect("open");
    cx.foreground_executor()
        .block_test(future)
        .expect("archived");
    let (slot, store) = reopen(cx, &slot, &dir);
    let before = store.read_with(cx, |store, _| store.library().clone());
    let message = duplicate_arr(cx, &store, id).expect_err("archived Song");
    assert_eq!(
        message.as_ref(),
        format!("Song work {song} is archived and inspect-only until restored")
    );
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_a_failed_arrangement_write_leaves_memory_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-arr-write-fails");
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);

    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    let file = dir.join("not-a-folder");
    std::fs::write(&file, b"x").expect("file");
    slot.put(Storage::spawn(StorageConfig::in_directory(&file)));

    let edited = ArrangementDetails {
        notes: Some("Never lands".into()),
        ..arrangement_of(cx, &store, id).details()
    };
    for outcome in [
        create_arr(cx, &store, song, details("Latin")),
        save_arr(cx, &store, id, edited),
        duplicate_arr(cx, &store, id),
    ] {
        assert_eq!(outcome, Err(SharedString::from("storage is not open")));
        store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    }
    assert_eq!(*changes.borrow(), 0);

    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    assert_eq!(
        save_arr(cx, &store, id, details("Latin")),
        Err(SharedString::from("The parish store is not open."))
    );
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn ac6_store_arrangement_writes_of_every_kind_complete_in_submission_order(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-arr-order");
    let (slot, store) = open_parish(cx, &dir);
    let song = create(cx, &store, SongDraft::titled("Ave Maria")).expect("song");

    // After each Changed: (Choirs, Arrangements).
    let seen: Rc<RefCell<Vec<(usize, usize)>>> = Rc::default();
    let log = seen.clone();
    cx.update(|cx| {
        cx.subscribe(&store, move |store, _: &LibraryEvent, cx| {
            let library = store.read(cx).library();
            log.borrow_mut()
                .push((library.choirs().count(), library.arrangements().count()));
        })
        .detach();
    });

    // A Choir, a refused Arrangement and a valid one, back to back with the
    // executor not yet run: the first is in flight, the others wait behind it.
    let (choir, refused, created) = store.update(cx, |store, cx| {
        let choir = store.create_choir(ChoirDraft::named("Koro"), cx);
        let refused = store.create_arrangement(song, ArrangementDetails::default(), cx);
        let created = store.create_arrangement(song, details("Latin"), cx);
        (choir, refused, created)
    });
    assert_eq!(
        store.read_with(cx, |store, _| store.write_state()),
        (true, 2)
    );
    let choir = run(cx, choir).expect("choir");
    assert_eq!(
        run(cx, refused),
        Err(SharedString::from("language is blank"))
    );
    let created = run(cx, created).expect("created");

    // The Duplicate is queued behind the Save, so it copies the saved Choir.
    let (saved, copied) = store.update(cx, |store, cx| {
        let edited = ArrangementDetails {
            choirs: vec![choir],
            ..details("Latin")
        };
        let saved = store.save_arrangement_details(created, edited, cx);
        let copied = store.duplicate_arrangement(created, cx);
        (saved, copied)
    });
    assert_eq!(run(cx, saved), Ok(created));
    let copy = run(cx, copied).expect("copied");
    assert_eq!(
        *seen.borrow(),
        [(1, 0), (1, 1), (1, 1), (1, 2)],
        "only the valid writes land, in order"
    );
    assert_eq!(
        arrangement_of(cx, &store, copy).details().choirs,
        vec![choir]
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        assert_eq!(store.library().arrangements().count(), 2);
    });
    finish(cx, &slot, &dir);
}
