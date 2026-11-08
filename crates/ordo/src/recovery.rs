//! The recovery journal's writer: an owned worker that turns the Presenter's
//! recovery points into one small file, off the GPUI thread (T1.7b, AC2).
//!
//! The GPUI side holds a [`JournalWriter`] whose calls are an unbounded
//! `mpsc` send and return at once. The worker is the only thing that touches
//! the file, so the Presenter and the hook never see a path, a handle or a
//! clock.
//!
//! # Ordering and coalescing
//!
//! Commands reach the worker in the order they were sent. After each blocking
//! receive the worker drains what is already queued and handles it as a batch:
//! a run of records collapses to the last one (a burst of Next presses is one
//! write), an end is never dropped and is handled in order, and a record queued
//! after an end belongs to a later session. On top of that the worker refuses
//! to write a point whose `(session_id, seq)` is not newer than the last one it
//! wrote for the same session, so a late point can never replace a newer one.
//!
//! # The file
//!
//! Each write goes to `session-recovery.json.tmp`, is synced, then renamed over
//! `session-recovery.json`, so a reader sees the old complete file or the new
//! complete file, never a torn one. A failed write leaves the previous file as
//! it was. A clean end writes its `ended` point and then deletes the file.

use std::{
    ffi::OsString,
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use platform::{
    display::DisplayEvidence,
    tasks::{CancellationToken, OwnedWorker, WorkerExit, WorkerKind, WorkerStartError},
};
use presentation::journal::{BoundsRecord, DisplayRecord, RecoveryPoint, SessionId, encode};

use crate::performance::hot_path::{HotPathKind, note};

/// File name of the journal, beside the installation preferences file.
pub const JOURNAL_FILE: &str = "session-recovery.json";

/// Suffix of the file a write is staged in before it replaces the journal.
const TEMP_SUFFIX: &str = ".tmp";

/// How long to wait before the one retry of a refused rename. On Windows an
/// antivirus scan or the indexer can hold the target for a moment.
const RENAME_RETRY_DELAY: Duration = Duration::from_millis(50);

/// The journal beside the installation preferences file.
pub fn journal_path(preferences_path: &Path) -> PathBuf {
    preferences_path.with_file_name(JOURNAL_FILE)
}

/// A fresh session id (UUID v4, lowercase hyphenated).
pub fn new_session_id() -> SessionId {
    let text = uuid::Uuid::new_v4().hyphenated().to_string();
    SessionId::parse(&text).expect("a freshly generated UUID v4 is a valid session id")
}

/// Platform display evidence as the journal's record of it.
pub fn display_record(evidence: &DisplayEvidence) -> DisplayRecord {
    DisplayRecord {
        name: evidence.name.clone(),
        uuid: evidence.uuid.clone(),
        bounds: BoundsRecord {
            x: evidence.bounds.x,
            y: evidence.bounds.y,
            width: evidence.bounds.width,
            height: evidence.bounds.height,
        },
    }
}

/// Why one journal write or delete failed. Reported, never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalFailure {
    Write(io::ErrorKind),
    Delete(io::ErrorKind),
}

enum JournalCommand {
    Record(RecoveryPoint),
    End(RecoveryPoint),
    Shutdown,
}

/// Cheap, cloneable handle the GPUI thread uses. Both calls are a
/// non-blocking `Sender::send` and return at once; if the worker is gone the
/// point is dropped, because recovery must never stall or fail the Presenter.
#[derive(Clone)]
pub struct JournalWriter {
    commands: Sender<JournalCommand>,
}

impl JournalWriter {
    /// Queue `point` to replace the journal. A later point supersedes it if
    /// the worker has not written it yet.
    pub fn record(&self, point: RecoveryPoint) {
        let _ = self.commands.send(JournalCommand::Record(point));
    }

    /// Clean end: writes `point` (which has `ended: true`) and then deletes
    /// the file, after every earlier command.
    pub fn end_session(&self, point: RecoveryPoint) {
        let _ = self.commands.send(JournalCommand::End(point));
    }
}

/// Unblocks the worker for shutdown. The worker waits in `recv`, which
/// cancelling and unparking cannot wake while a [`JournalWriter`] is alive, so
/// shutdown sends this instead (as `PreferenceCommand::Shutdown` does).
pub struct JournalShutdown {
    commands: Sender<JournalCommand>,
}

impl JournalShutdown {
    /// Ask the worker to write what is queued and stop.
    pub fn send(&self) {
        let _ = self.commands.send(JournalCommand::Shutdown);
    }
}

/// Spawn the writer as an owned [`WorkerKind::Journal`] worker. `report` runs
/// on the worker for each failure.
pub fn spawn_journal_writer(
    path: PathBuf,
    report: impl FnMut(JournalFailure) + Send + 'static,
) -> Result<(JournalWriter, OwnedWorker<()>, JournalShutdown), WorkerStartError> {
    let (commands, received) = mpsc::channel();
    let worker = OwnedWorker::spawn(WorkerKind::Journal, move |cancel| {
        serve_journal(received, path, report);
        Ok(exit_of(&cancel))
    })?;
    Ok((
        JournalWriter {
            commands: commands.clone(),
        },
        worker,
        JournalShutdown { commands },
    ))
}

fn exit_of(cancel: &CancellationToken) -> WorkerExit<()> {
    if cancel.is_cancelled() {
        WorkerExit::Cancelled
    } else {
        WorkerExit::Completed(())
    }
}

/// The worker loop. Returns how many points it wrote, which the tests count.
fn serve_journal(
    commands: Receiver<JournalCommand>,
    path: PathBuf,
    mut report: impl FnMut(JournalFailure),
) -> usize {
    let mut file = JournalFile::new(path);
    // `recv` ends when every sender is gone or Shutdown arrives.
    while let Ok(first) = commands.recv() {
        let mut batch = vec![first];
        while let Ok(next) = commands.try_recv() {
            batch.push(next);
        }
        let mut pending: Option<RecoveryPoint> = None;
        let mut stop = false;
        for command in batch {
            match command {
                JournalCommand::Record(point) => pending = Some(point),
                JournalCommand::End(point) => {
                    // The file is deleted next, so a record still waiting
                    // before the end has nothing left to protect.
                    pending = None;
                    if let Err(kind) = file.write(&point) {
                        report(JournalFailure::Write(kind));
                    }
                    if let Err(kind) = file.delete() {
                        report(JournalFailure::Delete(kind));
                    }
                }
                JournalCommand::Shutdown => {
                    stop = true;
                    break;
                }
            }
        }
        if let Some(point) = pending
            && let Err(kind) = file.write(&point)
        {
            report(JournalFailure::Write(kind));
        }
        if stop {
            break;
        }
    }
    file.writes
}

/// The journal file and the ordering guard over what was written to it.
struct JournalFile {
    path: PathBuf,
    /// `(session_id, seq)` of the last point written.
    last: Option<(String, u64)>,
    writes: usize,
}

impl JournalFile {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            last: None,
            writes: 0,
        }
    }

    fn temp_path(&self) -> PathBuf {
        let mut name: OsString = self.path.as_os_str().to_owned();
        name.push(TEMP_SUFFIX);
        PathBuf::from(name)
    }

    /// Whether `point` is newer than the last one written for its session.
    /// A different session resets the guard.
    fn is_newer(&self, point: &RecoveryPoint) -> bool {
        match &self.last {
            Some((session, seq)) if session == point.session_id.as_str() => point.seq > *seq,
            _ => true,
        }
    }

    /// Atomically replace the journal with `point`. A point the guard refuses
    /// is dropped and is not a failure. On error the previous file is intact.
    fn write(&mut self, point: &RecoveryPoint) -> Result<(), io::ErrorKind> {
        if !self.is_newer(point) {
            return Ok(());
        }
        // File access on this worker is never inside a GPUI-thread scope. The
        // note counts only if this write is moved onto the GPUI thread.
        note(HotPathKind::FileAccess);
        let bytes = encode(point);
        let temp = self.temp_path();
        let result = self.stage_and_replace(&temp, &bytes);
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(|error| error.kind())?;
        self.last = Some((point.session_id.as_str().to_owned(), point.seq));
        self.writes += 1;
        Ok(())
    }

    fn stage_and_replace(&self, temp: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut staged = fs::File::create(temp)?;
        staged.write_all(bytes)?;
        staged.sync_all()?;
        drop(staged);
        if fs::rename(temp, &self.path).is_err() {
            thread::sleep(RENAME_RETRY_DELAY);
            fs::rename(temp, &self.path)?;
        }
        Ok(())
    }

    /// Remove the journal; a file that is already gone is success. A leftover
    /// staging file goes too, best effort.
    fn delete(&mut self) -> Result<(), io::ErrorKind> {
        note(HotPathKind::FileAccess);
        let _ = fs::remove_file(self.temp_path());
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.kind()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use platform::display::DisplayBounds;
    use presentation::{
        journal::{LiveIntent, OutputEvidence, WrittenAt, decode},
        snapshot_reader::{DirectorySource, LoadedSnapshot, read_snapshot},
    };

    use super::*;

    /// A unique, empty directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "ordo-recovery-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn journal(&self) -> PathBuf {
            self.0.join(JOURNAL_FILE)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn snapshot() -> LoadedSnapshot {
        let directory = crate::presenter::fixture_snapshot_dir();
        read_snapshot(&DirectorySource::new(directory)).expect("the fixture snapshot loads")
    }

    fn session(n: u8) -> SessionId {
        SessionId::parse(&format!("00000000-0000-4000-8000-0000000008{n:02}")).unwrap()
    }

    fn point(snapshot: &LoadedSnapshot, session_n: u8, seq: u64, ended: bool) -> RecoveryPoint {
        let cue = snapshot.snapshot().cue(0).unwrap().id();
        RecoveryPoint {
            session_id: session(session_n),
            seq,
            written_at: WrittenAt::parse("2026-11-08T09:00:00Z").unwrap(),
            snapshot_id: snapshot.snapshot().id(),
            manifest_hash: snapshot.manifest_hash(),
            source_dir: "fixture".to_owned(),
            preview: cue,
            program: None,
            state: LiveIntent::EnabledBlack,
            output: OutputEvidence {
                enabled: true,
                display: None,
            },
            ended,
        }
    }

    /// Run the worker loop to completion over `commands` already queued, on
    /// this thread, and return the writes made and the failures reported.
    fn run(path: PathBuf, commands: Vec<JournalCommand>) -> (usize, Vec<JournalFailure>) {
        let (sender, received) = mpsc::channel();
        for command in commands {
            sender.send(command).unwrap();
        }
        sender.send(JournalCommand::Shutdown).unwrap();
        let mut failures = Vec::new();
        let writes = serve_journal(received, path, |failure| failures.push(failure));
        (writes, failures)
    }

    fn on_disk(path: &Path) -> RecoveryPoint {
        decode(&fs::read(path).unwrap()).expect("the journal on disk decodes")
    }

    #[test]
    fn a_recorded_point_reaches_the_file_and_shutdown_joins() {
        let dir = TempDir::new();
        let snap = snapshot();
        let failures = Arc::new(Mutex::new(Vec::new()));
        let seen = failures.clone();
        let (writer, mut worker, shutdown) =
            spawn_journal_writer(dir.journal(), move |f| seen.lock().unwrap().push(f)).unwrap();
        writer.record(point(&snap, 1, 1, false));
        shutdown.send();
        assert_eq!(worker.join().unwrap(), WorkerExit::Completed(()));
        assert_eq!(on_disk(&dir.journal()), point(&snap, 1, 1, false));
        assert!(failures.lock().unwrap().is_empty());
        assert!(!dir.0.join("session-recovery.json.tmp").exists());
    }

    #[test]
    fn a_burst_of_records_is_one_write_of_the_last_point() {
        let dir = TempDir::new();
        let snap = snapshot();
        let commands = (1..=50)
            .map(|seq| JournalCommand::Record(point(&snap, 1, seq, false)))
            .collect();
        let (writes, failures) = run(dir.journal(), commands);
        assert_eq!(writes, 1, "the burst coalesces");
        assert!(failures.is_empty());
        assert_eq!(on_disk(&dir.journal()).seq, 50);
    }

    #[test]
    fn an_older_point_never_replaces_a_newer_one() {
        let dir = TempDir::new();
        let snap = snapshot();
        // Separate batches: the first is written before the second arrives.
        let (writer, mut worker, shutdown) = spawn_journal_writer(dir.journal(), |_| {}).unwrap();
        writer.record(point(&snap, 1, 5, false));
        wait_for_seq(&dir.journal(), 5);
        writer.record(point(&snap, 1, 3, false));
        shutdown.send();
        worker.join().unwrap();
        assert_eq!(on_disk(&dir.journal()).seq, 5);
    }

    #[test]
    fn a_new_session_restarts_the_sequence() {
        let dir = TempDir::new();
        let snap = snapshot();
        let (writes, _) = run(
            dir.journal(),
            vec![
                JournalCommand::Record(point(&snap, 1, 9, false)),
                JournalCommand::End(point(&snap, 1, 10, true)),
                JournalCommand::Record(point(&snap, 2, 1, false)),
            ],
        );
        assert_eq!(writes, 2);
        assert_eq!(on_disk(&dir.journal()).session_id, session(2));
        assert_eq!(on_disk(&dir.journal()).seq, 1);
    }

    #[test]
    fn a_new_session_id_with_a_low_seq_is_written_after_a_high_one() {
        let dir = TempDir::new();
        let snap = snapshot();
        let (writer, mut worker, shutdown) = spawn_journal_writer(dir.journal(), |_| {}).unwrap();
        writer.record(point(&snap, 1, 9, false));
        wait_for_seq(&dir.journal(), 9);
        writer.record(point(&snap, 2, 1, false));
        shutdown.send();
        worker.join().unwrap();
        let written = on_disk(&dir.journal());
        assert_eq!((written.session_id, written.seq), (session(2), 1));
    }

    #[test]
    fn ending_a_session_leaves_no_journal_and_no_staging_file() {
        let dir = TempDir::new();
        let snap = snapshot();
        let (writes, failures) = run(
            dir.journal(),
            vec![
                JournalCommand::Record(point(&snap, 1, 1, false)),
                JournalCommand::Record(point(&snap, 1, 2, false)),
                JournalCommand::End(point(&snap, 1, 3, true)),
            ],
        );
        assert_eq!(writes, 1, "only the end point is written, then deleted");
        assert!(failures.is_empty());
        assert!(!dir.journal().exists());
        assert!(!dir.0.join("session-recovery.json.tmp").exists());
    }

    #[test]
    fn a_failed_write_is_reported_and_the_worker_carries_on() {
        let dir = TempDir::new();
        let snap = snapshot();
        // The "directory" is a file, so the journal can never be created.
        let blocker = dir.0.join("blocker");
        fs::write(&blocker, b"not a directory").unwrap();
        let failures = Arc::new(Mutex::new(Vec::new()));
        let seen = failures.clone();
        let (writer, mut worker, shutdown) =
            spawn_journal_writer(blocker.join(JOURNAL_FILE), move |f| {
                seen.lock().unwrap().push(f)
            })
            .unwrap();
        writer.record(point(&snap, 1, 1, false));
        writer.end_session(point(&snap, 1, 2, true));
        shutdown.send();
        assert_eq!(worker.join().unwrap(), WorkerExit::Completed(()));
        let failures = failures.lock().unwrap();
        assert!(
            failures
                .iter()
                .any(|failure| matches!(failure, JournalFailure::Write(_))),
            "{failures:?}"
        );
        assert_eq!(fs::read(&blocker).unwrap(), b"not a directory");
    }

    #[test]
    fn a_failed_write_keeps_the_previous_file() {
        let dir = TempDir::new();
        let snap = snapshot();
        let (writes, _) = run(
            dir.journal(),
            vec![JournalCommand::Record(point(&snap, 1, 1, false))],
        );
        assert_eq!(writes, 1);
        // Block the staging file with a directory so the next write fails.
        fs::create_dir(dir.0.join("session-recovery.json.tmp")).unwrap();
        let (writes, failures) = run(
            dir.journal(),
            vec![JournalCommand::Record(point(&snap, 1, 2, false))],
        );
        assert_eq!(writes, 0);
        assert!(matches!(failures.as_slice(), [JournalFailure::Write(_)]));
        assert_eq!(on_disk(&dir.journal()).seq, 1);
    }

    #[test]
    fn the_journal_sits_beside_the_preferences_file() {
        let preferences = Path::new("some").join("dir").join("preferences.json");
        assert_eq!(
            journal_path(&preferences),
            Path::new("some").join("dir").join(JOURNAL_FILE)
        );
    }

    #[test]
    fn a_new_session_id_is_a_lowercase_uuid_v4_each_time() {
        let (first, second) = (new_session_id(), new_session_id());
        assert_ne!(first, second);
        assert_eq!(first.as_str(), first.as_str().to_lowercase());
        assert_eq!(first.as_str().len(), 36);
    }

    #[test]
    fn display_evidence_maps_name_uuid_and_bounds() {
        let evidence = DisplayEvidence {
            name: Some("Hall projector".to_owned()),
            uuid: Some("00000000-0000-5000-8000-000000000001".to_owned()),
            bounds: DisplayBounds::new(-1920, 10, 1920, 1080),
        };
        let record = display_record(&evidence);
        assert_eq!(record.name.as_deref(), Some("Hall projector"));
        assert_eq!(
            record.uuid.as_deref(),
            Some("00000000-0000-5000-8000-000000000001")
        );
        assert_eq!(
            record.bounds,
            BoundsRecord {
                x: -1920,
                y: 10,
                width: 1920,
                height: 1080
            }
        );
    }

    /// Wait (bounded) until the journal on disk holds `seq`.
    fn wait_for_seq(path: &Path, seq: u64) {
        for _ in 0..200 {
            if fs::read(path)
                .ok()
                .and_then(|bytes| decode(&bytes).ok())
                .is_some_and(|point| point.seq == seq)
            {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("the journal never reached seq {seq}");
    }
}
