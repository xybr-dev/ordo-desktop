//! Behavior of `LibraryStore` writes (Songs and Arrangements) over a real store in a temporary
//! parish folder: what reaches disk, what reaches memory, and in what order.

use std::{cell::RefCell, path::Path, rc::Rc};

use domain::{
    identity::{
        ArrangementId, ChoirId, EntityId, HistorySessionId, OccurrenceId, SectionId, SongId,
    },
    revisions::{EditPoint, RevisionEntity, RevisionKind},
    song_library::{
        ArchiveState, Arrangement, ArrangementDetails, ArrangementDraft, ArrangementLyrics,
        ChoirDraft, HydrateArrangement, HydrateSong, OccurrenceDraft, PaginationState,
        SectionDraft, SongDraft, SongLibrary, SongLibraryError, Step,
    },
};
use gpui::{AppContext, Entity, SharedString, Task, TestAppContext};
use storage::{
    ArrangementContent, NewArrangement, NewSection, RevisionRecord, ShutdownOutcome, Storage,
    StorageConfig,
};

use crate::{
    edit_session::{CommitPoint, EditSession, NavigationGuard, RevisionPort, SaveFailure},
    songs::{
        LibraryEvent, LibraryStore, StorageSlot,
        choirs::tests::{open_parish, temp_dir},
        lyrics::port::LyricsPort,
    },
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

/// Swaps the store's memory for a library holding one legacy Arrangement of
/// `song` with no language, as a parish stored before T2.6 may hold.
pub(crate) fn hold_a_legacy_arrangement(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    song: SongId,
    id: ArrangementId,
) {
    hold_a_legacy_arrangement_in(cx, store, song, id, Vec::new());
}

/// [`hold_a_legacy_arrangement`] with the given languages, which need not be
/// in the built-in list.
pub(crate) fn hold_a_legacy_arrangement_in(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    song: SongId,
    id: ArrangementId,
    languages: Vec<String>,
) {
    let at = domain::identity::Timestamp::from_millis(0);
    let library = SongLibrary::hydrate(
        vec![HydrateSong {
            id: song,
            created_at: at,
            draft: SongDraft::titled("Legacy"),
            archive_state: ArchiveState::Active,
        }],
        vec![HydrateArrangement {
            id,
            song,
            created_at: at,
            draft: ArrangementDraft {
                languages,
                ..ArrangementDraft::default()
            },
            choirs: Vec::new(),
            pagination_state: PaginationState::Draft,
            archive_state: ArchiveState::Active,
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("hydrate");
    store.update(cx, |store, cx| store.replace_library(library, cx));
}

#[gpui::test]
fn ac2_store_duplicating_an_arrangement_without_a_language_is_refused(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-arr-dup-legacy");
    let start = history(cx, &slot, id).len();
    hold_a_legacy_arrangement(cx, &store, song, id);
    let changes = count_changes(cx, &store);

    assert_eq!(
        duplicate_arr(cx, &store, id),
        Err(SharedString::from("language is blank"))
    );
    assert_eq!(history(cx, &slot, id).len(), start, "no revision");
    assert_eq!(*changes.borrow(), 0, "no event");
    store.read_with(cx, |store, _| {
        assert_eq!(store.library().arrangements().count(), 1);
    });
    let future = slot
        .with(|storage| storage.arrangements_for_song(song))
        .expect("storage is open");
    let stored = cx.foreground_executor().block_test(future).expect("list");
    assert_eq!(stored.len(), 1, "no copy was written");
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac2_store_details_may_keep_a_stored_language_that_is_not_listed(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-arr-keep-language");
    // Memory holds an Arrangement whose language is not in the built-in list.
    let at = domain::identity::Timestamp::from_millis(0);
    let library = SongLibrary::hydrate(
        vec![HydrateSong {
            id: song,
            created_at: at,
            draft: SongDraft::titled("Panis Angelicus"),
            archive_state: ArchiveState::Active,
        }],
        vec![HydrateArrangement {
            id,
            song,
            created_at: at,
            draft: ArrangementDraft {
                languages: vec!["Latin".into(), "ceb".into()],
                ..ArrangementDraft::default()
            },
            choirs: Vec::new(),
            pagination_state: PaginationState::Approved,
            archive_state: ArchiveState::Active,
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("hydrate");
    store.update(cx, |store, cx| store.replace_library(library, cx));

    let kept = ArrangementDetails {
        languages: vec!["Latin".into(), "ceb".into()],
        notes: Some("Kept".into()),
        ..arrangement_of(cx, &store, id).details()
    };
    assert_eq!(save_arr(cx, &store, id, kept), Ok(id));
    let (slot, store) = reopen(cx, &slot, &dir);
    let stored = arrangement_of(cx, &store, id);
    assert_eq!(stored.languages(), ["Latin", "ceb"]);
    assert_eq!(stored.notes(), Some("Kept"));
    let added = ArrangementDetails {
        languages: vec!["Latin".into(), "ceb".into(), "Klingon".into()],
        ..arrangement_of(cx, &store, id).details()
    };
    assert_eq!(
        save_arr(cx, &store, id, added),
        Err(SharedString::from(
            "\"Klingon\" is not in Ordo's list of languages"
        ))
    );
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_a_save_never_overwrites_newer_sections(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-arr-newer-sections");
    let before = arrangement_of(cx, &store, id);

    // Another writer changes the lyrics and the pagination state behind the
    // store, after its memory was loaded.
    let future = slot
        .with(|storage| storage.arrangement(id))
        .expect("storage is open");
    let stored = cx.foreground_executor().block_test(future).expect("record");
    let mut newer = ArrangementContent::of(&stored);
    newer.sections[0].lyrics = "Newer lyrics".to_owned();
    let head = history(cx, &slot, id).last().expect("head").id;
    let session = HistorySessionId::from_entity(EntityId::from_random_bytes([7; 16]));
    let future = slot
        .with(|storage| {
            storage.commit_arrangement_edit(
                id,
                newer.clone(),
                PaginationState::Draft,
                RevisionKind::Edit,
                Some(session),
                head,
            )
        })
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("newer sections");

    let edited = ArrangementDetails {
        notes: Some("Details only".into()),
        ..before.details()
    };
    assert_eq!(save_arr(cx, &store, id, edited.clone()), Ok(id));

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(after.details(), edited);
        assert_eq!(after.sections()[0].lyrics(), "Newer lyrics");
        // Memory takes the pagination state as stored. The domain rule would
        // have moved an Approved Arrangement to NeedsReview here, so Draft can
        // only come from storage.
        assert_eq!(after.pagination_state(), PaginationState::Draft);
    };
    check(cx, &store);
    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_a_save_that_changes_nothing_writes_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-arr-noop-save");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    assert_eq!(save_arr(cx, &store, id, before.details()), Ok(id));
    assert_eq!(history(cx, &slot, id).len(), start, "no revision");
    assert_eq!(*changes.borrow(), 0, "no event");
    assert_eq!(arrangement_of(cx, &store, id), before);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_store_a_storage_refusal_after_validation_leaves_memory_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-arr-storage-refuses");
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);

    // Swap in an open store of another, empty parish: memory still holds the
    // Arrangement, the domain accepts the edit, and storage cannot find it.
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    let other = temp_dir("store-arr-other-parish");
    slot.put(Storage::spawn(StorageConfig::in_directory(&other)));

    let edited = ArrangementDetails {
        notes: Some("Never lands".into()),
        ..arrangement_of(cx, &store, id).details()
    };
    let outcome = save_arr(cx, &store, id, edited);
    assert_eq!(
        outcome,
        Err(SharedString::from(format!(
            "arrangement {id} was not found"
        )))
    );
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    assert_eq!(*changes.borrow(), 0);

    finish(cx, &slot, &other);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Lyrics writes and the Lyrics revision port (T2.7) ----

fn session(byte: u8) -> HistorySessionId {
    HistorySessionId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

fn save_lyrics(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    lyrics: ArrangementLyrics,
    session: HistorySessionId,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| {
        store.save_arrangement_lyrics(id, lyrics, session, cx)
    });
    run(cx, task)
}

/// One editor session over `store`, committed at `committed` and writing
/// through the real [`LyricsPort`].
fn lyrics_editor(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
    committed: ArrangementLyrics,
) -> Entity<EditSession<ArrangementLyrics>> {
    let port: Rc<dyn RevisionPort<ArrangementLyrics>> =
        Rc::new(LyricsPort::new(store.downgrade(), id, cx.to_async()));
    cx.new(|cx| EditSession::new(RevisionEntity::Arrangement(id), committed, port, cx))
}

/// Flushes the session and asserts the queue is durable.
fn settle(
    cx: &mut TestAppContext,
    session: &Entity<EditSession<ArrangementLyrics>>,
) -> NavigationGuard {
    let task = session.update(cx, |session, cx| session.request_navigation(cx));
    run(cx, task)
}

#[gpui::test]
fn ac8_reorder_commits_one_revision_and_leaves_the_sequence(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-reorder");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    let mut reordered = before.lyrics();
    let first = reordered.sections[0].id;
    let second = reordered.sections[1].id;
    assert!(reordered.move_section(first, Step::Down).expect("moved"));
    assert_eq!(
        reordered
            .sections
            .iter()
            .map(|section| section.id)
            .collect::<Vec<_>>(),
        vec![second, first]
    );
    assert_eq!(
        reordered.pagination,
        PaginationState::Approved,
        "order alone never moves the review state"
    );
    let sequence_before: Vec<_> = before
        .sequence()
        .iter()
        .map(|occurrence| (occurrence.id(), occurrence.section()))
        .collect();

    assert_eq!(
        save_lyrics(cx, &store, id, reordered, session(0x21)),
        Ok(id)
    );
    assert_eq!(*changes.borrow(), 1, "one Changed for one lyrics commit");

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(
            after
                .sections()
                .iter()
                .map(|section| section.id())
                .collect::<Vec<_>>(),
            vec![second, first]
        );
        assert_eq!(
            after
                .sequence()
                .iter()
                .map(|occurrence| (occurrence.id(), occurrence.section()))
                .collect::<Vec<_>>(),
            sequence_before,
            "the Sequence keeps its identities and order"
        );
        assert_eq!(after.details(), before.details(), "metadata is untouched");
        assert_eq!(after.pagination_state(), PaginationState::Approved);
    };
    check(cx, &store);

    let revisions = history(cx, &slot, id);
    assert_eq!(revisions.len(), start + 1, "exactly one revision");
    let last = revisions.last().expect("revision");
    assert_eq!(last.kind, RevisionKind::Edit);
    assert_eq!(
        last.parent,
        revisions.get(start - 1).map(|record| record.id)
    );
    assert_eq!(last.session, Some(session(0x21)));

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac9_undo_through_the_port_restores_approval_and_occurrences(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-undo");
    let before = arrangement_of(cx, &store, id);
    let baseline = before.lyrics();
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    let removed = baseline.sections[0].id;
    let removed_label = baseline.sections[0].label.clone();
    assert_eq!(baseline.occurrences_of(removed), 2);
    let session = lyrics_editor(cx, &store, id, baseline.clone());

    let accepted = session.update(cx, |session, cx| {
        session.structural(
            EditPoint::Structural,
            |lyrics| {
                lyrics.remove_section(removed).expect("section");
                *lyrics = lyrics.clone().reviewed_against(&baseline);
            },
            cx,
        )
    });
    assert!(accepted, "the removal is a structural edit");
    assert_eq!(settle(cx, &session), NavigationGuard::Proceed);

    let after_remove = arrangement_of(cx, &store, id);
    assert_eq!(after_remove.sections().len(), 1);
    assert_eq!(
        after_remove.sequence().len(),
        1,
        "the section's occurrences went with it"
    );
    assert_eq!(
        after_remove.pagination_state(),
        PaginationState::NeedsReview,
        "changed lyrics need review"
    );
    let revisions = history(cx, &slot, id);
    assert_eq!(revisions.len(), start + 1);
    assert_eq!(revisions.last().expect("revision").kind, RevisionKind::Edit);

    assert!(session.update(cx, |session, cx| session.undo(cx)));
    assert_eq!(settle(cx, &session), NavigationGuard::Proceed);

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let restored = arrangement_of(cx, store, id);
        assert_eq!(
            restored.lyrics(),
            baseline,
            "Undo restores identity, order, lyrics and pagination"
        );
        assert_eq!(restored.sections()[0].label(), removed_label.as_deref());
        assert_eq!(restored.sequence().len(), 3);
        assert_eq!(restored.pagination_state(), PaginationState::Approved);
        assert_eq!(
            restored.details(),
            before.details(),
            "metadata is untouched"
        );
    };
    check(cx, &store);
    assert_eq!(*changes.borrow(), 2, "one Changed per commit");

    let revisions = history(cx, &slot, id);
    assert_eq!(revisions.len(), start + 2, "Undo is one more revision");
    let [.., removal, undo] = revisions.as_slice() else {
        panic!("two lyrics revisions");
    };
    assert_eq!(removal.kind, RevisionKind::Edit);
    assert_eq!(undo.kind, RevisionKind::Edit);
    assert_eq!(undo.parent, Some(removal.id));
    assert!(undo.session.is_some());

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn lyrics_and_details_writes_in_either_order_keep_both(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-interleaved");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();

    // Lyrics first, then details.
    let mut lyrics_after = before.lyrics();
    lyrics_after
        .set_section_lyrics(lyrics_after.sections[0].id, "First words".to_owned())
        .expect("section");
    assert_eq!(
        save_lyrics(cx, &store, id, lyrics_after, session(0x31)),
        Ok(id)
    );
    let details_after = ArrangementDetails {
        title: Some("After the lyrics".to_owned()),
        notes: Some("Details kept".to_owned()),
        ..before.details()
    };
    assert_eq!(save_arr(cx, &store, id, details_after.clone()), Ok(id));
    let after = arrangement_of(cx, &store, id);
    assert_eq!(after.sections()[0].lyrics(), "First words");
    assert_eq!(after.details(), details_after);

    // Details first, then lyrics.
    let details_mid = ArrangementDetails {
        notes: Some("Second details".to_owned()),
        ..details_after.clone()
    };
    assert_eq!(save_arr(cx, &store, id, details_mid.clone()), Ok(id));
    let mut second_lyrics = after.lyrics();
    second_lyrics
        .set_section_lyrics(second_lyrics.sections[1].id, "Second words".to_owned())
        .expect("section");
    assert_eq!(
        save_lyrics(cx, &store, id, second_lyrics, session(0x32)),
        Ok(id)
    );

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(after.sections()[0].lyrics(), "First words");
        assert_eq!(after.sections()[1].lyrics(), "Second words");
        assert_eq!(after.details(), details_mid);
        assert_eq!(after.sequence(), before.sequence());
        assert_eq!(after.pagination_state(), PaginationState::Approved);
    };
    check(cx, &store);
    assert_eq!(
        history(cx, &slot, id).len(),
        start + 4,
        "each successful write is one revision"
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn a_failed_lyrics_write_leaves_memory_and_history_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-write-fails");
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    let mut edited = arrangement_of(cx, &store, id).lyrics();
    edited
        .set_section_lyrics(edited.sections[1].id, "Never lands".to_owned())
        .expect("section");
    let future = slot
        .with(|storage| storage.inject_write_failure(1))
        .expect("storage is open");
    cx.foreground_executor().block_test(future).expect("armed");

    let outcome = save_lyrics(cx, &store, id, edited.clone(), session(0x41));
    let message = outcome.expect_err("the injected fault reaches the caller");
    assert!(
        message.contains("injected write failure"),
        "unexpected message {message:?}"
    );
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    assert_eq!(history(cx, &slot, id).len(), start);
    assert_eq!(*changes.borrow(), 0, "a failed write emits nothing");

    // The fault hook cleared with the failed transaction, so the same edit
    // lands on retry.
    assert_eq!(
        save_lyrics(cx, &store, id, edited.clone(), session(0x42)),
        Ok(id)
    );
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), edited);
    assert_eq!(history(cx, &slot, id).len(), start + 1);

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), edited);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn lyrics_equal_to_memory_write_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-noop");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    assert_eq!(
        save_lyrics(cx, &store, id, before.lyrics(), session(0x51)),
        Ok(id)
    );
    assert_eq!(history(cx, &slot, id).len(), start, "no revision");
    assert_eq!(*changes.borrow(), 0, "no event");
    assert_eq!(arrangement_of(cx, &store, id), before);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn a_lyrics_write_the_domain_refuses_writes_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-refused");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    let mut blank = before.lyrics();
    blank.sections[0].label = Some("   ".to_owned());
    assert_eq!(
        save_lyrics(cx, &store, id, blank, session(0x61)),
        Err(SharedString::from("section label is blank"))
    );
    assert_eq!(history(cx, &slot, id).len(), start);
    assert_eq!(arrangement_of(cx, &store, id), before);
    assert_eq!(*changes.borrow(), 0);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn an_archived_arrangement_or_song_refuses_a_lyrics_write(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-archived");
    let lyrics = arrangement_of(cx, &store, id).lyrics();
    let future = slot
        .with(|storage| storage.archive_arrangement(id))
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("archived");
    let (slot, store) = reopen(cx, &slot, &dir);
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);
    assert_eq!(
        save_lyrics(cx, &store, id, lyrics, session(0x71)),
        Err(SharedString::from(format!(
            "Arrangement {id} is archived and inspect-only until restored"
        )))
    );
    assert_eq!(history(cx, &slot, id).len(), start);
    assert_eq!(*changes.borrow(), 0);
    finish(cx, &slot, &dir);

    let (dir, slot, store, song, id) = seeded(cx, "store-lyrics-archived-song");
    let lyrics = arrangement_of(cx, &store, id).lyrics();
    let future = slot
        .with(|storage| storage.archive_song(song))
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("archived");
    let (slot, store) = reopen(cx, &slot, &dir);
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);
    assert_eq!(
        save_lyrics(cx, &store, id, lyrics, session(0x72)),
        Err(SharedString::from(format!(
            "Song work {song} is archived and inspect-only until restored"
        )))
    );
    assert_eq!(history(cx, &slot, id).len(), start);
    assert_eq!(*changes.borrow(), 0);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn the_port_refuses_other_commit_points_and_a_released_store(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-port");
    let lyrics = arrangement_of(cx, &store, id).lyrics();
    let start = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);
    let port = LyricsPort::new(store.downgrade(), id, cx.to_async());

    for point in [
        CommitPoint {
            kind: RevisionKind::Creation,
            session: None,
        },
        CommitPoint {
            kind: RevisionKind::Edit,
            session: None,
        },
    ] {
        let outcome = cx
            .foreground_executor()
            .block_test(port.commit(lyrics.clone(), point));
        assert_eq!(
            outcome,
            Err(SaveFailure {
                message: "The Lyrics editor writes Edit revisions with a History session.".into()
            })
        );
    }
    assert_eq!(
        history(cx, &slot, id).len(),
        start,
        "refusals write nothing"
    );
    assert_eq!(*changes.borrow(), 0);

    // A port whose Store was released fails cleanly instead of panicking.
    let weak = store.downgrade();
    drop(store);
    drop(port);
    let port = LyricsPort::new(weak, id, cx.to_async());
    let outcome = cx.foreground_executor().block_test(port.commit(
        lyrics,
        CommitPoint {
            kind: RevisionKind::Edit,
            session: Some(session(0x81)),
        },
    ));
    assert_eq!(
        outcome,
        Err(SaveFailure {
            message: "The parish store is not open.".into()
        })
    );
    finish(cx, &slot, &dir);
}

// ---- Review fixes: B1 no language, B2 whole-library ids, B3 stored duplicates ----

fn section_ident(byte: u8) -> SectionId {
    SectionId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

fn occurrence_ident(byte: u8) -> OccurrenceId {
    OccurrenceId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

/// Commits a `Replacement` revision that removes the stored languages, as a
/// pre-T2.6 Arrangement may hold (only an Edit requires a language).
fn strip_language(cx: &mut TestAppContext, slot: &StorageSlot, id: ArrangementId) {
    let future = slot
        .with(|storage| storage.arrangement(id))
        .expect("storage is open");
    let stored = cx.foreground_executor().block_test(future).expect("record");
    let mut content = ArrangementContent::of(&stored);
    content.languages.clear();
    let head = history(cx, slot, id).last().expect("head").id;
    let future = slot
        .with(|storage| {
            storage.commit_arrangement_edit(
                id,
                content,
                stored.pagination,
                RevisionKind::Replacement,
                None,
                head,
            )
        })
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("languages removed");
}

#[gpui::test]
fn b1_a_lyrics_save_succeeds_on_an_arrangement_stored_with_no_language(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, _store, _song, id) = seeded(cx, "store-lyrics-no-language");
    strip_language(cx, &slot, id);
    let (slot, store) = reopen(cx, &slot, &dir);
    let before = arrangement_of(cx, &store, id);
    assert!(
        before.languages().is_empty(),
        "the stored row has no language"
    );
    let start = history(cx, &slot, id).len();

    let mut edited = before.lyrics();
    edited
        .set_section_lyrics(edited.sections[0].id, "Typed with no language".to_owned())
        .expect("section");
    assert_eq!(
        save_lyrics(cx, &store, id, edited.clone(), session(0x81)),
        Ok(id)
    );
    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(after.sections()[0].lyrics(), "Typed with no language");
        assert_eq!(after.details(), before.details(), "metadata is untouched");
        assert!(after.languages().is_empty());
    };
    check(cx, &store);
    assert_eq!(history(cx, &slot, id).len(), start + 1);

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn b2_ids_held_or_once_held_by_another_arrangement_are_refused_before_any_io(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-lyrics-foreign-ids");
    let other = create_arr(cx, &store, song, details("Latin")).expect("other");
    let held_section = section_ident(0x91);
    let held_occurrence = occurrence_ident(0x92);
    let kept_section = section_ident(0x93);
    let kept_occurrence = occurrence_ident(0x94);
    let lyrics_of_other = |section: SectionId, occurrence: OccurrenceId| ArrangementLyrics {
        sections: vec![SectionDraft::labeled(section, "Other", "words")],
        sequence: vec![OccurrenceDraft::new(occurrence, section)],
        pagination: PaginationState::Draft,
    };
    assert_eq!(
        save_lyrics(
            cx,
            &store,
            other,
            lyrics_of_other(held_section, held_occurrence),
            session(0x95)
        ),
        Ok(other)
    );
    // The other Arrangement then swaps both identities for new ones, so the
    // first pair is reserved to it by history alone.
    assert_eq!(
        save_lyrics(
            cx,
            &store,
            other,
            lyrics_of_other(kept_section, kept_occurrence),
            session(0x96)
        ),
        Ok(other)
    );
    let (slot, store) = reopen(cx, &slot, &dir);

    let before_library: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let before = arrangement_of(cx, &store, id).lyrics();
    let start = history(cx, &slot, id).len();
    let other_start = history(cx, &slot, other).len();
    let changes = count_changes(cx, &store);

    let some_section = before.sections[0].id;
    let cases: Vec<(ArrangementLyrics, SongLibraryError)> = vec![
        {
            let mut next = before.clone();
            next.sections
                .push(SectionDraft::labeled(kept_section, "Current", ""));
            (next, SongLibraryError::DuplicateSection(kept_section))
        },
        {
            let mut next = before.clone();
            next.sections
                .push(SectionDraft::labeled(held_section, "History", ""));
            (next, SongLibraryError::DuplicateSection(held_section))
        },
        {
            let mut next = before.clone();
            next.sequence
                .push(OccurrenceDraft::new(kept_occurrence, some_section));
            (next, SongLibraryError::DuplicateOccurrence(kept_occurrence))
        },
        {
            let mut next = before.clone();
            next.sequence
                .push(OccurrenceDraft::new(held_occurrence, some_section));
            (next, SongLibraryError::DuplicateOccurrence(held_occurrence))
        },
    ];
    for (n, (next, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            save_lyrics(cx, &store, id, next, session(0xa0 + n as u8)),
            Err(SharedString::from(expected.to_string())),
            "case {n}"
        );
        assert_eq!(history(cx, &slot, id).len(), start, "case {n}: no revision");
        assert_eq!(
            history(cx, &slot, other).len(),
            other_start,
            "case {n}: the owner is untouched"
        );
        store.read_with(cx, |store, _| {
            assert_eq!(
                store.library(),
                &before_library,
                "case {n}: memory unchanged"
            );
        });
    }
    assert_eq!(*changes.borrow(), 0, "a refusal emits nothing");
    finish(cx, &slot, &dir);
}

/// A Song with one Arrangement whose two sections are both named "Chorus"
/// (stored duplicates, which Decision 7 keeps valid), approved, then reopened.
fn seeded_duplicates(
    cx: &mut TestAppContext,
    tag: &str,
) -> (
    std::path::PathBuf,
    StorageSlot,
    Entity<LibraryStore>,
    ArrangementId,
) {
    let dir = temp_dir(tag);
    let (slot, store) = open_parish(cx, &dir);
    let song = create(cx, &store, SongDraft::titled("Kyrie")).expect("song");
    let future = slot
        .with(|storage| {
            storage.create_arrangement(NewArrangement {
                song,
                title: None,
                languages: vec!["English".into()],
                choirs: Vec::new(),
                rendition_attribution: None,
                occasions: Vec::new(),
                notes: None,
                sections: vec![
                    NewSection {
                        label: Some("Chorus".into()),
                        lyrics: "first".into(),
                    },
                    NewSection {
                        label: Some("Chorus".into()),
                        lyrics: "second".into(),
                    },
                ],
                sequence: vec![0, 1],
            })
        })
        .expect("open");
    let record = cx.foreground_executor().block_test(future).expect("seeded");
    let future = slot
        .with(|storage| storage.set_pagination_state(record.id, PaginationState::Approved))
        .expect("open");
    cx.foreground_executor().block_test(future).expect("state");
    let (slot, store) = reopen(cx, &slot, &dir);
    (dir, slot, store, record.id)
}

#[gpui::test]
fn b3_undo_restores_a_stored_duplicate_name_through_the_store_and_the_port(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let (dir, slot, store, id) = seeded_duplicates(cx, "store-lyrics-dup-undo");
    let baseline = arrangement_of(cx, &store, id).lyrics();
    assert_eq!(baseline.sections[0].label, baseline.sections[1].label);
    let first = baseline.sections[0].id;
    let start = history(cx, &slot, id).len();

    // Directly through the store: the removal, then the earlier snapshot.
    let mut removed = baseline.clone();
    removed.remove_section(first).expect("section");
    let removed = removed.reviewed_against(&baseline);
    assert_eq!(save_lyrics(cx, &store, id, removed, session(0xb1)), Ok(id));
    assert_eq!(
        save_lyrics(cx, &store, id, baseline.clone(), session(0xb2)),
        Ok(id),
        "the stored duplicate is committable again"
    );
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    assert_eq!(history(cx, &slot, id).len(), start + 2);

    // Through the port and a real EditSession: remove then Undo, then rename
    // then Undo.
    let editor = lyrics_editor(cx, &store, id, baseline.clone());
    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                lyrics.remove_section(first).expect("section");
                *lyrics = lyrics.clone().reviewed_against(&baseline);
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(arrangement_of(cx, &store, id).sections().len(), 1);
    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(
        arrangement_of(cx, &store, id).lyrics(),
        baseline,
        "Undo of the removal restores the duplicate"
    );

    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                lyrics.rename_section(first, "Koro").expect("rename");
                *lyrics = lyrics.clone().reviewed_against(&baseline);
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(
        arrangement_of(cx, &store, id).sections()[0].label(),
        Some("Koro")
    );
    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(
        arrangement_of(cx, &store, id).lyrics(),
        baseline,
        "Undo of the rename restores the duplicate"
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn n8_both_writes_queued_before_either_runs_keep_both(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-queued");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();

    let mut lyrics_after = before.lyrics();
    lyrics_after
        .set_section_lyrics(lyrics_after.sections[0].id, "Queued words".to_owned())
        .expect("section");
    let details_after = ArrangementDetails {
        notes: Some("Queued details".to_owned()),
        ..before.details()
    };
    // Both are queued (the store validates each at dequeue) before either is
    // awaited.
    let lyrics_task = store.update(cx, |store, cx| {
        store.save_arrangement_lyrics(id, lyrics_after, session(0xc1), cx)
    });
    let details_task = store.update(cx, |store, cx| {
        store.save_arrangement_details(id, details_after.clone(), cx)
    });
    assert_eq!(run(cx, lyrics_task), Ok(id));
    assert_eq!(run(cx, details_task), Ok(id));

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(after.sections()[0].lyrics(), "Queued words");
        assert_eq!(after.details(), details_after);
    };
    check(cx, &store);
    assert_eq!(history(cx, &slot, id).len(), start + 2);
    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn n8_undo_of_a_reorder_through_the_port_restores_the_order(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-reorder-undo");
    let baseline = arrangement_of(cx, &store, id).lyrics();
    let first = baseline.sections[0].id;
    let start = history(cx, &slot, id).len();
    let editor = lyrics_editor(cx, &store, id, baseline.clone());

    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                assert!(lyrics.move_section(first, Step::Down).expect("section"));
                *lyrics = lyrics.clone().reviewed_against(&baseline);
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    let moved = arrangement_of(cx, &store, id);
    assert_eq!(moved.sections()[1].id(), first);
    assert_eq!(moved.pagination_state(), PaginationState::Approved);

    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    assert_eq!(history(cx, &slot, id).len(), start + 2);

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    finish(cx, &slot, &dir);
}
