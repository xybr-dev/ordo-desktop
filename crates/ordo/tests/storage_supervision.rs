//! Supervision of the Storage worker inside GPUI tests (T0.7 WP3).
//!
//! The worker is a real OS thread, so its replies wake the awaiting test from a
//! foreign thread. Every test uses a fresh temporary database and never
//! touches Parish data.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use gpui::TestAppContext;
use storage::{ShutdownOutcome, Storage, StorageConfig, StorageError};

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ordo-storage-supervision-{tag}-{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    fn db(&self) -> PathBuf {
        self.0.join("parish.sqlite3")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Drives `future` to completion on this thread, parking until the storage
/// worker's reply wakes it. A stall fails the test after 30 seconds instead of
/// hanging the harness.
///
/// The GPUI test scheduler parks with its own 15 second guard and did not
/// observe the worker's panic reply, so these futures are driven by a plain
/// thread waker. The tests still run inside `#[gpui::test]`.
fn run<R>(future: impl Future<Output = R>) -> R {
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut task_cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut task_cx) {
            return output;
        }
        let now = Instant::now();
        assert!(
            now < deadline,
            "storage operation did not complete within 30 seconds"
        );
        std::thread::park_timeout(deadline - now);
    }
}

fn spawn_on(path: &Path) -> Storage {
    Storage::spawn(StorageConfig::new(path))
}

#[gpui::test]
fn ready_then_operations_then_clean_shutdown(_cx: &mut TestAppContext) {
    let dir = TempDir::new("clean");
    let storage = spawn_on(&dir.db());

    let status = run(storage.ready());
    assert_eq!(status.map(|s| s.version), Ok(2));

    let count = run(storage.parish_note_count());
    assert_eq!(count, Ok(0));

    let outcome = run(storage.shutdown());
    assert_eq!(outcome, ShutdownOutcome::Clean);
}

#[gpui::test]
fn injected_panic_is_reported_then_worker_is_unavailable(_cx: &mut TestAppContext) {
    let dir = TempDir::new("panic");
    let storage = spawn_on(&dir.db());
    assert!(run(storage.ready()).is_ok());

    let panicked = run(storage.inject_panic());
    assert!(
        matches!(&panicked, Err(StorageError::WorkerPanicked(message)) if message.contains("injected")),
        "unexpected panic result: {panicked:?}"
    );

    let next = run(storage.parish_name());
    assert_eq!(next, Err(StorageError::WorkerUnavailable));

    let outcome = run(storage.shutdown());
    assert!(
        matches!(&outcome, ShutdownOutcome::WorkerPanicked(message) if message.contains("injected")),
        "unexpected shutdown outcome: {outcome:?}"
    );
}

#[gpui::test]
fn dropping_without_shutdown_does_not_block_and_releases_the_file(_cx: &mut TestAppContext) {
    let dir = TempDir::new("drop");
    let storage = spawn_on(&dir.db());
    assert!(run(storage.ready()).is_ok());
    // Drop sends shutdown without joining; this must return promptly.
    drop(storage);

    // A reopen of the same file succeeds once the dropped worker has closed it.
    let reopened = spawn_on(&dir.db());
    let status = run(reopened.ready());
    assert_eq!(status.map(|s| s.version), Ok(2));
    assert_eq!(run(reopened.shutdown()), ShutdownOutcome::Clean);
}
