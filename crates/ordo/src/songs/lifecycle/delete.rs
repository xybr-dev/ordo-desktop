//! The reference-safety deletion dialog.

use gpui::{App, Entity, Window};
use ui::overlay::OverlayHost;

use super::{AfterLifecycle, LifecycleTarget, Request};
use crate::songs::store::LibraryStore;

/// Placeholder until the deletion step.
pub fn blocked_description(_name: &str) -> String {
    String::new()
}

/// Placeholder until the deletion step.
pub fn erase_summary(_name: &str, _sections: usize, _revisions: usize) -> String {
    String::new()
}

/// Placeholder until the deletion step.
pub fn review_deletion(
    _target: LifecycleTarget,
    _store: Entity<LibraryStore>,
    _host: OverlayHost,
    _after: AfterLifecycle,
    _window: &mut Window,
    _cx: &mut App,
) {
}

pub(super) fn review(_request: Request, _window: &mut Window, _cx: &mut App) {}
