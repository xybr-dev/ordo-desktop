//! T2.9 WP3 storage acceptance tests: archive independence, permanent deletion
//! and reference safety (AC4, AC6, AC9). Every await runs under a 10 s bound so
//! a hang fails the test instead of blocking the run. Databases live in unique
//! temporary directories.

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::{ChoirId, EntityId, HistorySessionId, SongId};
use domain::revisions::RevisionEntity;
use domain::song_library::{DurableReference, RecordRef};
use futures_lite::future::block_on;
use rusqlite::types::Value;
use rusqlite::{Connection, params};
use storage::{
    ArrangementContent, ArrangementRecord, Deleted, HistoryRevision, NewArrangement, NewChoir,
    NewSection, NewSong, RecordKind, RevisionRecord, ShutdownOutcome, SongRecord, Storage,
    StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);

/// Runs `work` on its own thread; returns a receiver so the caller can bound
/// the wait.
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
            "ordo-lifecycle-tests-{tag}-{}-{}-{nanos}",
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

fn open(dir: &TempDir) -> Storage {
    let storage = Storage::spawn(StorageConfig::new(dir.db()));
    bounded(storage.ready()).expect("storage ready");
    storage
}

fn shutdown(storage: Storage) {
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

fn raw(dir: &TempDir) -> Connection {
    Connection::open(dir.db()).expect("open raw connection")
}

fn entity_id(byte: u8) -> EntityId {
    EntityId::from_random_bytes([byte; 16])
}

fn session(byte: u8) -> HistorySessionId {
    HistorySessionId::from_entity(entity_id(byte))
}

fn new_section(label: &str, lyrics: &str) -> NewSection {
    NewSection {
        label: Some(label.to_owned()),
        lyrics: lyrics.to_owned(),
    }
}

/// Every user table, ordered by name.
fn table_names(conn: &Connection) -> Vec<String> {
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("table list");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("table query")
        .map(|row| row.expect("table name"))
        .collect()
}

/// Every row of every user table except the excluded names, ordered by all
/// columns so two snapshots compare deterministically.
fn snapshot(conn: &Connection, excluded: &[&str]) -> BTreeMap<String, Vec<Vec<Value>>> {
    let mut tables = BTreeMap::new();
    for table in table_names(conn) {
        if excluded.contains(&table.as_str()) {
            continue;
        }
        let mut info = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .expect("table info");
        let columns: Vec<String> = info
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column query")
            .map(|row| row.expect("column name"))
            .collect();
        let projection = columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let order = (1..=columns.len())
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut statement = conn
            .prepare(&format!(
                "SELECT {projection} FROM \"{table}\" ORDER BY {order}"
            ))
            .expect("snapshot query");
        let rows: Vec<Vec<Value>> = statement
            .query_map([], |row| {
                (0..columns.len())
                    .map(|index| row.get::<_, Value>(index))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("snapshot rows")
            .map(|row| row.expect("snapshot row"))
            .collect();
        tables.insert(table, rows);
    }
    tables
}

/// How many rows of `table` name `id` in `column`.
fn rows_naming(conn: &Connection, table: &str, column: &str, id: impl ToString) -> i64 {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM \"{table}\" WHERE \"{column}\" = ?1"),
        params![id.to_string()],
        |row| row.get(0),
    )
    .expect("count rows")
}

/// A Song with aliases, moments and an occasion, so every Song-owned table has
/// rows to erase.
fn song_input(title: &str) -> NewSong {
    NewSong {
        title: title.to_owned(),
        aliases: vec!["Bread from Heaven".to_owned(), "Tinapay".to_owned()],
        original_attribution: Some("Traditional".to_owned()),
        provenance: Some("Cebuano hymnal".to_owned()),
        rights_notes: Some("Public domain".to_owned()),
        liturgical_moments: vec!["Advent".to_owned(), "Lent".to_owned()],
        occasions: vec!["Christmas".to_owned()],
    }
}

/// A Song with two Arrangements; the first carries a Choir, an occasion,
/// notes, two sections and three occurrences.
fn seeded(storage: &Storage) -> (SongId, ArrangementRecord, ArrangementRecord) {
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Koro".to_owned(),
        short_label: None,
        languages: vec!["Cebuano".to_owned()],
        notes: None,
    }))
    .expect("choir");
    let song = bounded(storage.create_song(song_input("Tinapay sa Langit")))
        .expect("song")
        .id;
    let first = bounded(storage.create_arrangement(NewArrangement {
        song,
        title: Some("Cebuano SATB".to_owned()),
        languages: vec!["Cebuano".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: Some("Arr. Cruz".to_owned()),
        occasions: vec!["Christmas".to_owned()],
        notes: Some("Public domain".to_owned()),
        sections: vec![
            new_section("Verse 1", "Tinapay sa langit"),
            new_section("Refrain", "Dayandayan ang gugma"),
        ],
        sequence: vec![0, 1, 0],
    }))
    .expect("first arrangement");
    let second = bounded(storage.create_arrangement(NewArrangement {
        song,
        title: Some("English SATB".to_owned()),
        languages: vec!["English".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![new_section("Verse", "Bread from heaven")],
        sequence: vec![0],
    }))
    .expect("second arrangement");
    (song, first, second)
}

/// Appends one lyrics revision so the Arrangement owns more than its creation.
fn add_lyrics_revision(storage: &Storage, record: &ArrangementRecord) -> RevisionRecord {
    let content = ArrangementContent::of(record);
    bounded(storage.commit_arrangement_lyrics(
        record.id,
        content.sections,
        content.sequence,
        session(0x40),
    ))
    .expect("lyrics revision")
    .1
}

fn archive_arrangement(storage: &Storage, record: &ArrangementRecord) -> ArrangementRecord {
    bounded(storage.archive_arrangement(record.id)).expect("archive arrangement")
}

fn archive_song(storage: &Storage, song: SongId) -> SongRecord {
    bounded(storage.archive_song(song)).expect("archive song")
}

fn latest_history(storage: &Storage, entity: RevisionEntity) -> Vec<HistoryRevision> {
    bounded(storage.history(entity)).expect("history")
}

#[test]
fn ac4_song_archive_writes_only_the_song_row() {
    let dir = TempDir::new("archive-song");
    let storage = open(&dir);
    let (song, _, _) = seeded(&storage);
    shutdown(storage);
    let before = snapshot(&raw(&dir), &[]);

    let storage = open(&dir);
    let archived = archive_song(&storage, song);
    assert!(archived.archived_at.is_some());
    shutdown(storage);

    let conn = raw(&dir);
    let after = snapshot(&conn, &[]);
    let mut before_without_song = before.clone();
    let mut after_without_song = after.clone();
    before_without_song.remove("song");
    after_without_song.remove("song");
    assert_eq!(
        before_without_song, after_without_song,
        "archiving a Song writes no other row"
    );
    assert_ne!(before["song"], after["song"], "the Song row changed");
    let arrangement_states: Vec<Option<String>> = conn
        .prepare("SELECT archived_at FROM arrangement ORDER BY id")
        .expect("arrangement states")
        .query_map([], |row| row.get(0))
        .expect("state query")
        .map(|row| row.expect("state"))
        .collect();
    assert!(
        arrangement_states.iter().all(Option::is_none),
        "a Song archive never changes a child's archive state"
    );
}

#[test]
fn ac6_delete_arrangement_refuses_while_a_reference_exists() {
    let dir = TempDir::new("reference-block");
    let storage = open(&dir);
    let (_, first, _) = seeded(&storage);
    archive_arrangement(&storage, &first);
    let reference = DurableReference::MassPlan(entity_id(0x90));
    bounded(storage.plant_reference(RecordRef::Arrangement(first.id), reference.clone()))
        .expect("plant");

    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Err(StorageError::DeletionBlocked {
            references: vec![reference.clone()],
            children: Vec::new(),
        })
    );
    let facts = bounded(storage.deletion_facts(RecordRef::Arrangement(first.id))).expect("facts");
    assert_eq!(facts.references, vec![reference]);
    assert_eq!(
        bounded(storage.arrangement(first.id)).map(|record| record.id),
        Ok(first.id),
        "a refused deletion changes nothing"
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Arrangement(first.id)))
            .expect("history")
            .len(),
        1
    );
    shutdown(storage);
}

#[test]
fn ac6_delete_arrangement_refuses_an_active_arrangement() {
    let dir = TempDir::new("active-arrangement");
    let storage = open(&dir);
    let (_, first, _) = seeded(&storage);

    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Err(StorageError::NotArchived {
            kind: RecordKind::Arrangement,
        })
    );
    assert_eq!(
        bounded(storage.arrangement(first.id)).map(|record| record.id),
        Ok(first.id),
        "the active Arrangement is untouched"
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Arrangement(first.id)))
            .expect("history")
            .len(),
        1
    );
    shutdown(storage);
}

#[test]
fn ac6_delete_song_refuses_while_any_arrangement_remains() {
    let dir = TempDir::new("song-children");
    let storage = open(&dir);
    let (song, first, second) = seeded(&storage);
    archive_song(&storage, song);
    // One child archived in its own right still counts.
    archive_arrangement(&storage, &first);

    let Err(StorageError::DeletionBlocked {
        references,
        children,
    }) = bounded(storage.delete_song(song))
    else {
        panic!("a Song that still owns an Arrangement is blocked");
    };
    assert!(references.is_empty());
    let mut children = children;
    children.sort();
    let mut expected = vec![first.id, second.id];
    expected.sort();
    assert_eq!(children, expected);
    assert!(bounded(storage.song(song)).is_ok(), "the Song is untouched");
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Song(song)))
            .expect("history")
            .len(),
        1
    );
    shutdown(storage);
}

#[test]
fn ac6_safe_deletion_removes_the_rows_and_only_the_owned_revisions() {
    let dir = TempDir::new("safe-delete");
    let storage = open(&dir);
    let (song, first, second) = seeded(&storage);
    add_lyrics_revision(&storage, &first);
    let song_history =
        bounded(storage.revisions(RevisionEntity::Song(song))).expect("song history");
    let sibling_history =
        bounded(storage.revisions(RevisionEntity::Arrangement(second.id))).expect("sibling");
    let sibling_head = sibling_history[0].id;
    archive_arrangement(&storage, &first);

    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Ok(Deleted { revisions: 2 }),
        "both of the Arrangement's own revisions are erased"
    );
    assert_eq!(
        bounded(storage.arrangement(first.id)),
        Err(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: first.id.entity(),
        })
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Arrangement(first.id))),
        Ok(Vec::new()),
        "no revision row remains for the deleted Arrangement"
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Song(song))),
        Ok(song_history)
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Arrangement(second.id))),
        Ok(sibling_history)
    );
    assert_eq!(
        bounded(storage.latest_revision(RevisionEntity::Arrangement(second.id)))
            .expect("sibling head"),
        sibling_head
    );
    shutdown(storage);

    let conn = raw(&dir);
    for table in ["arrangement_section", "section_occurrence"] {
        assert_eq!(
            rows_naming(&conn, table, "arrangement_id", first.id),
            0,
            "{table} still names the deleted Arrangement"
        );
    }
    for table in [
        "arrangement_language",
        "arrangement_choir",
        "arrangement_occasion",
        "edit_revision",
        "arrangement",
    ] {
        assert_eq!(
            rows_naming(&conn, table, "arrangement_id", first.id),
            0,
            "{table} still names the deleted Arrangement"
        );
    }
}

#[test]
fn ac6_deletion_never_touches_the_song_or_sibling_arrangements() {
    let dir = TempDir::new("siblings");
    let storage = open(&dir);
    let (song, first, second) = seeded(&storage);
    add_lyrics_revision(&storage, &second);
    let song_before = bounded(storage.song(song)).expect("song");
    let second_before = bounded(storage.arrangement(second.id)).expect("sibling");
    let second_history =
        bounded(storage.revisions(RevisionEntity::Arrangement(second.id))).expect("sibling");
    let song_history = bounded(storage.revisions(RevisionEntity::Song(song))).expect("song");

    archive_arrangement(&storage, &first);
    bounded(storage.delete_arrangement(first.id)).expect("delete");

    assert_eq!(bounded(storage.song(song)), Ok(song_before));
    assert_eq!(bounded(storage.arrangement(second.id)), Ok(second_before));
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Arrangement(second.id))),
        Ok(second_history)
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Song(song))),
        Ok(song_history)
    );
    shutdown(storage);
}

#[test]
fn ac6_every_table_naming_a_song_or_arrangement_is_cleared_or_blocks() {
    let dir = TempDir::new("foreign-keys");
    let storage = open(&dir);
    let (song, first, second) = seeded(&storage);
    for record in [&first, &second] {
        archive_arrangement(&storage, record);
        bounded(storage.delete_arrangement(record.id)).expect("delete arrangement");
    }
    archive_song(&storage, song);
    bounded(storage.delete_song(song)).expect("delete song");
    shutdown(storage);

    let conn = raw(&dir);
    let mut discovered: Vec<(String, String, String)> = Vec::new();
    for table in table_names(&conn) {
        let mut statement = conn
            .prepare(&format!("PRAGMA foreign_key_list(\"{table}\")"))
            .expect("foreign key list");
        let rows = statement
            .query_map([], |row| {
                Ok((
                    table.clone(),
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("foreign key query");
        for row in rows {
            let (table, from, referenced) = row.expect("foreign key");
            if referenced == "song" || referenced == "arrangement" {
                discovered.push((table, from, referenced));
            }
        }
    }
    discovered.sort();
    let mut expected: Vec<(String, String, String)> = [
        ("arrangement", "song_id", "song"),
        ("arrangement_choir", "arrangement_id", "arrangement"),
        ("arrangement_language", "arrangement_id", "arrangement"),
        ("arrangement_occasion", "arrangement_id", "arrangement"),
        ("arrangement_section", "arrangement_id", "arrangement"),
        ("edit_revision", "arrangement_id", "arrangement"),
        ("edit_revision", "song_id", "song"),
        ("section_occurrence", "arrangement_id", "arrangement"),
        ("song_alias", "song_id", "song"),
        ("song_liturgical_moment", "song_id", "song"),
        ("song_occasion", "song_id", "song"),
    ]
    .into_iter()
    .map(|(table, from, referenced)| (table.to_owned(), from.to_owned(), referenced.to_owned()))
    .collect();
    expected.sort();
    assert_eq!(
        discovered, expected,
        "every foreign key to song or arrangement is known to deletion"
    );

    for (table, column, referenced) in &discovered {
        let id = if referenced == "song" {
            song.to_string()
        } else {
            String::new()
        };
        if referenced == "song" {
            assert_eq!(
                rows_naming(&conn, table, column, &id),
                0,
                "{table}.{column} still names the deleted Song"
            );
        } else {
            assert_eq!(
                rows_naming(&conn, table, column, first.id),
                0,
                "{table}.{column} still names the deleted Arrangement"
            );
            assert_eq!(
                rows_naming(&conn, table, column, second.id),
                0,
                "{table}.{column} still names the deleted Arrangement"
            );
        }
    }
}

#[test]
fn ac6_a_failed_delete_at_any_step_changes_nothing() {
    let dir = TempDir::new("failed-delete");
    let storage = open(&dir);
    let (_, first, _) = seeded(&storage);
    add_lyrics_revision(&storage, &first);
    archive_arrangement(&storage, &first);
    let conn = raw(&dir);
    let before = snapshot(&conn, &[]);

    // Arrangement deletion: five child tables, the revisions and the row.
    for step in 1..=7 {
        bounded(storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            bounded(storage.delete_arrangement(first.id)),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "arrangement step {step}"
        );
        assert_eq!(
            snapshot(&conn, &[]),
            before,
            "an Arrangement delete failing at step {step} changes nothing"
        );
    }
    // A failure scripted one past the last statement never fires: the delete
    // succeeds, which proves the step count is exact.
    bounded(storage.inject_write_failure(8)).expect("arm");
    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Ok(Deleted { revisions: 2 })
    );

    let lone = bounded(storage.create_song(song_input("Gone")))
        .expect("childless song")
        .id;
    archive_song(&storage, lone);
    let before_song = snapshot(&conn, &[]);
    // Song deletion: aliases, moments, occasions, revisions and the row.
    for step in 1..=5 {
        bounded(storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            bounded(storage.delete_song(lone)),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "song step {step}"
        );
        assert_eq!(
            snapshot(&conn, &[]),
            before_song,
            "a Song delete failing at step {step} changes nothing"
        );
    }
    bounded(storage.inject_write_failure(6)).expect("arm");
    assert_eq!(
        bounded(storage.delete_song(lone)),
        Ok(Deleted { revisions: 1 })
    );
    shutdown(storage);
}

#[test]
fn ac6_deletion_facts_count_sections_occurrences_and_revisions() {
    let dir = TempDir::new("facts");
    let storage = open(&dir);
    let (song, first, second) = seeded(&storage);
    add_lyrics_revision(&storage, &first);

    let arrangement_facts =
        bounded(storage.deletion_facts(RecordRef::Arrangement(first.id))).expect("facts");
    assert_eq!(arrangement_facts.references, Vec::new());
    assert_eq!(arrangement_facts.children, Vec::new());
    assert_eq!(arrangement_facts.sections, 2);
    assert_eq!(arrangement_facts.occurrences, 3);
    assert_eq!(arrangement_facts.revisions, 2);

    let song_facts = bounded(storage.deletion_facts(RecordRef::Song(song))).expect("facts");
    assert_eq!(song_facts.references, Vec::new());
    assert_eq!(song_facts.sections, 0);
    assert_eq!(song_facts.occurrences, 0);
    assert_eq!(song_facts.revisions, 1);
    let mut children = song_facts.children;
    children.sort();
    let mut expected = vec![first.id, second.id];
    expected.sort();
    assert_eq!(children, expected);

    let ghost = SongId::from_entity(entity_id(0x91));
    assert_eq!(
        bounded(storage.deletion_facts(RecordRef::Song(ghost))),
        Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: ghost.entity(),
        })
    );
    let choir = ChoirId::from_entity(entity_id(0x92));
    assert_eq!(
        bounded(storage.deletion_facts(RecordRef::Choir(choir))),
        Err(StorageError::InvalidInput {
            field: "target",
            reason: "a Choir cannot be deleted",
        })
    );
    shutdown(storage);
}

#[test]
fn ac6_removing_a_planted_reference_unblocks_deletion() {
    let dir = TempDir::new("remove-reference");
    let storage = open(&dir);
    let (_, first, _) = seeded(&storage);
    archive_arrangement(&storage, &first);
    let mass = DurableReference::MassPlan(entity_id(0x90));
    let snapshot = DurableReference::PresentationSnapshot(entity_id(0x91));
    bounded(storage.plant_reference(RecordRef::Arrangement(first.id), mass.clone()))
        .expect("plant mass plan");
    bounded(storage.plant_reference(RecordRef::Arrangement(first.id), snapshot.clone()))
        .expect("plant snapshot");
    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Err(StorageError::DeletionBlocked {
            references: vec![mass.clone(), snapshot.clone()],
            children: Vec::new(),
        })
    );

    bounded(storage.remove_reference(RecordRef::Arrangement(first.id), mass)).expect("remove");
    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Err(StorageError::DeletionBlocked {
            references: vec![snapshot.clone()],
            children: Vec::new(),
        }),
        "one remaining reference still blocks"
    );

    bounded(storage.remove_reference(RecordRef::Arrangement(first.id), snapshot)).expect("remove");
    assert_eq!(
        bounded(storage.delete_arrangement(first.id)),
        Ok(Deleted { revisions: 1 })
    );

    // The Song path re-checks its own references the same way.
    let lone = bounded(storage.create_song(song_input("Lone")))
        .expect("song")
        .id;
    archive_song(&storage, lone);
    let package = DurableReference::CelebrationPackage(entity_id(0x93));
    bounded(storage.plant_reference(RecordRef::Song(lone), package.clone())).expect("plant");
    assert_eq!(
        bounded(storage.delete_song(lone)),
        Err(StorageError::DeletionBlocked {
            references: vec![package.clone()],
            children: Vec::new(),
        })
    );
    bounded(storage.remove_reference(RecordRef::Song(lone), package)).expect("remove");
    assert_eq!(
        bounded(storage.delete_song(lone)),
        Ok(Deleted { revisions: 1 })
    );
    shutdown(storage);
}

#[test]
fn ac6_deletion_survives_reopen() {
    let dir = TempDir::new("reopen");
    let storage = open(&dir);
    let (song, first, second) = seeded(&storage);
    add_lyrics_revision(&storage, &first);
    let song_before = bounded(storage.song(song)).expect("song");
    let sibling_before = bounded(storage.arrangement(second.id)).expect("sibling");
    let song_history = latest_history(&storage, RevisionEntity::Song(song));
    let lone = bounded(storage.create_song(song_input("Gone")))
        .expect("song")
        .id;
    archive_arrangement(&storage, &first);
    bounded(storage.delete_arrangement(first.id)).expect("delete arrangement");
    archive_song(&storage, lone);
    bounded(storage.delete_song(lone)).expect("delete song");
    shutdown(storage);

    let storage = open(&dir);
    assert_eq!(
        bounded(storage.arrangement(first.id)),
        Err(StorageError::NotFound {
            kind: RecordKind::Arrangement,
            id: first.id.entity(),
        })
    );
    assert_eq!(
        bounded(storage.song(lone)),
        Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: lone.entity(),
        })
    );
    assert_eq!(bounded(storage.song(song)), Ok(song_before));
    assert_eq!(bounded(storage.arrangement(second.id)), Ok(sibling_before));
    assert_eq!(
        latest_history(&storage, RevisionEntity::Song(song)),
        song_history
    );
    assert_eq!(
        bounded(storage.history(RevisionEntity::Arrangement(first.id))),
        Ok(Vec::new()),
        "the deleted Arrangement's History is gone"
    );
    assert_eq!(
        bounded(storage.history(RevisionEntity::Song(lone))),
        Ok(Vec::new()),
        "the deleted Song's History is gone"
    );
    let facts = bounded(storage.deletion_facts(RecordRef::Song(song))).expect("facts");
    assert_eq!(facts.children, vec![second.id]);
    let library = bounded(storage.library()).expect("library");
    assert_eq!(
        library
            .songs
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        vec![song]
    );
    assert_eq!(
        library
            .arrangements
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        vec![second.id]
    );
    shutdown(storage);

    let conn = raw(&dir);
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check");
    assert_eq!(integrity, "ok");
    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign key check");
    assert_eq!(violations, 0);
}

#[test]
fn ac9_schema_has_no_merge_survivor_or_redirect_table_or_column() {
    let dir = TempDir::new("schema");
    shutdown(open(&dir));
    let conn = raw(&dir);
    let banned = ["merge", "survivor", "redirect", "revers"];

    let mut statement = conn
        .prepare(
            "SELECT name, COALESCE(sql, '') FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("schema list");
    let schema: Vec<(String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("schema query")
        .map(|row| row.expect("schema row"))
        .collect();
    for (name, sql) in &schema {
        let lowered = format!("{name} {sql}").to_lowercase();
        for word in banned {
            assert!(!lowered.contains(word), "schema object {name} names {word}");
        }
    }

    for table in table_names(&conn) {
        let mut info = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .expect("table info");
        let columns: Vec<String> = info
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column query")
            .map(|row| row.expect("column name"))
            .collect();
        for column in columns {
            let lowered = column.to_lowercase();
            for word in banned {
                assert!(!lowered.contains(word), "{table}.{column} names {word}");
            }
        }
    }

    // The only revision kinds are the five the product has, and none of them is
    // a merge or a redirect.
    let revision_sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'edit_revision'",
            [],
            |row| row.get(0),
        )
        .expect("edit_revision schema");
    for kind in ["creation", "edit", "import", "replacement", "restoration"] {
        assert!(
            revision_sql.contains(&format!("'{kind}'")),
            "the revision kinds include {kind}"
        );
    }
}
