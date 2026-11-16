//! T2.9 WP3 storage acceptance tests: History reads and the legacy-restore
//! state rule (AC1, AC3). Every await runs under a 10 s bound so a hang fails
//! the test instead of blocking the run. Databases live in unique temporary
//! directories.

use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::{
    ArrangementId, EntityId, HistorySessionId, OccurrenceId, RevisionId, SectionId, SongId,
};
use domain::revisions::{RevisionEntity, RevisionKind};
use domain::song_library::PaginationState;
use futures_lite::future::block_on;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use storage::{
    ArrangementContent, ArrangementRecord, NewArrangement, NewSection, NewSong, RevisionContent,
    RevisionRecord, ShutdownOutcome, SongContent, Storage, StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);
const STAMP: &str = "2023-11-14T22:13:20.000Z";

/// A stored format-2 arrangement document whose two sections belong to the
/// current Arrangement in the carry fixture. The identities are built through
/// `section_id` so they match the rows exactly.
fn carry_format_2_content() -> String {
    let verse = section_id(0x51);
    let refrain = section_id(0x52);
    let occurrence = occurrence_id(0x53);
    format!(
        "{{\"format\":2,\"entity\":\"arrangement\",\"title\":\"Legacy SATB\",\
         \"languages\":[\"Cebuano\"],\"choirs\":[],\"rendition_attribution\":null,\
         \"occasions\":[],\"sections\":[{{\"id\":\"{verse}\",\"label\":\"Verse\",\
         \"lyrics\":\"Alpha\"}},{{\"id\":\"{refrain}\",\"label\":\"Refrain\",\
         \"lyrics\":\"Bravo\"}}],\"sequence\":[{{\"id\":\"{occurrence}\",\
         \"section\":\"{verse}\"}}]}}"
    )
}

/// A stored format-1 arrangement document: one section keeps its words, one
/// changed, and one identity the current Arrangement no longer has.
fn changed_format_1_content() -> String {
    let verse = section_id(0x51);
    let refrain = section_id(0x52);
    let bridge = section_id(0x5a);
    let occurrence = occurrence_id(0x53);
    format!(
        "{{\"format\":1,\"entity\":\"arrangement\",\"title\":\"Legacy SATB\",\
         \"languages\":[\"Cebuano\"],\"choirs\":[],\"rendition_attribution\":null,\
         \"occasions\":[],\"sections\":[{{\"id\":\"{verse}\",\"label\":\"Verse\",\
         \"lyrics\":\"Alpha\"}},{{\"id\":\"{refrain}\",\"label\":\"Refrain\",\
         \"lyrics\":\"Bravo then\"}},{{\"id\":\"{bridge}\",\"label\":\"Bridge\",\
         \"lyrics\":\"Delta\"}}],\"sequence\":[{{\"id\":\"{occurrence}\",\
         \"section\":\"{verse}\"}}]}}"
    )
}

/// A format-1 revision of the same Arrangement, exactly as an old build wrote
/// it, used to prove History marks legacy documents.
const LEGACY_FORMAT_1_CONTENT: &str = concat!(
    r#"{"format":1,"entity":"arrangement","title":"Legacy SATB","#,
    r#""languages":["Cebuano","English"],"#,
    r#""choirs":["73737373-7373-4373-b373-737373737373"],"#,
    r#""rendition_attribution":"Arr. Cruz","occasions":["Christmas"],"#,
    r#""sections":[{"id":"74747474-7474-4474-b474-747474747474","#,
    r#""label":"Verse 1","lyrics":"Tinapay sa langit"}],"#,
    r#""sequence":[{"id":"75757575-7575-4575-b575-757575757575","#,
    r#""section":"74747474-7474-4474-b474-747474747474"}]}"#,
);

/// A format-2 revision of the same Arrangement.
const LEGACY_FORMAT_2_CONTENT: &str = concat!(
    r#"{"format":2,"entity":"arrangement","title":"Legacy SATB","#,
    r#""languages":["Cebuano","English"],"#,
    r#""choirs":["73737373-7373-4373-b373-737373737373"],"#,
    r#""rendition_attribution":"Arr. Cruz","occasions":["Christmas"],"#,
    r#""sections":[{"id":"74747474-7474-4474-b474-747474747474","#,
    r#""label":"Verse 1","lyrics":"Tinapay sa langit"}],"#,
    r#""sequence":[{"id":"75757575-7575-4575-b575-757575757575","#,
    r#""section":"74747474-7474-4474-b474-747474747474"}]}"#,
);

/// A format-2 head revision of the Approved Arrangement in
/// [`migrated_approved_parish`].
const V6_APPROVED_HEAD_CONTENT: &str = concat!(
    r#"{"format":2,"entity":"arrangement","title":"Arrangement approved","lan"#,
    r#"guages":["Cebuano"],"choirs":[],"rendition_attribution":null,"occasion"#,
    r#"s":[],"sections":[{"id":"31313131-3131-4131-b131-313131313131","label""#,
    r#":"Verse","lyrics":"Lyrics"},{"id":"32323232-3232-4232-b232-32323232323"#,
    r#"2","label":"Verse","lyrics":"Lyrics"}],"sequence":[]}"#,
);

/// SHA-256 of exactly [`V6_APPROVED_HEAD_CONTENT`], computed outside the test.
const V6_APPROVED_HEAD_SHA256: &str =
    "fe177a0a5a07348cbc70676f883cfc7cfff6e156f05710bf7efc1ac6e9f37bdc";

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
            "ordo-history-tests-{tag}-{}-{}-{nanos}",
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

/// Creates the schema so direct rows can be planted before a real open.
fn bootstrap(dir: &TempDir) {
    shutdown(open(dir));
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

fn section_id(byte: u8) -> SectionId {
    SectionId::from_entity(entity_id(byte))
}

fn occurrence_id(byte: u8) -> OccurrenceId {
    OccurrenceId::from_entity(entity_id(byte))
}

fn song_input(title: &str) -> NewSong {
    NewSong {
        title: title.to_owned(),
        aliases: vec!["Lord".to_owned()],
        original_attribution: Some("Anon".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Lent".to_owned()],
        occasions: Vec::new(),
    }
}

fn song_content(title: &str) -> SongContent {
    SongContent {
        title: title.to_owned(),
        aliases: vec!["Lord".to_owned()],
        original_attribution: Some("Anon".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Lent".to_owned()],
        occasions: Vec::new(),
    }
}

fn states(record: &ArrangementRecord) -> Vec<PaginationState> {
    record.sections.iter().map(|s| s.pagination).collect()
}

/// Directly plants the Song row every legacy fixture needs.
fn insert_song(conn: &Connection, id: SongId, title: &str) {
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![id.to_string(), title, STAMP],
    )
    .expect("song");
}

/// Directly plants an Arrangement row with one Cebuano language.
fn insert_arrangement(conn: &Connection, id: ArrangementId, song: SongId) {
    conn.execute(
        "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id.to_string(), song.to_string(), "Legacy SATB", STAMP],
    )
    .expect("arrangement");
    conn.execute(
        "INSERT INTO arrangement_language (arrangement_id, position, language) \
         VALUES (?1, 0, 'Cebuano')",
        params![id.to_string()],
    )
    .expect("language");
}

/// Directly plants one section row in a given pagination state.
fn insert_section(
    conn: &Connection,
    arrangement: ArrangementId,
    id: SectionId,
    position: usize,
    label: &str,
    lyrics: &str,
    state: &str,
) {
    conn.execute(
        "INSERT INTO arrangement_section \
         (id, arrangement_id, position, label, lyrics, pagination_state, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id.to_string(),
            arrangement.to_string(),
            position as i64,
            label,
            lyrics,
            state,
            STAMP
        ],
    )
    .expect("section");
}

/// Directly plants one Sequence occurrence row.
fn insert_occurrence(
    conn: &Connection,
    arrangement: ArrangementId,
    id: OccurrenceId,
    section: SectionId,
    position: usize,
) {
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

/// Directly plants one revision row with its canonical content and hash. The
/// kind follows the parent link, and an `edit` row carries `session_id`.
fn insert_revision(
    conn: &Connection,
    id: RevisionId,
    arrangement: ArrangementId,
    parent: Option<RevisionId>,
    session: Option<HistorySessionId>,
    content: &str,
) {
    conn.execute(
        "INSERT INTO edit_revision \
         (id, song_id, arrangement_id, parent_id, kind, session_id, restored_from_id, \
         content, content_sha256, created_at) \
         VALUES (?1, NULL, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8)",
        params![
            id.to_string(),
            arrangement.to_string(),
            parent.map(|parent| parent.to_string()),
            if parent.is_some() { "edit" } else { "creation" },
            session.map(|session| session.to_string()),
            content,
            hex_sha256(content),
            STAMP
        ],
    )
    .expect("revision");
}

/// The edits a created Song supports, with its creation revision id.
struct SongFixture {
    storage: Storage,
    song: SongId,
    creation: RevisionId,
}

fn song_fixture(dir: &TempDir) -> SongFixture {
    let storage = open(dir);
    let song = bounded(storage.create_song(song_input("Kyrie")))
        .expect("song")
        .id;
    let chain = bounded(storage.revisions(RevisionEntity::Song(song))).expect("revisions");
    SongFixture {
        storage,
        song,
        creation: chain[0].id,
    }
}

fn edit_song(
    fixture: &SongFixture,
    title: &str,
    session_id: HistorySessionId,
    parent: RevisionId,
) -> RevisionRecord {
    bounded(fixture.storage.commit_song_edit(
        fixture.song,
        song_content(title),
        RevisionKind::Edit,
        Some(session_id),
        parent,
    ))
    .expect("song edit")
    .1
}

#[test]
fn ac1_history_lists_each_revision_with_verified_content_oldest_first() {
    let dir = TempDir::new("list");
    let fixture = song_fixture(&dir);
    let first = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation);
    let second = edit_song(&fixture, "Kyrie 3", session(1), first.id);
    shutdown(fixture.storage);

    let storage = open(&dir);
    let entity = RevisionEntity::Song(fixture.song);
    let history = bounded(storage.history(entity)).expect("history");
    assert_eq!(
        history
            .iter()
            .map(|revision| revision.record.id)
            .collect::<Vec<_>>(),
        vec![fixture.creation, first.id, second.id],
        "oldest first, following the chain"
    );
    assert_eq!(history[0].record.kind, RevisionKind::Creation);
    assert_eq!(history[1].record.kind, RevisionKind::Edit);
    assert_eq!(history[1].record.parent, Some(fixture.creation));
    assert_eq!(history[2].record.parent, Some(first.id));
    assert_eq!(
        history
            .iter()
            .map(|revision| revision.content.clone())
            .collect::<Vec<_>>(),
        vec![
            RevisionContent::Song(song_content("Kyrie")),
            RevisionContent::Song(song_content("Kyrie 2")),
            RevisionContent::Song(song_content("Kyrie 3")),
        ],
        "each document is decoded through the hash-checked path"
    );
    assert!(
        history.iter().all(|revision| revision.pagination_recorded),
        "a Song document always records its content"
    );
    shutdown(storage);

    // A damaged document is reported, never skipped or repaired.
    let conn = raw(&dir);
    conn.execute(
        "UPDATE edit_revision SET content = content || ' ' WHERE id = ?1",
        params![first.id.to_string()],
    )
    .expect("tamper");
    drop(conn);

    let storage = open(&dir);
    assert_eq!(
        bounded(storage.history(entity)),
        Err(StorageError::RevisionIntegrity { id: first.id })
    );
    shutdown(storage);
}

#[test]
fn ac1_history_marks_legacy_arrangement_documents_as_unrecorded() {
    let dir = TempDir::new("legacy-marks");
    bootstrap(&dir);
    let song = SongId::from_entity(entity_id(0x55));
    let arrangement = ArrangementId::from_entity(entity_id(0x56));
    let first = RevisionId::from_entity(entity_id(0x54));
    let second = RevisionId::from_entity(entity_id(0x57));
    {
        let conn = raw(&dir);
        insert_song(&conn, song, "Legacy Song");
        insert_arrangement(&conn, arrangement, song);
        insert_revision(
            &conn,
            first,
            arrangement,
            None,
            None,
            LEGACY_FORMAT_1_CONTENT,
        );
        insert_revision(
            &conn,
            second,
            arrangement,
            Some(first),
            Some(session(0x58)),
            LEGACY_FORMAT_2_CONTENT,
        );
    }

    let storage = open(&dir);
    let history =
        bounded(storage.history(RevisionEntity::Arrangement(arrangement))).expect("history");
    assert_eq!(
        history
            .iter()
            .map(|revision| revision.record.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    assert!(
        history.iter().all(|revision| !revision.pagination_recorded),
        "formats 1 and 2 recorded no section states"
    );
    let RevisionContent::Arrangement(decoded) = &history[0].content else {
        panic!("a legacy document decodes as Arrangement content");
    };
    assert_eq!(
        decoded
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        vec![PaginationState::Draft],
        "a legacy document decodes every section as Draft"
    );

    // A document this build wrote reports recorded states.
    let current = bounded(storage.create_arrangement(NewArrangement {
        song,
        title: Some("Current SATB".to_owned()),
        languages: vec!["Cebuano".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![NewSection {
            label: Some("Verse".to_owned()),
            lyrics: "Words".to_owned(),
        }],
        sequence: vec![0],
    }))
    .expect("current arrangement");
    let current_history =
        bounded(storage.history(RevisionEntity::Arrangement(current.id))).expect("history");
    assert_eq!(current_history.len(), 1);
    assert!(
        current_history[0].pagination_recorded,
        "a format-3 document records its section states"
    );
    shutdown(storage);
}

#[test]
fn ac1_song_history_has_no_arrangement_revision() {
    let dir = TempDir::new("ownership");
    let fixture = song_fixture(&dir);
    let song_edit = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation);
    let arrangement = bounded(fixture.storage.create_arrangement(NewArrangement {
        song: fixture.song,
        title: Some("SATB".to_owned()),
        languages: vec!["Cebuano".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![NewSection {
            label: Some("Verse".to_owned()),
            lyrics: "Alpha".to_owned(),
        }],
        sequence: vec![0],
    }))
    .expect("arrangement");
    let mut content = ArrangementContent::of(&arrangement);
    content.sections[0].lyrics = "Beta".to_owned();
    bounded(fixture.storage.commit_arrangement_lyrics(
        arrangement.id,
        content.sections,
        content.sequence,
        session(2),
    ))
    .expect("lyrics edit");

    let song_history =
        bounded(fixture.storage.history(RevisionEntity::Song(fixture.song))).expect("song history");
    assert_eq!(
        song_history
            .iter()
            .map(|revision| revision.record.id)
            .collect::<Vec<_>>(),
        vec![fixture.creation, song_edit.id]
    );
    assert!(
        song_history
            .iter()
            .all(|revision| matches!(revision.content, RevisionContent::Song(_))),
        "a Song's History never lists an Arrangement revision"
    );

    let arrangement_history = bounded(
        fixture
            .storage
            .history(RevisionEntity::Arrangement(arrangement.id)),
    )
    .expect("arrangement history");
    assert_eq!(arrangement_history.len(), 2);
    assert!(
        arrangement_history
            .iter()
            .all(|revision| matches!(revision.content, RevisionContent::Arrangement(_))),
        "an Arrangement's History never lists a Song revision"
    );
    shutdown(fixture.storage);
}

#[test]
fn ac3_restoring_a_format_2_revision_carries_matching_section_states() {
    let dir = TempDir::new("carry-format-2");
    bootstrap(&dir);
    let song = SongId::from_entity(entity_id(0x55));
    let arrangement = ArrangementId::from_entity(entity_id(0x56));
    let revision = RevisionId::from_entity(entity_id(0x54));
    {
        let conn = raw(&dir);
        insert_song(&conn, song, "Legacy Song");
        insert_arrangement(&conn, arrangement, song);
        insert_section(
            &conn,
            arrangement,
            section_id(0x51),
            0,
            "Verse",
            "Alpha",
            "approved",
        );
        insert_section(
            &conn,
            arrangement,
            section_id(0x52),
            1,
            "Refrain",
            "Bravo",
            "needs_review",
        );
        insert_occurrence(&conn, arrangement, occurrence_id(0x53), section_id(0x51), 0);
        insert_revision(
            &conn,
            revision,
            arrangement,
            None,
            None,
            &carry_format_2_content(),
        );
    }

    let storage = open(&dir);
    let (content, restoration) =
        bounded(storage.restore_revision(revision, revision)).expect("restore");
    let RevisionContent::Arrangement(content) = content else {
        panic!("a legacy Arrangement restores as Arrangement content");
    };
    assert_eq!(
        content
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        vec![PaginationState::Approved, PaginationState::NeedsReview],
        "each restored section takes the state of the current section with its identity"
    );
    assert_eq!(restoration.kind, RevisionKind::Restoration);
    assert_eq!(restoration.restored_from, Some(revision));
    assert_eq!(restoration.parent, Some(revision));
    let stored = bounded(storage.arrangement(arrangement)).expect("arrangement");
    assert_eq!(
        states(&stored),
        vec![PaginationState::Approved, PaginationState::NeedsReview],
        "the carried states are what was written"
    );
    shutdown(storage);
}

#[test]
fn ac3_restoring_a_format_1_revision_returns_changed_approved_sections_to_review() {
    let dir = TempDir::new("carry-format-1");
    bootstrap(&dir);
    let song = SongId::from_entity(entity_id(0x55));
    let arrangement = ArrangementId::from_entity(entity_id(0x56));
    let revision = RevisionId::from_entity(entity_id(0x54));
    {
        let conn = raw(&dir);
        insert_song(&conn, song, "Legacy Song");
        insert_arrangement(&conn, arrangement, song);
        insert_section(
            &conn,
            arrangement,
            section_id(0x51),
            0,
            "Verse",
            "Alpha",
            "approved",
        );
        insert_section(
            &conn,
            arrangement,
            section_id(0x52),
            1,
            "Refrain",
            "Bravo now",
            "approved",
        );
        insert_occurrence(&conn, arrangement, occurrence_id(0x53), section_id(0x51), 0);
        insert_revision(
            &conn,
            revision,
            arrangement,
            None,
            None,
            &changed_format_1_content(),
        );
    }

    let storage = open(&dir);
    let (content, _) = bounded(storage.restore_revision(revision, revision)).expect("restore");
    let RevisionContent::Arrangement(content) = content else {
        panic!("a legacy Arrangement restores as Arrangement content");
    };
    assert_eq!(
        content
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        vec![
            PaginationState::Approved,
            PaginationState::NeedsReview,
            PaginationState::Draft
        ],
        "unchanged approved words stay approved, changed words return to review, \
         and an identity the current Arrangement lacks stays Draft"
    );
    let stored = bounded(storage.arrangement(arrangement)).expect("arrangement");
    assert_eq!(
        states(&stored),
        vec![
            PaginationState::Approved,
            PaginationState::NeedsReview,
            PaginationState::Draft
        ]
    );
    shutdown(storage);
}

#[test]
fn ac3_restoring_a_migrated_approved_head_keeps_it_approved() {
    let dir = TempDir::new("migrated-head");
    bootstrap(&dir);
    let song = SongId::from_entity(entity_id(0x10));
    let arrangement = ArrangementId::from_entity(entity_id(0x11));
    let head = RevisionId::from_entity(entity_id(0x90));
    {
        let conn = raw(&dir);
        insert_song(&conn, song, "Tinapay sa Langit");
        insert_arrangement(&conn, arrangement, song);
        for (position, section) in [section_id(0x31), section_id(0x32)].into_iter().enumerate() {
            insert_section(
                &conn,
                arrangement,
                section,
                position,
                "Verse",
                "Lyrics",
                "approved",
            );
        }
        insert_revision(
            &conn,
            head,
            arrangement,
            None,
            None,
            V6_APPROVED_HEAD_CONTENT,
        );
        // Prove the fixture hash matches the copied payload.
        let stored: String = conn
            .query_row(
                "SELECT content_sha256 FROM edit_revision WHERE id = ?1",
                params![head.to_string()],
                |row| row.get(0),
            )
            .expect("stored hash");
        assert_eq!(stored, V6_APPROVED_HEAD_SHA256);
    }

    let storage = open(&dir);
    let (content, restoration) =
        bounded(storage.restore_revision(head, head)).expect("restore the migrated head");
    let RevisionContent::Arrangement(content) = content else {
        panic!("a migrated Arrangement restores as Arrangement content");
    };
    assert_eq!(
        content
            .sections
            .iter()
            .map(|section| section.pagination)
            .collect::<Vec<_>>(),
        vec![PaginationState::Approved; 2],
        "restoring the migrated Approved head never un-approves matching sections"
    );
    assert_eq!(restoration.kind, RevisionKind::Restoration);
    assert_eq!(
        states(&bounded(storage.arrangement(arrangement)).expect("arrangement")),
        vec![PaginationState::Approved; 2]
    );
    shutdown(storage);
}

#[test]
fn ac3_restore_keeps_every_intervening_revision() {
    let dir = TempDir::new("keep-revisions");
    let fixture = song_fixture(&dir);
    let first = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation);
    let second = edit_song(&fixture, "Kyrie 3", session(1), first.id);
    let third = edit_song(&fixture, "Kyrie 4", session(1), second.id);
    bounded(fixture.storage.restore_revision(fixture.creation, third.id)).expect("restore");

    let history =
        bounded(fixture.storage.history(RevisionEntity::Song(fixture.song))).expect("history");
    assert_eq!(
        history
            .iter()
            .map(|revision| revision.record.id)
            .collect::<Vec<_>>(),
        vec![
            fixture.creation,
            first.id,
            second.id,
            third.id,
            history[4].record.id
        ],
        "the restoration appends; no intervening revision is erased"
    );
    assert_eq!(history[4].record.kind, RevisionKind::Restoration);
    assert_eq!(history[4].record.restored_from, Some(fixture.creation));
    assert_eq!(
        history[0].content,
        RevisionContent::Song(song_content("Kyrie"))
    );
    assert_eq!(
        history[1].content,
        RevisionContent::Song(song_content("Kyrie 2"))
    );
    assert_eq!(
        history[2].content,
        RevisionContent::Song(song_content("Kyrie 3"))
    );
    assert_eq!(
        history[3].content,
        RevisionContent::Song(song_content("Kyrie 4"))
    );
    assert_eq!(
        history[4].content,
        RevisionContent::Song(song_content("Kyrie"))
    );
    shutdown(fixture.storage);
}

#[test]
fn ac3_undo_of_a_restore_is_a_second_restoration() {
    let dir = TempDir::new("undo-restore");
    let fixture = song_fixture(&dir);
    let edited = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation);
    let (_, restoration) = bounded(
        fixture
            .storage
            .restore_revision(fixture.creation, edited.id),
    )
    .expect("restore");
    let (content, undo) =
        bounded(fixture.storage.restore_revision(edited.id, restoration.id)).expect("undo");

    assert_eq!(
        content,
        RevisionContent::Song(song_content("Kyrie 2")),
        "Undo restores the revision that was current before the restore"
    );
    assert_eq!(undo.kind, RevisionKind::Restoration);
    assert_eq!(undo.restored_from, Some(edited.id));
    assert_eq!(undo.parent, Some(restoration.id));
    let stored = bounded(fixture.storage.song(fixture.song)).expect("song");
    assert_eq!(SongContent::of(&stored), song_content("Kyrie 2"));
    let history =
        bounded(fixture.storage.history(RevisionEntity::Song(fixture.song))).expect("history");
    assert_eq!(history.len(), 4);
    assert_eq!(history[2].record, restoration);
    assert_eq!(
        history[2].content,
        RevisionContent::Song(song_content("Kyrie"))
    );
    assert_eq!(history[3].record, undo);
    shutdown(fixture.storage);
}
