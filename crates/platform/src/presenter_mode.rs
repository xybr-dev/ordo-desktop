//! Presenter Mode entry and exit as plain data (T1.6a).
//!
//! The main window enters native full-screen for Presenter Mode and leaves it
//! again on exit (Decision 203). This module holds only what that needs to be
//! decided and tested without a window: what the window looked like before
//! entry, whether entry itself toggled full-screen, and whether leaving needs
//! the operator's confirmation (Decision 179). The GPUI window calls live in
//! the owner (`ordo`'s shell), which maps GPUI's `WindowBounds` onto
//! [`WindowRestore`].

/// A window rectangle in logical pixels, as plain numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RestoreBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RestoreBounds {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// How the window was shown before Presenter Mode, mirroring GPUI's
/// `WindowBounds`. Each variant carries the bounds to come back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowRestore {
    Windowed(RestoreBounds),
    Maximized(RestoreBounds),
    Fullscreen(RestoreBounds),
}

impl WindowRestore {
    pub fn is_fullscreen(self) -> bool {
        matches!(self, Self::Fullscreen(_))
    }

    pub fn bounds(self) -> RestoreBounds {
        match self {
            Self::Windowed(bounds) | Self::Maximized(bounds) | Self::Fullscreen(bounds) => bounds,
        }
    }
}

/// Whether the main window shows Preparation or Presenter Mode.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PresenterMode {
    #[default]
    Preparation,
    Presenter {
        /// The window as it was when Presenter Mode was entered.
        restore: WindowRestore,
        /// Whether entry switched full-screen on. Exit switches it off again
        /// only then, so a window that was already full-screen stays so.
        toggled_fullscreen: bool,
    },
}

/// What leaving the window asks of the owner, in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leave {
    /// Toggle native full-screen back off.
    pub toggle_fullscreen: bool,
    /// The window state to restore, for an owner that must re-apply it.
    pub restore: WindowRestore,
}

impl PresenterMode {
    pub fn is_presenter(self) -> bool {
        matches!(self, Self::Presenter { .. })
    }

    /// Enter from the window's current state. Returns the mode and whether the
    /// owner must toggle full-screen on (not when it already is). Entering
    /// while already in Presenter Mode changes nothing and asks for nothing.
    pub fn enter(self, current: WindowRestore) -> (Self, bool) {
        if self.is_presenter() {
            return (self, false);
        }
        let toggle = !current.is_fullscreen();
        (
            Self::Presenter {
                restore: current,
                toggled_fullscreen: toggle,
            },
            toggle,
        )
    }

    /// Leave Presenter Mode. `None` when not in it.
    pub fn leave(self) -> (Self, Option<Leave>) {
        match self {
            Self::Preparation => (self, None),
            Self::Presenter {
                restore,
                toggled_fullscreen,
            } => (
                Self::Preparation,
                Some(Leave {
                    toggle_fullscreen: toggled_fullscreen,
                    restore,
                }),
            ),
        }
    }
}

/// How Exit Presenter proceeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitPlan {
    /// Output is disabled: nothing is on a congregation display, so leave.
    Direct,
    /// Output is enabled: ask "End presentation?" first.
    Confirm,
}

/// Exit is direct while output is disabled and guarded while it is enabled
/// (Decision 179). A lost display counts as enabled: output is still on.
pub fn exit_plan(output_enabled: bool) -> ExitPlan {
    if output_enabled {
        ExitPlan::Confirm
    } else {
        ExitPlan::Direct
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: RestoreBounds = RestoreBounds::new(10.0, 20.0, 1280.0, 720.0);

    #[test]
    fn exit_is_direct_only_while_output_is_disabled() {
        assert_eq!(exit_plan(false), ExitPlan::Direct);
        assert_eq!(exit_plan(true), ExitPlan::Confirm);
    }

    #[test]
    fn entering_from_a_window_toggles_full_screen_and_leaving_toggles_back() {
        for restore in [
            WindowRestore::Windowed(BOUNDS),
            WindowRestore::Maximized(BOUNDS),
        ] {
            let (mode, toggle) = PresenterMode::Preparation.enter(restore);
            assert!(toggle, "{restore:?}");
            assert_eq!(
                mode,
                PresenterMode::Presenter {
                    restore,
                    toggled_fullscreen: true
                }
            );
            let (mode, leave) = mode.leave();
            assert_eq!(mode, PresenterMode::Preparation);
            assert_eq!(
                leave,
                Some(Leave {
                    toggle_fullscreen: true,
                    restore
                })
            );
        }
    }

    #[test]
    fn a_window_already_full_screen_stays_full_screen() {
        let restore = WindowRestore::Fullscreen(BOUNDS);
        let (mode, toggle) = PresenterMode::Preparation.enter(restore);
        assert!(!toggle);
        let (_, leave) = mode.leave();
        assert_eq!(
            leave,
            Some(Leave {
                toggle_fullscreen: false,
                restore
            })
        );
        assert_eq!(restore.bounds(), BOUNDS);
    }

    #[test]
    fn entering_twice_keeps_the_first_restore_state_and_leaving_twice_is_nothing() {
        let first = WindowRestore::Windowed(BOUNDS);
        let (mode, _) = PresenterMode::Preparation.enter(first);
        let (again, toggle) = mode.enter(WindowRestore::Fullscreen(BOUNDS));
        assert_eq!(again, mode);
        assert!(!toggle);
        let (mode, _) = mode.leave();
        assert_eq!(mode.leave(), (PresenterMode::Preparation, None));
    }
}
