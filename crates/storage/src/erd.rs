//! Generated physical-schema reference (`ERD.md`).
//!
//! The renderer migrates an in-memory database with the same bundled SQL the
//! application uses, then reads `sqlite_master` and the `pragma_*` table-valued
//! functions. The committed `ERD.md` must equal the render, so a schema change
//! without a regenerated reference fails `cargo test -p storage`. Set
//! `ORDO_BLESS_ERD=1` to rewrite the file.
//!
//! Ownership and lifecycle notes sit beside the renderer in [`NOTES`]. Every
//! table needs exactly one note, and every note must name a live table.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::migrations;

/// Write one line, treating a write to a `String` as infallible.
macro_rules! write_line {
    ($out:expr) => {
        writeln!($out).expect("writing to a String cannot fail")
    };
    ($out:expr, $($arg:tt)*) => {
        writeln!($out, $($arg)*).expect("writing to a String cannot fail")
    };
}

/// One table as the migrated schema declares it.
struct Table {
    name: String,
    columns: Vec<Column>,
    indexes: Vec<Index>,
    foreign_keys: Vec<ForeignKey>,
    checks: Checks,
    without_rowid: bool,
}

struct Column {
    name: String,
    type_name: String,
    not_null: bool,
    default: Option<String>,
    primary_key: i64,
}

struct Index {
    name: String,
    unique: bool,
    origin: String,
    partial: bool,
    columns: Vec<String>,
}

struct ForeignKey {
    from: Vec<String>,
    table: String,
    to: Vec<String>,
    on_update: String,
    on_delete: String,
}

/// CHECK expressions parsed from one `CREATE TABLE` statement.
#[derive(Default)]
struct Checks {
    /// `(column, expression)` pairs in declaration order.
    columns: Vec<(String, String)>,
    /// Table-level checks in declaration order.
    table: Vec<String>,
}

impl Checks {
    fn column(&self, name: &str) -> Option<&str> {
        self.columns
            .iter()
            .find(|(column, _)| column == name)
            .map(|(_, expression)| expression.as_str())
    }
}

/// Ownership and lifecycle text for one table, verbatim from the T2.1 spec.
const NOTES: &[(&str, &str, &str)] = &[
    (
        "ordo_migrations",
        "Storage migration runner",
        "One row per applied migration, written in the same transaction as the migration; never edited.",
    ),
    (
        "parish_profile",
        "T0.7 storage spike",
        "Spike table kept so checksummed history stays valid; no product code reads it.",
    ),
    (
        "parish_note",
        "T0.7 storage spike",
        "Spike table kept so checksummed history stays valid; no product code reads it.",
    ),
    (
        "schema_metadata",
        "Storage Module",
        "Written by migrations only; identifies the file as an Ordo parish database.",
    ),
    (
        "parish",
        "Parish setup",
        "Exactly one row, created when new-Parish setup commits; never deleted.",
    ),
    (
        "parish_setting",
        "Parish settings (placeholder)",
        "Upserted per key; schema only until Settings tasks use it.",
    ),
    (
        "choir",
        "Song Library",
        "Created by the operator; archived by setting archived_at; never cascades.",
    ),
    (
        "song",
        "Song Library",
        "Song work; archived by setting archived_at (non-cascading); permanent deletion only after reference checks.",
    ),
    (
        "song_alias",
        "Song Library",
        "Owned by its song; ordered by position; replaced with the song's details.",
    ),
    (
        "song_liturgical_moment",
        "Song Library",
        "Owned by its song; replaced with the song's details.",
    ),
    (
        "song_occasion",
        "Song Library",
        "Owned by its song; replaced with the song's details.",
    ),
    (
        "arrangement",
        "Song Library",
        "Belongs to one song; archived independently of the song; permanent deletion only after reference checks.",
    ),
    (
        "arrangement_language",
        "Song Library",
        "Owned by its arrangement; ordered by position.",
    ),
    (
        "arrangement_choir",
        "Song Library",
        "Choir association of an arrangement; many choirs may share one arrangement.",
    ),
    (
        "arrangement_occasion",
        "Song Library",
        "Owned by its arrangement.",
    ),
    (
        "arrangement_section",
        "Song Library",
        "Owned by its arrangement; ordered by position; a blank line in lyrics is a slide boundary.",
    ),
    (
        "section_occurrence",
        "Song Library",
        "One placement of a section of the same arrangement in its Sequence; ordered by position.",
    ),
    (
        "managed_asset",
        "Storage Module (managed assets)",
        "Row committed together with its file under assets/<id>; bytes never change under one id.",
    ),
    (
        "theme",
        "Themes (placeholder)",
        "Placeholder for the Theme tasks; background_asset_id references a managed asset.",
    ),
    (
        "edit_revision",
        "Edit revisions (T2.2)",
        "Append-only, one linear chain per entity; every committed edit point and every restoration appends one; content is canonical JSON hashed by content_sha256.",
    ),
    (
        "presentation_snapshot",
        "Presentation snapshots (placeholder)",
        "Immutable once captured; schema only until snapshot capture uses it.",
    ),
    (
        "celebration_package",
        "Celebration Packages (placeholder)",
        "Schema only until package export and install use it.",
    ),
    (
        "parish_backup",
        "Parish Backups (placeholder)",
        "Schema only until backup and restore use it.",
    ),
];

/// Render the committed reference from a database migrated by this build.
fn render(tables: &[Table]) -> String {
    let (version, name) = head_migration();
    let mut out = String::new();

    write_line!(out, "# Ordo parish database ERD");
    write_line!(out);
    write_line!(
        out,
        "<!-- Generated by `crates/storage/src/erd.rs` from the bundled migrations."
    );
    write_line!(
        out,
        "     Do not edit by hand: run `ORDO_BLESS_ERD=1 cargo test -p storage erd`"
    );
    write_line!(
        out,
        "     and commit the result. Domain meaning lives in CONTEXT.md. -->"
    );
    write_line!(out);
    write_line!(out, "Schema version: {version} (`{name}`).");
    write_line!(out);
    write_line!(out, "## Overview");
    write_line!(out);
    write_line!(out, "```mermaid");
    write_line!(out, "erDiagram");

    for table in tables {
        let foreign_columns: BTreeSet<&str> = table
            .foreign_keys
            .iter()
            .flat_map(|foreign_key| foreign_key.from.iter().map(String::as_str))
            .collect();
        write_line!(out, "    {} {{", table.name);
        for column in &table.columns {
            let mut key = String::new();
            if column.primary_key != 0 {
                key.push_str(" PK");
            }
            if foreign_columns.contains(column.name.as_str()) {
                if key.is_empty() {
                    key.push_str(" FK");
                } else {
                    key.push_str(", FK");
                }
            }
            write_line!(
                out,
                "        {} {}{}",
                mermaid_type(&column.type_name),
                column.name,
                key
            );
        }
        write_line!(out, "    }}");
    }

    for table in tables {
        for foreign_key in &table.foreign_keys {
            let all_not_null = foreign_key.from.iter().all(|column| {
                table
                    .columns
                    .iter()
                    .find(|candidate| candidate.name == *column)
                    .is_some_and(|candidate| candidate.not_null)
            });
            let cardinality = if all_not_null { "||" } else { "|o" };
            write_line!(
                out,
                "    {} {}--o{{ {} : \"{}\"",
                foreign_key.table,
                cardinality,
                table.name,
                foreign_key.from.join(", ")
            );
        }
    }
    write_line!(out, "```");
    write_line!(out);

    for table in tables {
        let (owner, lifecycle) = notes_for(&table.name);
        write_line!(out, "## {}", table.name);
        write_line!(out);
        write_line!(out, "**Owner:** {owner}. **Lifecycle:** {lifecycle}");
        write_line!(out);
        write_line!(out, "| Column | Type | Null | Default | Key | Check |");
        write_line!(out, "|---|---|---|---|---|---|");

        for column in &table.columns {
            let type_name = if column.type_name.trim().is_empty() {
                "—".to_owned()
            } else {
                column.type_name.trim().to_ascii_lowercase()
            };
            let default = column.default.as_deref().unwrap_or("—");
            let check = table
                .checks
                .column(&column.name)
                .map(|expression| {
                    format!(
                        "`{}`",
                        escape_cell(&summarize_check(&column.name, expression))
                    )
                })
                .unwrap_or_else(|| "—".to_owned());
            write_line!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                escape_cell(&column.name),
                escape_cell(&type_name),
                if column.not_null || is_implicit_integer_primary_key(table, column) {
                    "no"
                } else {
                    "yes"
                },
                escape_cell(default),
                primary_key_label(column.primary_key),
                check
            );
        }
        write_line!(out);

        let primary_key: Vec<&str> = table
            .columns
            .iter()
            .filter(|column| column.primary_key != 0)
            .map(|column| column.name.as_str())
            .collect();
        let mut details = false;
        if primary_key.len() > 1 {
            write_line!(out, "Primary key: ({}).", primary_key.join(", "));
            details = true;
        }
        if table.without_rowid {
            write_line!(out, "WITHOUT ROWID.");
            details = true;
        }
        if details {
            write_line!(out);
        }

        if table.indexes.is_empty() {
            write_line!(out, "Indexes: none.");
        } else {
            write_line!(out, "Indexes:");
            for index in &table.indexes {
                let mut line = format!("- `{}` on ({})", index.name, index.columns.join(", "));
                if index.unique {
                    line.push_str(", unique");
                }
                if index.origin == "u" || index.origin == "pk" {
                    line.push_str(" (constraint)");
                }
                if index.partial {
                    line.push_str(" (partial)");
                }
                write_line!(out, "{line}");
            }
        }
        write_line!(out);

        if table.foreign_keys.is_empty() {
            write_line!(out, "Foreign keys: none.");
        } else {
            write_line!(out, "Foreign keys:");
            for foreign_key in &table.foreign_keys {
                write_line!(
                    out,
                    "- ({}) → `{}`({}), on delete {}, on update {}",
                    foreign_key.from.join(", "),
                    foreign_key.table,
                    foreign_key.to.join(", "),
                    foreign_key.on_delete,
                    foreign_key.on_update
                );
            }
        }
        write_line!(out);

        if !table.checks.table.is_empty() {
            write_line!(out, "Table checks:");
            for check in &table.checks.table {
                write_line!(out, "- `{check}`");
            }
            write_line!(out);
        }
    }

    out
}

fn notes_for(table: &str) -> (&'static str, &'static str) {
    let entry = NOTES
        .iter()
        .find(|(name, _, _)| *name == table)
        .unwrap_or_else(|| panic!("table `{table}` has no ownership notes"));
    (entry.1, entry.2)
}

/// The bundled head migration: schema version and name.
fn head_migration() -> (u32, &'static str) {
    let head = migrations::bundled()
        .pop()
        .expect("at least one bundled migration");
    (head.id, head.name)
}

fn primary_key_label(position: i64) -> String {
    match position {
        0 => "—".to_owned(),
        1 => "PK".to_owned(),
        position => format!("PK({position})"),
    }
}

/// SQLite's `INTEGER PRIMARY KEY` is the rowid alias and is implicitly NOT
/// NULL even though `pragma_table_info` reports `notnull = 0`.
fn is_implicit_integer_primary_key(table: &Table, column: &Column) -> bool {
    if table.without_rowid || column.primary_key == 0 {
        return false;
    }
    let primary_key_columns = table
        .columns
        .iter()
        .filter(|candidate| candidate.primary_key != 0)
        .count();
    primary_key_columns == 1 && column.type_name.trim().eq_ignore_ascii_case("integer")
}

fn mermaid_type(type_name: &str) -> String {
    let lower = type_name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        "any".to_owned()
    } else {
        lower
    }
}

/// The exact canonical UUIDv4 CHECK the migration authors for `column`.
fn canonical_uuid_check(column: &str) -> String {
    format!(
        "length({column}) = 36 AND {column} NOT GLOB '*[^0-9a-f-]*' \
         AND length(replace({column}, '-', '')) = 32 \
         AND substr({column}, 9, 1) = '-' \
         AND substr({column}, 14, 1) = '-' \
         AND substr({column}, 19, 1) = '-' \
         AND substr({column}, 24, 1) = '-' \
         AND substr({column}, 15, 1) = '4' \
         AND substr({column}, 20, 1) IN ('8', '9', 'a', 'b')"
    )
}

/// The GLOB half of the canonical UTC-time CHECK for `column`.
fn canonical_time_glob(column: &str) -> String {
    format!(
        "{column} GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'"
    )
}

/// The exact canonical UTC-time CHECK the migration authors for a non-null
/// `column`.
fn canonical_time_check(column: &str) -> String {
    format!(
        "{} AND strftime('%Y-%m-%dT%H:%M:%fZ', {column}) IS {column} \
         AND substr({column}, 12, 2) <= '23'",
        canonical_time_glob(column)
    )
}

/// The exact nullable canonical UTC-time CHECK for `column`.
fn nullable_canonical_time_check(column: &str) -> String {
    format!("{column} IS NULL OR ({})", canonical_time_check(column))
}

fn summarize_check(column: &str, expression: &str) -> String {
    if expression == canonical_uuid_check(column) {
        return "canonical UUIDv4".to_owned();
    }
    if expression == nullable_canonical_uuid_check(column) {
        return "NULL or canonical UUIDv4".to_owned();
    }
    if expression == canonical_time_check(column) {
        return "canonical UTC time".to_owned();
    }
    if expression == nullable_canonical_time_check(column) {
        return "NULL or canonical UTC time".to_owned();
    }
    expression.to_owned()
}

/// The exact nullable canonical UUIDv4 CHECK for `column`.
fn nullable_canonical_uuid_check(column: &str) -> String {
    format!("{column} IS NULL OR ({})", canonical_uuid_check(column))
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// The committed file lives at the repository root, two levels above this crate.
fn erd_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ERD.md")
}

fn migrate_and_read() -> Vec<Table> {
    let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
    migrations::open_and_migrate(&mut conn, &migrations::bundled()).expect("migrations apply");

    let mut statement = conn
        .prepare(
            "SELECT name, sql FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("read sqlite_master");
    let tables: Vec<(String, Option<String>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query sqlite_master")
        .map(|row| row.expect("sqlite_master row"))
        .collect();
    drop(statement);

    tables
        .into_iter()
        .map(|(name, sql)| read_table(&conn, name, sql.unwrap_or_default()))
        .collect()
}

fn read_table(conn: &Connection, name: String, sql: String) -> Table {
    let columns = {
        let mut statement = conn
            .prepare(
                "SELECT name, type, \"notnull\", dflt_value, pk \
                 FROM pragma_table_info(?1) ORDER BY cid",
            )
            .expect("prepare table_info");
        let rows = statement
            .query_map([name.as_str()], |row| {
                Ok(Column {
                    name: row.get(0)?,
                    type_name: row.get(1)?,
                    not_null: row.get::<_, i64>(2)? != 0,
                    default: row.get(3)?,
                    primary_key: row.get(4)?,
                })
            })
            .expect("query table_info");
        rows.map(|row| row.expect("table_info row")).collect()
    };

    let indexes = {
        let mut statement = conn
            .prepare(
                "SELECT name, \"unique\", origin, partial \
                 FROM pragma_index_list(?1) ORDER BY name",
            )
            .expect("prepare index_list");
        let rows: Vec<(String, bool, String, bool)> = statement
            .query_map([name.as_str()], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? != 0,
                    row.get(2)?,
                    row.get::<_, i64>(3)? != 0,
                ))
            })
            .expect("query index_list")
            .map(|row| row.expect("index_list row"))
            .collect();
        drop(statement);
        rows.into_iter()
            .map(|(index, unique, origin, partial)| Index {
                columns: index_columns(conn, &index),
                name: index,
                unique,
                origin,
                partial,
            })
            .collect()
    };

    let foreign_keys = {
        let mut statement = conn
            .prepare(
                "SELECT id, seq, \"table\", \"from\", \"to\", on_update, on_delete \
                 FROM pragma_foreign_key_list(?1) ORDER BY id, seq",
            )
            .expect("prepare foreign_key_list");
        let rows = statement
            .query_map([name.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .expect("query foreign_key_list");
        let mut grouped: BTreeMap<i64, ForeignKey> = BTreeMap::new();
        for row in rows {
            let (id, _seq, table, from, to, on_update, on_delete) =
                row.expect("foreign_key_list row");
            let entry = grouped.entry(id).or_insert_with(|| ForeignKey {
                from: Vec::new(),
                table,
                to: Vec::new(),
                on_update,
                on_delete,
            });
            entry.from.push(from);
            if let Some(to) = to {
                entry.to.push(to);
            }
        }
        grouped.into_values().collect()
    };

    Table {
        checks: parse_checks(&sql),
        name,
        columns,
        indexes,
        foreign_keys,
        without_rowid: sql.to_ascii_uppercase().contains("WITHOUT ROWID"),
    }
}

fn index_columns(conn: &Connection, index: &str) -> Vec<String> {
    let mut statement = conn
        .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
        .expect("prepare index_info");
    statement
        .query_map([index], |row| row.get::<_, Option<String>>(0))
        .expect("query index_info")
        .map(|row| row.expect("index_info row").unwrap_or_default())
        .collect()
}

/// Split one table body at top-level commas, respecting quoted strings and
/// quoted identifiers.
fn split_segments(body: &str) -> Vec<&str> {
    let bytes = body.as_bytes();
    let mut segments = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    let mut quote: Option<u8> = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(active) = quote {
            if byte == active {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                segments.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    segments.push(&body[start..]);
    segments
}

/// The first token of a column definition, with double-quoted identifiers
/// unquoted. A quoted name may contain whitespace; an unquoted name ends at
/// whitespace or the opening parenthesis of its declared type.
fn first_token(segment: &str) -> &str {
    let segment = segment.trim_start();
    if let Some(rest) = segment.strip_prefix('"')
        && let Some(end) = rest.find('"')
    {
        return &rest[..end];
    }
    segment
        .split(|character: char| character.is_ascii_whitespace() || character == '(')
        .next()
        .unwrap_or_default()
}

/// Parse column-level and table-level CHECK expressions from one table SQL.
fn parse_checks(sql: &str) -> Checks {
    let mut checks = Checks::default();
    let Some(open) = sql.find('(') else {
        return checks;
    };
    let Some(close) = matching_paren(sql.as_bytes(), open) else {
        return checks;
    };
    let body = &sql[open + 1..close];

    for segment in split_segments(body) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let first = first_token(segment);
        let expressions = extract_checks(segment);
        if expressions.is_empty() {
            continue;
        }
        if first.eq_ignore_ascii_case("check") {
            checks.table.extend(expressions);
        } else {
            checks.columns.extend(
                expressions
                    .into_iter()
                    .map(|expression| (first.to_owned(), expression)),
            );
        }
    }

    checks
}

/// Extract every `CHECK (...)` expression outside quoted strings and quoted
/// identifiers.
fn extract_checks(fragment: &str) -> Vec<String> {
    let lowercase = fragment.to_ascii_lowercase();
    let bytes = fragment.as_bytes();
    let lower = lowercase.as_bytes();
    let mut checks = Vec::new();
    let mut index = 0;
    let mut quote: Option<u8> = None;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if lower[index..].starts_with(b"check") {
            let after = index + "check".len();
            let word_before = index > 0 && is_word(bytes[index - 1]);
            let word_after = after < bytes.len() && is_word(bytes[after]);
            if word_before || word_after {
                index = after;
                continue;
            }
            let mut open = after;
            while open < bytes.len() && bytes[open].is_ascii_whitespace() {
                open += 1;
            }
            if bytes.get(open) == Some(&b'(')
                && let Some(close) = matching_paren(bytes, open)
            {
                checks.push(normalize_whitespace(&fragment[open + 1..close]));
                index = close + 1;
                continue;
            }
            index = after;
            continue;
        }
        index += 1;
    }

    checks
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quote: Option<u8> = None;
    for (index, byte) in bytes.iter().copied().enumerate().skip(open) {
        if let Some(active) = quote {
            if byte == active {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn erd_md_matches_the_bundled_schema() {
    let tables = migrate_and_read();
    let rendered = render(&tables);
    let path = erd_path();

    if std::env::var("ORDO_BLESS_ERD").as_deref() == Ok("1") {
        std::fs::write(&path, &rendered).expect("write ERD.md");
        return;
    }

    let committed = std::fs::read_to_string(&path).expect("read ERD.md");
    assert_eq!(
        committed, rendered,
        "ERD.md is stale; regenerate it with `ORDO_BLESS_ERD=1 cargo test -p storage erd`"
    );
}

#[test]
fn every_table_has_ownership_notes() {
    let tables = migrate_and_read();

    for table in &tables {
        let matches = NOTES
            .iter()
            .filter(|(name, _, _)| *name == table.name)
            .count();
        assert_eq!(
            matches, 1,
            "table `{}` must have exactly one ownership note",
            table.name
        );
    }

    for (name, _, _) in NOTES {
        assert!(
            tables.iter().any(|table| table.name == *name),
            "ownership note names missing table `{name}`"
        );
    }
}

#[test]
fn check_parser_handles_nested_parentheses() {
    let checks = parse_checks(
        "CREATE TABLE t (a TEXT CHECK (length(a) IN (1, 2)), b INTEGER, \
         CHECK ((a IS NULL) <> (b IS NULL)))",
    );

    assert_eq!(checks.column("a"), Some("length(a) IN (1, 2)"));
    assert_eq!(checks.table, vec!["(a IS NULL) <> (b IS NULL)"]);
}

#[test]
fn check_parser_ignores_quoted_check_text_and_unquotes_columns() {
    let checks = parse_checks(
        "CREATE TABLE t (\
         a TEXT DEFAULT 'check (fake)', \
         \"b\" TEXT CHECK (\"b\" GLOB '*[(]x'), \
         c TEXT CHECK (c <> 'check (not a check)'), \
         CHECK (a IS NULL OR c IS NULL))",
    );

    assert_eq!(checks.column("a"), None, "quoted CHECK text is ignored");
    assert_eq!(checks.column("b"), Some("\"b\" GLOB '*[(]x'"));
    assert_eq!(checks.column("c"), Some("c <> 'check (not a check)'"));
    assert_eq!(checks.table, vec!["a IS NULL OR c IS NULL"]);
}

#[test]
fn weakened_id_check_renders_verbatim() {
    let canonical = canonical_uuid_check("id");
    assert_eq!(summarize_check("id", &canonical), "canonical UUIDv4");

    let weakened = canonical.replace(" AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')", "");
    assert_ne!(weakened, canonical);
    assert_eq!(summarize_check("id", &weakened), weakened);
}

#[test]
fn canonical_check_templates_match_the_bundled_migration() {
    let tables = migrate_and_read();
    let song = tables
        .iter()
        .find(|table| table.name == "song")
        .expect("song table");

    assert_eq!(
        song.checks.column("id"),
        Some(canonical_uuid_check("id").as_str())
    );
    assert_eq!(
        song.checks.column("created_at"),
        Some(canonical_time_check("created_at").as_str())
    );
    assert_eq!(
        song.checks.column("archived_at"),
        Some(nullable_canonical_time_check("archived_at").as_str())
    );
}

#[test]
fn nullable_uuid_check_is_summarised_and_partial_indexes_are_marked() {
    let tables = migrate_and_read();
    let revision = tables
        .iter()
        .find(|table| table.name == "edit_revision")
        .expect("edit_revision table");
    assert_eq!(
        revision.checks.column("session_id"),
        Some(nullable_canonical_uuid_check("session_id").as_str())
    );
    assert_eq!(
        summarize_check("session_id", &nullable_canonical_uuid_check("session_id")),
        "NULL or canonical UUIDv4"
    );
    let rendered = render(&tables);
    assert!(
        rendered.contains("- `edit_revision_one_song_root` on (song_id), unique (partial)"),
        "partial index must be marked"
    );
    assert!(
        rendered.contains(
            "- `edit_revision_by_song` on (song_id, created_at)
"
        ),
        "ordinary index must not be marked partial"
    );
}

#[test]
fn mermaid_block_has_one_relationship_per_foreign_key() {
    let tables = migrate_and_read();
    let rendered = render(&tables);
    let expected: usize = tables.iter().map(|table| table.foreign_keys.len()).sum();
    let relationships = rendered
        .lines()
        .filter(|line| line.contains("--o{"))
        .count();

    assert_eq!(relationships, expected);
}
