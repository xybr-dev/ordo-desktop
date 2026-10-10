//! Supervision of the Storage worker inside GPUI tests (T0.7 WP3).
//!
//! The worker is a real OS thread, so its replies wake GPUI tasks from a
//! foreign thread. Every storage future is awaited inside a GPUI foreground
//! task owned by a test entity, so the GPUI scheduler observes the wait. Every
//! test uses a fresh temporary database and never touches Parish data.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::future::Future;

use gpui::{AppContext, TestAppContext};
use storage::{ShutdownOutcome, Storage, StorageConfig, StorageError};

/// Upper bound for a single storage round trip before the test fails.
const DEADLINE: Duration = Duration::from_secs(10);

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

/// Test entity that records the result of one awaited storage future.
struct Probe<T> {
    result: Option<T>,
}

/// Awaits `future` inside a GPUI foreground task and returns its output.
///
/// The task is spawned from a test entity, so the GPUI scheduler owns the
/// wait. Worker replies arrive from a foreign thread after the scheduler
/// parks, so the loop re-runs `run_until_parked` after short sleeps until the
/// entity holds a result or the deadline passes.
fn drive<T: 'static>(cx: &mut TestAppContext, future: impl Future<Output = T> + 'static) -> T {
    cx.executor().allow_parking();
    let probe = cx.new(|_| Probe::<T> { result: None });
    probe.update(cx, |_, cx| {
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |probe, _| probe.result = Some(result)).ok();
        })
        .detach();
    });

    let deadline = Instant::now() + DEADLINE;
    loop {
        cx.run_until_parked();
        if let Some(result) = probe.update(cx, |probe, _| probe.result.take()) {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "storage operation did not complete within {DEADLINE:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn spawn_on(path: &Path) -> Storage {
    Storage::spawn(StorageConfig::new(path))
}

#[gpui::test]
fn ready_then_operations_then_clean_shutdown(cx: &mut TestAppContext) {
    let dir = TempDir::new("clean");
    let storage = spawn_on(&dir.db());

    let status = drive(cx, storage.ready());
    assert_eq!(status.map(|s| s.version), Ok(2));

    let count = drive(cx, storage.parish_note_count());
    assert_eq!(count, Ok(0));

    let outcome = drive(cx, storage.shutdown());
    assert_eq!(outcome, ShutdownOutcome::Clean);
}

#[gpui::test]
fn injected_panic_is_reported_then_worker_is_unavailable(cx: &mut TestAppContext) {
    let dir = TempDir::new("panic");
    let storage = spawn_on(&dir.db());
    assert!(drive(cx, storage.ready()).is_ok());

    let panicked = drive(cx, storage.inject_panic());
    assert!(
        matches!(&panicked, Err(StorageError::WorkerPanicked(message)) if message.contains("injected")),
        "unexpected panic result: {panicked:?}"
    );

    // The worker's request channel is closed after the panic.
    let next = drive(cx, storage.parish_name());
    assert_eq!(next, Err(StorageError::WorkerUnavailable));

    let outcome = drive(cx, storage.shutdown());
    assert!(
        matches!(&outcome, ShutdownOutcome::WorkerPanicked(message) if message.contains("injected")),
        "unexpected shutdown outcome: {outcome:?}"
    );
}

#[gpui::test]
fn dropping_without_shutdown_does_not_block_and_releases_the_file(cx: &mut TestAppContext) {
    let dir = TempDir::new("drop");
    let storage = spawn_on(&dir.db());
    assert!(drive(cx, storage.ready()).is_ok());

    // Drop sends shutdown without joining; this must return promptly.
    let started = Instant::now();
    drop(storage);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "dropping Storage blocked for {:?}",
        started.elapsed()
    );

    // A reopen of the same file succeeds once the dropped worker has closed it.
    let reopened = spawn_on(&dir.db());
    let status = drive(cx, reopened.ready());
    assert_eq!(status.map(|s| s.version), Ok(2));
    assert_eq!(drive(cx, reopened.shutdown()), ShutdownOutcome::Clean);
}
