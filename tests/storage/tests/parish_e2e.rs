//! T2.1 end-to-end acceptance: one Parish with a Choir, a Song, an Arrangement
//! and a Theme with a background image, created through the storage Module
//! Interface only. Every await runs under a 10 s bound so a hang fails the test
//! instead of blocking the run. The database and asset directories live in a
//! unique temporary directory that is removed on drop.

use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::SongId;
use futures_lite::future::block_on;
use rusqlite::{Connection, OpenFlags};
use storage::{
    ArrangementRecord, AssetReport, AssetUpload, NewArrangement, NewChoir, NewParish, NewSection,
    NewSong, NewTheme, ShutdownOutcome, SongRecord, Storage, StorageConfig, StorageError,
};

const BOUND: Duration = Duration::from_secs(10);

/// The background bytes: a PNG signature followed by a marker.
const BACKGROUND: &[u8] = b"\x89PNG\r\n\x1a\nE2E background";

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
            "ordo-parish-e2e-{tag}-{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    fn config(&self) -> StorageConfig {
        StorageConfig::in_directory(&self.0)
    }

    fn db(&self) -> PathBuf {
        self.0.join("parish.sqlite3")
    }

    fn assets(&self) -> PathBuf {
        self.0.join("assets")
    }

    fn staging(&self) -> PathBuf {
        self.0.join("assets").join(".staging")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Windows can hold a handle to a just-closed database for a moment, so
        // removal is retried briefly rather than failing or leaking the directory.
        for _ in 0..5 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn open(dir: &TempDir) -> Storage {
    let storage = Storage::spawn(dir.config());
    bounded(storage.ready()).expect("storage ready");
    storage
}

fn close(storage: Storage) {
    assert_eq!(bounded(storage.shutdown()), ShutdownOutcome::Clean);
}

fn section(label: &str, lyrics: &str) -> NewSection {
    NewSection {
        label: Some(label.to_owned()),
        lyrics: lyrics.to_owned(),
    }
}

fn png(bytes: &[u8]) -> Option<AssetUpload> {
    Some(AssetUpload {
        bytes: bytes.to_vec(),
        media_type: "image/png".to_owned(),
    })
}

/// Everything a reader can see of the stored state, compared whole.
#[derive(Debug, PartialEq)]
struct Snapshot {
    songs: Vec<SongRecord>,
    arrangements: Vec<ArrangementRecord>,
    report: AssetReport,
    /// Sorted names of every entry directly under `assets/`, `.staging` included.
    files: Vec<String>,
}

fn snapshot(storage: &Storage, dir: &TempDir, song: SongId) -> Snapshot {
    let mut files: Vec<String> = fs::read_dir(dir.assets())
        .expect("read assets")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("utf8")
        })
        .collect();
    files.sort();
    Snapshot {
        songs: bounded(storage.songs()).expect("songs"),
        arrangements: bounded(storage.arrangements_for_song(song)).expect("arrangements"),
        report: bounded(storage.verify_assets()).expect("report"),
        files,
    }
}

fn staging_is_empty(dir: &TempDir) -> bool {
    fs::read_dir(dir.staging()).expect("read staging").count() == 0
}

/// Theme row count, read through a separate read-only connection.
fn theme_count(dir: &TempDir) -> i64 {
    let conn = Connection::open_with_flags(dir.db(), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("read-only open");
    conn.query_row("SELECT COUNT(*) FROM theme", [], |row| row.get(0))
        .expect("count")
}

#[test]
fn t2_1_e2e_parish_rehydrates_and_asset_failure_rolls_back() {
    let dir = TempDir::new("e2e");

    // 1. Open a fresh directory: the worker migrates and creates assets/.
    let storage = open(&dir);

    // 2. Create the Parish, Choir, Song, Arrangement and a Theme with a PNG background.
    let parish = bounded(storage.create_parish(NewParish {
        name: "St Mary".to_owned(),
    }))
    .expect("parish");
    let choir = bounded(storage.create_choir(NewChoir {
        name: "Adults".to_owned(),
        short_label: None,
        languages: Vec::new(),
        notes: None,
    }))
    .expect("choir");
    let song = bounded(storage.create_song(NewSong {
        title: "Kyrie".to_owned(),
        aliases: vec!["Lord have mercy".to_owned()],
        original_attribution: Some("Anon".to_owned()),
        provenance: None,
        rights_notes: None,
        liturgical_moments: vec!["Penitential Act".to_owned()],
        occasions: vec!["Lent".to_owned()],
    }))
    .expect("song");
    let arrangement = bounded(storage.create_arrangement(NewArrangement {
        song: song.id,
        title: "SATB".to_owned(),
        languages: vec!["en".to_owned()],
        choirs: vec![choir.id],
        rendition_attribution: Some("Arr. Smith".to_owned()),
        occasions: vec!["Lent".to_owned()],
        sections: vec![
            section("Verse", "Line one\nLine two"),
            section("Refrain", "Refrain text"),
        ],
        sequence: vec![0, 1, 0],
    }))
    .expect("arrangement");
    let theme = bounded(storage.create_theme(NewTheme {
        name: "Easter".to_owned(),
        background: png(BACKGROUND),
    }))
    .expect("theme");
    let background = theme.background.clone().expect("background");

    // 3. Shut down cleanly and spawn a fresh worker on the same directory.
    close(storage);
    let storage = open(&dir);

    // 4. Every record and the asset bytes rehydrate exactly as created.
    assert_eq!(bounded(storage.parish()), Ok(Some(parish)));
    assert_eq!(bounded(storage.choirs()), Ok(vec![choir]));
    assert_eq!(bounded(storage.song(song.id)), Ok(song.clone()));
    assert_eq!(
        bounded(storage.arrangement(arrangement.id)),
        Ok(arrangement)
    );
    assert_eq!(bounded(storage.theme(theme.id)), Ok(theme.clone()));
    assert_eq!(
        bounded(storage.read_asset(background.id)),
        Ok(BACKGROUND.to_vec())
    );

    // 5. Snapshot the stored state, and confirm it is not vacuous.
    let before = snapshot(&storage, &dir, song.id);
    assert_eq!(before.songs, vec![song.clone()]);
    assert_eq!(before.report.checked, 1);
    assert!(before.report.missing.is_empty() && before.report.corrupt.is_empty());
    assert!(before.files.contains(&background.id.to_string()));

    // 6. Arm an asset-write failure, then a second Theme with a background is refused.
    bounded(storage.inject_asset_write_failure()).expect("arm");
    let failed = bounded(storage.create_theme(NewTheme {
        name: "Doomed".to_owned(),
        background: png(b"doomed background"),
    }));
    assert!(
        matches!(failed, Err(StorageError::AssetWrite(_))),
        "{failed:?}"
    );

    // 7. Nothing half-committed: the snapshot is unchanged, staging is empty,
    //    and the failed Theme row is absent, also after one more restart.
    assert_eq!(snapshot(&storage, &dir, song.id), before);
    assert!(staging_is_empty(&dir));
    close(storage);
    assert_eq!(theme_count(&dir), 1);

    let reopened = open(&dir);
    assert_eq!(snapshot(&reopened, &dir, song.id), before);
    assert!(staging_is_empty(&dir));
    close(reopened);
}
