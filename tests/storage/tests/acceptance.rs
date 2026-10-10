//! T0.7 acceptance harness. Each test names the AC bullet or E2E step it proves.
//! Every await runs under a 10 s bound so a hang fails the test instead of
//! blocking the run. Databases live in unique temporary directories.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_lite::future::block_on;
use rusqlite::Connection;
use storage::{ShutdownOutcome, Storage, StorageConfig, StorageError};

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
            "ordo-storage-tests-{tag}-{}-{}-{nanos}",
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
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config(db: &Path) -> StorageConfig {
    StorageConfig::new(db)
}

fn raw(db: &Path) -> Connection {
    Connection::open(db).expect("raw open")
}

type LedgerRow = (i64, String, String, String);

fn ledger_rows(conn: &Connection) -> Vec<LedgerRow> {
    let mut statement = conn
        .prepare("SELECT id, name, checksum, applied_at FROM ordo_migrations ORDER BY id")
        .expect("prepare ledger");
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query ledger")
        .map(|row| row.expect("ledger row"))
        .collect()
}

fn user_version(conn: &Connection) -> i64 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version")
}

/// Bytes of the database file and its WAL sidecar, when present.
fn file_snapshot(db: &Path) -> Vec<(String, Vec<u8>)> {
    ["", "-wal"]
        .into_iter()
        .filter_map(|suffix| {
            let path = PathBuf::from(format!("{}{suffix}", db.display()));
            path.exists()
                .then(|| (suffix.to_owned(), fs::read(&path).expect("read db file")))
        })
        .collect()
}

fn is_hex_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

// --- AC: domain-shaped operations, fresh open, ledger -----------------------

#[test]
fn fresh_database_migrates_to_head_with_ordered_checksummed_ledger() {
    let dir = TempDir::new("fresh");
    let storage = Storage::spawn(config(&dir.db()));

    let status = bounded(storage.ready()).expect("ready");
    assert_eq!(status.version, 2);
    let ids: Vec<u32> = status.applied.iter().map(|m| m.id).collect();
    assert_eq!(ids, vec![1, 2]);
    for applied in &status.applied {
        assert!(
            !applied.name.is_empty(),
            "migration {} has a name",
            applied.id
        );
        assert!(
            is_hex_sha256(&applied.checksum),
            "migration {} checksum is a SHA-256 hex digest",
            applied.id
        );
    }
    assert_ne!(status.applied[0].checksum, status.applied[1].checksum);

    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    let conn = raw(&dir.db());
    let rows = ledger_rows(&conn);
    assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), vec![1, 2]);
    assert!(rows.iter().all(|r| is_hex_sha256(&r.2)));
    assert_eq!(user_version(&conn), 2);
}

// --- AC: re-running at head is a no-op ---------------------------------------

#[test]
fn respawn_at_head_is_a_noop_and_keeps_data() {
    let dir = TempDir::new("noop");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("first ready");
    bounded(storage.set_parish_name("St Mary".to_owned())).expect("set name");
    bounded(storage.add_parish_note("note".to_owned())).expect("add note");
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    let before = ledger_rows(&raw(&dir.db()));

    let storage = Storage::spawn(config(&dir.db()));
    let status = bounded(storage.ready()).expect("second ready");
    assert_eq!(status.version, 2);
    assert_eq!(
        bounded(storage.parish_name()),
        Ok(Some("St Mary".to_owned()))
    );
    assert_eq!(bounded(storage.parish_note_count()), Ok(1));
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    assert_eq!(
        ledger_rows(&raw(&dir.db())),
        before,
        "ledger unchanged by re-run"
    );
}

// --- E2E and AC: failed migration rolls back schema, data, and history -------

#[test]
fn broken_next_migration_leaves_schema_data_and_ledger_at_last_valid_version() {
    let dir = TempDir::new("e2e");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready at head");
    bounded(storage.set_parish_name("St Mary".to_owned())).expect("set name");
    bounded(storage.add_parish_note("kept".to_owned())).expect("add note");
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    let broken = Storage::spawn_with_extra_migration(
        config(&dir.db()),
        3,
        "broken_probe",
        "CREATE TABLE extra_probe (id INTEGER PRIMARY KEY); \
         SELECT * FROM table_that_does_not_exist;",
    );
    let outcome = bounded(broken.ready());
    assert!(
        matches!(outcome, Err(StorageError::MigrationFailed { id: 3, .. })),
        "expected MigrationFailed {{ id: 3 }}, got {outcome:?}"
    );
    assert_eq!(bounded(broken.shutdown()), ShutdownOutcome::Clean);

    let conn = raw(&dir.db());
    let probe_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'extra_probe'",
            [],
            |row| row.get(0),
        )
        .expect("probe count");
    assert_eq!(
        probe_tables, 0,
        "schema change from the failed migration rolled back"
    );

    let name: String = conn
        .query_row("SELECT name FROM parish_profile WHERE id = 1", [], |row| {
            row.get(0)
        })
        .expect("profile survives");
    assert_eq!(name, "St Mary");
    let notes: i64 = conn
        .query_row("SELECT COUNT(*) FROM parish_note", [], |row| row.get(0))
        .expect("notes survive");
    assert_eq!(notes, 1);

    let ids: Vec<i64> = ledger_rows(&conn).iter().map(|r| r.0).collect();
    assert_eq!(
        ids,
        vec![1, 2],
        "ledger still ends at the last valid migration"
    );
    assert_eq!(user_version(&conn), 2);
}

// --- AC: newer schema refused without mutation -------------------------------

#[test]
fn newer_schema_is_refused_without_mutating_the_file() {
    let dir = TempDir::new("newer");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    {
        let conn = raw(&dir.db());
        conn.execute(
            "INSERT INTO ordo_migrations (id, name, checksum, applied_at) \
             VALUES (999, 'future_probe', 'unknown', '2030-01-01T00:00:00Z')",
            [],
        )
        .expect("plant newer ledger row");
        conn.pragma_update(None, "user_version", 999)
            .expect("plant user_version");
    }
    let before = file_snapshot(&dir.db());

    let storage = Storage::spawn(config(&dir.db()));
    let outcome = bounded(storage.ready());
    assert_eq!(
        outcome,
        Err(StorageError::NewerSchema {
            found: 999,
            supported: 2
        })
    );
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);

    assert_eq!(
        file_snapshot(&dir.db()),
        before,
        "db and -wal bytes unchanged"
    );
}

// --- AC: foreign keys, WAL, and explicit connection policy -------------------

#[test]
fn foreign_keys_are_enforced_and_wal_mode_is_reported() {
    let dir = TempDir::new("policy");
    let storage = Storage::spawn(config(&dir.db()));
    let status = bounded(storage.ready()).expect("ready");
    assert!(status.foreign_keys, "foreign_keys reported on");
    assert_eq!(status.journal_mode, "wal");

    // parish_note references parish_profile, which is empty on a fresh file.
    let orphan = bounded(storage.add_parish_note("orphan".to_owned()));
    match orphan {
        Err(StorageError::Sqlite(message)) => {
            assert!(message.contains("FOREIGN KEY"), "got {message}")
        }
        other => panic!("expected FK rejection, got {other:?}"),
    }
    assert_eq!(bounded(storage.parish_note_count()), Ok(0));

    bounded(storage.set_parish_name("St Mary".to_owned())).expect("profile");
    assert!(bounded(storage.add_parish_note("ok".to_owned())).is_ok());
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

// --- AC: busy handling is explicit and bounded -------------------------------

#[test]
fn held_exclusive_lock_yields_busy_after_the_timeout_not_a_hang() {
    let dir = TempDir::new("busy");
    let storage = Storage::spawn(StorageConfig {
        busy_timeout: Duration::from_millis(200),
        ..config(&dir.db())
    });
    bounded(storage.ready()).expect("ready");

    let holder = raw(&dir.db());
    holder.execute_batch("BEGIN EXCLUSIVE;").expect("hold lock");

    let started = Instant::now();
    let outcome = bounded(storage.set_parish_name("blocked".to_owned()));
    let waited = started.elapsed();
    assert_eq!(outcome, Err(StorageError::Busy));
    assert!(
        waited >= Duration::from_millis(150),
        "waited for busy timeout: {waited:?}"
    );
    assert!(waited < BOUND, "did not hang: {waited:?}");

    holder.execute_batch("COMMIT;").expect("release lock");
    assert_eq!(bounded(storage.set_parish_name("free".to_owned())), Ok(()));
    assert_eq!(bounded(storage.parish_name()), Ok(Some("free".to_owned())));
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

// --- AC: concurrent typed requests are serialized deterministically -----------

#[test]
fn eight_threads_of_requests_produce_an_exact_serialized_count() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 50;

    let dir = TempDir::new("concurrent");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");
    bounded(storage.set_parish_name("St Mary".to_owned())).expect("profile");

    let storage = Arc::new(storage);
    let receivers: Vec<_> = (0..THREADS)
        .map(|thread_index| {
            let storage = Arc::clone(&storage);
            spawn_bounded(move || {
                block_on(async move {
                    let mut ids = Vec::with_capacity(PER_THREAD);
                    for note in 0..PER_THREAD {
                        let id = storage
                            .add_parish_note(format!("thread {thread_index} note {note}"))
                            .await
                            .expect("add note");
                        ids.push(id);
                    }
                    ids
                })
            })
        })
        .collect();

    let mut ids = Vec::new();
    for receiver in receivers {
        ids.extend(
            receiver
                .recv_timeout(BOUND)
                .expect("thread finished within 10 s"),
        );
    }
    assert_eq!(ids.len(), THREADS * PER_THREAD);
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        THREADS * PER_THREAD,
        "every insert got a distinct row id"
    );

    let storage = Arc::try_unwrap(storage)
        .ok()
        .expect("all threads released the handle");
    assert_eq!(
        bounded(storage.parish_note_count()),
        Ok((THREADS * PER_THREAD) as u64)
    );
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

// --- AC: submission never blocks behind slow work ----------------------------

#[test]
fn submitting_during_a_slow_operation_returns_immediately() {
    let dir = TempDir::new("nonblocking");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");

    let delay = Duration::from_millis(1500);
    let slow = storage.inject_delay(delay);

    let submitted = Instant::now();
    let count = storage.parish_note_count();
    let submit_cost = submitted.elapsed();
    assert!(
        submit_cost < Duration::from_millis(500),
        "submission took {submit_cost:?} while a {delay:?} operation was in flight"
    );

    assert_eq!(bounded(slow), Ok(()));
    let completed_after = submitted.elapsed();
    assert_eq!(bounded(count), Ok(0));
    assert!(
        completed_after >= Duration::from_millis(1200),
        "queued request was served after the slow one (FIFO): {completed_after:?}"
    );
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

// --- AC: panic, closed channel, and shutdown are structured results ----------

#[test]
fn worker_panic_yields_structured_errors_then_unavailable_then_panicked_shutdown() {
    let dir = TempDir::new("panic");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");

    match bounded(storage.inject_panic()) {
        Err(StorageError::WorkerPanicked(message)) => {
            assert!(
                message.contains("injected storage worker panic"),
                "got {message}"
            )
        }
        other => panic!("expected WorkerPanicked, got {other:?}"),
    }
    // The worker closes its request channel before it sends the panic reply, so
    // this request is refused immediately rather than queued behind a dead worker.
    assert_eq!(
        bounded(storage.parish_note_count()),
        Err(StorageError::WorkerUnavailable)
    );

    match bounded(storage.shutdown()) {
        ShutdownOutcome::WorkerPanicked(message) => {
            assert!(
                message.contains("injected storage worker panic"),
                "got {message}"
            )
        }
        other => panic!("expected WorkerPanicked shutdown, got {other:?}"),
    }
}

#[test]
fn clean_shutdown_reports_clean() {
    let dir = TempDir::new("clean");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

#[test]
fn requests_queued_before_shutdown_resolve_and_do_not_hang() {
    let dir = TempDir::new("queued-shutdown");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");

    let queued = storage.parish_note_count();
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
    assert_eq!(bounded(queued), Ok(0));
}

#[test]
fn dropping_the_handle_never_hangs_and_releases_the_database() {
    let dir = TempDir::new("drop");
    let storage = Storage::spawn(config(&dir.db()));
    bounded(storage.ready()).expect("ready");
    bounded(storage.set_parish_name("St Mary".to_owned())).expect("profile");

    let queued = storage.parish_note_count();
    let dropped_at = Instant::now();
    drop(storage);
    assert!(
        dropped_at.elapsed() < Duration::from_millis(500),
        "drop does not block"
    );
    assert_eq!(
        bounded(queued),
        Ok(0),
        "request queued before drop still resolves"
    );

    // Once the worker has exited, the file is free for a fresh owner.
    let reopened = Storage::spawn(config(&dir.db()));
    assert_eq!(
        bounded(reopened.parish_name()),
        Ok(Some("St Mary".to_owned()))
    );
    assert_eq!(bounded(reopened.shutdown()), ShutdownOutcome::Clean);
}
