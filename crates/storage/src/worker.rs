//! The single thread that owns the SQLite connection.
//!
//! Requests arrive on one unbounded FIFO channel and are served one at a time.
//! Each handler runs inside `catch_unwind`. After a handler panics, the worker
//! replies `WorkerPanicked` to that request, drops the connection, and exits.

use std::any::Any;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};

use async_channel::{Receiver, Sender};
use rusqlite::{Connection, OptionalExtension, params};

use crate::migrations::{self, Migration};
use crate::{SchemaStatus, StorageConfig, StorageError, map_error};

/// Work sent to the worker.
pub(crate) enum Message {
    Job(Job),
    /// Checkpoint, close, reply, exit.
    Shutdown(Sender<()>),
}

/// A typed operation, type-erased so the request channel has one item type.
/// `run` sends the typed result to its caller; `on_panic` sends
/// `WorkerPanicked` if `run` unwinds.
pub(crate) struct Job {
    run: Box<dyn FnOnce(&mut Core) + Send>,
    on_panic: Box<dyn FnOnce(String) + Send>,
}

/// How the worker thread finished.
pub(crate) enum Exit {
    Clean,
    Panicked(String),
}

/// Worker-owned state. Never leaves the worker thread.
pub(crate) struct Core {
    conn: Option<Connection>,
    status: Result<SchemaStatus, StorageError>,
}

/// Queues `op` and returns a future for its result. Never blocks.
pub(crate) fn submit<T, F>(
    sender: Option<&Sender<Message>>,
    op: F,
) -> impl Future<Output = Result<T, StorageError>> + use<T, F>
where
    T: Send + 'static,
    F: FnOnce(&mut Core) -> Result<T, StorageError> + Send + 'static,
{
    let (reply, outcome) = async_channel::bounded(1);
    let panic_reply = reply.clone();
    let job = Job {
        run: Box::new(move |core: &mut Core| {
            let result = op(core);
            let _ = reply.try_send(result);
        }),
        on_panic: Box::new(move |message: String| {
            let _ = panic_reply.try_send(Err(StorageError::WorkerPanicked(message)));
        }),
    };
    let accepted = sender.is_some_and(|sender| sender.try_send(Message::Job(job)).is_ok());
    async move {
        if !accepted {
            return Err(StorageError::WorkerUnavailable);
        }
        outcome
            .recv()
            .await
            .unwrap_or(Err(StorageError::WorkerUnavailable))
    }
}

/// Thread body. Opens and migrates, then serves requests until shutdown or
/// until a handler panics.
pub(crate) fn run(
    config: StorageConfig,
    migrations: Vec<Migration>,
    requests: Receiver<Message>,
) -> Exit {
    let mut core = Core::open(&config, &migrations);
    while let Ok(message) = requests.recv_blocking() {
        match message {
            Message::Job(Job { run, on_panic }) => {
                let outcome = catch_unwind(AssertUnwindSafe(|| run(&mut core)));
                if let Err(payload) = outcome {
                    let message = panic_message(&*payload);
                    on_panic(message.clone());
                    return Exit::Panicked(message);
                }
            }
            Message::Shutdown(reply) => {
                core.checkpoint();
                drop(core);
                let _ = reply.try_send(());
                return Exit::Clean;
            }
        }
    }
    Exit::Clean
}

pub(crate) fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

impl Core {
    fn open(config: &StorageConfig, migrations: &[Migration]) -> Core {
        let mut conn = match Connection::open(&config.path) {
            Ok(conn) => conn,
            Err(error) => {
                return Core {
                    conn: None,
                    status: Err(map_error(error)),
                };
            }
        };
        let status = conn
            .busy_timeout(config.busy_timeout)
            .map_err(map_error)
            .and_then(|()| migrations::open_and_migrate(&mut conn, migrations));
        match status {
            Ok(status) => Core {
                conn: Some(conn),
                status: Ok(status),
            },
            Err(error) => Core {
                conn: None,
                status: Err(error),
            },
        }
    }

    fn conn(&self) -> Result<&Connection, StorageError> {
        self.conn.as_ref().ok_or(StorageError::NotReady)
    }

    /// Checkpoint failure is not data loss: the WAL is replayed on next open.
    fn checkpoint(&self) {
        if let Some(conn) = &self.conn {
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
                .ok();
        }
    }

    pub(crate) fn ready(&self) -> Result<SchemaStatus, StorageError> {
        self.status.clone()
    }

    pub(crate) fn schema_status(&self) -> Result<SchemaStatus, StorageError> {
        migrations::status(self.conn()?)
    }

    pub(crate) fn set_parish_name(&self, name: &str) -> Result<(), StorageError> {
        self.conn()?
            .execute(
                "INSERT INTO parish_profile (id, name) VALUES (1, ?1) \
                 ON CONFLICT (id) DO UPDATE SET name = excluded.name",
                params![name],
            )
            .map(|_| ())
            .map_err(map_error)
    }

    pub(crate) fn parish_name(&self) -> Result<Option<String>, StorageError> {
        self.conn()?
            .query_row("SELECT name FROM parish_profile WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(map_error)
    }

    pub(crate) fn add_parish_note(&self, body: &str) -> Result<i64, StorageError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO parish_note (profile_id, body) VALUES (1, ?1)",
            params![body],
        )
        .map_err(map_error)?;
        Ok(conn.last_insert_rowid())
    }

    pub(crate) fn parish_note_count(&self) -> Result<u64, StorageError> {
        let count: i64 = self
            .conn()?
            .query_row("SELECT COUNT(*) FROM parish_note", [], |row| row.get(0))
            .map_err(map_error)?;
        Ok(u64::try_from(count).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_message_reads_str_and_string_payloads() {
        let from_str: Box<dyn Any + Send> = Box::new("static");
        let from_string: Box<dyn Any + Send> = Box::new(String::from("owned"));
        let opaque: Box<dyn Any + Send> = Box::new(7_u8);
        assert_eq!(panic_message(&*from_str), "static");
        assert_eq!(panic_message(&*from_string), "owned");
        assert_eq!(panic_message(&*opaque), "unknown panic payload");
    }

    #[test]
    fn submit_reports_unavailable_without_a_sender() {
        let result = futures_lite::future::block_on(submit(None, |_core: &mut Core| {
            Ok::<(), StorageError>(())
        }));
        assert_eq!(result, Err(StorageError::WorkerUnavailable));
    }

    #[test]
    fn duration_default_is_five_seconds() {
        assert_eq!(
            StorageConfig::new("x").busy_timeout,
            std::time::Duration::from_secs(5)
        );
    }
}
