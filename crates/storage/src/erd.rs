//! Generated physical-schema reference (`ERD.md`).
//!
//! The renderer migrates an in-memory database with the same bundled SQL the
//! application uses, then reads `sqlite_master` and the `pragma_*` table-valued
//! functions. The committed `ERD.md` must equal the render, so a schema change
//! without a regenerated reference fails `cargo test -p storage`. Set
//! `ORDO_BLESS_ERD=1` to rewrite the file.
//!
//! Ownership and lifecycle notes sit beside the renderer in [`TABLE_NOTES`].
//! A new table without notes fails the same test, which keeps the narrative
//! reference from quietly falling behind the physical schema.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::migrations;

/// Write one line, treating a write to a `String` as infallible.
macro_rules! write_line {
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
    checks: Vec<String>,
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
    columns: Vec<String>,
}

struct ForeignKey {
    from: Vec<String>,
    table: String,
    to: Vec<String>,
    on_delete: String,
}

/// Human ownership and lifecycle text for one table.
struct Notes {
    owner: &'static str,
    lifecycle: &'static str,
}

const TABLE_NOTES: &[(&str, Notes)] = &[
    (
        "arrangement",
        Notes {
            owner: "T2.1 parish foundation; the Song Library (T2.3) owns Arrangement behavior.",
            lifecycle: "Belongs to one Song work. Archived rather than deleted while history references it (Decision 43).",
        },
    ),
    (
        "arrangement_choir",
        Notes {
            owner: "T2.1 parish foundation; the Song Library owns Choir applicability.",
            lifecycle: "Many-to-many link between an Arrangement and the Choirs that share that exact rendition.",
        },
    ),
    (
        "arrangement_language",
        Notes {
            owner: "T2.1 parish foundation; the Song Library owns language metadata.",
            lifecycle: "One language tag per Arrangement. A translated rendition remains a separate Arrangement.",
        },
    ),
    (
        "arrangement_occasion",
        Notes {
            owner: "T2.1 parish foundation; the Song Library owns occasion applicability.",
            lifecycle: "Advisory occasion tag on one Arrangement; never a liturgical restriction.",
        },
    ),
    (
        "arrangement_section",
        Notes {
            owner: "T2.1 parish foundation; the Arrangement editor owns section content.",
            lifecycle: "Reusable musical or lyrical unit. Occurrences reference it, so deletion is reference-checked.",
        },
    ),
    (
        "celebration_package",
        Notes {
            owner: "T2.1 placeholder for portable Celebration Packages.",
            lifecycle: "Rows arrive with package persistence; T2.1 creates the table only.",
        },
    ),
    (
        "choir",
        Notes {
            owner: "T2.1 parish foundation; the Choir workspace (T2.4) owns presentation.",
            lifecycle: "Created explicitly and retained while an Arrangement references it.",
        },
    ),
    (
        "edit_revision",
        Notes {
            owner: "T2.2 edit-safety ledger (placeholder here).",
            lifecycle: "Append-only revision history; each row names its parent and a content hash.",
        },
    ),
    (
        "managed_asset",
        Notes {
            owner: "T2.1 managed assets (T2.1c).",
            lifecycle: "The original bytes live under `assets/<id>`; a row commits with its referencing record or not at all.",
        },
    ),
    (
        "ordo_migrations",
        Notes {
            owner: "Storage migration runner (T0.7); infrastructure, not parish data.",
            lifecycle: "Append-only checksummed history. Each row records one applied migration.",
        },
    ),
    (
        "parish",
        Notes {
            owner: "T2.1 parish foundation; the parish store (T2.1b) owns access.",
            lifecycle: "The single local Parish row, enforced by `singleton`; retained for the database lifetime.",
        },
    ),
    (
        "parish_backup",
        Notes {
            owner: "T2.1 placeholder for portable Parish Backups.",
            lifecycle: "Rows arrive with backup persistence; T2.1 creates the table only.",
        },
    ),
    (
        "parish_note",
        Notes {
            owner: "T0.7 storage spike (`Storage::add_parish_note`).",
            lifecycle: "Retained as applied migration history; T2.1 leaves the spike behavior untouched.",
        },
    ),
    (
        "parish_profile",
        Notes {
            owner: "T0.7 storage spike (`Storage::set_parish_name`); superseded by `parish`.",
            lifecycle: "Retained as applied migration history; T2.1 leaves the spike behavior untouched.",
        },
    ),
    (
        "parish_setting",
        Notes {
            owner: "T2.1 placeholder for parish-scoped key/value settings.",
            lifecycle: "Rows arrive with later settings work; T2.1 does not write them.",
        },
    ),
    (
        "presentation_snapshot",
        Notes {
            owner: "T2.1 placeholder for the Presentation-snapshot ledger.",
            lifecycle: "Rows arrive with snapshot persistence; T2.1 creates the table only.",
        },
    ),
    (
        "schema_metadata",
        Notes {
            owner: "T2.1 schema-format marker.",
            lifecycle: "Seeded with format `ordo-parish` version 1; later migrations advance the version.",
        },
    ),
    (
        "section_occurrence",
        Notes {
            owner: "T2.1 parish foundation; the Arrangement editor owns performance order.",
            lifecycle: "One ordered placement of a section. Several occurrences may reference one reusable section.",
        },
    ),
    (
        "song",
        Notes {
            owner: "T2.1 parish foundation; the Song Library (T2.3) owns Song-work behavior.",
            lifecycle: "Canonical Song work. Archived rather than deleted while retained by history (Decision 43).",
        },
    ),
    (
        "song_alias",
        Notes {
            owner: "T2.1 parish foundation; the Song Library owns alias authoring.",
            lifecycle: "Alias of one Song work. Never merges Songs and never rewrites the authored title.",
        },
    ),
    (
        "song_liturgical_moment",
        Notes {
            owner: "T2.1 parish foundation; the Song Library owns liturgical moments.",
            lifecycle: "One liturgical-moment tag per Song work; advisory metadata only.",
        },
    ),
    (
        "song_occasion",
        Notes {
            owner: "T2.1 parish foundation; the Song Library owns occasion applicability.",
            lifecycle: "One advisory occasion tag per Song work; never a liturgical restriction.",
        },
    ),
    (
        "theme",
        Notes {
            owner: "T2.1 placeholder carrying the first managed-asset reference (T2.1c).",
            lifecycle: "Created with its background asset in one transaction; retained while referenced.",
        },
    ),
];

/// Render the committed reference from a database migrated by this build.
fn render(tables: &[Table]) -> Result<String, String> {
    for (name, _) in TABLE_NOTES {
        if !tables.iter().any(|table| table.name == *name) {
            return Err(format!("ERD notes name missing table `{name}`"));
        }
    }

    let mut out = String::new();
    write_line!(
        out,
        "# Ordo parish storage ERD\n\n\
         <!-- Generated from the authoritative migrations by \
         `crates/storage/src/erd.rs`; do not edit by hand. -->\n\n\
         This is the physical schema the bundled migrations create: the Mermaid\n\
         overview lists every table and foreign-key relationship, and each table\n\
         section records its columns, constraints, indexes, ownership, and lifecycle.\n\n\
         Regenerate with `ORDO_BLESS_ERD=1 cargo test -p storage erd`.\n"
    );

    write_line!(out, "## Physical overview\n");
    write_line!(out, "```mermaid");
    write_line!(out, "erDiagram");
    for table in tables {
        write_line!(out, "    {} {{", table.name);
        for column in &table.columns {
            let mut key = String::new();
            if column.primary_key != 0 {
                key.push_str(" PK");
            }
            if table
                .foreign_keys
                .iter()
                .any(|foreign_key| foreign_key.from.contains(&column.name))
            {
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
            write_line!(
                out,
                "    {} ||--o{{ {} : \"{}\"",
                foreign_key.table,
                table.name,
                foreign_key.from.join(", ")
            );
        }
    }
    write_line!(out, "```\n");

    write_line!(out, "## Tables\n");
    for table in tables {
        let Some((_, notes)) = TABLE_NOTES
            .iter()
            .find(|(name, _)| *name == table.name.as_str())
        else {
            return Err(format!("table `{}` has no ERD notes", table.name));
        };
        write_line!(out, "### `{}`\n", table.name);
        write_line!(out, "- **Ownership:** {}", notes.owner);
        write_line!(out, "- **Lifecycle:** {}\n", notes.lifecycle);

        write_line!(out, "| Column | Type | Null | Default | Key |");
        write_line!(out, "| --- | --- | --- | --- | --- |");
        for column in &table.columns {
            let key = if column.primary_key == 0 {
                String::new()
            } else {
                "PK".to_owned()
            };
            let default = column
                .default
                .as_deref()
                .filter(|value| !value.is_empty())
                .map_or_else(|| "(none)".to_owned(), |value| format!("`{value}`"));
            write_line!(
                out,
                "| `{}` | {} | {} | {} | {} |",
                column.name,
                column.type_name,
                if column.not_null { "no" } else { "yes" },
                default,
                key
            );
        }
        write_line!(out, "");

        write_line!(out, "**Checks**\n");
        if table.checks.is_empty() {
            write_line!(out, "- (none)\n");
        } else {
            for check in &table.checks {
                write_line!(out, "- `{}`", check);
            }
            write_line!(out, "");
        }

        write_line!(out, "**Indexes**\n");
        if table.indexes.is_empty() {
            write_line!(out, "- (none)\n");
        } else {
            write_line!(out, "| Name | Unique | Origin | Columns |");
            write_line!(out, "| --- | --- | --- | --- |");
            for index in &table.indexes {
                let columns = index
                    .columns
                    .iter()
                    .map(|column| format!("`{column}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                write_line!(
                    out,
                    "| `{}` | {} | {} | {} |",
                    index.name,
                    if index.unique { "yes" } else { "no" },
                    origin(&index.origin),
                    columns
                );
            }
            write_line!(out, "");
        }

        write_line!(out, "**Foreign keys**\n");
        if table.foreign_keys.is_empty() {
            write_line!(out, "- (none)\n");
        } else {
            write_line!(out, "| Columns | References | On delete |");
            write_line!(out, "| --- | --- | --- |");
            for foreign_key in &table.foreign_keys {
                let from = foreign_key
                    .from
                    .iter()
                    .map(|column| format!("`{column}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let to = if foreign_key.to.is_empty() {
                    "(primary key)".to_owned()
                } else {
                    foreign_key
                        .to
                        .iter()
                        .map(|column| format!("`{column}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                write_line!(
                    out,
                    "| {} | `{}`({}) | {} |",
                    from,
                    foreign_key.table,
                    to,
                    foreign_key.on_delete
                );
            }
            write_line!(out, "");
        }
    }

    Ok(out)
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
    let names: Vec<(String, Option<String>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query sqlite_master")
        .map(|row| row.expect("sqlite_master row"))
        .collect();
    drop(statement);

    names
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
                "SELECT name, \"unique\", origin \
                 FROM pragma_index_list(?1) ORDER BY name",
            )
            .expect("prepare index_list");
        let rows: Vec<(String, bool, String)> = statement
            .query_map([name.as_str()], |row| {
                Ok((row.get(0)?, row.get::<_, i64>(1)? != 0, row.get(2)?))
            })
            .expect("query index_list")
            .map(|row| row.expect("index_list row"))
            .collect();
        drop(statement);
        rows.into_iter()
            .map(|(index, unique, origin)| Index {
                columns: index_columns(conn, &index),
                name: index,
                unique,
                origin,
            })
            .collect()
    };

    let foreign_keys = {
        let mut statement = conn
            .prepare(
                "SELECT id, \"table\", \"from\", \"to\", on_delete \
                 FROM pragma_foreign_key_list(?1) ORDER BY id, seq",
            )
            .expect("prepare foreign_key_list");
        let rows = statement
            .query_map([name.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .expect("query foreign_key_list");
        let mut grouped: BTreeMap<i64, ForeignKey> = BTreeMap::new();
        for row in rows {
            let (id, table, from, to, on_delete) = row.expect("foreign_key_list row");
            let entry = grouped.entry(id).or_insert_with(|| ForeignKey {
                from: Vec::new(),
                table,
                to: Vec::new(),
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
        checks: checks_in(&sql),
        name,
        columns,
        indexes,
        foreign_keys,
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

/// Extract `CHECK (...)` expressions from one table's original SQL text.
fn checks_in(sql: &str) -> Vec<String> {
    let lowercase = sql.to_ascii_lowercase();
    let bytes = sql.as_bytes();
    let mut checks = Vec::new();
    let mut search = 0;
    while let Some(relative) = lowercase[search..].find("check") {
        let at = search + relative;
        let after = at + "check".len();
        let word_before = at > 0 && is_word(bytes[at - 1]);
        let word_after = after < bytes.len() && is_word(bytes[after]);
        if word_before || word_after {
            search = after;
            continue;
        }
        let mut open = after;
        while open < bytes.len() && bytes[open].is_ascii_whitespace() {
            open += 1;
        }
        if bytes.get(open) != Some(&b'(') {
            search = after;
            continue;
        }
        let Some(close) = matching_paren(bytes, open) else {
            break;
        };
        checks.push(normalize_whitespace(&sql[open + 1..close]));
        search = close + 1;
    }
    checks
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quoted = false;
    for (index, byte) in bytes.iter().copied().enumerate().skip(open) {
        match byte {
            b'\'' => quoted = !quoted,
            b'(' if !quoted => depth += 1,
            b')' if !quoted => {
                depth -= 1;
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

fn mermaid_type(type_name: &str) -> String {
    let compact = type_name.split_whitespace().collect::<Vec<_>>().join("");
    if compact.is_empty() {
        "ANY".to_owned()
    } else {
        compact
    }
}

fn origin(origin: &str) -> &str {
    match origin {
        "c" => "created",
        "u" => "unique constraint",
        "pk" => "primary key",
        other => other,
    }
}

#[test]
fn erd_matches_the_authoritative_migrations() {
    let tables = migrate_and_read();
    let rendered = render(&tables).unwrap_or_else(|problem| panic!("ERD render failed: {problem}"));

    let path = erd_path();
    if std::env::var_os("ORDO_BLESS_ERD").is_some() {
        std::fs::write(&path, &rendered).expect("write ERD.md");
    }
    let committed = std::fs::read_to_string(&path).expect("read ERD.md");
    assert_eq!(
        committed, rendered,
        "ERD.md is stale; regenerate it with ORDO_BLESS_ERD=1 cargo test -p storage erd"
    );
}
