//! The single owner of the parish store and the hydrated Song Library.
//!
//! [`LibraryStore`] is a GPUI entity. Opening it never blocks the GPUI thread:
//! the folder is created, the database opened, migrated and read, and the
//! library hydrated on a background task, and the entity then emits
//! [`LibraryEvent::Changed`]. A failure at any step leaves the entity
//! [`LibraryState::Unavailable`] with a message the Library screen shows; the
//! rest of the app keeps working. The store never opens for Presenter Mode.

mod import;

pub(crate) use import::STALE_IMPORT;
pub use import::{ImportOutcome, ImportStaging, MARKDOWN_LIMIT, MarkdownFile};

use std::{cell::RefCell, collections::VecDeque, future::Future, path::PathBuf, pin::Pin, rc::Rc};

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, HistorySessionId, RevisionId, SongId, Timestamp},
    revisions::RevisionEntity,
    song_library::{
        ArchiveState, Arrangement, ArrangementDetails, ArrangementDraft, ArrangementLyrics,
        BlockerGroup, ChoirDraft, DeletionBlocker, DeletionVerdict, Field, HydrateArrangement,
        HydrateChoir, HydrateSong, OccurrenceDraft, RecordRef, SectionDraft, SongDraft,
        SongLibrary, SongLibraryError, blocker_groups, can_restore_arrangement_revision,
        can_restore_song_revision, match_key,
    },
};
use gpui::{App, AppContext as _, Context, EventEmitter, SharedString, Task};

use storage::{
    ArrangementContent, ArrangementMetadata, ArrangementRecord, ChoirDetails, ChoirRecord,
    DeletionFacts, HistoryRevision, ImportBatch, ImportCommittedRecords, NewArrangement, NewChoir,
    NewSection, NewSong, OccurrenceContent, RevisionContent, SectionContent, ShutdownOutcome,
    SongContent, SongRecord, Storage, StorageConfig, StorageError,
};

/// Where the parish library is in its life.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryState {
    /// The database is being opened and read.
    Loading,
    /// The library is hydrated and readable.
    Ready,
    /// The library could not be opened; the message says why.
    Unavailable(SharedString),
}

/// What observers of a [`LibraryStore`] are told.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryEvent {
    /// The state or the library changed; recompute anything derived from it.
    Changed,
}

/// The one app-level owner of the parish [`Storage`] handle.
///
/// It is created by the runtime, outlives every window and entity, and is
/// shut down by [`StorageSlot::shutdown_on_quit`], so the worker is stopped on
/// quit even while the library is still loading or its window is already gone.
#[derive(Clone, Default)]
pub struct StorageSlot(Rc<RefCell<Option<Storage>>>);

impl StorageSlot {
    /// An empty slot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a storage handle is currently held.
    pub fn is_open(&self) -> bool {
        self.0.borrow().is_some()
    }

    /// Runs `f` over the held storage handle, or returns `None` when the slot
    /// is empty (never opened, or already shut down).
    pub fn with<R>(&self, f: impl FnOnce(&Storage) -> R) -> Option<R> {
        self.0.borrow().as_ref().map(f)
    }

    pub(crate) fn put(&self, storage: Storage) {
        *self.0.borrow_mut() = Some(storage);
    }

    /// Takes the handle out and stops its worker. `None` when nothing was
    /// held; otherwise the worker's exit outcome.
    pub fn shutdown(&self) -> impl Future<Output = Option<ShutdownOutcome>> + use<> {
        let storage = self.0.borrow_mut().take();
        async move {
            match storage {
                Some(storage) => Some(storage.shutdown().await),
                None => None,
            }
        }
    }

    /// Registers the app-level quit handler that shuts the storage down. GPUI
    /// polls it for a short grace period, ample for the worker to finish.
    pub fn shutdown_on_quit(&self, cx: &App) {
        let slot = self.clone();
        cx.on_app_quit(move |_| {
            let outcome = slot.shutdown();
            async move {
                if let Some(outcome) = outcome.await
                    && outcome != ShutdownOutcome::Clean
                {
                    eprintln!("parish storage shutdown: {outcome:?}");
                }
            }
        })
        .detach();
    }
}

/// The parish store handle and the library hydrated from it.
pub struct LibraryStore {
    state: LibraryState,
    library: SongLibrary,
    /// The app-level owner of the database handle. The slot, not this entity,
    /// shuts the worker down.
    storage: StorageSlot,
    /// Library writes (Choir and Song alike) waiting for the one in flight to
    /// finish, in submission order.
    queue: VecDeque<(LibraryWrite, Reply)>,
    /// Whether a write is currently in flight.
    writing: bool,
    /// Bumped whenever the held library changes, so a verdict computed on a
    /// snapshot can tell the library moved before its write starts.
    generation: u64,
}

/// What a finished write resolves to.
#[derive(Clone)]
enum Done {
    /// The id of the record the write created or changed.
    Id(EntityId),
    /// How many revisions a permanent deletion erased.
    Count(usize),
    /// What an import commit or Markdown save wrote.
    Imported(ImportOutcome),
}

type Reply = async_channel::Sender<Result<Done, SharedString>>;

/// The refusal for a permanent deletion that something now blocks. The
/// lifecycle UI keys on this exact text (`is_deletion_blocked`) to redraw the
/// blocked dialog, so a copy edit here must keep the two in step.
const DELETION_BLOCKED: &str = "Something now uses it, so it can’t be deleted.";
/// Whether `message` is the refusal for a deletion that something now blocks.
pub(crate) fn is_deletion_blocked(message: &str) -> bool {
    message == DELETION_BLOCKED
}
/// The refusal for a restore over a head that moved on.
const STALE_REVISION: &str = "This changed since you opened History. Reopen it to restore.";
/// The refusal for archiving what is already archived.
const ALREADY_ARCHIVED: &str = "It’s already archived.";
/// The refusal for deleting what is not archived.
pub(crate) const NOT_ARCHIVED: &str = "Archive it first.";
/// The refusal for restoring what is not archived.
const NOT_ARCHIVED_TO_RESTORE: &str = "It isn’t archived.";
/// The refusal for restoring a revision of an archived entity.
const RESTORE_FIRST: &str = "Restore it before restoring a revision.";
/// Every other storage failure of a lifecycle or History write.
const COULDNT_SAVE: &str = "Couldn't save.";
/// A failed History read.
const COULDNT_OPEN_HISTORY: &str = "Couldn't open History.";
/// A failed read of what still uses an entity.
pub(crate) const COULDNT_CHECK_USES: &str = "Couldn't check what uses it.";

/// One change the store can make. Choir and Song writes share one queue, so
/// the order they were submitted in is the order they run in.
enum LibraryWrite {
    Create(ChoirDraft),
    Edit(ChoirId, ChoirDraft),
    Archive(ChoirId),
    Restore(ChoirId),
    CreateSong(SongDraft),
    /// A new Arrangement of a Song, with no sections yet.
    CreateArrangement(SongId, ArrangementDetails),
    /// Replaces an Arrangement's details, keeping its sections and Sequence.
    SaveArrangementDetails(ArrangementId, ArrangementDetails),
    /// Replaces an Arrangement's sections, Sequence and each section's
    /// pagination state as one Edit revision of the given History session.
    SaveArrangementLyrics(ArrangementId, ArrangementLyrics, HistorySessionId),
    /// Copies the stored Arrangement into a new one under the same Song.
    DuplicateArrangement(ArrangementId),
    /// Archives a Song without touching its Arrangements.
    ArchiveSong(SongId),
    /// Restores an archived Song.
    RestoreSong(SongId),
    /// Archives an Arrangement in its own right.
    ArchiveArrangement(ArrangementId),
    /// Restores an archived Arrangement.
    RestoreArrangement(ArrangementId),
    /// Permanently deletes an archived, unreferenced Song that owns nothing.
    DeleteSong(SongId),
    /// Permanently deletes an archived, unreferenced Arrangement.
    DeleteArrangement(ArrangementId),
    /// Restores `revision` of `entity` over `head` as one restoration.
    RestoreRevision {
        revision: RevisionId,
        head: RevisionId,
        entity: RevisionEntity,
    },
    /// An approved import or a Markdown save: the plan is built when the
    /// write reaches the front of the queue, so it never interleaves with an
    /// autosave.
    CommitImport(Box<import::ImportRequest>),
}

impl LibraryWrite {
    /// Whether storage failures of this write map to operator copy rather than
    /// the storage text (the lifecycle, deletion and History writes).
    fn has_operator_copy(&self) -> bool {
        matches!(
            self,
            Self::ArchiveSong(_)
                | Self::RestoreSong(_)
                | Self::ArchiveArrangement(_)
                | Self::RestoreArrangement(_)
                | Self::DeleteSong(_)
                | Self::DeleteArrangement(_)
                | Self::RestoreRevision { .. }
        )
    }
}

/// What storage returned for a write.
enum Stored {
    Choir(ChoirRecord),
    Song(SongRecord),
    Arrangement(ArrangementRecord),
    /// A permanent deletion: how many revisions went with it.
    Deleted(usize),
    /// A restored revision: the new head and the content storage wrote.
    Restored(RevisionId, Restored),
    /// What an import batch wrote, exactly.
    Imported(Box<ImportCommittedRecords>),
}

/// The entity a revision restore rewrote, as the restoration wrote it. Memory
/// is built from this, not from a second read, so a failing read cannot hide a
/// restoration that committed.
enum Restored {
    Song(SongContent),
    Arrangement(ArrangementContent),
}

/// What validation decided for a write.
enum Verdict {
    /// The write changes nothing, so there is no revision, no write and no
    /// event: the task resolves to this id.
    Unchanged(EntityId),
    /// The storage call to make.
    Write(Prepared),
    /// The write resolved to this without any storage call (an import with
    /// nothing to write).
    Resolved(Done),
}

/// The storage call for a write, built from the domain-checked values.
/// Dispatch is over this enum alone, so every case is handled and none can be
/// "not prepared".
enum Prepared {
    CreateSong(Box<NewSong>),
    CreateChoir(NewChoir),
    EditChoir(ChoirId, ChoirDetails),
    ArchiveChoir(ChoirId),
    RestoreChoir(ChoirId),
    CreateArrangement(Box<NewArrangement>),
    /// An atomic metadata-only commit: storage reads the sections and
    /// Sequence in the same transaction, so memory never supplies them.
    SaveMetadata(ArrangementId, Box<ArrangementMetadata>),
    /// An atomic lyrics-only commit: storage reads the metadata in the same
    /// transaction, so memory never supplies it.
    SaveLyrics(ArrangementId, Box<ArrangementLyrics>, HistorySessionId),
    Copy(Box<NewArrangement>),
    ArchiveSong(SongId),
    RestoreSong(SongId),
    ArchiveArrangement(ArrangementId),
    RestoreArrangement(ArrangementId),
    DeleteSong(SongId),
    DeleteArrangement(ArrangementId),
    RestoreRevision(RevisionId, RevisionId),
    /// One import batch, written in a single storage transaction.
    CommitImport(Box<ImportBatch>),
}

/// What a reference-safety review found, for the deletion dialog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionReview {
    /// The domain verdict on the current library with the stored references.
    pub verdict: DeletionVerdict,
    /// The verdict's blockers grouped for display; empty when allowed.
    pub groups: Vec<BlockerGroup>,
    /// What storage read: references, children and the counts the dialog shows.
    pub facts: DeletionFacts,
}

/// The text shown when storage returned a record the write cannot use.
const WRONG_KIND: &str = "The parish store returned a record of the wrong kind.";

/// The stored archive stamp; storage always stamps an archived record, so a
/// missing one is a storage fault, not something to paper over.
fn archived_stamp(at: Option<Timestamp>, kind: &str) -> Result<Timestamp, SharedString> {
    at.ok_or_else(|| {
        SharedString::from(format!(
            "The parish store returned an archived {kind} without a date."
        ))
    })
}

/// Reads the erased-revision count a deletion resolved to.
fn count_of(done: Done) -> Result<usize, SharedString> {
    match done {
        Done::Count(revisions) => Ok(revisions),
        Done::Id(_) | Done::Imported(_) => Err(SharedString::from(WRONG_KIND)),
    }
}

/// The text shown when a write cannot reach the parish store.
const NOT_OPEN: &str = "The parish store is not open.";

/// A boxed storage future, so every write shares one type.
type StoredWrite = Pin<Box<dyn Future<Output = Result<Stored, StorageError>> + Send>>;

fn boxed(
    future: impl Future<Output = Result<ChoirRecord, StorageError>> + Send + 'static,
) -> StoredWrite {
    Box::pin(async move { future.await.map(Stored::Choir) })
}

fn boxed_song(
    future: impl Future<Output = Result<SongRecord, StorageError>> + Send + 'static,
) -> StoredWrite {
    Box::pin(async move { future.await.map(Stored::Song) })
}

fn boxed_arrangement(
    future: impl Future<Output = Result<ArrangementRecord, StorageError>> + Send + 'static,
) -> StoredWrite {
    Box::pin(async move { future.await.map(Stored::Arrangement) })
}

fn refusal(error: SongLibraryError) -> SharedString {
    error.to_string().into()
}

/// The operator copy for a refused lifecycle write: fixed strings for the
/// refusals the Interface names, the domain's own text otherwise.
fn lifecycle_refusal(error: SongLibraryError) -> SharedString {
    match error {
        SongLibraryError::AlreadyArchived(_) => ALREADY_ARCHIVED.into(),
        SongLibraryError::NotArchived(_) => NOT_ARCHIVED_TO_RESTORE.into(),
        SongLibraryError::DeletionBlocked(blockers) => blocked_copy(&blockers),
        other => refusal(other),
    }
}

/// The copy for a deletion the verdict blocks: archive first when that is the
/// blocker, otherwise something still uses it.
fn blocked_copy(blockers: &[DeletionBlocker]) -> SharedString {
    if blockers.contains(&DeletionBlocker::NotArchived) {
        NOT_ARCHIVED.into()
    } else {
        DELETION_BLOCKED.into()
    }
}

/// The operator copy for a storage failure of a lifecycle, deletion or
/// History call; `fallback` is the text for a failure the Interface does not
/// name, which differs for a write and for a read.
fn storage_copy(error: &StorageError, fallback: &str) -> String {
    match error {
        StorageError::DeletionBlocked { .. } => DELETION_BLOCKED,
        StorageError::StaleRevision { .. } => STALE_REVISION,
        StorageError::AlreadyArchived { .. } => ALREADY_ARCHIVED,
        StorageError::NotArchived { .. } => NOT_ARCHIVED,
        _ => fallback,
    }
    .to_owned()
}

/// The text shown when a write's task was dropped before it finished.
const ABANDONED: &str = "The change was abandoned.";

/// Checks a draft against the domain's own rules without touching the library.
fn check_draft(draft: &ChoirDraft) -> Result<(), SharedString> {
    let probe = ChoirId::from_entity(EntityId::from_random_bytes([1; 16]));
    SongLibrary::new()
        .create_choir(probe, Timestamp::from_millis(0), draft.clone())
        .map(|_| ())
        .map_err(refusal)
}

/// Checks a Song draft against the domain's own rules without touching the
/// library. An exact duplicate title is not a refusal: the operator decides.
fn check_song_draft(draft: &SongDraft) -> Result<(), SharedString> {
    let probe = SongId::from_entity(EntityId::from_random_bytes([1; 16]));
    SongLibrary::new()
        .create_song(probe, Timestamp::from_millis(0), draft.clone())
        .map(|_| ())
        .map_err(refusal)
}

/// Checks Arrangement details against the domain's own rules on a scratch
/// library holding only a probe Song, the named Choirs and (for an existing
/// Arrangement) a probe holding its current languages and Choirs, so the cost
/// does not grow with the parish library and a language the Arrangement
/// already has may be kept. Returns the details with the languages under their
/// display names, the form storage must be given.
fn check_arrangement_details(
    details: &ArrangementDetails,
    existing: Option<&Arrangement>,
) -> Result<ArrangementDetails, SongLibraryError> {
    let at = Timestamp::from_millis(0);
    let song = SongId::from_entity(EntityId::from_random_bytes([1; 16]));
    let held_choirs: Vec<ChoirId> = existing
        .map(|arrangement| arrangement.choirs().iter().copied().collect())
        .unwrap_or_default();
    let mut named: Vec<ChoirId> = Vec::new();
    for choir in held_choirs.iter().chain(&details.choirs) {
        if !named.contains(choir) {
            named.push(*choir);
        }
    }
    let probe = ArrangementId::from_entity(EntityId::from_random_bytes([2; 16]));
    let held = HydrateArrangement {
        id: probe,
        song,
        created_at: at,
        draft: ArrangementDraft {
            languages: existing.map(|a| a.languages().to_vec()).unwrap_or_default(),
            ..ArrangementDraft::default()
        },
        choirs: held_choirs,
        archive_state: ArchiveState::Active,
    };
    let mut scratch = SongLibrary::hydrate(
        vec![HydrateSong {
            id: song,
            created_at: at,
            draft: SongDraft::titled("probe"),
            archive_state: ArchiveState::Active,
        }],
        vec![held],
        named
            .iter()
            .enumerate()
            .map(|(index, id)| HydrateChoir {
                id: *id,
                created_at: at,
                draft: ChoirDraft::named(format!("probe {index}")),
                archive_state: ArchiveState::Active,
            })
            .collect(),
        Vec::new(),
    )
    .map_err(|error| error.cause)?;
    scratch
        .edit_arrangement_details(probe, details.clone())
        .map(Arrangement::details)
}

fn arrangement_draft_of(record: &ArrangementRecord) -> ArrangementDraft {
    ArrangementDraft {
        title: record.title.clone(),
        languages: record.languages.clone(),
        rendition_attribution: record.rendition_attribution.clone(),
        occasions: record.occasions.clone(),
        notes: record.notes.clone(),
        sections: record
            .sections
            .iter()
            .map(|section| SectionDraft {
                id: section.id,
                label: section.label.clone(),
                lyrics: section.lyrics.clone(),
                pagination: section.pagination,
            })
            .collect(),
        sequence: record
            .sequence
            .iter()
            .map(|occurrence| OccurrenceDraft::new(occurrence.id, occurrence.section))
            .collect(),
    }
}

/// What storage must be given to copy `source`: its stored metadata, sections
/// and Sequence, without its archived Choirs. Identities are storage's to
/// generate.
fn copy_of(library: &SongLibrary, source: &Arrangement) -> NewArrangement {
    let positions: std::collections::BTreeMap<_, _> = source
        .sections()
        .iter()
        .enumerate()
        .map(|(index, section)| (section.id(), index))
        .collect();
    NewArrangement {
        song: source.song(),
        title: source.title().map(str::to_owned),
        languages: source.languages().to_vec(),
        choirs: source
            .choirs()
            .iter()
            .copied()
            .filter(|choir| library.choir(*choir).is_ok_and(|c| !c.is_archived()))
            .collect(),
        rendition_attribution: source.rendition_attribution().map(str::to_owned),
        occasions: source.occasions().iter().cloned().collect(),
        notes: source.notes().map(str::to_owned),
        sections: source
            .sections()
            .iter()
            .map(|section| NewSection {
                label: section.label().map(str::to_owned),
                lyrics: section.lyrics().to_owned(),
            })
            .collect(),
        sequence: source
            .sequence()
            .iter()
            .filter_map(|occurrence| positions.get(&occurrence.section()).copied())
            .collect(),
    }
}

/// A fresh History-session id from OS randomness: each details Save is its own
/// session.
fn fresh_session() -> HistorySessionId {
    HistorySessionId::from_entity(EntityId::from_random_bytes(
        *uuid::Uuid::new_v4().as_bytes(),
    ))
}

fn song_draft_of(record: &SongRecord) -> SongDraft {
    SongDraft {
        title: record.title.clone(),
        aliases: record.aliases.clone(),
        original_attribution: record.original_attribution.clone(),
        provenance: record.provenance.clone(),
        rights_notes: record.rights_notes.clone(),
        liturgical_moments: record.liturgical_moments.clone(),
        occasions: record.occasions.clone(),
    }
}

/// The Song draft a restoration wrote.
fn song_draft_from_content(content: &SongContent) -> SongDraft {
    SongDraft {
        title: content.title.clone(),
        aliases: content.aliases.clone(),
        original_attribution: content.original_attribution.clone(),
        provenance: content.provenance.clone(),
        rights_notes: content.rights_notes.clone(),
        liturgical_moments: content.liturgical_moments.clone(),
        occasions: content.occasions.clone(),
    }
}

/// The Arrangement draft a restoration wrote.
fn arrangement_draft_from_content(content: &ArrangementContent) -> ArrangementDraft {
    ArrangementDraft {
        title: content.title.clone(),
        languages: content.languages.clone(),
        rendition_attribution: content.rendition_attribution.clone(),
        occasions: content.occasions.clone(),
        notes: content.notes.clone(),
        sections: content
            .sections
            .iter()
            .map(|section| SectionDraft {
                id: section.id,
                label: section.label.clone(),
                lyrics: section.lyrics.clone(),
                pagination: section.pagination,
            })
            .collect(),
        sequence: content
            .sequence
            .iter()
            .map(|occurrence| OccurrenceDraft::new(occurrence.id, occurrence.section))
            .collect(),
    }
}

fn draft_of(record: &ChoirRecord) -> ChoirDraft {
    ChoirDraft {
        name: record.name.clone(),
        short_label: record.short_label.clone(),
        languages: record.languages.clone(),
        notes: record.notes.clone(),
    }
}

impl EventEmitter<LibraryEvent> for LibraryStore {}

impl LibraryStore {
    /// A store over `directory`, created if missing. It starts `Loading` and
    /// settles on a background task. The database handle goes into `slot` the
    /// moment it exists, so a quit at any point stops the worker.
    pub fn open(directory: PathBuf, slot: StorageSlot, cx: &mut Context<Self>) -> Self {
        let owner = slot.clone();
        cx.spawn(async move |this, cx| {
            let created = cx
                .background_spawn(async move {
                    std::fs::create_dir_all(&directory)
                        .map(|()| directory)
                        .map_err(|error| {
                            format!("The parish folder could not be created: {error}.")
                        })
                })
                .await;
            let opened = match created {
                Ok(directory) => {
                    let storage = Storage::spawn(StorageConfig::in_directory(&directory));
                    let ready = storage.ready();
                    let records = storage.library();
                    owner.put(storage);
                    let loaded = cx
                        .background_spawn(async move {
                            ready.await.map_err(|error| error.to_string())?;
                            let records = records.await.map_err(|error| error.to_string())?;
                            records.hydrate().map_err(|error| error.to_string())
                        })
                        .await;
                    if loaded.is_err() {
                        let _ = owner.shutdown().await;
                    }
                    loaded
                }
                Err(message) => Err(message),
            };
            let _ = this.update(cx, |this, cx| {
                match opened {
                    Ok(library) => {
                        this.library = library;
                        this.generation += 1;
                        this.state = LibraryState::Ready;
                    }
                    Err(message) => this.state = LibraryState::Unavailable(message.into()),
                }
                cx.emit(LibraryEvent::Changed);
                cx.notify();
            });
        })
        .detach();
        Self {
            state: LibraryState::Loading,
            library: SongLibrary::new(),
            storage: slot,
            queue: VecDeque::new(),
            writing: false,
            generation: 0,
        }
    }

    /// A store that stays `Loading`, for tests of that state.
    #[cfg(test)]
    pub(crate) fn loading(_cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Loading,
            library: SongLibrary::new(),
            storage: StorageSlot::new(),
            queue: VecDeque::new(),
            writing: false,
            generation: 0,
        }
    }

    /// A ready store over an in-memory library with no database, for views
    /// and tests.
    pub fn with_library(library: SongLibrary, _cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Ready,
            library,
            storage: StorageSlot::new(),
            queue: VecDeque::new(),
            writing: false,
            generation: 0,
        }
    }

    /// A store that is permanently unavailable, with the reason shown to the
    /// operator.
    pub fn unavailable(message: impl Into<SharedString>, _cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Unavailable(message.into()),
            library: SongLibrary::new(),
            storage: StorageSlot::new(),
            queue: VecDeque::new(),
            writing: false,
            generation: 0,
        }
    }

    /// Where the library is in its life.
    pub fn state(&self) -> &LibraryState {
        &self.state
    }

    /// The hydrated library. Empty until the state is `Ready`.
    pub fn library(&self) -> &SongLibrary {
        &self.library
    }

    /// Creates a Choir. The domain checks the draft first (a refusal does no
    /// I/O), the write runs off the GPUI thread, and only then does the
    /// returned record enter the in-memory library. The new id is the one
    /// storage generated.
    pub fn create_choir(
        &mut self,
        draft: ChoirDraft,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(LibraryWrite::Create(draft), ChoirId::from_entity, cx)
    }

    /// Creates a Song with no Arrangement, through the same queue as the
    /// Choir writes. The domain checks the draft first (a draft the domain
    /// refuses is refused without I/O), the write runs off the GPUI thread, and only then does the returned
    /// record enter the in-memory library under the id and timestamp storage
    /// gave it. An exact duplicate title is allowed: the operator decides.
    pub fn create_song(
        &mut self,
        draft: SongDraft,
        cx: &mut Context<Self>,
    ) -> Task<Result<SongId, SharedString>> {
        self.submit(LibraryWrite::CreateSong(draft), SongId::from_entity, cx)
    }

    /// Creates an Arrangement of `song` from its details, with no sections
    /// yet. The domain checks the details first (no language, a blank title,
    /// an unknown or archived Song or Choir are refused without I/O), the write
    /// runs off the GPUI thread, and only then does the returned record enter
    /// the in-memory library under the id and time storage gave it.
    pub fn create_arrangement(
        &mut self,
        song: SongId,
        details: ArrangementDetails,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        self.submit(
            LibraryWrite::CreateArrangement(song, details),
            ArrangementId::from_entity,
            cx,
        )
    }

    /// Replaces an Arrangement's details as one History session's `Edit`
    /// revision, leaving its sections, Sequence and pagination state as they
    /// are. It is an atomic metadata-only commit: storage reads the head
    /// revision, sections and Sequence in the same transaction and returns the
    /// whole stored Arrangement, which memory then takes, so a Save never
    /// overwrites newer sections. Details equal to the stored ones write
    /// nothing and resolve to the id. A storage error leaves memory unchanged.
    pub fn save_arrangement_details(
        &mut self,
        id: ArrangementId,
        details: ArrangementDetails,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        self.submit(
            LibraryWrite::SaveArrangementDetails(id, details),
            ArrangementId::from_entity,
            cx,
        )
    }

    /// Replaces an Arrangement's sections, Sequence and each section's
    /// pagination state as one History session's `Edit` revision, leaving its
    /// metadata and Choirs as stored. It is an atomic lyrics-only commit, the
    /// mirror of [`Self::save_arrangement_details`]: storage reads the head
    /// revision and the metadata in the same transaction and returns the whole
    /// stored Arrangement, which memory then takes, so a lyrics autosave never
    /// overwrites newer details. The domain checks the edit first on the whole
    /// library (a blank section name or a foreign identity is refused without
    /// I/O). Lyrics equal to the stored ones write nothing and resolve to the
    /// id. A storage error leaves memory unchanged.
    pub fn save_arrangement_lyrics(
        &mut self,
        id: ArrangementId,
        lyrics: ArrangementLyrics,
        session: HistorySessionId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        self.submit(
            LibraryWrite::SaveArrangementLyrics(id, lyrics, session),
            ArrangementId::from_entity,
            cx,
        )
    }

    /// Copies the stored Arrangement (not any staged edit) into a new one under
    /// the same Song, in one storage transaction: new Arrangement, section and
    /// occurrence identities, Draft pagination and a fresh Creation revision.
    /// Archived Choirs are dropped from the copy; an archived Song is refused.
    /// The task resolves to the new Arrangement's id.
    pub fn duplicate_arrangement(
        &mut self,
        source: ArrangementId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        self.submit(
            LibraryWrite::DuplicateArrangement(source),
            ArrangementId::from_entity,
            cx,
        )
    }

    /// Replaces a Choir's details; an archived Choir is refused.
    pub fn edit_choir(
        &mut self,
        id: ChoirId,
        draft: ChoirDraft,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(LibraryWrite::Edit(id, draft), ChoirId::from_entity, cx)
    }

    /// Archives a Choir.
    pub fn archive_choir(
        &mut self,
        id: ChoirId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(LibraryWrite::Archive(id), ChoirId::from_entity, cx)
    }

    /// Restores an archived Choir.
    pub fn restore_choir(
        &mut self,
        id: ChoirId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(LibraryWrite::Restore(id), ChoirId::from_entity, cx)
    }

    /// Archives a Song work. Its Arrangements keep their own states.
    pub fn archive_song(
        &mut self,
        id: SongId,
        cx: &mut Context<Self>,
    ) -> Task<Result<SongId, SharedString>> {
        self.submit(LibraryWrite::ArchiveSong(id), SongId::from_entity, cx)
    }

    /// Restores an archived Song work.
    pub fn restore_song(
        &mut self,
        id: SongId,
        cx: &mut Context<Self>,
    ) -> Task<Result<SongId, SharedString>> {
        self.submit(LibraryWrite::RestoreSong(id), SongId::from_entity, cx)
    }

    /// Archives an Arrangement in its own right.
    pub fn archive_arrangement(
        &mut self,
        id: ArrangementId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        self.submit(
            LibraryWrite::ArchiveArrangement(id),
            ArrangementId::from_entity,
            cx,
        )
    }

    /// Restores an archived Arrangement.
    pub fn restore_arrangement(
        &mut self,
        id: ArrangementId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        self.submit(
            LibraryWrite::RestoreArrangement(id),
            ArrangementId::from_entity,
            cx,
        )
    }

    /// Permanently deletes an archived, unreferenced Song that owns no
    /// Arrangement; resolves to how many revisions went with it.
    pub fn delete_song(
        &mut self,
        id: SongId,
        cx: &mut Context<Self>,
    ) -> Task<Result<usize, SharedString>> {
        self.submit_done(LibraryWrite::DeleteSong(id), count_of, cx)
    }

    /// Permanently deletes an archived, unreferenced Arrangement; resolves to
    /// how many revisions went with it.
    pub fn delete_arrangement(
        &mut self,
        id: ArrangementId,
        cx: &mut Context<Self>,
    ) -> Task<Result<usize, SharedString>> {
        self.submit_done(LibraryWrite::DeleteArrangement(id), count_of, cx)
    }

    /// Reads what still uses `target` off the GPUI thread, then judges it: the
    /// domain verdict on the current library with the stored references, and
    /// the blockers grouped for the reference-safety dialog.
    pub fn deletion_review(
        &mut self,
        target: RecordRef,
        cx: &mut Context<Self>,
    ) -> Task<Result<DeletionReview, SharedString>> {
        if self.state != LibraryState::Ready {
            return Task::ready(Err(NOT_OPEN.into()));
        }
        let Some(read) = self.storage.with(|storage| storage.deletion_facts(target)) else {
            return Task::ready(Err(NOT_OPEN.into()));
        };
        cx.spawn(async move |this, cx| {
            let facts = cx
                .background_spawn(async move {
                    read.await.map_err(|error| {
                        SharedString::from(storage_copy(&error, COULDNT_CHECK_USES))
                    })
                })
                .await?;
            this.update(cx, |this, _| this.review(target, facts))
                .unwrap_or_else(|_| Err(SharedString::from(ABANDONED)))
        })
    }

    /// The domain verdict for `target` with the facts storage just read.
    fn review(
        &self,
        target: RecordRef,
        facts: DeletionFacts,
    ) -> Result<DeletionReview, SharedString> {
        let verdict = match target {
            RecordRef::Song(id) => self.library.song_deletion_verdict(id, &facts.references),
            RecordRef::Arrangement(id) => self
                .library
                .arrangement_deletion_verdict(id, &facts.references),
            RecordRef::Choir(_) => return Err(COULDNT_SAVE.into()),
        }
        .map_err(refusal)?;
        let groups = match &verdict {
            DeletionVerdict::Allowed => Vec::new(),
            DeletionVerdict::Blocked(blockers) => blocker_groups(blockers),
        };
        Ok(DeletionReview {
            verdict,
            groups,
            facts,
        })
    }

    /// An entity's History, oldest first, with each revision's verified
    /// content. A T2.1-era entity with no revision first gets its creation
    /// revision backfilled, so History is never empty for a stored entity.
    pub fn history(
        &mut self,
        entity: RevisionEntity,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<HistoryRevision>, SharedString>> {
        if self.state != LibraryState::Ready {
            return Task::ready(Err(NOT_OPEN.into()));
        }
        let Some(reads) = self
            .storage
            .with(|storage| (storage.latest_revision(entity), storage.history(entity)))
        else {
            return Task::ready(Err(NOT_OPEN.into()));
        };
        cx.background_spawn(async move {
            let (backfill, history) = reads;
            backfill
                .await
                .map_err(|error| SharedString::from(storage_copy(&error, COULDNT_OPEN_HISTORY)))?;
            history
                .await
                .map_err(|error| SharedString::from(storage_copy(&error, COULDNT_OPEN_HISTORY)))
        })
    }

    /// Restores `revision` over `head` as one restoration revision and
    /// refreshes the entity in memory from what storage then holds. Resolves
    /// to the new head. Refused when the entity is archived (restore it
    /// first) or `head` is no longer the entity's latest revision.
    pub fn restore_revision(
        &mut self,
        revision: RevisionId,
        head: RevisionId,
        cx: &mut Context<Self>,
    ) -> Task<Result<RevisionId, SharedString>> {
        if self.state != LibraryState::Ready {
            return Task::ready(Err(NOT_OPEN.into()));
        }
        let Some(lookup) = self
            .storage
            .with(|storage| storage.revision_entity(revision))
        else {
            return Task::ready(Err(NOT_OPEN.into()));
        };
        cx.spawn(async move |this, cx| {
            let entity = cx
                .background_spawn(async move {
                    lookup.await.map_err(|error| {
                        SharedString::from(storage_copy(&error, COULDNT_OPEN_HISTORY))
                    })
                })
                .await?;
            let write = LibraryWrite::RestoreRevision {
                revision,
                head,
                entity,
            };
            let task = this
                .update(cx, |this, cx| {
                    this.submit(write, RevisionId::from_entity, cx)
                })
                .map_err(|_| SharedString::from(ABANDONED))?;
            task.await
        })
    }

    fn submit<T: 'static>(
        &mut self,
        write: LibraryWrite,
        convert: fn(EntityId) -> T,
        cx: &mut Context<Self>,
    ) -> Task<Result<T, SharedString>> {
        self.submit_done(
            write,
            move |done| match done {
                Done::Id(id) => Ok(convert(id)),
                Done::Count(_) | Done::Imported(_) => Err(SharedString::from(WRONG_KIND)),
            },
            cx,
        )
    }

    /// Queues a write and converts what it resolves to.
    fn submit_done<T: 'static>(
        &mut self,
        write: LibraryWrite,
        convert: impl FnOnce(Done) -> Result<T, SharedString> + 'static,
        cx: &mut Context<Self>,
    ) -> Task<Result<T, SharedString>> {
        let (reply, result) = async_channel::bounded(1);
        self.queue.push_back((write, reply));
        self.pump(cx);
        cx.spawn(async move |_, _| {
            let outcome = result.recv().await;
            outcome
                .unwrap_or_else(|_| Err(SharedString::from(ABANDONED)))
                .and_then(convert)
        })
    }

    /// Starts the next queued write when none is in flight.
    fn pump(&mut self, cx: &mut Context<Self>) {
        while !self.writing {
            let Some((mut write, reply)) = self.queue.pop_front() else {
                return;
            };
            if self.state != LibraryState::Ready {
                let _ = reply.try_send(Err(NOT_OPEN.into()));
                continue;
            }
            if matches!(&write, LibraryWrite::CommitImport(request) if request.is_batch()) {
                // The batch verdict is off the GPUI thread; it sets `writing`.
                self.verdict_in_background(write, reply, cx);
                continue;
            }
            let prepared = match self.validate(&mut write) {
                Ok(Verdict::Write(prepared)) => prepared,
                Ok(Verdict::Unchanged(id)) => {
                    let _ = reply.try_send(Ok(Done::Id(id)));
                    continue;
                }
                Ok(Verdict::Resolved(done)) => {
                    let _ = reply.try_send(Ok(done));
                    continue;
                }
                Err(message) => {
                    let _ = reply.try_send(Err(message));
                    continue;
                }
            };
            self.dispatch(write, reply, prepared, cx);
        }
    }

    /// Starts the storage call for a validated write, and applies its result
    /// when it lands. The write holds the queue (`writing`) until then.
    fn dispatch(
        &mut self,
        write: LibraryWrite,
        reply: Reply,
        prepared: Prepared,
        cx: &mut Context<Self>,
    ) {
        let started = self.storage.with(|storage| match prepared {
            Prepared::CreateSong(new) => boxed_song(storage.create_song(*new)),
            Prepared::CreateChoir(new) => boxed(storage.create_choir(new)),
            Prepared::EditChoir(id, details) => boxed(storage.edit_choir(id, details)),
            Prepared::ArchiveChoir(id) => boxed(storage.archive_choir(id)),
            Prepared::RestoreChoir(id) => boxed(storage.restore_choir(id)),
            Prepared::CreateArrangement(new) | Prepared::Copy(new) => {
                boxed_arrangement(storage.create_arrangement(*new))
            }
            Prepared::ArchiveSong(id) => boxed_song(storage.archive_song(id)),
            Prepared::RestoreSong(id) => boxed_song(storage.restore_song(id)),
            Prepared::ArchiveArrangement(id) => boxed_arrangement(storage.archive_arrangement(id)),
            Prepared::RestoreArrangement(id) => boxed_arrangement(storage.restore_arrangement(id)),
            Prepared::DeleteSong(id) => {
                let delete = storage.delete_song(id);
                Box::pin(async move {
                    delete
                        .await
                        .map(|deleted| Stored::Deleted(deleted.revisions))
                })
            }
            Prepared::DeleteArrangement(id) => {
                let delete = storage.delete_arrangement(id);
                Box::pin(async move {
                    delete
                        .await
                        .map(|deleted| Stored::Deleted(deleted.revisions))
                })
            }
            Prepared::CommitImport(batch) => {
                let commit = storage.commit_import(*batch);
                let write: StoredWrite =
                    Box::pin(async move { commit.await.map(|r| Stored::Imported(Box::new(r))) });
                write
            }
            Prepared::RestoreRevision(revision, head) => {
                let restore = storage.restore_revision(revision, head);
                let write: StoredWrite = Box::pin(async move {
                    let (content, record) = restore.await?;
                    let restored = match content {
                        RevisionContent::Song(song) => Restored::Song(song),
                        RevisionContent::Arrangement(arrangement) => {
                            Restored::Arrangement(arrangement)
                        }
                    };
                    Ok(Stored::Restored(record.id, restored))
                });
                write
            }
            Prepared::SaveMetadata(id, metadata) => {
                let commit = storage.commit_arrangement_metadata(id, *metadata, fresh_session());
                boxed_arrangement(async move { commit.await.map(|(record, _)| record) })
            }
            Prepared::SaveLyrics(id, lyrics, session) => {
                let ArrangementLyrics { sections, sequence } = *lyrics;
                let sections = sections
                    .into_iter()
                    .map(|section| SectionContent {
                        id: section.id,
                        label: section.label,
                        lyrics: section.lyrics,
                        pagination: section.pagination,
                    })
                    .collect();
                let sequence = sequence
                    .into_iter()
                    .map(|occurrence| OccurrenceContent {
                        id: occurrence.id,
                        section: occurrence.section,
                    })
                    .collect();
                let commit = storage.commit_arrangement_lyrics(id, sections, sequence, session);
                boxed_arrangement(async move { commit.await.map(|(record, _)| record) })
            }
        });
        let Some(started) = started else {
            let _ = reply.try_send(Err(NOT_OPEN.into()));
            return;
        };
        self.writing = true;
        let operator_copy = write.has_operator_copy();
        let importing = matches!(write, LibraryWrite::CommitImport(_));
        let restoring = matches!(
            write,
            LibraryWrite::Restore(_)
                | LibraryWrite::RestoreSong(_)
                | LibraryWrite::RestoreArrangement(_)
        );
        cx.spawn(async move |this, cx| {
            let stored = cx
                .background_spawn(async move {
                    started.await.map_err(|error| {
                        if importing {
                            import::import_copy(&error).to_owned()
                        } else if restoring && matches!(error, StorageError::NotArchived { .. }) {
                            NOT_ARCHIVED_TO_RESTORE.to_owned()
                        } else if operator_copy {
                            storage_copy(&error, COULDNT_SAVE)
                        } else {
                            error.to_string()
                        }
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let outcome = match stored {
                    Ok(record) => this.apply(&write, &record),
                    Err(message) => Err(SharedString::from(message)),
                };
                this.generation += 1;
                this.writing = false;
                if outcome.is_ok() {
                    cx.emit(LibraryEvent::Changed);
                    cx.notify();
                }
                let _ = reply.try_send(outcome);
                this.pump(cx);
            });
        })
        .detach();
    }

    /// The domain's verdict on a write, before any I/O, with the storage call
    /// it needs. An Arrangement write's languages are rewritten to their
    /// display names, the form storage is given.
    fn validate(&self, write: &mut LibraryWrite) -> Result<Verdict, SharedString> {
        let prepared = match write {
            LibraryWrite::CreateArrangement(song, details) => {
                self.check_arrangement_target(*song, None, details)?;
                Prepared::CreateArrangement(Box::new(NewArrangement {
                    song: *song,
                    title: details.title.clone(),
                    languages: details.languages.clone(),
                    choirs: details.choirs.clone(),
                    rendition_attribution: details.rendition_attribution.clone(),
                    occasions: details.occasions.clone(),
                    notes: details.notes.clone(),
                    sections: Vec::new(),
                    sequence: Vec::new(),
                }))
            }
            LibraryWrite::SaveArrangementDetails(id, details) => {
                let arrangement = self.library.arrangement(*id).map_err(refusal)?;
                self.check_arrangement_target(arrangement.song(), Some(arrangement), details)?;
                if details.same_as(&arrangement.details()) {
                    return Ok(Verdict::Unchanged(id.entity()));
                }
                Prepared::SaveMetadata(
                    *id,
                    Box::new(ArrangementMetadata {
                        title: details.title.clone(),
                        languages: details.languages.clone(),
                        choirs: details.choirs.clone(),
                        rendition_attribution: details.rendition_attribution.clone(),
                        occasions: details.occasions.clone(),
                        notes: details.notes.clone(),
                    }),
                )
            }
            LibraryWrite::SaveArrangementLyrics(id, lyrics, session) => {
                let arrangement = self.library.arrangement(*id).map_err(refusal)?;
                self.require_song_active(arrangement.song())?;
                if arrangement.is_archived() {
                    return Err(refusal(SongLibraryError::Archived(RecordRef::Arrangement(
                        *id,
                    ))));
                }
                self.library
                    .check_arrangement_lyrics(*id, lyrics)
                    .map_err(refusal)?;
                if *lyrics == arrangement.lyrics() {
                    return Ok(Verdict::Unchanged(id.entity()));
                }
                Prepared::SaveLyrics(*id, Box::new(lyrics.clone()), *session)
            }
            LibraryWrite::DuplicateArrangement(source) => {
                let arrangement = self.library.arrangement(*source).map_err(refusal)?;
                self.require_song_active(arrangement.song())?;
                // A copy is a creation, so a legacy source without a language
                // is refused as the domain refuses it.
                if arrangement.languages().is_empty() {
                    return Err(refusal(SongLibraryError::BlankField(Field::Language)));
                }
                Prepared::Copy(Box::new(copy_of(&self.library, arrangement)))
            }
            LibraryWrite::Create(draft) => {
                check_draft(draft)?;
                // One Choir per name, however it is cased or spaced, archived
                // Choirs included: the Module refuses a duplicate itself.
                let key = match_key(&draft.name);
                if let Some(existing) = self
                    .library
                    .choirs()
                    .find(|choir| match_key(choir.name()) == key)
                {
                    return Err(SharedString::from(format!(
                        "A Choir named '{}' already exists{}.",
                        existing.name(),
                        if existing.is_archived() {
                            " (archived)"
                        } else {
                            ""
                        }
                    )));
                }
                Prepared::CreateChoir(NewChoir {
                    name: draft.name.clone(),
                    short_label: draft.short_label.clone(),
                    languages: draft.languages.clone(),
                    notes: draft.notes.clone(),
                })
            }
            LibraryWrite::CreateSong(draft) => {
                check_song_draft(draft)?;
                Prepared::CreateSong(Box::new(NewSong {
                    title: draft.title.clone(),
                    aliases: draft.aliases.clone(),
                    original_attribution: draft.original_attribution.clone(),
                    provenance: draft.provenance.clone(),
                    rights_notes: draft.rights_notes.clone(),
                    liturgical_moments: draft.liturgical_moments.clone(),
                    occasions: draft.occasions.clone(),
                }))
            }
            LibraryWrite::Edit(id, draft) => {
                if self.library.choir(*id).map_err(refusal)?.is_archived() {
                    return Err(refusal(SongLibraryError::Archived(RecordRef::Choir(*id))));
                }
                check_draft(draft)?;
                Prepared::EditChoir(
                    *id,
                    ChoirDetails {
                        name: draft.name.clone(),
                        short_label: draft.short_label.clone(),
                        languages: draft.languages.clone(),
                        notes: draft.notes.clone(),
                    },
                )
            }
            LibraryWrite::Archive(id) => {
                if self.library.choir(*id).map_err(refusal)?.is_archived() {
                    return Err(refusal(SongLibraryError::AlreadyArchived(
                        RecordRef::Choir(*id),
                    )));
                }
                Prepared::ArchiveChoir(*id)
            }
            LibraryWrite::Restore(id) => {
                if !self.library.choir(*id).map_err(refusal)?.is_archived() {
                    return Err(refusal(SongLibraryError::NotArchived(RecordRef::Choir(
                        *id,
                    ))));
                }
                Prepared::RestoreChoir(*id)
            }
            LibraryWrite::CommitImport(request) => {
                return Self::import_verdict(&self.library, request);
            }
            lifecycle => self.validate_lifecycle(lifecycle)?,
        };
        Ok(Verdict::Write(prepared))
    }

    /// The domain checks for one lifecycle or History write, on a clone where a
    /// mutator is the rule, so memory is never touched before storage succeeds.
    fn validate_lifecycle(&self, write: &LibraryWrite) -> Result<Prepared, SharedString> {
        // Only the check runs here, so the stamp is a placeholder.
        let probe = Timestamp::from_millis(0);
        match write {
            LibraryWrite::ArchiveSong(id) => {
                self.library
                    .clone()
                    .archive_song(*id, probe)
                    .map_err(lifecycle_refusal)?;
                Ok(Prepared::ArchiveSong(*id))
            }
            LibraryWrite::RestoreSong(id) => {
                self.library
                    .clone()
                    .restore_song(*id)
                    .map_err(lifecycle_refusal)?;
                Ok(Prepared::RestoreSong(*id))
            }
            LibraryWrite::ArchiveArrangement(id) => {
                self.library
                    .clone()
                    .archive_arrangement(*id, probe)
                    .map_err(lifecycle_refusal)?;
                Ok(Prepared::ArchiveArrangement(*id))
            }
            LibraryWrite::RestoreArrangement(id) => {
                self.library
                    .clone()
                    .restore_arrangement(*id)
                    .map_err(lifecycle_refusal)?;
                Ok(Prepared::RestoreArrangement(*id))
            }
            LibraryWrite::DeleteSong(id) => {
                // Storage re-checks references; the domain judges the rest.
                match self
                    .library
                    .song_deletion_verdict(*id, &[])
                    .map_err(refusal)?
                {
                    DeletionVerdict::Allowed => Ok(Prepared::DeleteSong(*id)),
                    DeletionVerdict::Blocked(blockers) => Err(blocked_copy(&blockers)),
                }
            }
            LibraryWrite::DeleteArrangement(id) => {
                match self
                    .library
                    .arrangement_deletion_verdict(*id, &[])
                    .map_err(refusal)?
                {
                    DeletionVerdict::Allowed => Ok(Prepared::DeleteArrangement(*id)),
                    DeletionVerdict::Blocked(blockers) => Err(blocked_copy(&blockers)),
                }
            }
            LibraryWrite::RestoreRevision {
                revision,
                head,
                entity,
            } => {
                let restorable = match entity {
                    RevisionEntity::Song(id) => {
                        can_restore_song_revision(self.library.song(*id).map_err(refusal)?)
                    }
                    RevisionEntity::Arrangement(id) => can_restore_arrangement_revision(
                        self.library.arrangement_visibility(*id).map_err(refusal)?,
                    ),
                };
                if !restorable {
                    return Err(RESTORE_FIRST.into());
                }
                Ok(Prepared::RestoreRevision(*revision, *head))
            }
            _ => Err(SharedString::from(WRONG_KIND)),
        }
    }

    /// The Song must exist and be active.
    fn require_song_active(&self, song: SongId) -> Result<(), SharedString> {
        if self.library.song(song).map_err(refusal)?.is_archived() {
            return Err(refusal(SongLibraryError::Archived(RecordRef::Song(song))));
        }
        Ok(())
    }

    /// The checks an Arrangement's details need beyond their own fields: the
    /// Song and the Arrangement being active, and every Choir known and either
    /// active or already associated. The fields themselves are checked on a
    /// scratch library, and the details come back with canonical languages.
    fn check_arrangement_target(
        &self,
        song: SongId,
        existing: Option<&Arrangement>,
        details: &mut ArrangementDetails,
    ) -> Result<(), SharedString> {
        self.require_song_active(song)?;
        if let Some(arrangement) = existing
            && arrangement.is_archived()
        {
            return Err(refusal(SongLibraryError::Archived(RecordRef::Arrangement(
                arrangement.id(),
            ))));
        }
        for choir in &details.choirs {
            let stored = self.library.choir(*choir).map_err(refusal)?;
            let kept = existing.is_some_and(|a| a.choirs().contains(choir));
            if stored.is_archived() && !kept {
                return Err(refusal(SongLibraryError::Archived(RecordRef::Choir(
                    *choir,
                ))));
            }
        }
        *details = check_arrangement_details(details, existing).map_err(refusal)?;
        Ok(())
    }

    /// Applies the record storage returned to the in-memory library. Rarely,
    /// storage has already succeeded and the library refuses the record (for
    /// example an archive stamp is missing): the operator sees the failure
    /// while the database is ahead of memory until the next open re-hydrates
    /// it.
    fn apply(&mut self, write: &LibraryWrite, stored: &Stored) -> Result<Done, SharedString> {
        match (write, stored) {
            (LibraryWrite::ArchiveSong(id), Stored::Song(record)) => {
                let at = archived_stamp(record.archived_at, "Song")?;
                self.library.archive_song(*id, at).map_err(refusal)?;
                Ok(Done::Id(record.id.entity()))
            }
            (LibraryWrite::RestoreSong(id), Stored::Song(record)) => {
                self.library.restore_song(*id).map_err(refusal)?;
                Ok(Done::Id(record.id.entity()))
            }
            (LibraryWrite::ArchiveArrangement(id), Stored::Arrangement(record)) => {
                let at = archived_stamp(record.archived_at, "Arrangement")?;
                self.library.archive_arrangement(*id, at).map_err(refusal)?;
                Ok(Done::Id(record.id.entity()))
            }
            (LibraryWrite::RestoreArrangement(id), Stored::Arrangement(record)) => {
                self.library.restore_arrangement(*id).map_err(refusal)?;
                Ok(Done::Id(record.id.entity()))
            }
            (LibraryWrite::DeleteSong(id), Stored::Deleted(revisions)) => {
                self.library.delete_song(*id, &[]).map_err(refusal)?;
                Ok(Done::Count(*revisions))
            }
            (LibraryWrite::DeleteArrangement(id), Stored::Deleted(revisions)) => {
                self.library.delete_arrangement(*id, &[]).map_err(refusal)?;
                Ok(Done::Count(*revisions))
            }
            (
                LibraryWrite::RestoreRevision { .. },
                Stored::Restored(head, Restored::Song(content)),
            ) => {
                let LibraryWrite::RestoreRevision {
                    entity: RevisionEntity::Song(id),
                    ..
                } = write
                else {
                    return Err(WRONG_KIND.into());
                };
                self.library
                    .edit_song(*id, song_draft_from_content(content))
                    .map_err(refusal)?;
                Ok(Done::Id(head.entity()))
            }
            (
                LibraryWrite::RestoreRevision { .. },
                Stored::Restored(head, Restored::Arrangement(content)),
            ) => {
                let LibraryWrite::RestoreRevision {
                    entity: RevisionEntity::Arrangement(id),
                    ..
                } = write
                else {
                    return Err(WRONG_KIND.into());
                };
                self.library
                    .refresh_arrangement(
                        *id,
                        arrangement_draft_from_content(content),
                        content.choirs.clone(),
                    )
                    .map_err(refusal)?;
                Ok(Done::Id(head.entity()))
            }
            (LibraryWrite::CommitImport(request), Stored::Imported(records)) => {
                self.apply_imported(request, records)
            }
            _ => self.apply_record(write, stored).map(Done::Id),
        }
    }

    /// Applies a stored Choir, Song or Arrangement record that a create, edit,
    /// save or copy returned.
    fn apply_record(
        &mut self,
        write: &LibraryWrite,
        stored: &Stored,
    ) -> Result<EntityId, SharedString> {
        match (write, stored) {
            (LibraryWrite::Create(_), Stored::Choir(record)) => {
                self.library
                    .create_choir(record.id, record.created_at, draft_of(record))
                    .map_err(refusal)?;
                Ok(record.id.entity())
            }
            (LibraryWrite::Edit(id, _), Stored::Choir(record)) => {
                self.library
                    .edit_choir(*id, draft_of(record))
                    .map_err(refusal)?;
                Ok(record.id.entity())
            }
            (LibraryWrite::Archive(id), Stored::Choir(record)) => {
                // Storage always stamps an archived record; one without a stamp
                // is a storage fault, not something to paper over.
                let at = record.archived_at.ok_or_else(|| {
                    SharedString::from(
                        "The parish store returned an archived Choir without a date.",
                    )
                })?;
                self.library.archive_choir(*id, at).map_err(refusal)?;
                Ok(record.id.entity())
            }
            (LibraryWrite::Restore(id), Stored::Choir(record)) => {
                self.library.restore_choir(*id).map_err(refusal)?;
                Ok(record.id.entity())
            }
            (LibraryWrite::CreateSong(_), Stored::Song(record)) => {
                // The id and timestamp are the ones storage wrote, so memory
                // and disk agree.
                self.library
                    .create_song(record.id, record.created_at, song_draft_of(record))
                    .map_err(refusal)?;
                Ok(record.id.entity())
            }
            (LibraryWrite::CreateArrangement(..), Stored::Arrangement(record)) => {
                self.library
                    .create_arrangement(
                        record.id,
                        record.song,
                        record.created_at,
                        arrangement_draft_of(record),
                        record.choirs.clone(),
                    )
                    .map_err(refusal)?;
                Ok(record.id.entity())
            }
            (
                LibraryWrite::SaveArrangementDetails(id, _)
                | LibraryWrite::SaveArrangementLyrics(id, ..),
                Stored::Arrangement(record),
            ) => {
                // The record is the whole stored Arrangement, so memory takes it
                // as stored (content, Choirs and each section's pagination
                // state), even when another writer changed them since this
                // library was loaded.
                self.library
                    .refresh_arrangement(*id, arrangement_draft_of(record), record.choirs.clone())
                    .map_err(refusal)?;
                Ok(record.id.entity())
            }
            (LibraryWrite::DuplicateArrangement(source), Stored::Arrangement(record)) => {
                self.library
                    .duplicate_arrangement(
                        *source,
                        record.id,
                        record.created_at,
                        record.sections.iter().map(|section| section.id).collect(),
                        record
                            .sequence
                            .iter()
                            .map(|occurrence| occurrence.id)
                            .collect(),
                    )
                    .map_err(refusal)?;
                Ok(record.id.entity())
            }
            _ => Err(SharedString::from(
                "The parish store returned a record of the wrong kind.",
            )),
        }
    }

    /// Whether a write is in flight, and how many more are queued behind it.
    #[cfg(test)]
    pub(crate) fn write_state(&self) -> (bool, usize) {
        (self.writing, self.queue.len())
    }

    /// Replaces the library and tells observers. A test-only seam: production
    /// changes go through the Choir write methods, which write to storage first.
    #[cfg(test)]
    pub(crate) fn replace_library(&mut self, library: SongLibrary, cx: &mut Context<Self>) {
        self.library = library;
        self.generation += 1;
        self.state = LibraryState::Ready;
        cx.emit(LibraryEvent::Changed);
        cx.notify();
    }
}

#[cfg(test)]
pub(crate) mod tests;
