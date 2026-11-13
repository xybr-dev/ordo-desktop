//! T2.7 WP3 storage acceptance tests: the atomic lyrics commit keeps stored
//! metadata, writes exactly one revision and the pagination state it is given,
//! and rolls every table back on a fault. Every await runs under a 10 s bound
//! so a hang fails the test instead of blocking the run. Databases live in
//! unique temporary directories.

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::{ArrangementId, ChoirId, EntityId, HistorySessionId, SectionId, SongId};
use domain::revisions::{RevisionEntity, RevisionKind};
use domain::song_library::PaginationState;
use futures_lite::future::block_on;
use rusqlite::types::Value;
use rusqlite::{Connection, params};
use storage::{
    ArrangementContent, ArrangementMetadata, ArrangementRecord, NewArrangement, NewChoir,
    NewSection, NewSong, OccurrenceContent, RevisionContent, SectionContent, ShutdownOutcome,
    Storage, StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);
const STAMP: &str = "2023-11-14T22:13:20.000Z";

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
            "ordo-lyrics-tests-{tag}-{}-{}-{nanos}",
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

fn invalid(field: &'static str, reason: &'static str) -> StorageError {
    StorageError::InvalidInput { field, reason }
}

fn song_input(title: &str) -> NewSong {
    NewSong {
        title: title.to_owned(),
        aliases: vec!["Bread of Life".to_owned()],
        original_attribution: Some("Fr. Manoling Francisco".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Communion".to_owned()],
        occasions: vec!["Christmas".to_owned()],
    }
}

fn choir_input(name: &str) -> NewChoir {
    NewChoir {
        name: name.to_owned(),
        short_label: None,
        languages: vec!["Cebuano".to_owned()],
        notes: None,
    }
}

fn arrangement_input(song: SongId, choir: ChoirId) -> NewArrangement {
    NewArrangement {
        song,
        title: Some("Cebuano SATB".to_owned()),
        notes: Some("Source: Jesuit Music".to_owned()),
        languages: vec!["Cebuano".to_owned(), "English".to_owned()],
        choirs: vec![choir],
        rendition_attribution: Some("Arr. Cruz".to_owned()),
        occasions: vec!["Christmas".to_owned()],
        sections: vec![
            NewSection {
                label: Some("Verse 1".to_owned()),
                lyrics: "Tinapay sa langit".to_owned(),
            },
            NewSection {
                label: Some("Refrain".to_owned()),
                lyrics: "Dayandayan ang gugma".to_owned(),
            },
        ],
        sequence: vec![0, 1, 0],
    }
}

/// A Song, a Choir and one two-section, three-occurrence Arrangement.
fn seeded(storage: &Storage) -> (SongId, ChoirId, ArrangementRecord) {
    let choir = bounded(storage.create_choir(choir_input("Koro"))).expect("choir");
    let song = bounded(storage.create_song(song_input("Tinapay sa Langit"))).expect("song");
    let arrangement =
        bounded(storage.create_arrangement(arrangement_input(song.id, choir.id))).expect("created");
    (song.id, choir.id, arrangement)
}

/// The sections and Sequence of a record in the shapes the commit takes.
fn contents(record: &ArrangementRecord) -> (Vec<SectionContent>, Vec<OccurrenceContent>) {
    (
        record
            .sections
            .iter()
            .map(|section| SectionContent {
                id: section.id,
                label: section.label.clone(),
                lyrics: section.lyrics.clone(),
            })
            .collect(),
        record
            .sequence
            .iter()
            .map(|occurrence| OccurrenceContent {
                id: occurrence.id,
                section: occurrence.section,
            })
            .collect(),
    )
}

fn table_names(conn: &Connection) -> Vec<String> {
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name <> 'ordo_migrations' ORDER BY name",
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

fn revision_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM edit_revision", [], |row| row.get(0))
        .expect("revision count")
}

#[test]
fn ac8_lyrics_commit_keeps_metadata_and_writes_one_revision() {
    let dir = TempDir::new("metadata");
    let storage = open(&dir);
    let (_song, _choir, created) = seeded(&storage);
    let approved = bounded(storage.set_pagination_state(created.id, PaginationState::Approved))
        .expect("approve");
    assert_eq!(approved.pagination, PaginationState::Approved);
    let entity = RevisionEntity::Arrangement(created.id);
    let creation = bounded(storage.latest_revision(entity)).expect("creation");
    let (mut sections, sequence) = contents(&approved);
    let chain_before = bounded(storage.revisions(entity)).expect("chain");

    // Reorder the Lyrics list and rewrite one section's boundaries; the
    // Sequence, every identity and the metadata stay as stored.
    sections.swap(0, 1);
    sections[1].lyrics = "Tinapay sa langit\n\nDayandayan ang gugma".to_owned();
    let expected_sections = sections.clone();

    let shared_before = {
        let conn = raw(&dir);
        let before = snapshot(
            &conn,
            &[
                "arrangement",
                "arrangement_section",
                "section_occurrence",
                "edit_revision",
            ],
        );
        let count = revision_count(&conn);
        (before, count)
    };

    let (stored, revision) = bounded(storage.commit_arrangement_lyrics(
        created.id,
        sections,
        sequence.clone(),
        PaginationState::NeedsReview,
        session(0x21),
    ))
    .expect("lyrics commit");

    assert_eq!(stored.title, approved.title);
    assert_eq!(stored.notes, approved.notes);
    assert_eq!(stored.languages, approved.languages);
    assert_eq!(stored.choirs, approved.choirs);
    assert_eq!(stored.rendition_attribution, approved.rendition_attribution);
    assert_eq!(stored.occasions, approved.occasions);
    assert_eq!(stored.sections.len(), expected_sections.len());
    assert_eq!(
        stored
            .sections
            .iter()
            .map(|section| (section.id, section.label.clone(), section.lyrics.clone()))
            .collect::<Vec<_>>(),
        expected_sections
            .iter()
            .map(|section| (section.id, section.label.clone(), section.lyrics.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        stored
            .sequence
            .iter()
            .map(|occurrence| (occurrence.id, occurrence.section))
            .collect::<Vec<_>>(),
        sequence
            .iter()
            .map(|occurrence| (occurrence.id, occurrence.section))
            .collect::<Vec<_>>(),
        "the Sequence keeps its order and identities"
    );
    assert_eq!(stored.pagination, PaginationState::NeedsReview);
    assert_eq!(revision.parent, Some(creation));
    assert_eq!(revision.kind, RevisionKind::Edit);
    assert_eq!(revision.session, Some(session(0x21)));

    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(chain.len(), chain_before.len() + 1);
    assert_eq!(chain.last(), Some(&revision));
    assert_eq!(
        bounded(storage.revision_content(revision.id)),
        Ok(RevisionContent::Arrangement(ArrangementContent::of(
            &stored
        ))),
        "the revision carries the sections and Sequence it wrote"
    );

    let conn = raw(&dir);
    assert_eq!(
        revision_count(&conn),
        shared_before.1 + 1,
        "exactly one revision"
    );
    assert_eq!(
        snapshot(
            &conn,
            &[
                "arrangement",
                "arrangement_section",
                "section_occurrence",
                "edit_revision",
            ],
        ),
        shared_before.0,
        "languages, Choirs, occasions and every other table are unchanged"
    );
    drop(conn);
    shutdown(storage);
}

#[test]
fn ac9_lyrics_commit_removes_occurrences_and_restoring_readds_them_with_approval() {
    let dir = TempDir::new("remove-restore");
    let storage = open(&dir);
    let (_song, _choir, created) = seeded(&storage);
    let approved = bounded(storage.set_pagination_state(created.id, PaginationState::Approved))
        .expect("approve");
    let entity = RevisionEntity::Arrangement(created.id);
    let (sections, sequence) = contents(&approved);
    let removed = sections[0].id;
    assert!(
        sequence.iter().filter(|o| o.section == removed).count() == 2,
        "the first section is placed twice"
    );
    let kept_sections: Vec<SectionContent> = sections
        .iter()
        .filter(|section| section.id != removed)
        .cloned()
        .collect();
    let kept_sequence: Vec<OccurrenceContent> = sequence
        .iter()
        .filter(|occurrence| occurrence.section != removed)
        .cloned()
        .collect();

    let (removed_record, removal) = bounded(storage.commit_arrangement_lyrics(
        created.id,
        kept_sections,
        kept_sequence,
        PaginationState::NeedsReview,
        session(0x31),
    ))
    .expect("remove section");
    assert_eq!(removed_record.sections.len(), 1);
    assert_eq!(removed_record.sequence.len(), 1);
    assert_eq!(removed_record.pagination, PaginationState::NeedsReview);

    let conn = raw(&dir);
    let sections_left: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM arrangement_section WHERE arrangement_id = ?1",
            params![created.id.to_string()],
            |row| row.get(0),
        )
        .expect("section count");
    let occurrences_left: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM section_occurrence WHERE arrangement_id = ?1",
            params![created.id.to_string()],
            |row| row.get(0),
        )
        .expect("occurrence count");
    assert_eq!(
        (sections_left, occurrences_left),
        (1, 1),
        "the section and its occurrences went in the same transaction"
    );
    drop(conn);

    // Undo restores the earlier snapshot, approval included, and accepts the
    // section and occurrence identities this Arrangement used before.
    let (restored, undo) = bounded(storage.commit_arrangement_lyrics(
        created.id,
        sections.clone(),
        sequence.clone(),
        PaginationState::Approved,
        session(0x31),
    ))
    .expect("restore earlier lyrics");
    // `created_at` is not part of revision content, so a section deleted with
    // its Arrangement and later restored starts a fresh timestamp; identity,
    // label, lyrics and order are the snapshot's contract.
    let (restored_sections, restored_sequence) = contents(&restored);
    assert_eq!(restored_sections, sections);
    assert_eq!(restored_sequence, sequence);
    assert_eq!(restored.pagination, PaginationState::Approved);
    assert_eq!(undo.parent, Some(removal.id));
    assert_eq!(
        bounded(storage.revisions(entity))
            .expect("chain")
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        vec![removal.parent.expect("creation"), removal.id, undo.id]
    );
    shutdown(storage);
}

#[test]
fn lyrics_commit_keeps_metadata_committed_before_it() {
    let dir = TempDir::new("metadata-first");
    let storage = open(&dir);
    let (_song, choir, created) = seeded(&storage);
    let (sections, sequence) = contents(&created);
    let mut edited = sections;
    edited[1].lyrics = "Newer lyrics".to_owned();

    let metadata = ArrangementMetadata {
        title: Some("Newer details".to_owned()),
        languages: vec!["Latin".to_owned()],
        choirs: vec![choir],
        rendition_attribution: Some("Arr. New".to_owned()),
        occasions: vec!["Lent".to_owned()],
        notes: Some("Newer notes".to_owned()),
    };
    let (_, details) =
        bounded(storage.commit_arrangement_metadata(created.id, metadata.clone(), session(0x41)))
            .expect("metadata commit");
    let (stored, lyrics) = bounded(storage.commit_arrangement_lyrics(
        created.id,
        edited.clone(),
        sequence,
        PaginationState::Draft,
        session(0x42),
    ))
    .expect("lyrics commit");

    assert_eq!(lyrics.parent, Some(details.id));
    assert_eq!(stored.title, metadata.title);
    assert_eq!(stored.notes, metadata.notes);
    assert_eq!(stored.languages, metadata.languages);
    assert_eq!(stored.choirs, metadata.choirs);
    assert_eq!(stored.rendition_attribution, metadata.rendition_attribution);
    assert_eq!(stored.occasions, metadata.occasions);
    assert_eq!(stored.sections.len(), edited.len());
    assert_eq!(stored.sections[1].lyrics, "Newer lyrics");
    shutdown(storage);
}

#[test]
fn metadata_commit_keeps_lyrics_committed_before_it() {
    let dir = TempDir::new("lyrics-first");
    let storage = open(&dir);
    let (_song, choir, created) = seeded(&storage);
    let (mut sections, sequence) = contents(&created);
    sections[0].lyrics = "Tinapay sa langit\n\nDayandayan".to_owned();
    let edited = sections.clone();

    let (_, lyrics) = bounded(storage.commit_arrangement_lyrics(
        created.id,
        sections,
        sequence,
        PaginationState::NeedsReview,
        session(0x51),
    ))
    .expect("lyrics commit");
    let metadata = ArrangementMetadata {
        title: Some("Details after lyrics".to_owned()),
        languages: vec!["Cebuano".to_owned(), "English".to_owned()],
        choirs: vec![choir],
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: Some("Notes after lyrics".to_owned()),
    };
    let (stored, details) =
        bounded(storage.commit_arrangement_metadata(created.id, metadata.clone(), session(0x52)))
            .expect("metadata commit");

    assert_eq!(details.parent, Some(lyrics.id));
    assert_eq!(stored.title, metadata.title);
    assert_eq!(stored.notes, metadata.notes);
    assert_eq!(stored.languages, metadata.languages);
    assert_eq!(stored.sections.len(), edited.len());
    assert_eq!(stored.sections[0].lyrics, "Tinapay sa langit\n\nDayandayan");
    assert_eq!(stored.pagination, PaginationState::NeedsReview);
    shutdown(storage);
}

#[test]
fn lyrics_commit_write_steps_are_exact_and_atomic() {
    let dir = TempDir::new("steps");
    let storage = open(&dir);
    let (_song, _choir, created) = seeded(&storage);
    let (mut sections, sequence) = contents(&created);
    let entity = RevisionEntity::Arrangement(created.id);

    // 1 update + 5 deletes + languages + Choirs + occasions + sections +
    // occurrences + 1 revision + 1 pagination.
    let steps = 1
        + 5
        + created.languages.len()
        + created.choirs.len()
        + created.occasions.len()
        + created.sections.len()
        + created.sequence.len()
        + 1
        + 1;

    // A failure scripted one past the last statement never fires: the edit
    // succeeds, which proves the step count is exact.
    sections[0].lyrics = "Committed".to_owned();
    bounded(storage.inject_write_failure(steps + 1)).expect("arm");
    let (committed, _) = bounded(storage.commit_arrangement_lyrics(
        created.id,
        sections.clone(),
        sequence.clone(),
        PaginationState::NeedsReview,
        session(0x61),
    ))
    .expect("edit succeeds");

    let content_before: ArrangementRecord = bounded(storage.arrangement(created.id)).expect("read");
    let history_before = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(content_before, committed);
    assert_eq!(history_before.len(), 2);

    let mut failing = sections;
    failing[0].lyrics = "Failing".to_owned();
    for step in 1..=steps {
        bounded(storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            bounded(storage.commit_arrangement_lyrics(
                created.id,
                failing.clone(),
                sequence.clone(),
                PaginationState::NeedsReview,
                session(0x61),
            ))
            .map(|_| ()),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "step {step}"
        );
        assert_eq!(
            bounded(storage.arrangement(created.id)),
            Ok(content_before.clone()),
            "sections, Sequence and pagination after failure at step {step}"
        );
        assert_eq!(
            bounded(storage.revisions(entity)),
            Ok(history_before.clone()),
            "history after failure at step {step}"
        );
    }
    shutdown(storage);
}

#[test]
fn a_section_or_occurrence_id_owned_by_another_arrangement_is_refused() {
    let dir = TempDir::new("foreign-id");
    let storage = open(&dir);
    let (song, choir, created) = seeded(&storage);
    let other = bounded(storage.create_arrangement(arrangement_input(song, choir)))
        .expect("second arrangement");
    let entity = RevisionEntity::Arrangement(created.id);
    let chain_before = bounded(storage.revisions(entity)).expect("chain");
    let (sections, sequence) = contents(&created);

    let mut borrowed_section = sections.clone();
    borrowed_section.push(SectionContent {
        id: other.sections[0].id,
        label: Some("Borrowed".to_owned()),
        lyrics: String::new(),
    });
    assert_eq!(
        bounded(storage.commit_arrangement_lyrics(
            created.id,
            borrowed_section,
            sequence.clone(),
            PaginationState::Draft,
            session(0x71),
        ))
        .map(|_| ()),
        Err(invalid("section", "foreign id"))
    );

    let mut borrowed_occurrence = sequence;
    borrowed_occurrence.push(OccurrenceContent {
        id: other.sequence[0].id,
        section: sections[0].id,
    });
    assert_eq!(
        bounded(storage.commit_arrangement_lyrics(
            created.id,
            sections,
            borrowed_occurrence,
            PaginationState::Draft,
            session(0x72),
        ))
        .map(|_| ()),
        Err(invalid("occurrence", "foreign id"))
    );
    assert_eq!(
        bounded(storage.revisions(entity)).expect("chain"),
        chain_before,
        "a refused commit writes nothing"
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Arrangement(other.id)))
            .expect("other chain")
            .len(),
        1,
        "the owning Arrangement is untouched"
    );
    shutdown(storage);
}

/// Plants a Song and an Arrangement with sections and occurrences but no
/// revision, as a T2.1-era row.
fn plant_legacy_arrangement(dir: &TempDir) -> (SongId, ArrangementId, SectionId) {
    let song = SongId::from_entity(entity_id(0x81));
    let arrangement = ArrangementId::from_entity(entity_id(0x82));
    let section = SectionId::from_entity(entity_id(0x83));
    let occurrence = domain::identity::OccurrenceId::from_entity(entity_id(0x84));
    let conn = raw(dir);
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, 'Kyrie', ?2)",
        params![song.to_string(), STAMP],
    )
    .expect("song");
    conn.execute(
        "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, 'Old', ?3)",
        params![arrangement.to_string(), song.to_string(), STAMP],
    )
    .expect("arrangement");
    conn.execute(
        "INSERT INTO arrangement_language (arrangement_id, position, language) \
         VALUES (?1, 0, 'Cebuano')",
        params![arrangement.to_string()],
    )
    .expect("language");
    conn.execute(
        "INSERT INTO arrangement_section \
         (id, arrangement_id, position, label, lyrics, created_at) \
         VALUES (?1, ?2, 0, 'Verse 1', 'Old words', ?3)",
        params![section.to_string(), arrangement.to_string(), STAMP],
    )
    .expect("section");
    conn.execute(
        "INSERT INTO section_occurrence \
         (id, arrangement_id, section_id, position, created_at) \
         VALUES (?1, ?2, ?3, 0, ?4)",
        params![
            occurrence.to_string(),
            arrangement.to_string(),
            section.to_string(),
            STAMP
        ],
    )
    .expect("occurrence");
    (song, arrangement, section)
}

#[test]
fn lyrics_commit_backfills_a_missing_creation_revision() {
    let dir = TempDir::new("backfill");
    shutdown(open(&dir));
    let (_song, arrangement, section) = plant_legacy_arrangement(&dir);

    let storage = open(&dir);
    let entity = RevisionEntity::Arrangement(arrangement);
    let (stored, revision) = bounded(storage.commit_arrangement_lyrics(
        arrangement,
        vec![SectionContent {
            id: section,
            label: Some("Verse 1".to_owned()),
            lyrics: "New words".to_owned(),
        }],
        vec![OccurrenceContent {
            id: domain::identity::OccurrenceId::from_entity(entity_id(0x84)),
            section,
        }],
        PaginationState::Draft,
        session(0x85),
    ))
    .expect("lyrics commit on a row with no history");
    assert_eq!(stored.sections[0].lyrics, "New words");

    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].kind, RevisionKind::Creation);
    assert_eq!(revision.parent, Some(chain[0].id));
    assert_eq!(revision.kind, RevisionKind::Edit);
    let RevisionContent::Arrangement(backfilled) =
        bounded(storage.revision_content(chain[0].id)).expect("creation content")
    else {
        panic!("an Arrangement revision holds Arrangement content");
    };
    assert_eq!(backfilled.sections[0].lyrics, "Old words");
    shutdown(storage);
}

#[test]
fn lyrics_commit_refuses_repeated_and_unknown_identities_before_writing() {
    let dir = TempDir::new("invalid");
    let storage = open(&dir);
    let (_song, _choir, created) = seeded(&storage);
    let (sections, sequence) = contents(&created);
    let entity = RevisionEntity::Arrangement(created.id);
    let chain_before = bounded(storage.revisions(entity)).expect("chain");

    let mut repeated_sections = sections.clone();
    repeated_sections.push(sections[0].clone());
    assert_eq!(
        bounded(storage.commit_arrangement_lyrics(
            created.id,
            repeated_sections,
            sequence.clone(),
            PaginationState::Draft,
            session(0x91),
        ))
        .map(|_| ()),
        Err(invalid("section", "duplicate"))
    );

    let mut repeated_occurrences = sequence.clone();
    repeated_occurrences.push(sequence[0].clone());
    assert_eq!(
        bounded(storage.commit_arrangement_lyrics(
            created.id,
            sections.clone(),
            repeated_occurrences,
            PaginationState::Draft,
            session(0x92),
        ))
        .map(|_| ()),
        Err(invalid("occurrence", "duplicate"))
    );

    let unknown = SectionId::from_entity(entity_id(0x93));
    assert_eq!(
        bounded(storage.commit_arrangement_lyrics(
            created.id,
            sections,
            vec![OccurrenceContent {
                id: domain::identity::OccurrenceId::from_entity(entity_id(0x94)),
                section: unknown,
            }],
            PaginationState::Draft,
            session(0x95),
        ))
        .map(|_| ()),
        Err(invalid("sequence", "unknown section"))
    );
    assert_eq!(
        bounded(storage.revisions(entity)).expect("chain"),
        chain_before
    );
    assert_eq!(
        bounded(storage.arrangement(created.id)).map(|r| r.id),
        Ok(created.id)
    );
    shutdown(storage);
}
