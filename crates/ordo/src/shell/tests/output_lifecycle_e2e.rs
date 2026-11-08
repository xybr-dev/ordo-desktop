//! Output lifecycle E2E (T1.6 WP8): one pass through the whole Presenter
//! session, driven from the shell the way an operator would.
//!
//! Steps and the T1.6 `[UI-AUTO]` rows they evidence:
//!
//! 1. Host state is set, then Present enters Presenter in the same window,
//!    full-screen, one window, output disabled (entry and no-output rows).
//! 2. Rehearse: the keyboard moves Preview while the Engine stays
//!    `OutputDisabled` and no window opens (rehearsal-without-output row).
//! 3. A fake projector is installed and Enable Output opens one congregation
//!    window, Program black (enable and black-before-Take rows).
//! 4. Space takes the Cue live and Program shows it (Take row).
//! 5. Exit asks first, Stay in Presenter cancels, nothing changes (guarded
//!    exit and cancel rows).
//! 6. End presentation closes the output after Program is black (black before
//!    close row).
//! 7. Preparation returns with the exact selection and scroll state, out of
//!    full-screen, one window, shell focused (restoration row).
//!
//! Black before close is evidenced two ways, both recorded by the real exit
//! path rather than inferred: the congregation owner stamps the Engine's live
//! state at the moment it closes the window (`output_closed_at`, a test-only
//! accessor), and the owner's `close_output` session hook, which runs after
//! the window is closed, records the Engine state it saw. Both must be
//! `Blackout`, and the output window must be gone from the app.

use super::*;
use platform::display::{
    DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
    PlacementBounds,
};
use presentation::state::{LiveState, ProgramContent};
use std::cell::RefCell;
use std::rc::Rc;

fn display(id: u64, x: i32, width: u32, height: u32) -> DetectedDisplay {
    let bounds = DisplayBounds::new(x, 0, width, height);
    DetectedDisplay {
        id: DisplayId::new(id),
        evidence: DisplayEvidence {
            name: None,
            uuid: None,
            bounds,
        },
        placement: PlacementBounds::from(bounds),
    }
}

/// An operator display and a projector.
fn with_projector() -> Rc<RefCell<InMemoryDisplayAdapter>> {
    Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 1920, 1080), display(2, 1920, 1280, 720)],
        Some(DisplayId::new(1)),
    )))
}

fn engine_state(
    shell: &gpui::Entity<ShellView>,
    cx: &mut VisualTestContext,
) -> presentation::state::EngineState {
    shell.read_with(cx, |shell, cx| {
        let presenter = shell.presenter().expect("the Presenter is mounted");
        presenter.read(cx).engine().state().clone()
    })
}

#[gpui::test]
fn the_output_lifecycle_runs_from_entry_to_a_restored_exit(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);

    // 1. Host state, then enter.
    shell.update_in(cx, |shell, window, cx| {
        shell.selected_row = Some(4);
        shell
            .list_scroll
            .handle()
            .set_offset(gpui::point(px(0.0), px(-120.0)));
        shell
            .viewer_scroll
            .handle()
            .set_offset(gpui::point(px(0.0), px(-75.0)));
        shell
            .editor_scroll
            .handle()
            .set_offset(gpui::point(px(0.0), px(-33.0)));
        shell
            .masses
            .scroll
            .handle()
            .set_offset(gpui::point(px(0.0), px(-18.0)));
        shell.masses.toggle(1);
        shell.masses.search.set_value("advent", window, cx);
        cx.notify();
    });
    cx.run_until_parked();
    let before = shell.read_with(cx, preparation_state);
    assert_eq!(before.selected_row, Some(4));
    assert_eq!(before.masses_search, "advent");

    click(cx, "button-Present");
    assert!(shell.read_with(cx, |shell, _| shell.presenter_mode().is_presenter()));
    assert_eq!(engine_state(&shell, cx).live(), LiveState::OutputDisabled);
    assert!(fullscreen(cx));
    assert_eq!(windows_open(cx), 1);
    let presenter = shell.read_with(cx, |shell, _| shell.presenter().cloned().unwrap());
    assert!(presenter.read_with(cx, |view, _| view.output_window().is_none()));

    // 2. Rehearse with no output window.
    let start = engine_state(&shell, cx).preview();
    keys(cx, "right");
    let rehearsed = engine_state(&shell, cx);
    assert_ne!(rehearsed.preview(), start, "Preview moved");
    assert_eq!(rehearsed.live(), LiveState::OutputDisabled);
    assert_eq!(windows_open(cx), 1, "rehearsal opens no window");

    // 3. Fake projector, Enable Output.
    let adapter = with_projector();
    presenter.update_in(cx, |view, window, cx| {
        view.install_output(Box::new(adapter), window, cx)
    });
    cx.run_until_parked();
    click(cx, "primary-action");
    let enabled = engine_state(&shell, cx);
    assert_eq!(enabled.live(), LiveState::EnabledBlack);
    assert_eq!(enabled.program(), ProgramContent::Black);
    assert_eq!(windows_open(cx), 2, "one congregation window");
    assert!(presenter.read_with(cx, |view, _| view.output_window().is_some()));

    // 4. Take live.
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    presenter.update(cx, |view, _| {
        let closed = log.clone();
        view.set_session_hooks(crate::presenter::session_header::exit::SessionHooks {
            close_output: Rc::new(move |engine| {
                closed
                    .borrow_mut()
                    .push(format!("close:{:?}", engine.state().live()));
            }),
            on_session_ended: Rc::new(|| {}),
            ..Default::default()
        });
    });
    keys(cx, "space");
    let taken = engine_state(&shell, cx);
    assert_eq!(taken.live(), LiveState::Live);
    assert!(
        matches!(taken.program(), ProgramContent::Frame { cue, .. } if cue == taken.preview()),
        "Program shows the Cue that was Preview: {:?}",
        taken.program()
    );

    // 5. Exit, then Stay.
    click(cx, "header-exit-presenter");
    assert!(presenter.read_with(cx, |view, cx| view.exit_confirmation_open(cx)));
    assert_eq!(engine_state(&shell, cx), taken, "asking changes nothing");
    assert_eq!(windows_open(cx), 2);
    click(cx, "button-Stay in Presenter");
    assert!(shell.read_with(cx, |shell, _| shell.presenter_mode().is_presenter()));
    assert!(!presenter.read_with(cx, |view, cx| view.exit_confirmation_open(cx)));
    assert_eq!(engine_state(&shell, cx), taken, "still live");
    assert_eq!(windows_open(cx), 2, "output window still open");
    assert!(log.borrow().is_empty());

    // 6. Exit, then End presentation.
    click(cx, "header-exit-presenter");
    click(cx, "button-End presentation");
    assert_eq!(
        *log.borrow(),
        ["close:Blackout"],
        "the hook, which runs after the window closes, saw black"
    );
    assert_eq!(
        presenter.read_with(cx, |view, _| view.output_closed_at()),
        [LiveState::Blackout],
        "the window was closed while Program was black"
    );

    // 7. Preparation restored.
    shell.read_with(cx, |shell, _| {
        assert!(!shell.presenter_mode().is_presenter());
        assert!(shell.presenter().is_none());
    });
    assert!(!fullscreen(cx));
    assert_eq!(windows_open(cx), 1, "the congregation window is gone");
    assert!(cx.debug_bounds("workspace-header").is_some());
    assert!(cx.debug_bounds("presenter-header").is_none());
    assert_eq!(shell.read_with(cx, preparation_state), before);
    let focused = cx.update(|window, cx| {
        shell.read_with(cx, |shell, _| {
            shell.focus_handle.contains_focused(window, cx)
        })
    });
    assert!(focused, "focus returns to the shell");
}
