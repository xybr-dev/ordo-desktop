//! T2.8 WP3 storage acceptance tests: migration 0007 (per-section pagination
//! state), revision payload format 3, and Sequence persistence. Every await runs
//! under a 10 s bound so a hang fails the test instead of blocking the run.
//! Databases live in unique temporary directories.

use std::fs;
use std::future::Future;
use std::path::PathBuf;
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
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use storage::{
    ArrangementContent, ArrangementMetadata, ArrangementRecord, NewArrangement, NewChoir,
    NewSection, NewSong, RevisionContent, ShutdownOutcome, Storage, StorageConfig,
};

const BOUND: Duration = Duration::from_secs(10);
const STAMP: &str = "2023-11-14T22:13:20.000Z";

/// A stored format-1 Arrangement payload, exactly as an old build wrote it.
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

/// SHA-256 of exactly [`FORMAT_1_CONTENT`], computed outside the test.
const FORMAT_1_SHA256: &str = "c1727ace61b9cfb312daf88688f0cb659f8eb2c00d24ebaa95ddee14745042ee";

/// A stored format-2 payload for the same Arrangement.
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

/// SHA-256 of exactly [`FORMAT_2_CONTENT`], computed outside the test.
const FORMAT_2_SHA256: &str = "e62100b812dfcd1e2c2e58cb5f3f020d5a07f23ed2f7ecc6fddd114f91ab8aee";

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
            "ordo-sequence-tests-{tag}-{}-{}-{nanos}",
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

fn hex_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn seeded(storage: &Storage) -> ArrangementRecord {
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Koro".to_owned(),
        short_label: None,
        languages: vec!["Cebuano".to_owned()],
        notes: None,
    }))
    .expect("choir");
    let song = bounded(storage.create_song(NewSong {
        title: "Tinapay sa Langit".to_owned(),
        aliases: Vec::new(),
        original_attribution: None,
        provenance: None,
        rights_notes: None,
        liturgical_moments: Vec::new(),
        occasions: Vec::new(),
    }))
    .expect("song");
    bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: Some("Cebuano SATB".to_owned()),
        notes: None,
        languages: vec!["Cebuano".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: None,
        occasions: Vec::new(),
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
    }))
    .expect("arrangement")
}

fn states(record: &ArrangementRecord) -> Vec<PaginationState> {
    record.sections.iter().map(|s| s.pagination).collect()
}

fn latest(storage: &Storage, record: &ArrangementRecord) -> RevisionId {
    bounded(storage.latest_revision(RevisionEntity::Arrangement(record.id))).expect("latest")
}

/// Commits `content` as a lyrics revision (sections and Sequence only).
fn commit(
    storage: &Storage,
    record: &ArrangementRecord,
    content: ArrangementContent,
    byte: u8,
) -> ArrangementRecord {
    bounded(storage.commit_arrangement_lyrics(
        record.id,
        content.sections,
        content.sequence,
        session(byte),
    ))
    .expect("lyrics commit")
    .0
}

/// The migration chain up to a version-6 database: the schema a 0006 build left.
fn apply_to_v6(dir: &TempDir) -> Connection {
    let migrations = [
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
        (
            "0006_arrangement_details",
            include_str!("../../../migrations/0006_arrangement_details.sql"),
        ),
    ];
    let conn = Connection::open(dir.db()).expect("raw connection");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("foreign keys");
    conn.execute_batch(
        "CREATE TABLE ordo_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL, \
         checksum TEXT NOT NULL, applied_at TEXT NOT NULL)",
    )
    .expect("ledger");
    for (index, (name, sql)) in migrations.iter().enumerate() {
        conn.execute_batch(sql).expect("apply migration");
        conn.execute(
            "INSERT INTO ordo_migrations (id, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            params![index as i64 + 1, name, hex_sha256(sql), STAMP],
        )
        .expect("ledger row");
    }
    conn.pragma_update(None, "user_version", 6)
        .expect("version");
    conn
}

/// A version-6 parish with three Arrangements, one per pagination state, each
/// holding two sections. Returns the Arrangements with their states.
fn v6_parish(dir: &TempDir) -> Vec<(ArrangementId, &'static str, PaginationState)> {
    let conn = apply_to_v6(dir);
    let song = SongId::from_entity(entity_id(0x10));
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, 'Tinapay sa Langit', ?2)",
        params![song.to_string(), STAMP],
    )
    .expect("song");
    let mut parish = Vec::new();
    for (index, (state, expected)) in [
        ("draft", PaginationState::Draft),
        ("approved", PaginationState::Approved),
        ("needs_review", PaginationState::NeedsReview),
    ]
    .into_iter()
    .enumerate()
    {
        let base = 0x20 + 0x10 * index as u8;
        let arrangement = ArrangementId::from_entity(entity_id(base));
        conn.execute(
            "INSERT INTO arrangement (id, song_id, title, created_at, pagination_state) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                arrangement.to_string(),
                song.to_string(),
                format!("Arrangement {state}"),
                STAMP,
                state
            ],
        )
        .expect("arrangement");
        conn.execute(
            "INSERT INTO arrangement_language (arrangement_id, position, language) \
             VALUES (?1, 0, 'Cebuano')",
            params![arrangement.to_string()],
        )
        .expect("language");
        for position in 0..2u8 {
            conn.execute(
                "INSERT INTO arrangement_section \
                 (id, arrangement_id, position, label, lyrics, created_at) \
                 VALUES (?1, ?2, ?3, 'Verse', 'Lyrics', ?4)",
                params![
                    SectionId::from_entity(entity_id(base + 1 + position)).to_string(),
                    arrangement.to_string(),
                    i64::from(position),
                    STAMP
                ],
            )
            .expect("section");
        }
        parish.push((arrangement, state, expected));
    }
    parish
}

#[test]
fn migration_0007_copies_each_arrangement_state_to_its_sections() {
    let dir = TempDir::new("copy");
    let parish = v6_parish(&dir);

    let storage = open(&dir);
    let status = bounded(storage.schema_status()).expect("status");
    assert_eq!(status.version, 7);
    for (arrangement, label, expected) in parish {
        let record = bounded(storage.arrangement(arrangement)).expect("arrangement");
        assert_eq!(record.sections.len(), 2, "{label}");
        assert_eq!(states(&record), vec![expected; 2], "{label}");
    }
    shutdown(storage);

    let conn = raw(&dir);
    let stored: Vec<(String, i64)> = conn
        .prepare(
            "SELECT pagination_state, COUNT(*) FROM arrangement_section \
             GROUP BY pagination_state ORDER BY pagination_state",
        )
        .expect("count")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("count query")
        .map(|row| row.expect("count row"))
        .collect();
    assert_eq!(
        stored,
        vec![
            ("approved".to_owned(), 2),
            ("draft".to_owned(), 2),
            ("needs_review".to_owned(), 2)
        ]
    );
}

#[test]
fn migration_0007_drops_the_arrangement_column() {
    let dir = TempDir::new("drop");
    {
        // The 0006 table still carries the column and its CHECK.
        let _ = v6_parish(&dir);
    }
    let columns = |conn: &Connection, table: &str| -> Vec<String> {
        conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .expect("table info")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column query")
            .map(|row| row.expect("column"))
            .collect()
    };
    {
        let conn = raw(&dir);
        assert!(columns(&conn, "arrangement").contains(&"pagination_state".to_owned()));
    }
    shutdown(open(&dir));
    let conn = raw(&dir);
    assert!(
        !columns(&conn, "arrangement").contains(&"pagination_state".to_owned()),
        "the Arrangement no longer stores a pagination state"
    );
    assert!(columns(&conn, "arrangement_section").contains(&"pagination_state".to_owned()));
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
    let refused = conn
        .execute(
            "UPDATE arrangement_section SET pagination_state = 'bogus'",
            [],
        )
        .expect_err("the section CHECK refuses an unknown state");
    assert!(refused.to_string().contains("CHECK constraint failed"));
}

#[test]
fn format_3_round_trips_section_states() {
    let dir = TempDir::new("format-3");
    let storage = open(&dir);
    let created = seeded(&storage);
    let mut content = ArrangementContent::of(&created);
    content.sections[0].pagination = PaginationState::Approved;
    content.sections[1].pagination = PaginationState::NeedsReview;
    let committed = commit(&storage, &created, content.clone(), 0x41);
    assert_eq!(
        states(&committed),
        vec![PaginationState::Approved, PaginationState::NeedsReview]
    );
    let head = latest(&storage, &committed);
    assert_eq!(
        bounded(storage.revision_content(head)),
        Ok(RevisionContent::Arrangement(content.clone()))
    );
    shutdown(storage);

    let conn = raw(&dir);
    let (text, hash): (String, String) = conn
        .query_row(
            "SELECT content, content_sha256 FROM edit_revision WHERE id = ?1",
            params![head.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored revision");
    assert!(text.starts_with(r#"{"format":3,"#));
    assert!(text.contains(r#""pagination":"approved""#));
    assert!(text.contains(r#""pagination":"needs_review""#));
    assert_eq!(hash, hex_sha256(&text));
    drop(conn);

    let storage = open(&dir);
    assert_eq!(
        bounded(storage.revision_content(head)),
        Ok(RevisionContent::Arrangement(content)),
        "the revision still verifies and decodes after a restart"
    );
    shutdown(storage);
}

#[test]
fn format_2_and_1_revisions_still_verify_and_decode_as_draft() {
    let dir = TempDir::new("legacy");
    shutdown(open(&dir));

    let song = SongId::from_entity(entity_id(0x71));
    let arrangement = ArrangementId::from_entity(entity_id(0x72));
    let choir = ChoirId::from_entity(entity_id(0x73));
    let section = SectionId::from_entity(entity_id(0x74));
    let occurrence = OccurrenceId::from_entity(entity_id(0x75));
    let first = RevisionId::from_entity(entity_id(0x76));
    let second = RevisionId::from_entity(entity_id(0x77));
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
            "INSERT INTO arrangement (id, song_id, title, rendition_attribution, created_at) \
             VALUES (?1, ?2, 'Legacy SATB', 'Arr. Cruz', ?3)",
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
                first.to_string(),
                arrangement.to_string(),
                FORMAT_1_CONTENT,
                FORMAT_1_SHA256,
                STAMP
            ],
        )
        .expect("format-1 revision");
        conn.execute(
            "INSERT INTO edit_revision \
             (id, song_id, arrangement_id, parent_id, kind, session_id, restored_from_id, \
             content, content_sha256, created_at) \
             VALUES (?1, NULL, ?2, ?3, 'edit', ?4, NULL, ?5, ?6, ?7)",
            params![
                second.to_string(),
                arrangement.to_string(),
                first.to_string(),
                session(0x78).to_string(),
                FORMAT_2_CONTENT,
                FORMAT_2_SHA256,
                STAMP
            ],
        )
        .expect("format-2 revision");
    }

    let storage = open(&dir);
    let expected = ArrangementContent {
        title: Some("Legacy SATB".to_owned()),
        languages: vec!["Cebuano".to_owned(), "English".to_owned()],
        choirs: vec![choir],
        rendition_attribution: Some("Arr. Cruz".to_owned()),
        occasions: vec!["Christmas".to_owned()],
        notes: None,
        sections: vec![storage::SectionContent {
            id: section,
            label: Some("Verse 1".to_owned()),
            lyrics: "Tinapay sa langit".to_owned(),
            pagination: PaginationState::Draft,
        }],
        sequence: vec![storage::OccurrenceContent {
            id: occurrence,
            section,
        }],
    };
    let chain =
        bounded(storage.revisions(RevisionEntity::Arrangement(arrangement))).expect("chain");
    assert_eq!(
        chain.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(chain[0].content_sha256.to_string(), FORMAT_1_SHA256);
    assert_eq!(chain[1].content_sha256.to_string(), FORMAT_2_SHA256);
    for revision in [first, second] {
        assert_eq!(
            bounded(storage.revision_content(revision)),
            Ok(RevisionContent::Arrangement(expected.clone())),
            "a legacy revision verifies against its stored text and decodes as Draft"
        );
    }
    shutdown(storage);

    let conn = raw(&dir);
    let stored: Vec<String> = conn
        .prepare("SELECT content FROM edit_revision ORDER BY created_at, id")
        .expect("stored")
        .query_map([], |row| row.get(0))
        .expect("stored query")
        .map(|row| row.expect("content"))
        .collect();
    assert!(stored.contains(&FORMAT_1_CONTENT.to_owned()));
    assert!(stored.contains(&FORMAT_2_CONTENT.to_owned()));
}

#[test]
fn approving_one_section_writes_one_revision_with_a_new_hash() {
    let dir = TempDir::new("approve-one");
    let storage = open(&dir);
    let created = seeded(&storage);
    let entity = RevisionEntity::Arrangement(created.id);
    let before = bounded(storage.revisions(entity)).expect("chain");
    let before_total: i64 = raw(&dir)
        .query_row("SELECT COUNT(*) FROM edit_revision", [], |row| row.get(0))
        .expect("count");

    let mut content = ArrangementContent::of(&created);
    content.sections[0].pagination = PaginationState::Approved;
    let approved = commit(&storage, &created, content, 0x42);

    assert_eq!(
        states(&approved),
        vec![PaginationState::Approved, PaginationState::Draft]
    );
    let after = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(after.len(), before.len() + 1);
    let new = after.last().expect("new revision");
    assert_eq!(new.kind, RevisionKind::Edit);
    assert_ne!(
        new.content_sha256,
        before.last().expect("old head").content_sha256,
        "the section state is part of the hashed content"
    );
    let after_total: i64 = raw(&dir)
        .query_row("SELECT COUNT(*) FROM edit_revision", [], |row| row.get(0))
        .expect("count");
    assert_eq!(after_total, before_total + 1);
    shutdown(storage);
}

#[test]
fn ac5_removing_an_occurrence_keeps_its_section_and_restore_puts_it_back() {
    let dir = TempDir::new("ac5");
    let storage = open(&dir);
    let created = seeded(&storage);
    assert_eq!(created.sequence.len(), 3);
    let creation = latest(&storage, &created);

    let mut content = ArrangementContent::of(&created);
    let removed = content.sequence.remove(2);
    let edited = commit(&storage, &created, content, 0x43);
    assert_eq!(edited.sections, created.sections, "the section is kept");
    assert_eq!(edited.sequence.len(), 2);
    assert!(edited.sequence.iter().all(|o| o.id != removed.id));

    let head = latest(&storage, &edited);
    bounded(storage.restore_revision(creation, head)).expect("restore");
    let restored = bounded(storage.arrangement(created.id)).expect("arrangement");
    let places = |record: &ArrangementRecord| {
        record
            .sequence
            .iter()
            .map(|o| (o.id, o.section))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        places(&restored),
        places(&created),
        "the occurrence returns with its identity and position"
    );
    assert_eq!(restored.sections, created.sections);
    shutdown(storage);
}

#[test]
fn ac6_an_empty_sequence_persists_across_reopen() {
    let dir = TempDir::new("ac6");
    let storage = open(&dir);
    let created = seeded(&storage);
    let mut content = ArrangementContent::of(&created);
    content.sequence.clear();
    let edited = commit(&storage, &created, content, 0x44);
    assert!(edited.sequence.is_empty());
    assert_eq!(edited.sections.len(), 2);
    shutdown(storage);

    let storage = open(&dir);
    let reopened = bounded(storage.arrangement(created.id)).expect("arrangement");
    assert!(reopened.sequence.is_empty());
    assert_eq!(reopened.sections, edited.sections);
    let head = latest(&storage, &reopened);
    assert_eq!(
        bounded(storage.revision_content(head)),
        Ok(RevisionContent::Arrangement(ArrangementContent::of(
            &reopened
        )))
    );
    shutdown(storage);
}

#[test]
fn a_metadata_commit_keeps_section_approvals() {
    let dir = TempDir::new("metadata");
    let storage = open(&dir);
    let created = seeded(&storage);

    // Approvals first, then metadata: the approvals stay.
    let mut content = ArrangementContent::of(&created);
    content.sections[0].pagination = PaginationState::Approved;
    content.sections[1].pagination = PaginationState::NeedsReview;
    let approved = commit(&storage, &created, content, 0x45);
    let metadata = ArrangementMetadata {
        title: Some("Renamed".to_owned()),
        languages: approved.languages.clone(),
        choirs: approved.choirs.clone(),
        rendition_attribution: approved.rendition_attribution.clone(),
        occasions: approved.occasions.clone(),
        notes: Some("A note".to_owned()),
    };
    let (renamed, _) =
        bounded(storage.commit_arrangement_metadata(created.id, metadata, session(0x46)))
            .expect("metadata commit");
    assert_eq!(renamed.title.as_deref(), Some("Renamed"));
    assert_eq!(states(&renamed), states(&approved));

    // Metadata first, then a lyrics commit: the metadata stays.
    let mut content = ArrangementContent::of(&renamed);
    content.sections[1].pagination = PaginationState::Approved;
    let finished = commit(&storage, &renamed, content, 0x47);
    assert_eq!(finished.title.as_deref(), Some("Renamed"));
    assert_eq!(finished.notes.as_deref(), Some("A note"));
    assert_eq!(states(&finished), vec![PaginationState::Approved; 2]);
    shutdown(storage);
}
