use gpui::TestAppContext;
use ordo::{CongregationView, open_native_runtime_at};
use presentation::{CongregationState, OutputStatus, ProgramFrame};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use ui::{
    IncreaseInterfaceScale, OpenSongs, UseDarkAppearance,
    environment::{InterfaceScale, PreparationAppearance},
};

fn temporary_preferences(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ordo-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[gpui::test]
fn closing_operator_closes_congregation_and_owned_workers(cx: &mut TestAppContext) {
    let preferences = temporary_preferences("runtime-shutdown");
    let runtime = cx
        .update(|cx| open_native_runtime_at(cx, true, &preferences).expect("runtime should open"));
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);

    runtime
        .operator()
        .update(cx, |_, window, _| window.remove_window())
        .expect("operator window should still be open");
    cx.run_until_parked();

    assert_eq!(cx.read(|cx| cx.windows().len()), 0);
    assert!(
        runtime
            .shutdown_report()
            .is_some_and(|report| report.is_empty())
    );
}

#[gpui::test]
async fn runtime_restores_and_persists_installation_preferences(cx: &mut TestAppContext) {
    let preferences = temporary_preferences("runtime-preferences");
    fs::write(&preferences, "appearance=light\nscale=175\n").unwrap();
    let runtime = cx
        .update(|cx| open_native_runtime_at(cx, false, &preferences).expect("runtime should open"));
    let root = runtime
        .operator()
        .update(cx, |_, _, cx| cx.entity())
        .unwrap();
    cx.condition(&root, |root, _| {
        root.environment().preferences().appearance == PreparationAppearance::Light
            && root.environment().preferences().scale == InterfaceScale::Percent175
    })
    .await;

    runtime
        .operator()
        .update(cx, |root, _, _| {
            assert_eq!(
                root.environment().preferences().appearance,
                PreparationAppearance::Light
            );
            assert_eq!(
                root.environment().preferences().scale,
                InterfaceScale::Percent175
            );
        })
        .unwrap();

    cx.dispatch_action(runtime.operator().into(), UseDarkAppearance);
    cx.dispatch_action(runtime.operator().into(), IncreaseInterfaceScale);
    runtime
        .operator()
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        fs::read_to_string(&preferences).unwrap(),
        "appearance=dark\nscale=200\n"
    );
    fs::remove_file(preferences).unwrap();
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
