//! T2.6 WP2 storage acceptance tests: migration 6, optional Arrangement title
//! and notes, required languages, and format-2 revisions that still read
//! format-1 payloads. Every await runs under a 10 s bound so a hang fails the
//! test instead of blocking the run. Databases live in unique temporary
//! directories.

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::{
    ArrangementId, ChoirId, EntityId, HistorySessionId, OccurrenceId, RevisionId, SectionId, SongId,
};
use domain::revisions::{RevisionEntity, RevisionKind};
use domain::song_library::PaginationState;
use futures_lite::future::block_on;
use rusqlite::types::Value;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use storage::{
    ArrangementContent, NewArrangement, NewChoir, NewSection, NewSong, OccurrenceContent,
    RevisionContent, SectionContent, ShutdownOutcome, Storage, StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);
const STAMP: &str = "2023-11-14T22:13:20.000Z";

/// The exact format-1 Arrangement payload planted by the legacy fixture.
const FORMAT_1_CONTENT: &str = concat!(
    r#"{"format":1,"entity":"arrangement","title":"Legacy SATB","#,
    r#""languages":["Cebuano","English"],"#,
    r#""choirs":["73737373-7373-4373-b373-737373737373"],"#,
    r#""rendition_attribution":"Arr. Cruz","occasions":["Christmas"],"#,
    r#""sections":[{"id":"74747474-7474-4474-b474-747474747474","#,
    r#""label":"Verse 1","lyrics":"Tinapay sa langit"}],"#,
    r#""sequence":[{"id":"75757575-7575-4575-b575-757575757575","#,
    r#""section":"74747474-7474-4474-b474-747474747474"}]}"#,
);

/// SHA-256 of exactly [`FORMAT_1_CONTENT`], computed outside the test so the
/// fixture verifies stored bytes rather than a re-serialization.
const FORMAT_1_SHA256: &str = "c1727ace61b9cfb312daf88688f0cb659f8eb2c00d24ebaa95ddee14745042ee";

/// The canonical format-2 payload this build writes when the format-1 fixture
/// is restored: title kept, absent notes omitted.
const FORMAT_2_CONTENT: &str = concat!(
    r#"{"format":2,"entity":"arrangement","title":"Legacy SATB","#,
    r#""languages":["Cebuano","English"],"#,
    r#""choirs":["73737373-7373-4373-b373-737373737373"],"#,
    r#""rendition_attribution":"Arr. Cruz","occasions":["Christmas"],"#,
    r#""sections":[{"id":"74747474-7474-4474-b474-747474747474","#,
    r#""label":"Verse 1","lyrics":"Tinapay sa langit"}],"#,
    r#""sequence":[{"id":"75757575-7575-4575-b575-757575757575","#,
    r#""section":"74747474-7474-4474-b474-747474747474"}]}"#,
);

/// SHA-256 of exactly [`FORMAT_2_CONTENT`].
const FORMAT_2_SHA256: &str = "e62100b812dfcd1e2c2e58cb5f3f020d5a07f23ed2f7ecc6fddd114f91ab8aee";

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
            "ordo-arrangement-details-tests-{tag}-{}-{}-{nanos}",
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

fn open(dir: &TempDir) -> Storage {
    let storage = Storage::spawn(config(&dir.db()));
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

fn hex_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
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

fn seeded(storage: &Storage) -> (SongId, ChoirId) {
    let choir = bounded(storage.create_choir(choir_input("Koro"))).expect("choir");
    let song = bounded(storage.create_song(song_input("Tinapay sa Langit"))).expect("song");
    (song.id, choir.id)
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

/// The pre-0006 Arrangement columns, so a row snapshot is comparable across
/// the rebuild even though the table gains a `notes` column.
fn arrangement_rows(conn: &Connection) -> Vec<Vec<Value>> {
    let mut statement = conn
        .prepare(
            "SELECT id, song_id, title, rendition_attribution, created_at, \
             archived_at, pagination_state FROM arrangement ORDER BY id",
        )
        .expect("arrangement rows");
    statement
        .query_map([], |row| {
            (0..7)
                .map(|index| row.get::<_, Value>(index))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .expect("arrangement query")
        .map(|row| row.expect("arrangement row"))
        .collect()
}

/// An Arrangement's language rows in authored order.
fn arrangement_languages(conn: &Connection, arrangement: ArrangementId) -> Vec<(i64, String)> {
    let mut statement = conn
        .prepare(
            "SELECT position, language FROM arrangement_language \
             WHERE arrangement_id = ?1 ORDER BY position",
        )
        .expect("language rows");
    statement
        .query_map(params![arrangement.to_string()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .expect("language query")
        .map(|row| row.expect("language row"))
        .collect()
}

fn column_not_null(conn: &Connection, table: &str, column: &str) -> Option<bool> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))
        .expect("table info");
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)? != 0))
        })
        .expect("column query")
        .map(|row| row.expect("column info"))
        .find(|(name, _)| name == column)
        .map(|(_, not_null)| not_null)
}

fn foreign_key_violations(conn: &Connection) -> Vec<(String, i64, String, i64)> {
    let mut statement = conn.prepare("PRAGMA foreign_key_check").expect("fk check");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("fk query")
        .map(|row| row.expect("fk row"))
        .collect()
}

#[test]
fn t2_6_migrates_populated_v5_parish() {
    let dir = TempDir::new("migrate-v5");
    let bundled_v5 = [
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
        (
            "0005_library_details",
            include_str!("../../../migrations/0005_library_details.sql"),
        ),
    ];
    let parish = entity_id(0x50);
    let choir = ChoirId::from_entity(entity_id(0x51));
    let song = SongId::from_entity(entity_id(0x52));
    let arrangement = ArrangementId::from_entity(entity_id(0x53));
    let first_section = SectionId::from_entity(entity_id(0x54));
    let second_section = SectionId::from_entity(entity_id(0x55));
    let first_occurrence = OccurrenceId::from_entity(entity_id(0x56));
    let second_occurrence = OccurrenceId::from_entity(entity_id(0x57));
    let revision = RevisionId::from_entity(entity_id(0x58));
    let legacy_content = format!(
        concat!(
            r#"{{"format":1,"entity":"arrangement","title":"Cebuano SATB","#,
            r#""languages":["Cebuano","English"],"#,
            r#""choirs":["{choir}"],"rendition_attribution":"Arr. Cruz","#,
            r#""occasions":["Christmas"],"#,
            r#""sections":[{{"id":"{first_section}","label":"Verse 1","lyrics":"Tinapay sa langit"}},"#,
            r#"{{"id":"{second_section}","label":"Refrain","lyrics":"Dayandayan ang gugma"}}],"#,
            r#""sequence":[{{"id":"{first_occurrence}","section":"{first_section}"}},"#,
            r#"{{"id":"{second_occurrence}","section":"{second_section}"}}]}}"#,
        ),
        choir = choir,
        first_section = first_section,
        second_section = second_section,
        first_occurrence = first_occurrence,
        second_occurrence = second_occurrence,
    );

    let (before_tables, before_arrangement) = {
        // A version-5 database exactly as the migration runner leaves it,
        // populated with one of each library record and an Arrangement revision.
        let conn = Connection::open(dir.db()).expect("raw connection");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");
        conn.execute_batch(
            "CREATE TABLE ordo_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL, \
             checksum TEXT NOT NULL, applied_at TEXT NOT NULL)",
        )
        .expect("ledger");
        for (index, (name, sql)) in bundled_v5.iter().enumerate() {
            conn.execute_batch(sql).expect("apply migration");
            conn.execute(
                "INSERT INTO ordo_migrations (id, name, checksum, applied_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![index as i64 + 1, name, hex_sha256(sql), STAMP],
            )
            .expect("ledger row");
        }
        conn.pragma_update(None, "user_version", 5)
            .expect("version");

        conn.execute(
            "INSERT INTO parish_profile (id, name) VALUES (1, 'St Mary')",
            [],
        )
        .expect("profile");
        conn.execute(
            "INSERT INTO parish_note (id, profile_id, body) VALUES (1, 1, 'kept')",
            [],
        )
        .expect("note");
        conn.execute(
            "INSERT INTO parish (id, name, created_at) VALUES (?1, 'St Mary', ?2)",
            params![parish.to_string(), STAMP],
        )
        .expect("parish");
        conn.execute(
            "INSERT INTO parish_setting (key, value, updated_at) VALUES ('locale', 'ceb', ?1)",
            params![STAMP],
        )
        .expect("setting");
        conn.execute(
            "INSERT INTO choir (id, name, short_label, notes, created_at) \
             VALUES (?1, 'Koro', 'K', 'choir notes', ?2)",
            params![choir.to_string(), STAMP],
        )
        .expect("choir");
        conn.execute(
            "INSERT INTO choir_language (choir_id, position, language) VALUES (?1, 0, 'Cebuano')",
            params![choir.to_string()],
        )
        .expect("choir language");
        conn.execute(
            "INSERT INTO song (id, title, original_attribution, provenance, rights_notes, created_at) \
             VALUES (?1, 'Tinapay sa Langit', 'Fr. Manoling Francisco', 'Jesuit Music', \
             'Used with permission', ?2)",
            params![song.to_string(), STAMP],
        )
        .expect("song");
        conn.execute(
            "INSERT INTO song_alias (song_id, position, alias) VALUES (?1, 0, 'Bread of Life')",
            params![song.to_string()],
        )
        .expect("alias");
        conn.execute(
            "INSERT INTO song_liturgical_moment (song_id, moment) VALUES (?1, 'Communion')",
            params![song.to_string()],
        )
        .expect("moment");
        conn.execute(
            "INSERT INTO song_occasion (song_id, occasion) VALUES (?1, 'Christmas')",
            params![song.to_string()],
        )
        .expect("song occasion");
        conn.execute(
            "INSERT INTO arrangement \
             (id, song_id, title, rendition_attribution, created_at, archived_at, pagination_state) \
             VALUES (?1, ?2, 'Cebuano SATB', 'Arr. Cruz', ?3, NULL, 'approved')",
            params![arrangement.to_string(), song.to_string(), STAMP],
        )
        .expect("arrangement");
        for (position, language) in ["Cebuano", "English"].into_iter().enumerate() {
            conn.execute(
                "INSERT INTO arrangement_language (arrangement_id, position, language) \
                 VALUES (?1, ?2, ?3)",
                params![arrangement.to_string(), position as i64, language],
            )
            .expect("arrangement language");
        }
        conn.execute(
            "INSERT INTO arrangement_choir (arrangement_id, choir_id) VALUES (?1, ?2)",
            params![arrangement.to_string(), choir.to_string()],
        )
        .expect("arrangement choir");
        conn.execute(
            "INSERT INTO arrangement_occasion (arrangement_id, occasion) VALUES (?1, 'Christmas')",
            params![arrangement.to_string()],
        )
        .expect("arrangement occasion");
        for (position, (id, label, lyrics)) in [
            (first_section, "Verse 1", "Tinapay sa langit"),
            (second_section, "Refrain", "Dayandayan ang gugma"),
        ]
        .into_iter()
        .enumerate()
        {
            conn.execute(
                "INSERT INTO arrangement_section \
                 (id, arrangement_id, position, label, lyrics, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id.to_string(),
                    arrangement.to_string(),
                    position as i64,
                    label,
                    lyrics,
                    STAMP
                ],
            )
            .expect("section");
        }
        for (position, (id, section)) in [
            (first_occurrence, first_section),
            (second_occurrence, second_section),
        ]
        .into_iter()
        .enumerate()
        {
            conn.execute(
                "INSERT INTO section_occurrence \
                 (id, arrangement_id, section_id, position, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id.to_string(),
                    arrangement.to_string(),
                    section.to_string(),
                    position as i64,
                    STAMP
                ],
            )
            .expect("occurrence");
        }
        conn.execute(
            "INSERT INTO edit_revision \
             (id, song_id, arrangement_id, parent_id, kind, session_id, restored_from_id, \
             content, content_sha256, created_at) \
             VALUES (?1, NULL, ?2, NULL, 'creation', NULL, NULL, ?3, ?4, ?5)",
            params![
                revision.to_string(),
                arrangement.to_string(),
                legacy_content,
                hex_sha256(&legacy_content),
                STAMP
            ],
        )
        .expect("revision");

        (snapshot(&conn, &["arrangement"]), arrangement_rows(&conn))
    };

    let storage = open(&dir);
    let status = bounded(storage.schema_status()).expect("status");
    assert_eq!(status.version, 6);
    assert_eq!(status.applied.last().map(|migration| migration.id), Some(6));

    let record = bounded(storage.arrangement(arrangement)).expect("arrangement");
    assert_eq!(record.title.as_deref(), Some("Cebuano SATB"));
    assert_eq!(record.notes, None);
    assert_eq!(record.languages, vec!["Cebuano", "English"]);
    assert_eq!(record.choirs, vec![choir]);
    assert_eq!(record.occasions, vec!["Christmas"]);
    assert_eq!(record.pagination, PaginationState::Approved);
    assert_eq!(
        record.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![first_section, second_section]
    );
    assert_eq!(
        record
            .sequence
            .iter()
            .map(|o| (o.id, o.section))
            .collect::<Vec<_>>(),
        vec![
            (first_occurrence, first_section),
            (second_occurrence, second_section)
        ]
    );
    let content = bounded(storage.revision_content(revision)).expect("legacy revision");
    let RevisionContent::Arrangement(content) = content else {
        panic!("an Arrangement revision holds Arrangement content");
    };
    assert_eq!(content.title.as_deref(), Some("Cebuano SATB"));
    assert_eq!(content.notes, None);
    assert_eq!(content.languages, vec!["Cebuano", "English"]);
    shutdown(storage);

    let conn = raw(&dir);
    assert_eq!(
        foreign_key_violations(&conn),
        Vec::new(),
        "the rebuild leaves no foreign-key violations"
    );
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check");
    assert_eq!(
        integrity, "ok",
        "the migrated database is structurally sound"
    );
    assert_eq!(
        arrangement_rows(&conn),
        before_arrangement,
        "every pre-0006 Arrangement column is preserved"
    );
    assert_eq!(
        snapshot(&conn, &["arrangement"]),
        before_tables,
        "no other table changes"
    );
    let notes: Option<String> = conn
        .query_row(
            "SELECT notes FROM arrangement WHERE id = ?1",
            params![arrangement.to_string()],
            |row| row.get(0),
        )
        .expect("migrated notes");
    assert_eq!(notes, None, "migrated rows gain NULL notes");
    assert_eq!(
        column_not_null(&conn, "arrangement", "title"),
        Some(false),
        "title becomes nullable"
    );
    assert_eq!(
        column_not_null(&conn, "arrangement", "notes"),
        Some(false),
        "notes is nullable"
    );
    let index_columns: Vec<String> = {
        let mut statement = conn
            .prepare("PRAGMA index_info('arrangement_by_song')")
            .expect("index info");
        statement
            .query_map([], |row| row.get::<_, String>(2))
            .expect("index query")
            .map(|row| row.expect("index column"))
            .collect()
    };
    assert_eq!(index_columns, vec!["song_id"]);

    // The rebuilt CHECK accepts a NULL title and NULL notes.
    conn.execute_batch("SAVEPOINT nullable").expect("savepoint");
    conn.execute(
        "INSERT INTO arrangement (id, song_id, title, notes, created_at) \
         VALUES (?1, ?2, NULL, NULL, ?3)",
        params![entity_id(0x59).to_string(), song.to_string(), STAMP],
    )
    .expect("a NULL title is valid");
    conn.execute_batch("ROLLBACK TO nullable; RELEASE nullable")
        .expect("rollback");
}

#[test]
fn t2_6_creates_arrangement_without_title() {
    let dir = TempDir::new("no-title");
    let storage = open(&dir);
    let (song, choir) = seeded(&storage);

    let mut input = arrangement_input(song, choir);
    input.title = None;
    input.notes = Some("Source: Jesuit Music".to_owned());
    let created = bounded(storage.create_arrangement(input.clone())).expect("arrangement");
    assert_eq!(created.title, None);
    assert_eq!(created.notes.as_deref(), Some("Source: Jesuit Music"));
    let entity = RevisionEntity::Arrangement(created.id);
    let latest = bounded(storage.latest_revision(entity)).expect("latest");
    let content = bounded(storage.revision_content(latest)).expect("content");
    let RevisionContent::Arrangement(content) = content else {
        panic!("an Arrangement revision holds Arrangement content");
    };
    assert_eq!(content.title, None);
    assert_eq!(content.notes.as_deref(), Some("Source: Jesuit Music"));
    assert_eq!(
        bounded(storage.arrangement(created.id)).expect("read back"),
        created
    );

    let conn = raw(&dir);
    let (title, notes): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT title, notes FROM arrangement WHERE id = ?1",
            params![created.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored row");
    assert_eq!(title, None);
    assert_eq!(notes.as_deref(), Some("Source: Jesuit Music"));
    drop(conn);

    // Present-but-blank title and notes are refused on create.
    let mut blank_title = input.clone();
    blank_title.title = Some("  \t ".to_owned());
    assert_eq!(
        bounded(storage.create_arrangement(blank_title)).map(|_| ()),
        Err(invalid("title", "blank"))
    );
    let mut blank_notes = input.clone();
    blank_notes.notes = Some(" \n ".to_owned());
    assert_eq!(
        bounded(storage.create_arrangement(blank_notes)).map(|_| ()),
        Err(invalid("notes", "blank"))
    );

    // At least one language is required, and a present language is not blank.
    let mut no_language = input.clone();
    no_language.languages.clear();
    assert_eq!(
        bounded(storage.create_arrangement(no_language)).map(|_| ()),
        Err(invalid("language", "required"))
    );
    let mut blank_language = input.clone();
    blank_language.languages = vec!["  ".to_owned()];
    assert_eq!(
        bounded(storage.create_arrangement(blank_language)).map(|_| ()),
        Err(invalid("language", "blank"))
    );

    // The same rules hold on a revision commit, which writes nothing on refusal.
    let latest = bounded(storage.latest_revision(entity)).expect("latest");
    let current = bounded(storage.arrangement(created.id)).expect("arrangement");
    let current_content = ArrangementContent::of(&current);
    let mut commit_blank_title = current_content.clone();
    commit_blank_title.title = Some("  ".to_owned());
    assert_eq!(
        bounded(storage.commit_arrangement_edit(
            created.id,
            commit_blank_title,
            PaginationState::Draft,
            RevisionKind::Edit,
            Some(session(0x41)),
            latest,
        ))
        .map(|_| ()),
        Err(invalid("title", "blank"))
    );
    let mut commit_blank_notes = current_content.clone();
    commit_blank_notes.notes = Some("\t".to_owned());
    assert_eq!(
        bounded(storage.commit_arrangement_edit(
            created.id,
            commit_blank_notes,
            PaginationState::Draft,
            RevisionKind::Edit,
            Some(session(0x42)),
            latest,
        ))
        .map(|_| ()),
        Err(invalid("notes", "blank"))
    );
    let mut commit_no_language = current_content;
    commit_no_language.languages.clear();
    assert_eq!(
        bounded(storage.commit_arrangement_edit(
            created.id,
            commit_no_language,
            PaginationState::Draft,
            RevisionKind::Edit,
            Some(session(0x43)),
            latest,
        ))
        .map(|_| ()),
        Err(invalid("language", "required"))
    );
    assert_eq!(
        bounded(storage.revisions(entity)).expect("chain").len(),
        1,
        "refused commits append no revision"
    );
    shutdown(storage);
}

#[test]
fn ac6_metadata_commit_adds_exactly_one_revision() {
    let dir = TempDir::new("metadata-commit");
    let storage = open(&dir);
    let (song, choir) = seeded(&storage);
    let created =
        bounded(storage.create_arrangement(arrangement_input(song, choir))).expect("arrangement");
    let entity = RevisionEntity::Arrangement(created.id);
    let creation = bounded(storage.latest_revision(entity)).expect("creation");

    // First make the pagination state non-default through the normal edit path.
    let mut approved_content = ArrangementContent::of(&created);
    approved_content.title = Some("Cebuano SATB, approved".to_owned());
    let (approved, approved_revision) = bounded(storage.commit_arrangement_edit(
        created.id,
        approved_content,
        PaginationState::Approved,
        RevisionKind::Edit,
        Some(session(0x44)),
        creation,
    ))
    .expect("approve");
    assert_eq!(approved.pagination, PaginationState::Approved);

    let conn = raw(&dir);
    let before_tables = snapshot(
        &conn,
        &["arrangement", "arrangement_language", "edit_revision"],
    );
    let before_total: i64 = conn
        .query_row("SELECT COUNT(*) FROM edit_revision", [], |row| row.get(0))
        .expect("revision count");
    let before_identity: (
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT id, song_id, rendition_attribution, created_at, archived_at, \
             pagination_state FROM arrangement WHERE id = ?1",
            params![created.id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("arrangement identity");
    let before_languages = arrangement_languages(&conn, created.id);
    assert_eq!(
        before_languages,
        vec![(0, "Cebuano".to_owned()), (1, "English".to_owned())]
    );
    drop(conn);
    let before_chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(before_chain.len(), 2);

    // A metadata Save clears the title, replaces notes and changes languages;
    // every identity, section, Sequence and pagination value is carried
    // through unchanged.
    let mut metadata = ArrangementContent::of(&approved);
    metadata.title = None;
    metadata.notes = Some("Source: revised".to_owned());
    metadata.languages = vec!["Cebuano".to_owned(), "Latin".to_owned()];
    let (updated, revision) = bounded(storage.commit_arrangement_edit(
        created.id,
        metadata.clone(),
        PaginationState::Approved,
        RevisionKind::Edit,
        Some(session(0x45)),
        approved_revision.id,
    ))
    .expect("metadata commit");

    assert_eq!(updated.title, None);
    assert_eq!(updated.notes.as_deref(), Some("Source: revised"));
    assert_eq!(
        updated.languages,
        vec!["Cebuano".to_owned(), "Latin".to_owned()]
    );
    assert_eq!(updated.choirs, approved.choirs);
    assert_eq!(
        updated.rendition_attribution,
        approved.rendition_attribution
    );
    assert_eq!(updated.occasions, approved.occasions);
    assert_eq!(updated.sections, approved.sections);
    assert_eq!(updated.sequence, approved.sequence);
    assert_eq!(updated.pagination, PaginationState::Approved);
    assert_eq!(revision.parent, Some(approved_revision.id));
    assert_eq!(revision.kind, RevisionKind::Edit);
    assert_eq!(revision.session, Some(session(0x45)));

    let after_chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(after_chain.len(), before_chain.len() + 1);
    assert_eq!(after_chain.last(), Some(&revision));
    assert_eq!(
        bounded(storage.revision_content(revision.id)),
        Ok(RevisionContent::Arrangement(metadata))
    );

    let conn = raw(&dir);
    let after_total: i64 = conn
        .query_row("SELECT COUNT(*) FROM edit_revision", [], |row| row.get(0))
        .expect("revision count");
    assert_eq!(after_total, before_total + 1, "exactly one revision");
    let after_tables = snapshot(
        &conn,
        &["arrangement", "arrangement_language", "edit_revision"],
    );
    assert_eq!(
        after_tables, before_tables,
        "sections, Sequence, links and every other table are unchanged"
    );
    let after_identity: (
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT id, song_id, rendition_attribution, created_at, archived_at, \
             pagination_state FROM arrangement WHERE id = ?1",
            params![created.id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("arrangement identity");
    assert_eq!(
        after_identity, before_identity,
        "only the title and notes change in the arrangement row"
    );
    assert_eq!(
        arrangement_languages(&conn, created.id),
        vec![(0, "Cebuano".to_owned()), (1, "Latin".to_owned())],
        "the language rows change as asked"
    );
    let (title, notes, pagination): (Option<String>, Option<String>, String) = conn
        .query_row(
            "SELECT title, notes, pagination_state FROM arrangement WHERE id = ?1",
            params![created.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("stored arrangement");
    assert_eq!(title, None);
    assert_eq!(notes.as_deref(), Some("Source: revised"));
    assert_eq!(pagination, "approved");
    drop(conn);
    shutdown(storage);
}

#[test]
fn t2_6_format_1_revision_still_reads_and_verifies() {
    let dir = TempDir::new("format-1");
    shutdown(open(&dir));

    let song = SongId::from_entity(entity_id(0x71));
    let arrangement = ArrangementId::from_entity(entity_id(0x72));
    let choir = ChoirId::from_entity(entity_id(0x73));
    let section = SectionId::from_entity(entity_id(0x74));
    let occurrence = OccurrenceId::from_entity(entity_id(0x75));
    let revision = RevisionId::from_entity(entity_id(0x76));
    {
        let conn = raw(&dir);
        conn.execute(
            "INSERT INTO song (id, title, created_at) VALUES (?1, 'Tinapay sa Langit', ?2)",
            params![song.to_string(), STAMP],
        )
        .expect("song");
        conn.execute(
            "INSERT INTO choir (id, name, created_at) VALUES (?1, 'Koro', ?2)",
            params![choir.to_string(), STAMP],
        )
        .expect("choir");
        conn.execute(
            "INSERT INTO arrangement \
             (id, song_id, title, notes, rendition_attribution, created_at) \
             VALUES (?1, ?2, 'Legacy SATB', NULL, 'Arr. Cruz', ?3)",
            params![arrangement.to_string(), song.to_string(), STAMP],
        )
        .expect("arrangement");
        for (position, language) in ["Cebuano", "English"].into_iter().enumerate() {
            conn.execute(
                "INSERT INTO arrangement_language (arrangement_id, position, language) \
                 VALUES (?1, ?2, ?3)",
                params![arrangement.to_string(), position as i64, language],
            )
            .expect("language");
        }
        conn.execute(
            "INSERT INTO arrangement_choir (arrangement_id, choir_id) VALUES (?1, ?2)",
            params![arrangement.to_string(), choir.to_string()],
        )
        .expect("choir link");
        conn.execute(
            "INSERT INTO arrangement_occasion (arrangement_id, occasion) VALUES (?1, 'Christmas')",
            params![arrangement.to_string()],
        )
        .expect("occasion");
        conn.execute(
            "INSERT INTO arrangement_section \
             (id, arrangement_id, position, label, lyrics, created_at) \
             VALUES (?1, ?2, 0, 'Verse 1', 'Tinapay sa langit', ?3)",
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
        conn.execute(
            "INSERT INTO edit_revision \
             (id, song_id, arrangement_id, parent_id, kind, session_id, restored_from_id, \
             content, content_sha256, created_at) \
             VALUES (?1, NULL, ?2, NULL, 'creation', NULL, NULL, ?3, ?4, ?5)",
            params![
                revision.to_string(),
                arrangement.to_string(),
                FORMAT_1_CONTENT,
                FORMAT_1_SHA256,
                STAMP
            ],
        )
        .expect("format-1 revision");
    }

    let storage = open(&dir);
    let entity = RevisionEntity::Arrangement(arrangement);
    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].id, revision);
    assert_eq!(chain[0].content_sha256.to_string(), FORMAT_1_SHA256);
    let expected = ArrangementContent {
        title: Some("Legacy SATB".to_owned()),
        languages: vec!["Cebuano".to_owned(), "English".to_owned()],
        choirs: vec![choir],
        rendition_attribution: Some("Arr. Cruz".to_owned()),
        occasions: vec!["Christmas".to_owned()],
        notes: None,
        sections: vec![SectionContent {
            id: section,
            label: Some("Verse 1".to_owned()),
            lyrics: "Tinapay sa langit".to_owned(),
        }],
        sequence: vec![OccurrenceContent {
            id: occurrence,
            section,
        }],
    };
    assert_eq!(
        bounded(storage.revision_content(revision)),
        Ok(RevisionContent::Arrangement(expected.clone()))
    );
    assert_eq!(
        bounded(storage.latest_revision(entity)),
        Ok(revision),
        "a legacy revision is already the head"
    );

    // Reading never re-serializes or rewrites the stored bytes.
    let conn = raw(&dir);
    let (stored, stored_hash): (String, String) = conn
        .query_row(
            "SELECT content, content_sha256 FROM edit_revision WHERE id = ?1",
            params![revision.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored revision");
    assert_eq!(stored, FORMAT_1_CONTENT);
    assert_eq!(stored_hash, FORMAT_1_SHA256);
    drop(conn);

    // Restoring the legacy revision writes a canonical format-2 payload.
    let (restored, restored_revision) =
        bounded(storage.restore_revision(revision, revision, PaginationState::Draft))
            .expect("restore legacy");
    assert_eq!(restored, RevisionContent::Arrangement(expected));
    assert_eq!(restored_revision.kind, RevisionKind::Restoration);
    assert_eq!(restored_revision.parent, Some(revision));
    assert_eq!(restored_revision.restored_from, Some(revision));
    let record = bounded(storage.arrangement(arrangement)).expect("arrangement");
    assert_eq!(record.title.as_deref(), Some("Legacy SATB"));
    assert_eq!(record.notes, None);
    assert_eq!(record.languages, vec!["Cebuano", "English"]);
    assert_eq!(record.choirs, vec![choir]);
    assert_eq!(record.occasions, vec!["Christmas"]);
    assert_eq!(record.sections.len(), 1);
    assert_eq!(record.sequence.len(), 1);

    let conn = raw(&dir);
    let (written, written_hash): (String, String) = conn
        .query_row(
            "SELECT content, content_sha256 FROM edit_revision WHERE id = ?1",
            params![restored_revision.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("restoration revision");
    assert_eq!(written, FORMAT_2_CONTENT);
    assert_eq!(written_hash, FORMAT_2_SHA256);
    assert!(!written.contains("\"notes\""), "absent notes are omitted");
    assert_eq!(foreign_key_violations(&conn), Vec::new());
    drop(conn);
    shutdown(storage);
}

#[test]
fn t2_6_restores_older_revision() {
    let dir = TempDir::new("restore-older");
    let storage = open(&dir);
    let (song, choir) = seeded(&storage);
    let created =
        bounded(storage.create_arrangement(arrangement_input(song, choir))).expect("arrangement");
    let entity = RevisionEntity::Arrangement(created.id);
    let creation = bounded(storage.latest_revision(entity)).expect("creation");
    let original = bounded(storage.arrangement(created.id)).expect("original");
    let original_content = ArrangementContent::of(&original);

    let mut edited_content = original_content.clone();
    edited_content.title = Some("Revised title".to_owned());
    edited_content.notes = Some("Source: revised".to_owned());
    edited_content.languages = vec!["Cebuano".to_owned(), "Latin".to_owned()];
    edited_content.choirs.clear();
    edited_content.occasions = vec!["Christmas".to_owned(), "Lent".to_owned()];
    let (edited, edit_revision) = bounded(storage.commit_arrangement_edit(
        created.id,
        edited_content,
        PaginationState::Draft,
        RevisionKind::Edit,
        Some(session(0x46)),
        creation,
    ))
    .expect("metadata edit");
    assert_eq!(edited.title.as_deref(), Some("Revised title"));
    assert_eq!(edited.notes.as_deref(), Some("Source: revised"));
    assert_eq!(edited.languages, vec!["Cebuano", "Latin"]);
    assert_eq!(edited.choirs, Vec::new());
    assert_eq!(edited.occasions, vec!["Christmas", "Lent"]);

    let (restored, restored_revision) =
        bounded(storage.restore_revision(creation, edit_revision.id, PaginationState::Draft))
            .expect("restore creation");
    assert_eq!(
        restored,
        RevisionContent::Arrangement(original_content.clone())
    );
    assert_eq!(restored_revision.kind, RevisionKind::Restoration);
    assert_eq!(restored_revision.parent, Some(edit_revision.id));
    assert_eq!(restored_revision.restored_from, Some(creation));
    assert_eq!(restored_revision.session, None);

    let current = bounded(storage.arrangement(created.id)).expect("restored");
    assert_eq!(current.title, original.title);
    assert_eq!(current.notes, original.notes);
    assert_eq!(current.languages, original.languages);
    assert_eq!(current.choirs, original.choirs);
    assert_eq!(current.occasions, original.occasions);
    assert_eq!(current.sections, original.sections);
    assert_eq!(current.sequence, original.sequence);
    assert_eq!(current.pagination, original.pagination);
    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(
        chain.iter().map(|record| record.id).collect::<Vec<_>>(),
        vec![creation, edit_revision.id, restored_revision.id]
    );
    assert_eq!(
        bounded(storage.revision_content(restored_revision.id)),
        Ok(RevisionContent::Arrangement(original_content))
    );
    let conn = raw(&dir);
    assert_eq!(foreign_key_violations(&conn), Vec::new());
    drop(conn);
    shutdown(storage);
}
