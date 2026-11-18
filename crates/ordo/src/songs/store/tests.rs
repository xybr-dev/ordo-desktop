//! Behavior of `LibraryStore` writes (Songs and Arrangements) over a real store in a temporary
//! parish folder: what reaches disk, what reaches memory, and in what order.

use std::{cell::RefCell, path::Path, rc::Rc};

use domain::{
    identity::{
        ArrangementId, ChoirId, EntityId, HistorySessionId, OccurrenceId, RevisionId, SectionId,
        SongId,
    },
    revisions::{EditPoint, RevisionEntity, RevisionKind},
    song_library::{
        ArchiveState, Arrangement, ArrangementDetails, ArrangementDraft, ArrangementLyrics,
        ArrangementVisibility, BlockerGroup, ChoirDraft, DeletionBlocker, DeletionVerdict,
        DurableReference, HydrateArrangement, HydrateSong, OccurrenceDraft, PaginationState,
        RecordRef, SectionDraft, SongDraft, SongLibrary, SongLibraryError, Step,
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

/// Each stored section's pagination state, in authored order.
fn pagination_states(arrangement: &Arrangement) -> Vec<PaginationState> {
    arrangement
        .sections()
        .iter()
        .map(|section| section.pagination())
        .collect()
}

/// Approves every section of a stored Arrangement through storage, as one
/// lyrics revision, and returns the new stored record.
fn approve_stored(
    cx: &mut TestAppContext,
    slot: &StorageSlot,
    record: &storage::ArrangementRecord,
) {
    let mut content = ArrangementContent::of(record);
    for section in &mut content.sections {
        section.pagination = PaginationState::Approved;
    }
    let id = record.id;
    let future = slot
        .with(|storage| {
            storage.commit_arrangement_lyrics(
                id,
                content.sections,
                content.sequence,
                HistorySessionId::from_entity(EntityId::from_random_bytes([0xee; 16])),
            )
        })
        .expect("open");
    cx.foreground_executor()
        .block_test(future)
        .expect("approved");
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
    approve_stored(cx, &slot, &record);
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
    assert!(
        arrangement.sections().is_empty(),
        "a new Arrangement has no sections to approve"
    );
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
    assert_eq!(
        pagination_states(&before),
        vec![PaginationState::Approved, PaginationState::Approved]
    );
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
        assert_eq!(pagination_states(&after), pagination_states(&before));
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
        assert_eq!(
            pagination_states(&copy),
            vec![PaginationState::Draft, PaginationState::Draft],
            "a duplicated Arrangement starts every section in Draft"
        );
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
    for section in &mut newer.sections {
        section.pagination = PaginationState::Draft;
    }
    let head = history(cx, &slot, id).last().expect("head").id;
    let session = HistorySessionId::from_entity(EntityId::from_random_bytes([7; 16]));
    let future = slot
        .with(|storage| {
            storage.commit_arrangement_edit(
                id,
                newer.clone(),
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
        // Memory takes every section's state as stored. The domain rule would
        // have kept the sections Approved here, so Draft can only come from
        // storage.
        assert_eq!(
            pagination_states(&after),
            vec![PaginationState::Draft, PaginationState::Draft]
        );
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
        reordered
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        vec![PaginationState::Approved, PaginationState::Approved],
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
        assert_eq!(pagination_states(&after), pagination_states(&before));
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
        pagination_states(&after_remove),
        vec![PaginationState::Approved],
        "removing a section keeps the remaining section's approval"
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
        assert_eq!(
            pagination_states(&restored),
            vec![PaginationState::Approved, PaginationState::Approved],
            "Undo restores every section's approval"
        );
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
fn lyrics_and_details_writes_in_either_order_keep_section_states(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-lyrics-interleaved");
    // Mixed states, so a write that reset them to one value would be seen.
    let mut mixed = arrangement_of(cx, &store, id).lyrics();
    mixed.sections[1].pagination = PaginationState::NeedsReview;
    assert_eq!(save_lyrics(cx, &store, id, mixed, session(0x30)), Ok(id));
    let before = arrangement_of(cx, &store, id);
    assert_eq!(
        pagination_states(&before),
        vec![PaginationState::Approved, PaginationState::NeedsReview]
    );
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
        assert_eq!(pagination_states(&after), pagination_states(&before));
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
            storage.commit_arrangement_edit(id, content, RevisionKind::Replacement, None, head)
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
    approve_stored(cx, &slot, &record);
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
    assert_eq!(
        pagination_states(&moved),
        vec![PaginationState::Approved, PaginationState::Approved]
    );

    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    assert_eq!(history(cx, &slot, id).len(), start + 2);

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    finish(cx, &slot, &dir);
}

// ---- T2.8: Sequence edits and per-section approval through the store and port ----

#[gpui::test]
fn ac2_inserting_an_occurrence_commits_one_revision_and_leaves_sections(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-seq-insert");
    let before = arrangement_of(cx, &store, id);
    let start = history(cx, &slot, id).len();

    let mut edited = before.lyrics();
    let section = edited.sections[1].id;
    let new = occurrence_ident(0xa1);
    edited
        .insert_occurrence(new, section, None)
        .expect("insert");
    assert_eq!(save_lyrics(cx, &store, id, edited, session(0x61)), Ok(id));

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let after = arrangement_of(cx, store, id);
        assert_eq!(after.sequence().len(), before.sequence().len() + 1);
        assert_eq!(after.sequence().last().map(|o| o.id()), Some(new));
        assert_eq!(after.sequence().last().map(|o| o.section()), Some(section));
        assert_eq!(
            after.lyrics().sections,
            before.lyrics().sections,
            "inserting an occurrence leaves every section and its approval"
        );
        assert_eq!(after.details(), before.details());
    };
    check(cx, &store);
    assert_eq!(
        history(cx, &slot, id).len(),
        start + 1,
        "exactly one revision"
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac4_moving_an_occurrence_through_the_port_then_undo_restores_order(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-seq-move");
    let baseline = arrangement_of(cx, &store, id).lyrics();
    let first = baseline.sequence[0].id;
    let start = history(cx, &slot, id).len();
    let editor = lyrics_editor(cx, &store, id, baseline.clone());

    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                assert!(
                    lyrics
                        .move_occurrence(first, Step::Down)
                        .expect("occurrence")
                );
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    let moved = arrangement_of(cx, &store, id);
    assert_eq!(moved.sequence()[1].id(), first);
    assert_eq!(
        moved.lyrics().sections,
        baseline.sections,
        "sections are untouched"
    );

    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    assert_eq!(history(cx, &slot, id).len(), start + 2);

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac5_removal_then_undo_through_the_port_restores_the_occurrence_id_and_position(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-seq-remove");
    let baseline = arrangement_of(cx, &store, id).lyrics();
    let removed = baseline.sequence[1].id;
    let start = history(cx, &slot, id).len();
    let editor = lyrics_editor(cx, &store, id, baseline.clone());

    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                lyrics.remove_occurrence(removed).expect("occurrence");
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    let after = arrangement_of(cx, &store, id);
    assert_eq!(after.sequence().len(), 2);
    assert_eq!(
        after.lyrics().sections,
        baseline.sections,
        "removing an occurrence keeps its section"
    );

    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    let restored = arrangement_of(cx, &store, id);
    assert_eq!(restored.lyrics(), baseline);
    assert_eq!(
        restored.sequence()[1].id(),
        removed,
        "same identity, same position"
    );
    assert_eq!(history(cx, &slot, id).len(), start + 2);

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(arrangement_of(cx, &store, id).lyrics(), baseline);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn approve_then_undo_restores_the_previous_state(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-approve-undo");
    let baseline = arrangement_of(cx, &store, id).lyrics();
    let first = baseline.sections[0].id;
    let editor = lyrics_editor(cx, &store, id, baseline.clone());

    // An edit to approved lyrics sends the section to review.
    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                lyrics
                    .set_section_lyrics(first, "Changed words".to_owned())
                    .expect("section");
                *lyrics = lyrics.clone().reviewed_against(&baseline);
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(
        pagination_states(&arrangement_of(cx, &store, id)),
        vec![PaginationState::NeedsReview, PaginationState::Approved]
    );
    let start = history(cx, &slot, id).len();

    // Approving that one section writes one revision; Undo puts review back.
    let accepted = editor.update(cx, |editor, cx| {
        editor.structural(
            EditPoint::Structural,
            |lyrics| {
                assert!(lyrics.approve_section(first).expect("section"));
            },
            cx,
        )
    });
    assert!(accepted);
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    assert_eq!(
        pagination_states(&arrangement_of(cx, &store, id)),
        vec![PaginationState::Approved; 2]
    );
    assert_eq!(history(cx, &slot, id).len(), start + 1);

    assert!(editor.update(cx, |editor, cx| editor.undo(cx)));
    assert_eq!(settle(cx, &editor), NavigationGuard::Proceed);
    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        assert_eq!(
            pagination_states(&arrangement_of(cx, store, id)),
            vec![PaginationState::NeedsReview, PaginationState::Approved],
            "Undo restores the previous section state"
        );
    };
    check(cx, &store);
    assert_eq!(history(cx, &slot, id).len(), start + 2);

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
}

// ---- Lifecycle, deletion, History and revision restore (T2.9) ----

const BLOCKED: &str = "Something now uses it, so it can’t be deleted.";
const STALE: &str = "This changed since you opened History. Reopen it to restore.";
const ALREADY: &str = "It’s already archived.";
const ARCHIVE_FIRST: &str = "Archive it first.";
const RESTORE_FIRST: &str = "Restore it before restoring a revision.";
const ISNT_ARCHIVED: &str = "It isn’t archived.";
const COULDNT_SAVE: &str = "Couldn't save.";

fn library_of(cx: &mut TestAppContext, store: &Entity<LibraryStore>) -> SongLibrary {
    store.read_with(cx, |store, _| store.library().clone())
}

fn archive_song_of(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: SongId,
) -> Result<SongId, SharedString> {
    let task = store.update(cx, |store, cx| store.archive_song(id, cx));
    run(cx, task)
}

fn restore_song_of(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: SongId,
) -> Result<SongId, SharedString> {
    let task = store.update(cx, |store, cx| store.restore_song(id, cx));
    run(cx, task)
}

fn archive_arr(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| store.archive_arrangement(id, cx));
    run(cx, task)
}

fn restore_arr(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| store.restore_arrangement(id, cx));
    run(cx, task)
}

fn delete_arr(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: ArrangementId,
) -> Result<usize, SharedString> {
    let task = store.update(cx, |store, cx| store.delete_arrangement(id, cx));
    run(cx, task)
}

fn delete_song_of(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    id: SongId,
) -> Result<usize, SharedString> {
    let task = store.update(cx, |store, cx| store.delete_song(id, cx));
    run(cx, task)
}

fn review(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    target: RecordRef,
) -> Result<super::DeletionReview, SharedString> {
    let task = store.update(cx, |store, cx| store.deletion_review(target, cx));
    run(cx, task)
}

fn history_of(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    entity: RevisionEntity,
) -> Result<Vec<storage::HistoryRevision>, SharedString> {
    let task = store.update(cx, |store, cx| store.history(entity, cx));
    run(cx, task)
}

fn restore_rev(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    revision: RevisionId,
    head: RevisionId,
) -> Result<RevisionId, SharedString> {
    let task = store.update(cx, |store, cx| store.restore_revision(revision, head, cx));
    run(cx, task)
}

fn mass_plan(byte: u8) -> DurableReference {
    DurableReference::MassPlan(EntityId::from_random_bytes([byte; 16]))
}

fn plant(
    cx: &mut TestAppContext,
    slot: &StorageSlot,
    target: RecordRef,
    reference: DurableReference,
) {
    let future = slot
        .with(|storage| storage.plant_reference(target, reference))
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("planted");
}

fn unplant(
    cx: &mut TestAppContext,
    slot: &StorageSlot,
    target: RecordRef,
    reference: DurableReference,
) {
    let future = slot
        .with(|storage| storage.remove_reference(target, reference))
        .expect("storage is open");
    cx.foreground_executor()
        .block_test(future)
        .expect("removed");
}

#[gpui::test]
fn ac4_archive_and_restore_through_the_store_update_memory_after_storage(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-lifecycle");
    let revisions = history(cx, &slot, id).len();
    let changes = count_changes(cx, &store);

    assert_eq!(archive_arr(cx, &store, id), Ok(id));
    assert!(arrangement_of(cx, &store, id).is_archived());
    assert_eq!(*changes.borrow(), 1, "one Changed per successful write");

    // A refusal leaves memory byte for byte as it was and emits nothing.
    let before = library_of(cx, &store);
    assert_eq!(
        archive_arr(cx, &store, id),
        Err(SharedString::from(ALREADY))
    );
    assert_eq!(
        restore_song_of(cx, &store, song),
        Err(SharedString::from(ISNT_ARCHIVED))
    );
    assert_eq!(library_of(cx, &store), before);
    assert_eq!(*changes.borrow(), 1);

    assert_eq!(restore_arr(cx, &store, id), Ok(id));
    assert_eq!(
        restore_arr(cx, &store, id),
        Err(SharedString::from(ISNT_ARCHIVED)),
        "restoring what is active is refused, not told to archive it first"
    );
    assert_eq!(*changes.borrow(), 2);
    assert!(!arrangement_of(cx, &store, id).is_archived());
    assert_eq!(archive_song_of(cx, &store, song), Ok(song));
    assert!(store.read_with(cx, |store, _| {
        store.library().song(song).expect("song").is_archived()
    }));
    assert_eq!(
        archive_song_of(cx, &store, song),
        Err(SharedString::from(ALREADY))
    );
    assert_eq!(*changes.borrow(), 3);
    assert_eq!(
        history(cx, &slot, id).len(),
        revisions,
        "archive and restore write no revision"
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    assert!(store.read_with(cx, |store, _| {
        store.library().song(song).expect("song").is_archived()
    }));
    assert_eq!(restore_song_of(cx, &store, song), Ok(song));
    assert!(!store.read_with(cx, |store, _| {
        store.library().song(song).expect("song").is_archived()
    }));
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac4_archiving_a_song_leaves_child_states_in_memory_and_storage(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, own) = seeded(cx, "store-lifecycle-children");
    let other = create_arr(cx, &store, song, details("English")).expect("second arrangement");
    assert_eq!(archive_arr(cx, &store, own), Ok(own));
    assert_eq!(archive_song_of(cx, &store, song), Ok(song));

    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        store.read_with(cx, |store, _| {
            let library = store.library();
            assert_eq!(
                library.arrangement_visibility(own),
                Ok(ArrangementVisibility::Archived)
            );
            assert_eq!(
                library.arrangement_visibility(other),
                Ok(ArrangementVisibility::SuppressedBySong)
            );
            assert!(!library.arrangement(other).expect("other").is_archived());
        });
    };
    check(cx, &store);
    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);

    assert_eq!(restore_song_of(cx, &store, song), Ok(song));
    store.read_with(cx, |store, _| {
        let library = store.library();
        assert_eq!(
            library.arrangement_visibility(own),
            Ok(ArrangementVisibility::Archived)
        );
        assert_eq!(
            library.arrangement_visibility(other),
            Ok(ArrangementVisibility::Active)
        );
    });
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_deletion_review_reports_planted_references(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-review");
    let target = RecordRef::Arrangement(id);

    let active = review(cx, &store, target).expect("review of an active arrangement");
    assert_eq!(
        active.verdict,
        DeletionVerdict::Blocked(vec![DeletionBlocker::NotArchived])
    );
    assert_eq!(active.groups, vec![BlockerGroup::NotArchived]);

    assert_eq!(archive_arr(cx, &store, id), Ok(id));
    let safe = review(cx, &store, target).expect("review");
    assert_eq!(safe.verdict, DeletionVerdict::Allowed);
    assert!(safe.groups.is_empty());
    assert_eq!((safe.facts.sections, safe.facts.occurrences), (2, 3));
    assert_eq!(safe.facts.revisions, history(cx, &slot, id).len());

    let plan = mass_plan(0x31);
    plant(cx, &slot, target, plan.clone());
    let blocked = review(cx, &store, target).expect("review");
    let DurableReference::MassPlan(plan_id) = plan.clone() else {
        unreachable!("a Mass plan");
    };
    assert_eq!(
        blocked.verdict,
        DeletionVerdict::Blocked(vec![DeletionBlocker::Referenced(vec![plan.clone()])])
    );
    assert_eq!(blocked.groups, vec![BlockerGroup::MassPlans(vec![plan_id])]);
    assert_eq!(blocked.facts.references, vec![plan.clone()]);

    // A Song with an Arrangement: the Arrangement is a blocker, with the facts.
    let song_review = review(cx, &store, RecordRef::Song(song)).expect("song review");
    assert_eq!(song_review.facts.children, vec![id]);
    assert_eq!(
        song_review.groups,
        vec![
            BlockerGroup::NotArchived,
            BlockerGroup::Arrangements(vec![id])
        ]
    );

    unplant(cx, &slot, target, plan);
    assert_eq!(
        review(cx, &store, target).expect("review").verdict,
        DeletionVerdict::Allowed
    );
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_delete_refused_by_storage_leaves_memory_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-delete-refused");
    assert_eq!(archive_arr(cx, &store, id), Ok(id));
    let target = RecordRef::Arrangement(id);
    assert_eq!(
        review(cx, &store, target).expect("review").verdict,
        DeletionVerdict::Allowed
    );
    let revisions = history(cx, &slot, id).len();
    let before = library_of(cx, &store);
    let changes = count_changes(cx, &store);

    // A reference appears between the review and the delete: storage refuses.
    plant(cx, &slot, target, mass_plan(0x32));
    assert_eq!(delete_arr(cx, &store, id), Err(SharedString::from(BLOCKED)));
    assert_eq!(library_of(cx, &store), before);
    assert_eq!(*changes.borrow(), 0, "a refusal emits nothing");
    assert_eq!(history(cx, &slot, id).len(), revisions);

    // The domain refuses an active Arrangement before storage is asked.
    assert_eq!(restore_arr(cx, &store, id), Ok(id));
    let before = library_of(cx, &store);
    assert_eq!(
        delete_arr(cx, &store, id),
        Err(SharedString::from(ARCHIVE_FIRST))
    );
    assert_eq!(library_of(cx, &store), before);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac6_delete_removes_the_arrangement_from_memory_and_reload(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-delete");
    let erased = history(cx, &slot, id).len();
    assert_eq!(archive_arr(cx, &store, id), Ok(id));

    // The Song cannot go while it owns the Arrangement, archived or not.
    assert_eq!(archive_song_of(cx, &store, song), Ok(song));
    assert_eq!(
        delete_song_of(cx, &store, song),
        Err(SharedString::from(BLOCKED))
    );
    assert_eq!(restore_song_of(cx, &store, song), Ok(song));
    assert_eq!(
        delete_song_of(cx, &store, song),
        Err(SharedString::from(ARCHIVE_FIRST))
    );
    let changes = count_changes(cx, &store);

    assert_eq!(delete_arr(cx, &store, id), Ok(erased));
    assert_eq!(*changes.borrow(), 1);
    store.read_with(cx, |store, _| {
        assert_eq!(
            store.library().arrangement(id).err(),
            Some(SongLibraryError::UnknownArrangement(id))
        );
        assert!(store.library().song(song).is_ok(), "the Song stays");
    });
    assert!(
        history(cx, &slot, id).is_empty(),
        "its revisions went with it"
    );

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        assert!(store.library().arrangement(id).is_err());
    });
    assert_eq!(archive_song_of(cx, &store, song), Ok(song));
    let song_revisions = history_of(cx, &store, RevisionEntity::Song(song))
        .expect("song history")
        .len();
    assert_eq!(delete_song_of(cx, &store, song), Ok(song_revisions));
    store.read_with(cx, |store, _| assert!(store.library().song(song).is_err()));
    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| assert!(store.library().song(song).is_err()));
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac3_restore_revision_refreshes_the_arrangement_and_returns_the_new_head(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-restore-revision");
    let entity = RevisionEntity::Arrangement(id);
    let list = history_of(cx, &store, entity).expect("history");
    assert!(list.len() >= 2, "a creation and the approval");
    let first = list.first().expect("first").record.id;
    let head = list.last().expect("head").record.id;
    assert_eq!(
        pagination_states(&arrangement_of(cx, &store, id)),
        vec![PaginationState::Approved; 2]
    );
    let changes = count_changes(cx, &store);

    let new_head = restore_rev(cx, &store, first, head).expect("restored");
    assert_ne!(new_head, head);
    assert_eq!(*changes.borrow(), 1);
    let chain = history(cx, &slot, id);
    assert_eq!(
        chain.len(),
        list.len() + 1,
        "every earlier revision is kept"
    );
    let last = chain.last().expect("new head");
    assert_eq!(last.id, new_head);
    assert_eq!(last.kind, RevisionKind::Restoration);
    assert_eq!(last.restored_from, Some(first));
    let restored = arrangement_of(cx, &store, id);
    assert_eq!(
        pagination_states(&restored),
        vec![PaginationState::Draft; 2],
        "memory takes the stored record, approval included"
    );

    // Undo is one more restoration over the new head.
    let undone = restore_rev(cx, &store, head, new_head).expect("undone");
    assert_eq!(
        pagination_states(&arrangement_of(cx, &store, id)),
        vec![PaginationState::Approved; 2]
    );
    assert_ne!(undone, new_head);

    let (slot, store) = reopen(cx, &slot, &dir);
    assert_eq!(
        pagination_states(&arrangement_of(cx, &store, id)),
        vec![PaginationState::Approved; 2]
    );
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac3_restore_over_a_stale_head_is_refused_with_operator_copy(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, id) = seeded(cx, "store-restore-stale");
    let entity = RevisionEntity::Arrangement(id);
    let list = history_of(cx, &store, entity).expect("history");
    let first = list.first().expect("first").record.id;
    let head = list.last().expect("head").record.id;
    let before = library_of(cx, &store);
    let changes = count_changes(cx, &store);

    assert_eq!(
        restore_rev(cx, &store, head, first),
        Err(SharedString::from(STALE))
    );
    assert_eq!(library_of(cx, &store), before);
    assert_eq!(*changes.borrow(), 0);
    assert_eq!(history(cx, &slot, id).len(), list.len());

    // An archived Arrangement, or one under an archived Song, is inspect-only.
    assert_eq!(archive_arr(cx, &store, id), Ok(id));
    let before = library_of(cx, &store);
    let seen = *changes.borrow();
    assert_eq!(
        restore_rev(cx, &store, first, head),
        Err(SharedString::from(RESTORE_FIRST))
    );
    assert_eq!(library_of(cx, &store), before);
    assert_eq!(*changes.borrow(), seen, "a refusal emits nothing");
    assert_eq!(restore_arr(cx, &store, id), Ok(id));
    assert_eq!(archive_song_of(cx, &store, song), Ok(song));
    let before = library_of(cx, &store);
    let seen = *changes.borrow();
    assert_eq!(
        restore_rev(cx, &store, first, head),
        Err(SharedString::from(RESTORE_FIRST))
    );
    assert_eq!(library_of(cx, &store), before);
    assert_eq!(*changes.borrow(), seen, "a refusal emits nothing");
    assert_eq!(history(cx, &slot, id).len(), list.len());
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac4_a_failed_archive_write_leaves_memory_and_storage_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, id) = seeded(cx, "store-archive-fails");
    let before = library_of(cx, &store);
    let changes = count_changes(cx, &store);
    let armed = slot
        .with(|storage| storage.inject_write_failure(1))
        .expect("storage is open");
    cx.foreground_executor().block_test(armed).expect("armed");

    assert_eq!(
        archive_arr(cx, &store, id),
        Err(SharedString::from(COULDNT_SAVE))
    );
    assert_eq!(
        library_of(cx, &store),
        before,
        "memory changes only after storage"
    );
    assert_eq!(*changes.borrow(), 0, "a failed write emits nothing");
    let stored = slot
        .with(|storage| storage.arrangement(id))
        .expect("storage is open");
    assert_eq!(
        cx.foreground_executor()
            .block_test(stored)
            .expect("stored")
            .archived_at,
        None,
        "the stored archive stamp is still empty"
    );
    assert_eq!(archive_arr(cx, &store, id), Ok(id), "the hook cleared");
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac3_restoring_a_song_revision_refreshes_the_song_in_memory(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, _id) = seeded(cx, "store-restore-song-revision");
    let entity = RevisionEntity::Song(song);
    let list = history_of(cx, &store, entity).expect("history");
    let creation = list.last().expect("creation").record.id;
    let edited = slot
        .with(|storage| {
            storage.commit_song_edit(
                song,
                storage::SongContent {
                    title: "Other Title".into(),
                    aliases: Vec::new(),
                    original_attribution: None,
                    provenance: None,
                    rights_notes: None,
                    liturgical_moments: Vec::new(),
                    occasions: Vec::new(),
                },
                RevisionKind::Edit,
                Some(HistorySessionId::from_entity(EntityId::from_random_bytes(
                    [7; 16],
                ))),
                creation,
            )
        })
        .expect("storage is open");
    let (_, head) = cx
        .foreground_executor()
        .block_test(edited)
        .expect("song edit");
    // A fresh open so memory holds the edited title.
    let (slot, store) = reopen(cx, &slot, &dir);
    let title = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        store.read_with(cx, |store, _| {
            store.library().song(song).expect("song").title().to_owned()
        })
    };
    assert_eq!(title(cx, &store), "Other Title");
    let changes = count_changes(cx, &store);

    let new_head = restore_rev(cx, &store, creation, head.id).expect("restored");
    assert_ne!(new_head, head.id);
    assert_eq!(title(cx, &store), "Panis Angelicus");
    assert_eq!(*changes.borrow(), 1, "one Changed for the restoration");
    let list = history_of(cx, &store, entity).expect("history");
    let last = list.last().expect("new head");
    assert_eq!(last.record.id, new_head);
    assert_eq!(last.record.kind, RevisionKind::Restoration);
    finish(cx, &slot, &dir);
}

#[gpui::test]
fn ac1_history_backfills_a_t2_1_era_entity_first(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-history-backfill");
    let (slot, store) = open_parish(cx, &dir);
    let outcome = cx.foreground_executor().block_test(slot.shutdown());
    assert_eq!(outcome, Some(ShutdownOutcome::Clean));
    // A Song written before revisions existed: a row and no revision.
    let legacy = Storage::spawn_with_extra_migration(
        StorageConfig::in_directory(&dir),
        8,
        "t2_1_era_song",
        "INSERT INTO song (id, title, created_at) VALUES \
         ('41414141-4141-4141-8141-414141414141', 'Legacy', '2023-11-14T22:13:20.000Z');",
    );
    let ready = legacy.ready();
    let records = legacy.library();
    slot.put(legacy);
    let song = cx
        .foreground_executor()
        .block_test(async move {
            ready.await.expect("ready");
            records.await.expect("records").hydrate().expect("hydrate")
        })
        .songs()
        .next()
        .expect("the legacy song")
        .id();

    let list = history_of(cx, &store, RevisionEntity::Song(song)).expect("history");
    assert_eq!(list.len(), 1, "one backfilled creation revision");
    assert_eq!(list[0].record.kind, RevisionKind::Creation);
    assert!(list[0].pagination_recorded);
    finish(cx, &slot, &dir);
}

// ---------------------------------------------------------------------------
// T2.10: import staging, commit and Markdown save over a real parish store.
// ---------------------------------------------------------------------------

use domain::song_library::{
    ReadinessState,
    import::{
        Choices, DocumentReview, ImportArrangement, ImportDocument, ImportSection, Outcome, Review,
        SequenceEntry, SongChoice, SongIdentity,
    },
    readiness,
};
use storage::exchange::{ManifestOverrides, encode_song_import, pack_for_tests};

use crate::songs::store::{ImportOutcome, ImportStaging};

const STALE_IMPORT: &str =
    "Something in the Library changed while you reviewed. Review the import again.";
const COULDNT: &str = "Couldn't import. Nothing was changed.";

fn song_id(byte: u8) -> SongId {
    SongId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

fn arrangement_id(byte: u8) -> ArrangementId {
    ArrangementId::from_entity(EntityId::from_random_bytes([byte; 16]))
}

/// A one-Arrangement document: a Verse and a Chorus, sung Verse, Chorus.
fn import_doc(
    song: SongId,
    arrangement: ArrangementId,
    title: &str,
    verse: &str,
) -> ImportDocument {
    ImportDocument {
        id: song,
        song: SongDraft::titled(title),
        arrangements: vec![ImportArrangement {
            id: arrangement,
            title: Some("Imported".to_owned()),
            languages: vec!["English".to_owned()],
            choirs: Vec::new(),
            rendition_attribution: None,
            occasions: Vec::new(),
            notes: None,
            sections: vec![
                ImportSection {
                    label: Some("Verse".to_owned()),
                    lyrics: verse.to_owned(),
                },
                ImportSection {
                    label: Some("Chorus".to_owned()),
                    lyrics: "Sing it again".to_owned(),
                },
            ],
            sequence: vec![SequenceEntry::Section(0), SequenceEntry::Section(1)],
            warnings: Vec::new(),
        }],
        warnings: Vec::new(),
    }
}

fn write_ordo(dir: &Path, name: &str, document: &ImportDocument) -> std::path::PathBuf {
    let payload = encode_song_import(document).to_string().into_bytes();
    let bytes = pack_for_tests(&ManifestOverrides::default(), &payload, &[]);
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write the .ordo file");
    path
}

fn stage(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    paths: Vec<std::path::PathBuf>,
) -> ImportStaging {
    let task = store.update(cx, |store, cx| store.stage_import(paths, cx));
    run(cx, task)
}

fn commit(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    staging: &ImportStaging,
    choices: Choices,
) -> Result<ImportOutcome, SharedString> {
    let task = store.update(cx, |store, cx| store.commit_import(staging, choices, cx));
    run(cx, task)
}

fn arm_failure(cx: &mut TestAppContext, slot: &StorageSlot, step: usize) {
    let future = slot
        .with(|storage| storage.inject_write_failure(step))
        .expect("storage is open");
    cx.foreground_executor().block_test(future).expect("armed");
}

fn markdown_file(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write the Markdown file");
    path
}

fn read_md(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    path: std::path::PathBuf,
) -> domain::song_library::import::MarkdownDraft {
    let task = store.update(cx, |store, cx| store.read_markdown(path, cx));
    run(cx, task).expect("read").draft
}

#[gpui::test]
fn a_markdown_file_is_read_once_with_its_name_and_bytes(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-md-once");
    let files = temp_dir("store-md-once-files");
    let (slot, store) = open_parish(cx, &dir);
    let path = markdown_file(&files, "ave.md", AVE_VERUM);
    let task = store.update(cx, |store, cx| store.read_markdown(path, cx));
    let read = run(cx, task).expect("read");
    assert_eq!(read.name, "ave.md");
    assert_eq!(read.bytes, AVE_VERUM.as_bytes());
    assert_eq!(read.draft.document.song.title, "Ave Verum");
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn a_markdown_file_over_the_limit_is_refused_as_unreadable(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-md-limit");
    let files = temp_dir("store-md-limit-files");
    let (slot, store) = open_parish(cx, &dir);
    let big = "a".repeat(usize::try_from(crate::songs::store::MARKDOWN_LIMIT).unwrap_or(0) + 1);
    let path = markdown_file(&files, "huge.md", &big);
    let task = store.update(cx, |store, cx| store.read_markdown(path, cx));
    let refusal = run(cx, task).expect_err("over the limit");
    assert_eq!(refusal, "Couldn't read huge.md.");
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

fn save_md(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    draft: domain::song_library::import::MarkdownDraft,
    target: SongChoice,
) -> Result<ArrangementId, SharedString> {
    let task = store.update(cx, |store, cx| store.save_markdown(draft, target, cx));
    run(cx, task)
}

const AVE_VERUM: &str = "---\ntitle: Ave Verum\nlanguage: Latin\n---\n[Verse]\nAve verum corpus\nnatum de Maria\n\n[Chorus]\nEsto nobis\n\n[Repeat: Verse]\n";

#[gpui::test]
fn ac1_staging_reads_many_files_and_writes_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-stage");
    let files = temp_dir("store-import-stage-files");
    let (slot, store) = open_parish(cx, &dir);
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);

    let mut first = import_doc(song_id(0x11), arrangement_id(0x21), "First Hymn", "One");
    first.arrangements[0].choirs = vec!["Koro Nuevo".to_owned()];
    let second = import_doc(song_id(0x12), arrangement_id(0x22), "Second Hymn", "Two");
    let paths = vec![
        files.join("missing.ordo"),
        write_ordo(&files, "b.ordo", &second),
        write_ordo(&files, "a.ordo", &first),
    ];
    let staging = stage(cx, &store, paths);

    let names: Vec<&str> = staging
        .documents
        .iter()
        .map(|d| d.source.as_str())
        .collect();
    assert_eq!(
        names,
        ["a.ordo", "b.ordo", "missing.ordo"],
        "file-name order"
    );
    assert_eq!(staging.reviews.len(), 3);
    assert!(matches!(staging.reviews[0], Review::Valid(_)));
    assert!(matches!(staging.reviews[1], Review::Valid(_)));
    assert!(
        matches!(staging.reviews[2], Review::Invalid(_)),
        "an unreadable file is an Invalid document"
    );
    store.read_with(cx, |store, _| {
        assert_eq!(store.library(), &before, "staging changed memory");
        assert_eq!(store.library().choirs().count(), 0, "no Choir is created");
    });
    assert_eq!(*changes.borrow(), 0, "staging emits nothing");

    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| {
        assert_eq!(store.library(), &before, "staging wrote to disk");
    });
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac8_commit_is_refused_without_io_while_a_decision_is_open(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-open");
    let files = temp_dir("store-import-open-files");
    let (slot, store) = open_parish(cx, &dir);
    create(cx, &store, SongDraft::titled("Ambiguous")).expect("first");
    create(cx, &store, SongDraft::titled("Ambiguous")).expect("second");
    let document = import_doc(song_id(0x31), arrangement_id(0x41), "Ambiguous", "Words");
    let staging = stage(cx, &store, vec![write_ordo(&files, "x.ordo", &document)]);
    assert!(
        matches!(
            &staging.reviews[0],
            Review::Valid(DocumentReview {
                identity: SongIdentity::NeedsDecision { .. },
                ..
            })
        ),
        "two exact titles need a decision"
    );
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);

    // A write would hit the armed fault; the refusal proves none was tried.
    arm_failure(cx, &slot, 1);
    let message = commit(cx, &store, &staging, Choices::default()).expect_err("open decision");
    assert_eq!(message, "Some items still need a decision.");
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    assert_eq!(*changes.borrow(), 0);
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac9_a_storage_failure_leaves_memory_and_disk_unchanged(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-fault");
    let files = temp_dir("store-import-fault-files");
    let (slot, store) = open_parish(cx, &dir);
    let document = import_doc(song_id(0x51), arrangement_id(0x61), "Faulty Hymn", "Words");
    let staging = stage(cx, &store, vec![write_ordo(&files, "f.ordo", &document)]);
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);

    for step in [1, 4] {
        arm_failure(cx, &slot, step);
        let message = commit(cx, &store, &staging, Choices::default()).expect_err("injected");
        assert_eq!(message, COULDNT, "step {step}");
        store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
        assert_eq!(*changes.borrow(), 0);
    }
    let (slot, store) = reopen(cx, &slot, &dir);
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));

    // The hook cleared with the failed transaction: the same review commits.
    let staging = stage(cx, &store, vec![files.join("f.ordo")]);
    let outcome = commit(cx, &store, &staging, Choices::default()).expect("clean commit");
    assert_eq!(outcome.songs, [document.id]);
    assert_eq!(outcome.arrangements, [document.arrangements[0].id]);
    store.read_with(cx, |store, _| {
        assert_eq!(
            store.library().song(document.id).expect("song").title(),
            "Faulty Hymn"
        );
    });
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac9_a_library_change_after_staging_is_refused_as_stale(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-stale");
    let files = temp_dir("store-import-stale-files");
    let (slot, store) = open_parish(cx, &dir);
    let document = import_doc(song_id(0x71), arrangement_id(0x81), "Late Hymn", "Words");
    let staging = stage(cx, &store, vec![write_ordo(&files, "l.ordo", &document)]);
    assert!(matches!(
        &staging.reviews[0],
        Review::Valid(DocumentReview {
            identity: SongIdentity::New,
            ..
        })
    ));

    // Someone adds a Song with the same title while the review is open.
    create(cx, &store, SongDraft::titled("Late Hymn")).expect("created");
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let message = commit(cx, &store, &staging, Choices::default()).expect_err("stale");
    assert_eq!(message, STALE_IMPORT);
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac9_a_library_replaced_while_the_verdict_runs_off_thread_is_refused_and_nothing_is_written(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-moved");
    let files = temp_dir("store-import-moved-files");
    let (slot, store) = open_parish(cx, &dir);
    let document = import_doc(song_id(0x72), arrangement_id(0x82), "Moved Hymn", "Words");
    let staging = stage(cx, &store, vec![write_ordo(&files, "m.ordo", &document)]);

    // The verdict is computed on a snapshot in the background; the library is
    // replaced (same content, new generation) before it comes back.
    let import = store.update(cx, |store, cx| {
        let import = store.commit_import(&staging, Choices::default(), cx);
        let library = store.library().clone();
        store.replace_library(library, cx);
        import
    });
    assert_eq!(run(cx, import), Err(SharedString::from(STALE_IMPORT)));
    store.read_with(cx, |store, _| {
        assert!(store.library().song(document.id).is_err());
        assert_eq!(store.write_state(), (false, 0));
    });
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac10_reimporting_after_commit_stages_only_skips_and_commits_nothing(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-reimport");
    let files = temp_dir("store-import-reimport-files");
    let (slot, store) = open_parish(cx, &dir);
    let document = import_doc(song_id(0x91), arrangement_id(0xa1), "Stable Hymn", "Words");
    let path = write_ordo(&files, "s.ordo", &document);
    let staging = stage(cx, &store, vec![path.clone()]);
    commit(cx, &store, &staging, Choices::default()).expect("first import");

    let again = stage(cx, &store, vec![path]);
    let Review::Valid(review) = &again.reviews[0] else {
        panic!("valid");
    };
    assert!(matches!(review.identity, SongIdentity::Existing { .. }));
    assert_eq!(
        review.arrangements[0].classification,
        domain::song_library::import::Classification::SkipUnchanged
    );
    let before: SongLibrary = store.read_with(cx, |store, _| store.library().clone());
    let changes = count_changes(cx, &store);
    // A write would hit the armed fault; an empty commit never tries one.
    arm_failure(cx, &slot, 1);
    let outcome = commit(cx, &store, &again, Choices::default()).expect("nothing to commit");
    assert_eq!(outcome, ImportOutcome::default());
    store.read_with(cx, |store, _| assert_eq!(store.library(), &before));
    assert_eq!(*changes.borrow(), 0);
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn an_update_replaces_one_arrangement_as_one_replacement_revision(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, arrangement) = seeded(cx, "store-import-update");
    let files = temp_dir("store-import-update-files");
    let mut document = import_doc(song, arrangement, "Panis Angelicus", "Panis reworded");
    document.arrangements[0].title = Some("Latin SATB".to_owned());
    document.arrangements[0].languages = vec!["Latin".to_owned()];
    document.arrangements[0].occasions = vec!["Corpus Christi".to_owned()];
    let staging = stage(cx, &store, vec![write_ordo(&files, "u.ordo", &document)]);
    let Review::Valid(review) = &staging.reviews[0] else {
        panic!("valid");
    };
    assert!(
        review.arrangements[0]
            .offered
            .contains(&Outcome::UpdateExisting)
    );
    assert!(
        staging.heads.contains_key(&arrangement),
        "the reviewed head is captured"
    );

    // Needs a decision until the operator picks Update.
    assert_eq!(
        commit(cx, &store, &staging, Choices::default()),
        Err(SharedString::from("Some items still need a decision."))
    );
    let before = history(cx, &slot, arrangement);
    let mut choices = Choices::default();
    choices.arrangements.insert((0, 0), Outcome::UpdateExisting);
    let outcome = commit(cx, &store, &staging, choices).expect("update");
    assert_eq!(outcome.replaced, [arrangement]);
    assert!(outcome.arrangements.is_empty() && outcome.songs.is_empty());

    let after = history(cx, &slot, arrangement);
    assert_eq!(after.len(), before.len() + 1, "one revision");
    let head = after.last().expect("head");
    assert_eq!(head.kind, RevisionKind::Replacement);
    assert_eq!(head.parent, before.last().map(|revision| revision.id));
    let stored = arrangement_of(cx, &store, arrangement);
    assert_eq!(stored.sections()[0].lyrics(), "Panis reworded");
    assert_eq!(stored.sections().len(), 2);
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac3_markdown_save_creates_song_and_arrangement_in_one_transaction(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-md-new");
    let files = temp_dir("store-md-new-files");
    let (slot, store) = open_parish(cx, &dir);
    let draft = read_md(cx, &store, markdown_file(&files, "ave.md", AVE_VERUM));
    assert!(draft.can_save());
    let changes = count_changes(cx, &store);

    // A fault inside the transaction leaves nothing behind.
    arm_failure(cx, &slot, 3);
    save_md(cx, &store, draft.clone(), SongChoice::New).expect_err("injected");
    assert_eq!(song_count(cx, &store), 0);
    assert_eq!(*changes.borrow(), 0);

    let id = save_md(cx, &store, draft, SongChoice::New).expect("saved");
    assert_eq!(*changes.borrow(), 1, "one Changed for one save");
    let check = |cx: &mut TestAppContext, store: &Entity<LibraryStore>| {
        let stored = arrangement_of(cx, store, id);
        assert_eq!(stored.sections().len(), 2);
        assert_eq!(stored.sequence().len(), 3);
        assert!(
            stored
                .sections()
                .iter()
                .all(|section| section.pagination() == PaginationState::Draft),
            "Markdown sections begin Draft"
        );
        assert_eq!(song_count(cx, store), 1);
    };
    check(cx, &store);
    let revisions = history(cx, &slot, id);
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Creation);

    let (slot, store) = reopen(cx, &slot, &dir);
    check(cx, &store);
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac3_markdown_save_to_an_existing_song_adds_one_arrangement(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, song, existing) = seeded(cx, "store-md-existing");
    let files = temp_dir("store-md-existing-files");
    let draft = read_md(cx, &store, markdown_file(&files, "ave.md", AVE_VERUM));

    let id = save_md(cx, &store, draft, SongChoice::Existing(song)).expect("saved");
    assert_ne!(id, existing);
    store.read_with(cx, |store, _| {
        assert_eq!(store.library().songs().count(), 1, "no new Song");
        let owned = store.library().arrangements_for_song(song).expect("song");
        assert_eq!(owned.len(), 2, "one Arrangement was added");
        assert_eq!(
            store.library().song(song).expect("song").title(),
            "Panis Angelicus",
            "the Song is untouched"
        );
    });
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn ac2_markdown_save_with_an_unresolved_repeat_stores_an_incomplete_arrangement(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-md-unresolved");
    let files = temp_dir("store-md-unresolved-files");
    let (slot, store) = open_parish(cx, &dir);
    let text =
        "---\ntitle: Gloria\nlanguage: English\n---\n[Verse]\nGlory to God\n\n[Repeat: Coda]\n";
    let draft = read_md(cx, &store, markdown_file(&files, "g.md", text));
    assert!(draft.can_save(), "an unresolved repeat is not blocking");

    let id = save_md(cx, &store, draft, SongChoice::New).expect("saved");
    let stored = arrangement_of(cx, &store, id);
    let labels: Vec<Option<&str>> = stored.sections().iter().map(|s| s.label()).collect();
    assert_eq!(
        labels,
        [Some("Verse"), Some("Coda")],
        "a placeholder section"
    );
    assert_eq!(stored.sections()[1].lyrics(), "");
    let state = readiness(&stored.lyrics(), &std::collections::BTreeSet::new()).state;
    assert_eq!(state, ReadinessState::Incomplete);
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn import_and_autosave_share_one_queue(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, slot, store, _song, arrangement) = seeded(cx, "store-import-queue");
    let files = temp_dir("store-import-queue-files");
    let document = import_doc(song_id(0xb1), arrangement_id(0xc1), "Queued Hymn", "Words");
    let staging = stage(cx, &store, vec![write_ordo(&files, "q.ordo", &document)]);
    let changes = count_changes(cx, &store);

    let mut edited = arrangement_of(cx, &store, arrangement).lyrics();
    edited
        .set_section_lyrics(edited.sections[1].id, "Typed meanwhile".to_owned())
        .expect("section");
    let (import, autosave) = store.update(cx, |store, cx| {
        let import = store.commit_import(&staging, Choices::default(), cx);
        let autosave =
            store.save_arrangement_lyrics(arrangement, edited.clone(), session(0xd1), cx);
        (import, autosave)
    });
    store.read_with(cx, |store, _| {
        assert_eq!(
            store.write_state(),
            (true, 1),
            "the autosave waits its turn"
        );
    });
    assert!(run(cx, import).is_ok());
    assert_eq!(run(cx, autosave), Ok(arrangement));
    assert_eq!(*changes.borrow(), 2);
    store.read_with(cx, |store, _| {
        assert!(store.library().song(document.id).is_ok());
    });
    assert_eq!(arrangement_of(cx, &store, arrangement).lyrics(), edited);
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

#[gpui::test]
fn a_write_queued_during_the_background_verdict_runs_after_the_import_exactly_once(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-queued-verdict");
    let files = temp_dir("store-import-queued-verdict-files");
    let (slot, store) = open_parish(cx, &dir);
    let document = import_doc(song_id(0xb2), arrangement_id(0xc2), "Verdict Hymn", "Words");
    let staging = stage(cx, &store, vec![write_ordo(&files, "v.ordo", &document)]);
    let changes = count_changes(cx, &store);
    let generation = store.read_with(cx, |store, _| store.generation());

    // The details save names an Arrangement only the import creates: it can
    // succeed only if the queue runs it after the import, and only once.
    let details = ArrangementDetails {
        title: Some("Retitled while the verdict ran".to_owned()),
        languages: vec!["English".to_owned()],
        ..ArrangementDetails::default()
    };
    let (import, save) = store.update(cx, |store, cx| {
        let import = store.commit_import(&staging, Choices::default(), cx);
        let save = store.save_arrangement_details(document.arrangements[0].id, details, cx);
        (import, save)
    });
    store.read_with(cx, |store, _| {
        assert_eq!(
            store.write_state(),
            (true, 1),
            "the verdict holds the queue; the save waits behind it"
        );
        assert!(
            store.library().song(document.id).is_err(),
            "nothing is applied before the verdict lands"
        );
    });
    assert!(run(cx, import).is_ok());
    assert_eq!(run(cx, save), Ok(document.arrangements[0].id));
    store.read_with(cx, |store, _| {
        assert_eq!(store.write_state(), (false, 0));
        assert_eq!(
            store.generation(),
            generation + 2,
            "exactly one completion per write"
        );
        assert_eq!(
            store
                .library()
                .arrangement(document.arrangements[0].id)
                .expect("the imported Arrangement")
                .title(),
            Some("Retitled while the verdict ran")
        );
    });
    assert_eq!(*changes.borrow(), 2, "each write announced once");
    let revisions = history(cx, &slot, document.arrangements[0].id);
    assert_eq!(revisions.len(), 2, "the creation, then the one edit");
    assert_eq!(revisions[0].kind, RevisionKind::Creation);
    assert_eq!(revisions[1].parent, Some(revisions[0].id));
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}

fn reclassify_with(
    cx: &mut TestAppContext,
    store: &Entity<LibraryStore>,
    staging: &ImportStaging,
    songs: std::collections::BTreeMap<usize, SongChoice>,
) -> ImportStaging {
    let documents = staging.documents.clone();
    let task = store.update(cx, |store, cx| store.reclassify(documents, songs, cx));
    run(cx, task)
}

#[gpui::test]
fn ac5_a_song_choice_then_commit_writes_under_the_chosen_song(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let dir = temp_dir("store-import-song-choice");
    let files = temp_dir("store-import-song-choice-files");
    let (slot, store) = open_parish(cx, &dir);
    // Two Songs with the same title: the second is forced in as new.
    let first = import_doc(song_id(0x31), arrangement_id(0x41), "Twin Hymn", "One");
    let staging = stage(cx, &store, vec![write_ordo(&files, "a.ordo", &first)]);
    commit(cx, &store, &staging, Choices::default()).expect("first song");
    let second = import_doc(song_id(0x32), arrangement_id(0x42), "Twin Hymn", "Two");
    let staging = stage(cx, &store, vec![write_ordo(&files, "b.ordo", &second)]);
    let mut songs = std::collections::BTreeMap::new();
    songs.insert(0, SongChoice::New);
    let staging = reclassify_with(cx, &store, &staging, songs.clone());
    let choices = Choices {
        songs,
        ..Choices::default()
    };
    commit(cx, &store, &staging, choices).expect("second song");

    // A third document now has two exact candidates and needs a decision.
    let third = import_doc(song_id(0x33), arrangement_id(0x43), "Twin Hymn", "Three");
    let staging = stage(cx, &store, vec![write_ordo(&files, "c.ordo", &third)]);
    let Review::Valid(review) = &staging.reviews[0] else {
        panic!("valid");
    };
    assert!(matches!(
        review.identity,
        SongIdentity::NeedsDecision { .. }
    ));

    // Choosing a Song and re-reviewing must not make the commit stale.
    let mut songs = std::collections::BTreeMap::new();
    songs.insert(0, SongChoice::Existing(song_id(0x32)));
    let staging = reclassify_with(cx, &store, &staging, songs.clone());
    let Review::Valid(review) = &staging.reviews[0] else {
        panic!("valid");
    };
    // The chosen Song already has a similar Arrangement: add this one too.
    assert!(review.arrangements[0].offered.contains(&Outcome::AddAsNew));
    let mut choices = Choices {
        songs,
        ..Choices::default()
    };
    choices.arrangements.insert((0, 0), Outcome::AddAsNew);
    let outcome = commit(cx, &store, &staging, choices).expect("commit under the chosen Song");
    assert_eq!(outcome.arrangements, vec![arrangement_id(0x43)]);
    store.read_with(cx, |store, _| {
        let arrangement = store
            .library()
            .arrangement(arrangement_id(0x43))
            .expect("the imported Arrangement");
        assert_eq!(arrangement.song(), song_id(0x32));
    });
    finish(cx, &slot, &dir);
    let _ = std::fs::remove_dir_all(&files);
}
