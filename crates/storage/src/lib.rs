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
//! Managed assets: image originals live under `assets/<id>` beside the
//! database, committed together with their rows (see `create_theme`). After
//! migrations succeed, open creates `assets/`, `assets/.staging/` and the
//! disposable `cache/`, and sweeps staging leftovers and unreferenced
//! canonical-id files. Open refuses a `cache_dir` that overlaps `assets_dir`.
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
//!
//! The parish store creates and reads the Parish, Choirs, Songs and
//! Arrangements. The worker generates every id and creation time, and every
//! multi-row create is one immediate transaction that rolls back in full on
//! any failure.
//!
//! The committed `ERD.md` is rendered from the bundled migrations and checked
//! for drift by the test suite.

use std::fmt;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod assets;
#[cfg(test)]
mod erd;
mod migrations;
#[cfg(test)]
mod parish_schema_tests;
mod parish_store;
mod revisions;
mod worker;

use domain::identity::{
    ArrangementId, ChoirId, EntityId, HistorySessionId, ManagedAssetId, RevisionId, SongId, ThemeId,
};
use domain::revisions::{RevisionEntity, RevisionKind};
use domain::song_library::PaginationState;
use migrations::Migration;
use worker::{Core, Exit, Message, Sources};

pub use domain::presentation::ContentHash;
pub use parish_store::{
    ArrangementRecord, AssetReport, AssetUpload, ChoirDetails, ChoirRecord, LibraryRecords,
    ManagedAssetRecord, NewArrangement, NewChoir, NewParish, NewSection, NewSong, NewTheme,
    OccurrenceRecord, ParishRecord, ReservedIds, SectionRecord, SongRecord, ThemeRecord,
};
pub use revisions::{
    ArrangementContent, ArrangementMetadata, OccurrenceContent, RevisionContent, RevisionRecord,
    SectionContent, SongContent,
};

/// Where the database, managed assets and disposable cache live, and how long
/// a request waits on a locked file.
///
/// `cache_dir` must not equal `assets_dir` or be nested with it; open refuses
/// such a configuration with `InvalidInput { field: "cache_dir", .. }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub busy_timeout: Duration,
    /// Authoritative asset files, one per managed asset, plus `.staging/`.
    pub assets_dir: PathBuf,
    /// Disposable cache; safe to delete at any time.
    pub cache_dir: PathBuf,
}

impl StorageConfig {
    /// Configuration with the default 5 second busy timeout. The asset and
    /// cache directories are `assets` and `cache` beside the database file.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        Self {
            assets_dir: parent.join("assets"),
            cache_dir: parent.join("cache"),
            path,
            busy_timeout: Duration::from_secs(5),
        }
    }

    /// Configuration for a parish data directory: `root/parish.sqlite3`,
    /// `root/assets` and `root/cache`.
    pub fn in_directory(root: impl AsRef<Path>) -> Self {
        Self::new(root.as_ref().join("parish.sqlite3"))
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

/// The kind of record a [`StorageError`] names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordKind {
    /// The single Parish.
    Parish,
    /// A Choir.
    Choir,
    /// A Song work.
    Song,
    /// An Arrangement of a Song.
    Arrangement,
    /// A section of an Arrangement.
    Section,
    /// A Sequence occurrence of a section.
    Occurrence,
    /// A visual Theme.
    Theme,
    /// A managed binary asset.
    ManagedAsset,
    /// An Edit revision.
    Revision,
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Parish => "parish",
            Self::Choir => "choir",
            Self::Song => "song",
            Self::Arrangement => "arrangement",
            Self::Section => "section",
            Self::Occurrence => "occurrence",
            Self::Theme => "theme",
            Self::ManagedAsset => "managed asset",
            Self::Revision => "revision",
        })
    }
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
    /// A record the operation needs does not exist.
    NotFound { kind: RecordKind, id: EntityId },
    /// The input names a record that does not exist (a song or choir).
    MissingReference { kind: RecordKind, id: EntityId },
    /// The input breaks a rule the store checks before writing.
    InvalidInput {
        field: &'static str,
        reason: &'static str,
    },
    /// The record is already archived, so it cannot be archived again.
    AlreadyArchived { kind: RecordKind },
    /// The record is not archived, so it cannot be restored.
    NotArchived { kind: RecordKind },
    /// `create_parish` was called when the database already has its Parish.
    ParishExists,
    /// The id source produced colliding ids too many times in a row.
    IdentityExhausted { kind: RecordKind },
    /// Writing an asset file failed; nothing was committed and no file was left behind.
    AssetWrite(String),
    /// The stored bytes of an asset are missing or do not match its recorded hash.
    AssetIntegrity { id: ManagedAssetId },
    /// A stored revision's content is damaged or unreadable.
    RevisionIntegrity { id: RevisionId },
    /// The edit was based on a revision that is no longer the entity's latest.
    StaleRevision { entity_latest: RevisionId },
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
            Self::NotFound { kind, id } => write!(f, "{kind} {id} was not found"),
            Self::MissingReference { kind, id } => {
                write!(f, "referenced {kind} {id} does not exist")
            }
            Self::InvalidInput { field, reason } => write!(f, "invalid {field}: {reason}"),
            Self::AlreadyArchived { kind } => write!(f, "the {kind} is already archived"),
            Self::NotArchived { kind } => write!(f, "the {kind} is not archived"),
            Self::ParishExists => write!(f, "the parish already exists"),
            Self::IdentityExhausted { kind } => {
                write!(f, "could not generate a unique {kind} id")
            }
            Self::AssetWrite(message) => write!(f, "asset file error: {message}"),
            Self::AssetIntegrity { id } => {
                write!(
                    f,
                    "asset {id} is missing or does not match its recorded hash"
                )
            }
            Self::RevisionIntegrity { .. } => {
                write!(f, "stored revision content is damaged or unreadable")
            }
            Self::StaleRevision { .. } => {
                write!(f, "the entity changed since this edit was based on it")
            }
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
        spawn_with(config, migrations::bundled(), Sources::system())
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

    /// Creates the Parish. Refused with `ParishExists` when one is stored.
    pub fn create_parish(
        &self,
        parish: NewParish,
    ) -> impl Future<Output = Result<ParishRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.create_parish(parish)
        })
    }

    /// Reads the Parish, or `None` before it is created.
    pub fn parish(
        &self,
    ) -> impl Future<Output = Result<Option<ParishRecord>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.parish())
    }

    /// Creates a Choir.
    pub fn create_choir(
        &self,
        choir: NewChoir,
    ) -> impl Future<Output = Result<ChoirRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.create_choir(choir)
        })
    }

    /// Reads every Choir ordered by creation time and identity.
    pub fn choirs(&self) -> impl Future<Output = Result<Vec<ChoirRecord>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.choirs())
    }

    /// Replaces a Choir's details (name, short label, languages, notes).
    ///
    /// `NotFound` when the Choir is absent. Archive state is unchanged, and
    /// the store does not refuse archived Choirs (the domain does).
    pub fn edit_choir(
        &self,
        id: ChoirId,
        details: ChoirDetails,
    ) -> impl Future<Output = Result<ChoirRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.edit_choir(id, details)
        })
    }

    /// Archives a Choir, stamping `archived_at` with the worker clock.
    ///
    /// `AlreadyArchived` when it already is. Archive never cascades and
    /// appends no revision (archive History is T2.9).
    pub fn archive_choir(
        &self,
        id: ChoirId,
    ) -> impl Future<Output = Result<ChoirRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.archive_choir(id)
        })
    }

    /// Restores an archived Choir; `NotArchived` when it is active.
    pub fn restore_choir(
        &self,
        id: ChoirId,
    ) -> impl Future<Output = Result<ChoirRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.restore_choir(id)
        })
    }

    /// Archives a Song work, stamping `archived_at` with the worker clock.
    ///
    /// `AlreadyArchived` when it already is. Its Arrangements are untouched
    /// and no revision is appended (archive History is T2.9).
    pub fn archive_song(
        &self,
        id: SongId,
    ) -> impl Future<Output = Result<SongRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.archive_song(id)
        })
    }

    /// Restores an archived Song work; `NotArchived` when it is active.
    pub fn restore_song(
        &self,
        id: SongId,
    ) -> impl Future<Output = Result<SongRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.restore_song(id)
        })
    }

    /// Archives an Arrangement, stamping `archived_at` with the worker clock.
    ///
    /// `AlreadyArchived` when it already is. No revision is appended
    /// (archive History is T2.9).
    pub fn archive_arrangement(
        &self,
        id: ArrangementId,
    ) -> impl Future<Output = Result<ArrangementRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.archive_arrangement(id)
        })
    }

    /// Restores an archived Arrangement; `NotArchived` when it is active.
    pub fn restore_arrangement(
        &self,
        id: ArrangementId,
    ) -> impl Future<Output = Result<ArrangementRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.restore_arrangement(id)
        })
    }

    /// Sets an Arrangement's pagination state; `NotFound` when absent.
    pub fn set_pagination_state(
        &self,
        id: ArrangementId,
        state: PaginationState,
    ) -> impl Future<Output = Result<ArrangementRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.set_pagination_state(id, state)
        })
    }

    /// Reads every Song, Arrangement and Choir in one read transaction, each
    /// list ordered by creation time and identity.
    pub fn library(&self) -> impl Future<Output = Result<LibraryRecords, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.library())
    }

    /// Creates a Song with its aliases, liturgical moments and occasions.
    pub fn create_song(
        &self,
        song: NewSong,
    ) -> impl Future<Output = Result<SongRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.create_song(song)
        })
    }

    /// Replaces a Song's content and appends a revision in one transaction.
    /// `expected_parent` must be the Song's latest revision, else
    /// `StaleRevision`. A Song with no revisions (a T2.1-era row) is refused
    /// with `InvalidInput { field: "expected_parent" }`: call
    /// [`Storage::latest_revision`] first, which backfills one. `Replacement`
    /// is whole-Arrangement only and is refused here.
    pub fn commit_song_edit(
        &self,
        song: SongId,
        content: SongContent,
        kind: RevisionKind,
        session: Option<HistorySessionId>,
        expected_parent: RevisionId,
    ) -> impl Future<Output = Result<(SongRecord, RevisionRecord), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.commit_song_edit(song, content, kind, session, expected_parent)
        })
    }

    /// Replaces an Arrangement's content and appends a revision in one
    /// transaction. `expected_parent` must be its latest revision, else
    /// `StaleRevision`. An Arrangement with no revisions is refused with
    /// `InvalidInput { field: "expected_parent" }`: call
    /// [`Storage::latest_revision`] first.
    ///
    /// `pagination` is the Arrangement's resulting pagination state, computed
    /// by the caller with the domain rule. It is written in the same
    /// transaction as the content and the revision, so a failure at any step
    /// leaves content, history and pagination unchanged.
    pub fn commit_arrangement_edit(
        &self,
        arrangement: ArrangementId,
        content: ArrangementContent,
        pagination: PaginationState,
        kind: RevisionKind,
        session: Option<HistorySessionId>,
        expected_parent: RevisionId,
    ) -> impl Future<Output = Result<(ArrangementRecord, RevisionRecord), StorageError>> + use<>
    {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.commit_arrangement_edit(
                arrangement,
                content,
                pagination,
                kind,
                session,
                expected_parent,
            )
        })
    }

    /// Replaces only an Arrangement's metadata (title, notes, languages,
    /// Choirs, attribution, occasions) and appends one `edit` revision.
    ///
    /// The head revision, sections, Sequence and pagination state are read in
    /// the same immediate transaction and kept as they are there, so a commit
    /// that landed since the caller last looked is never overwritten. An
    /// Arrangement with no revisions first gets its `creation` revision in
    /// that transaction. At least one language is required. The archive state
    /// is not checked: the caller gates that. Returns the whole stored
    /// Arrangement and the new revision.
    pub fn commit_arrangement_metadata(
        &self,
        arrangement: ArrangementId,
        metadata: ArrangementMetadata,
        session: HistorySessionId,
    ) -> impl Future<Output = Result<(ArrangementRecord, RevisionRecord), StorageError>> + use<>
    {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.commit_arrangement_metadata(arrangement, metadata, session)
        })
    }

    /// Replaces only an Arrangement's sections, Sequence and pagination state
    /// and appends one `edit` revision.
    ///
    /// The head revision and the stored metadata are read in the same immediate
    /// transaction and kept as they are there, so a details commit that landed
    /// since the caller last looked is never overwritten. An Arrangement with
    /// no revisions first gets its `creation` revision in that transaction.
    /// `pagination` is the resulting review state, computed by the caller with
    /// the domain rule (the editor restores an earlier state on Undo). At least
    /// one language is required, as for any Edit. The archive state is not
    /// checked: the caller gates that. Returns the whole stored Arrangement and
    /// the new revision.
    pub fn commit_arrangement_lyrics(
        &self,
        arrangement: ArrangementId,
        sections: Vec<SectionContent>,
        sequence: Vec<OccurrenceContent>,
        pagination: PaginationState,
        session: HistorySessionId,
    ) -> impl Future<Output = Result<(ArrangementRecord, RevisionRecord), StorageError>> + use<>
    {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.commit_arrangement_lyrics(arrangement, sections, sequence, pagination, session)
        })
    }

    /// Commits an earlier revision's content as a new `restoration` revision.
    /// `expected_parent` must be the entity's latest revision, else
    /// `StaleRevision`; an entity with no revisions is refused with
    /// `InvalidInput { field: "expected_parent" }` (call
    /// [`Storage::latest_revision`] first).
    ///
    /// Restoring an Arrangement revision also writes `pagination`, the
    /// resulting state the caller computed with the domain rule, in the same
    /// transaction. It is ignored when the revision belongs to a Song.
    pub fn restore_revision(
        &self,
        revision: RevisionId,
        expected_parent: RevisionId,
        pagination: PaginationState,
    ) -> impl Future<Output = Result<(RevisionContent, RevisionRecord), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.restore_revision(revision, expected_parent, pagination)
        })
    }

    /// The entity's latest revision; a T2.1-era entity with no revision first
    /// gets a backfilled `creation` revision of its current content.
    pub fn latest_revision(
        &self,
        entity: RevisionEntity,
    ) -> impl Future<Output = Result<RevisionId, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.latest_revision(entity)
        })
    }

    /// An entity's revisions, oldest first; empty when it has none.
    pub fn revisions(
        &self,
        entity: RevisionEntity,
    ) -> impl Future<Output = Result<Vec<RevisionRecord>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.revisions(entity)
        })
    }

    /// One revision's content, verified against its recorded hash.
    pub fn revision_content(
        &self,
        revision: RevisionId,
    ) -> impl Future<Output = Result<RevisionContent, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.revision_content(revision)
        })
    }

    /// Reads one Song, or `NotFound`.
    pub fn song(
        &self,
        id: SongId,
    ) -> impl Future<Output = Result<SongRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| core.song(id))
    }

    /// Reads every Song ordered by creation time and identity.
    pub fn songs(&self) -> impl Future<Output = Result<Vec<SongRecord>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.songs())
    }

    /// Creates an Arrangement with its sections, Sequence and Choir links.
    pub fn create_arrangement(
        &self,
        arrangement: NewArrangement,
    ) -> impl Future<Output = Result<ArrangementRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.create_arrangement(arrangement)
        })
    }

    /// Reads one Arrangement, or `NotFound`.
    pub fn arrangement(
        &self,
        id: ArrangementId,
    ) -> impl Future<Output = Result<ArrangementRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.arrangement(id)
        })
    }

    /// Reads a Song's Arrangements ordered by creation time and identity;
    /// `NotFound` when the Song is absent.
    pub fn arrangements_for_song(
        &self,
        song: SongId,
    ) -> impl Future<Output = Result<Vec<ArrangementRecord>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.arrangements_for_song(song)
        })
    }

    /// Creates a Theme. A background image is written under `assets/` and
    /// committed with its `managed_asset` row and the Theme row, or not at all.
    pub fn create_theme(
        &self,
        theme: NewTheme,
    ) -> impl Future<Output = Result<ThemeRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.create_theme(theme)
        })
    }

    /// Reads one Theme, or `NotFound`.
    pub fn theme(
        &self,
        id: ThemeId,
    ) -> impl Future<Output = Result<ThemeRecord, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| core.theme(id))
    }

    /// Reads the verified bytes of one asset: `NotFound` without a row,
    /// `AssetIntegrity` when the file is missing or does not match its hash.
    /// Read-only; never repairs.
    pub fn read_asset(
        &self,
        id: ManagedAssetId,
    ) -> impl Future<Output = Result<Vec<u8>, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| core.read_asset(id))
    }

    /// Re-hashes every asset file and reports missing and corrupt ones.
    /// Read-only; never repairs. A read error other than a missing file
    /// fails the report with `AssetWrite` instead of being classified.
    pub fn verify_assets(&self) -> impl Future<Output = Result<AssetReport, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.verify_assets())
    }

    /// Removes and recreates the disposable cache directory.
    pub fn clear_cache(&self) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| core.clear_cache())
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

fn spawn_with(config: StorageConfig, migrations: Vec<Migration>, sources: Sources) -> Storage {
    let (tx, rx) = async_channel::unbounded();
    let (exit_tx, exit_rx) = async_channel::bounded(1);
    let spawned = std::thread::Builder::new()
        .name("ordo-storage".to_owned())
        .spawn(move || {
            // Any unwind that escapes `worker::run` is still recorded, so the
            // outcome channel always delivers WorkerPanicked for a panic.
            let exit = match catch_unwind(AssertUnwindSafe(|| {
                worker::run(config, migrations, sources, rx)
            })) {
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
        spawn_with(config, migrations, Sources::system())
    }

    /// Spawns with scripted id bytes (used in order, then OS randomness) and a fixed clock.
    pub fn spawn_with_sources(
        config: StorageConfig,
        ids: Vec<[u8; 16]>,
        now: domain::identity::Timestamp,
    ) -> Storage {
        spawn_with(config, migrations::bundled(), Sources::scripted(ids, now))
    }

    /// Like [`Storage::spawn_with_sources`], but the clock advances by
    /// `step_millis` after every reading, so each operation gets its own time.
    pub fn spawn_with_stepping_clock(
        config: StorageConfig,
        ids: Vec<[u8; 16]>,
        start: domain::identity::Timestamp,
        step_millis: i64,
    ) -> Storage {
        spawn_with(
            config,
            migrations::bundled(),
            Sources::stepping(ids, start, step_millis),
        )
    }

    /// The next write transaction fails at its `step`-th write statement
    /// (1-based) with `StorageError::Sqlite("injected write failure")`, then
    /// the hook clears. A create that fails validation never opens a write
    /// transaction and leaves the hook armed.
    pub fn inject_write_failure(
        &self,
        step: usize,
    ) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.inject_write_failure(step);
            Ok::<(), StorageError>(())
        })
    }

    /// The next asset file write writes half the bytes to the staging file and
    /// then fails with `StorageError::AssetWrite`, then the hook clears. A
    /// create that fails before its file write leaves the hook armed.
    pub fn inject_asset_write_failure(
        &self,
    ) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), |core: &mut Core| {
            core.inject_asset_write_failure();
            Ok::<(), StorageError>(())
        })
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
        assert_eq!(status.map(|s| s.version), Ok(6));
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
