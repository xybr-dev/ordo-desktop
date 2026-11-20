//! Pure Ordo domain types and rules.
//!
//! Domain Modules enter through milestone tasks and stay free of GPUI,
//! persistence, filesystem, network, and operating-system window types.
//!
//! T1.1 adds [`presentation`]: an immutable Presentation snapshot whose
//! constructors reject duplicate identities, unresolved Themes, and auto-play
//! outside General Presentations before the Presenter can see it.
//!
//! T2.1 adds [`identity`]: canonical UUIDv4 entity handles, the typed ids the
//! parish store uses, and the canonical UTC timestamp codec.
//!
//! T2.2 adds [`revisions`]: pure edit-safety mechanics (text coalescing, the
//! editor Undo stack, History sessions and grouping) with time passed in.
//!
//! T2.3 adds [`song_library`]: Song works, Arrangements, Choirs, and their
//! repertoire, pagination, and authored-content invariants.
//!
//! T2.7 adds [`pagination`]: the manual slide boundary that a blank lyric line
//! creates, and the Split and Join edits over it.

pub mod identity;
pub mod pagination;
pub mod presentation;
pub mod revisions;
pub mod song_library;
