//! The single owner of the parish store and the hydrated Song Library.
//!
//! [`LibraryStore`] is a GPUI entity. Opening it never blocks the GPUI thread:
//! the folder is created, the database opened, migrated and read, and the
//! library hydrated on a background task, and the entity then emits
//! [`LibraryEvent::Changed`]. A failure at any step leaves the entity
//! [`LibraryState::Unavailable`] with a message the Library screen shows; the
//! rest of the app keeps working. The store never opens for Presenter Mode.

use std::{cell::RefCell, future::Future, path::PathBuf, rc::Rc};

use domain::song_library::SongLibrary;
use gpui::{App, AppContext as _, Context, EventEmitter, SharedString};
use storage::{ShutdownOutcome, Storage, StorageConfig};

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
    /// The app-level owner of the database handle, kept for later writes. The
    /// slot, not this entity, shuts the worker down.
    _storage: StorageSlot,
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
            _storage: slot,
        }
    }

    /// A store that stays `Loading`, for tests of that state.
    #[cfg(test)]
    pub(crate) fn loading(_cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Loading,
            library: SongLibrary::new(),
            _storage: StorageSlot::new(),
        }
    }

    /// A ready store over an in-memory library with no database, for views
    /// and tests.
    pub fn with_library(library: SongLibrary, _cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Ready,
            library,
            _storage: StorageSlot::new(),
        }
    }

    /// A store that is permanently unavailable, with the reason shown to the
    /// operator.
    pub fn unavailable(message: impl Into<SharedString>, _cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Unavailable(message.into()),
            library: SongLibrary::new(),
            _storage: StorageSlot::new(),
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

    /// Replaces the library and tells observers. Test and lab seam: no write
    /// path exists in T2.4.
    pub fn replace_library(&mut self, library: SongLibrary, cx: &mut Context<Self>) {
        self.library = library;
        self.state = LibraryState::Ready;
        cx.emit(LibraryEvent::Changed);
        cx.notify();
    }
}
