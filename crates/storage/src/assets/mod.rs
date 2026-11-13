//! Managed asset files and the disposable cache directory.
//!
//! `assets/<id>` is the one authoritative file for a `managed_asset` row: it is
//! named by the canonical identity, has no extension, and its bytes never
//! change under one identity. `assets/.staging/` holds in-flight writes only,
//! and `cache/` is disposable and never read as authoritative data.
//!
//! The write protocol (see the parish store) stages the bytes, syncs them,
//! renames them into place, and only then inserts the rows. A crash between the
//! rename and the commit leaves an unreferenced file, which the open-time sweep
//! removes. The sweep deletes only `.staging/*` and canonical-id-named files
//! directly inside `assets/` that have no row, and never fails open.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use domain::identity::EntityId;
use domain::presentation::ContentHash;
use rusqlite::{Connection, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::StorageError;
use crate::worker::Core;

/// Name of the in-flight write directory inside `assets/`.
const STAGING: &str = ".staging";

/// Where the asset files of one open store live.
#[derive(Clone, Debug)]
pub(crate) struct AssetLayout {
    dir: PathBuf,
    staging: PathBuf,
}

impl AssetLayout {
    /// The layout rooted at `dir`.
    pub(crate) fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            staging: dir.join(STAGING),
        }
    }

    /// The authoritative file for one asset identity.
    pub(crate) fn path(&self, id: EntityId) -> PathBuf {
        self.dir.join(id.to_string())
    }

    /// The staging file for one asset identity.
    fn part(&self, id: EntityId) -> PathBuf {
        self.staging.join(format!("{id}.part"))
    }
}

/// SHA-256 of the bytes.
pub(crate) fn hash(bytes: &[u8]) -> ContentHash {
    ContentHash::from_bytes(Sha256::digest(bytes).into())
}

/// Maps a file error to the public taxonomy.
fn io(error: &std::io::Error) -> StorageError {
    StorageError::AssetWrite(error.to_string())
}

/// True when a final file for `id` already exists; a check that cannot be
/// made is an `AssetWrite` error.
pub(crate) fn file_exists(layout: &AssetLayout, id: EntityId) -> Result<bool, StorageError> {
    layout.path(id).try_exists().map_err(|error| io(&error))
}

/// Writes `bytes` to `assets/<id>` through the staging directory.
///
/// Creates `<id>.part` with `create_new`, writes and syncs it, then renames it
/// into place. The caller has already checked the destination is absent;
/// Windows `rename` would replace an existing file. With `fail_midway`, only
/// half of the bytes reach the staging file before the write fails. On error
/// the caller removes both files with [`remove_files`].
pub(crate) fn write_file(
    layout: &AssetLayout,
    id: EntityId,
    bytes: &[u8],
    fail_midway: bool,
) -> Result<(), StorageError> {
    fs::create_dir_all(&layout.staging).map_err(|error| io(&error))?;
    let part = layout.part(id);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .map_err(|error| io(&error))?;
    if fail_midway {
        file.write_all(&bytes[..bytes.len() / 2])
            .map_err(|error| io(&error))?;
        return Err(StorageError::AssetWrite(
            "injected asset write failure".to_owned(),
        ));
    }
    file.write_all(bytes).map_err(|error| io(&error))?;
    file.sync_all().map_err(|error| io(&error))?;
    drop(file);
    fs::rename(&part, layout.path(id)).map_err(|error| io(&error))?;
    #[cfg(unix)]
    fs::File::open(&layout.dir)
        .and_then(|dir| dir.sync_all())
        .map_err(|error| io(&error))?;
    Ok(())
}

/// Removes the staging and final files of one identity, ignoring absence and
/// any other error (the next open's sweep retries).
pub(crate) fn remove_files(layout: &AssetLayout, id: EntityId) {
    let _ = fs::remove_file(layout.part(id));
    let _ = fs::remove_file(layout.path(id));
}

/// Reads the file of one asset. A file that does not exist is `Ok(None)`; any
/// other read error is an `AssetWrite` error, so a permissions or I/O problem
/// is never misreported as a missing or corrupt asset.
pub(crate) fn read_file(
    layout: &AssetLayout,
    id: EntityId,
) -> Result<Option<Vec<u8>>, StorageError> {
    match fs::read(layout.path(id)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io(&error)),
    }
}

/// Checks the configured directories before anything touches the disk.
///
/// Refuses `..` components in the database path or either directory, equal or
/// nested assets and cache directories, and a cache directory that is the
/// database's directory or an ancestor of the database file (`clear_cache`
/// deletes it recursively).
/// Paths are made absolute against the current directory first, so a relative
/// spelling cannot hide an overlap; they need not exist. Comparison is by path
/// spelling only: letter case or symlinks (on Windows, also junctions and 8.3
/// names) can still hide an overlap.
pub(crate) fn validate_dirs(db: &Path, assets: &Path, cache: &Path) -> Result<(), StorageError> {
    for (field, dir) in [("path", db), ("assets_dir", assets), ("cache_dir", cache)] {
        if dir.components().any(|part| part == Component::ParentDir) {
            return Err(StorageError::InvalidInput {
                field,
                reason: "relative parent",
            });
        }
    }
    let unresolved = |field| StorageError::InvalidInput {
        field,
        reason: "unresolvable path",
    };
    let db = std::path::absolute(db).map_err(|_| unresolved("path"))?;
    let assets = std::path::absolute(assets).map_err(|_| unresolved("assets_dir"))?;
    let cache = std::path::absolute(cache).map_err(|_| unresolved("cache_dir"))?;
    if assets.starts_with(&cache) || cache.starts_with(&assets) {
        return Err(StorageError::InvalidInput {
            field: "cache_dir",
            reason: "overlaps assets",
        });
    }
    if db.starts_with(&cache) {
        return Err(StorageError::InvalidInput {
            field: "cache_dir",
            reason: "contains database",
        });
    }
    Ok(())
}

/// Creates the directories, then sweeps. Called after migrations succeed.
/// Nothing here fails open.
pub(crate) fn prepare(layout: &AssetLayout, cache_dir: &Path, conn: &mut Connection) {
    let _ = fs::create_dir_all(&layout.staging);
    let _ = fs::create_dir_all(cache_dir);
    sweep(layout, conn);
}

/// Deletes staging leftovers and canonical-id-named files without a row.
///
/// The whole sweep runs inside an immediate transaction that is dropped
/// (rolled back) at the end. Holding the write lock means no `create_theme`
/// in another process sits between its file rename and its commit, so an
/// unreferenced file really is an orphan. If the transaction cannot start
/// (busy or any error), or the row list cannot be read, nothing is deleted.
fn sweep(layout: &AssetLayout, conn: &mut Connection) {
    let Ok(tx) = conn.transaction_with_behavior(TransactionBehavior::Immediate) else {
        return;
    };
    let Some(known) = known_ids(&tx) else {
        return;
    };
    if let Ok(entries) = fs::read_dir(&layout.staging) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let Ok(entries) = fs::read_dir(&layout.dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name();
        let Some(text) = name.to_str() else {
            continue;
        };
        let canonical = EntityId::parse(text).is_ok_and(|id| id.to_string() == text);
        if canonical && !known.contains(text) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Every `managed_asset` id, or `None` when the table cannot be read.
fn known_ids(conn: &Connection) -> Option<HashSet<String>> {
    let mut statement = conn.prepare("SELECT id FROM managed_asset").ok()?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?;
    let mut ids = HashSet::new();
    for row in rows {
        ids.insert(row.ok()?);
    }
    Some(ids)
}

impl Core {
    /// Removes and recreates the disposable cache directory.
    pub(crate) fn clear_cache(&mut self) -> Result<(), StorageError> {
        let cache = self.cache_dir.clone();
        match fs::remove_dir_all(&cache) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io(&error)),
        }
        fs::create_dir_all(&cache).map_err(|error| io(&error))
    }
}
