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
//! never unite two Song works automatically. Song merge and merged-Song
//! redirects are post-MVP (Decision 42, MVP-PLAN 6.18); duplicate evidence only
//! reports likely duplicates.

mod duplicate_evidence;
mod interface;
mod languages;
mod model;
mod search;
mod sections;

pub use duplicate_evidence::{
    DuplicateEvidence, IdentityEvidence, MatchStrength, MatchedOn, PossibleSong, identity_key,
};
pub use interface::{
    ArrangementVisibility, DeletionBlocker, DeletionVerdict, DurableReference, Field,
    HydrateArrangement, HydrateChoir, HydrateReserved, HydrateSong, HydrationError, RecordRef,
    SongLibrary, SongLibraryError, StatusFilter, match_key,
};
pub use languages::{Language, LanguageMatch, find_language, is_listed, languages};
pub use model::{
    ArchiveState, Arrangement, ArrangementDetails, ArrangementDraft, Choir, ChoirDraft, Occurrence,
    OccurrenceDraft, PaginationState, Section, SectionDraft, SongDraft, SongWork,
};
pub use search::{
    AttributionField, Evidence, FilterValue, LibraryQuery, SongHit, search, search_key,
};
pub use sections::{ArrangementLyrics, Step, section_name_suggestions};
