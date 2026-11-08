//! T2.1b parish store acceptance tests. Every await runs under a 10 s bound so
//! a hang fails the test instead of blocking the run. Databases live in unique
//! temporary directories.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::{ChoirId, EntityId, SongId, Timestamp};
use futures_lite::future::block_on;
use storage::{
    ArrangementRecord, NewArrangement, NewChoir, NewParish, NewSection, NewSong, RecordKind,
    ShutdownOutcome, Storage, StorageConfig, StorageError,
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
            "ordo-parish-store-tests-{tag}-{}-{}-{nanos}",
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
        // Windows can hold a handle to a just-closed database for a moment, so
        // removal is retried briefly rather than failing or leaking the directory.
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

/// The fixed clock used by stores that script their sources.
fn fixed_now() -> Timestamp {
    Timestamp::from_millis(1_700_000_000_000)
}

fn open(dir: &TempDir) -> Storage {
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("storage ready");
    storage
}

fn open_scripted(dir: &TempDir, ids: Vec<[u8; 16]>) -> Storage {
    let storage = Storage::spawn_with_sources(config(&dir.db()), ids, fixed_now());
    bounded(storage.ready()).expect("storage ready");
    storage
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

fn section(label: Option<&str>, lyrics: &str) -> NewSection {
    NewSection {
        label: label.map(str::to_owned),
        lyrics: lyrics.to_owned(),
    }
}

/// An arrangement with the given song and otherwise minimal valid fields.
fn arrangement_input(song: SongId) -> NewArrangement {
    NewArrangement {
        song,
        title: "Plainchant".to_owned(),
        languages: Vec::new(),
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        sections: Vec::new(),
        sequence: Vec::new(),
    }
}

fn unknown_song() -> SongId {
    SongId::from_entity(EntityId::from_random_bytes([7; 16]))
}

fn invalid(field: &'static str, reason: &'static str) -> StorageError {
    StorageError::InvalidInput { field, reason }
}

#[test]
fn created_records_rehydrate_exactly_after_restart() {
    let dir = TempDir::new("rehydrate");
    let storage = open(&dir);

    let parish = bounded(storage.create_parish(NewParish {
        name: "St Mary".to_owned(),
    }))
    .expect("parish");
    let first_choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("choir");
    let second_choir = bounded(storage.create_choir(NewChoir {
        name: "Youth".to_owned(),
    }))
    .expect("choir");
    let song = bounded(storage.create_song(NewSong {
        title: "Kyrie".to_owned(),
        aliases: vec!["Lord have mercy".to_owned(), "Kyrie eleison".to_owned()],
        original_attribution: Some("Anon".to_owned()),
        provenance: Some("Hymnal".to_owned()),
        rights_notes: Some("Public domain".to_owned()),
        liturgical_moments: vec!["Penitential Act".to_owned(), "Entrance".to_owned()],
        occasions: vec!["Lent".to_owned(), "Advent".to_owned()],
    }))
    .expect("song");
    let arrangement = bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: "SATB".to_owned(),
        languages: vec!["en".to_owned(), "la".to_owned()],
        choirs: vec![second_choir.id, first_choir.id],
        rendition_attribution: Some("Arr. Smith".to_owned()),
        occasions: vec!["Lent".to_owned(), "Advent".to_owned()],
        sections: vec![
            section(Some("Verse"), "Line one\nLine two"),
            section(None, "Unlabelled"),
            section(Some("Refrain"), "Refrain text"),
        ],
        sequence: vec![0, 1, 0, 2, 0],
    }))
    .expect("arrangement");
    assert_eq!(arrangement.sequence.len(), 5);
    assert_eq!(arrangement.sections[1].label, None);
    assert_eq!(arrangement.sequence[0].section, arrangement.sections[0].id);
    assert_eq!(arrangement.sequence[2].section, arrangement.sections[0].id);
    assert_ne!(arrangement.sequence[0].id, arrangement.sequence[2].id);

    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    let reopened = open(&dir);
    assert_eq!(bounded(reopened.parish()), Ok(Some(parish)));
    assert_eq!(
        bounded(reopened.choirs()),
        Ok(vec![first_choir, second_choir])
    );
    assert_eq!(bounded(reopened.song(song.id)), Ok(song.clone()));
    assert_eq!(bounded(reopened.songs()), Ok(vec![song.clone()]));
    assert_eq!(
        bounded(reopened.arrangement(arrangement.id)),
        Ok(arrangement.clone())
    );
    assert_eq!(
        bounded(reopened.arrangements_for_song(song.id)),
        Ok(vec![arrangement])
    );
    assert_eq!(bounded(reopened.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn ids_are_canonical_v4_and_times_explicit() {
    let dir = TempDir::new("ids");
    let storage = open_scripted(&dir, Vec::new());

    let parish = bounded(storage.create_parish(NewParish {
        name: "St Mary".to_owned(),
    }))
    .expect("parish");
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("choir");
    let song = bounded(storage.create_song(song_input("Kyrie"))).expect("song");
    let mut input = arrangement_input(song.id);
    input.sections = vec![section(None, "a"), section(None, "b")];
    input.sequence = vec![0, 1];
    let arrangement = bounded(storage.create_arrangement(input)).expect("arrangement");

    let mut ids = vec![
        parish.id.to_string(),
        choir.id.to_string(),
        song.id.to_string(),
        arrangement.id.to_string(),
    ];
    ids.extend(arrangement.sections.iter().map(|s| s.id.to_string()));
    ids.extend(arrangement.sequence.iter().map(|o| o.id.to_string()));
    for text in &ids {
        assert!(EntityId::parse(text).is_ok(), "{text} is canonical");
        assert_eq!(text.as_bytes()[14], b'4', "{text} is version 4");
    }

    assert_eq!(parish.created_at, fixed_now());
    assert_eq!(choir.created_at, fixed_now());
    assert_eq!(song.created_at, fixed_now());
    assert_eq!(arrangement.created_at, fixed_now());
    assert!(
        arrangement
            .sections
            .iter()
            .all(|s| s.created_at == fixed_now())
    );
    assert!(
        arrangement
            .sequence
            .iter()
            .all(|o| o.created_at == fixed_now())
    );
    assert_eq!(choir.archived_at, None);
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn same_title_never_unites_songs() {
    let dir = TempDir::new("same-title");
    let storage = open(&dir);
    let first = bounded(storage.create_song(song_input("Ave Maria"))).expect("first");
    let second = bounded(storage.create_song(song_input("Ave Maria"))).expect("second");
    assert_ne!(first.id, second.id);

    let songs = bounded(storage.songs()).expect("songs");
    assert_eq!(songs.len(), 2);
    assert!(songs.contains(&first));
    assert!(songs.contains(&second));
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn authored_text_is_stored_verbatim() {
    let dir = TempDir::new("verbatim");
    let storage = open(&dir);
    let hostile = "Robert'); DROP TABLE song;--";

    let mut spaced = song_input("  Kyrie 8th Mass ");
    spaced.aliases = vec!["KYRIE viii".to_owned(), "  spaced alias  ".to_owned()];
    spaced.original_attribution = Some(" Anon ".to_owned());
    let spaced = bounded(storage.create_song(spaced)).expect("spaced");
    let hostile = bounded(storage.create_song(song_input(hostile))).expect("hostile");

    let read_spaced = bounded(storage.song(spaced.id)).expect("read spaced");
    assert_eq!(read_spaced.title, "  Kyrie 8th Mass ");
    assert_eq!(read_spaced.aliases, vec!["KYRIE viii", "  spaced alias  "]);
    assert_eq!(read_spaced.original_attribution.as_deref(), Some(" Anon "));
    assert_eq!(read_spaced, spaced);
    let read_hostile = bounded(storage.song(hostile.id)).expect("read hostile");
    assert_eq!(read_hostile.title, "Robert'); DROP TABLE song;--");
    assert_eq!(read_hostile, hostile);
    assert_eq!(bounded(storage.songs()).expect("songs").len(), 2);
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn invalid_input_writes_nothing() {
    let dir = TempDir::new("invalid");
    let storage = open(&dir);
    let song = bounded(storage.create_song(song_input("Kyrie"))).expect("song");
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("choir");

    let songs_before = bounded(storage.songs()).expect("songs");
    let choirs_before = bounded(storage.choirs()).expect("choirs");
    let arrangements_before = bounded(storage.arrangements_for_song(song.id)).expect("list");

    // Songs.
    let mut cases: Vec<(NewSong, StorageError)> = Vec::new();
    cases.push((song_input(" \t "), invalid("title", "blank")));
    let mut blank_alias = song_input("T");
    blank_alias.aliases = vec!["ok".to_owned(), "  ".to_owned()];
    cases.push((blank_alias, invalid("alias", "blank")));
    let mut blank_moment = song_input("T");
    blank_moment.liturgical_moments = vec![String::new()];
    cases.push((blank_moment, invalid("liturgical_moment", "blank")));
    let mut duplicate_moment = song_input("T");
    duplicate_moment.liturgical_moments = vec!["Entrance".to_owned(), "Entrance".to_owned()];
    cases.push((duplicate_moment, invalid("liturgical_moment", "duplicate")));
    let mut blank_occasion = song_input("T");
    blank_occasion.occasions = vec![" ".to_owned()];
    cases.push((blank_occasion, invalid("occasion", "blank")));
    let mut duplicate_occasion = song_input("T");
    duplicate_occasion.occasions = vec!["Lent".to_owned(), "Lent".to_owned()];
    cases.push((duplicate_occasion, invalid("occasion", "duplicate")));
    for (input, expected) in cases {
        assert_eq!(bounded(storage.create_song(input)), Err(expected));
    }

    // Names.
    assert_eq!(
        bounded(storage.create_choir(NewChoir {
            name: "   ".to_owned()
        })),
        Err(invalid("name", "blank"))
    );
    assert_eq!(
        bounded(storage.create_parish(NewParish {
            name: "\n".to_owned()
        })),
        Err(invalid("name", "blank"))
    );

    // Arrangements.
    let mut arrangement_cases: Vec<(NewArrangement, StorageError)> = Vec::new();
    let mut blank_title = arrangement_input(song.id);
    blank_title.title = "  ".to_owned();
    arrangement_cases.push((blank_title, invalid("title", "blank")));
    let mut blank_language = arrangement_input(song.id);
    blank_language.languages = vec![" ".to_owned()];
    arrangement_cases.push((blank_language, invalid("language", "blank")));
    let mut duplicate_language = arrangement_input(song.id);
    duplicate_language.languages = vec!["en".to_owned(), "en".to_owned()];
    arrangement_cases.push((duplicate_language, invalid("language", "duplicate")));
    let mut blank_occasion = arrangement_input(song.id);
    blank_occasion.occasions = vec![String::new()];
    arrangement_cases.push((blank_occasion, invalid("occasion", "blank")));
    let mut duplicate_occasion = arrangement_input(song.id);
    duplicate_occasion.occasions = vec!["Lent".to_owned(), "Lent".to_owned()];
    arrangement_cases.push((duplicate_occasion, invalid("occasion", "duplicate")));
    let mut blank_label = arrangement_input(song.id);
    blank_label.sections = vec![section(Some("  "), "lyrics")];
    arrangement_cases.push((blank_label, invalid("label", "blank")));
    let mut duplicate_choir = arrangement_input(song.id);
    duplicate_choir.choirs = vec![choir.id, choir.id];
    arrangement_cases.push((duplicate_choir, invalid("choir", "duplicate")));
    let mut bad_sequence = arrangement_input(song.id);
    bad_sequence.sections = vec![section(None, "a")];
    bad_sequence.sequence = vec![0, 1];
    arrangement_cases.push((bad_sequence, invalid("sequence", "unknown section")));
    for (input, expected) in arrangement_cases {
        assert_eq!(bounded(storage.create_arrangement(input)), Err(expected));
    }

    assert_eq!(bounded(storage.songs()), Ok(songs_before));
    assert_eq!(bounded(storage.choirs()), Ok(choirs_before));
    assert_eq!(bounded(storage.parish()), Ok(None));
    assert_eq!(
        bounded(storage.arrangements_for_song(song.id)),
        Ok(arrangements_before)
    );
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn missing_choir_or_song_rolls_back() {
    let dir = TempDir::new("missing");
    let storage = open(&dir);
    let song = bounded(storage.create_song(song_input("Kyrie"))).expect("song");
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("choir");
    let ghost_choir = ChoirId::from_entity(EntityId::from_random_bytes([9; 16]));

    let mut with_ghost_choir = arrangement_input(song.id);
    with_ghost_choir.choirs = vec![choir.id, ghost_choir];
    with_ghost_choir.sections = vec![section(None, "a")];
    with_ghost_choir.sequence = vec![0];
    assert_eq!(
        bounded(storage.create_arrangement(with_ghost_choir)),
        Err(StorageError::MissingReference {
            kind: RecordKind::Choir,
            id: ghost_choir.entity(),
        })
    );

    let ghost_song = unknown_song();
    assert_eq!(
        bounded(storage.create_arrangement(arrangement_input(ghost_song))),
        Err(StorageError::MissingReference {
            kind: RecordKind::Song,
            id: ghost_song.entity(),
        })
    );

    assert_eq!(bounded(storage.arrangements_for_song(song.id)), Ok(vec![]));
    assert_eq!(
        bounded(storage.arrangements_for_song(ghost_song)),
        Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: ghost_song.entity(),
        })
    );
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

/// The write statements a create of `injected_input` performs:
/// 1 arrangement + 1 language + 2 choir links + 1 occasion + 3 sections +
/// 5 occurrences.
const ARRANGEMENT_WRITE_STATEMENTS: usize = 1 + 1 + 2 + 1 + 3 + 5;

#[test]
fn injected_failure_mid_arrangement_preserves_prior_state() {
    let dir = TempDir::new("injected");
    let storage = open(&dir);
    let song = bounded(storage.create_song(song_input("Kyrie"))).expect("song");
    let first_choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("choir");
    let second_choir = bounded(storage.create_choir(NewChoir {
        name: "Youth".to_owned(),
    }))
    .expect("choir");

    let injected_input = || NewArrangement {
        song: song.id,
        title: "SATB".to_owned(),
        languages: vec!["en".to_owned()],
        choirs: vec![first_choir.id, second_choir.id],
        rendition_attribution: None,
        occasions: vec!["Lent".to_owned()],
        sections: vec![
            section(Some("A"), "a"),
            section(None, "b"),
            section(None, "c"),
        ],
        sequence: vec![0, 1, 0, 2, 0],
    };
    let mut good = arrangement_input(song.id);
    good.title = "Good".to_owned();
    good.sections = vec![section(None, "kept")];
    good.sequence = vec![0];
    let good = bounded(storage.create_arrangement(good)).expect("good arrangement");
    let before: Vec<ArrangementRecord> = vec![good];
    assert_eq!(
        bounded(storage.arrangements_for_song(song.id)),
        Ok(before.clone())
    );

    for step in 1..=ARRANGEMENT_WRITE_STATEMENTS {
        bounded(storage.inject_write_failure(step)).expect("arm hook");
        assert_eq!(
            bounded(storage.create_arrangement(injected_input())),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "step {step}"
        );
        assert_eq!(
            bounded(storage.arrangements_for_song(song.id)),
            Ok(before.clone()),
            "state after failure at step {step}"
        );
    }

    // A failure scripted past the last statement never fires, which proves the
    // documented count is exact: the create succeeds and adds one arrangement.
    bounded(storage.inject_write_failure(ARRANGEMENT_WRITE_STATEMENTS + 1)).expect("arm hook");
    let created = bounded(storage.create_arrangement(injected_input())).expect("create succeeds");
    let after = bounded(storage.arrangements_for_song(song.id)).expect("list");
    assert_eq!(after.len(), 2);
    assert!(after.contains(&created));
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn colliding_id_is_regenerated() {
    let dir = TempDir::new("collision");
    let duplicate = [0x11; 16];
    let storage = open_scripted(&dir, vec![duplicate, duplicate, [0x22; 16]]);

    let first = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("first choir");
    let second = bounded(storage.create_choir(NewChoir {
        name: "Youth".to_owned(),
    }))
    .expect("second choir");

    assert_eq!(first.id.entity(), EntityId::from_random_bytes(duplicate));
    assert_ne!(second.id, first.id);
    assert_eq!(second.id.entity(), EntityId::from_random_bytes([0x22; 16]));
    assert_eq!(bounded(storage.choirs()).expect("choirs").len(), 2);
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn exhausted_collisions_fail_cleanly() {
    let dir = TempDir::new("exhausted");
    let storage = open_scripted(&dir, vec![[0x33; 16]; 9]);

    let first = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
    }))
    .expect("first choir");
    assert_eq!(
        bounded(storage.create_choir(NewChoir {
            name: "Youth".to_owned()
        })),
        Err(StorageError::IdentityExhausted {
            kind: RecordKind::Choir
        })
    );
    assert_eq!(bounded(storage.choirs()), Ok(vec![first]));
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn create_parish_twice_is_refused() {
    let dir = TempDir::new("parish-twice");
    let storage = open(&dir);
    let first = bounded(storage.create_parish(NewParish {
        name: "St Mary".to_owned(),
    }))
    .expect("first");
    assert_eq!(
        bounded(storage.create_parish(NewParish {
            name: "St John".to_owned()
        })),
        Err(StorageError::ParishExists)
    );
    assert_eq!(bounded(storage.parish()), Ok(Some(first)));
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn not_found_reads() {
    let dir = TempDir::new("not-found");
    let storage = open(&dir);
    let song = unknown_song();
    let arrangement =
        domain::identity::ArrangementId::from_entity(EntityId::from_random_bytes([5; 16]));
    assert_eq!(
        bounded(storage.song(song)),
        Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: song.entity(),
        })
    );
    assert_eq!(
        bounded(storage.arrangement(arrangement)),
        Err(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: arrangement.entity(),
        })
    );
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}
