//! Pure Ordo domain types and rules.
//!
//! T0.1 intentionally leaves this crate behavior-free. Domain Modules enter
//! through later milestone tasks, without GPUI, persistence, filesystem,
//! network, or operating-system window types.
//!
//! T1.1 adds [`presentation`]: an immutable Presentation snapshot whose
//! constructors reject duplicate identities, unresolved Themes, and auto-play
//! outside General Presentations before the Presenter can see it.

pub mod presentation;
