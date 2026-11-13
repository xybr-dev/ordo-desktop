//! The Arrangement editor: its frame, task tabs and the Lyrics side.
//!
//! [`editor::ArrangementEditor`] owns one editor visit: the
//! [`crate::edit_session::EditSession`] (when the Arrangement is editable),
//! the Lyrics/Sequence task tabs and the three pane slots the shell lays out.
//! [`panes`] draws those slots; the sections pane, lyric editor and Inspector
//! bodies are replaced there without touching the frame.
//! [`port::LyricsPort`] lets the session commit through the one
//! [`crate::songs::LibraryStore`] queue, so the GPUI thread never waits on
//! storage.

pub mod add_section;
pub mod editor;
pub mod navigator;
pub mod editor_pane;
pub mod panes;
pub mod port;
pub mod section_actions;
pub mod sections;
#[cfg(test)]
mod tests;
mod view;

pub use editor::{ArrangementEditor, TaskTab};
