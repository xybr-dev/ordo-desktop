//! The congregation output as the Presenter drives it (T1.6 WP3 and WP4):
//! Enable Output refuses without a display, opens one passive window with
//! one, draws the same frame as Program, and closes with the output.
//!
//! The harness is `super`'s; displays come from the in-memory adapter.

use super::*;
use gpui::{VisualTestContext, point, px};
use platform::display::{
    DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
    OutputLifecycle, PlacementBounds,
};

/// Selectors only the operator's Presenter draws. None may reach the output.
const OPERATOR_ONLY: [&str; 10] = [
    "program-state",
    "monitor-program-preparing",
    "program-auto-play",
    "program-auto-play-progress",
    "primary-action",
    "display-health",
    "presenter-header",
    "header-cursor-locator",
    "recovery-strip",
    "recovery-prior",
];

type Adapter = Rc<RefCell<InMemoryDisplayAdapter>>;

fn display(id: u64, x: i32, y: i32, width: u32, height: u32) -> DetectedDisplay {
    let bounds = DisplayBounds::new(x, y, width, height);
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

/// An operator display, plus a projector when `projector`.
fn displays(projector: bool) -> Adapter {
    let mut all = vec![display(1, 0, 0, 1920, 1080)];
    if projector {
        all.push(display(2, 1920, 0, 1280, 720));
    }
    Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        all,
        Some(DisplayId::new(1)),
    )))
}

fn install(open: &Open, cx: &mut VisualTestContext, adapter: &Adapter) {
    let adapter = adapter.clone();
    open.view.update_in(cx, |view, window, cx| {
        view.install_output(Box::new(adapter), window, cx)
    });
    cx.run_until_parked();
}

fn window_count(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| cx.windows().len())
}

fn output_window(
    open: &Open,
    cx: &VisualTestContext,
) -> Option<gpui::WindowHandle<output_window::OutputWindowView>> {
    open.view.read_with(cx, |view, _| view.output_window())
}

/// A context on the congregation window, drawn at the projector's size.
fn on_output(open: &Open, cx: &mut VisualTestContext) -> VisualTestContext {
    let handle = output_window(open, cx).expect("the output window is open");
    let output = VisualTestContext::from_window(handle.into(), cx);
    output.simulate_resize(gpui::size(px(1280.0), px(720.0)));
    output.run_until_parked();
    output
}

fn drawn_by_output(open: &Open, cx: &mut VisualTestContext) -> Option<Arc<rendering::Frame>> {
    let handle = output_window(open, cx).expect("the output window is open");
    on_output(open, cx);
    handle
        .update(cx, |view, _, _| view.drawn_frame().cloned())
        .unwrap()
}

#[gpui::test]
fn enable_with_no_candidate_display_is_refused_and_opens_nothing(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(false));
    let before = window_count(cx);
    open.view
        .update(cx, |view, _| view.sent.borrow_mut().clear());

    click(cx, "primary-action");

    assert_eq!(
        status_line(&open, cx),
        "No congregation display found — rehearsing on Preview"
    );
    assert_eq!(window_count(cx), before, "no window opened");
    assert_eq!(live(&open, cx), LiveState::OutputDisabled);
    let sent = open
        .view
        .read_with(cx, |view, _| view.sent.borrow().clone());
    assert!(sent.is_empty(), "the Engine was not called");
}

#[gpui::test]
fn enable_with_one_candidate_opens_one_window_and_program_is_black(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(true));
    let before = window_count(cx);

    click(cx, "primary-action");

    assert_eq!(window_count(cx), before + 1, "exactly one extra window");
    assert_eq!(
        live(&open, cx),
        LiveState::EnabledBlack,
        "black before Take"
    );
    assert!(
        drawn_by_output(&open, cx).is_none(),
        "the output paints black"
    );

    // A second Enable does not open a second window.
    open.view.update_in(cx, |view, window, cx| {
        view.user_command(Command::EnableOutput, window, cx)
    });
    cx.run_until_parked();
    assert_eq!(window_count(cx), before + 1);
}

#[gpui::test]
fn the_output_draws_programs_own_frame_and_nothing_operator_only(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(true));
    click(cx, "primary-action");
    run(&open, cx, Command::Take).unwrap();
    settle(&open, cx, 10_000);
    assert_eq!(live(&open, cx), LiveState::Live);

    let program = open
        .view
        .read_with(cx, |view, _| view.last_program.frame.clone())
        .expect("Program drew a frame");
    let output = drawn_by_output(&open, cx).expect("the output drew a frame");
    assert!(Arc::ptr_eq(&program, &output), "the same Arc<Frame>");

    let mut on_output = on_output(&open, cx);
    assert!(on_output.debug_bounds("output-window").is_some());
    for selector in OPERATOR_ONLY {
        assert!(
            on_output.debug_bounds(selector).is_none(),
            "{selector} is operator-only"
        );
    }
}

#[gpui::test]
fn a_confirmed_exit_closes_the_window_after_black_and_before_disable(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let log = watch_exit(&open, cx);
    install(&open, cx, &displays(true));
    let before = window_count(cx);
    click(cx, "primary-action");
    run(&open, cx, Command::Take).unwrap();
    settle(&open, cx, 10_000);
    assert_eq!(window_count(cx), before + 1);

    click(cx, "header-exit-presenter");
    assert_eq!(window_count(cx), before + 1, "asking does not close it");
    click(cx, "button-End presentation");

    assert_eq!(window_count(cx), before, "the output window is closed");
    let closed_at = open.view.read_with(cx, |view, _| {
        view.output.as_ref().unwrap().closed_at.clone()
    });
    assert_eq!(closed_at, [LiveState::Blackout], "Program was black first");
    assert_eq!(*log.borrow(), ["close:Blackout", "ended", "exit"]);
    assert_eq!(live(&open, cx), LiveState::OutputDisabled);
}

#[gpui::test]
fn disabling_the_output_closes_the_window(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(true));
    let before = window_count(cx);
    click(cx, "primary-action");
    assert_eq!(window_count(cx), before + 1);

    run(&open, cx, Command::DisableOutput).unwrap();

    assert_eq!(window_count(cx), before);
    assert!(output_window(&open, cx).is_none());
    assert_eq!(live(&open, cx), LiveState::OutputDisabled);
}

#[gpui::test]
fn a_window_closed_by_the_system_disables_the_output(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(true));
    let before = window_count(cx);
    click(cx, "primary-action");
    let handle = output_window(&open, cx).unwrap();

    // Closed from outside the owner, as the system would.
    handle
        .update(cx, |_, window, _| window.remove_window())
        .ok();
    cx.run_until_parked();

    assert_eq!(window_count(cx), before);
    assert!(output_window(&open, cx).is_none());
    assert_eq!(live(&open, cx), LiveState::OutputDisabled);
}

#[gpui::test]
fn the_pointer_on_the_output_shows_the_way_back_in_the_header(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(true));
    click(cx, "primary-action");
    let mut output = on_output(&open, cx);
    let move_to = |output: &mut VisualTestContext, x: f32, y: f32| {
        output.simulate_mouse_move(point(px(x), px(y)), None, gpui::Modifiers::default());
        output.run_until_parked();
    };
    // The click that enabled the output left the pointer where the new
    // window now is, so start from a point outside it.
    move_to(&mut output, 5000.0, 5000.0);
    cx.run_until_parked();
    assert!(
        open.view
            .read_with(cx, |view, _| view.cursor_locator())
            .is_none()
    );
    assert!(cx.debug_bounds("header-cursor-locator").is_none());

    move_to(&mut output, 100.0, 100.0);
    cx.run_until_parked();

    let locator = open.view.read_with(cx, |view, _| view.cursor_locator());
    assert_eq!(
        locator.map(|locator| locator.direction),
        Some(platform::display::RelativeDirection::Left),
        "the operator display is to the left of the projector"
    );
    assert!(cx.debug_bounds("header-cursor-locator").is_some());
}

// ---- Display loss and return (T1.6c) --------------------------------------

fn projector(id: u64, name: &str) -> DetectedDisplay {
    let mut projector = display(id, 1920, 0, 1280, 720);
    projector.evidence.name = Some(name.into());
    projector
}

/// An operator display and a projector named "Projector".
fn named_displays() -> Adapter {
    Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 0, 1920, 1080), projector(2, "Projector")],
        Some(DisplayId::new(1)),
    )))
}

/// What the operator window's bounds observer does after a display change,
/// without going through the platform: read the displays and act on them.
fn display_changed(open: &Open, cx: &mut VisualTestContext) {
    open.view.update_in(cx, |view, window, cx| {
        let operator = crate::output::window_display(window, &cx.displays(), cx);
        view.display_changed(operator, cx)
    });
    cx.run_until_parked();
}

fn sent(open: &Open, cx: &VisualTestContext) -> Vec<Command> {
    open.view
        .read_with(cx, |view, _| view.sent.borrow().clone())
}

fn clear_sent(open: &Open, cx: &mut VisualTestContext) {
    open.view
        .update(cx, |view, _| view.sent.borrow_mut().clear());
}

/// Live on the projector, then the projector is unplugged.
fn live_then_unplugged(
    open: &Open,
    cx: &mut VisualTestContext,
    adapter: &Adapter,
) -> DetectedDisplay {
    install(open, cx, adapter);
    click(cx, "primary-action");
    run(open, cx, Command::Take).unwrap();
    settle(open, cx, 10_000);
    assert_eq!(live(open, cx), LiveState::Live);
    let unplugged = adapter
        .borrow_mut()
        .disconnect(DisplayId::new(2))
        .expect("the projector was connected");
    display_changed(open, cx);
    unplugged
}

#[gpui::test]
fn losing_the_display_with_no_key_press_closes_the_window_and_disconnects(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    install(&open, cx, &adapter);
    let before = window_count(cx);
    click(cx, "primary-action");
    run(&open, cx, Command::Take).unwrap();
    settle(&open, cx, 10_000);
    assert_eq!(window_count(cx), before + 1);
    clear_sent(&open, cx);

    adapter.borrow_mut().disconnect(DisplayId::new(2));
    display_changed(&open, cx);

    assert_eq!(sent(&open, cx), [Command::DisplayLost]);
    assert_eq!(
        live(&open, cx),
        LiveState::Disconnected {
            display_back: false
        }
    );
    assert_eq!(window_count(cx), before, "back to the operator window only");
    assert!(output_window(&open, cx).is_none());
    assert_eq!(status_line(&open, cx), "Congregation output disconnected");
    assert!(
        cx.debug_bounds("program-state").is_some(),
        "Program shows the disconnected placeholder"
    );
    assert!(cx.debug_bounds("display-health").is_some());
    assert!(
        cx.debug_bounds("button-Resume Output").is_none(),
        "no stale Live and no resume without a display"
    );

    // Nothing opens on the operator display, whatever else changes.
    display_changed(&open, cx);
    assert_eq!(window_count(cx), before);
    assert_eq!(
        sent(&open, cx),
        [Command::DisplayLost],
        "one event per fact"
    );
}

/// Decision 272: Space on an unavailable Preview is refused even with the
/// display back; output stays Disconnected and no window opens.
#[gpui::test]
fn space_on_an_unavailable_preview_keeps_the_output_disconnected(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    let before = window_count(cx);
    let unplugged = live_then_unplugged(&open, cx, &adapter);
    adapter.borrow_mut().connect(unplugged);
    display_changed(&open, cx);
    let frame = open.view.read_with(cx, |view, _| {
        let snapshot = view.engine.snapshot();
        let preview = view.engine.state().preview();
        match snapshot.cue(preview).unwrap().behavior() {
            CueBehavior::Project { frame, .. } => *frame,
            other => panic!("Preview projects: {other:?}"),
        }
    });
    let _ = run(
        &open,
        cx,
        Command::FrameUnavailable {
            frame,
            reason: rendering::UnavailableReason::DecodeFailed,
        },
    );
    clear_sent(&open, cx);

    open.view.update_in(cx, |view, window, cx| {
        view.user_command(Command::Take, window, cx)
    });
    cx.run_until_parked();

    assert_eq!(
        live(&open, cx),
        LiveState::Disconnected { display_back: true }
    );
    assert_eq!(window_count(cx), before, "no window opens");
    assert!(output_window(&open, cx).is_none());
    assert!(
        !status_line(&open, cx).is_empty(),
        "the operator is told why"
    );
}

#[gpui::test]
fn the_same_display_returning_offers_resume_and_space_goes_live_on_it(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    let before = window_count(cx);
    let unplugged = live_then_unplugged(&open, cx, &adapter);
    // The port comes back under a new operating-system id.
    adapter.borrow_mut().connect(DetectedDisplay {
        id: DisplayId::new(9),
        ..unplugged
    });
    display_changed(&open, cx);

    assert_eq!(
        live(&open, cx),
        LiveState::Disconnected { display_back: true }
    );
    assert!(cx.debug_bounds("button-Resume Output").is_some());
    assert_eq!(window_count(cx), before, "no window until Resume");

    open.view.update_in(cx, |view, window, cx| {
        view.user_command(Command::Take, window, cx)
    });
    cx.run_until_parked();

    assert_eq!(window_count(cx), before + 1, "one output window");
    assert_eq!(live(&open, cx), LiveState::Live);
    assert!(output_window(&open, cx).is_some(), "the window is back");
    settle(&open, cx, 20_000);
    let program = open
        .view
        .read_with(cx, |view, _| view.last_program.frame.clone())
        .expect("Program drew Preview's frame");
    let output = drawn_by_output(&open, cx).expect("the output draws it");
    assert!(Arc::ptr_eq(&program, &output));
}

#[gpui::test]
fn another_display_with_the_same_resolution_is_refused(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    let before = window_count(cx);
    live_then_unplugged(&open, cx, &adapter);
    clear_sent(&open, cx);

    adapter.borrow_mut().connect(projector(5, "Other TV"));
    display_changed(&open, cx);

    assert_eq!(
        sent(&open, cx),
        [Command::DisplayReturned { recognized: false }]
    );
    let disconnected = LiveState::Disconnected {
        display_back: false,
    };
    assert_eq!(live(&open, cx), disconnected);
    assert!(cx.debug_bounds("button-Resume Output").is_none());
    clear_sent(&open, cx);

    open.view.update_in(cx, |view, window, cx| {
        view.user_command(Command::Take, window, cx)
    });
    cx.run_until_parked();

    assert_eq!(
        status_line(&open, cx),
        "A different display is connected \u{2014} choose Select display"
    );
    assert!(sent(&open, cx).is_empty(), "the Engine was not touched");
    assert_eq!(live(&open, cx), disconnected);
    assert_eq!(window_count(cx), before, "no window");

    // Enable Output from here obeys the same rule.
    open.view.update_in(cx, |view, window, cx| {
        view.user_command(Command::EnableOutput, window, cx)
    });
    cx.run_until_parked();
    assert!(sent(&open, cx).is_empty());
    assert_eq!(window_count(cx), before);
}

#[gpui::test]
fn the_display_going_away_again_before_resume_is_noticed(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    let unplugged = live_then_unplugged(&open, cx, &adapter);
    adapter.borrow_mut().connect(unplugged);
    display_changed(&open, cx);
    assert_eq!(
        live(&open, cx),
        LiveState::Disconnected { display_back: true }
    );

    adapter.borrow_mut().disconnect(DisplayId::new(2));
    display_changed(&open, cx);

    assert_eq!(
        live(&open, cx),
        LiveState::Disconnected {
            display_back: false
        }
    );
}

#[gpui::test]
fn losing_a_display_while_output_is_disabled_leaves_the_engine_alone(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    install(&open, cx, &adapter);
    let before = window_count(cx);
    clear_sent(&open, cx);

    adapter.borrow_mut().disconnect(DisplayId::new(2));
    display_changed(&open, cx);
    adapter.borrow_mut().connect(projector(2, "Projector"));
    display_changed(&open, cx);

    assert!(sent(&open, cx).is_empty());
    assert_eq!(live(&open, cx), LiveState::OutputDisabled);
    assert_eq!(window_count(cx), before);
}

#[gpui::test]
fn a_bounds_change_of_the_operator_window_notices_the_loss(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    let adapter = named_displays();
    install(&open, cx, &adapter);
    click(cx, "primary-action");
    let before = window_count(cx);
    clear_sent(&open, cx);

    // The platform fires the operator window's bounds callbacks on a display
    // change (Decision 308); a resize drives the same observer.
    adapter.borrow_mut().disconnect(DisplayId::new(2));
    cx.simulate_resize(gpui::size(px(1000.0), px(700.0)));
    cx.run_until_parked();

    assert_eq!(sent(&open, cx), [Command::DisplayLost]);
    assert_eq!(window_count(cx), before - 1);
}

// ---- Recovery on the output owner (T1.7d review) ------------------------

/// A validated recovery of `snapshot`'s first Cue, interrupted live on a
/// display called `name` at the projector's place.
fn recovery_of(snapshot: &LoadedSnapshot, name: &str) -> presentation::journal::ValidatedRecovery {
    use presentation::journal::{
        BoundsRecord, DisplayRecord, LiveIntent, OutputEvidence, RecoveryPoint, SessionId,
        WrittenAt,
    };
    let cue = snapshot.snapshot().cue(0).unwrap().id();
    let point = RecoveryPoint {
        session_id: SessionId::parse("00000000-0000-4000-8000-000000000801").unwrap(),
        seq: 1,
        written_at: WrittenAt::parse("2026-11-08T09:00:00Z").unwrap(),
        snapshot_id: snapshot.snapshot().id(),
        manifest_hash: snapshot.manifest_hash(),
        source_dir: "fixture".to_owned(),
        preview: cue,
        program: Some(cue),
        state: LiveIntent::Live,
        output: OutputEvidence {
            enabled: true,
            display: Some(DisplayRecord {
                name: Some(name.to_owned()),
                uuid: None,
                bounds: BoundsRecord {
                    x: 1920,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
            }),
        },
        ended: false,
    };
    presentation::journal::validate(point, snapshot).expect("the point matches its snapshot")
}

/// The Engine cannot be made to refuse a validated recovery here (it checks
/// only what `validate` already did), so this drives the reset it falls back
/// to: a recovered controller with a window open goes back to a fresh,
/// disabled one, and Enable Output then behaves as in a new session.
#[gpui::test]
fn resetting_the_output_owner_forgets_the_journaled_display(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(true));
    let projector = DisplayEvidence {
        name: None,
        uuid: None,
        bounds: DisplayBounds::new(1920, 0, 1280, 720),
    };
    open.view.update_in(cx, |view, window, cx| {
        let operator = crate::output::window_display(window, &cx.displays(), cx);
        view.output
            .as_mut()
            .unwrap()
            .recover(projector, operator, cx);
        assert!(view.try_open_output_window(window, cx).is_ok());
    });
    cx.run_until_parked();
    assert!(output_window(&open, cx).is_some());

    open.view.update_in(cx, |view, window, cx| {
        let operator = crate::output::window_display(window, &cx.displays(), cx);
        view.output.as_mut().unwrap().reset(operator, cx);
        assert!(matches!(
            view.output.as_ref().unwrap().lifecycle(),
            OutputLifecycle::Disabled
        ));
    });
    cx.run_until_parked();
    assert!(output_window(&open, cx).is_none(), "the window closed");

    open.view
        .update_in(cx, |view, window, cx| view.enable_output(window, cx));
    cx.run_until_parked();
    assert_eq!(
        open.view
            .read_with(cx, |view, _| view.engine().state().live()),
        presentation::state::LiveState::EnabledBlack
    );
    assert!(output_window(&open, cx).is_some());
}

#[gpui::test]
fn the_strip_keeps_its_height_with_a_long_display_name(cx: &mut TestAppContext) {
    let (open, cx) = open(cx, SIZES[0]);
    install(&open, cx, &displays(false));
    let long = "A very long congregation display name that goes on and on ".repeat(4);
    let recovery = recovery_of(&fixture_snapshot(), &long);

    open.view.update_in(cx, |view, window, cx| {
        view.recover_session(&recovery, "9:31 AM".into(), window, cx)
    });
    cx.run_until_parked();

    let strip = bounds(cx, "recovery-strip");
    let limit = SIZES[0].2.apply(recovery_strip::STRIP_HEIGHT) + 0.5;
    assert!(
        f32::from(strip.size.height) <= limit,
        "{:?} exceeds {limit}",
        strip.size.height
    );
    assert!(
        present(cx, "recovery-prior"),
        "the prior block is still there"
    );
    assert!(
        f32::from(bounds(cx, "recovery-detail").size.height) < limit,
        "the detail is one truncated line"
    );
}
