//! The Arrangement editor's Lyrics side and its persistence seam.
//!
//! T2.7a lands the revision port only: the editor frame, the sections pane and
//! the lyric task arrive with T2.7b. [`port::LyricsPort`] lets an
//! [`crate::edit_session::EditSession`] commit a
//! [`domain::song_library::ArrangementLyrics`] through the one
//! [`crate::songs::LibraryStore`] queue, so the GPUI thread never waits on
//! storage.

pub mod port;
