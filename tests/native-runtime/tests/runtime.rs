use gpui::TestAppContext;
use ordo::{
    CongregationView, open_native_runtime_at, open_native_runtime_with,
    shell::{OpenSongs, UseDarkAppearance},
};
use platform::display::{
    DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
    PlacementBounds,
};
use presentation::{CongregationState, OutputStatus, ProgramFrame};
use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};
use ui::{
    IncreaseInterfaceScale,
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

fn display(id: u64, x: i32) -> DetectedDisplay {
    let bounds = DisplayBounds::new(x, 0, 1920, 1080);
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
fn closing_operator_closes_congregation_and_owned_workers(cx: &mut TestAppContext) {
    // The runtime's preferences worker is a real OS thread whose restore wakes a
    // GPUI task; without this the test scheduler reports that cross-thread
    // wakeup as non-determinism whenever the worker finishes after the task
    // registered its waker, as on a loaded CI runner.
    cx.executor().allow_parking();
    let preferences = temporary_preferences("runtime-shutdown");
    let displays = Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0), display(2, 1920)],
        Some(DisplayId::new(1)),
    )));
    let runtime = cx.update(|cx| {
        open_native_runtime_with(cx, &preferences, displays).expect("runtime should open")
    });
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    cx.update(|cx| {
        runtime.output().update(cx, |output, cx| {
            output.assign(DisplayId::new(2), cx).unwrap();
            output.enable_output(cx).expect("output should enable");
        })
    });
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
    // The preferences worker is a real OS thread, so the restore crosses a
    // boundary the test executor cannot see. Without this it advances its
    // virtual clock past `condition`'s 3s timer while the read is still in
    // flight, and the test fails on a loaded machine rather than on a defect.
    cx.executor().allow_parking();
    let preferences = temporary_preferences("runtime-preferences");
    fs::write(&preferences, "appearance=light\nscale=175\n").unwrap();
    let runtime =
        cx.update(|cx| open_native_runtime_at(cx, &preferences).expect("runtime should open"));
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
        ProgramFrame::new(7, 1, 1, vec![0, 0, 0, 255]).expect("complete frame"),
        OutputStatus::Connected,
    );
    let (view, cx) = cx.add_window_view(|_, cx| CongregationView::new(state, cx));

    view.read_with(cx, |view, _| {
        assert_eq!(view.state().program().revision(), 7);
        assert_eq!(view.state().output_status(), OutputStatus::Connected);
    });
    // The selector names the revision actually painted, which lands once the
    // background encode completes.
    cx.run_until_parked();
    assert!(cx.debug_bounds("program-frame-7").is_some());
    assert!(cx.debug_bounds("output-status-Connected").is_some());
    assert!(!cx.update(|_, cx| cx.is_action_available(&OpenSongs)));
    assert!(cx.update(|window, cx| window.focused(cx).is_none()));
}
