//! T0.5 Display Control acceptance: congregation-output lifecycle through the
//! real runtime composition, driven by the in-memory display Adapter and the
//! topology fixtures in `tests/fixtures/display-topologies/`.
//!
//! The pinned GPUI test platform has exactly one display and ignores
//! `display_id`, so placement is asserted through Display Control and the
//! window bounds the passive options request.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use gpui::{
    App, Bounds, Modifiers, MouseExitEvent, Pixels, TestAppContext, VisualTestContext, Window,
    WindowBounds, WindowKind, point, px, size,
};
use ordo::{
    NativeRuntime, open_native_runtime_with,
    output::{DISPLAY_WATCH_INTERVAL, DisplaySource, OutputError, locate_cursor},
    shell::OpenSongs,
};
use platform::{
    congregation_window::congregation_window_options,
    display::{
        DetectedDisplay, DisplayAdapter, DisplayBounds, DisplayControl, DisplayEvent,
        DisplayEvidence, DisplayId, DisplayStatus, EnableError, InMemoryDisplayAdapter,
        OutputLifecycle, OutputPlacement, PlacementBounds, RelativeDirection, Topology,
    },
};
use presentation::{CommandRejected, OutputStatus, ProgramFrame};

/// Parser for the documented line format in
/// `tests/fixtures/display-topologies/*.txt`.
mod fixtures {
    use super::*;

    pub struct Fixture {
        pub name: String,
        pub displays: Vec<DetectedDisplay>,
        pub operator: Option<DisplayId>,
        pub congregation: Option<DisplayId>,
        pub expect: Option<RelativeDirection>,
    }

    impl Fixture {
        pub fn adapter(&self) -> InMemoryDisplayAdapter {
            InMemoryDisplayAdapter::new(self.displays.clone(), self.operator)
        }
    }

    fn directory() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/display-topologies")
    }

    /// Every fixture, sorted by name.
    pub fn all() -> Vec<Fixture> {
        let mut paths: Vec<_> = fs::read_dir(directory())
            .expect("display-topology fixtures must exist")
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|path| {
                let name = path.file_stem().unwrap().to_string_lossy().into_owned();
                let text = fs::read_to_string(path).unwrap();
                parse(&name, &text).unwrap_or_else(|error| panic!("{name}: {error}"))
            })
            .collect()
    }

    pub fn parse(name: &str, text: &str) -> Result<Fixture, String> {
        let mut fixture = Fixture {
            name: name.to_owned(),
            displays: Vec::new(),
            operator: None,
            congregation: None,
            expect: None,
        };
        let mut saw_expect = false;
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = line.split_whitespace().collect();
            let at = |message: &str| format!("line {}: {message}", number + 1);
            match fields.as_slice() {
                ["expect", direction] => {
                    fixture.expect = direction_named(direction).map_err(|error| at(&error))?;
                    saw_expect = true;
                }
                [role, id, x, y, width, height] => {
                    let bad = |value: &str| at(&format!("bad number {value}"));
                    let id: u64 = id.parse().map_err(|_| bad(id))?;
                    let x: i32 = x.parse().map_err(|_| bad(x))?;
                    let y: i32 = y.parse().map_err(|_| bad(y))?;
                    let width: u32 = width.parse().map_err(|_| bad(width))?;
                    let height: u32 = height.parse().map_err(|_| bad(height))?;
                    let id = DisplayId::new(id);
                    let bounds = DisplayBounds::new(x, y, width, height);
                    let display = DetectedDisplay {
                        id,
                        evidence: DisplayEvidence {
                            name: None,
                            uuid: None,
                            bounds,
                        },
                        placement: PlacementBounds::from(bounds),
                    };
                    match *role {
                        "operator" => fixture.operator = Some(id),
                        "congregation" => fixture.congregation = Some(id),
                        "display" => {}
                        other => return Err(at(&format!("unknown role {other}"))),
                    }
                    fixture.displays.push(display);
                }
                _ => return Err(at("expected a display record or `expect`")),
            }
        }
        if !saw_expect {
            return Err("missing `expect` record".to_owned());
        }
        Ok(fixture)
    }

    fn direction_named(name: &str) -> Result<Option<RelativeDirection>, String> {
        use RelativeDirection::*;
        Ok(Some(match name {
            "none" => return Ok(None),
            "Left" => Left,
            "Right" => Right,
            "Above" => Above,
            "Below" => Below,
            "AboveLeft" => AboveLeft,
            "AboveRight" => AboveRight,
            "BelowLeft" => BelowLeft,
            "BelowRight" => BelowRight,
            "Overlapping" => Overlapping,
            other => return Err(format!("unknown direction {other}")),
        }))
    }

    #[test]
    fn parser_rejects_malformed_records() {
        assert!(parse("x", "operator 1 0 0 1920\nexpect none").is_err());
        assert!(parse("x", "projector 1 0 0 1920 1080\nexpect none").is_err());
        assert!(parse("x", "operator 1 0 0 1920 1080").is_err());
        assert!(parse("x", "operator 1 0 0 1920 1080\nexpect Sideways").is_err());
        // Negative or fractional values are rejected, not wrapped by a cast.
        assert!(parse("x", "operator -1 0 0 1920 1080\nexpect none").is_err());
        assert!(parse("x", "operator 1 0 0 1920 -1080\nexpect none").is_err());
        assert!(parse("x", "operator 1 0 0 1920.5 1080\nexpect none").is_err());
        let fixture = parse(
            "x",
            "# c\r\n\r\noperator 1 0 0 1920 1080\r\nexpect none\r\n",
        )
        .unwrap();
        assert_eq!(fixture.operator, Some(DisplayId::new(1)));
        assert_eq!(fixture.expect, None);
    }
}

const OPERATOR: DisplayId = DisplayId::new(1);
const PROJECTOR: DisplayId = DisplayId::new(2);
const SPARE: DisplayId = DisplayId::new(3);

fn temporary_preferences(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ordo-display-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// A detected display whose placement is its exact integer evidence bounds.
fn detected(id: DisplayId, evidence: DisplayEvidence) -> DetectedDisplay {
    let placement = PlacementBounds::from(evidence.bounds);
    DetectedDisplay {
        id,
        evidence,
        placement,
    }
}

fn operator_display() -> DetectedDisplay {
    detected(
        OPERATOR,
        DisplayEvidence {
            name: Some("Laptop".to_owned()),
            uuid: Some("port-0".to_owned()),
            bounds: DisplayBounds::new(0, 0, 1920, 1080),
        },
    )
}

fn projector(id: DisplayId) -> DetectedDisplay {
    detected(
        id,
        DisplayEvidence {
            name: Some("Projector".to_owned()),
            uuid: Some("port-1".to_owned()),
            bounds: DisplayBounds::new(1920, 0, 1280, 720),
        },
    )
}

fn spare() -> DetectedDisplay {
    detected(
        SPARE,
        DisplayEvidence {
            name: Some("Television".to_owned()),
            uuid: Some("port-2".to_owned()),
            bounds: DisplayBounds::new(-1280, 0, 1280, 720),
        },
    )
}

/// The in-memory topology behind a platform whose placement can be faulted.
/// While `misplace` is set, the platform reports every window on the operator
/// display, which is what GPUI does for a display id it does not know.
/// `seen_operator` records the operator display each snapshot was given.
struct FaultyPlatformSource {
    adapter: Rc<RefCell<InMemoryDisplayAdapter>>,
    misplace: Rc<Cell<bool>>,
    seen_operator: Rc<Cell<Option<gpui::DisplayId>>>,
}

impl DisplaySource for FaultyPlatformSource {
    fn snapshot(&self, _: &App, operator: Option<gpui::DisplayId>) -> Topology {
        self.seen_operator.set(operator);
        self.adapter.borrow().snapshot()
    }

    fn reported_display(&self, _: &Window, requested: DisplayId, _: &App) -> Option<DisplayId> {
        Some(if self.misplace.get() {
            OPERATOR
        } else {
            requested
        })
    }
}

struct Harness {
    runtime: NativeRuntime,
    displays: Rc<RefCell<InMemoryDisplayAdapter>>,
    misplace: Rc<Cell<bool>>,
    seen_operator: Rc<Cell<Option<gpui::DisplayId>>>,
    preferences: PathBuf,
}

impl Harness {
    fn open(cx: &mut TestAppContext, name: &str, adapter: InMemoryDisplayAdapter) -> Self {
        // The runtime's preferences worker is a real OS thread whose restore wakes a
        // GPUI task; without this the test scheduler reports that cross-thread
        // wakeup as non-determinism whenever the worker finishes after the task
        // registered its waker, as on a loaded CI runner.
        cx.executor().allow_parking();
        let preferences = temporary_preferences(name);
        let displays = Rc::new(RefCell::new(adapter));
        let misplace = Rc::new(Cell::new(false));
        let seen_operator = Rc::new(Cell::new(None));
        let source = FaultyPlatformSource {
            adapter: displays.clone(),
            misplace: misplace.clone(),
            seen_operator: seen_operator.clone(),
        };
        let runtime = cx.update(|cx| {
            open_native_runtime_with(cx, &preferences, source).expect("runtime should open")
        });
        cx.run_until_parked();
        Self {
            runtime,
            displays,
            misplace,
            seen_operator,
            preferences,
        }
    }

    /// Makes the platform place windows on the operator display.
    fn misplace_windows(&self, on: bool) {
        self.misplace.set(on);
    }

    fn enabled_display(&self, cx: &mut TestAppContext) -> Option<DisplayId> {
        let output = self.runtime.output();
        match cx.read(|cx| output.read(cx).display_control().output().clone()) {
            OutputLifecycle::Enabled { display, .. } => Some(display),
            _ => None,
        }
    }

    fn two_displays(cx: &mut TestAppContext, name: &str) -> Self {
        Self::open(
            cx,
            name,
            InMemoryDisplayAdapter::new(
                vec![operator_display(), projector(PROJECTOR)],
                Some(OPERATOR),
            ),
        )
    }

    fn output<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(
            &mut ordo::output::CongregationOutput,
            &mut gpui::Context<ordo::output::CongregationOutput>,
        ) -> R,
    ) -> R {
        let output = self.runtime.output();
        let result = cx.update(|cx| output.update(cx, f));
        cx.run_until_parked();
        result
    }

    fn refresh(&self, cx: &mut TestAppContext) -> Vec<DisplayEvent> {
        self.output(cx, |output, cx| output.refresh_displays(cx))
    }

    fn enable(&self, cx: &mut TestAppContext) -> gpui::WindowHandle<ordo::CongregationView> {
        self.output(cx, |output, cx| output.enable_output(cx))
            .expect("output should enable")
    }

    fn window_count(&self, cx: &mut TestAppContext) -> usize {
        cx.read(|cx| cx.windows().len())
    }

    fn status(&self, cx: &mut TestAppContext) -> (OutputStatus, bool) {
        let output = self.runtime.output();
        cx.read(|cx| {
            let state = output.read(cx).congregation();
            (state.output_status(), state.program().is_black())
        })
    }

    fn display_status(&self, cx: &mut TestAppContext) -> DisplayStatus {
        let output = self.runtime.output();
        cx.read(|cx| output.read(cx).display_control().status())
    }

    fn congregation_window(
        &self,
        cx: &mut TestAppContext,
    ) -> Option<gpui::WindowHandle<ordo::CongregationView>> {
        cx.read(|cx| self.runtime.congregation(cx))
    }

    fn close(self, cx: &mut TestAppContext) {
        self.runtime
            .operator()
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| cx.windows().len()), 0);
        fs::remove_file(&self.preferences).ok();
    }
}

/// A real canonical frame through the one rasterization path.
fn canonical_frame() -> ProgramFrame {
    use rendering::{TargetDensity, TemplateContent, TemplateKind, Theme};

    let content = TemplateContent {
        kind: TemplateKind::Lyrics,
        header: Some("Entrance".to_owned()),
        body: "Maayong buntag, mga igsoon!".to_owned(),
        footer: None,
        image: None,
    };
    let density = TargetDensity::SCALE_100;
    let (frame, _) = rendering::render_template(&content, &Theme::default(), density).unwrap();
    let raster = rendering::rasterize(&frame, density, &HashMap::new()).unwrap();
    let program = ProgramFrame::from_raster(1, raster).unwrap();
    assert!(!program.is_black(), "fixture frame must carry content");
    program
}

/// `debug_bounds` takes a `&'static str`; leak the few selectors built here.
fn selector(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

#[gpui::test]
fn app_starts_without_a_congregation_window(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "startup");

    assert_eq!(
        harness.window_count(cx),
        1,
        "only the operator window opens"
    );
    assert!(harness.congregation_window(cx).is_none());
    assert_eq!(harness.status(cx), (OutputStatus::Disabled, true));
    let output = harness.runtime.output();
    assert_eq!(
        cx.read(|cx| output.read(cx).display_control().output().clone()),
        OutputLifecycle::Disabled
    );
    harness.close(cx);
}

#[gpui::test]
fn explicit_enable_opens_one_passive_black_window_on_the_assigned_display(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "enable");
    assert_eq!(
        harness
            .output(cx, |output, cx| output.enable_output(cx))
            .err(),
        Some(OutputError::Display(EnableError::NotAssigned)),
        "enabling needs an assigned display"
    );
    assert_eq!(harness.window_count(cx), 1);

    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    assert_eq!(harness.window_count(cx), 1, "assignment never opens output");
    let window = harness.enable(cx);

    assert_eq!(harness.window_count(cx), 2);
    assert_eq!(harness.congregation_window(cx), Some(window));
    assert_eq!(harness.enabled_display(cx), Some(PROJECTOR));

    // The window requests the passive policy at exactly the assigned bounds.
    let placement = OutputPlacement {
        display: PROJECTOR,
        bounds: projector(PROJECTOR).placement,
    };
    let options = congregation_window_options(&placement);
    assert_eq!(options.kind, WindowKind::PopUp);
    assert!(!options.focus);
    assert!(options.titlebar.is_none());
    assert!(!options.is_movable && !options.is_resizable && !options.is_minimizable);
    assert_eq!(
        options.display_id,
        Some(gpui::DisplayId::new(PROJECTOR.raw()))
    );
    let assigned = Bounds::<Pixels>::from(placement.bounds);
    assert_eq!(
        options.window_bounds,
        Some(WindowBounds::Windowed(assigned))
    );
    assert_eq!(
        window.update(cx, |_, window, _| window.bounds()).unwrap(),
        assigned
    );

    // Black first: Program is black and no image is painted.
    window
        .update(cx, |view, _, _| {
            assert!(view.state().program().is_black());
            assert_eq!(view.state().output_status(), OutputStatus::Connected);
            assert!(view.image().is_none());
            assert_eq!(view.painted_revision(), None);
        })
        .unwrap();
    let mut congregation = VisualTestContext::from_window(window.into(), cx);
    assert!(
        congregation
            .debug_bounds("congregation-frame-pixels")
            .is_some()
    );
    assert!(
        congregation
            .debug_bounds("output-status-Connected")
            .is_some()
    );

    // Passive: holds no focus. Activation on click is covered separately.
    assert!(
        window
            .update(cx, |_, window, cx| window.focused(cx).is_none())
            .unwrap()
    );

    // A second enable while open opens nothing.
    assert_eq!(
        harness
            .output(cx, |output, cx| output.enable_output(cx))
            .err(),
        Some(OutputError::Display(EnableError::AlreadyEnabled))
    );
    assert_eq!(harness.window_count(cx), 2);
    harness.close(cx);
}

#[gpui::test]
fn congregation_window_exposes_no_operator_content_or_actions(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "passive");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    let window = harness.enable(cx);
    let operator = harness.runtime.operator();
    let destination = operator
        .update(cx, |shell, _, _| shell.destination())
        .unwrap();

    let mut congregation = VisualTestContext::from_window(window.into(), cx);
    assert!(!congregation.update(|_, cx| cx.is_action_available(&OpenSongs)));
    congregation.dispatch_action(OpenSongs);
    congregation.run_until_parked();
    assert_eq!(
        operator
            .update(cx, |shell, _, _| shell.destination())
            .unwrap(),
        destination,
        "operator actions sent to congregation output do nothing"
    );
    assert!(congregation.update(|window, cx| window.focused(cx).is_none()));

    let mut operator_cx = VisualTestContext::from_window(operator.into(), cx);
    for operator_only in ["build-identity", "connection-status", "workspace-title"] {
        assert!(
            operator_cx.debug_bounds(operator_only).is_some(),
            "{operator_only} is on the operator window"
        );
        assert!(
            congregation.debug_bounds(operator_only).is_none(),
            "{operator_only} never reaches congregation output"
        );
    }
    harness.close(cx);
}

#[gpui::test]
fn pointer_over_output_shows_an_operator_only_locator(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "locator");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    let window = harness.enable(cx);
    let operator = harness.runtime.operator();
    let shown = selector(format!("cursor-locator-{:?}", RelativeDirection::Left));

    let mut congregation = VisualTestContext::from_window(window.into(), cx);
    congregation.simulate_mouse_move(point(px(200.), px(200.)), None, Modifiers::none());
    congregation.run_until_parked();

    let locator = operator
        .update(cx, |shell, _, _| shell.cursor_locator())
        .unwrap();
    // The projector is right of the operator, so the way back is left.
    assert_eq!(
        locator.map(|locator| locator.direction),
        Some(RelativeDirection::Left)
    );
    let mut operator_cx = VisualTestContext::from_window(operator.into(), cx);
    operator_cx.run_until_parked();
    assert!(operator_cx.debug_bounds(shown).is_some());
    assert!(
        congregation.debug_bounds(shown).is_none(),
        "never on Program"
    );

    congregation.simulate_event(MouseExitEvent {
        position: point(px(-1.), px(200.)),
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    congregation.run_until_parked();
    operator_cx.run_until_parked();
    assert_eq!(
        operator
            .update(cx, |shell, _, _| shell.cursor_locator())
            .unwrap(),
        None
    );
    assert!(
        operator_cx.debug_bounds(shown).is_none(),
        "locator leaves with the pointer"
    );
    harness.close(cx);
}

#[gpui::test]
fn topology_fixtures_drive_locator_direction(cx: &mut TestAppContext) {
    let fixtures = fixtures::all();
    let names: Vec<_> = fixtures
        .iter()
        .map(|fixture| fixture.name.as_str())
        .collect();
    for required in [
        "operator-left",
        "operator-right",
        "congregation-above",
        "congregation-below",
        "diagonal",
        "single-display",
    ] {
        assert!(names.contains(&required), "missing fixture {required}");
    }

    let harness = Harness::open(cx, "fixtures", InMemoryDisplayAdapter::default());
    let operator = harness.runtime.operator();
    for fixture in &fixtures {
        let topology = fixture.adapter().snapshot();
        let Some(congregation) = fixture.congregation else {
            // A single display: nothing to locate and nothing to enable.
            assert_eq!(fixture.expect, None, "{}", fixture.name);
            let control = DisplayControl::new(topology);
            assert_eq!(control.suggestion(), None, "{}", fixture.name);
            continue;
        };
        let guidance = locate_cursor(&topology, congregation).map(|locator| locator.direction);
        assert_eq!(guidance, fixture.expect, "{}", fixture.name);

        // Through the composed runtime: hover over output, read the shell.
        harness.output(cx, |output, cx| output.disable_output(cx));
        *harness.displays.borrow_mut() = fixture.adapter();
        harness.refresh(cx);
        harness
            .output(cx, |output, cx| output.assign(congregation, cx))
            .unwrap();
        harness.enable(cx);
        harness.output(cx, |output, cx| output.set_pointer_over_output(true, cx));
        let shown = operator
            .update(cx, |shell, _, _| {
                shell.cursor_locator().map(|locator| locator.direction)
            })
            .unwrap();
        assert_eq!(shown, fixture.expect, "{}", fixture.name);
        let mut operator_cx = VisualTestContext::from_window(operator.into(), cx);
        operator_cx.run_until_parked();
        let selector = selector(format!("cursor-locator-{:?}", fixture.expect.unwrap()));
        assert!(
            operator_cx.debug_bounds(selector).is_some(),
            "{}",
            fixture.name
        );
    }

    // Single display: Presenter stays usable with output unavailable.
    harness.output(cx, |output, cx| output.disable_output(cx));
    let single = fixtures
        .iter()
        .find(|fixture| fixture.name == "single-display")
        .unwrap();
    *harness.displays.borrow_mut() = single.adapter();
    harness.refresh(cx);
    assert_eq!(
        harness.output(cx, |output, cx| output.assign(OPERATOR, cx)),
        Err(platform::display::AssignError::OperatorDisplay(OPERATOR))
    );
    assert_eq!(harness.window_count(cx), 1);
    harness.close(cx);
}

#[gpui::test]
fn disappearance_disconnects_closes_and_never_retargets(cx: &mut TestAppContext) {
    let harness = Harness::open(
        cx,
        "disconnect",
        InMemoryDisplayAdapter::new(
            vec![operator_display(), projector(PROJECTOR), spare()],
            Some(OPERATOR),
        ),
    );
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    harness.enable(cx);
    harness
        .output(cx, |output, cx| output.take_live(canonical_frame(), cx))
        .unwrap();
    assert_eq!(harness.status(cx), (OutputStatus::Connected, false));

    harness.displays.borrow_mut().disconnect(PROJECTOR);
    let events = harness.refresh(cx);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, DisplayEvent::OutputDisconnected { .. }))
    );
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));
    assert_eq!(
        harness.display_status(cx),
        DisplayStatus::Disconnected { recognized: None }
    );
    assert!(harness.congregation_window(cx).is_none(), "window closed");
    assert_eq!(harness.window_count(cx), 1, "never reopened elsewhere");

    // A spare candidate is present, yet nothing retargets to it.
    assert_eq!(
        harness
            .output(cx, |output, cx| output.enable_output(cx))
            .err(),
        Some(OutputError::Display(EnableError::AssignedDisplayMissing))
    );
    assert_eq!(harness.window_count(cx), 1);
    assert_eq!(
        harness.output(cx, |output, cx| output.take_preview_live(cx)),
        Err(CommandRejected::OutputNotConnected(
            OutputStatus::Disconnected
        ))
    );

    // Reconnection under a new OS id is recognized but exposes nothing.
    let returned = DisplayId::new(20);
    harness.displays.borrow_mut().connect(projector(returned));
    let events = harness.refresh(cx);
    assert!(events.contains(&DisplayEvent::AssignedDisplayRecognized(returned)));
    assert_eq!(
        harness.display_status(cx),
        DisplayStatus::Disconnected {
            recognized: Some(returned)
        }
    );
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));
    assert!(harness.congregation_window(cx).is_none());
    assert_eq!(
        harness.output(cx, |output, cx| output.take_preview_live(cx)),
        Err(CommandRejected::OutputNotConnected(
            OutputStatus::Disconnected
        ))
    );
    harness.close(cx);
}

/// Milestone E2E: fake two-display Adapter → enable → black-first Program →
/// fixture frame live → disconnect → protective Disconnected → reconnect
/// under a new OS id with matching evidence → black until explicit restore.
#[gpui::test]
fn e2e_disconnect_and_reconnect_stay_black_until_restored(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "e2e");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();

    // Enable: black-first Program.
    let window = harness.enable(cx);
    assert_eq!(harness.status(cx), (OutputStatus::Connected, true));
    window
        .update(cx, |view, _, _| {
            assert!(view.state().program().is_black());
            assert!(view.image().is_none());
        })
        .unwrap();

    // Take a real canonical fixture frame live.
    let frame = canonical_frame();
    let expected = frame.rgba().to_vec();
    harness
        .output(cx, |output, cx| output.take_live(frame, cx))
        .unwrap();
    assert_eq!(harness.status(cx), (OutputStatus::Connected, false));
    window
        .update(cx, |view, _, _| {
            let program = view.state().program();
            assert_eq!(program.rgba(), expected.as_slice());
            assert!(view.image().is_some(), "complete image painted");
            assert_eq!(view.painted_revision(), Some(program.revision()));
        })
        .unwrap();

    // Disconnect the assigned display: protective Disconnected.
    let unplugged = harness.displays.borrow_mut().disconnect(PROJECTOR).unwrap();
    harness.refresh(cx);
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));
    assert!(harness.congregation_window(cx).is_none());
    assert_eq!(harness.window_count(cx), 1);

    // Reconnect under a new OS id with matching evidence: still black and
    // no window until the operator explicitly enables.
    let returned = DisplayId::new(77);
    harness.displays.borrow_mut().connect(DetectedDisplay {
        id: returned,
        ..unplugged
    });
    harness.refresh(cx);
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));
    assert!(harness.congregation_window(cx).is_none());

    // Explicit enable: black on the recognized display.
    let window = harness.enable(cx);
    assert_eq!(harness.enabled_display(cx), Some(returned));
    assert_eq!(harness.status(cx), (OutputStatus::Connected, true));
    window
        .update(cx, |view, _, _| assert!(view.image().is_none()))
        .unwrap();

    // The documented restore path (`Space`): the Preview returns.
    harness
        .output(cx, |output, cx| output.take_preview_live(cx))
        .unwrap();
    assert_eq!(harness.status(cx), (OutputStatus::Connected, false));
    window
        .update(cx, |view, _, _| {
            assert_eq!(view.state().program().rgba(), expected.as_slice());
            assert_eq!(
                view.painted_revision(),
                Some(view.state().program().revision())
            );
        })
        .unwrap();
    harness.close(cx);
}

/// A display unplugged without a refresh must not be used: the enable reads a
/// fresh topology first, refuses, and opens nothing.
#[gpui::test]
fn enable_reads_the_topology_before_placing_output(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "stale");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    harness.enable(cx);
    harness.output(cx, |output, cx| output.disable_output(cx));
    assert_eq!(harness.window_count(cx), 1);

    // Unplugged, and the controller is not told.
    harness.displays.borrow_mut().disconnect(PROJECTOR);
    assert_eq!(
        harness
            .output(cx, |output, cx| output.enable_output(cx))
            .err(),
        Some(OutputError::Display(EnableError::AssignedDisplayMissing)),
        "a stale topology must not place output on a display that is gone"
    );
    assert_eq!(
        harness.window_count(cx),
        1,
        "only the operator window exists"
    );
    assert!(harness.congregation_window(cx).is_none());
    harness.close(cx);
}

/// GPUI falls back to the primary display for an unknown id. The controller
/// closes the misplaced window and returns to the prior lifecycle.
#[gpui::test]
fn placement_mismatch_closes_the_window_and_restores_the_prior_lifecycle(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "mismatch");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();

    harness.misplace_windows(true);
    assert_eq!(
        harness
            .output(cx, |output, cx| output.enable_output(cx))
            .err(),
        Some(OutputError::PlacementMismatch {
            requested: PROJECTOR,
            reported: Some(OPERATOR),
        })
    );
    assert_eq!(
        harness.window_count(cx),
        1,
        "the misplaced window is closed"
    );
    assert!(harness.congregation_window(cx).is_none());
    assert_eq!(harness.display_status(cx), DisplayStatus::Ready(PROJECTOR));
    assert_eq!(harness.status(cx), (OutputStatus::Disabled, true));

    harness.misplace_windows(false);
    harness.enable(cx);
    assert_eq!(harness.enabled_display(cx), Some(PROJECTOR));
    harness.close(cx);
}

/// A re-enable that fails after a reconnect leaves output Disconnected, black,
/// and windowless, rather than forcing it to Disabled.
#[gpui::test]
fn failed_reenable_after_reconnect_stays_disconnected(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "reenable");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    harness.enable(cx);

    let unplugged = harness.displays.borrow_mut().disconnect(PROJECTOR).unwrap();
    harness.refresh(cx);
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));

    let returned = DisplayId::new(77);
    harness.displays.borrow_mut().connect(DetectedDisplay {
        id: returned,
        ..unplugged
    });
    harness.refresh(cx);
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));

    harness.misplace_windows(true);
    assert_eq!(
        harness
            .output(cx, |output, cx| output.enable_output(cx))
            .err(),
        Some(OutputError::PlacementMismatch {
            requested: returned,
            reported: Some(OPERATOR),
        })
    );
    assert_eq!(
        harness.status(cx),
        (OutputStatus::Disconnected, true),
        "a failed re-enable does not reopen output"
    );
    assert_eq!(
        harness.display_status(cx),
        DisplayStatus::Disconnected {
            recognized: Some(returned)
        }
    );
    assert_eq!(harness.window_count(cx), 1);
    harness.close(cx);
}

/// The Windows backend activates a passive window on click, which takes
/// keyboard focus from the operator. A press on Program hands it back.
#[gpui::test]
fn click_on_congregation_output_returns_focus_to_the_operator(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "focus");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    let window = harness.enable(cx);
    let operator = harness.runtime.operator();

    operator
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.active_window()), Some(operator.into()));

    // Activation on click is what the Windows backend does (`MA_ACTIVATE`).
    // The test platform does not activate on mouse down, so it is simulated.
    window
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.active_window()), Some(window.into()));

    let mut congregation = VisualTestContext::from_window(window.into(), cx);
    congregation.simulate_mouse_move(point(px(200.), px(200.)), None, Modifiers::none());
    congregation.simulate_click(point(px(200.), px(200.)), Modifiers::none());
    congregation.run_until_parked();
    assert_eq!(
        cx.read(|cx| cx.active_window()),
        Some(operator.into()),
        "a click on congregation output returns focus to the operator"
    );
    harness.close(cx);
}

/// Windows relocates a window whose display is removed and reports the move
/// as a bounds change. The controller refreshes on that change without a
/// caller.
#[gpui::test]
fn moving_the_congregation_window_refreshes_topology(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "bounds");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    let window = harness.enable(cx);

    harness.displays.borrow_mut().disconnect(PROJECTOR);
    cx.simulate_window_resize(window.into(), size(px(1280.), px(720.)));
    cx.run_until_parked();

    assert!(harness.congregation_window(cx).is_none(), "window closed");
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));
    assert_eq!(harness.window_count(cx), 1);
    harness.close(cx);
}

/// A lost display does not always move the window, so enabled output notices
/// the loss on its own, with no caller and no bounds change.
#[gpui::test]
fn enabled_output_fails_to_black_without_a_refresh(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "watch-loss");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    harness.enable(cx);
    harness
        .output(cx, |output, cx| output.take_live(canonical_frame(), cx))
        .unwrap();
    assert_eq!(harness.status(cx), (OutputStatus::Connected, false));

    harness.displays.borrow_mut().disconnect(PROJECTOR);
    cx.executor().advance_clock(DISPLAY_WATCH_INTERVAL);
    cx.run_until_parked();

    assert!(harness.congregation_window(cx).is_none(), "window closed");
    assert_eq!(harness.status(cx), (OutputStatus::Disconnected, true));
    assert_eq!(harness.window_count(cx), 1, "nothing retargeted");
    harness.close(cx);
}

/// The watch must not redraw a static Program: an unchanged topology wakes no
/// observer, however many intervals pass.
#[gpui::test]
fn an_unchanged_topology_wakes_no_observer(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "watch-static");
    harness
        .output(cx, |output, cx| output.assign(PROJECTOR, cx))
        .unwrap();
    harness.enable(cx);

    let wakes = Rc::new(Cell::new(0));
    let output = harness.runtime.output();
    let _observer = cx.update(|cx| {
        let wakes = wakes.clone();
        cx.observe(&output, move |_, _| wakes.set(wakes.get() + 1))
    });
    cx.executor().advance_clock(DISPLAY_WATCH_INTERVAL * 4);
    cx.run_until_parked();

    assert_eq!(wakes.get(), 0);
    harness.close(cx);
}

/// A refresh that runs inside the operator window's own update cannot read
/// that window's display. It must reuse the last value read, not drop it.
#[gpui::test]
fn refresh_inside_the_operator_window_keeps_the_operator_display(cx: &mut TestAppContext) {
    let harness = Harness::two_displays(cx, "operator-cache");
    let output = harness.runtime.output();
    let operator = harness.runtime.operator();
    // The operator window's GPUI display, converted through the platform Adapter.
    let gpui_operator: gpui::DisplayId = OPERATOR.into();

    harness.output(cx, |output, cx| output.refresh_displays(cx));
    assert_eq!(harness.seen_operator.get(), Some(gpui_operator));

    operator
        .update(cx, |_, _, cx| {
            output.update(cx, |output, cx| {
                output.refresh_displays(cx);
            });
        })
        .unwrap();
    assert_eq!(
        harness.seen_operator.get(),
        Some(gpui_operator),
        "the operator display is the cached read while the window is busy"
    );
    harness.close(cx);
}
