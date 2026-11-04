//! Pure Ordo domain types and rules.
//!
//! Domain Modules enter through milestone tasks and stay free of GPUI,
//! persistence, filesystem, network, and operating-system window types.
//!
//! T1.1 adds [`presentation`]: an immutable Presentation snapshot whose
//! constructors reject duplicate identities, unresolved Themes, and auto-play
//! outside General Presentations before the Presenter can see it.

pub mod presentation;
