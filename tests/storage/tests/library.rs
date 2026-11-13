//! T2.4 WP1 Song Library store acceptance tests: choir details, archive and
//! restore, pagination state, the one-transaction `library()` read, and the
//! version 4 to 5 upgrade. Every await runs under a 10 s bound so a hang fails
//! the test instead of blocking the run. Databases live in unique temporary
//! directories.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::{
    ArrangementId, ChoirId, EntityId, HistorySessionId, SectionId, SongId, Timestamp,
};
use domain::revisions::{RevisionEntity, RevisionKind};
use domain::song_library::{
    ArchiveState, ArrangementDraft, OccurrenceDraft, PaginationState, SectionDraft, SongLibrary,
    SongLibraryError,
};
use futures_lite::future::block_on;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use storage::{
    ArrangementContent, ArrangementRecord, ChoirDetails, ChoirRecord, LibraryRecords,
    NewArrangement, NewChoir, NewSection, NewSong, RecordKind, ShutdownOutcome, SongRecord,
    Storage, StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);

/// Runs `work` on its own thread; returns a receiver so the caller can bound the wait.
fn spawn_bounded<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> mpsc::Receiver<T> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx
}

/// Drives a Storage future to completion, failing the test after `BOUND`.
fn bounded<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    spawn_bounded(move || block_on(future))
        .recv_timeout(BOUND)
        .expect("storage operation did not complete within 10 s")
}

/// Unique temporary directory under `std::env::temp_dir()`, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ordo-library-tests-{tag}-{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    fn db(&self) -> PathBuf {
        self.0.join("parish.sqlite3")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        for _ in 0..5 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn config(db: &Path) -> StorageConfig {
    StorageConfig::new(db)
}

/// The fixed clock of scripted stores; archive times are asserted against it.
fn fixed_now() -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000)
}

fn open(dir: &TempDir) -> Storage {
    let storage = Storage::spawn_with_sources(config(&dir.db()), Vec::new(), fixed_now());
    bounded(storage.ready()).expect("storage ready");
    storage
}

fn shutdown(storage: Storage) {
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

fn invalid(field: &'static str, reason: &'static str) -> StorageError {
    StorageError::InvalidInput { field, reason }
}

fn name_only(name: &str) -> NewChoir {
    NewChoir {
        name: name.to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    }
}

fn details(
    name: &str,
    short_label: Option<&str>,
    languages: &[&str],
    notes: Option<&str>,
) -> ChoirDetails {
    ChoirDetails {
        name: name.to_owned(),
        short_label: short_label.map(str::to_owned),
        languages: languages.iter().map(|l| (*l).to_owned()).collect(),
        notes: notes.map(str::to_owned),
    }
}

fn song_input(title: &str) -> NewSong {
    NewSong {
        title: title.to_owned(),
        aliases: Vec::new(),
        original_attribution: None,
        provenance: None,
        rights_notes: None,
        liturgical_moments: Vec::new(),
        occasions: Vec::new(),
    }
}

fn arrangement_input(song: SongId, title: &str, choirs: Vec<ChoirId>) -> NewArrangement {
    NewArrangement {
        song,
        title: title.to_owned(),
        languages: vec!["English".to_owned()],
        choirs,
        rendition_attribution: None,
        occasions: Vec::new(),
        sections: vec![
            NewSection {
                label: Some("Verse".to_owned()),
                lyrics: "one".to_owned(),
            },
            NewSection {
                label: None,
                lyrics: "two".to_owned(),
            },
        ],
        sequence: vec![0, 1, 0],
    }
}

/// A small library: two Choirs (one with details), two Songs, two
/// Arrangements of the first Song.
struct Fixture {
    storage: Storage,
    youth: ChoirRecord,
    adult: ChoirRecord,
    song: SongRecord,
    other_song: SongRecord,
    first: ArrangementRecord,
    second: ArrangementRecord,
}

fn fixture(dir: &TempDir) -> Fixture {
    let storage = open(dir);
    let youth = bounded(storage.create_choir(NewChoir {
        name: "Youth".to_owned(),
        short_label: Some("YC".to_owned()),
        languages: vec!["ceb".to_owned(), "en".to_owned()],
        notes: Some("Thursdays".to_owned()),
    }))
    .expect("choir");
    let adult = bounded(storage.create_choir(name_only("Adult"))).expect("choir");
    let song = bounded(storage.create_song(song_input("Pagdayeg"))).expect("song");
    let other_song = bounded(storage.create_song(song_input("Kyrie"))).expect("song");
    let first = bounded(storage.create_arrangement(arrangement_input(
        song.id,
        "SATB",
        vec![youth.id, adult.id],
    )))
    .expect("arrangement");
    let second =
        bounded(storage.create_arrangement(arrangement_input(song.id, "Unison", Vec::new())))
            .expect("arrangement");
    Fixture {
        storage,
        youth,
        adult,
        song,
        other_song,
        first,
        second,
    }
}

fn library(storage: &Storage) -> LibraryRecords {
    bounded(storage.library()).expect("library")
}

#[test]
fn library_returns_everything_with_exact_states_after_restart() {
    let dir = TempDir::new("restart");
    let f = fixture(&dir);
    assert_eq!(f.first.pagination, PaginationState::Draft);

    let archived_choir = bounded(f.storage.archive_choir(f.adult.id)).expect("archive");
    let archived_song = bounded(f.storage.archive_song(f.other_song.id)).expect("archive");
    let archived_arrangement =
        bounded(f.storage.archive_arrangement(f.second.id)).expect("archive");
    let approved = bounded(
        f.storage
            .set_pagination_state(f.first.id, PaginationState::Approved),
    )
    .expect("approve");
    let review = bounded(
        f.storage
            .set_pagination_state(f.second.id, PaginationState::NeedsReview),
    )
    .expect("review");
    assert_eq!(archived_choir.archived_at, Some(fixed_now()));
    assert_eq!(archived_song.archived_at, Some(fixed_now()));
    assert_eq!(archived_arrangement.archived_at, Some(fixed_now()));
    let before = library(&f.storage);
    shutdown(f.storage);

    let storage = open(&dir);
    let after = library(&storage);
    assert_eq!(after, before);
    assert_eq!(after.choirs.len(), 2);
    assert_eq!(after.songs.len(), 2);
    assert_eq!(after.arrangements.len(), 2);

    let choir = |id: ChoirId| {
        after
            .choirs
            .iter()
            .find(|c| c.id == id)
            .expect("choir present")
    };
    assert_eq!(choir(f.youth.id), &f.youth);
    assert_eq!(choir(f.adult.id).archived_at, Some(fixed_now()));
    assert_eq!(choir(f.adult.id).languages, Vec::<String>::new());

    let arrangement = |id: ArrangementId| {
        after
            .arrangements
            .iter()
            .find(|a| a.id == id)
            .expect("arrangement present")
    };
    assert_eq!(
        arrangement(f.first.id).pagination,
        PaginationState::Approved
    );
    assert_eq!(arrangement(f.first.id).archived_at, None);
    assert_eq!(
        arrangement(f.second.id).pagination,
        PaginationState::NeedsReview
    );
    assert_eq!(arrangement(f.second.id).archived_at, Some(fixed_now()));
    // Arrangements carry sections, Sequence and Choirs like `arrangement()`.
    assert_eq!(arrangement(f.first.id).sections.len(), 2);
    assert_eq!(arrangement(f.first.id).sequence.len(), 3);
    assert_eq!(arrangement(f.first.id).choirs, {
        let mut choirs = vec![f.youth.id, f.adult.id];
        choirs.sort_unstable();
        choirs
    });
    assert_eq!(&approved, arrangement(f.first.id));
    assert_eq!(&review, arrangement(f.second.id));
    assert_eq!(bounded(storage.arrangement(f.first.id)), Ok(approved));

    let song_row = |id: SongId| {
        after
            .songs
            .iter()
            .find(|s| s.id == id)
            .expect("song present")
    };
    assert_eq!(song_row(f.song.id).archived_at, None);
    assert_eq!(song_row(f.other_song.id).archived_at, Some(fixed_now()));
    shutdown(storage);
}

#[test]
fn an_empty_database_has_an_empty_library() {
    let dir = TempDir::new("empty");
    let storage = open(&dir);
    assert_eq!(
        bounded(storage.library()),
        Ok(LibraryRecords {
            songs: Vec::new(),
            arrangements: Vec::new(),
            choirs: Vec::new(),
            reserved: Vec::new(),
        })
    );
    shutdown(storage);
}

#[test]
fn archive_and_restore_each_kind() {
    let dir = TempDir::new("archive");
    let f = fixture(&dir);

    // Song.
    let archived = bounded(f.storage.archive_song(f.song.id)).expect("archive song");
    assert_eq!(archived.archived_at, Some(fixed_now()));
    assert_eq!(
        bounded(f.storage.archive_song(f.song.id)),
        Err(StorageError::AlreadyArchived {
            kind: RecordKind::Song
        })
    );
    let restored = bounded(f.storage.restore_song(f.song.id)).expect("restore song");
    assert_eq!(restored, f.song);
    assert_eq!(
        bounded(f.storage.restore_song(f.song.id)),
        Err(StorageError::NotArchived {
            kind: RecordKind::Song
        })
    );

    // Arrangement.
    let archived = bounded(f.storage.archive_arrangement(f.first.id)).expect("archive arrangement");
    assert_eq!(archived.archived_at, Some(fixed_now()));
    assert_eq!(
        bounded(f.storage.archive_arrangement(f.first.id)),
        Err(StorageError::AlreadyArchived {
            kind: RecordKind::Arrangement
        })
    );
    let restored = bounded(f.storage.restore_arrangement(f.first.id)).expect("restore arrangement");
    assert_eq!(restored, f.first);
    assert_eq!(
        bounded(f.storage.restore_arrangement(f.first.id)),
        Err(StorageError::NotArchived {
            kind: RecordKind::Arrangement
        })
    );

    // Choir.
    let archived = bounded(f.storage.archive_choir(f.youth.id)).expect("archive choir");
    assert_eq!(archived.archived_at, Some(fixed_now()));
    assert_eq!(
        bounded(f.storage.archive_choir(f.youth.id)),
        Err(StorageError::AlreadyArchived {
            kind: RecordKind::Choir
        })
    );
    let restored = bounded(f.storage.restore_choir(f.youth.id)).expect("restore choir");
    assert_eq!(restored, f.youth);
    assert_eq!(
        bounded(f.storage.restore_choir(f.youth.id)),
        Err(StorageError::NotArchived {
            kind: RecordKind::Choir
        })
    );

    // Unknown ids.
    let unknown = EntityId::from_random_bytes([0x77; 16]);
    assert_eq!(
        bounded(f.storage.archive_song(SongId::from_entity(unknown))),
        Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: unknown
        })
    );
    assert_eq!(
        bounded(
            f.storage
                .restore_arrangement(ArrangementId::from_entity(unknown))
        ),
        Err(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: unknown
        })
    );
    assert_eq!(
        bounded(f.storage.archive_choir(ChoirId::from_entity(unknown))),
        Err(StorageError::NotFound {
            kind: RecordKind::Choir,
            id: unknown
        })
    );
    assert_eq!(
        bounded(f.storage.set_pagination_state(
            ArrangementId::from_entity(unknown),
            PaginationState::Approved
        )),
        Err(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: unknown
        })
    );
    shutdown(f.storage);
}

#[test]
fn archiving_a_song_leaves_its_arrangements_unchanged() {
    let dir = TempDir::new("no-cascade");
    let f = fixture(&dir);
    let before = bounded(f.storage.arrangements_for_song(f.song.id)).expect("arrangements");
    let mut expected = vec![f.first.clone(), f.second.clone()];
    expected.sort_by_key(|a| a.id.to_string());
    let mut sorted = before.clone();
    sorted.sort_by_key(|a| a.id.to_string());
    assert_eq!(sorted, expected);

    bounded(f.storage.archive_song(f.song.id)).expect("archive song");

    assert_eq!(
        bounded(f.storage.arrangements_for_song(f.song.id)),
        Ok(before)
    );
    // Archiving a Choir does not touch the Arrangements that name it either.
    bounded(f.storage.archive_choir(f.youth.id)).expect("archive choir");
    assert_eq!(bounded(f.storage.arrangement(f.first.id)), Ok(f.first));
    shutdown(f.storage);
}

#[test]
fn edit_choir_details_round_trip() {
    let dir = TempDir::new("edit-choir");
    let f = fixture(&dir);

    let edited = bounded(f.storage.edit_choir(
        f.youth.id,
        details(
            "Youth Choir",
            Some("YCH"),
            &["en", "ceb", "fil"],
            Some("Sundays"),
        ),
    ))
    .expect("edit");
    assert_eq!(edited.id, f.youth.id);
    assert_eq!(edited.name, "Youth Choir");
    assert_eq!(edited.short_label.as_deref(), Some("YCH"));
    assert_eq!(
        edited.languages,
        vec!["en".to_owned(), "ceb".to_owned(), "fil".to_owned()]
    );
    assert_eq!(edited.notes.as_deref(), Some("Sundays"));
    assert_eq!(edited.created_at, f.youth.created_at);
    assert_eq!(edited.archived_at, None);

    // Clearing every optional detail replaces rather than merges.
    let cleared = bounded(
        f.storage
            .edit_choir(f.youth.id, details("Youth", None, &[], None)),
    )
    .expect("edit");
    assert_eq!(cleared.short_label, None);
    assert_eq!(cleared.languages, Vec::<String>::new());
    assert_eq!(cleared.notes, None);

    // An archived Choir keeps its archive state through an edit.
    bounded(f.storage.archive_choir(f.adult.id)).expect("archive");
    let archived_edit = bounded(
        f.storage
            .edit_choir(f.adult.id, details("Adults", Some("A"), &["en"], None)),
    )
    .expect("edit archived");
    assert_eq!(archived_edit.archived_at, Some(fixed_now()));

    let all = library(&f.storage);
    assert!(all.choirs.contains(&cleared));
    assert!(all.choirs.contains(&archived_edit));
    shutdown(f.storage);

    // Choir names are not unique in storage.
    let dir = TempDir::new("same-name");
    let storage = open(&dir);
    let first = bounded(storage.create_choir(name_only("Choir"))).expect("choir");
    let second = bounded(storage.create_choir(name_only("Choir"))).expect("choir");
    assert_ne!(first.id, second.id);
    shutdown(storage);
}

#[test]
fn choir_details_are_validated_like_the_domain() {
    let dir = TempDir::new("choir-validation");
    let f = fixture(&dir);
    let before = library(&f.storage);

    for (input, expected) in [
        (details(" ", None, &[], None), invalid("name", "blank")),
        (
            details("A", Some(" "), &[], None),
            invalid("short_label", "blank"),
        ),
        (
            details("A", None, &["en", " "], None),
            invalid("language", "blank"),
        ),
        (
            details("A", None, &["en", "en"], None),
            invalid("language", "duplicate"),
        ),
        (details("A", None, &[], Some("")), invalid("notes", "blank")),
    ] {
        assert_eq!(
            bounded(f.storage.edit_choir(f.youth.id, input.clone())),
            Err(expected.clone())
        );
        assert_eq!(
            bounded(f.storage.create_choir(NewChoir {
                name: input.name,
                short_label: input.short_label,
                languages: input.languages,
                notes: input.notes,
            })),
            Err(expected)
        );
    }
    let unknown = EntityId::from_random_bytes([0x78; 16]);
    assert_eq!(
        bounded(
            f.storage
                .edit_choir(ChoirId::from_entity(unknown), details("A", None, &[], None))
        ),
        Err(StorageError::NotFound {
            kind: RecordKind::Choir,
            id: unknown
        })
    );
    assert_eq!(library(&f.storage), before);
    shutdown(f.storage);
}

#[test]
fn injected_write_failures_change_nothing() {
    let dir = TempDir::new("faults");
    let f = fixture(&dir);
    let before = library(&f.storage);
    let injected = Err(StorageError::Sqlite("injected write failure".to_owned()));

    // archive: one write statement.
    bounded(f.storage.inject_write_failure(1)).expect("arm");
    assert_eq!(
        bounded(f.storage.archive_song(f.song.id)).map(|_| ()),
        injected
    );
    assert_eq!(library(&f.storage), before);
    bounded(f.storage.inject_write_failure(1)).expect("arm");
    assert_eq!(
        bounded(f.storage.archive_arrangement(f.first.id)).map(|_| ()),
        injected
    );
    assert_eq!(library(&f.storage), before);
    bounded(f.storage.inject_write_failure(1)).expect("arm");
    assert_eq!(
        bounded(f.storage.archive_choir(f.youth.id)).map(|_| ()),
        injected
    );
    assert_eq!(library(&f.storage), before);

    // restore: one write statement, on an archived record.
    bounded(f.storage.archive_choir(f.adult.id)).expect("archive");
    let archived = library(&f.storage);
    bounded(f.storage.inject_write_failure(1)).expect("arm");
    assert_eq!(
        bounded(f.storage.restore_choir(f.adult.id)).map(|_| ()),
        injected
    );
    assert_eq!(library(&f.storage), archived);
    bounded(f.storage.restore_choir(f.adult.id)).expect("restore");
    assert_eq!(library(&f.storage), before);

    // set_pagination_state: one write statement.
    bounded(f.storage.inject_write_failure(1)).expect("arm");
    assert_eq!(
        bounded(
            f.storage
                .set_pagination_state(f.first.id, PaginationState::Approved)
        )
        .map(|_| ()),
        injected
    );
    assert_eq!(library(&f.storage), before);

    // edit_choir: UPDATE, DELETE, then one INSERT per language (2 here).
    let replacement = details("Changed", Some("C"), &["fil", "en"], Some("n"));
    for step in 1..=4 {
        bounded(f.storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            bounded(f.storage.edit_choir(f.youth.id, replacement.clone())).map(|_| ()),
            injected,
            "step {step}"
        );
        assert_eq!(library(&f.storage), before, "state after step {step}");
    }
    // A failure scripted past the last statement never fires.
    bounded(f.storage.inject_write_failure(5)).expect("arm");
    let edited = bounded(f.storage.edit_choir(f.youth.id, replacement)).expect("edit succeeds");
    assert_eq!(edited.name, "Changed");
    assert_eq!(edited.languages, vec!["fil".to_owned(), "en".to_owned()]);
    shutdown(f.storage);
}

fn archive_of(at: Option<Timestamp>) -> ArchiveState {
    at.map_or(ArchiveState::Active, ArchiveState::Archived)
}

/// Every field the records hold, read back through the hydrated domain
/// accessors.
fn assert_hydrated(library: &SongLibrary, records: &LibraryRecords) {
    assert_eq!(library.songs().count(), records.songs.len());
    assert_eq!(library.arrangements().count(), records.arrangements.len());
    assert_eq!(library.choirs().count(), records.choirs.len());
    for record in &records.songs {
        let song = library.song(record.id).expect("song hydrated");
        assert_eq!(song.title(), record.title);
        assert_eq!(song.aliases(), record.aliases);
        assert_eq!(
            song.original_attribution(),
            record.original_attribution.as_deref()
        );
        assert_eq!(song.provenance(), record.provenance.as_deref());
        assert_eq!(song.rights_notes(), record.rights_notes.as_deref());
        assert_eq!(
            song.liturgical_moments()
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            record.liturgical_moments
        );
        assert_eq!(
            song.occasions().iter().cloned().collect::<Vec<_>>(),
            record.occasions
        );
        assert_eq!(song.created_at(), record.created_at);
        assert_eq!(song.archive_state(), archive_of(record.archived_at));
    }
    for record in &records.choirs {
        let choir = library.choir(record.id).expect("choir hydrated");
        assert_eq!(choir.name(), record.name);
        assert_eq!(choir.short_label(), record.short_label.as_deref());
        assert_eq!(choir.languages(), record.languages);
        assert_eq!(choir.notes(), record.notes.as_deref());
        assert_eq!(choir.created_at(), record.created_at);
        assert_eq!(choir.archive_state(), archive_of(record.archived_at));
    }
    for record in &records.arrangements {
        let a = library
            .arrangement(record.id)
            .expect("arrangement hydrated");
        assert_eq!(a.song(), record.song);
        assert_eq!(a.title(), Some(record.title.as_str()));
        assert_eq!(a.languages(), record.languages);
        assert_eq!(
            a.choirs().iter().copied().collect::<Vec<_>>(),
            record.choirs
        );
        assert_eq!(
            a.rendition_attribution(),
            record.rendition_attribution.as_deref()
        );
        assert_eq!(
            a.occasions().iter().cloned().collect::<Vec<_>>(),
            record.occasions
        );
        assert_eq!(a.sections().len(), record.sections.len());
        for (got, want) in a.sections().iter().zip(&record.sections) {
            assert_eq!(got.id(), want.id);
            assert_eq!(got.label(), want.label.as_deref());
            assert_eq!(got.lyrics(), want.lyrics);
        }
        assert_eq!(a.sequence().len(), record.sequence.len());
        for (got, want) in a.sequence().iter().zip(&record.sequence) {
            assert_eq!(got.id(), want.id);
            assert_eq!(got.section(), want.section);
        }
        assert_eq!(a.pagination_state(), record.pagination);
        assert_eq!(a.created_at(), record.created_at);
        assert_eq!(a.archive_state(), archive_of(record.archived_at));
    }
}

#[test]
fn a_mixed_library_hydrates_to_exactly_what_was_written() {
    let dir = TempDir::new("hydrate");
    let f = fixture(&dir);
    bounded(f.storage.archive_choir(f.adult.id)).expect("archive choir");
    bounded(f.storage.archive_song(f.other_song.id)).expect("archive song");
    bounded(f.storage.archive_arrangement(f.second.id)).expect("archive arrangement");
    bounded(
        f.storage
            .set_pagination_state(f.first.id, PaginationState::Approved),
    )
    .expect("approve");
    shutdown(f.storage);

    let storage = open(&dir);
    let records = library(&storage);
    // The first Arrangement is shared between two Choirs, one of them archived.
    assert_eq!(records.arrangements.len(), 2);
    let hydrated = records.clone().hydrate().expect("stored library hydrates");
    assert_hydrated(&hydrated, &records);
    shutdown(storage);
}

fn draft_of(record: &ArrangementRecord) -> ArrangementDraft {
    ArrangementDraft {
        title: Some(record.title.clone()),
        languages: record.languages.clone(),
        rendition_attribution: record.rendition_attribution.clone(),
        notes: None,
        occasions: record.occasions.clone(),
        sections: record
            .sections
            .iter()
            .map(|s| SectionDraft {
                id: s.id,
                label: s.label.clone(),
                lyrics: s.lyrics.clone(),
            })
            .collect(),
        sequence: record
            .sequence
            .iter()
            .map(|o| OccurrenceDraft::new(o.id, o.section))
            .collect(),
    }
}

#[test]
fn identities_edited_away_stay_reserved_after_a_restart() {
    let dir = TempDir::new("reserved");
    let f = fixture(&dir);
    let removed_section = f.first.sections[1].id;
    let removed_occurrence = f.first.sequence[1].id;
    let mut content = ArrangementContent::of(&f.first);
    content.sections.truncate(1);
    content.sequence.retain(|o| o.id != removed_occurrence);
    let head = bounded(
        f.storage
            .latest_revision(RevisionEntity::Arrangement(f.first.id)),
    )
    .expect("head");
    bounded(f.storage.commit_arrangement_edit(
        f.first.id,
        content,
        PaginationState::NeedsReview,
        RevisionKind::Edit,
        Some(HistorySessionId::from_entity(entity_id(7))),
        head,
    ))
    .expect("edit away");
    shutdown(f.storage);

    let storage = open(&dir);
    let records = library(&storage);
    let reserved = records
        .reserved
        .iter()
        .find(|r| r.arrangement == f.first.id)
        .expect("reservation");
    assert!(reserved.sections.contains(&removed_section));
    assert!(reserved.occurrences.contains(&removed_occurrence));
    let mut domain = records.hydrate().expect("hydrates");

    // Another Arrangement may not take the identities.
    let fresh_section = SectionId::from_entity(entity_id(0xA1));
    let steal_section = ArrangementDraft {
        sections: vec![SectionDraft::new(removed_section, "x")],
        ..ArrangementDraft::in_language("English")
    };
    assert_eq!(
        domain
            .create_arrangement(
                ArrangementId::from_entity(entity_id(0xA3)),
                f.song.id,
                fixed_now(),
                steal_section,
                Vec::new(),
            )
            .map(|_| ()),
        Err(SongLibraryError::DuplicateSection(removed_section))
    );
    let steal_occurrence = ArrangementDraft {
        sections: vec![SectionDraft::new(fresh_section, "x")],
        sequence: vec![OccurrenceDraft::new(removed_occurrence, fresh_section)],
        ..ArrangementDraft::in_language("English")
    };
    assert_eq!(
        domain
            .create_arrangement(
                ArrangementId::from_entity(entity_id(0xA4)),
                f.song.id,
                fixed_now(),
                steal_occurrence,
                Vec::new(),
            )
            .map(|_| ()),
        Err(SongLibraryError::DuplicateOccurrence(removed_occurrence))
    );

    // The owner may bring them back.
    let mut restored = draft_of(&f.first);
    restored.title = Some("Back".to_owned());
    domain
        .edit_arrangement(f.first.id, restored)
        .expect("owner reuses its identities");
    shutdown(storage);
}

#[test]
fn a_damaged_revision_fails_the_library_read() {
    let dir = TempDir::new("damaged");
    let f = fixture(&dir);
    shutdown(f.storage);
    let conn = Connection::open(dir.db()).expect("open raw");
    let damaged = conn
        .execute(
            "UPDATE edit_revision SET content = content || ' ' WHERE arrangement_id = ?1",
            params![f.first.id.to_string()],
        )
        .expect("damage");
    assert!(damaged > 0);
    drop(conn);
    let storage = open(&dir);
    assert!(matches!(
        bounded(storage.library()),
        Err(StorageError::RevisionIntegrity { .. })
    ));
    shutdown(storage);
}

#[test]
fn an_arrangement_edit_and_restore_write_pagination_in_the_same_transaction() {
    let dir = TempDir::new("pagination");
    let f = fixture(&dir);
    let entity = RevisionEntity::Arrangement(f.first.id);
    let creation = bounded(f.storage.latest_revision(entity)).expect("head");
    let mut content = ArrangementContent::of(&f.first);
    content.title = "Edited".to_owned();
    let injected = StorageError::Sqlite("injected write failure".to_owned());

    // Commit: every injected failure leaves content, history and pagination alone.
    let before = library(&f.storage);
    let history = bounded(f.storage.revisions(entity)).expect("chain");
    let mut step = 1;
    let committed = loop {
        bounded(f.storage.inject_write_failure(step)).expect("arm");
        match bounded(f.storage.commit_arrangement_edit(
            f.first.id,
            content.clone(),
            PaginationState::Approved,
            RevisionKind::Edit,
            Some(HistorySessionId::from_entity(entity_id(7))),
            creation,
        )) {
            Err(error) => {
                assert_eq!(error, injected);
                assert_eq!(library(&f.storage), before, "after failure at {step}");
                assert_eq!(
                    bounded(f.storage.revisions(entity)).expect("chain"),
                    history
                );
                step += 1;
            }
            Ok(done) => break done,
        }
    };
    assert!(step > 2, "the commit has several write steps");
    assert_eq!(committed.0.pagination, PaginationState::Approved);
    assert_eq!(committed.0.title, "Edited");

    // Restore the creation revision: same guarantee.
    let before = library(&f.storage);
    let history = bounded(f.storage.revisions(entity)).expect("chain");
    let mut step = 1;
    let restored = loop {
        bounded(f.storage.inject_write_failure(step)).expect("arm");
        match bounded(f.storage.restore_revision(
            creation,
            committed.1.id,
            PaginationState::NeedsReview,
        )) {
            Err(error) => {
                assert_eq!(error, injected);
                assert_eq!(library(&f.storage), before, "after failure at {step}");
                assert_eq!(
                    bounded(f.storage.revisions(entity)).expect("chain"),
                    history
                );
                step += 1;
            }
            Ok(done) => break done,
        }
    };
    assert!(step > 2);
    let after = bounded(f.storage.arrangement(f.first.id)).expect("read");
    assert_eq!(after.pagination, PaginationState::NeedsReview);
    assert_eq!(after.title, f.first.title);
    assert_ne!(restored.1.id, committed.1.id);
    shutdown(f.storage);
}

#[test]
fn library_lists_follow_creation_order_and_archive_times_come_from_the_clock() {
    let dir = TempDir::new("order");
    // Scripted ids descend, so identity order opposes creation order.
    let ids: Vec<[u8; 16]> = (0..120u8).map(|i| [250 - i; 16]).collect();
    let start = Timestamp::from_millis(1_700_000_000_000);
    let storage = Storage::spawn_with_stepping_clock(config(&dir.db()), ids, start, 1_000);
    bounded(storage.ready()).expect("ready");

    let mut choirs = Vec::new();
    let mut songs = Vec::new();
    let mut arrangements = Vec::new();
    for name in ["C1", "C2", "C3"] {
        choirs.push(
            bounded(storage.create_choir(name_only(name)))
                .expect("choir")
                .id,
        );
    }
    for title in ["S1", "S2", "S3"] {
        songs.push(
            bounded(storage.create_song(song_input(title)))
                .expect("song")
                .id,
        );
    }
    for title in ["A1", "A2", "A3"] {
        arrangements.push(
            bounded(storage.create_arrangement(arrangement_input(songs[0], title, Vec::new())))
                .expect("arrangement")
                .id,
        );
    }
    assert!(choirs.windows(2).all(|w| w[0] > w[1]), "ids descend");

    let archived = bounded(storage.archive_choir(choirs[0])).expect("archive");
    let archived_at = archived.archived_at.expect("archived");
    assert!(
        archived_at > archived.created_at,
        "worker clock, not creation"
    );

    let records = library(&storage);
    assert_eq!(
        records.choirs.iter().map(|c| c.id).collect::<Vec<_>>(),
        choirs
    );
    assert_eq!(
        records.songs.iter().map(|s| s.id).collect::<Vec<_>>(),
        songs
    );
    assert_eq!(
        records
            .arrangements
            .iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        arrangements
    );
    assert!(
        records
            .choirs
            .windows(2)
            .all(|w| w[0].created_at < w[1].created_at)
    );
    assert!(
        records
            .songs
            .windows(2)
            .all(|w| w[0].created_at < w[1].created_at)
    );
    assert!(
        records
            .arrangements
            .windows(2)
            .all(|w| w[0].created_at < w[1].created_at)
    );
    shutdown(storage);
}

#[test]
fn create_choir_language_inserts_are_atomic_with_the_choir() {
    let dir = TempDir::new("choir-steps");
    let storage = open(&dir);
    let input = || NewChoir {
        name: "Two".to_owned(),
        short_label: None,
        languages: vec!["ceb".to_owned(), "en".to_owned()],
        notes: None,
    };
    // Step 1 is the choir row, steps 2 and 3 the language rows.
    for step in 1..=3 {
        bounded(storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            bounded(storage.create_choir(input())).map(|_| ()),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "step {step}"
        );
        assert_eq!(library(&storage).choirs, Vec::new(), "after step {step}");
    }
    bounded(storage.inject_write_failure(4)).expect("arm");
    let created = bounded(storage.create_choir(input())).expect("past the last step");
    assert_eq!(created.languages, vec!["ceb".to_owned(), "en".to_owned()]);
    assert_eq!(library(&storage).choirs, vec![created]);
    shutdown(storage);
}

fn entity_id(byte: u8) -> EntityId {
    EntityId::from_random_bytes([byte; 16])
}

fn hex_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[test]
fn v4_database_with_t2_1_data_upgrades_to_v5() {
    let dir = TempDir::new("upgrade");
    let stamp = "2023-11-14T22:13:20.000Z";
    let bundled_v4 = [
        (
            "0001_parish_profile",
            include_str!("../../../migrations/0001_parish_profile.sql"),
        ),
        (
            "0002_parish_note",
            include_str!("../../../migrations/0002_parish_note.sql"),
        ),
        (
            "0003_parish_foundation",
            include_str!("../../../migrations/0003_parish_foundation.sql"),
        ),
        (
            "0004_edit_revisions",
            include_str!("../../../migrations/0004_edit_revisions.sql"),
        ),
    ];
    let choir = ChoirId::from_entity(entity_id(0x60));
    let song = SongId::from_entity(entity_id(0x61));
    let arrangement = ArrangementId::from_entity(entity_id(0x62));
    let (first, second) = (entity_id(0x63), entity_id(0x64));
    {
        // A version-4 database exactly as the migration runner leaves it.
        let conn = Connection::open(dir.db()).expect("raw connection");
        conn.execute_batch(
            "CREATE TABLE ordo_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL, \
             checksum TEXT NOT NULL, applied_at TEXT NOT NULL)",
        )
        .expect("ledger");
        for (index, (name, sql)) in bundled_v4.iter().enumerate() {
            conn.execute_batch(sql).expect("apply migration");
            conn.execute(
                "INSERT INTO ordo_migrations (id, name, checksum, applied_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![index as i64 + 1, name, hex_sha256(sql), stamp],
            )
            .expect("ledger row");
        }
        conn.pragma_update(None, "user_version", 4)
            .expect("version");

        conn.execute(
            "INSERT INTO choir (id, name, created_at) VALUES (?1, 'Legacy Choir', ?2)",
            params![choir.to_string(), stamp],
        )
        .expect("choir");
        conn.execute(
            "INSERT INTO song (id, title, created_at) VALUES (?1, 'Legacy', ?2)",
            params![song.to_string(), stamp],
        )
        .expect("song");
        conn.execute(
            "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, 'SATB', ?3)",
            params![arrangement.to_string(), song.to_string(), stamp],
        )
        .expect("arrangement");
        conn.execute(
            "INSERT INTO arrangement_choir (arrangement_id, choir_id) VALUES (?1, ?2)",
            params![arrangement.to_string(), choir.to_string()],
        )
        .expect("link");
        for (position, (id, lyrics)) in [(first, "a"), (second, "b")].into_iter().enumerate() {
            conn.execute(
                "INSERT INTO arrangement_section \
                 (id, arrangement_id, position, label, lyrics, created_at) \
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
                params![
                    id.to_string(),
                    arrangement.to_string(),
                    position as i64,
                    lyrics,
                    stamp
                ],
            )
            .expect("section");
        }
    }

    // Opening migrates 4 -> 5 and the legacy rows read back with defaults.
    let storage = Storage::spawn(config(&dir.db()));
    let status = bounded(storage.ready()).expect("ready");
    assert_eq!(status.version, 5);
    let all = library(&storage);
    assert_eq!(all.choirs.len(), 1);
    assert_eq!(all.choirs[0].name, "Legacy Choir");
    assert_eq!(all.choirs[0].short_label, None);
    assert_eq!(all.choirs[0].languages, Vec::<String>::new());
    assert_eq!(all.choirs[0].notes, None);
    assert_eq!(all.choirs[0].archived_at, None);
    assert_eq!(all.songs.len(), 1);
    assert_eq!(all.songs[0].title, "Legacy");
    assert_eq!(all.arrangements.len(), 1);
    assert_eq!(all.arrangements[0].pagination, PaginationState::Draft);
    assert_eq!(all.arrangements[0].choirs, vec![choir]);
    assert_eq!(all.arrangements[0].sections.len(), 2);

    // The new columns and table are usable on the upgraded database.
    let edited = bounded(storage.edit_choir(
        choir,
        details("Legacy Choir", Some("LC"), &["en"], Some("note")),
    ))
    .expect("edit");
    assert_eq!(edited.languages, vec!["en".to_owned()]);
    let approved = bounded(storage.set_pagination_state(arrangement, PaginationState::Approved))
        .expect("approve");
    assert_eq!(approved.pagination, PaginationState::Approved);
    shutdown(storage);
}
