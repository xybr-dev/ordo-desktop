//! Storage Module: asynchronous, domain-shaped local persistence (T0.7).
//!
//! One supervised thread named `ordo-storage` is the sole owner of the SQLite
//! connection. Callers hold a [`Storage`] handle and await typed operations;
//! they never receive a connection, never submit SQL, and never control
//! transactions. Requests are served strictly FIFO from one channel. The
//! bundled `rusqlite` driver stays private to this crate.
//!
//! Open sequence: the migration ledger is inspected before any mutation, and a
//! newer schema or a checksum mismatch is refused. Only then are pragmas set
//! (`foreign_keys=ON`, `journal_mode=WAL`, `synchronous=FULL`) and pending
//! migrations applied, each in its own immediate transaction together with its
//! ledger row and `user_version`. Shutdown checkpoints the WAL with TRUNCATE.
//!
//! The compile-fail examples below are the automated evidence that callers
//! cannot reach the driver, a connection, SQL, or a transaction (T0.7 AC1-2).
//! The baseline compiles:
//!
//! ```no_run
//! let storage = storage::Storage::spawn(storage::StorageConfig::new("parish.db"));
//! let _ready = storage.ready();
//! ```
//!
//! The driver is not re-exported:
//!
//! ```compile_fail
//! use storage::rusqlite::Connection;
//! ```
//!
//! No connection accessor exists:
//!
//! ```compile_fail
//! let storage = storage::Storage::spawn(storage::StorageConfig::new("parish.db"));
//! let _connection = storage.connection();
//! ```
//!
//! No SQL can be submitted:
//!
//! ```compile_fail
//! let storage = storage::Storage::spawn(storage::StorageConfig::new("parish.db"));
//! let _rows = storage.execute("DELETE FROM parish_note");
//! ```
//!
//! No transaction can be opened:
//!
//! ```compile_fail
//! let storage = storage::Storage::spawn(storage::StorageConfig::new("parish.db"));
//! let _transaction = storage.transaction();
//! ```

use std::fmt;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::Duration;

mod migrations;
mod worker;

use migrations::Migration;
use worker::{Core, Exit, Message};

/// Where the database lives and how long a request waits on a locked file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub busy_timeout: Duration,
}

impl StorageConfig {
    /// Configuration with the default 5 second busy timeout.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            busy_timeout: Duration::from_secs(5),
        }
    }
}

/// Snapshot of the migration ledger and connection policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaStatus {
    pub version: u32,
    pub applied: Vec<AppliedMigration>,
    pub journal_mode: String,
    pub foreign_keys: bool,
}

/// One row of the `ordo_migrations` ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub id: u32,
    pub name: String,
    pub checksum: String,
}

/// Result of [`Storage::shutdown`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownOutcome {
    Clean,
    WorkerPanicked(String),
    AlreadyStopped,
}

/// Structured failure from the Storage Module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// The database was migrated by a newer build than this one supports.
    NewerSchema { found: u32, supported: u32 },
    /// An applied migration's recorded checksum differs from the bundled SQL.
    ChecksumMismatch { id: u32 },
    /// The ledger is not a contiguous prefix 1..=k of the bundled migration
    /// ids. `id` is the first ledger row that does not match its position.
    UnknownMigrationLedger { id: u32 },
    /// A migration failed and was rolled back in full.
    MigrationFailed { id: u32, message: String },
    /// An operation ran before a successful open.
    NotReady,
    /// The handler for this request panicked.
    WorkerPanicked(String),
    /// The worker is gone (panicked earlier, shut down, or never started).
    WorkerUnavailable,
    /// SQLITE_BUSY or SQLITE_LOCKED remained after the busy timeout.
    Busy,
    /// Any other driver error, carried as a message only.
    Sqlite(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewerSchema { found, supported } => write!(
                f,
                "database schema version {found} is newer than supported version {supported}"
            ),
            Self::ChecksumMismatch { id } => {
                write!(f, "migration {id} checksum does not match the bundled SQL")
            }
            Self::UnknownMigrationLedger { id } => write!(
                f,
                "migration ledger row {id} does not continue the bundled migration sequence"
            ),
            Self::MigrationFailed { id, message } => {
                write!(f, "migration {id} failed and was rolled back: {message}")
            }
            Self::NotReady => write!(f, "storage is not open"),
            Self::WorkerPanicked(message) => write!(f, "storage worker panicked: {message}"),
            Self::WorkerUnavailable => write!(f, "storage worker is unavailable"),
            Self::Busy => write!(f, "database is busy"),
            Self::Sqlite(message) => write!(f, "sqlite error: {message}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Handle to the storage worker. Not `Clone`: exactly one owner.
///
/// The worker thread is detached at spawn; the handle keeps only the request
/// sender and the receiving end of the oneshot exit-outcome channel.
pub struct Storage {
    tx: Option<async_channel::Sender<Message>>,
    exit: Option<async_channel::Receiver<Exit>>,
}

impl Storage {
    /// Spawns the `ordo-storage` thread and returns immediately. Open and
    /// migrate run on the worker; the result arrives through [`Storage::ready`].
    pub fn spawn(config: StorageConfig) -> Storage {
        spawn_with(config, migrations::bundled())
    }

    /// Resolves once the open sequence has finished, with the schema status or
    /// the reason the database was refused.
    pub fn ready(&self) -> impl Future<Output = Result<SchemaStatus, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.ready())
    }

    pub fn schema_status(
        &self,
    ) -> impl Future<Output = Result<SchemaStatus, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.schema_status())
    }

    pub fn set_parish_name(
        &self,
        name: String,
    ) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.set_parish_name(&name)
        })
    }

    pub fn parish_name(
        &self,
    ) -> impl Future<Output = Result<Option<String>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.parish_name())
    }

    pub fn add_parish_note(
        &self,
        body: String,
    ) -> impl Future<Output = Result<i64, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.add_parish_note(&body)
        })
    }

    pub fn parish_note_count(&self) -> impl Future<Output = Result<u64, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.parish_note_count())
    }

    /// Checkpoints the WAL (TRUNCATE), closes the connection, and reports how
    /// the worker thread ended.
    ///
    /// This future never blocks: it sends the shutdown message without waiting
    /// and then awaits the worker's exit outcome, so it is safe to poll on the
    /// GPUI thread. The thread itself is detached, not joined.
    ///
    /// Outcomes: `Clean` after an orderly checkpoint; `WorkerPanicked(msg)` if
    /// the worker panicked (before or during shutdown); `AlreadyStopped` if the
    /// thread is gone without recording an outcome, or never started.
    pub fn shutdown(mut self) -> impl Future<Output = ShutdownOutcome> + use<> {
        let tx = self.tx.take();
        let exit = self.exit.take();
        async move {
            let Some(exit) = exit else {
                return ShutdownOutcome::AlreadyStopped;
            };
            if let Some(tx) = tx {
                // If the worker already exited, the send fails and the exit
                // outcome (if any) is already waiting in the oneshot channel.
                let _ = tx.try_send(Message::Shutdown);
            }
            match exit.recv().await {
                Ok(Exit::Clean) => ShutdownOutcome::Clean,
                Ok(Exit::Panicked(message)) => ShutdownOutcome::WorkerPanicked(message),
                Err(_) => ShutdownOutcome::AlreadyStopped,
            }
        }
    }
}

impl Drop for Storage {
    /// Requests shutdown and returns immediately. The worker thread is
    /// detached and finishes its work (including the checkpoint) independently.
    /// Callers that want an orderly WAL checkpoint must await [`Storage::shutdown`].
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.try_send(Message::Shutdown);
        }
    }
}

fn spawn_with(config: StorageConfig, migrations: Vec<Migration>) -> Storage {
    let (tx, rx) = async_channel::unbounded();
    let (exit_tx, exit_rx) = async_channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name("ordo-storage".to_owned())
        .spawn(move || {
            // Any unwind that escapes `worker::run` is still recorded, so the
            // outcome channel always delivers WorkerPanicked for a panic.
            let exit = match catch_unwind(AssertUnwindSafe(|| worker::run(config, migrations, rx)))
            {
                Ok(exit) => exit,
                Err(payload) => Exit::Panicked(worker::panic_message(payload.as_ref())),
            };
            let _ = exit_tx.try_send(exit);
        });
    match spawned {
        // Dropping the JoinHandle detaches the thread.
        Ok(_detached) => Storage {
            tx: Some(tx),
            exit: Some(exit_rx),
        },
        // Spawn failed: every operation reports WorkerUnavailable.
        Err(_) => Storage {
            tx: None,
            exit: None,
        },
    }
}

#[cfg(feature = "fault-injection")]
impl Storage {
    /// Spawns with one extra migration appended after the bundled head.
    pub fn spawn_with_extra_migration(
        config: StorageConfig,
        id: u32,
        name: &'static str,
        sql: &'static str,
    ) -> Storage {
        let mut migrations = migrations::bundled();
        migrations.push(Migration { id, name, sql });
        spawn_with(config, migrations)
    }

    pub fn inject_panic(&self) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(
            self.tx.as_ref(),
            |_core: &mut Core| -> Result<(), StorageError> {
                panic!("injected storage worker panic")
            },
        )
    }

    pub fn inject_delay(
        &self,
        delay: Duration,
    ) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |_core: &mut Core| {
            std::thread::sleep(delay);
            Ok::<(), StorageError>(())
        })
    }
}

/// Maps a driver error to the public taxonomy, keeping `rusqlite` private.
pub(crate) fn map_error(error: rusqlite::Error) -> StorageError {
    if is_busy(&error) {
        StorageError::Busy
    } else {
        StorageError::Sqlite(error.to_string())
    }
}

pub(crate) fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

#[cfg(test)]
pub(crate) mod testing {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique temporary directory under `std::env::temp_dir()`, removed on drop.
    pub(crate) struct TempDir(PathBuf);

    impl TempDir {
        pub(crate) fn new(tag: &str) -> TempDir {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!(
                "ordo-storage-{tag}-{}-{}-{nanos}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let created = std::fs::create_dir_all(&path);
            assert!(created.is_ok(), "create temp dir {}", path.display());
            TempDir(path)
        }

        pub(crate) fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(all(test, feature = "fault-injection"))]
mod panic_drain_tests {
    use super::*;
    use crate::testing::TempDir;
    use std::sync::mpsc;

    /// Resolves a storage future on its own thread, failing instead of hanging.
    fn resolve<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> T {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(futures_lite::future::block_on(future));
        });
        rx.recv_timeout(Duration::from_secs(10))
            .expect("storage future did not resolve")
    }

    #[test]
    fn requests_queued_behind_a_panic_resolve_as_worker_unavailable() {
        let dir = TempDir::new("queued-panic");
        let storage = Storage::spawn(StorageConfig::new(dir.path().join("parish.sqlite3")));
        assert!(resolve(storage.ready()).is_ok());

        // Hold the worker inside a job until every request below is queued, so
        // the panic and the requests behind it are all in the channel together.
        let (release, gate) = mpsc::channel::<()>();
        let hold = worker::submit(storage.tx.as_ref(), move |_core: &mut Core| {
            let _ = gate.recv();
            Ok::<(), StorageError>(())
        });
        let panicked = storage.inject_panic();
        let note_count = storage.parish_note_count();
        let name = storage.parish_name();
        let status = storage.schema_status();
        let note = storage.add_parish_note("queued".to_owned());

        release.send(()).expect("worker is holding the gate");
        assert_eq!(resolve(hold), Ok(()));
        assert!(
            matches!(resolve(panicked), Err(StorageError::WorkerPanicked(message))
                if message.contains("injected storage worker panic"))
        );
        assert_eq!(resolve(note_count), Err(StorageError::WorkerUnavailable));
        assert_eq!(resolve(name), Err(StorageError::WorkerUnavailable));
        assert_eq!(
            resolve(status).map(|_| ()),
            Err(StorageError::WorkerUnavailable)
        );
        assert_eq!(resolve(note), Err(StorageError::WorkerUnavailable));

        // A request sent after the exit fails fast, and shutdown reports the panic.
        assert_eq!(
            resolve(storage.parish_note_count()),
            Err(StorageError::WorkerUnavailable)
        );
        assert!(matches!(
            resolve(storage.shutdown()),
            ShutdownOutcome::WorkerPanicked(_)
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    #[test]
    fn worker_persists_domain_data_across_reopen() {
        let dir = TempDir::new("reopen");
        let db = dir.path().join("parish.sqlite3");

        let storage = Storage::spawn(StorageConfig::new(&db));
        let status = futures_lite::future::block_on(storage.ready());
        assert_eq!(status.map(|s| s.version), Ok(2));
        let set = futures_lite::future::block_on(storage.set_parish_name("St Mary".to_owned()));
        assert_eq!(set, Ok(()));
        let note = futures_lite::future::block_on(storage.add_parish_note("hi".to_owned()));
        assert!(note.is_ok());
        let count = futures_lite::future::block_on(storage.parish_note_count());
        assert_eq!(count, Ok(1));
        assert_eq!(
            futures_lite::future::block_on(storage.shutdown()),
            ShutdownOutcome::Clean
        );

        let reopened = Storage::spawn(StorageConfig::new(&db));
        let name = futures_lite::future::block_on(reopened.parish_name());
        assert_eq!(name, Ok(Some("St Mary".to_owned())));
        let count = futures_lite::future::block_on(reopened.parish_note_count());
        assert_eq!(count, Ok(1));
        assert_eq!(
            futures_lite::future::block_on(reopened.shutdown()),
            ShutdownOutcome::Clean
        );
    }
}
