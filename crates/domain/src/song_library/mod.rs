//! Song Library model and its one Module Interface.
//!
//! [`SongLibrary`] owns Song works, Arrangements, and Choirs in memory and
//! enforces their invariants:
//!
//! - A Song work needs only a primary title. Authored text is kept verbatim.
//! - An Arrangement belongs to exactly one Song work and independently owns
//!   its title, languages, Choir associations, rendition attribution, occasion
//!   applicability, sections, Sequence, pagination review state, and archive
//!   state.
//! - Several Choirs may share one exact Arrangement, and Songs and Arrangements
//!   stay valid with no Choir at all.
//! - Numbered settings such as Kyrie 8th and Kyrie 13th are separate Song
//!   works, while translated renditions retaining one composition are separate
//!   Arrangements of the same work.
//! - An approved pagination returns to review when content that affects slides
//!   changes.
//!
//! The module is pure. Callers pass identities from [`crate::identity`] and
//! creation times in; it reads no clock, draws no randomness, and performs no
//! I/O. The Interface has no merge and no redirect operation: similar titles
//! never unite two Song works automatically.

mod interface;
mod model;

pub use interface::{Field, SongLibrary, SongLibraryError};
pub use model::{
    ArchiveState, Arrangement, ArrangementDraft, Choir, Occurrence, OccurrenceDraft,
    PaginationState, Section, SectionDraft, SongDraft, SongWork,
};
