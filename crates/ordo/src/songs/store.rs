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
    identity::{ChoirId, EntityId, Timestamp},
    song_library::{ChoirDraft, RecordRef, SongLibrary, SongLibraryError},
};
use gpui::{App, AppContext as _, Context, EventEmitter, SharedString, Task};
use storage::{
    ChoirDetails, ChoirRecord, NewChoir, ShutdownOutcome, Storage, StorageConfig, StorageError,
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

    fn put(&self, storage: Storage) {
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
    /// Choir writes waiting for the one in flight to finish.
    queue: VecDeque<(ChoirWrite, Reply)>,
    /// Whether a write is currently in flight.
    writing: bool,
}

type Reply = async_channel::Sender<Result<ChoirId, SharedString>>;

/// One Choir change the store can make.
enum ChoirWrite {
    Create(ChoirDraft),
    Edit(ChoirId, ChoirDraft),
    Archive(ChoirId),
    Restore(ChoirId),
}

/// The text shown when a write cannot reach the parish store.
const NOT_OPEN: &str = "The parish store is not open.";

/// A boxed storage future, so the four operations share one type.
type StoredChoir = Pin<Box<dyn Future<Output = Result<ChoirRecord, StorageError>> + Send>>;

fn boxed(
    future: impl Future<Output = Result<ChoirRecord, StorageError>> + Send + 'static,
) -> StoredChoir {
    Box::pin(future)
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
        self.submit(ChoirWrite::Create(draft), cx)
    }

    /// Replaces a Choir's details; an archived Choir is refused.
    pub fn edit_choir(
        &mut self,
        id: ChoirId,
        draft: ChoirDraft,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(ChoirWrite::Edit(id, draft), cx)
    }

    /// Archives a Choir.
    pub fn archive_choir(
        &mut self,
        id: ChoirId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(ChoirWrite::Archive(id), cx)
    }

    /// Restores an archived Choir.
    pub fn restore_choir(
        &mut self,
        id: ChoirId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        self.submit(ChoirWrite::Restore(id), cx)
    }

    fn submit(
        &mut self,
        write: ChoirWrite,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChoirId, SharedString>> {
        let (reply, result) = async_channel::bounded(1);
        self.queue.push_back((write, reply));
        self.pump(cx);
        cx.spawn(async move |_, _| {
            result
                .recv()
                .await
                .unwrap_or_else(|_| Err(SharedString::from("The change was abandoned.")))
        })
    }

    /// Starts the next queued write when none is in flight.
    fn pump(&mut self, cx: &mut Context<Self>) {
        while !self.writing {
            let Some((write, reply)) = self.queue.pop_front() else {
                return;
            };
            if self.state != LibraryState::Ready {
                let _ = reply.try_send(Err(NOT_OPEN.into()));
                continue;
            }
            if let Err(message) = self.validate(&write) {
                let _ = reply.try_send(Err(message));
                continue;
            }
            let started = self.storage.with(|storage| match &write {
                ChoirWrite::Create(draft) => boxed(storage.create_choir(NewChoir {
                    name: draft.name.clone(),
                    short_label: draft.short_label.clone(),
                    languages: draft.languages.clone(),
                    notes: draft.notes.clone(),
                })),
                ChoirWrite::Edit(id, draft) => boxed(storage.edit_choir(
                    *id,
                    ChoirDetails {
                        name: draft.name.clone(),
                        short_label: draft.short_label.clone(),
                        languages: draft.languages.clone(),
                        notes: draft.notes.clone(),
                    },
                )),
                ChoirWrite::Archive(id) => boxed(storage.archive_choir(*id)),
                ChoirWrite::Restore(id) => boxed(storage.restore_choir(*id)),
            });
            let Some(future) = started else {
                let _ = reply.try_send(Err(NOT_OPEN.into()));
                continue;
            };
            self.writing = true;
            cx.spawn(async move |this, cx| {
                let stored = cx
                    .background_spawn(
                        async move { future.await.map_err(|error| error.to_string()) },
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

    /// The domain's verdict on a write, before any I/O.
    fn validate(&self, write: &ChoirWrite) -> Result<(), SharedString> {
        match write {
            ChoirWrite::Create(draft) => check_draft(draft),
            ChoirWrite::Edit(id, draft) => {
                if self.library.choir(*id).map_err(refusal)?.is_archived() {
                    return Err(refusal(SongLibraryError::Archived(RecordRef::Choir(*id))));
                }
                check_draft(draft)
            }
            ChoirWrite::Archive(id) => {
                if self.library.choir(*id).map_err(refusal)?.is_archived() {
                    return Err(refusal(SongLibraryError::AlreadyArchived(
                        RecordRef::Choir(*id),
                    )));
                }
                Ok(())
            }
            ChoirWrite::Restore(id) => {
                if self.library.choir(*id).map_err(refusal)?.is_archived() {
                    Ok(())
                } else {
                    Err(refusal(SongLibraryError::NotArchived(RecordRef::Choir(
                        *id,
                    ))))
                }
            }
        }
    }

    /// Applies the record storage returned to the in-memory library.
    fn apply(&mut self, write: &ChoirWrite, record: &ChoirRecord) -> Result<ChoirId, SharedString> {
        match write {
            ChoirWrite::Create(_) => {
                self.library
                    .create_choir(record.id, record.created_at, draft_of(record))
                    .map_err(refusal)?;
            }
            ChoirWrite::Edit(id, _) => {
                self.library
                    .edit_choir(*id, draft_of(record))
                    .map_err(refusal)?;
            }
            ChoirWrite::Archive(id) => {
                let at = record.archived_at.unwrap_or(record.created_at);
                self.library.archive_choir(*id, at).map_err(refusal)?;
            }
            ChoirWrite::Restore(id) => self.library.restore_choir(*id).map_err(refusal)?,
        }
        Ok(record.id)
    }

    /// Replaces the library and tells observers. Test and lab seam: no write
    /// path exists in T2.4.
    pub fn replace_library(&mut self, library: SongLibrary, cx: &mut Context<Self>) {
        self.library = library;
        self.state = LibraryState::Ready;
        cx.emit(LibraryEvent::Changed);
        cx.notify();
    }
}
