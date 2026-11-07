//! Entering and leaving Presenter Mode in the one main window (T1.6a WP1).
//!
//! The shell entity stays alive the whole time. Entering builds a
//! [`PresenterView`], switches the window to native full-screen, and the
//! shell's root renders the Presenter in place of the preparation chrome.
//! Every preparation field (destination, selections, scroll handles, the
//! Masses index) is left alone, so leaving renders the same chrome from the
//! same state. Nothing is serialized and nothing is rebuilt (Decision 203).
//!
//! The Presenter never calls back into the shell directly: it emits
//! `ExitRequested` and the shell subscribes.

use std::{rc::Rc, sync::Arc, time::Instant};

use gpui::{AppContext as _, Bounds, Context, Entity, Pixels, Window, WindowBounds};
use platform::presenter_mode::{PresenterMode, RestoreBounds, WindowRestore};
use presentation::{
    snapshot_reader::{DirectorySource, LoadedSnapshot, SnapshotLoadError, read_snapshot},
    state::Millis,
};
use ui::{
    environment::{EnvironmentChange, UiPreferences},
    overlay::Toast,
};

use super::ShellView;
use crate::presenter::{PresenterView, session_header::exit::ExitRequested};

fn restore_bounds(bounds: Bounds<Pixels>) -> RestoreBounds {
    RestoreBounds::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}

/// GPUI's window state as the platform crate's plain copy of it.
fn window_restore(bounds: WindowBounds) -> WindowRestore {
    match bounds {
        WindowBounds::Windowed(bounds) => WindowRestore::Windowed(restore_bounds(bounds)),
        WindowBounds::Maximized(bounds) => WindowRestore::Maximized(restore_bounds(bounds)),
        WindowBounds::Fullscreen(bounds) => WindowRestore::Fullscreen(restore_bounds(bounds)),
    }
}

impl ShellView {
    /// The Presenter while the window is in Presenter Mode.
    pub fn presenter(&self) -> Option<&Entity<PresenterView>> {
        self.presenter.as_ref()
    }

    pub fn presenter_mode(&self) -> PresenterMode {
        self.mode
    }

    /// Start entering Presenter Mode. The snapshot is read on the background
    /// executor and the window changes only once it is in hand, so a slow
    /// read never blocks a frame and a failed one leaves Preparation as it was.
    pub(super) fn enter_presenter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_presenter() || self.presenter_load.is_some() {
            return;
        }
        self.presenter_load = Some(cx.spawn_in(window, async move |this, cx| {
            // Finding the fixture touches the filesystem, so it runs with the
            // read, off the GPUI thread.
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let directory = crate::presenter::fixture_snapshot_dir();
                    read_snapshot(&DirectorySource::new(directory))
                })
                .await;
            this.update_in(cx, |shell, window, cx| {
                shell.presenter_loaded(loaded, window, cx)
            })
            .ok();
        }));
    }

    fn presenter_loaded(
        &mut self,
        loaded: Result<LoadedSnapshot, SnapshotLoadError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.presenter_load = None;
        let snapshot = match loaded {
            Ok(snapshot) => Arc::new(snapshot),
            Err(error) => {
                self.overlays.notify(
                    Toast::failure(format!("The Presenter did not open: {error}")),
                    cx,
                );
                return;
            }
        };
        self.return_focus = window.focused(cx).map(|focus| focus.downgrade());
        let (mode, toggle_fullscreen) = self.mode.enter(window_restore(window.window_bounds()));
        self.mode = mode;
        if toggle_fullscreen {
            window.toggle_fullscreen();
        }
        self.change_environment(EnvironmentChange::EnterPresenter, false, cx);

        let started = Instant::now();
        let clock: Rc<dyn Fn() -> Millis> =
            Rc::new(move || Millis(started.elapsed().as_millis() as u64));
        let environment = self.environment;
        let save = self.save_preferences.clone();
        let presenter = cx.new(|cx| {
            let mut view = PresenterView::new(snapshot, clock, environment, window, cx);
            if let Some(save) = save {
                view.set_save_preferences(save);
            }
            view.install_output(Box::new(crate::output::GpuiDisplaySource), window, cx);
            view
        });
        self._presenter_exit = Some(cx.subscribe_in(
            &presenter,
            window,
            |shell, _, _: &ExitRequested, window, cx| shell.leave_presenter(window, cx),
        ));
        presenter.update(cx, |view, cx| view.focus_timeline(window, cx));
        self.presenter = Some(presenter);
        cx.notify();
    }

    /// Exit Presenter from outside the Presenter's own header. It takes the
    /// same guarded path as the header button, so a live session is never
    /// ended without the confirmation. With no Presenter it only cancels an
    /// entry still reading its snapshot.
    pub(super) fn request_leave_presenter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.presenter.clone() {
            Some(presenter) => presenter.update(cx, |view, cx| view.request_exit(window, cx)),
            None => {
                self.presenter_load = None;
                self.change_environment(EnvironmentChange::LeavePresenter, false, cx);
            }
        }
    }

    /// Drop the Presenter and bring Preparation back as it was.
    fn leave_presenter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(presenter) = self.presenter.take() else {
            return;
        };
        self._presenter_exit = None;
        let cue_view = presenter.read(cx).cue_view();
        drop(presenter);

        let (mode, leave) = self.mode.leave();
        self.mode = mode;
        // GPUI's Windows backend remembers the placement full-screen replaced
        // and restores it itself, so toggling back is the whole restore.
        if leave.is_some_and(|leave| leave.toggle_fullscreen) {
            window.toggle_fullscreen();
        }
        self.change_environment(EnvironmentChange::LeavePresenter, false, cx);
        // Grid/List was chosen (and saved) inside the Presenter; keep the
        // shell's copy of the preferences in step so a later save keeps it.
        self.environment
            .apply(EnvironmentChange::SetPreferences(UiPreferences {
                cue_view,
                ..self.environment.preferences()
            }));

        match self.return_focus.take().and_then(|focus| focus.upgrade()) {
            Some(focus) => focus.focus(window, cx),
            None => window.focus(&self.focus_handle, cx),
        }
        cx.notify();
    }
}
