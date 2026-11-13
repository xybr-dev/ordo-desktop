//! The Songs screens and the store they read.
//!
//! The shell owns chrome only; everything a Library view draws, and the one
//! owner of the parish database it draws from, lives here. [`store`] opens the
//! parish data folder off the GPUI thread and holds the hydrated Song Library;
//! [`library`] is the Songs master-detail workspace over it.

pub mod arrangement_details;
pub mod choirs;
#[cfg(test)]
mod e2e;
pub mod library;
pub mod lyrics;
pub mod new_song;
pub mod store;

pub use arrangement_details::ArrangementDetailsPanel;
pub use choirs::ChoirsWorkspace;
pub use library::SongsWorkspace;
pub use new_song::NewSongPanel;
pub use store::{LibraryEvent, LibraryState, LibraryStore, StorageSlot};
