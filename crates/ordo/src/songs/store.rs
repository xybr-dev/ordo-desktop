//! The single owner of the parish store and the hydrated Song Library.
//!
//! [`LibraryStore`] is a GPUI entity. Opening it never blocks the GPUI thread:
//! the folder is created, the database opened, migrated and read, and the
//! library hydrated on a background task, and the entity then emits
//! [`LibraryEvent::Changed`]. A failure at any step leaves the entity
//! [`LibraryState::Unavailable`] with a message the Library screen shows; the
//! rest of the app keeps working. The store never opens for Presenter Mode.

use std::path::PathBuf;

use domain::song_library::SongLibrary;
use gpui::{AppContext as _, Context, EventEmitter, SharedString, Subscription};
use storage::{Storage, StorageConfig};

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

/// The parish store handle and the library hydrated from it.
pub struct LibraryStore {
    state: LibraryState,
    library: SongLibrary,
    /// Kept so the worker thread lives as long as the entity and is shut down
    /// on quit. `None` for a store with no database behind it.
    storage: Option<Storage>,
    _quit: Option<Subscription>,
}

impl EventEmitter<LibraryEvent> for LibraryStore {}

impl LibraryStore {
    /// A store over `directory`, created if missing. It starts `Loading` and
    /// settles on a background task.
    pub fn open(directory: PathBuf, cx: &mut Context<Self>) -> Self {
        // The quit handler owns the shutdown: GPUI polls it for a short grace
        // period, which is ample for the worker to finish its queue.
        let quit = cx.on_app_quit(|this, _| {
            let storage = this.storage.take();
            async move {
                if let Some(storage) = storage {
                    let _ = storage.shutdown().await;
                }
            }
        });
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_spawn(async move {
                    std::fs::create_dir_all(&directory).map_err(|error| {
                        format!("The parish folder could not be created: {error}.")
                    })?;
                    let storage = Storage::spawn(StorageConfig::in_directory(&directory));
                    let loaded = async {
                        storage.ready().await.map_err(|error| error.to_string())?;
                        let records = storage.library().await.map_err(|error| error.to_string())?;
                        records.hydrate().map_err(|error| error.to_string())
                    }
                    .await;
                    match loaded {
                        Ok(library) => Ok((storage, library)),
                        Err(message) => {
                            let _ = storage.shutdown().await;
                            Err(message)
                        }
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match opened {
                    Ok((storage, library)) => {
                        this.storage = Some(storage);
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
            storage: None,
            _quit: Some(quit),
        }
    }

    /// A ready store over an in-memory library with no database, for views
    /// and tests.
    pub fn with_library(library: SongLibrary, _cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Ready,
            library,
            storage: None,
            _quit: None,
        }
    }

    /// A store that is permanently unavailable, with the reason shown to the
    /// operator.
    pub fn unavailable(message: impl Into<SharedString>, _cx: &mut Context<Self>) -> Self {
        Self {
            state: LibraryState::Unavailable(message.into()),
            library: SongLibrary::new(),
            storage: None,
            _quit: None,
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
