//! Development-only display lab (T0.5 manual gate).
//!
//! `cargo run -p ordo --example display-lab` opens the real operator runtime
//! on the real GPUI display Adapter so the `[MANUAL]` pointer and
//! cursor-locator criterion can be judged on physical hardware. It is not an
//! application workflow: Presenter Mode owns output controls in later
//! milestones. Installation preferences go to a temporary file, so the lab
//! never touches real Parish data.
//!
//! Keys, with the operator window focused:
//! `F6` assigns the only non-operator display and enables output (black);
//! `F7` takes a test pattern live; `F8` re-reads the display topology;
//! `F9` disables output. Each step logs the topology and output state to
//! stderr.

use gpui::{App, Entity, KeyBinding, actions};
use gpui_platform::application;
use ordo::{open_native_runtime_at, output::CongregationOutput};
use presentation::ProgramFrame;

actions!(
    display_lab,
    [EnableOutput, TakePatternLive, Refresh, DisableOutput]
);

/// A 16x9 pattern the congregation surface scales to the full display: a
/// dark field with a white frame and a white centre cross, so edges and
/// centre are both checkable from the room.
fn test_pattern() -> ProgramFrame {
    const W: u32 = 16;
    const H: u32 = 9;
    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let edge = x == 0 || y == 0 || x == W - 1 || y == H - 1;
            let cross = x == W / 2 || y == H / 2;
            let pixel = if edge || cross {
                [255, 255, 255, 255]
            } else {
                [24, 40, 72, 255]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    ProgramFrame::new(0, W, H, rgba).expect("the pattern is a complete frame")
}

fn log_state(output: &CongregationOutput, step: &str) {
    let control = output.display_control();
    eprintln!("--- {step}");
    eprintln!("operator display: {:?}", control.topology().operator());
    for display in control.topology().displays() {
        let direction = control.topology().direction_from_operator(display.id);
        eprintln!(
            "  {:?} {:?} direction from operator: {direction:?}",
            display.id, display.evidence.bounds
        );
    }
    eprintln!("assignment: {:?}", control.assignment());
    eprintln!("output: {:?}", control.output());
    eprintln!("status: {:?}", control.status());
}

fn bind(output: &Entity<CongregationOutput>, cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f6", EnableOutput, None),
        KeyBinding::new("f7", TakePatternLive, None),
        KeyBinding::new("f8", Refresh, None),
        KeyBinding::new("f9", DisableOutput, None),
    ]);

    let target = output.clone();
    cx.on_action(move |_: &EnableOutput, cx| {
        target.update(cx, |output, cx| {
            output.refresh_displays(cx);
            if let Some(candidate) = output.display_control().suggestion() {
                if let Err(error) = output.assign(candidate, cx) {
                    eprintln!("assign failed: {error:?}");
                }
            } else {
                eprintln!("no single non-operator display to assign");
            }
            if let Err(error) = output.enable_output(cx) {
                eprintln!("enable failed: {error:?}");
            }
            log_state(output, "F6 enable output");
        });
    });

    let target = output.clone();
    cx.on_action(move |_: &TakePatternLive, cx| {
        target.update(cx, |output, cx| {
            if let Err(error) = output.take_live(test_pattern(), cx) {
                eprintln!("take live rejected: {error:?}");
            }
            log_state(output, "F7 take pattern live");
        });
    });

    let target = output.clone();
    cx.on_action(move |_: &Refresh, cx| {
        target.update(cx, |output, cx| {
            let events = output.refresh_displays(cx);
            eprintln!("events: {events:?}");
            log_state(output, "F8 refresh");
        });
    });

    let target = output.clone();
    cx.on_action(move |_: &DisableOutput, cx| {
        target.update(cx, |output, cx| {
            output.disable_output(cx);
            log_state(output, "F9 disable output");
        });
    });
}

fn main() {
    let preferences = std::env::temp_dir().join(format!("ordo-display-lab-{}", std::process::id()));
    application().run(move |cx: &mut App| {
        let runtime =
            open_native_runtime_at(cx, &preferences).expect("the native runtime should open");
        let output = runtime.output();
        log_state(output.read(cx), "startup");
        bind(&output, cx);
        cx.activate(true);
    });
}
