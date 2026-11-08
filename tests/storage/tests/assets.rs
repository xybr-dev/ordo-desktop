//! T2.1c managed asset and cache acceptance tests. Every await runs under a
//! 10 s bound so a hang fails the test instead of blocking the run. Databases
//! and asset directories live in unique temporary directories.

use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::identity::ManagedAssetId;
use futures_lite::future::block_on;
use rusqlite::{Connection, OpenFlags};
use storage::{
    AssetUpload, ContentHash, NewTheme, ShutdownOutcome, Storage, StorageConfig, StorageError,
    ThemeRecord,
};

const BOUND: Duration = Duration::from_secs(10);

/// SHA-256 of the ASCII bytes `abc` (FIPS 180-4 test vector).
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

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
            "ordo-assets-tests-{tag}-{}-{}-{nanos}",
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

    fn cache(&self) -> PathBuf {
        self.0.join("cache")
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

fn png(bytes: &[u8]) -> Option<AssetUpload> {
    Some(AssetUpload {
        bytes: bytes.to_vec(),
        media_type: "image/png".to_owned(),
    })
}

fn theme_input(name: &str, background: Option<AssetUpload>) -> NewTheme {
    NewTheme {
        name: name.to_owned(),
        background,
    }
}

fn create(storage: &Storage, name: &str, bytes: &[u8]) -> ThemeRecord {
    bounded(storage.create_theme(theme_input(name, png(bytes)))).expect("theme")
}

/// Names of the entries directly in `assets/`, sorted, `.staging` excluded.
fn asset_names(dir: &TempDir) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir.assets())
        .expect("read assets")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("utf8")
        })
        .filter(|name| name != ".staging")
        .collect();
    names.sort();
    names
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

/// Asset row count, read through a separate read-only connection.
fn asset_count(dir: &TempDir) -> i64 {
    let conn = Connection::open_with_flags(dir.db(), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("read-only open");
    conn.query_row("SELECT COUNT(*) FROM managed_asset", [], |row| row.get(0))
        .expect("count")
}

fn background_id(theme: &ThemeRecord) -> ManagedAssetId {
    theme.background.as_ref().expect("background").id
}

#[test]
fn theme_background_commits_with_its_asset() {
    let dir = TempDir::new("commit");
    let storage = open(&dir);
    let created = create(&storage, "Easter", b"abc");

    let asset = created.background.clone().expect("background");
    assert_eq!(
        asset.sha256,
        ABC_SHA256.parse::<ContentHash>().expect("hash")
    );
    assert_eq!(asset.byte_len, 3);
    assert_eq!(asset.media_type, "image/png");
    assert_eq!(bounded(storage.theme(created.id)), Ok(created.clone()));
    assert_eq!(bounded(storage.read_asset(asset.id)), Ok(b"abc".to_vec()));
    assert_eq!(asset_names(&dir), vec![asset.id.to_string()]);
    assert!(staging_is_empty(&dir));
    close(storage);

    let reopened = open(&dir);
    assert_eq!(bounded(reopened.theme(created.id)), Ok(created));
    assert_eq!(bounded(reopened.read_asset(asset.id)), Ok(b"abc".to_vec()));
    assert_eq!(asset_names(&dir), vec![asset.id.to_string()]);
    assert!(staging_is_empty(&dir));
    let report = bounded(reopened.verify_assets()).expect("report");
    assert_eq!(report.checked, 1);
    assert!(report.missing.is_empty() && report.corrupt.is_empty());
    close(reopened);
}

#[test]
fn theme_without_background_is_a_plain_row() {
    let dir = TempDir::new("plain");
    let storage = open(&dir);
    let created = bounded(storage.create_theme(theme_input("Plain", None))).expect("theme");
    assert_eq!(created.background, None);
    assert_eq!(bounded(storage.theme(created.id)), Ok(created));
    assert!(asset_names(&dir).is_empty());
    close(storage);
}

#[test]
fn failed_asset_write_leaves_nothing() {
    let dir = TempDir::new("write-fail");
    let storage = open(&dir);
    let kept = create(&storage, "Kept", b"kept bytes");
    let before = asset_names(&dir);

    bounded(storage.inject_asset_write_failure()).expect("arm");
    let failed = bounded(storage.create_theme(theme_input("Doomed", png(b"doomed bytes"))));
    assert!(
        matches!(failed, Err(StorageError::AssetWrite(_))),
        "{failed:?}"
    );

    assert_eq!(asset_names(&dir), before);
    assert!(staging_is_empty(&dir));
    assert_eq!(bounded(storage.verify_assets()).expect("report").checked, 1);
    assert_eq!(
        bounded(storage.read_asset(background_id(&kept))),
        Ok(b"kept bytes".to_vec())
    );
    close(storage);
    assert_eq!(theme_count(&dir), 1);
    assert_eq!(asset_count(&dir), 1);
}

#[test]
fn failed_row_write_removes_the_file() {
    let dir = TempDir::new("row-fail");
    let storage = open(&dir);
    create(&storage, "Kept", b"kept bytes");
    let before = asset_names(&dir);

    for step in [1, 2] {
        bounded(storage.inject_write_failure(step)).expect("arm");
        let failed = bounded(storage.create_theme(theme_input("Doomed", png(b"doomed bytes"))));
        assert_eq!(
            failed,
            Err(StorageError::Sqlite("injected write failure".to_owned())),
            "step {step}"
        );
        assert_eq!(asset_names(&dir), before, "step {step}");
        assert!(staging_is_empty(&dir), "step {step}");
    }
    assert_eq!(bounded(storage.verify_assets()).expect("report").checked, 1);
    close(storage);
    assert_eq!(theme_count(&dir), 1);
    assert_eq!(asset_count(&dir), 1);
}

#[test]
fn orphan_files_are_swept_on_open() {
    let dir = TempDir::new("sweep");
    let storage = open(&dir);
    let kept = create(&storage, "Kept", b"kept bytes");
    close(storage);

    let orphan = dir.assets().join("5f0c2b1e-9d3a-4c7b-8e21-0a4b6c8d1e2f");
    let part = dir.staging().join("x.part");
    let notes = dir.assets().join("notes.txt");
    fs::write(&orphan, b"orphan").expect("orphan");
    fs::write(&part, b"half").expect("part");
    fs::write(&notes, b"keep me").expect("notes");

    let reopened = open(&dir);
    assert!(!orphan.exists());
    assert!(!part.exists());
    assert!(notes.exists());
    assert!(dir.assets().join(background_id(&kept).to_string()).exists());
    let report = bounded(reopened.verify_assets()).expect("report");
    assert_eq!(report.checked, 1);
    assert!(report.missing.is_empty() && report.corrupt.is_empty());
    close(reopened);
}

#[test]
fn corrupt_or_missing_asset_is_reported() {
    let dir = TempDir::new("integrity");
    let storage = open(&dir);
    let intact = create(&storage, "Intact", b"intact bytes");
    let flipped = create(&storage, "Flipped", b"flipped bytes");
    let removed = create(&storage, "Removed", b"removed bytes");
    close(storage);

    let flipped_id = background_id(&flipped);
    let removed_id = background_id(&removed);
    let flipped_path = dir.assets().join(flipped_id.to_string());
    let mut bytes = fs::read(&flipped_path).expect("read");
    bytes[0] ^= 0xff;
    fs::write(&flipped_path, bytes).expect("flip");
    fs::remove_file(dir.assets().join(removed_id.to_string())).expect("remove");

    let reopened = open(&dir);
    let report = bounded(reopened.verify_assets()).expect("report");
    assert_eq!(report.checked, 3);
    assert_eq!(report.corrupt, vec![flipped_id]);
    assert_eq!(report.missing, vec![removed_id]);
    assert_eq!(
        bounded(reopened.read_asset(flipped_id)),
        Err(StorageError::AssetIntegrity { id: flipped_id })
    );
    assert_eq!(
        bounded(reopened.read_asset(removed_id)),
        Err(StorageError::AssetIntegrity { id: removed_id })
    );
    assert_eq!(
        bounded(reopened.read_asset(background_id(&intact))),
        Ok(b"intact bytes".to_vec())
    );
    // Reads never repair: the removed file stays removed.
    assert!(!dir.assets().join(removed_id.to_string()).exists());
    close(reopened);
}

#[test]
fn cache_is_disposable() {
    let dir = TempDir::new("cache");
    let storage = open(&dir);
    let theme = create(&storage, "Cached", b"cached bytes");
    let id = background_id(&theme);
    assert!(dir.cache().is_dir());

    fs::write(dir.cache().join("thumb.bin"), b"thumb").expect("cache file");
    fs::create_dir_all(dir.cache().join("nested")).expect("cache dir");
    fs::write(dir.cache().join("nested").join("x"), b"x").expect("nested file");
    bounded(storage.clear_cache()).expect("clear");
    assert!(dir.cache().is_dir());
    assert_eq!(fs::read_dir(dir.cache()).expect("read").count(), 0);
    assert_eq!(bounded(storage.theme(theme.id)), Ok(theme.clone()));
    assert_eq!(
        bounded(storage.read_asset(id)),
        Ok(b"cached bytes".to_vec())
    );
    close(storage);

    fs::remove_dir_all(dir.cache()).expect("delete cache");
    let reopened = open(&dir);
    assert!(dir.cache().is_dir());
    assert_eq!(bounded(reopened.theme(theme.id)), Ok(theme));
    assert_eq!(
        bounded(reopened.read_asset(id)),
        Ok(b"cached bytes".to_vec())
    );
    close(reopened);
}

#[test]
fn overlapping_cache_and_assets_is_refused() {
    let dir = TempDir::new("overlap");
    let nested_cache = StorageConfig {
        cache_dir: dir.assets().join("c"),
        ..dir.config()
    };
    let nested_assets = StorageConfig {
        assets_dir: dir.cache().join("a"),
        ..dir.config()
    };
    let same = StorageConfig {
        cache_dir: dir.assets(),
        ..dir.config()
    };
    for config in [nested_cache, nested_assets, same] {
        let storage = Storage::spawn(config);
        let status = bounded(storage.ready());
        assert!(
            matches!(
                status,
                Err(StorageError::InvalidInput {
                    field: "cache_dir",
                    reason: "overlaps assets"
                })
            ),
            "{status:?}"
        );
    }
}

#[test]
fn unsupported_media_type_writes_nothing() {
    let dir = TempDir::new("media");
    let storage = open(&dir);
    let gif = bounded(storage.create_theme(theme_input(
        "Gif",
        Some(AssetUpload {
            bytes: b"GIF89a".to_vec(),
            media_type: "image/gif".to_owned(),
        }),
    )));
    assert_eq!(
        gif,
        Err(StorageError::InvalidInput {
            field: "media_type",
            reason: "unsupported"
        })
    );
    let empty = bounded(storage.create_theme(theme_input("Empty", png(b""))));
    assert_eq!(
        empty,
        Err(StorageError::InvalidInput {
            field: "background",
            reason: "empty"
        })
    );
    let blank = bounded(storage.create_theme(theme_input("  ", png(b"abc"))));
    assert_eq!(
        blank,
        Err(StorageError::InvalidInput {
            field: "name",
            reason: "blank"
        })
    );
    assert!(asset_names(&dir).is_empty());
    assert!(staging_is_empty(&dir));
    assert_eq!(bounded(storage.verify_assets()).expect("report").checked, 0);
    close(storage);
    assert_eq!(theme_count(&dir), 0);
    assert_eq!(asset_count(&dir), 0);
}
