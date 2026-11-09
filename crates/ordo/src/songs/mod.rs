//! The Songs screens and the store they read.
//!
//! The shell owns chrome only; everything a Library view draws, and the one
//! owner of the parish database it draws from, lives here. [`store`] opens the
//! parish data folder off the GPUI thread and holds the hydrated Song Library;
//! [`library`] is the Songs master-detail workspace over it.

pub mod library;
pub mod store;

pub use library::{ArrangementOverview, SongsWorkspace};
pub use store::{LibraryEvent, LibraryState, LibraryStore};
