//! Parish-schema conformance tests for migration 3 (T2.1).
//!
//! Each test migrates a fresh in-memory database with the bundled SQL and
//! proves one documented shape or constraint through literal SQL. The live
//! schema is the authority; these tests keep the migration honest.

use std::collections::BTreeSet;

use rusqlite::{Connection, params};

use crate::migrations;

const CREATED_AT: &str = "2026-11-09T01:00:00.000Z";

/// One canonical UUIDv4 text form built from caller-supplied bytes.
fn id(seed: u8) -> String {
    domain::identity::EntityId::from_random_bytes([seed; 16]).to_string()
}

fn migrated() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
    migrations::open_and_migrate(&mut conn, &migrations::bundled()).expect("migrations apply");
    conn
}

fn table_names(conn: &Connection) -> Vec<String> {
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("prepare sqlite_master");
    statement
        .query_map([], |row| row.get(0))
        .expect("query sqlite_master")
        .map(|row| row.expect("sqlite_master row"))
        .collect()
}

#[test]
fn schema_lists_every_t2_1_table() {
    let conn = migrated();

    let actual: BTreeSet<String> = table_names(&conn).into_iter().collect();
    let expected: BTreeSet<String> = [
        "ordo_migrations",
        "parish_profile",
        "parish_note",
        "schema_metadata",
        "parish",
        "parish_setting",
        "choir",
        "choir_language",
        "song",
        "song_alias",
        "song_liturgical_moment",
        "song_occasion",
        "arrangement",
        "arrangement_language",
        "arrangement_choir",
        "arrangement_occasion",
        "arrangement_section",
        "section_occurrence",
        "managed_asset",
        "theme",
        "edit_revision",
        "presentation_snapshot",
        "celebration_package",
        "parish_backup",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    assert_eq!(actual, expected);
}

#[test]
fn foreign_keys_are_on_and_clean() {
    let conn = migrated();

    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("read foreign_keys pragma");
    assert_eq!(foreign_keys, 1);

    let song = id(1);
    let arrangement = id(2);
    let first_section = id(3);
    let second_section = id(4);
    let choir = id(8);
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![song, "Kyrie 8th", CREATED_AT],
    )
    .expect("insert song");
    conn.execute(
        "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![arrangement, song, "English", CREATED_AT],
    )
    .expect("insert arrangement");
    for (section, position, lyrics) in [
        (&first_section, 0_i64, "Kyrie eleison"),
        (&second_section, 1_i64, "Christe eleison"),
    ] {
        conn.execute(
            "INSERT INTO arrangement_section \
             (id, arrangement_id, position, lyrics, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![section, arrangement, position, lyrics, CREATED_AT],
        )
        .expect("insert section");
    }
    for (occurrence, section, position) in [
        (id(5), &first_section, 0_i64),
        (id(6), &second_section, 1),
        (id(7), &first_section, 2),
    ] {
        conn.execute(
            "INSERT INTO section_occurrence \
             (id, arrangement_id, section_id, position, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![occurrence, arrangement, section, position, CREATED_AT],
        )
        .expect("insert occurrence");
    }
    conn.execute(
        "INSERT INTO choir (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![choir, "Schola", CREATED_AT],
    )
    .expect("insert choir");
    conn.execute(
        "INSERT INTO arrangement_choir (arrangement_id, choir_id) VALUES (?1, ?2)",
        params![arrangement, choir],
    )
    .expect("link arrangement to choir");

    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("run foreign_key_check");
    assert_eq!(violations, 0);
}

#[test]
fn occurrence_cannot_place_another_arrangements_section() {
    let conn = migrated();

    let song = id(20);
    let arrangement_a = id(21);
    let arrangement_b = id(22);
    let section_of_b = id(23);
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![song, "Kyrie 8th", CREATED_AT],
    )
    .expect("insert song");
    for (arrangement, title) in [(&arrangement_a, "English"), (&arrangement_b, "Cebuano")] {
        conn.execute(
            "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![arrangement, song, title, CREATED_AT],
        )
        .expect("insert arrangement");
    }
    conn.execute(
        "INSERT INTO arrangement_section \
         (id, arrangement_id, position, lyrics, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            section_of_b,
            arrangement_b,
            0_i64,
            "Kyrie eleison",
            CREATED_AT
        ],
    )
    .expect("insert section");

    let error = conn
        .execute(
            "INSERT INTO section_occurrence \
             (id, arrangement_id, section_id, position, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id(24), arrangement_a, section_of_b, 0_i64, CREATED_AT],
        )
        .expect_err("a section of another arrangement must be refused");

    assert!(
        error.to_string().contains("FOREIGN KEY"),
        "expected a foreign-key constraint error, got: {error}"
    );
}

#[test]
fn ids_must_be_canonical_uuid_v4() {
    let conn = migrated();

    for (index, bad_id) in [
        id(30).to_uppercase(),
        "00000000-0000-11d1-8000-000000000001".to_owned(),
        "00000000-0000-4000-c000-000000000001".to_owned(),
        "00000000-0000-4000-8000-00000000001".to_owned(),
        "00000000-0000-4000-8000-00000000000-".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        let error = conn
            .execute(
                "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
                params![bad_id, format!("Song {index}"), CREATED_AT],
            )
            .expect_err("a non-canonical id must be refused");
        assert!(
            error.to_string().contains("CHECK"),
            "expected a CHECK constraint error for {bad_id}, got: {error}"
        );
    }

    let null_error = conn
        .execute(
            "INSERT INTO song (id, title, created_at) VALUES (NULL, ?1, ?2)",
            params!["No identity", CREATED_AT],
        )
        .expect_err("a NULL id must be refused");
    assert!(
        null_error.to_string().contains("NOT NULL"),
        "expected a NOT NULL constraint error for a NULL id, got: {null_error}"
    );

    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![id(31), "Canonical", CREATED_AT],
    )
    .expect("a canonical v4 id is accepted");
}

#[test]
fn times_must_be_canonical() {
    let conn = migrated();

    for (index, time) in [
        "2026-11-09 01:00:00",
        "2026-11-09T01:00:00Z",
        "2026-11-09T01:00:00.000+00:00",
        "2026-13-01T00:00:00.000Z",
        "2026-02-30T00:00:00.000Z",
        "2026-01-01T24:00:00.000Z",
        "2026-01-01T23:60:00.000Z",
    ]
    .into_iter()
    .enumerate()
    {
        let error = conn
            .execute(
                "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
                params![id(40 + index as u8), "Song", time],
            )
            .expect_err("a non-canonical time must be refused");
        assert!(
            error.to_string().contains("CHECK"),
            "expected a CHECK constraint error for {time}, got: {error}"
        );
    }

    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![id(47), "Leap day", "2024-02-29T23:59:59.999Z"],
    )
    .expect("a canonical leap day is accepted");

    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![id(48), "Canonical", CREATED_AT],
    )
    .expect("a canonical time is accepted");
}

#[test]
fn parish_is_a_single_row() {
    let conn = migrated();

    conn.execute(
        "INSERT INTO parish (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id(50), "St Mary", CREATED_AT],
    )
    .expect("insert the first parish row");

    let error = conn
        .execute(
            "INSERT INTO parish (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id(51), "St Joseph", CREATED_AT],
        )
        .expect_err("a second parish row must be refused");

    assert!(
        error.to_string().contains("parish.singleton"),
        "expected the singleton UNIQUE constraint, got: {error}"
    );
}

#[test]
fn revision_names_exactly_one_entity() {
    let conn = migrated();

    let song = id(60);
    let arrangement = id(61);
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![song, "Kyrie 8th", CREATED_AT],
    )
    .expect("insert song");
    conn.execute(
        "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![arrangement, song, "English", CREATED_AT],
    )
    .expect("insert arrangement");

    let content_sha256 = "a".repeat(64);
    let both = conn
        .execute(
            "INSERT INTO edit_revision              (id, song_id, arrangement_id, kind, content, content_sha256, created_at)              VALUES (?1, ?2, ?3, 'creation', '{}', ?4, ?5)",
            params![id(62), song, arrangement, content_sha256, CREATED_AT],
        )
        .expect_err("naming both a song and an arrangement must be refused");
    assert!(
        both.to_string()
            .contains("CHECK constraint failed: (song_id IS NULL) <> (arrangement_id IS NULL)"),
        "expected the one-entity CHECK, got: {both}"
    );

    let neither = conn
        .execute(
            "INSERT INTO edit_revision (id, kind, content, content_sha256, created_at)              VALUES (?1, 'creation', '{}', ?2, ?3)",
            params![id(63), content_sha256, CREATED_AT],
        )
        .expect_err("naming neither a song nor an arrangement must be refused");
    assert!(
        neither
            .to_string()
            .contains("CHECK constraint failed: (song_id IS NULL) <> (arrangement_id IS NULL)"),
        "expected the one-entity CHECK, got: {neither}"
    );
}

/// Inserts one revision row for `song`; `kind`, `parent`, `session` and
/// `restored_from` are caller-supplied so tests can break one rule at a time.
fn insert_song_revision(
    conn: &Connection,
    seed: u8,
    song: &str,
    kind: &str,
    parent: Option<String>,
    session: Option<String>,
    restored_from: Option<String>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO edit_revision          (id, song_id, parent_id, kind, session_id, restored_from_id, content,           content_sha256, created_at)          VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}', ?7, ?8)",
        params![
            id(seed),
            song,
            parent,
            kind,
            session,
            restored_from,
            "a".repeat(64),
            CREATED_AT
        ],
    )
}

#[test]
fn revision_kind_parent_session_and_restoration_rules() {
    let conn = migrated();
    let song = id(70);
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![song, "Kyrie 8th", CREATED_AT],
    )
    .expect("insert song");
    insert_song_revision(&conn, 71, &song, "creation", None, None, None).expect("root");

    let refused = |result: rusqlite::Result<usize>, fragment: &str| {
        let error = result.expect_err("must be refused");
        assert!(
            error.to_string().contains(fragment),
            "expected {fragment}, got: {error}"
        );
    };
    refused(
        insert_song_revision(&conn, 72, &song, "bogus", Some(id(71)), None, None),
        "CHECK constraint failed: kind IN",
    );
    refused(
        insert_song_revision(&conn, 73, &song, "edit", Some(id(71)), None, None),
        "CHECK constraint failed: (kind = 'edit') = (session_id IS NOT NULL)",
    );
    refused(
        insert_song_revision(&conn, 74, &song, "import", Some(id(71)), Some(id(9)), None),
        "CHECK constraint failed: (kind = 'edit') = (session_id IS NOT NULL)",
    );
    refused(
        insert_song_revision(&conn, 75, &song, "edit", None, Some(id(9)), None),
        "CHECK constraint failed: (kind = 'creation') = (parent_id IS NULL)",
    );
    refused(
        insert_song_revision(&conn, 76, &song, "restoration", Some(id(71)), None, None),
        "CHECK constraint failed: (kind = 'restoration') = (restored_from_id IS NOT NULL)",
    );
    refused(
        insert_song_revision(
            &conn,
            77,
            &song,
            "edit",
            Some(id(71)),
            Some("NOT-A-UUID".to_owned()),
            None,
        ),
        "CHECK constraint failed: session_id IS NULL OR",
    );
    insert_song_revision(&conn, 78, &song, "edit", Some(id(71)), Some(id(9)), None)
        .expect("first child");
    refused(
        insert_song_revision(&conn, 79, &song, "import", Some(id(71)), None, None),
        "UNIQUE constraint failed: edit_revision.parent_id",
    );
}

#[test]
fn one_creation_revision_per_entity() {
    let conn = migrated();
    let song = id(80);
    conn.execute(
        "INSERT INTO song (id, title, created_at) VALUES (?1, ?2, ?3)",
        params![song, "Kyrie 8th", CREATED_AT],
    )
    .expect("insert song");
    insert_song_revision(&conn, 81, &song, "creation", None, None, None).expect("root");
    let error = insert_song_revision(&conn, 82, &song, "creation", None, None, None)
        .expect_err("a second root for the same song must be refused");
    assert!(
        error
            .to_string()
            .contains("UNIQUE constraint failed: edit_revision.song_id"),
        "expected the root index, got: {error}"
    );

    let arrangement = id(83);
    conn.execute(
        "INSERT INTO arrangement (id, song_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![arrangement, song, "English", CREATED_AT],
    )
    .expect("insert arrangement");
    let insert_root = |seed: u8| {
        conn.execute(
            "INSERT INTO edit_revision              (id, arrangement_id, kind, content, content_sha256, created_at)              VALUES (?1, ?2, 'creation', '{}', ?3, ?4)",
            params![id(seed), arrangement, "a".repeat(64), CREATED_AT],
        )
    };
    insert_root(84).expect("arrangement root");
    let error = insert_root(85).expect_err("a second arrangement root must be refused");
    assert!(
        error
            .to_string()
            .contains("UNIQUE constraint failed: edit_revision.arrangement_id"),
        "expected the root index, got: {error}"
    );
}

#[test]
fn no_foreign_key_cascades() {
    let conn = migrated();

    let mut checked = 0_usize;
    for table in table_names(&conn) {
        let mut statement = conn
            .prepare(
                "SELECT \"table\", on_update, on_delete \
                 FROM pragma_foreign_key_list(?1)",
            )
            .expect("prepare foreign_key_list");
        let rows = statement
            .query_map([table.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("query foreign_key_list");
        for row in rows {
            let (parent, on_update, on_delete) = row.expect("foreign_key_list row");
            checked += 1;
            assert!(
                on_delete == "NO ACTION" && on_update == "NO ACTION",
                "{table} -> {parent} has on_delete={on_delete}, on_update={on_update}"
            );
        }
    }
    assert!(checked > 0, "the schema declares no foreign keys");
}

#[test]
fn migration_is_checksummed_and_versioned() {
    let conn = migrated();
    let status = migrations::status(&conn).expect("read status");

    assert_eq!(status.version, 5);
    let names: Vec<String> = status
        .applied
        .into_iter()
        .map(|migration| migration.name)
        .collect();
    assert_eq!(
        names,
        vec![
            "0001_parish_profile".to_owned(),
            "0002_parish_note".to_owned(),
            "0003_parish_foundation".to_owned(),
            "0004_edit_revisions".to_owned(),
            "0005_library_details".to_owned(),
        ]
    );
}

#[test]
fn bundled_migrations_have_no_carriage_returns() {
    for migration in migrations::bundled() {
        assert!(
            !migration.sql.contains('\r'),
            "{} contains carriage returns; migration SQL must be LF on every checkout",
            migration.name
        );
    }
}
