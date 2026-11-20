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

pub mod identity;
pub mod presentation;
pub mod revisions;
