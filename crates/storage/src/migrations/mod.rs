//! Numbered, checksummed, transactional migration runner.
//!
//! The ledger table `ordo_migrations` records each applied migration's id,
//! name, hex SHA-256 of its SQL bytes, and application time. Each migration
//! runs in its own immediate transaction together with its ledger row and
//! `PRAGMA user_version`, so a failure rolls back schema, data, and history.
//!
//! Authoring rules:
//! - Migration SQL must not contain `BEGIN`, `COMMIT`, or `ROLLBACK`. The
//!   runner owns the transaction; a nested transaction control statement would
//!   break the atomicity guarantee above.
//! - `PRAGMA foreign_keys` is a no-op inside a transaction, so policy pragmas
//!   are set before the apply transaction starts, never inside a migration.
//! - The ledger is inspected outside the apply transaction. That is acceptable
//!   for this single-owner spike, where only the storage worker opens the file.
//!   A concurrent second writer would need the inspection inside the
//!   transaction.

use rusqlite::{Connection, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::{AppliedMigration, SchemaStatus, StorageError, is_busy, map_error};

/// One numbered migration. Ids are strictly increasing.
pub(crate) struct Migration {
    pub(crate) id: u32,
    pub(crate) name: &'static str,
    pub(crate) sql: &'static str,
}

const LEDGER_DDL: &str = "CREATE TABLE IF NOT EXISTS ordo_migrations (\
    id INTEGER PRIMARY KEY, \
    name TEXT NOT NULL, \
    checksum TEXT NOT NULL, \
    applied_at TEXT NOT NULL)";

/// The migrations bundled into this build, in order.
pub(crate) fn bundled() -> Vec<Migration> {
    let migrations = vec![
        Migration {
            id: 1,
            name: "0001_parish_profile",
            sql: include_str!("../../../../migrations/0001_parish_profile.sql"),
        },
        Migration {
            id: 2,
            name: "0002_parish_note",
            sql: include_str!("../../../../migrations/0002_parish_note.sql"),
        },
    ];
    debug_assert!(
        migrations.windows(2).all(|pair| pair[0].id < pair[1].id),
        "bundled migration ids must be strictly increasing"
    );
    migrations
}

/// Hex-encoded SHA-256 of the migration SQL bytes.
pub(crate) fn checksum(sql: &str) -> String {
    Sha256::digest(sql.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Open sequence: validate the ledger, set policy pragmas, apply pending
/// migrations in order, and return the resulting status.
pub(crate) fn open_and_migrate(
    conn: &mut Connection,
    migrations: &[Migration],
) -> Result<SchemaStatus, StorageError> {
    let mut ordered: Vec<&Migration> = migrations.iter().collect();
    ordered.sort_by_key(|migration| migration.id);

    let applied = inspect(conn, &ordered)?;

    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(map_error)?;
    conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
        row.get::<_, String>(0)
    })
    .map_err(map_error)?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(map_error)?;

    for migration in ordered {
        if applied.iter().any(|row| row.id == migration.id) {
            continue;
        }
        apply(conn, migration)?;
    }
    status(conn)
}

/// Read-only validation of the database against the bundled migrations. Runs
/// before anything is mutated. Refuses, in order:
/// 1. a `user_version` or ledger id above the bundled head (`NewerSchema`);
/// 2. a ledger that is not a contiguous prefix of the bundled ids
///    (`UnknownMigrationLedger`);
/// 3. a recorded checksum that differs from the bundled SQL (`ChecksumMismatch`).
fn inspect(
    conn: &Connection,
    migrations: &[&Migration],
) -> Result<Vec<AppliedMigration>, StorageError> {
    let applied = read_ledger(conn)?;
    let head = migrations.last().map_or(0, |migration| migration.id);

    let user_version = user_version(conn)?;
    if user_version > i64::from(head) {
        return Err(StorageError::NewerSchema {
            found: u32::try_from(user_version).unwrap_or(u32::MAX),
            supported: head,
        });
    }
    if let Some(found) = applied.iter().map(|row| row.id).max()
        && found > head
    {
        return Err(StorageError::NewerSchema {
            found,
            supported: head,
        });
    }

    for (position, row) in applied.iter().enumerate() {
        let expected = migrations.get(position).map(|migration| migration.id);
        if expected != Some(row.id) {
            return Err(StorageError::UnknownMigrationLedger { id: row.id });
        }
    }
    for (migration, row) in migrations.iter().zip(&applied) {
        if checksum(migration.sql) != row.checksum {
            return Err(StorageError::ChecksumMismatch { id: row.id });
        }
    }
    Ok(applied)
}

fn user_version(conn: &Connection) -> Result<i64, StorageError> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(map_error)
}

fn read_ledger(conn: &Connection) -> Result<Vec<AppliedMigration>, StorageError> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ordo_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(map_error)?;
    if exists == 0 {
        return Ok(Vec::new());
    }

    let mut statement = conn
        .prepare("SELECT id, name, checksum FROM ordo_migrations ORDER BY id")
        .map_err(map_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(map_error)?;

    let mut applied = Vec::new();
    for row in rows {
        let (id, name, checksum) = row.map_err(map_error)?;
        let id = u32::try_from(id)
            .map_err(|_| StorageError::Sqlite(format!("invalid ledger id {id}")))?;
        applied.push(AppliedMigration { id, name, checksum });
    }
    Ok(applied)
}

/// Applies one migration atomically: schema SQL, ledger row, and
/// `user_version` commit together or not at all.
fn apply(conn: &mut Connection, migration: &Migration) -> Result<(), StorageError> {
    let id = migration.id;
    let failed = |error: rusqlite::Error| migration_error(id, error);

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failed)?;
    tx.execute_batch(LEDGER_DDL).map_err(failed)?;
    tx.execute_batch(migration.sql).map_err(failed)?;
    tx.execute(
        "INSERT INTO ordo_migrations (id, name, checksum, applied_at) \
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![i64::from(id), migration.name, checksum(migration.sql)],
    )
    .map_err(failed)?;
    tx.pragma_update(None, "user_version", id).map_err(failed)?;
    tx.commit().map_err(failed)?;
    Ok(())
}

fn migration_error(id: u32, error: rusqlite::Error) -> StorageError {
    if is_busy(&error) {
        StorageError::Busy
    } else {
        StorageError::MigrationFailed {
            id,
            message: error.to_string(),
        }
    }
}

/// Reports the ledger and connection policy. Reads only.
pub(crate) fn status(conn: &Connection) -> Result<SchemaStatus, StorageError> {
    let applied = read_ledger(conn)?;
    let version = applied.last().map_or(0, |row| row.id);
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(map_error)?;
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(map_error)?;
    Ok(SchemaStatus {
        version,
        applied,
        journal_mode,
        foreign_keys: foreign_keys != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    fn open(dir: &TempDir) -> Connection {
        let conn = Connection::open(dir.path().join("migrate.sqlite3"));
        assert!(conn.is_ok());
        conn.unwrap_or_else(|_| unreachable!())
    }

    fn scalar_i64(conn: &Connection, sql: &str) -> i64 {
        let value = conn.query_row(sql, [], |row| row.get(0));
        assert!(value.is_ok(), "query failed: {sql}");
        value.unwrap_or(-1)
    }

    #[test]
    fn fresh_database_applies_in_order_and_records_checksums() {
        let dir = TempDir::new("fresh");
        let mut conn = open(&dir);
        let status = open_and_migrate(&mut conn, &bundled());
        assert!(status.is_ok());
        let status = status.unwrap_or_else(|_| unreachable!());

        assert_eq!(status.version, 2);
        assert_eq!(status.applied.len(), 2);
        assert_eq!(status.applied[0].id, 1);
        assert_eq!(status.applied[0].name, "0001_parish_profile");
        assert_eq!(status.applied[1].id, 2);
        assert_eq!(status.applied[1].name, "0002_parish_note");
        let bundled = bundled();
        assert_eq!(status.applied[0].checksum, checksum(bundled[0].sql));
        assert_eq!(status.applied[1].checksum, checksum(bundled[1].sql));
        assert_eq!(status.applied[0].checksum.len(), 64);
        assert_eq!(status.journal_mode, "wal");
        assert!(status.foreign_keys);
        assert_eq!(scalar_i64(&conn, "PRAGMA user_version"), 2);
    }

    #[test]
    fn rerunning_at_head_is_a_no_op() {
        let dir = TempDir::new("rerun");
        let mut conn = open(&dir);
        let first = open_and_migrate(&mut conn, &bundled());
        let stamps = |conn: &Connection| {
            let mut statement = conn
                .prepare("SELECT applied_at FROM ordo_migrations ORDER BY id")
                .unwrap_or_else(|_| unreachable!());
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap_or_else(|_| unreachable!())
                .map(|row| row.unwrap_or_default())
                .collect::<Vec<_>>()
        };
        let before = stamps(&conn);
        let second = open_and_migrate(&mut conn, &bundled());
        assert_eq!(first, second);
        assert_eq!(before, stamps(&conn));
    }

    #[test]
    fn failed_migration_rolls_back_schema_data_and_ledger() {
        let dir = TempDir::new("rollback");
        let mut conn = open(&dir);
        assert!(open_and_migrate(&mut conn, &bundled()).is_ok());
        assert!(
            conn.execute(
                "INSERT INTO parish_profile (id, name) VALUES (1, 'Kept')",
                []
            )
            .is_ok()
        );

        let mut broken = bundled();
        broken.push(Migration {
            id: 3,
            name: "0003_broken",
            sql: "CREATE TABLE extra_table (x INTEGER); INSERT INTO missing_table VALUES (1);",
        });
        let result = open_and_migrate(&mut conn, &broken);
        assert!(
            matches!(result, Err(StorageError::MigrationFailed { id: 3, .. })),
            "unexpected result: {result:?}"
        );

        let extra = scalar_i64(
            &conn,
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'extra_table'",
        );
        assert_eq!(extra, 0);
        assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM ordo_migrations"), 2);
        assert_eq!(scalar_i64(&conn, "PRAGMA user_version"), 2);
        assert_eq!(
            scalar_i64(
                &conn,
                "SELECT COUNT(*) FROM parish_profile WHERE name = 'Kept'"
            ),
            1
        );
    }

    #[test]
    fn newer_schema_is_refused_without_mutation() {
        let dir = TempDir::new("newer");
        let mut conn = open(&dir);
        assert!(open_and_migrate(&mut conn, &bundled()).is_ok());
        assert!(
            conn.execute(
                "INSERT INTO ordo_migrations (id, name, checksum, applied_at) \
                 VALUES (999, 'future', 'x', 'now')",
                []
            )
            .is_ok()
        );
        let tables_before = scalar_i64(&conn, "SELECT COUNT(*) FROM sqlite_master");

        let result = open_and_migrate(&mut conn, &bundled());
        assert_eq!(
            result,
            Err(StorageError::NewerSchema {
                found: 999,
                supported: 2
            })
        );
        assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM ordo_migrations"), 3);
        assert_eq!(scalar_i64(&conn, "PRAGMA user_version"), 2);
        assert_eq!(
            scalar_i64(&conn, "SELECT COUNT(*) FROM sqlite_master"),
            tables_before
        );
    }

    #[test]
    fn user_version_above_head_is_refused_without_mutation() {
        let dir = TempDir::new("user-version-newer");
        let mut conn = open(&dir);
        assert!(open_and_migrate(&mut conn, &bundled()).is_ok());
        assert!(conn.pragma_update(None, "user_version", 7).is_ok());
        let tables_before = scalar_i64(&conn, "SELECT COUNT(*) FROM sqlite_master");

        assert_eq!(
            open_and_migrate(&mut conn, &bundled()),
            Err(StorageError::NewerSchema {
                found: 7,
                supported: 2
            })
        );
        assert_eq!(scalar_i64(&conn, "PRAGMA user_version"), 7);
        assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM ordo_migrations"), 2);
        assert_eq!(
            scalar_i64(&conn, "SELECT COUNT(*) FROM sqlite_master"),
            tables_before
        );
    }

    #[test]
    fn non_prefix_ledger_is_refused() {
        let dir = TempDir::new("gap-ledger");
        let mut conn = open(&dir);
        assert!(open_and_migrate(&mut conn, &bundled()).is_ok());
        // Ledger becomes [2] with no row 1: not a prefix of the bundled ids.
        assert!(
            conn.execute("DELETE FROM ordo_migrations WHERE id = 1", [])
                .is_ok()
        );

        assert_eq!(
            open_and_migrate(&mut conn, &bundled()),
            Err(StorageError::UnknownMigrationLedger { id: 2 })
        );
        assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM ordo_migrations"), 1);
    }

    #[test]
    fn bundled_ids_are_strictly_increasing() {
        let ids: Vec<u32> = bundled().iter().map(|migration| migration.id).collect();
        assert!(!ids.is_empty());
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "ids: {ids:?}");
    }

    #[test]
    fn bundled_sql_contains_no_transaction_control() {
        for migration in bundled() {
            let tokens = migration
                .sql
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .map(|token| token.to_ascii_uppercase());
            for token in tokens {
                assert!(
                    !matches!(token.as_str(), "BEGIN" | "COMMIT" | "ROLLBACK"),
                    "migration {} contains transaction control `{token}`",
                    migration.name
                );
            }
        }
    }

    #[test]
    fn checksum_mismatch_is_refused() {
        let dir = TempDir::new("checksum");
        let mut conn = open(&dir);
        assert!(open_and_migrate(&mut conn, &bundled()).is_ok());
        assert!(
            conn.execute(
                "UPDATE ordo_migrations SET checksum = 'tampered' WHERE id = 1",
                []
            )
            .is_ok()
        );
        assert_eq!(
            open_and_migrate(&mut conn, &bundled()),
            Err(StorageError::ChecksumMismatch { id: 1 })
        );
    }
}
