//! Entity and value types owned by the Song Library.
//!
//! The types here are read-only outside the Module Interface: their values are
//! private, and only [`crate::song_library::SongLibrary`] can create or change
//! one after validating the library's invariants.

mod arrangement;
mod choir;
mod song;
pub(crate) mod text;

pub use arrangement::{
    Arrangement, ArrangementDraft, Occurrence, OccurrenceDraft, PaginationState, Section,
    SectionDraft,
};
pub use choir::Choir;
pub use song::{SongDraft, SongWork};

use crate::identity::Timestamp;

/// Whether an entity takes part in ordinary library use.
///
/// Archival is independent per entity: archiving one entity never rewrites
/// another's state, and an archived entity keeps its full content and identity
/// until it is restored.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ArchiveState {
    /// The entity takes part in ordinary library use.
    Active,
    /// The entity is retired and records when that happened.
    Archived(Timestamp),
}

impl ArchiveState {
    /// Whether the entity is archived.
    pub const fn is_archived(self) -> bool {
        matches!(self, Self::Archived(_))
    }

    /// When the entity was archived, if it has been.
    pub const fn archived_at(self) -> Option<Timestamp> {
        match self {
            Self::Active => None,
            Self::Archived(at) => Some(at),
        }
    }
}
