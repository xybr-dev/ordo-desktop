//! Quit confirmation: closing the operator window (Alt+F4) asks first.
//!
//! GPUI's test platform has no way to deliver a close request, so these call
//! `allow_close`, the exact function the window's should-close handler runs.

use super::*;
use platform::display::{
    DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
    PlacementBounds,
};
use presentation::state::LiveState;
use std::cell::RefCell;
use std::rc::Rc;

fn request_close(shell: &gpui::Entity<ShellView>, cx: &mut VisualTestContext) -> bool {
    let allowed = shell.update_in(cx, |shell, window, cx| shell.allow_close(window, cx));
    cx.run_until_parked();
    allowed
}

fn dialog_open(shell: &gpui::Entity<ShellView>, cx: &mut VisualTestContext) -> bool {
    let top = shell.read_with(cx, |shell, cx| match shell.presenter() {
        Some(presenter) => presenter.read(cx).overlay_top(cx),
        None => shell.overlays.top(cx),
    });
    top.as_deref() == Some(quit::TITLE)
}

fn settle(cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(
            ui::tokens::motion::OVERLAY_CLOSE_DURATION_MS.into(),
        ));
    cx.run_until_parked();
}

/// For after the operator window is gone, when a window-bound context can no
/// longer be used.
fn windows_left(cx: &VisualTestContext) -> usize {
    cx.cx.windows().len()
}

#[gpui::test]
fn a_close_request_asks_first_and_the_window_stays(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    assert!(!request_close(&shell, cx), "the close is cancelled");
    assert!(dialog_open(&shell, cx));
    assert_eq!(windows_open(cx), 1);
    assert!(cx.debug_bounds("button-Quit").is_some());
    assert!(cx.debug_bounds("button-Cancel").is_some());
}

#[gpui::test]
fn enter_quits_because_quit_holds_initial_focus(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    request_close(&shell, cx);
    keys(cx, "enter");
    settle(cx);
    assert_eq!(windows_left(cx), 0, "the operator window closed");
}

#[gpui::test]
fn escape_and_cancel_keep_the_window_open(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    request_close(&shell, cx);
    keys(cx, "escape");
    settle(cx);
    assert_eq!(windows_open(cx), 1);
    assert!(!dialog_open(&shell, cx));
    assert!(cx.debug_bounds("button-Quit").is_none(), "no dialog left");

    request_close(&shell, cx);
    click(cx, "button-Cancel");
    settle(cx);
    assert_eq!(windows_open(cx), 1);
    assert!(cx.debug_bounds("button-Quit").is_none(), "no dialog left");
}

#[gpui::test]
fn right_then_enter_activates_cancel(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    request_close(&shell, cx);
    keys(cx, "right enter");
    settle(cx);
    assert_eq!(windows_open(cx), 1);
    assert!(cx.debug_bounds("button-Quit").is_none());
}

#[gpui::test]
fn a_second_close_request_does_not_stack_a_second_dialog(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    request_close(&shell, cx);
    assert!(!request_close(&shell, cx));
    assert_eq!(shell.read_with(cx, |shell, cx| shell.overlays.depth(cx)), 1);
    keys(cx, "escape");
    settle(cx);
    assert_eq!(shell.read_with(cx, |shell, cx| shell.overlays.depth(cx)), 0);
}

#[gpui::test]
fn a_confirmed_quit_lets_the_close_through(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    shell.update(cx, |shell, _| shell.quit_confirmed = true);
    assert!(request_close(&shell, cx));
}

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

#[gpui::test]
fn quitting_a_live_presentation_blacks_out_before_the_output_closes(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    click(cx, "button-Present");
    let presenter = shell.read_with(cx, |shell, _| shell.presenter().cloned().unwrap());
    let adapter = Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 1920, 1080), display(2, 1920, 1280, 720)],
        Some(DisplayId::new(1)),
    )));
    presenter.update_in(cx, |view, window, cx| {
        view.install_output(Box::new(adapter), window, cx)
    });
    cx.run_until_parked();
    click(cx, "primary-action");
    keys(cx, "space");
    let state = presenter.read_with(cx, |view, _| view.engine().state().live());
    assert_eq!(state, LiveState::Live);
    assert_eq!(windows_open(cx), 2);

    assert!(!request_close(&shell, cx));
    assert!(dialog_open(&shell, cx));
    assert_eq!(windows_open(cx), 2, "asking closes nothing");
    keys(cx, "enter");
    settle(cx);
    assert_eq!(
        presenter.read_with(cx, |view, _| view.output_closed_at()),
        [LiveState::Blackout],
        "the congregation window closed while Program was black"
    );
    assert_eq!(windows_left(cx), 0);
}
