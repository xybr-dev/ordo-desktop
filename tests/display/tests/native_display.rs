//! `[NATIVE-AUTO]` T0.5: enumerate the runner's real displays through the
//! production GPUI Adapter, then drive the congregation-output composition
//! with that topology.
//!
//! **Real:** the display enumeration. `gpui_platform::current_platform(true)`
//! lists monitors through `EnumDisplayMonitors`, and `topology_from_displays`
//! turns them into Display Control values with their ids, bounds, uuids, and
//! the primary display as operator.
//!
//! **Synthetic:** everything after enumeration. A headless test cannot show a
//! window on a real runner display, so the composition runs under
//! `TestAppContext`, with its display source the in-memory Adapter seeded from
//! the native topology. The refreshes (removal, re-adding under new ids, an
//! operator swap) are synthetic edits to that source, and window placement is
//! the source's requested placement, not a native one. A single-display runner
//! also gets a synthetic projector so the enabled path runs.
//!
//! Windows-only: Ordo is Windows-only for the MVP. The CI
//! `windows-display-control` job is the mandatory [NATIVE-AUTO] evidence gate;
//! this file also runs inside the workspace suite. The platform is created
//! headless (`WindowsPlatform::new(true)`), which still enumerates monitors
//! through `EnumDisplayMonitors` without opening a window.

#![cfg(target_os = "windows")]

use std::{cell::RefCell, rc::Rc};

use gpui::{AppContext, Entity, TestAppContext};
use ordo::output::CongregationOutput;
use platform::display::{
    DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, DisplayStatus,
    InMemoryDisplayAdapter, OutputLifecycle, PlacementBounds, Topology,
    gpui_adapter::topology_from_displays,
};

/// Real enumeration: the runner's displays as the GPUI Adapter reports them.
fn native_topology() -> Topology {
    let platform = gpui_platform::current_platform(true);
    let operator = platform.primary_display().map(|display| display.id());
    topology_from_displays(&platform.displays(), operator)
}

#[test]
fn native_runner_displays_are_enumerated() {
    let native = native_topology();

    assert!(
        !native.displays().is_empty(),
        "the runner must expose at least one display"
    );
    assert!(
        native.operator().is_some(),
        "a primary display is the operator"
    );
    for display in native.displays() {
        let bounds = display.evidence.bounds;
        assert!(bounds.width > 0 && bounds.height > 0, "{display:?}");
        println!(
            "native display {:#x}: {}x{} at ({}, {}), uuid {:?}",
            display.id.raw(),
            bounds.width,
            bounds.height,
            bounds.x,
            bounds.y,
            display.evidence.uuid
        );
    }
}

/// Synthetic refreshes over the native topology, driven through the
/// production controller rather than Display Control alone.
#[test]
fn native_topology_drives_the_congregation_composition() {
    let native = native_topology();
    let operator = native
        .operator()
        .expect("a primary display is the operator");
    let initial = native.clone();

    // Synthetic: a single-display runner gets a projector to the right of the
    // operator so the enabled path runs.
    let mut displays = native.displays().to_vec();
    let candidate = match native.candidates().next() {
        Some(display) => display.id,
        None => {
            let operator_bounds = native.display(operator).unwrap().evidence.bounds;
            let bounds = DisplayBounds::new(
                operator_bounds.x + operator_bounds.width as i32,
                operator_bounds.y,
                1280,
                720,
            );
            let synthetic = DetectedDisplay {
                id: DisplayId::new(u64::MAX - 1),
                evidence: DisplayEvidence {
                    name: Some("Synthetic projector".to_owned()),
                    uuid: None,
                    bounds,
                },
                placement: PlacementBounds::from(bounds),
            };
            displays.push(synthetic.clone());
            synthetic.id
        }
    };
    let source = Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        displays,
        native.operator(),
    )));

    // The production controller, with no operator window in this composition.
    let mut cx = TestAppContext::single();
    let output = cx.new(|cx| CongregationOutput::new(Box::new(source.clone()), None, cx));
    cx.update(|cx| output.update(cx, |output, cx| output.assign(candidate, cx)))
        .expect("the candidate display is assignable");
    cx.update(|cx| output.update(cx, |output, cx| output.enable_output(cx)))
        .expect("output enables on the candidate display");
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);

    // Synthetic refresh 1: the candidate is unplugged. Output goes
    // Disconnected and its window closes.
    source.borrow_mut().disconnect(candidate);
    refresh(&mut cx, &output);
    assert!(is_disconnected(&mut cx, &output));
    assert_eq!(cx.read(|cx| cx.windows().len()), 0);

    // Synthetic refresh 2: every display returns under a new OS id, and the
    // operator moves with its display.
    let renumbered = |id: DisplayId| DisplayId::new(id.raw() ^ 0x5A5A_0000);
    for display in initial.displays() {
        source.borrow_mut().connect(DetectedDisplay {
            id: renumbered(display.id),
            ..display.clone()
        });
    }
    source.borrow_mut().set_operator(Some(renumbered(operator)));
    refresh(&mut cx, &output);
    assert!(
        matches!(
            cx.read(|cx| output.read(cx).display_control().status()),
            DisplayStatus::Disconnected { .. }
        ),
        "reconnection never resumes output"
    );
    assert!(is_disconnected(&mut cx, &output));

    // Synthetic refresh 3: the operator moves onto the reconnected candidate.
    source
        .borrow_mut()
        .set_operator(Some(renumbered(candidate)));
    refresh(&mut cx, &output);
    assert!(is_disconnected(&mut cx, &output));

    // Finally the captured native topology again, through the explicit
    // `refresh_with` path rather than the source.
    cx.update(|cx| {
        output.update(cx, |output, cx| {
            output.refresh_with(initial.clone(), cx);
        })
    });
    cx.run_until_parked();
    assert!(is_disconnected(&mut cx, &output));
    assert_eq!(cx.read(|cx| cx.windows().len()), 0);
}

fn refresh(cx: &mut TestAppContext, output: &Entity<CongregationOutput>) {
    cx.update(|cx| {
        output.update(cx, |output, cx| {
            output.refresh_displays(cx);
        })
    });
    cx.run_until_parked();
}

fn is_disconnected(cx: &mut TestAppContext, output: &Entity<CongregationOutput>) -> bool {
    let lifecycle = cx.read(|cx| output.read(cx).display_control().output().clone());
    matches!(lifecycle, OutputLifecycle::Disconnected { .. })
}
