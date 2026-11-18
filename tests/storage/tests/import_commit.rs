//! T2.10 atomic import commit acceptance tests (WP5). Every await runs under a
//! 10 s bound. Databases live in unique temporary directories; nothing reads
//! or changes real Parish data.

use std::collections::BTreeMap;
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
use rusqlite::types::Value;
use rusqlite::{Connection, params};
use storage::{
    ArrangementContent, ArrangementReplacement, ImportBatch, ImportedArrangement, NewArrangement,
    NewChoir, NewSection, NewSong, OccurrenceContent, RecordKind, RevisionContent, SectionContent,
    ShutdownOutcome, Storage, StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);
const STAMP: &str = "2023-11-14T22:13:20.000Z";

fn bounded<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(block_on(future));
    });
    rx.recv_timeout(BOUND)
        .expect("storage operation did not complete within 10 s")
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ordo-import-commit-{tag}-{}-{}-{nanos}",
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

/// A distinct identity per (group, index).
fn eid(group: u8, index: u8) -> EntityId {
    let mut bytes = [0xA5u8; 16];
    bytes[0] = group;
    bytes[1] = index;
    EntityId::from_random_bytes(bytes)
}

fn session(byte: u8) -> HistorySessionId {
    HistorySessionId::from_entity(eid(0xEE, byte))
}

fn new_choir(name: &str) -> NewChoir {
    NewChoir {
        name: name.to_owned(),
        short_label: None,
        languages: vec!["Cebuano".to_owned()],
        notes: None,
    }
}

fn new_song(title: &str) -> NewSong {
    NewSong {
        title: title.to_owned(),
        aliases: vec![format!("{title} (alt)")],
        original_attribution: Some("Anon".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Communion".to_owned()],
        occasions: vec!["Easter".to_owned()],
    }
}

/// Arrangement content whose section and occurrence identities come from
/// `group`; the Sequence plays every section once and repeats the first.
fn content(group: u8, lyrics: &[&str], choirs: Vec<ChoirId>) -> ArrangementContent {
    let sections: Vec<SectionContent> = lyrics
        .iter()
        .enumerate()
        .map(|(index, lyrics)| SectionContent {
            id: SectionId::from_entity(eid(group, index as u8)),
            label: Some(format!("Part {}", index + 1)),
            lyrics: (*lyrics).to_owned(),
            pagination: PaginationState::NeedsReview,
        })
        .collect();
    let mut sequence: Vec<OccurrenceContent> = sections
        .iter()
        .enumerate()
        .map(|(index, section)| OccurrenceContent {
            id: OccurrenceId::from_entity(eid(group, 0x40 + index as u8)),
            section: section.id,
        })
        .collect();
    sequence.push(OccurrenceContent {
        id: OccurrenceId::from_entity(eid(group, 0x7F)),
        section: sections[0].id,
    });
    ArrangementContent {
        title: Some(format!("Arrangement {group}")),
        languages: vec!["Cebuano".to_owned()],
        choirs,
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: Some("from a document".to_owned()),
        sections,
        sequence,
    }
}

fn batch(first_kind: RevisionKind) -> ImportBatch {
    ImportBatch {
        choirs: Vec::new(),
        songs: Vec::new(),
        arrangements: Vec::new(),
        replacements: Vec::new(),
        first_kind,
    }
}

/// A seeded library: one Choir, one Song and one Arrangement with history.
struct Seed {
    choir: ChoirId,
    song: SongId,
    arrangement: ArrangementId,
    head: RevisionId,
}

fn seed(storage: &Storage) -> Seed {
    let choir = bounded(storage.create_choir(new_choir("Koro"))).expect("choir");
    let song = bounded(storage.create_song(new_song("Tinapay sa Langit"))).expect("song");
    let arrangement = bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: Some("Original".to_owned()),
        languages: vec!["Cebuano".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![
            NewSection {
                label: Some("Verse".to_owned()),
                lyrics: "Old verse".to_owned(),
            },
            NewSection {
                label: Some("Chorus".to_owned()),
                lyrics: "Old chorus".to_owned(),
            },
        ],
        sequence: vec![0, 1],
    }))
    .expect("arrangement");
    let head = bounded(storage.latest_revision(RevisionEntity::Arrangement(arrangement.id)))
        .expect("head");
    Seed {
        choir: choir.id,
        song: song.id,
        arrangement: arrangement.id,
        head,
    }
}

type Snapshot = BTreeMap<String, Vec<Vec<Value>>>;

/// Every row of every user table, ordered by all columns.
fn snapshot(conn: &Connection) -> Snapshot {
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("table list")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("table query")
        .map(|row| row.expect("table name"))
        .collect();
    let mut all = BTreeMap::new();
    for table in tables {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .expect("table info")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column query")
            .map(|row| row.expect("column"))
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
        all.insert(table, rows);
    }
    all
}

fn revision_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM edit_revision", [], |row| row.get(0))
        .expect("revision count")
}

fn arrangement_content(storage: &Storage, revision: RevisionId) -> ArrangementContent {
    match bounded(storage.revision_content(revision)).expect("revision content") {
        RevisionContent::Arrangement(content) => content,
        RevisionContent::Song(_) => panic!("an Arrangement revision holds Arrangement content"),
    }
}

/// A five-mutation batch over `seed`: a Choir, a Song, an Arrangement under the
/// new Song using the new Choir, an Arrangement under the seeded Song, and a
/// replacement of the seeded Arrangement.
fn five_mutations(seed: &Seed) -> ImportBatch {
    let choir = ChoirId::from_entity(eid(1, 0));
    let song = SongId::from_entity(eid(2, 0));
    ImportBatch {
        choirs: vec![(choir, new_choir("Imported Koro"))],
        songs: vec![(song, new_song("Imported Song"))],
        arrangements: vec![
            ImportedArrangement {
                id: ArrangementId::from_entity(eid(3, 0)),
                song,
                content: content(0x10, &["One", "Two"], vec![choir]),
            },
            ImportedArrangement {
                id: ArrangementId::from_entity(eid(3, 1)),
                song: seed.song,
                content: content(0x11, &["Three"], vec![seed.choir]),
            },
        ],
        replacements: vec![ArrangementReplacement {
            id: seed.arrangement,
            content: content(0x12, &["Replaced one", "Replaced two"], vec![seed.choir]),
            expected_parent: Some(seed.head),
        }],
        first_kind: RevisionKind::Import,
    }
}

#[test]
fn ac7_a_replacement_is_one_restorable_replacement_revision() {
    let dir = TempDir::new("restorable");
    let storage = open(&dir);
    let seed = seed(&storage);
    let entity = RevisionEntity::Arrangement(seed.arrangement);
    let before = arrangement_content(&storage, seed.head);

    let mut input = batch(RevisionKind::Import);
    input.replacements.push(ArrangementReplacement {
        id: seed.arrangement,
        content: content(0x20, &["New verse", "New chorus"], vec![seed.choir]),
        expected_parent: Some(seed.head),
    });
    let committed = bounded(storage.commit_import(input)).expect("replacement commit");

    let chain = bounded(storage.revisions(entity)).expect("chain");
    assert_eq!(chain.len(), 2);
    assert_eq!(committed.revisions.len(), 1);
    let replacement = &chain[1];
    assert_eq!(&committed.revisions[0], replacement);
    assert_eq!(replacement.kind, RevisionKind::Replacement);
    assert_eq!(replacement.parent, Some(seed.head));
    assert_eq!(replacement.session, None);
    assert_eq!(committed.arrangements[0].sections[0].lyrics, "New verse");

    let (restored, record) =
        bounded(storage.restore_revision(seed.head, replacement.id)).expect("restore old head");
    assert_eq!(record.kind, RevisionKind::Restoration);
    assert_eq!(restored, RevisionContent::Arrangement(before.clone()));
    let library = bounded(storage.library()).expect("library");
    let stored = library
        .arrangements
        .iter()
        .find(|a| a.id == seed.arrangement)
        .expect("arrangement");
    let lyrics: Vec<&str> = stored.sections.iter().map(|s| s.lyrics.as_str()).collect();
    assert_eq!(lyrics, ["Old verse", "Old chorus"]);
    shutdown(storage);
}

#[test]
fn ac7_a_replacement_over_a_moved_head_is_stale_and_changes_nothing() {
    let dir = TempDir::new("stale");
    let storage = open(&dir);
    let seed = seed(&storage);
    let entity = RevisionEntity::Arrangement(seed.arrangement);
    let mut edited = arrangement_content(&storage, seed.head);
    edited.notes = Some("changed meanwhile".to_owned());
    let (_, moved) = bounded(storage.commit_arrangement_edit(
        seed.arrangement,
        edited,
        RevisionKind::Edit,
        Some(session(1)),
        seed.head,
    ))
    .expect("a background edit");
    let before = snapshot(&raw(&dir));

    let mut input = five_mutations(&seed);
    input.replacements[0].expected_parent = Some(seed.head);
    let error = bounded(storage.commit_import(input)).expect_err("stale base");
    assert_eq!(
        error,
        StorageError::StaleRevision {
            entity_latest: moved.id
        }
    );
    assert_eq!(snapshot(&raw(&dir)), before);
    assert_eq!(
        bounded(storage.latest_revision(entity)).expect("head"),
        moved.id
    );
    shutdown(storage);
}

#[test]
fn ac7_a_replacement_of_a_t2_1_era_row_backfills_inside_the_commit() {
    let dir = TempDir::new("backfill");
    shutdown(open(&dir));
    let song = SongId::from_entity(eid(0x81, 0));
    let arrangement = ArrangementId::from_entity(eid(0x82, 0));
    let section = SectionId::from_entity(eid(0x83, 0));
    let occurrence = OccurrenceId::from_entity(eid(0x84, 0));
    {
        let conn = raw(&dir);
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
             (id, arrangement_id, section_id, position, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
            params![
                occurrence.to_string(),
                arrangement.to_string(),
                section.to_string(),
                STAMP
            ],
        )
        .expect("occurrence");
    }

    let storage = open(&dir);
    let mut input = batch(RevisionKind::Import);
    input.replacements.push(ArrangementReplacement {
        id: arrangement,
        content: content(0x30, &["New words"], Vec::new()),
        expected_parent: None,
    });
    let committed = bounded(storage.commit_import(input)).expect("commit over a legacy row");

    let chain =
        bounded(storage.revisions(RevisionEntity::Arrangement(arrangement))).expect("chain");
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].kind, RevisionKind::Creation);
    assert_eq!(chain[1].kind, RevisionKind::Replacement);
    assert_eq!(chain[1].parent, Some(chain[0].id));
    assert_eq!(committed.revisions, chain);
    let backfilled = arrangement_content(&storage, chain[0].id);
    assert_eq!(backfilled.sections[0].lyrics, "Old words");
    assert_eq!(committed.arrangements[0].sections[0].lyrics, "New words");
    shutdown(storage);
}

#[test]
fn ac9_a_failed_commit_at_any_step_changes_nothing() {
    let dir = TempDir::new("fault");
    let storage = open(&dir);
    let seed = seed(&storage);
    let before = snapshot(&raw(&dir));
    let library_before = bounded(storage.library()).expect("library");
    let entities = [
        RevisionEntity::Arrangement(seed.arrangement),
        RevisionEntity::Song(seed.song),
    ];
    let chains_before: Vec<_> = entities
        .iter()
        .map(|entity| bounded(storage.revisions(*entity)).expect("chain"))
        .collect();
    let revisions_before = revision_count(&raw(&dir));

    let mut step = 1;
    loop {
        bounded(storage.inject_write_failure(step)).expect("arm");
        match bounded(storage.commit_import(five_mutations(&seed))) {
            Err(error) => {
                assert_eq!(
                    error,
                    StorageError::Sqlite("injected write failure".to_owned()),
                    "step {step}"
                );
                assert_eq!(bounded(storage.library()).expect("library"), library_before);
                for (entity, chain) in entities.iter().zip(&chains_before) {
                    assert_eq!(
                        &bounded(storage.revisions(*entity)).expect("chain"),
                        chain,
                        "step {step}"
                    );
                }
                let conn = raw(&dir);
                assert_eq!(revision_count(&conn), revisions_before, "step {step}");
                assert_eq!(snapshot(&conn), before, "step {step}");
                step += 1;
            }
            Ok(committed) => {
                assert_eq!(committed.arrangements.len(), 3);
                break;
            }
        }
        assert!(step < 200, "the batch never succeeded");
    }
    assert!(
        step > 20,
        "a five-mutation batch has many write steps, got {step}"
    );
    assert_ne!(snapshot(&raw(&dir)), before);
    shutdown(storage);
}

#[test]
fn ac9_an_id_collision_refuses_the_whole_batch() {
    let dir = TempDir::new("collision");
    let storage = open(&dir);
    let seed = seed(&storage);
    let before = snapshot(&raw(&dir));

    // A Song whose id is already stored.
    let mut input = five_mutations(&seed);
    input.songs[0].0 = seed.song;
    assert!(matches!(
        bounded(storage.commit_import(input)),
        Err(StorageError::IdentityCollision {
            kind: RecordKind::Song,
            ..
        })
    ));
    assert_eq!(snapshot(&raw(&dir)), before);

    // An Arrangement whose id is already stored.
    let mut input = five_mutations(&seed);
    input.arrangements[1].id = seed.arrangement;
    assert!(matches!(
        bounded(storage.commit_import(input)),
        Err(StorageError::IdentityCollision {
            kind: RecordKind::Arrangement,
            ..
        })
    ));
    assert_eq!(snapshot(&raw(&dir)), before);

    // A section identity that another Arrangement already owns.
    let stored = bounded(storage.library()).expect("library");
    let owned = stored.arrangements[0].sections[0].id;
    let mut input = five_mutations(&seed);
    input.arrangements[0].content.sections[0].id = owned;
    input.arrangements[0].content.sequence[0].section = owned;
    input.arrangements[0].content.sequence[2].section = owned;
    assert!(matches!(
        bounded(storage.commit_import(input)),
        Err(StorageError::IdentityCollision {
            kind: RecordKind::Section,
            ..
        })
    ));
    assert_eq!(snapshot(&raw(&dir)), before);

    // The same identity twice in one batch.
    let mut input = five_mutations(&seed);
    let twin = input.songs[0].clone();
    input.songs.push(twin);
    assert!(matches!(
        bounded(storage.commit_import(input)),
        Err(StorageError::IdentityCollision {
            kind: RecordKind::Song,
            ..
        })
    ));
    assert_eq!(snapshot(&raw(&dir)), before);

    // The ids are never regenerated: the clean batch commits with them.
    let committed = bounded(storage.commit_import(five_mutations(&seed))).expect("clean commit");
    assert_eq!(committed.songs[0].id, SongId::from_entity(eid(2, 0)));
    shutdown(storage);
}

#[test]
fn ac9_an_archived_target_refuses_the_whole_batch() {
    let dir = TempDir::new("archived");
    let storage = open(&dir);
    let seed = seed(&storage);

    // Archiving the Song refuses a new Arrangement under it.
    bounded(storage.archive_arrangement(seed.arrangement)).expect("archive arrangement");
    bounded(storage.archive_song(seed.song)).expect("archive song");
    let before = snapshot(&raw(&dir));

    let mut input = batch(RevisionKind::Import);
    input
        .choirs
        .push((ChoirId::from_entity(eid(1, 0)), new_choir("Fresh")));
    input.arrangements.push(ImportedArrangement {
        id: ArrangementId::from_entity(eid(3, 0)),
        song: seed.song,
        content: content(0x10, &["One"], Vec::new()),
    });
    assert_eq!(
        bounded(storage.commit_import(input)),
        Err(StorageError::InvalidInput {
            field: "song",
            reason: "archived"
        })
    );
    assert_eq!(snapshot(&raw(&dir)), before);

    // An archived Arrangement refuses its replacement.
    let mut input = batch(RevisionKind::Import);
    input
        .choirs
        .push((ChoirId::from_entity(eid(1, 0)), new_choir("Fresh")));
    input.replacements.push(ArrangementReplacement {
        id: seed.arrangement,
        content: content(0x12, &["Again"], Vec::new()),
        expected_parent: Some(seed.head),
    });
    assert_eq!(
        bounded(storage.commit_import(input)),
        Err(StorageError::InvalidInput {
            field: "arrangement",
            reason: "archived"
        })
    );
    assert_eq!(snapshot(&raw(&dir)), before);

    // A missing Choir refuses the whole batch too.
    let mut input = batch(RevisionKind::Import);
    input
        .songs
        .push((SongId::from_entity(eid(2, 0)), new_song("New")));
    input.arrangements.push(ImportedArrangement {
        id: ArrangementId::from_entity(eid(3, 0)),
        song: SongId::from_entity(eid(2, 0)),
        content: content(0x10, &["One"], vec![ChoirId::from_entity(eid(9, 9))]),
    });
    assert!(matches!(
        bounded(storage.commit_import(input)),
        Err(StorageError::MissingReference {
            kind: RecordKind::Choir,
            ..
        })
    ));
    assert_eq!(snapshot(&raw(&dir)), before);
    shutdown(storage);
}

#[test]
fn ac10_new_entities_keep_the_document_ids() {
    let dir = TempDir::new("ids");
    let storage = open(&dir);
    let seed = seed(&storage);
    let input = five_mutations(&seed);
    let wanted = input.clone();
    let committed = bounded(storage.commit_import(input)).expect("commit");

    assert_eq!(committed.choirs[0].id, wanted.choirs[0].0);
    assert_eq!(committed.songs[0].id, wanted.songs[0].0);
    for (stored, imported) in committed.arrangements.iter().zip(&wanted.arrangements) {
        assert_eq!(stored.id, imported.id);
        assert_eq!(stored.song, imported.song);
        let sections: Vec<SectionId> = stored.sections.iter().map(|s| s.id).collect();
        let expected: Vec<SectionId> = imported.content.sections.iter().map(|s| s.id).collect();
        assert_eq!(sections, expected);
        let occurrences: Vec<OccurrenceId> = stored.sequence.iter().map(|o| o.id).collect();
        let expected: Vec<OccurrenceId> = imported.content.sequence.iter().map(|o| o.id).collect();
        assert_eq!(occurrences, expected);
    }
    // The replacement keeps the Arrangement's identity and takes the content's.
    assert_eq!(committed.arrangements[2].id, seed.arrangement);
    assert_eq!(
        committed.arrangements[2].sections[0].id,
        wanted.replacements[0].content.sections[0].id
    );
    shutdown(storage);
}

#[test]
fn new_import_and_markdown_entities_both_start_with_a_creation_revision() {
    let dir = TempDir::new("first-kind");
    let storage = open(&dir);
    let seed = seed(&storage);

    let mut input = five_mutations(&seed);
    input.replacements.clear();
    let committed = bounded(storage.commit_import(input)).expect("import commit");
    // The schema allows a parentless revision only as `creation`, so an
    // Import batch's first revisions are stored as creation revisions.
    for revision in &committed.revisions {
        assert_eq!(revision.kind, RevisionKind::Creation);
        assert_eq!(revision.parent, None);
        assert_eq!(revision.session, None);
    }
    assert_eq!(committed.revisions.len(), 3);

    let song = SongId::from_entity(eid(5, 0));
    let arrangement = ArrangementId::from_entity(eid(5, 1));
    let mut markdown = batch(RevisionKind::Creation);
    markdown.songs.push((song, new_song("From Markdown")));
    markdown.arrangements.push(ImportedArrangement {
        id: arrangement,
        song,
        content: content(0x50, &["Words"], Vec::new()),
    });
    bounded(storage.commit_import(markdown)).expect("markdown commit");
    for entity in [
        RevisionEntity::Song(song),
        RevisionEntity::Arrangement(arrangement),
    ] {
        let chain = bounded(storage.revisions(entity)).expect("chain");
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].kind, RevisionKind::Creation);
        assert_eq!(chain[0].parent, None);
    }

    let mut refused = batch(RevisionKind::Edit);
    refused
        .songs
        .push((SongId::from_entity(eid(6, 0)), new_song("Nope")));
    assert!(matches!(
        bounded(storage.commit_import(refused)),
        Err(StorageError::InvalidInput {
            field: "first_kind",
            ..
        })
    ));
    shutdown(storage);
}

#[test]
fn imported_section_states_are_stored_as_given() {
    let dir = TempDir::new("states");
    let storage = open(&dir);
    let song = SongId::from_entity(eid(5, 0));
    let arrangement = ArrangementId::from_entity(eid(5, 1));
    let mut given = content(0x50, &["One", "Two", "Three"], Vec::new());
    given.sections[0].pagination = PaginationState::Draft;
    given.sections[1].pagination = PaginationState::Approved;
    given.sections[2].pagination = PaginationState::NeedsReview;
    let mut input = batch(RevisionKind::Import);
    input.songs.push((song, new_song("States")));
    input.arrangements.push(ImportedArrangement {
        id: arrangement,
        song,
        content: given.clone(),
    });
    let committed = bounded(storage.commit_import(input)).expect("commit");

    let states: Vec<PaginationState> = committed.arrangements[0]
        .sections
        .iter()
        .map(|section| section.pagination)
        .collect();
    assert_eq!(
        states,
        [
            PaginationState::Draft,
            PaginationState::Approved,
            PaginationState::NeedsReview
        ]
    );
    let chain =
        bounded(storage.revisions(RevisionEntity::Arrangement(arrangement))).expect("chain");
    assert_eq!(arrangement_content(&storage, chain[0].id), given);
    shutdown(storage);
}

#[test]
fn the_commit_reports_exactly_what_it_wrote() {
    let dir = TempDir::new("report");
    let storage = open(&dir);
    let seed = seed(&storage);
    let before = revision_count(&raw(&dir));
    let committed = bounded(storage.commit_import(five_mutations(&seed))).expect("commit");

    let library = bounded(storage.library()).expect("library");
    for choir in &committed.choirs {
        assert!(library.choirs.contains(choir));
    }
    for song in &committed.songs {
        assert!(library.songs.contains(song));
    }
    for arrangement in &committed.arrangements {
        assert!(library.arrangements.contains(arrangement));
    }
    assert_eq!(
        revision_count(&raw(&dir)) - before,
        committed.revisions.len() as i64
    );
    assert_eq!(committed.revisions.len(), 4);
    for revision in &committed.revisions {
        let chain = bounded(storage.revisions(revision.entity)).expect("chain");
        assert!(chain.contains(revision));
    }
    assert_eq!(committed.choirs.len(), 1);
    assert_eq!(committed.songs.len(), 1);
    assert_eq!(committed.arrangements.len(), 3);
    shutdown(storage);
}

#[test]
fn a_reopened_parish_shows_the_committed_batch() {
    let dir = TempDir::new("reopen");
    let storage = open(&dir);
    let seed = seed(&storage);
    let committed = bounded(storage.commit_import(five_mutations(&seed))).expect("commit");
    let live = bounded(storage.library()).expect("library");
    shutdown(storage);

    let reopened = open(&dir);
    let library = bounded(reopened.library()).expect("library after reopen");
    assert_eq!(library, live);
    assert!(library.choirs.contains(&committed.choirs[0]));
    assert!(library.songs.contains(&committed.songs[0]));
    for arrangement in &committed.arrangements {
        assert!(library.arrangements.contains(arrangement));
    }
    shutdown(reopened);
}
