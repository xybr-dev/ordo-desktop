//! Leaving the Presenter: the planned-exit hooks, the "exit requested" event
//! and the **End presentation?** confirmation (T1.6 WP2, Decision 179).
//!
//! The Presenter does not know the shell. It asks to leave by emitting
//! [`ExitRequested`], and whoever owns the window subscribes.
//!
//! A confirmed exit runs in a fixed order, owned by `PresenterView`:
//! Program to black through the Engine, then the Presenter closes its own
//! congregation window (T1.6b), then [`SessionHooks::close_output`], then
//! `DisableOutput`, then [`SessionHooks::on_session_ended`], then the
//! event. Nothing on the congregation display may outlive the black frame.

use std::rc::Rc;

use gpui::{App, FocusHandle, Window};
use presentation::engine::Engine;
use ui::overlay::Overlay;

/// Emitted by `PresenterView` once the session is over and the owner should
/// leave Presenter Mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitRequested;

/// What the owner plugs into a planned exit. Both are no-ops by default.
#[derive(Clone)]
pub struct SessionHooks {
    /// An extra step after Program is black and the Presenter has closed its
    /// congregation window, before the Engine is told `DisableOutput`; it sees
    /// the Engine so the owner can verify that. No-op by default.
    pub close_output: Rc<dyn Fn(&Engine)>,
    /// The session ended on purpose.
    // T1.7: mark the session complete and clear recovery
    pub on_session_ended: Rc<dyn Fn()>,
}

impl Default for SessionHooks {
    fn default() -> Self {
        Self {
            close_output: Rc::new(|_| {}),
            on_session_ended: Rc::new(|| {}),
        }
    }
}

pub const CONFIRM_TITLE: &str = "End presentation?";
pub const STAY: &str = "Stay in Presenter";
pub const END: &str = "End presentation";

/// The centered dialog. **Stay in Presenter** is the primary action and holds
/// initial focus (`stay`); Esc and the scrim also stay. Only `on_end` leaves.
pub fn confirmation(
    stay: &FocusHandle,
    on_end: impl Fn(&mut Window, &mut App) + 'static,
) -> Overlay {
    Overlay::dialog(CONFIRM_TITLE)
        .description("Program goes black and the congregation display closes.")
        .primary(STAY, |_, _| {})
        .primary_focus(stay)
        .secondary(END, on_end)
}
