//! T2.2b durable Edit revision acceptance tests. Every await runs under a 10 s
//! bound so a hang fails the test instead of blocking the run. Databases live in
//! unique temporary directories.

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
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use storage::{
    ArrangementContent, ArrangementRecord, NewArrangement, NewChoir, NewSection, NewSong,
    OccurrenceContent, RecordKind, RevisionContent, SectionContent, ShutdownOutcome, SongContent,
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
            "ordo-revision-tests-{tag}-{}-{}-{nanos}",
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

fn section_id(byte: u8) -> SectionId {
    SectionId::from_entity(entity_id(byte))
}

fn occurrence_id(byte: u8) -> OccurrenceId {
    OccurrenceId::from_entity(entity_id(byte))
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
        aliases: vec!["Lord".to_owned()],
        original_attribution: None,
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Lent".to_owned(), "Advent".to_owned()],
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
        occasions: vec!["Easter".to_owned()],
    }
}

fn new_section(label: Option<&str>, lyrics: &str) -> NewSection {
    NewSection {
        label: label.map(str::to_owned),
        lyrics: lyrics.to_owned(),
    }
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
) -> Result<(storage::SongRecord, storage::RevisionRecord), StorageError> {
    bounded(fixture.storage.commit_song_edit(
        fixture.song,
        song_content(title),
        RevisionKind::Edit,
        Some(session_id),
        parent,
    ))
}

#[test]
fn creation_appends_the_first_revision() {
    let dir = TempDir::new("creation");
    let storage = open(&dir);
    let song = bounded(storage.create_song(song_input("Kyrie"))).expect("song");
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    }))
    .expect("choir");
    let arrangement = bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: Some("SATB".to_owned()),
        languages: vec!["en".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: None,
        occasions: vec!["Lent".to_owned()],
        notes: Some("Public domain".to_owned()),
        sections: vec![new_section(Some("A"), "a\nb"), new_section(None, "c")],
        sequence: vec![0, 1, 0],
    }))
    .expect("arrangement");

    let song_chain = bounded(storage.revisions(RevisionEntity::Song(song.id))).expect("chain");
    assert_eq!(song_chain.len(), 1);
    assert_eq!(song_chain[0].kind, RevisionKind::Creation);
    assert_eq!(song_chain[0].parent, None);
    assert_eq!(song_chain[0].session, None);
    assert_eq!(song_chain[0].restored_from, None);
    assert_eq!(
        bounded(storage.revision_content(song_chain[0].id)),
        Ok(RevisionContent::Song(SongContent::of(&song)))
    );
    let arrangement_chain =
        bounded(storage.revisions(RevisionEntity::Arrangement(arrangement.id))).expect("chain");
    assert_eq!(arrangement_chain.len(), 1);
    assert_eq!(
        bounded(storage.revision_content(arrangement_chain[0].id)),
        Ok(RevisionContent::Arrangement(ArrangementContent::of(
            &arrangement
        )))
    );
    shutdown(storage);

    let conn = raw(&dir);
    let (content, stored_hash, kind, parent): (String, String, String, Option<String>) = conn
        .query_row(
            "SELECT content, content_sha256, kind, parent_id FROM edit_revision \
             WHERE song_id = ?1",
            params![song.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("song revision row");
    assert_eq!(kind, "creation");
    assert_eq!(parent, None);
    assert_eq!(stored_hash, hex_sha256(&content));
    assert_eq!(
        content,
        "{\"format\":1,\"entity\":\"song\",\"title\":\"Kyrie\",\"aliases\":[\"Lord\"],\
         \"original_attribution\":null,\"provenance\":null,\"rights_notes\":null,\
         \"liturgical_moments\":[\"Advent\",\"Lent\"],\"occasions\":[]}"
    );

    let (content, stored_hash): (String, String) = conn
        .query_row(
            "SELECT content, content_sha256 FROM edit_revision WHERE arrangement_id = ?1",
            params![arrangement.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("arrangement revision row");
    assert_eq!(stored_hash, hex_sha256(&content));
    let expected_start = format!(
        "{{\"format\":2,\"entity\":\"arrangement\",\"title\":\"SATB\",\"languages\":[\"en\"],\
         \"choirs\":[\"{}\"],\"rendition_attribution\":null,\"occasions\":[\"Lent\"],\
         \"notes\":\"Public domain\",\"sections\":[{{\"id\":\"{}\",\"label\":\"A\",\
         \"lyrics\":\"a\\nb\"}},",
        choir.id, arrangement.sections[0].id
    );
    assert!(content.starts_with(&expected_start), "{content}");
    assert!(content.contains("\"sequence\":[{\"id\":\""), "{content}");
}

#[test]
fn edits_form_one_linear_chain() {
    let dir = TempDir::new("chain");
    let fixture = song_fixture(&dir);
    let first = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation).expect("first");
    let second = bounded(fixture.storage.commit_song_edit(
        fixture.song,
        song_content("Kyrie 3"),
        RevisionKind::Import,
        None,
        first.1.id,
    ))
    .expect("second");
    // Equal content still appends a revision.
    let third = bounded(fixture.storage.commit_song_edit(
        fixture.song,
        song_content("Kyrie 3"),
        RevisionKind::Import,
        None,
        second.1.id,
    ))
    .expect("third");

    assert_eq!(first.0.title, "Kyrie 2");
    assert_eq!(SongContent::of(&third.0), song_content("Kyrie 3"));
    let chain = bounded(
        fixture
            .storage
            .revisions(RevisionEntity::Song(fixture.song)),
    )
    .expect("chain");
    let ids: Vec<RevisionId> = chain.iter().map(|record| record.id).collect();
    assert_eq!(
        ids,
        vec![fixture.creation, first.1.id, second.1.id, third.1.id]
    );
    assert_eq!(chain[0].parent, None);
    assert_eq!(chain[1].parent, Some(fixture.creation));
    assert_eq!(chain[2].parent, Some(first.1.id));
    assert_eq!(chain[3].parent, Some(second.1.id));
    assert_eq!(chain[1].kind, RevisionKind::Edit);
    assert_eq!(chain[1].session, Some(session(1)));
    assert_eq!(chain[2].kind, RevisionKind::Import);
    assert_eq!(chain[3].kind, RevisionKind::Import);
    assert_eq!(chain[2].content_sha256, chain[3].content_sha256);
    assert_eq!(chain[1], first.1);
    assert_eq!(
        bounded(fixture.storage.revision_content(first.1.id)),
        Ok(RevisionContent::Song(song_content("Kyrie 2")))
    );
    assert_eq!(
        bounded(
            fixture
                .storage
                .latest_revision(RevisionEntity::Song(fixture.song))
        ),
        Ok(third.1.id)
    );
    shutdown(fixture.storage);
}

#[test]
fn arrangement_edit_reorders_sections_and_sequence_keeping_ids() {
    let dir = TempDir::new("reorder");
    let storage = open(&dir);
    let song = bounded(storage.create_song(song_input("Gloria"))).expect("song");
    let created = bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: Some("SATB".to_owned()),
        languages: vec!["en".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![
            new_section(Some("A"), "a"),
            new_section(Some("B"), "b"),
            new_section(Some("C"), "c"),
        ],
        sequence: vec![0, 1, 0, 2],
    }))
    .expect("arrangement");
    let head =
        bounded(storage.latest_revision(RevisionEntity::Arrangement(created.id))).expect("head");

    let mut content = ArrangementContent::of(&created);
    content.sections = vec![
        content.sections[2].clone(),
        content.sections[0].clone(),
        SectionContent {
            id: section_id(0x51),
            label: Some("D".to_owned()),
            lyrics: "d".to_owned(),
        },
    ];
    content.sequence = vec![
        OccurrenceContent {
            id: created.sequence[3].id,
            section: created.sections[2].id,
        },
        OccurrenceContent {
            id: occurrence_id(0x61),
            section: section_id(0x51),
        },
        OccurrenceContent {
            id: created.sequence[0].id,
            section: created.sections[0].id,
        },
    ];
    let (record, revision) = bounded(storage.commit_arrangement_edit(
        created.id,
        content.clone(),
        PaginationState::Draft,
        RevisionKind::Edit,
        Some(session(2)),
        head,
    ))
    .expect("edit");

    let section_order: Vec<SectionId> = record.sections.iter().map(|s| s.id).collect();
    assert_eq!(
        section_order,
        vec![
            created.sections[2].id,
            created.sections[0].id,
            section_id(0x51)
        ]
    );
    // Kept ids keep their creation time and their id; the new one is stamped now.
    assert_eq!(
        record.sections[0].created_at,
        created.sections[2].created_at
    );
    assert_eq!(
        record.sections[1].created_at,
        created.sections[0].created_at
    );
    assert!(record.sections[2].created_at >= created.sections[0].created_at);
    let sequence: Vec<(OccurrenceId, SectionId)> = record
        .sequence
        .iter()
        .map(|occurrence| (occurrence.id, occurrence.section))
        .collect();
    assert_eq!(
        sequence,
        vec![
            (created.sequence[3].id, created.sections[2].id),
            (occurrence_id(0x61), section_id(0x51)),
            (created.sequence[0].id, created.sections[0].id),
        ]
    );
    assert_eq!(
        record.sequence[0].created_at,
        created.sequence[3].created_at
    );
    assert_eq!(ArrangementContent::of(&record), content);
    assert_eq!(bounded(storage.arrangement(created.id)), Ok(record));
    assert_eq!(revision.parent, Some(head));
    assert_eq!(
        bounded(storage.revision_content(revision.id)),
        Ok(RevisionContent::Arrangement(content))
    );
    shutdown(storage);
}

#[test]
fn restoration_appends_a_later_revision() {
    let dir = TempDir::new("restore");
    let fixture = song_fixture(&dir);
    let original = bounded(fixture.storage.song(fixture.song)).expect("song");
    let edited = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation).expect("edit");

    let (content, restoration) = bounded(fixture.storage.restore_revision(
        fixture.creation,
        edited.1.id,
        PaginationState::Draft,
    ))
    .expect("restore");
    assert_eq!(content, RevisionContent::Song(SongContent::of(&original)));
    assert_eq!(restoration.kind, RevisionKind::Restoration);
    assert_eq!(restoration.session, None);
    assert_eq!(restoration.restored_from, Some(fixture.creation));
    assert_eq!(restoration.parent, Some(edited.1.id));

    let current = bounded(fixture.storage.song(fixture.song)).expect("song");
    assert_eq!(SongContent::of(&current), SongContent::of(&original));
    let chain = bounded(
        fixture
            .storage
            .revisions(RevisionEntity::Song(fixture.song)),
    )
    .expect("chain");
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[2], restoration);
    assert_eq!(chain[0].id, fixture.creation);
    assert_eq!(chain[1], edited.1);
    shutdown(fixture.storage);
}

#[test]
fn failed_commit_changes_nothing() {
    let dir = TempDir::new("atomic");
    let storage = open(&dir);
    let song = bounded(storage.create_song(song_input("Sanctus"))).expect("song");
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    }))
    .expect("choir");
    let created = bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: Some("SATB".to_owned()),
        languages: vec!["en".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: None,
        occasions: vec!["Lent".to_owned()],
        notes: None,
        sections: vec![new_section(Some("A"), "a"), new_section(None, "b")],
        sequence: vec![0, 1, 0],
    }))
    .expect("arrangement");
    let entity = RevisionEntity::Arrangement(created.id);
    let mut head = bounded(storage.revisions(entity)).expect("chain")[0].id;

    // Write steps of one arrangement edit: 1 update + 5 deletes + 1 language +
    // 1 choir + 1 occasion + 2 sections + 3 occurrences + 1 revision + 1 pagination.
    let steps = 1 + 5 + 1 + 1 + 1 + 2 + 3 + 1 + 1;
    let edited = |title: &str| {
        let mut content = ArrangementContent::of(&created);
        content.title = Some(title.to_owned());
        content.sections.swap(0, 1);
        content
    };

    // A failure scripted one past the last statement never fires: the edit
    // succeeds, which proves the step count is exact.
    bounded(storage.inject_write_failure(steps + 1)).expect("arm");
    let (_, committed) = bounded(storage.commit_arrangement_edit(
        created.id,
        edited("Committed"),
        PaginationState::Approved,
        RevisionKind::Edit,
        Some(session(3)),
        head,
    ))
    .expect("edit succeeds");
    head = committed.id;

    let content_before: ArrangementRecord = bounded(storage.arrangement(created.id)).expect("read");
    let history_before = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(history_before.len(), 2);
    for step in 1..=steps {
        bounded(storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            bounded(storage.commit_arrangement_edit(
                created.id,
                edited("Failing"),
                PaginationState::NeedsReview,
                RevisionKind::Edit,
                Some(session(3)),
                head,
            )),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "step {step}"
        );
        assert_eq!(
            bounded(storage.arrangement(created.id)),
            Ok(content_before.clone()),
            "content after failure at step {step}"
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
fn invalid_commits_are_refused() {
    let dir = TempDir::new("invalid");
    let fixture = song_fixture(&dir);
    let storage = &fixture.storage;
    let head = fixture.creation;
    let commit = |content: SongContent, kind, session_id| {
        bounded(storage.commit_song_edit(fixture.song, content, kind, session_id, head))
    };
    let err = |result: Result<_, StorageError>| {
        result.map(|_: (storage::SongRecord, storage::RevisionRecord)| ())
    };

    assert_eq!(
        err(commit(song_content("x"), RevisionKind::Creation, None)),
        Err(invalid("kind", "not allowed"))
    );
    assert_eq!(
        err(commit(song_content("x"), RevisionKind::Restoration, None)),
        Err(invalid("kind", "not allowed"))
    );
    // Replacement is whole-Arrangement only (Decision 140).
    assert_eq!(
        err(commit(song_content("x"), RevisionKind::Replacement, None)),
        Err(invalid("kind", "not allowed"))
    );
    assert_eq!(
        err(commit(song_content("x"), RevisionKind::Edit, None)),
        Err(invalid("session", "required"))
    );
    assert_eq!(
        err(commit(
            song_content("x"),
            RevisionKind::Import,
            Some(session(1))
        )),
        Err(invalid("session", "not allowed"))
    );
    assert_eq!(
        err(commit(song_content("  "), RevisionKind::Import, None)),
        Err(invalid("title", "blank"))
    );
    let mut duplicate = song_content("x");
    duplicate.occasions = vec!["Lent".to_owned(), "Lent".to_owned()];
    assert_eq!(
        err(commit(duplicate, RevisionKind::Import, None)),
        Err(invalid("occasion", "duplicate"))
    );
    let unknown = SongId::from_entity(entity_id(0x77));
    assert_eq!(
        bounded(storage.commit_song_edit(
            unknown,
            song_content("x"),
            RevisionKind::Import,
            None,
            head
        ))
        .map(|_| ()),
        Err(StorageError::NotFound {
            kind: RecordKind::Song,
            id: unknown.entity()
        })
    );
    // Restoring a revision id that does not exist is NotFound.
    assert_eq!(
        bounded(storage.restore_revision(
            RevisionId::from_entity(entity_id(0x78)),
            head,
            PaginationState::Draft
        ))
        .map(|_| ()),
        Err(StorageError::NotFound {
            kind: RecordKind::Revision,
            id: entity_id(0x78)
        })
    );

    // Arrangement rules.
    let song = fixture.song;
    let other = bounded(storage.create_arrangement(NewArrangement {
        song,
        title: Some("Other".to_owned()),
        languages: vec!["en".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![new_section(None, "o")],
        sequence: vec![0],
    }))
    .expect("other");
    let created = bounded(storage.create_arrangement(NewArrangement {
        song,
        title: Some("Mine".to_owned()),
        languages: vec!["en".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![new_section(None, "m")],
        sequence: vec![0],
    }))
    .expect("mine");
    let entity = RevisionEntity::Arrangement(created.id);
    let arrangement_head = bounded(storage.revisions(entity)).expect("chain")[0].id;
    let before = bounded(storage.arrangement(created.id)).expect("read");
    let history = bounded(storage.revisions(entity)).expect("chain");
    let try_arrangement = |content: ArrangementContent| {
        bounded(storage.commit_arrangement_edit(
            created.id,
            content,
            PaginationState::Draft,
            RevisionKind::Import,
            None,
            arrangement_head,
        ))
        .map(|_| ())
    };

    let mut duplicate_section = ArrangementContent::of(&created);
    duplicate_section
        .sections
        .push(duplicate_section.sections[0].clone());
    assert_eq!(
        try_arrangement(duplicate_section),
        Err(invalid("section", "duplicate"))
    );
    let mut duplicate_occurrence = ArrangementContent::of(&created);
    duplicate_occurrence
        .sequence
        .push(duplicate_occurrence.sequence[0].clone());
    assert_eq!(
        try_arrangement(duplicate_occurrence),
        Err(invalid("occurrence", "duplicate"))
    );
    let mut unknown_section = ArrangementContent::of(&created);
    unknown_section.sequence[0].section = section_id(0x79);
    assert_eq!(
        try_arrangement(unknown_section),
        Err(invalid("sequence", "unknown section"))
    );
    let mut missing_choir = ArrangementContent::of(&created);
    let ghost = ChoirId::from_entity(entity_id(0x7a));
    missing_choir.choirs = vec![ghost];
    assert_eq!(
        try_arrangement(missing_choir),
        Err(StorageError::MissingReference {
            kind: RecordKind::Choir,
            id: ghost.entity()
        })
    );
    let mut foreign_section = ArrangementContent::of(&created);
    foreign_section.sections[0].id = other.sections[0].id;
    foreign_section.sequence[0].section = other.sections[0].id;
    assert_eq!(
        try_arrangement(foreign_section),
        Err(invalid("section", "foreign id"))
    );
    let mut foreign_occurrence = ArrangementContent::of(&created);
    foreign_occurrence.sequence[0].id = other.sequence[0].id;
    assert_eq!(
        try_arrangement(foreign_occurrence),
        Err(invalid("occurrence", "foreign id"))
    );

    assert_eq!(bounded(storage.arrangement(created.id)), Ok(before));
    assert_eq!(bounded(storage.revisions(entity)), Ok(history));
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Song(fixture.song)))
            .expect("chain")
            .len(),
        1
    );

    // Validation failures leave a scripted fault armed for the next commit.
    bounded(storage.inject_write_failure(1)).expect("arm");
    assert_eq!(
        err(commit(song_content(" "), RevisionKind::Import, None)),
        Err(invalid("title", "blank"))
    );
    assert_eq!(
        err(commit(song_content("fine"), RevisionKind::Import, None)),
        Err(StorageError::Sqlite("injected write failure".to_owned()))
    );
    shutdown(fixture.storage);
}

#[test]
fn history_survives_restart() {
    let dir = TempDir::new("restart");
    let fixture = song_fixture(&dir);
    let first = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation).expect("edit");
    let (_, restoration) = bounded(fixture.storage.restore_revision(
        fixture.creation,
        first.1.id,
        PaginationState::Draft,
    ))
    .expect("restore");
    let song = fixture.song;
    let before = bounded(fixture.storage.revisions(RevisionEntity::Song(song))).expect("chain");
    let record = bounded(fixture.storage.song(song)).expect("song");
    shutdown(fixture.storage);

    let storage = open(&dir);
    let after = bounded(storage.revisions(RevisionEntity::Song(song))).expect("chain");
    assert_eq!(after, before);
    assert_eq!(after.len(), 3);
    assert_eq!(after[2], restoration);
    assert_eq!(bounded(storage.song(song)), Ok(record));
    assert_eq!(
        bounded(storage.revision_content(first.1.id)),
        Ok(RevisionContent::Song(song_content("Kyrie 2")))
    );
    shutdown(storage);
}

#[test]
fn t2_1_rows_get_a_creation_revision_on_first_edit() {
    let dir = TempDir::new("backfill");
    shutdown(open(&dir));
    let song = SongId::from_entity(entity_id(0x41));
    raw(&dir)
        .execute(
            "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
            params![song.to_string(), "Legacy", "2023-11-14T22:13:20.000Z"],
        )
        .expect("insert T2.1 row");

    let storage = open(&dir);
    let entity = RevisionEntity::Song(song);
    assert_eq!(bounded(storage.revisions(entity)), Ok(Vec::new()));
    let creation = bounded(storage.latest_revision(entity)).expect("backfill");
    assert_eq!(bounded(storage.latest_revision(entity)), Ok(creation));
    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].id, creation);
    assert_eq!(chain[0].kind, RevisionKind::Creation);
    let legacy = bounded(storage.song(song)).expect("song");
    assert_eq!(
        bounded(storage.revision_content(creation)),
        Ok(RevisionContent::Song(SongContent::of(&legacy)))
    );

    let (record, revision) = bounded(storage.commit_song_edit(
        song,
        song_content("Edited"),
        RevisionKind::Edit,
        Some(session(9)),
        creation,
    ))
    .expect("first edit");
    assert_eq!(record.title, "Edited");
    assert_eq!(revision.parent, Some(creation));
    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].kind, RevisionKind::Creation);
    assert_eq!(chain[1], revision);

    // A commit on an entity with no history is refused, not silently backfilled.
    let bare = SongId::from_entity(entity_id(0x42));
    shutdown(storage);
    raw(&dir)
        .execute(
            "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
            params![bare.to_string(), "Bare", "2023-11-14T22:13:20.000Z"],
        )
        .expect("insert second T2.1 row");
    let storage = open(&dir);
    assert_eq!(
        bounded(storage.commit_song_edit(
            bare,
            song_content("x"),
            RevisionKind::Import,
            None,
            creation
        ))
        .map(|_| ()),
        Err(invalid("expected_parent", "entity has no history"))
    );
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Song(bare))),
        Ok(Vec::new())
    );
    shutdown(storage);
}

#[test]
fn damaged_revision_content_is_reported() {
    let dir = TempDir::new("damaged");
    let fixture = song_fixture(&dir);
    let edited = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation).expect("edit");
    let other = edit_song(&fixture, "Kyrie 3", session(1), edited.1.id).expect("edit");
    let song = fixture.song;
    shutdown(fixture.storage);

    let conn = raw(&dir);
    // Bytes changed without the hash: a mismatch.
    conn.execute(
        "UPDATE edit_revision SET content = content || ' ' WHERE id = ?1",
        params![edited.1.id.to_string()],
    )
    .expect("tamper");
    // Hash kept consistent but the document has an unknown key.
    let unknown_key = "{\"format\":1,\"entity\":\"song\",\"title\":\"x\",\"aliases\":[],\
                       \"original_attribution\":null,\"provenance\":null,\"rights_notes\":null,\
                       \"liturgical_moments\":[],\"occasions\":[],\"extra\":1}";
    conn.execute(
        "UPDATE edit_revision SET content = ?2, content_sha256 = ?3 WHERE id = ?1",
        params![other.1.id.to_string(), unknown_key, hex_sha256(unknown_key)],
    )
    .expect("tamper");
    // An unknown format.
    let future_format = "{\"format\":2,\"entity\":\"song\"}";
    conn.execute(
        "UPDATE edit_revision SET content = ?2, content_sha256 = ?3 WHERE id = ?1",
        params![
            fixture.creation.to_string(),
            future_format,
            hex_sha256(future_format)
        ],
    )
    .expect("tamper");
    drop(conn);

    let storage = open(&dir);
    for id in [edited.1.id, other.1.id, fixture.creation] {
        assert_eq!(
            bounded(storage.revision_content(id)),
            Err(StorageError::RevisionIntegrity { id })
        );
    }
    // Restoring damaged content is refused and changes nothing.
    let before = bounded(storage.song(song)).expect("song");
    assert_eq!(
        bounded(storage.restore_revision(edited.1.id, other.1.id, PaginationState::Draft))
            .map(|_| ()),
        Err(StorageError::RevisionIntegrity { id: edited.1.id })
    );
    assert_eq!(bounded(storage.song(song)), Ok(before));
    assert_eq!(
        bounded(storage.revisions(RevisionEntity::Song(song)))
            .expect("chain")
            .len(),
        3
    );
    shutdown(storage);
}

#[test]
fn stale_parent_is_refused() {
    let dir = TempDir::new("stale");
    let fixture = song_fixture(&dir);
    let entity = RevisionEntity::Song(fixture.song);
    let first = edit_song(&fixture, "First", session(1), fixture.creation).expect("first");

    let history = bounded(fixture.storage.revisions(entity)).expect("chain");
    let content = bounded(fixture.storage.song(fixture.song)).expect("song");
    assert_eq!(
        edit_song(&fixture, "Second", session(2), fixture.creation).map(|_| ()),
        Err(StorageError::StaleRevision {
            entity_latest: first.1.id
        })
    );
    assert_eq!(bounded(fixture.storage.song(fixture.song)), Ok(content));
    assert_eq!(bounded(fixture.storage.revisions(entity)), Ok(history));

    // A restore based on an old head is stale too.
    assert_eq!(
        bounded(fixture.storage.restore_revision(
            fixture.creation,
            fixture.creation,
            PaginationState::Draft
        ))
        .map(|_| ()),
        Err(StorageError::StaleRevision {
            entity_latest: first.1.id
        })
    );

    // A parent that belongs to another entity is stale, not accepted.
    let song = fixture.song;
    let arrangement = bounded(fixture.storage.create_arrangement(NewArrangement {
        song,
        title: Some("SATB".to_owned()),
        languages: vec!["en".to_owned()],
        choirs: Vec::new(),
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: Vec::new(),
        sequence: Vec::new(),
    }))
    .expect("arrangement");
    let arrangement_head = bounded(
        fixture
            .storage
            .revisions(RevisionEntity::Arrangement(arrangement.id)),
    )
    .expect("chain")[0]
        .id;
    assert_eq!(
        bounded(fixture.storage.commit_arrangement_edit(
            arrangement.id,
            ArrangementContent::of(&arrangement),
            PaginationState::Draft,
            RevisionKind::Import,
            None,
            first.1.id,
        ))
        .map(|_| ()),
        Err(StorageError::StaleRevision {
            entity_latest: arrangement_head
        })
    );
    shutdown(fixture.storage);
}

#[test]
fn non_canonical_content_is_reported() {
    let dir = TempDir::new("canonical");
    let fixture = song_fixture(&dir);
    let edited = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation).expect("edit");
    let creation = fixture.creation;
    shutdown(fixture.storage);

    let conn = raw(&dir);
    // A document without the optional "provenance" key decodes with serde but
    // is not what this build writes; the hash is kept consistent.
    let missing = "{\"format\":1,\"entity\":\"song\",\"title\":\"x\",\"aliases\":[],\
                   \"original_attribution\":null,\"rights_notes\":null,\
                   \"liturgical_moments\":[],\"occasions\":[]}";
    // Complete but with whitespace between tokens.
    let spaced = "{\"format\":1, \"entity\":\"song\",\"title\":\"x\",\"aliases\":[],\
                  \"original_attribution\":null,\"provenance\":null,\"rights_notes\":null,\
                  \"liturgical_moments\":[],\"occasions\":[]}";
    for (id, text) in [(edited.1.id, missing), (creation, spaced)] {
        conn.execute(
            "UPDATE edit_revision SET content = ?2, content_sha256 = ?3 WHERE id = ?1",
            params![id.to_string(), text, hex_sha256(text)],
        )
        .expect("tamper");
    }
    drop(conn);

    let storage = open(&dir);
    for id in [edited.1.id, creation] {
        assert_eq!(
            bounded(storage.revision_content(id)),
            Err(StorageError::RevisionIntegrity { id })
        );
    }
    shutdown(storage);
}

#[test]
fn damaged_metadata_and_broken_chains_are_reported() {
    let dir = TempDir::new("metadata");
    let fixture = song_fixture(&dir);
    let edited = edit_song(&fixture, "Kyrie 2", session(1), fixture.creation).expect("edit");
    let entity = RevisionEntity::Song(fixture.song);
    let song = fixture.song;
    shutdown(fixture.storage);

    // A column the schema checks, damaged by bypassing the checks: the row's
    // id is still readable, so the error names it.
    let conn = raw(&dir);
    conn.execute_batch("PRAGMA ignore_check_constraints = ON; PRAGMA foreign_keys = OFF")
        .expect("pragma");
    conn.execute(
        "UPDATE edit_revision SET created_at = 'garbage' WHERE id = ?1",
        params![edited.1.id.to_string()],
    )
    .expect("tamper");
    drop(conn);
    let storage = open(&dir);
    assert_eq!(
        bounded(storage.revisions(entity)),
        Err(StorageError::RevisionIntegrity { id: edited.1.id })
    );
    shutdown(storage);

    // Restore the row, then add two revisions that name each other as parent:
    // a cycle detached from the real root. Nothing may loop or be skipped.
    let conn = raw(&dir);
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .expect("pragma");
    conn.execute(
        "UPDATE edit_revision SET created_at = '2023-11-14T22:13:20.000Z' WHERE id = ?1",
        params![edited.1.id.to_string()],
    )
    .expect("repair");
    let first = RevisionId::from_entity(entity_id(0x51));
    let second = RevisionId::from_entity(entity_id(0x52));
    for (id, parent) in [(first, second), (second, first)] {
        conn.execute(
            "INSERT INTO edit_revision (id, song_id, arrangement_id, parent_id, kind, session_id, \
             restored_from_id, content, content_sha256, created_at) \
             SELECT ?1, song_id, arrangement_id, ?2, 'import', NULL, NULL, content, \
             content_sha256, created_at FROM edit_revision WHERE id = ?3",
            params![id.to_string(), parent.to_string(), edited.1.id.to_string()],
        )
        .expect("insert cyclic revision");
    }
    drop(conn);
    let storage = open(&dir);
    assert!(matches!(
        bounded(storage.revisions(RevisionEntity::Song(song))),
        Err(StorageError::RevisionIntegrity { .. })
    ));
    shutdown(storage);
}

#[test]
fn song_commit_write_steps_are_exact() {
    let dir = TempDir::new("song-steps");
    let fixture = song_fixture(&dir);
    let storage = &fixture.storage;
    let entity = RevisionEntity::Song(fixture.song);

    // Write steps of one Song edit: 1 update + 3 deletes (aliases, moments,
    // occasions) + 1 alias + 1 moment + 1 occasion + 1 revision.
    let steps = 1 + 3 + 1 + 1 + 1 + 1;

    // A failure scripted one past the last statement never fires: the edit
    // succeeds, which proves the step count is exact.
    bounded(storage.inject_write_failure(steps + 1)).expect("arm");
    let (_, committed) =
        edit_song(&fixture, "Committed", session(2), fixture.creation).expect("edit succeeds");

    let song_before = bounded(storage.song(fixture.song)).expect("read");
    let history_before = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(history_before.len(), 2);
    for step in 1..=steps {
        bounded(storage.inject_write_failure(step)).expect("arm");
        assert_eq!(
            edit_song(&fixture, "Failing", session(2), committed.id).map(|_| ()),
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "step {step}"
        );
        assert_eq!(
            bounded(storage.song(fixture.song)),
            Ok(song_before.clone()),
            "song after failure at step {step}"
        );
        assert_eq!(
            bounded(storage.revisions(entity)),
            Ok(history_before.clone()),
            "history after failure at step {step}"
        );
    }
    shutdown(fixture.storage);
}

#[test]
fn v3_database_with_t2_1_data_upgrades_to_v4() {
    let dir = TempDir::new("upgrade");
    let stamp = "2023-11-14T22:13:20.000Z";
    let bundled_v3 = [
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
    ];
    let song = SongId::from_entity(entity_id(0x61));
    let arrangement = ArrangementId::from_entity(entity_id(0x62));
    let (first, second) = (section_id(0x63), section_id(0x64));
    let occurrences = [
        (occurrence_id(0x65), first),
        (occurrence_id(0x66), second),
        (occurrence_id(0x67), first),
    ];
    {
        // A version-3 database exactly as the migration runner leaves it.
        let conn = raw(&dir);
        conn.execute_batch(
            "CREATE TABLE ordo_migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL, \
             checksum TEXT NOT NULL, applied_at TEXT NOT NULL)",
        )
        .expect("ledger");
        for (index, (name, sql)) in bundled_v3.iter().enumerate() {
            conn.execute_batch(sql).expect("apply migration");
            conn.execute(
                "INSERT INTO ordo_migrations (id, name, checksum, applied_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![index as i64 + 1, name, hex_sha256(sql), stamp],
            )
            .expect("ledger row");
        }
        conn.pragma_update(None, "user_version", 3)
            .expect("version");

        conn.execute(
            "INSERT INTO song (id, title, created_at) VALUES (?1, 'Legacy', ?2)",
            params![song.to_string(), stamp],
        )
        .expect("song");
        conn.execute(
            "INSERT INTO song_alias (song_id, position, alias) VALUES (?1, 0, 'Old')",
            params![song.to_string()],
        )
        .expect("alias");
        conn.execute(
            "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, 'SATB', ?3)",
            params![arrangement.to_string(), song.to_string(), stamp],
        )
        .expect("arrangement");
        for (position, (id, label, lyrics)) in [(first, "A", "a"), (second, "B", "b")]
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
                    stamp
                ],
            )
            .expect("section");
        }
        for (position, (id, section)) in occurrences.iter().enumerate() {
            conn.execute(
                "INSERT INTO section_occurrence \
                 (id, arrangement_id, section_id, position, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id.to_string(),
                    arrangement.to_string(),
                    section.to_string(),
                    position as i64,
                    stamp
                ],
            )
            .expect("occurrence");
        }
    }

    // Opening migrates 3 -> head (6) and the T2.1 records read back unchanged.
    let storage = open(&dir);
    let status = bounded(storage.schema_status()).expect("status");
    assert_eq!(status.version, 6);
    let read_song = bounded(storage.song(song)).expect("song");
    assert_eq!(read_song.title, "Legacy");
    assert_eq!(read_song.aliases, vec!["Old".to_owned()]);
    let read = bounded(storage.arrangement(arrangement)).expect("arrangement");
    assert_eq!(read.title.as_deref(), Some("SATB"));
    assert_eq!(
        read.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(
        read.sequence
            .iter()
            .map(|o| (o.id, o.section))
            .collect::<Vec<_>>(),
        occurrences.to_vec()
    );

    // Neither entity has history yet; latest_revision backfills a creation
    // revision whose content is the current record.
    for entity in [
        RevisionEntity::Song(song),
        RevisionEntity::Arrangement(arrangement),
    ] {
        assert_eq!(bounded(storage.revisions(entity)), Ok(Vec::new()));
        let creation = bounded(storage.latest_revision(entity)).expect("backfill");
        let chain = bounded(storage.revisions(entity)).expect("chain");
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id, creation);
        assert_eq!(chain[0].kind, RevisionKind::Creation);
    }
    let creation =
        bounded(storage.latest_revision(RevisionEntity::Arrangement(arrangement))).expect("head");
    assert_eq!(
        bounded(storage.revision_content(creation)),
        Ok(RevisionContent::Arrangement(ArrangementContent::of(&read)))
    );
    shutdown(storage);
}
