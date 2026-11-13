//! The single owner of the parish store and the hydrated Song Library.
//!
//! [`LibraryStore`] is a GPUI entity. Opening it never blocks the GPUI thread:
//! the folder is created, the database opened, migrated and read, and the
//! library hydrated on a background task, and the entity then emits
//! [`LibraryEvent::Changed`]. A failure at any step leaves the entity
//! [`LibraryState::Unavailable`] with a message the Library screen shows; the
//! rest of the app keeps working. The store never opens for Presenter Mode.

use std::{cell::RefCell, collections::VecDeque, future::Future, path::PathBuf, pin::Pin, rc::Rc};

use domain::{
    identity::{ArrangementId, ChoirId, EntityId, HistorySessionId, SongId, Timestamp},
    song_library::{
        ArchiveState, Arrangement, ArrangementDetails, ArrangementDraft, ArrangementLyrics,
        ChoirDraft, Field, HydrateArrangement, HydrateChoir, HydrateReserved, HydrateSong,
        OccurrenceDraft, PaginationState, RecordRef, SectionDraft, SongDraft, SongLibrary,
        SongLibraryError, match_key,
    },
};
use gpui::{App, AppContext as _, Context, EventEmitter, SharedString, Task};
use storage::{
    ArrangementMetadata, ArrangementRecord, ChoirDetails, ChoirRecord, NewArrangement, NewChoir,
    NewSection, NewSong, OccurrenceContent, SectionContent, ShutdownOutcome, SongRecord, Storage,
    StorageConfig, StorageError,
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
}

/// The id of the record a write created or changed.
type Reply = async_channel::Sender<Result<EntityId, SharedString>>;

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
    /// Replaces an Arrangement's sections, Sequence and pagination state as one
    /// Edit revision of the given History session.
    SaveArrangementLyrics(ArrangementId, ArrangementLyrics, HistorySessionId),
    /// Copies the stored Arrangement into a new one under the same Song.
    DuplicateArrangement(ArrangementId),
}

/// What storage returned for a write.
enum Stored {
    Choir(ChoirRecord),
    Song(SongRecord),
    Arrangement(ArrangementRecord),
}

/// What validation decided for a write.
enum Verdict {
    /// The write changes nothing, so there is no revision, no write and no
    /// event: the task resolves to this id.
    Unchanged(EntityId),
    /// The storage call to make.
    Write(Prepared),
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
        pagination_state: PaginationState::Draft,
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

/// Checks a Lyrics edit against the domain's own rules on a scratch library
/// holding only this Arrangement's stored content and its current section and
/// occurrence identities, so the cost does not grow with the parish library.
fn check_lyrics(
    arrangement: &Arrangement,
    lyrics: &ArrangementLyrics,
) -> Result<(), SongLibraryError> {
    let at = Timestamp::from_millis(0);
    let song = arrangement.song();
    let choirs: Vec<ChoirId> = arrangement.choirs().iter().copied().collect();
    let stored = ArrangementDraft {
        title: arrangement.title().map(str::to_owned),
        languages: arrangement.languages().to_vec(),
        rendition_attribution: arrangement.rendition_attribution().map(str::to_owned),
        occasions: arrangement.occasions().iter().cloned().collect(),
        notes: arrangement.notes().map(str::to_owned),
        sections: arrangement
            .sections()
            .iter()
            .map(|section| SectionDraft {
                id: section.id(),
                label: section.label().map(str::to_owned),
                lyrics: section.lyrics().to_owned(),
            })
            .collect(),
        sequence: arrangement
            .sequence()
            .iter()
            .map(|occurrence| OccurrenceDraft::new(occurrence.id(), occurrence.section()))
            .collect(),
    };
    let reserved = HydrateReserved {
        arrangement: arrangement.id(),
        sections: stored.sections.iter().map(|section| section.id).collect(),
        occurrences: stored
            .sequence
            .iter()
            .map(|occurrence| occurrence.id)
            .collect(),
    };
    let mut scratch = SongLibrary::hydrate(
        vec![HydrateSong {
            id: song,
            created_at: at,
            draft: SongDraft::titled("probe"),
            archive_state: ArchiveState::Active,
        }],
        vec![HydrateArrangement {
            id: arrangement.id(),
            song,
            created_at: arrangement.created_at(),
            draft: stored,
            choirs: choirs.clone(),
            pagination_state: arrangement.pagination_state(),
            archive_state: ArchiveState::Active,
        }],
        choirs
            .iter()
            .enumerate()
            .map(|(index, id)| HydrateChoir {
                id: *id,
                created_at: at,
                draft: ChoirDraft::named(format!("probe {index}")),
                archive_state: ArchiveState::Active,
            })
            .collect(),
        vec![reserved],
    )
    .map_err(|error| error.cause)?;
    scratch
        .edit_arrangement_lyrics(arrangement.id(), lyrics.clone())
        .map(|_| ())
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

    /// Replaces an Arrangement's sections, Sequence and pagination state as one
    /// History session's `Edit` revision, leaving its metadata and Choirs as
    /// stored. It is an atomic lyrics-only commit, the mirror of
    /// [`Self::save_arrangement_details`]: storage reads the head revision and
    /// the metadata in the same transaction and returns the whole stored
    /// Arrangement, which memory then takes, so a lyrics autosave never
    /// overwrites newer details. The domain checks the edit first (a blank or
    /// duplicate section name is refused without I/O). Lyrics equal to the
    /// stored ones write nothing and resolve to the id. A storage error leaves
    /// memory unchanged.
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

    fn submit<T: 'static>(
        &mut self,
        write: LibraryWrite,
        convert: fn(EntityId) -> T,
        cx: &mut Context<Self>,
    ) -> Task<Result<T, SharedString>> {
        let (reply, result) = async_channel::bounded(1);
        self.queue.push_back((write, reply));
        self.pump(cx);
        cx.spawn(async move |_, _| {
            let outcome = result.recv().await;
            outcome
                .unwrap_or_else(|_| Err(SharedString::from("The change was abandoned.")))
                .map(convert)
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
            let prepared = match self.validate(&mut write) {
                Ok(Verdict::Write(prepared)) => prepared,
                Ok(Verdict::Unchanged(id)) => {
                    let _ = reply.try_send(Ok(id));
                    continue;
                }
                Err(message) => {
                    let _ = reply.try_send(Err(message));
                    continue;
                }
            };
            let started = self.storage.with(|storage| match prepared {
                Prepared::CreateSong(new) => boxed_song(storage.create_song(*new)),
                Prepared::CreateChoir(new) => boxed(storage.create_choir(new)),
                Prepared::EditChoir(id, details) => boxed(storage.edit_choir(id, details)),
                Prepared::ArchiveChoir(id) => boxed(storage.archive_choir(id)),
                Prepared::RestoreChoir(id) => boxed(storage.restore_choir(id)),
                Prepared::CreateArrangement(new) | Prepared::Copy(new) => {
                    boxed_arrangement(storage.create_arrangement(*new))
                }
                Prepared::SaveMetadata(id, metadata) => {
                    let commit =
                        storage.commit_arrangement_metadata(id, *metadata, fresh_session());
                    boxed_arrangement(async move { commit.await.map(|(record, _)| record) })
                }
                Prepared::SaveLyrics(id, lyrics, session) => {
                    let ArrangementLyrics {
                        sections,
                        sequence,
                        pagination,
                    } = *lyrics;
                    let sections = sections
                        .into_iter()
                        .map(|section| SectionContent {
                            id: section.id,
                            label: section.label,
                            lyrics: section.lyrics,
                        })
                        .collect();
                    let sequence = sequence
                        .into_iter()
                        .map(|occurrence| OccurrenceContent {
                            id: occurrence.id,
                            section: occurrence.section,
                        })
                        .collect();
                    let commit = storage
                        .commit_arrangement_lyrics(id, sections, sequence, pagination, session);
                    boxed_arrangement(async move { commit.await.map(|(record, _)| record) })
                }
            });
            let Some(started) = started else {
                let _ = reply.try_send(Err(NOT_OPEN.into()));
                continue;
            };
            self.writing = true;
            cx.spawn(async move |this, cx| {
                let stored = cx
                    .background_spawn(
                        async move { started.await.map_err(|error| error.to_string()) },
                    )
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let outcome = match stored {
                        Ok(record) => this.apply(&write, &record),
                        Err(message) => Err(SharedString::from(message)),
                    };
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
                check_lyrics(arrangement, lyrics).map_err(refusal)?;
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
        };
        Ok(Verdict::Write(prepared))
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
    fn apply(&mut self, write: &LibraryWrite, stored: &Stored) -> Result<EntityId, SharedString> {
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
                // as stored (content, Choirs and pagination state), even when
                // another writer changed them since this library was loaded.
                self.library
                    .refresh_arrangement(
                        *id,
                        arrangement_draft_of(record),
                        record.choirs.clone(),
                        record.pagination,
                    )
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
        self.state = LibraryState::Ready;
        cx.emit(LibraryEvent::Changed);
        cx.notify();
    }
}

#[cfg(test)]
pub(crate) mod tests;
