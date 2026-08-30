use gpui::TestAppContext;
use ordo::{CongregationView, open_native_runtime};
use presentation::{CongregationState, OutputStatus, ProgramFrame};
use ui::OpenSongs;

#[gpui::test]
fn closing_operator_closes_congregation_and_owned_workers(cx: &mut TestAppContext) {
    let runtime = cx.update(|cx| open_native_runtime(cx, true).expect("runtime should open"));
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);

    runtime
        .operator()
        .update(cx, |_, window, _| window.remove_window())
        .expect("operator window should still be open");
    cx.run_until_parked();

    assert_eq!(cx.read(|cx| cx.windows().len()), 0);
    assert!(runtime.shutdown_was_orderly());
}

#[gpui::test]
fn congregation_surface_has_only_program_and_output_status(cx: &mut TestAppContext) {
    let state = CongregationState::new(
        ProgramFrame::new(7, vec![0, 0, 0, 255]).expect("complete frame"),
        OutputStatus::Connected,
    );
    let (view, cx) = cx.add_window_view(|_, cx| CongregationView::new(state, cx));

    view.read_with(cx, |view, _| {
        assert_eq!(view.state().program().revision(), 7);
        assert_eq!(view.state().output_status(), OutputStatus::Connected);
    });
    assert!(cx.debug_bounds("program-frame-7").is_some());
    assert!(cx.debug_bounds("output-status-Connected").is_some());
    assert!(!cx.update(|_, cx| cx.is_action_available(&OpenSongs)));
    assert!(cx.update(|window, cx| window.focused(cx).is_none()));
}
