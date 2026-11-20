//! Development-only Live Control lab (T1.4 WP5).
//!
//! ```text
//! cargo run -p ordo --example live-control-lab -- [--size 1366x768] [--scale 200] [snapshot-dir]
//! ```
//!
//! Opens the Presenter view over a real Presentation Engine and a verified
//! snapshot (default: `tests/fixtures/presentations/normal-mass`), so the
//! header, rail placeholder, timeline placeholder and Output Dock can be
//! judged at any window size and Interface scale. It is not a workflow:
//! Presenter Mode entry is T1.6, and the product key bindings are T1.5.
//!
//! Lab-only keys (focus the window first): Left/Right previous/next Cue,
//! Space take Preview live, F freeze, B blackout, E enable output, D simulate
//! display lost, R simulate the recognized display returning.
//!
//! Safety and ownership:
//!
//! * The snapshot is read on the background executor; the window says
//!   "Loading..." until it is ready, or shows the load error.
//! * The bundled faces are verified when the window opens, and the lab fails
//!   closed with a visible message if they do not match.
//! * `--measure-memory <out.json> [--snapshot <dir>]` is the T1.3d native
//!   memory measurement `cargo xtask measure-presenter` runs, one fresh process
//!   per run. It opens the real `PresenterView` at 1920x1080, 100 %, Grid,
//!   rail pinned, over a synthetic 500-Cue snapshot it writes under the temp
//!   directory (or `--snapshot <dir>`), drives forward, reverse and seeded
//!   random navigation with output, Take, Freeze, Blackout and rail jumps
//!   interleaved, samples process memory after each phase, writes one JSON
//!   document and quits. It reads and writes no preferences; see `measure`.
//! * `--display-inventory <out.json>`, `--measure-presenter <out.json>
//!   [--commands N]` and `--measure-output <out.json>` are the church-kit
//!   measurements (see `kit`): displays and monitor names; keydown to paint over
//!   a seeded key stream with no output window; and keydown to paint on a real
//!   congregation display. Each takes `--snapshot <dir>`; without it they try
//!   `<exe dir>/fixtures/normal-mass`, then the compile-time fixture path.
//! * `--run-drill <out.json> [--snapshot <dir>]` replays the committed Recovery
//!   Drill (`commands.txt`) through the real `PresenterView` at 1920x1080, 100 %,
//!   on a stepped clock, compares every state with `commands.expect`, and reports
//!   keydown-to-paint latency and peak memory; `cargo xtask measure-presenter`
//!   judges it (see `kit`).
//! * Preferences (the Grid/List choice) are read from and written under
//!   `target/live-control-lab/`, never `%APPDATA%`. Saves go to one worker
//!   thread, owned by `main`, which drops the sender when the window closes
//!   and joins the thread after the app loop ends.

use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext as _, AsyncWindowContext, Bounds, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    Styled, Task, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, px, rgb,
    size,
};
use gpui_platform::application;
use ordo::{
    performance::{display_path::verify_bundled_faces, memory::MemoryWatch},
    presenter::{
        PresenterView, SavePreferences,
        images::{check_images, image_count},
        layout::RailChoice,
    },
};
use platform::preferences::PreferencesFile;
use presentation::{
    commands::Command,
    snapshot_reader::{DirectorySource, LoadedSnapshot, SnapshotLoadError, read_snapshot},
    state::Millis,
};
use ui::{
    environment::{
        CueView, EnvironmentChange, InterfaceScale, ResolvedAppearance, UiEnvironment,
        UiPreferences,
    },
    tokens,
};

actions!(
    live_control_lab,
    [EnableOutput, SimulateDisplayLost, SimulateDisplayReturned]
);

/// The window-title key help: every bound registry key, then the lab's own
/// development keys.
fn key_help() -> String {
    use ordo::commands::registry::{ENTRIES, Scope};
    let mut help: Vec<String> = ENTRIES
        .iter()
        .filter(|entry| !matches!(entry.scope, Scope::Surface | Scope::Editing))
        .map(|entry| format!("{} {}", entry.keycaps, entry.label.to_lowercase()))
        .collect();
    help.extend(["E enable output", "D display lost", "R display returned"].map(String::from));
    help.join(" · ")
}

/// What the command line asked for.
#[derive(Debug, PartialEq)]
struct Args {
    width: f32,
    height: f32,
    scale: InterfaceScale,
    snapshot: PathBuf,
    /// Whether `snapshot` was named, so the measurement need not generate one.
    snapshot_given: bool,
    /// `--measure-memory <out.json>`: run the memory measurement and quit.
    measure_memory: Option<PathBuf>,
    /// `--measure-animations <out.json>`: measure animation frame pacing and quit.
    measure_animations: Option<PathBuf>,
    /// `--display-inventory <out.json>`: list the displays and quit.
    display_inventory: Option<PathBuf>,
    /// `--measure-presenter <out.json>`: keydown-to-paint latency, no output window.
    measure_presenter: Option<PathBuf>,
    /// `--measure-output <out.json>`: the same on a real congregation display.
    measure_output: Option<PathBuf>,
    /// `--run-drill <out.json>`: replay the Recovery Drill and quit.
    run_drill: Option<PathBuf>,
    /// `--commands N` of `--measure-presenter`.
    commands: usize,
    /// Whether `--commands` was named.
    commands_given: bool,
    /// Whether `--size` was named; the church-kit modes open maximized without it.
    size_given: bool,
    /// `--cues 30|500`: the fixture the animation measurement runs over.
    cues: usize,
    /// `--list` / `--grid`: the Cue view the animation measurement runs in.
    list_view: bool,
}

fn default_snapshot() -> PathBuf {
    ordo::presenter::fixture_snapshot_dir()
}

fn parse_scale(text: &str) -> Option<InterfaceScale> {
    match text {
        "100" => Some(InterfaceScale::Percent100),
        "125" => Some(InterfaceScale::Percent125),
        "150" => Some(InterfaceScale::Percent150),
        "175" => Some(InterfaceScale::Percent175),
        "200" => Some(InterfaceScale::Percent200),
        _ => None,
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        width: 1366.0,
        height: 768.0,
        scale: InterfaceScale::Percent100,
        snapshot: default_snapshot(),
        snapshot_given: false,
        measure_memory: None,
        measure_animations: None,
        display_inventory: None,
        measure_presenter: None,
        measure_output: None,
        run_drill: None,
        commands: kit::DEFAULT_COMMANDS,
        commands_given: false,
        size_given: false,
        cues: 500,
        list_view: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => {
                let value = args.next().ok_or("--size needs a value such as 1366x768")?;
                let (width, height) = value.split_once('x').ok_or("--size looks like 1366x768")?;
                parsed.width = width.parse().map_err(|_| "--size width is not a number")?;
                parsed.height = height
                    .parse()
                    .map_err(|_| "--size height is not a number")?;
                if parsed.width < 320.0 || parsed.height < 240.0 {
                    return Err("--size is too small to show the Presenter".into());
                }
                parsed.size_given = true;
            }
            "--scale" => {
                let value = args
                    .next()
                    .ok_or("--scale needs 100, 125, 150, 175 or 200")?;
                parsed.scale = parse_scale(&value).ok_or("--scale is 100, 125, 150, 175 or 200")?;
            }
            "--measure-memory" => {
                let value = args.next().ok_or("--measure-memory needs an output file")?;
                parsed.measure_memory = Some(PathBuf::from(value));
            }
            "--measure-animations" => {
                let value = args
                    .next()
                    .ok_or("--measure-animations needs an output file")?;
                parsed.measure_animations = Some(PathBuf::from(value));
            }
            "--display-inventory" => {
                let value = args
                    .next()
                    .ok_or("--display-inventory needs an output file")?;
                parsed.display_inventory = Some(PathBuf::from(value));
            }
            "--measure-presenter" => {
                let value = args
                    .next()
                    .ok_or("--measure-presenter needs an output file")?;
                parsed.measure_presenter = Some(PathBuf::from(value));
            }
            "--measure-output" => {
                let value = args.next().ok_or("--measure-output needs an output file")?;
                parsed.measure_output = Some(PathBuf::from(value));
            }
            "--run-drill" => {
                let value = args.next().ok_or("--run-drill needs an output file")?;
                parsed.run_drill = Some(PathBuf::from(value));
            }
            "--commands" => {
                let value = args.next().ok_or("--commands needs a number")?;
                parsed.commands = value
                    .parse()
                    .ok()
                    .filter(|n| (1..=kit::MAX_COMMANDS).contains(n))
                    .ok_or_else(|| format!("--commands is 1 to {}", kit::MAX_COMMANDS))?;
                parsed.commands_given = true;
            }
            "--cues" => {
                let value = args.next().ok_or("--cues needs 30 or 500")?;
                parsed.cues = match value.as_str() {
                    "30" => 30,
                    "500" => 500,
                    _ => return Err("--cues is 30 (the normal-mass fixture) or 500".into()),
                };
            }
            "--list" => parsed.list_view = true,
            "--grid" => parsed.list_view = false,
            "--snapshot" => {
                let value = args.next().ok_or("--snapshot needs a directory")?;
                parsed.snapshot = PathBuf::from(value);
                parsed.snapshot_given = true;
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => {
                parsed.snapshot = PathBuf::from(arg);
                parsed.snapshot_given = true;
            }
        }
    }
    let modes = [
        parsed.measure_memory.is_some(),
        parsed.measure_animations.is_some(),
        parsed.display_inventory.is_some(),
        parsed.measure_presenter.is_some(),
        parsed.measure_output.is_some(),
        parsed.run_drill.is_some(),
    ];
    if modes.into_iter().filter(|on| *on).count() > 1 {
        return Err("the measurement modes (--measure-memory, --measure-animations, --display-inventory, --measure-presenter, --measure-output, --run-drill) are separate runs".into());
    }
    if parsed.commands_given && parsed.measure_presenter.is_none() {
        return Err("--commands goes with --measure-presenter".into());
    }
    if parsed.measure_memory.is_some() || parsed.run_drill.is_some() {
        // The measurement is defined at one size and scale.
        parsed.width = measure::WIDTH;
        parsed.height = measure::HEIGHT;
        parsed.scale = InterfaceScale::Percent100;
        // The drill opens a window of that size, not a maximized one.
        parsed.size_given |= parsed.run_drill.is_some();
    }
    Ok(parsed)
}

fn scale_percent(scale: InterfaceScale) -> u32 {
    match scale {
        InterfaceScale::Percent100 => 100,
        InterfaceScale::Percent125 => 125,
        InterfaceScale::Percent150 => 150,
        InterfaceScale::Percent175 => 175,
        InterfaceScale::Percent200 => 200,
    }
}

fn lab_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/live-control-lab")
}

enum Stage {
    Loading,
    Failed(SharedString),
    Ready(Entity<PresenterView>),
}

/// The lab's root: shows the load stage, then hosts the Presenter and turns
/// the lab-only keys into commands. Owns the load task.
struct LabRoot {
    stage: Stage,
    focus: FocusHandle,
    environment: UiEnvironment,
    save: SavePreferences,
    started: Instant,
    /// Dropped with the root; the load finishes or is cancelled with it.
    _load: Option<Task<()>>,
    /// The startup image check (the building block of Preflight's, T1.6). Owned
    /// by the root, so closing the window cancels it; it decodes on the
    /// background executor and only reports back on the GPUI thread.
    _images: Option<Task<()>>,
    /// The memory measurement, until the snapshot is loaded and it starts.
    measure: Option<measure::Run>,
    /// The measurement's driver task, owned by the root like the load task.
    _measure_task: Option<Task<()>>,
    /// The animation measurement, until the snapshot is loaded and it starts.
    animations: Option<animations::Run>,
    /// The frame recorder the window's canvases feed during that measurement.
    pacing: Option<Rc<animations::Pacing>>,
    /// The church-kit measurement, until the snapshot is loaded and it starts.
    kit: Option<kit::Run>,
}

impl LabRoot {
    #[allow(clippy::too_many_arguments)]
    fn new(
        snapshot_dir: PathBuf,
        environment: UiEnvironment,
        save: SavePreferences,
        measure: Option<measure::Run>,
        animations: Option<animations::Run>,
        kit: Option<kit::Run>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let verified = verify_bundled_faces(window, cx);
        let mut root = Self {
            stage: Stage::Loading,
            focus: cx.focus_handle(),
            environment,
            save,
            started: Instant::now(),
            _load: None,
            _images: None,
            measure,
            _measure_task: None,
            pacing: animations.as_ref().map(|run| run.pacing.clone()),
            animations,
            kit,
        };
        match verified {
            Err(error) => {
                let message = format!(
                    "The bundled fonts did not verify, so the Presenter will not open: {error}"
                );
                if let Some(run) = root.measure.take() {
                    run.fail(&message, cx);
                }
                if let Some(run) = root.animations.take() {
                    run.fail(&message, cx);
                }
                if let Some(run) = root.kit.take() {
                    run.fail(&message, cx);
                }
                root.stage = Stage::Failed(message.into());
            }
            Ok(_) => {
                root._load = Some(cx.spawn_in(window, async move |this, cx| {
                    let loaded = cx
                        .background_executor()
                        .spawn(async move { read_snapshot(&DirectorySource::new(snapshot_dir)) })
                        .await;
                    this.update_in(cx, |root, window, cx| root.loaded(loaded, window, cx))
                        .ok();
                }));
            }
        }
        root
    }

    fn loaded(
        &mut self,
        loaded: Result<LoadedSnapshot, SnapshotLoadError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match loaded {
            Err(error) => {
                let message = format!("The snapshot did not load: {error}");
                if let Some(run) = self.measure.take() {
                    run.fail(&message, cx);
                }
                if let Some(run) = self.animations.take() {
                    run.fail(&message, cx);
                }
                if let Some(run) = self.kit.take() {
                    run.fail(&message, cx);
                }
                self.stage = Stage::Failed(message.into());
            }
            Ok(snapshot) => {
                let snapshot = Arc::new(snapshot);
                // The measurement skips the image check: its transient decodes
                // would be counted in the memory it is measuring.
                if self.measure.is_none() && self.animations.is_none() && self.kit.is_none() {
                    self.check_images(snapshot.clone(), window, cx);
                }
                let started = self.started;
                // The drill steps its own clock, so a tick of minutes costs none.
                let clock: Rc<dyn Fn() -> Millis> = match self.kit.as_ref().map(|run| run.clock()) {
                    Some(Some(stepped)) => Rc::new(move || Millis(stepped.get())),
                    _ => Rc::new(move || Millis(started.elapsed().as_millis() as u64)),
                };
                let environment = self.environment;
                let save = self.save.clone();
                let view = cx.new(|cx| {
                    let mut view = PresenterView::new(snapshot, clock, environment, window, cx);
                    view.set_save_preferences(save);
                    view
                });
                if let Some(run) = self.measure.take() {
                    self._measure_task = Some(run.start(view.clone(), window, cx));
                }
                if let Some(run) = self.animations.take() {
                    self._measure_task = Some(run.start(view.clone(), window, cx));
                }
                if let Some(run) = self.kit.take() {
                    self._measure_task = Some(run.start(view.clone(), window, cx));
                }
                // Keys reach the Presenter only from inside it: the Cues
                // hold focus, and the lab root above still answers E, D, R.
                view.update(cx, |view, cx| view.focus_timeline(window, cx));
                self.stage = Stage::Ready(view);
            }
        }
        if !matches!(self.stage, Stage::Ready(_)) {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Decode-check every image of the snapshot on the background executor
    /// and show "Images: N checked, M failed" in the window title; a failure
    /// also lists its asset id on stderr.
    fn check_images(
        &mut self,
        snapshot: Arc<LoadedSnapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let renderer = cx.svg_renderer();
        self._images = Some(cx.spawn_in(window, async move |this, cx| {
            let (checked, failures) = cx
                .background_executor()
                .spawn(async move {
                    let failures = check_images(&snapshot, &renderer);
                    (image_count(&snapshot), failures)
                })
                .await;
            this.update_in(cx, |_, window, _| {
                for (asset, error) in &failures {
                    eprintln!("live-control-lab: image {asset:?} failed: {error}");
                }
                window.set_window_title(&format!(
                    "Ordo live-control lab (development only) · Images: {checked} checked, {} failed · {}",
                    failures.len(),
                    key_help()
                ));
            })
            .ok();
        }));
    }

    fn send(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Stage::Ready(view) = &self.stage {
            view.update(cx, |view, cx| {
                // A refusal is already on the status line.
                view.command(command, cx).ok();
            });
        }
    }
}

impl Focusable for LabRoot {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for LabRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let message = |text: SharedString| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(scale.apply(tokens::spacing::SECTION)))
                .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                .child(text)
                .into_any_element()
        };
        let body = match &self.stage {
            Stage::Loading => message("Loading…".into()),
            Stage::Failed(text) => message(text.clone()),
            Stage::Ready(view) => div().size_full().child(view.clone()).into_any_element(),
        };
        div()
            .id("live-control-lab")
            .key_context("LiveControlLab")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .font_family(tokens::typography::OPERATOR_FAMILY)
            .on_action(
                cx.listener(|this, _: &EnableOutput, _, cx| this.send(Command::EnableOutput, cx)),
            )
            .on_action(cx.listener(|this, _: &SimulateDisplayLost, _, cx| {
                this.send(Command::DisplayLost, cx);
            }))
            .on_action(cx.listener(|this, _: &SimulateDisplayReturned, _, cx| {
                this.send(Command::DisplayReturned { recognized: true }, cx);
            }))
            .children(self.pacing.as_ref().map(|pacing| pacing.stamp(false)))
            .child(body)
            .children(self.pacing.as_ref().map(|pacing| pacing.stamp(true)))
    }
}

/// The worker that writes saved preferences, and the sender that feeds it.
struct PreferenceWriter {
    sender: Rc<RefCell<Option<mpsc::Sender<String>>>>,
    thread: JoinHandle<()>,
}

fn start_preference_writer(file: PreferencesFile) -> PreferenceWriter {
    let (sender, receiver) = mpsc::channel::<String>();
    let thread = std::thread::spawn(move || {
        // Ends when every sender is dropped.
        for contents in receiver {
            if let Err(error) = file.write(&contents) {
                eprintln!("live-control-lab: preferences were not saved: {error}");
            }
        }
    });
    PreferenceWriter {
        sender: Rc::new(RefCell::new(Some(sender))),
        thread,
    }
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("live-control-lab: {message}");
            eprintln!(
                "usage: live-control-lab [--size 1366x768] [--scale 100|125|150|175|200] [snapshot-dir] | --measure-memory <out.json> [--snapshot <dir>] | --measure-animations <out.json> [--size WxH] [--scale N] [--cues 30|500] [--list|--grid] | --display-inventory <out.json> | --measure-presenter <out.json> [--commands N] [--snapshot <dir>] [--size WxH] [--scale N] | --measure-output <out.json> [--snapshot <dir>] [--size WxH] [--scale N] | --run-drill <out.json> [--snapshot <dir>]"
            );
            std::process::exit(2);
        }
    };

    // The display inventory opens no window and needs nothing else.
    if let Some(out) = args.display_inventory.clone() {
        application().run(move |cx: &mut App| kit::inventory(cx, out));
        if measure::FAILED.load(Ordering::SeqCst) {
            std::process::exit(1);
        }
        return;
    }
    // The presenter and output measurements open the fixture; say at once if it
    // is not there.
    let kit_out = args
        .measure_presenter
        .clone()
        .map(|out| (out, "presenter-latency"))
        .or_else(|| {
            args.measure_output
                .clone()
                .map(|out| (out, "output-latency"))
        })
        .or_else(|| args.run_drill.clone().map(|out| (out, kit::MODE_DRILL)));
    let mut args = args;
    if let Some((out, mode)) = &kit_out {
        let fixture = if args.run_drill.is_some() {
            "recovery-drill"
        } else {
            "normal-mass"
        };
        match kit::resolve_snapshot(&args, fixture) {
            Ok(path) => args.snapshot = path,
            Err(message) => kit::fail_early(out, mode, &message),
        }
    }

    // The measurement writes and reads no preferences, so a run depends only on
    // its own arguments.
    let measuring = args.measure_memory.is_some()
        || args.measure_animations.is_some()
        || args.display_inventory.is_some()
        || args.measure_presenter.is_some()
        || args.measure_output.is_some()
        || args.run_drill.is_some();
    let preferences_file = PreferencesFile::new(lab_dir().join("preferences"));
    // Before the app loop: a few bytes, read once, restoring the Grid/List choice.
    let restored = if measuring {
        UiPreferences::default()
    } else {
        preferences_file
            .read()
            .ok()
            .flatten()
            .and_then(|contents| UiPreferences::decode(&contents).ok())
            .unwrap_or_default()
    };
    let preferences = UiPreferences {
        scale: args.scale,
        ..restored
    };
    let mut environment = UiEnvironment::new(preferences, ResolvedAppearance::Dark);
    environment.apply(EnvironmentChange::EnterPresenter);

    let writer = start_preference_writer(preferences_file);
    let sender = writer.sender.clone();
    let save: SavePreferences = if measuring {
        Rc::new(|_: UiPreferences| {})
    } else {
        let sender = writer.sender.clone();
        Rc::new(move |preferences: UiPreferences| {
            if let Some(sender) = sender.borrow().as_ref() {
                sender.send(preferences.encode()).ok();
            }
        })
    };

    // A measurement without `--snapshot` writes the synthetic 500-Cue snapshot
    // under the temp directory, and removes it once the app loop has ended.
    let mut scratch: Option<PathBuf> = None;
    let wants_synthetic =
        args.measure_memory.is_some() || (args.measure_animations.is_some() && args.cues == 500);
    if wants_synthetic && !args.snapshot_given {
        let dir = std::env::temp_dir().join(format!("ordo-live-presenter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(error) = presentation::test_support::write_synthetic_snapshot(&dir) {
            eprintln!("live-control-lab: cannot write the synthetic snapshot: {error}");
            let _ = std::fs::remove_dir_all(&dir);
            std::process::exit(1);
        }
        args.snapshot = dir.clone();
        scratch = Some(dir);
    }

    application().run(move |cx: &mut App| {
        if let Err(error) = ui::init(cx) {
            eprintln!("live-control-lab: the UI could not start: {error}");
            cx.quit();
            return;
        }
        cx.bind_keys(ordo::commands::registry::key_bindings());
        cx.bind_keys([
            KeyBinding::new("e", EnableOutput, None),
            KeyBinding::new("d", SimulateDisplayLost, None),
            KeyBinding::new("r", SimulateDisplayReturned, None),
        ]);
        let bounds = Bounds::centered(None, size(px(args.width), px(args.height)), cx);
        let snapshot = args.snapshot.clone();
        let mut measure = None;
        let measure_out = args
            .measure_memory
            .clone()
            .or_else(|| args.measure_animations.clone())
            .or_else(|| kit_out.as_ref().map(|(out, _)| out.clone()));
        let mut kit_run = None;
        if let Some((out, _)) = kit_out.clone() {
            if args.measure_output.is_some() && cx.displays().len() < 2 {
                // Nothing to drive: a clean, successful skip.
                kit::finish(cx, &out, &kit::skipped_doc("no congregation display"), true);
                return;
            }
            // Started before the window opens, so the peak covers the load.
            let watch = if args.run_drill.is_some() {
                match MemoryWatch::start(cx) {
                    Ok(watch) => Some(watch),
                    Err(message) => {
                        let document = kit::error_doc(kit::MODE_DRILL, &message);
                        kit::finish(cx, &out, &document, false);
                        return;
                    }
                }
            } else {
                None
            };
            let run = kit::Run {
                out,
                kind: if args.run_drill.is_some() {
                    kit::Kind::Drill
                } else if args.measure_presenter.is_some() {
                    kit::Kind::Presenter {
                        commands: args.commands,
                    }
                } else {
                    kit::Kind::Output
                },
                snapshot: args.snapshot.clone(),
                board: Default::default(),
                clock: Default::default(),
                watch,
                width: args.width,
                height: args.height,
                scale_percent: scale_percent(args.scale),
            };
            run.arm(cx);
            kit_run = Some(run);
        }
        // The kit modes judge the Presenter at the size it really opens at,
        // unless `--size` says otherwise.
        let window_bounds = match (kit_run.is_some() && !args.size_given, cx.primary_display()) {
            (true, Some(display)) => WindowBounds::Maximized(display.visible_bounds()),
            _ => WindowBounds::Windowed(bounds),
        };
        let mut animations = None;
        if let Some(out) = args.measure_animations.clone() {
            measure::arm_timeout_after(cx, out.clone(), animations::TIMEOUT);
            animations = Some(animations::Run::new(
                out,
                animations::Config {
                    cues_requested: args.cues,
                    list_view: args.list_view,
                    scale_percent: scale_percent(args.scale),
                    width: args.width,
                    height: args.height,
                },
            ));
        }
        if let Some(out) = args.measure_memory.clone() {
            // Started before the window opens, so the peak covers the load.
            match MemoryWatch::start(cx) {
                Ok(watch) => {
                    measure::arm_timeout(cx, out.clone());
                    measure = Some(measure::Run { out, watch });
                }
                Err(message) => {
                    measure::finish(cx, &out, &measure::error_json(&message), false);
                    return;
                }
            }
        }
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(window_bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        format!("Ordo live-control lab (development only) · {}", key_help()).into(),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let root = LabRoot::new(
                        snapshot,
                        environment,
                        save,
                        measure,
                        animations,
                        kit_run,
                        window,
                        cx,
                    );
                    window.focus(&root.focus_handle(cx), cx);
                    root
                })
            },
        );
        if let Err(error) = opened {
            eprintln!("live-control-lab: cannot open the window: {error:?}");
            if let Some(out) = &measure_out {
                // The watch was dropped with the closure; the document says why.
                let message = format!("cannot open the window: {error:?}");
                measure::finish(cx, out, &measure::error_json(&message), false);
                return;
            }
            cx.quit();
            return;
        }
        // Closing the window ends the lab: drop the preference sender first
        // so the writer drains, then quit.
        // The kit modes end through `kit::finish`, which closes the congregation
        // window itself, so a window closing is not the end of the run there.
        let ends_on_close = kit_out.is_none();
        cx.on_window_closed(move |cx, _| {
            if ends_on_close || cx.windows().is_empty() {
                sender.borrow_mut().take();
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });

    // Every sender is gone once the window closed; wait for the last save.
    writer.sender.borrow_mut().take();
    if writer.thread.join().is_err() {
        eprintln!("live-control-lab: the preference writer panicked");
    }
    if let Some(dir) = scratch {
        // Best effort: it is only the generated snapshot.
        let _ = std::fs::remove_dir_all(dir);
    }
    if measure::FAILED.load(Ordering::SeqCst) {
        std::process::exit(1);
    }
}

/// The T1.3d memory measurement (`--measure-memory`): drive the real
/// `PresenterView` through forward, reverse and random navigation over 500
/// Cues, and sample process memory after each phase.
///
/// Ownership and shutdown: `main` starts the [`MemoryWatch`] and the timeout
/// before the window opens; the root owns the driver task, so closing the
/// window cancels it; `finish` is the only exit and writes the document
/// exactly once. Nothing here blocks the GPUI thread: every sample runs on the
/// background executor, and the driver yields a short step after each command
/// so the window paints and the frame store's jobs run as they do in use.
mod measure {
    use super::*;
    use ordo::performance::{
        memory::{json_string, sample_process},
        process::{PeakMemory, ProcessSample},
    };

    /// The window the measurement is defined at: 1920x1080 at 100 %.
    pub const WIDTH: f32 = 1920.0;
    pub const HEIGHT: f32 = 1080.0;
    /// Yield after each command, so the window paints between commands.
    const STEP: Duration = Duration::from_millis(4);
    /// How often a settle checks the frame store, how many quiet checks in a
    /// row end it, and the longest it waits.
    const SETTLE_POLL: Duration = Duration::from_millis(50);
    const SETTLE_QUIET_POLLS: usize = 4;
    const SETTLE_LIMIT: Duration = Duration::from_secs(30);
    /// A mid-phase settle every this many steps (no sample is taken).
    const CHECKPOINT_EVERY: usize = 250;
    /// Idle time after the frames settled, before a phase is sampled (the same
    /// 2 s the Phase 0 baselines use).
    const SETTLE_BEFORE_SAMPLE: Duration = Duration::from_secs(2);
    /// Every this many steps a rail jump moves the timeline, so Visible
    /// thumbnails churn.
    const JUMP_EVERY: usize = 12;
    /// The seed of the random phase.
    const SEED: u64 = 0x5EED_0013;
    /// The run gets this long, then writes an error document and exits
    /// nonzero, so CI cannot hang.
    const TIMEOUT: Duration = Duration::from_secs(420);
    /// What `private_bytes` and the peaks mean in the document.
    const PRIVATE_BYTES_NOTE: &str = "Windows PrivateUsage; peaks are OS-recorded since process start (Decision 269); null off Windows";

    /// Set by the first `finish`, so a late timeout cannot overwrite a result.
    static FINISHED: AtomicBool = AtomicBool::new(false);
    /// Set when the run ends with an error document; makes the process exit 1.
    pub static FAILED: AtomicBool = AtomicBool::new(false);

    /// What `main` hands the root: where to write, and the peak watch.
    pub struct Run {
        pub out: PathBuf,
        pub watch: MemoryWatch,
    }

    pub fn error_json(message: &str) -> String {
        format!("{{\"error\":{}}}", json_string(message))
    }

    /// Writes `document` to `out` and quits. Only the first call has an effect.
    pub fn finish(cx: &mut App, out: &Path, document: &str, ok: bool) {
        if FINISHED.swap(true, Ordering::SeqCst) {
            return;
        }
        if !ok {
            FAILED.store(true, Ordering::SeqCst);
        }
        if let Err(error) = std::fs::write(out, document) {
            eprintln!("live-control-lab: cannot write {}: {error}", out.display());
            FAILED.store(true, Ordering::SeqCst);
        }
        cx.quit();
    }

    /// Fails the run with an error document if it has not finished in time.
    pub fn arm_timeout(cx: &mut App, out: PathBuf) {
        arm_timeout_after(cx, out, TIMEOUT);
    }

    /// [`arm_timeout`] with an explicit limit.
    pub fn arm_timeout_after(cx: &mut App, out: PathBuf, limit: Duration) {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(limit).await;
            cx.update(|cx| {
                let message = format!("timed out after {} s", limit.as_secs());
                finish(cx, &out, &error_json(&message), false);
            });
        })
        .detach();
    }

    impl Run {
        /// The run cannot start (the fonts or the snapshot did not load).
        pub fn fail(self, message: &str, cx: &mut App) {
            self.watch.finish(None);
            finish(cx, &self.out, &error_json(message), false);
        }

        /// Starts the driver. The task is the caller's to own.
        pub fn start(
            self,
            view: Entity<PresenterView>,
            window: &Window,
            cx: &mut Context<LabRoot>,
        ) -> Task<()> {
            let Self { out, watch } = self;
            cx.spawn_in(window, async move |_, cx| {
                let result = drive(&view, &watch, cx).await;
                watch.finish(None);
                cx.update(|_, cx| match &result {
                    Ok(document) => finish(cx, &out, document, true),
                    Err(message) => finish(cx, &out, &error_json(message), false),
                })
                .ok();
            })
        }
    }

    /// A tiny deterministic generator (xorshift64), so the random phase is the
    /// same on every run.
    struct XorShift(u64);

    impl XorShift {
        fn below(&mut self, bound: usize) -> usize {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            (x % bound.max(1) as u64) as usize
        }
    }

    enum Step {
        Command(Command),
        /// A rail jump to a Mass item's group.
        Jump(usize),
    }

    /// Output, Take, Freeze and Blackout interleaved with navigation step `i`,
    /// the way the 500-Cue navigation test mixes them.
    fn interleaved(i: usize) -> Option<Command> {
        match i % 150 {
            0 => Some(Command::EnableOutput),
            9 | 39 | 69 | 99 | 129 => Some(Command::Take),
            19 | 21 => Some(Command::ToggleFreeze),
            29 | 31 => Some(Command::ToggleBlackout),
            _ => None,
        }
    }

    /// One sweep across every Cue: `first`, then `cues - 1` of `step`.
    fn sweep(
        first: Command,
        step: Command,
        cues: usize,
        groups: usize,
        rng: &mut XorShift,
    ) -> Vec<Step> {
        let mut plan = vec![Step::Command(first), Step::Command(Command::EnableOutput)];
        for i in 0..cues.saturating_sub(1) {
            plan.push(Step::Command(step));
            plan.extend(interleaved(i).map(Step::Command));
            if i % JUMP_EVERY == JUMP_EVERY - 1 {
                plan.push(Step::Jump(rng.below(groups)));
            }
        }
        plan
    }

    /// Seeded random navigation and live controls: 3 steps for every Cue.
    fn random(cues: usize, groups: usize, rng: &mut XorShift) -> Vec<Step> {
        let mut plan = Vec::new();
        for i in 0..cues * 3 {
            let command = match rng.below(100) {
                0..=34 => Command::Next,
                35..=59 => Command::Previous,
                60..=69 => Command::GoTo(rng.below(cues)),
                70..=77 => Command::NextItem,
                78..=85 => Command::PreviousItem,
                86..=89 => Command::Take,
                90..=92 => Command::ToggleFreeze,
                93..=95 => Command::ToggleBlackout,
                _ => Command::EnableOutput,
            };
            plan.push(Step::Command(command));
            if i % JUMP_EVERY == JUMP_EVERY - 1 {
                plan.push(Step::Jump(rng.below(groups)));
            }
        }
        plan
    }

    /// What the frame store holds at one instant.
    #[derive(Clone, Copy, Default)]
    struct Stats {
        cache_len: usize,
        full_images: usize,
        thumb_images: usize,
        jobs: usize,
        composed: usize,
        idle: bool,
    }

    /// The maxima over the whole run.
    #[derive(Default)]
    struct Maxima {
        cache_len: usize,
        full_images: usize,
        thumb_images: usize,
        jobs: usize,
        composed: usize,
    }

    impl Maxima {
        fn observe(&mut self, stats: Stats) {
            self.cache_len = self.cache_len.max(stats.cache_len);
            self.full_images = self.full_images.max(stats.full_images);
            self.thumb_images = self.thumb_images.max(stats.thumb_images);
            self.jobs = self.jobs.max(stats.jobs);
            self.composed = self.composed.max(stats.composed);
        }
    }

    fn stats(view: &Entity<PresenterView>, cx: &mut AsyncWindowContext) -> Result<Stats, String> {
        Ok(view.read_with(cx, |view, _| {
            let frames = view.frames();
            let (full_images, thumb_images) = frames.image_counts();
            Stats {
                cache_len: frames.cache_len(),
                full_images,
                thumb_images,
                jobs: frames.jobs_in_flight(),
                composed: frames.composed_count(),
                idle: frames.jobs_in_flight() == 0 && frames.upgrading() == 0,
            }
        }))
    }

    /// Waits until the frame store has had no job in flight for a few checks
    /// in a row. Returns whether it settled within the limit.
    async fn settle(
        view: &Entity<PresenterView>,
        cx: &mut AsyncWindowContext,
        maxima: &mut Maxima,
    ) -> Result<bool, String> {
        let background = cx.background_executor().clone();
        let started = Instant::now();
        let mut quiet = 0;
        while quiet < SETTLE_QUIET_POLLS {
            if started.elapsed() > SETTLE_LIMIT {
                return Ok(false);
            }
            background.timer(SETTLE_POLL).await;
            let now = stats(view, cx)?;
            maxima.observe(now);
            quiet = if now.idle { quiet + 1 } else { 0 };
        }
        Ok(true)
    }

    /// One phase's sample.
    struct Phase {
        name: &'static str,
        steps: usize,
        commands: usize,
        jumps: usize,
        settled: bool,
        sample: ProcessSample,
        peak: PeakMemory,
        at: Stats,
    }

    fn optional(value: Option<u64>) -> String {
        value.map_or_else(|| "null".to_owned(), |value| value.to_string())
    }

    impl Phase {
        fn json(&self) -> String {
            format!(
                "{{\"phase\":{},\"steps\":{},\"commands\":{},\"rail_jumps\":{},\"settled\":{},\"rss_bytes\":{},\"private_bytes\":{},\"virtual_bytes\":{},\"peak_rss_bytes\":{},\"peak_private_bytes\":{},\"cache_len\":{},\"full_images\":{},\"thumb_images\":{},\"frames_composed\":{}}}",
                json_string(self.name),
                self.steps,
                self.commands,
                self.jumps,
                self.settled,
                self.sample.rss_bytes,
                optional(self.sample.private_bytes),
                self.sample.virtual_bytes,
                self.peak.peak_rss_bytes,
                optional(self.peak.peak_private_bytes),
                self.at.cache_len,
                self.at.full_images,
                self.at.thumb_images,
                self.at.composed,
            )
        }
    }

    /// Runs one phase's plan, then settles and samples.
    async fn run_phase(
        name: &'static str,
        plan: Vec<Step>,
        view: &Entity<PresenterView>,
        watch: &MemoryWatch,
        cx: &mut AsyncWindowContext,
        maxima: &mut Maxima,
    ) -> Result<Phase, String> {
        let background = cx.background_executor().clone();
        let (mut commands, mut jumps) = (0, 0);
        // Every checkpoint settle counts: a phase is settled only if all were.
        let mut checkpoints_settled = true;
        for (index, step) in plan.iter().enumerate() {
            view.update_in(cx, |view, _, cx| match step {
                Step::Command(command) => {
                    // A refusal is the Engine doing its job; it is on the
                    // status line and changes nothing else.
                    view.command(*command, cx).ok();
                }
                Step::Jump(group) => view.jump_to_group(*group, cx),
            })
            .map_err(|error| format!("the Presenter is gone: {error}"))?;
            match step {
                Step::Command(_) => commands += 1,
                Step::Jump(_) => jumps += 1,
            }
            background.timer(STEP).await;
            maxima.observe(stats(view, cx)?);
            if (index + 1) % CHECKPOINT_EVERY == 0 {
                checkpoints_settled &= settle(view, cx, maxima).await?;
            }
        }
        let settled = settle(view, cx, maxima).await? && checkpoints_settled;
        background.timer(SETTLE_BEFORE_SAMPLE).await;
        let at = stats(view, cx)?;
        maxima.observe(at);
        let sample = background.spawn(async { sample_process() }).await?;
        let peak = watch.peak_with(Some(sample));
        Ok(Phase {
            name,
            steps: plan.len(),
            commands,
            jumps,
            settled,
            sample,
            peak,
            at,
        })
    }

    async fn drive(
        view: &Entity<PresenterView>,
        watch: &MemoryWatch,
        cx: &mut AsyncWindowContext,
    ) -> Result<String, String> {
        let (cues, groups, (width, height)) = view
            .update_in(cx, |view, window, cx| {
                view.set_cue_view(CueView::Grid, cx);
                view.set_rail_choice(RailChoice::Pinned, cx);
                let snapshot = view.engine().snapshot();
                let viewport = window.viewport_size();
                (
                    snapshot.cue_count(),
                    snapshot.item_count(),
                    (f32::from(viewport.width), f32::from(viewport.height)),
                )
            })
            .map_err(|error| format!("the Presenter is gone: {error}"))?;

        let mut maxima = Maxima::default();
        // The first Cue's frames and the first screen of thumbnails.
        settle(view, cx, &mut maxima).await?;

        let mut rng = XorShift(SEED);
        let forward = sweep(Command::First, Command::Next, cues, groups, &mut rng);
        let reverse = sweep(Command::Last, Command::Previous, cues, groups, &mut rng);
        let random = random(cues, groups, &mut rng);
        let mut phases = Vec::new();
        for (name, plan) in [
            ("forward", forward),
            ("reverse", reverse),
            ("random", random),
        ] {
            phases.push(run_phase(name, plan, view, watch, cx, &mut maxima).await?);
        }
        let last = phases.last().ok_or("no phase ran")?;
        let (sample, peak) = (last.sample, watch.finish(Some(last.sample)));
        let commands: usize = phases.iter().map(|phase| phase.commands).sum();
        let jumps: usize = phases.iter().map(|phase| phase.jumps).sum();
        let phase_json: Vec<String> = phases.iter().map(Phase::json).collect();
        Ok(format!(
            "{{\"mode\":\"live-presenter\",\"description\":{},\"rss_bytes\":{},\"private_bytes\":{},\"private_bytes_note\":{},\"virtual_bytes\":{},\"peak_rss_bytes\":{},\"peak_private_bytes\":{},\"seed\":{SEED},\"cue_count\":{cues},\"window\":{},\"commands\":{commands},\"rail_jumps\":{jumps},\"frames_composed\":{},\"max_cache_len\":{},\"max_full_images\":{},\"max_thumb_images\":{},\"max_jobs_in_flight\":{},\"phases\":[{}]}}",
            json_string(
                "the real PresenterView (timeline, rail, dock, frame store) over a synthetic 500-Cue snapshot; figures are after the random phase"
            ),
            sample.rss_bytes,
            optional(sample.private_bytes),
            json_string(PRIVATE_BYTES_NOTE),
            sample.virtual_bytes,
            peak.peak_rss_bytes,
            optional(peak.peak_private_bytes),
            json_string(&format!("{width}x{height}@100 Grid, rail pinned")),
            maxima.composed,
            maxima.cache_len,
            maxima.full_images,
            maxima.thumb_images,
            maxima.jobs,
            phase_json.join(","),
        ))
    }
}

/// The animation frame-pacing measurement (`--measure-animations`, #111): open
/// the real `PresenterView`, then script each animation in turn with idle gaps
/// and record, while it runs, every drawn frame's interval, the per-frame
/// render, layout, prepaint and paint cost, and what the frame store did.
///
/// A frame is stamped when the last paint canvas of the window runs (before
/// the GPU present, which Windows issues with a sync interval of 0), so an
/// interval is the draw cadence the vsync thread drives. Render time comes from
/// the Presenter's render probe; layout is the time from the end of the last
/// render to the first prepaint canvas; prepaint and paint are the spans
/// between the first and last canvas of each phase. The two canvases bracket
/// the whole window content.
///
/// Before the scripted animations a refresh probe draws frames back to back
/// for about a second with no state change; the median interval is
/// `refresh_ms`, the Presenter's own idle draw cadence (no OS display query).
/// That equals the display refresh only when an idle frame fits in one vsync
/// and the native vsync source is in use, so the probe's own frame cost p95 is
/// recorded as `refresh_cpu_p95_ms` and the gate in xtask checks both.
/// `limits_ms.hitch` (33) is a fixed bar and `limits_ms.p95_bar` is 1.5 refresh
/// intervals, echoed for the gate to cross-check; xtask owns the factor and
/// judges against the measured refresh, not a fixed 16.7 ms. `limits_ms.frame` and `dropped`
/// only feed the older `over_16_7ms_pct` and `over_25ms` fields. Per animation,
/// `p95_over_refresh` and `cpu_p95_over_refresh` are the interval and frame
/// cost p95 in refresh intervals.
///
/// Ownership and shutdown: `main` arms the timeout before the window opens;
/// the root owns the driver task and the [`Pacing`] recorder; `measure::finish`
/// is the only exit.
mod animations {
    use super::*;
    use ordo::{
        performance::{frames::FrameTimer, memory::json_string},
        presenter::{RenderProbe, RenderStat},
    };

    /// The run gets this long, then writes an error document and exits nonzero.
    pub const TIMEOUT: Duration = Duration::from_secs(150);
    /// Idle time between two scripted presses, with nothing recorded.
    const GAP: Duration = Duration::from_millis(250);
    /// How often a hold samples the frame store.
    const POLL: Duration = Duration::from_millis(8);
    const SETTLE_POLL: Duration = Duration::from_millis(50);
    const SETTLE_QUIET_POLLS: usize = 4;
    const SETTLE_LIMIT: Duration = Duration::from_secs(10);
    /// How long the refresh probe asks for continuous frames.
    const PROBE: Duration = Duration::from_secs(1);
    /// Fewer probe intervals than this and the refresh is not trusted.
    const PROBE_MIN_SAMPLES: usize = 20;
    /// The p95 bar is this many refresh intervals.
    const P95_BAR: f64 = 1.5;
    /// The 60 Hz reference of the legacy `over_16_7ms_pct` field.
    const FRAME_MS: f64 = 16.7;
    const DROP_MS: f64 = 25.0;
    const HITCH_MS: f64 = 33.0;

    fn ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1000.0
    }

    /// One drawn frame inside a segment.
    #[derive(Clone, Copy)]
    struct Frame {
        interval: Option<f64>,
        render: f64,
        layout: f64,
        prepaint: f64,
        paint: f64,
        arrival: bool,
    }

    impl Frame {
        fn cpu(&self) -> f64 {
            self.render + self.layout + self.prepaint + self.paint
        }
    }

    #[derive(Default)]
    struct Inner {
        open: bool,
        cmd_at: Option<Instant>,
        last: Option<Instant>,
        timer: Option<FrameTimer>,
        prev_composed: Option<usize>,
        render: Option<(Instant, Duration, usize)>,
        prepaint_first: Option<Instant>,
        prepaint_last: Option<Instant>,
        paint_first: Option<Instant>,
        frames: Vec<Frame>,
        first_frames: Vec<f64>,
        dropped_1_5x: u64,
        segments: usize,
        inactive_frames: usize,
        /// The refresh probe: every drawn frame asks for the next one.
        probing: bool,
    }

    /// What one animation recorded.
    #[derive(Default)]
    pub struct Capture {
        frames: Vec<Frame>,
        first_frames: Vec<f64>,
        dropped_1_5x: u64,
        segments: usize,
        inactive_frames: usize,
    }

    /// The recorder the window's canvases and the render probe feed.
    #[derive(Default)]
    pub struct Pacing(RefCell<Inner>);

    impl Pacing {
        pub fn probe(self: &Rc<Self>) -> RenderProbe {
            let this = self.clone();
            Rc::new(move |stat: RenderStat| {
                this.0.borrow_mut().render = Some((Instant::now(), stat.render, stat.composed));
            })
        }

        /// A zero-size canvas stamping the prepaint and paint phases. The one
        /// placed first in the window is `last == false`.
        pub fn stamp(self: &Rc<Self>, last: bool) -> gpui::AnyElement {
            let (prepaint, paint) = (self.clone(), self.clone());
            gpui::canvas(
                move |_, _, _| {
                    let now = Instant::now();
                    let mut inner = prepaint.0.borrow_mut();
                    if last {
                        inner.prepaint_last = Some(now);
                    } else {
                        inner.prepaint_first = Some(now);
                    }
                },
                move |_, _, window, _| {
                    let now = Instant::now();
                    let mut inner = paint.0.borrow_mut();
                    if last {
                        inner.finish_frame(now, window.is_window_active());
                        if inner.probing {
                            drop(inner);
                            window.request_animation_frame();
                        }
                    } else {
                        inner.paint_first = Some(now);
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_0()
            .into_any_element()
        }

        fn set_probing(&self, probing: bool) {
            self.0.borrow_mut().probing = probing;
        }

        fn begin_segment(&self, composed: usize) {
            let mut inner = self.0.borrow_mut();
            inner.open = true;
            inner.cmd_at = Some(Instant::now());
            inner.last = None;
            inner.timer = Some(FrameTimer::with_expected(
                Duration::from_micros(16_700),
                1.5,
            ));
            inner.prev_composed = Some(composed);
            inner.segments += 1;
        }

        fn end_segment(&self) {
            let mut inner = self.0.borrow_mut();
            inner.open = false;
            if let Some(timer) = inner.timer.take() {
                inner.dropped_1_5x += timer.dropped_frames();
            }
        }

        fn take(&self) -> Capture {
            let mut inner = self.0.borrow_mut();
            Capture {
                frames: std::mem::take(&mut inner.frames),
                first_frames: std::mem::take(&mut inner.first_frames),
                dropped_1_5x: std::mem::take(&mut inner.dropped_1_5x),
                segments: std::mem::take(&mut inner.segments),
                inactive_frames: std::mem::take(&mut inner.inactive_frames),
            }
        }
    }

    impl Inner {
        fn finish_frame(&mut self, now: Instant, active: bool) {
            let render = self.render.take();
            let (first, last_prepaint, paint_first) = (
                self.prepaint_first.take(),
                self.prepaint_last.take(),
                self.paint_first.take(),
            );
            if !self.open {
                return;
            }
            let span = |from: Option<Instant>, to: Option<Instant>| match (from, to) {
                (Some(from), Some(to)) => ms(to.saturating_duration_since(from)),
                _ => 0.0,
            };
            let (render_ms, layout_ms, arrival) = match render {
                Some((ended, took, composed)) => {
                    let arrival = self.prev_composed != Some(composed);
                    self.prev_composed = Some(composed);
                    (ms(took), span(Some(ended), first), arrival)
                }
                None => (0.0, 0.0, false),
            };
            if !active {
                self.inactive_frames += 1;
            }
            if let Some(timer) = self.timer.as_mut() {
                timer.record_frame(now);
            }
            let interval = self
                .last
                .map(|last| ms(now.saturating_duration_since(last)));
            if interval.is_none()
                && let Some(at) = self.cmd_at
            {
                self.first_frames
                    .push(ms(now.saturating_duration_since(at)));
            }
            self.last = Some(now);
            self.frames.push(Frame {
                interval,
                render: render_ms,
                layout: layout_ms,
                prepaint: span(first, last_prepaint),
                paint: span(paint_first, Some(now)),
                arrival,
            });
        }
    }

    /// Nearest-rank percentile of already sorted `values`; 0 when empty.
    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
        sorted[rank.min(sorted.len()) - 1]
    }

    fn summary(mut values: Vec<f64>) -> String {
        values.sort_by(|a, b| a.total_cmp(b));
        format!(
            "{{\"p50\":{:.2},\"p95\":{:.2},\"p99\":{:.2},\"max\":{:.2}}}",
            percentile(&values, 50.0),
            percentile(&values, 95.0),
            percentile(&values, 99.0),
            values.last().copied().unwrap_or(0.0),
        )
    }

    /// The frame store's counters at one instant.
    #[derive(Clone, Copy, Default)]
    struct Counters {
        composed: usize,
        full: usize,
        thumb: usize,
        jobs: usize,
        upgrading: usize,
    }

    fn counters(view: &Entity<PresenterView>, cx: &mut AsyncWindowContext) -> Counters {
        view.read_with(cx, |view, _| {
            let frames = view.frames();
            let (full, thumb) = frames.image_counts();
            Counters {
                composed: frames.composed_count(),
                full,
                thumb,
                jobs: frames.jobs_in_flight(),
                upgrading: frames.upgrading(),
            }
        })
    }

    async fn settle(view: &Entity<PresenterView>, cx: &mut AsyncWindowContext) -> bool {
        let background = cx.background_executor().clone();
        let started = Instant::now();
        let mut quiet = 0;
        while quiet < SETTLE_QUIET_POLLS {
            if started.elapsed() > SETTLE_LIMIT {
                return false;
            }
            background.timer(SETTLE_POLL).await;
            let now = counters(view, cx);
            quiet = if now.jobs == 0 && now.upgrading == 0 {
                quiet + 1
            } else {
                0
            };
        }
        true
    }

    #[derive(Clone, Copy)]
    enum Step {
        Command(Command),
        OpenRail,
        CloseRail,
        ToggleGroup(usize),
    }

    struct Script {
        name: &'static str,
        what: &'static str,
        hold: Duration,
        steps: Vec<Step>,
        /// Fold every group first: a diagnostic that shows how much of the
        /// per-frame cost is the Cue tiles.
        fold_all: bool,
    }

    fn scripts(groups: usize) -> Vec<Script> {
        use Command::*;
        let c = Step::Command;
        let group = |g: usize| Step::ToggleGroup(g % groups.max(1));
        vec![
            Script {
                name: "timeline_glide",
                what: "Next, NextItem, Last, PreviousItem and First across group boundaries",
                hold: Duration::from_millis(450),
                steps: [
                    Next,
                    Next,
                    Next,
                    NextItem,
                    NextItem,
                    NextItem,
                    NextItem,
                    NextItem,
                    Next,
                    NextItem,
                    NextItem,
                    NextItem,
                    Last,
                    PreviousItem,
                    PreviousItem,
                    First,
                    NextItem,
                    NextItem,
                    NextItem,
                    NextItem,
                ]
                .map(c)
                .to_vec(),
                fold_all: false,
            },
            Script {
                name: "rail_overlay",
                what: "open then close the rail overlay (rail unpinned)",
                hold: Duration::from_millis(500),
                steps: [Step::OpenRail, Step::CloseRail].repeat(4),
                fold_all: false,
            },
            Script {
                name: "group_fold",
                what: "fold then unfold Cue groups",
                hold: Duration::from_millis(450),
                steps: vec![
                    group(1),
                    group(1),
                    group(3),
                    group(3),
                    group(0),
                    group(0),
                    group(2),
                    group(2),
                ],
                fold_all: false,
            },
            Script {
                name: "output_fades",
                what: "EnableOutput, Take, Blackout on and off (500 ms fades)",
                hold: Duration::from_millis(750),
                steps: [
                    EnableOutput,
                    Next,
                    Take,
                    ToggleBlackout,
                    ToggleBlackout,
                    Next,
                    Take,
                    Next,
                    Take,
                    ToggleBlackout,
                    ToggleBlackout,
                ]
                .map(c)
                .to_vec(),
                fold_all: false,
            },
            Script {
                name: "output_fades_all_folded",
                what: "diagnostic: the output fades again with every group folded",
                hold: Duration::from_millis(750),
                steps: [
                    Next,
                    Take,
                    ToggleBlackout,
                    ToggleBlackout,
                    Next,
                    Take,
                    ToggleBlackout,
                    ToggleBlackout,
                ]
                .map(c)
                .to_vec(),
                fold_all: true,
            },
        ]
    }

    /// The command-line facts the document records.
    pub struct Config {
        pub cues_requested: usize,
        pub list_view: bool,
        pub scale_percent: u32,
        pub width: f32,
        pub height: f32,
    }

    pub struct Run {
        pub out: PathBuf,
        pub config: Config,
        pub pacing: Rc<Pacing>,
    }

    impl Run {
        pub fn new(out: PathBuf, config: Config) -> Self {
            Self {
                out,
                config,
                pacing: Rc::new(Pacing::default()),
            }
        }

        pub fn fail(self, message: &str, cx: &mut App) {
            measure::finish(cx, &self.out, &measure::error_json(message), false);
        }

        pub fn start(
            self,
            view: Entity<PresenterView>,
            window: &Window,
            cx: &mut Context<LabRoot>,
        ) -> Task<()> {
            view.update(cx, |view, _| view.set_render_probe(self.pacing.probe()));
            cx.spawn_in(window, async move |_, cx| {
                let result = drive(&self, &view, cx).await;
                cx.update(|_, cx| match &result {
                    Ok(document) => measure::finish(cx, &self.out, document, true),
                    Err(message) => {
                        measure::finish(cx, &self.out, &measure::error_json(message), false);
                    }
                })
                .ok();
            })
        }
    }

    /// A JSON number with `places` decimals, or `null`.
    fn number(value: Option<f64>, places: usize) -> String {
        match value {
            Some(value) => format!("{value:.places$}"),
            None => "null".to_owned(),
        }
    }

    fn pct(part: usize, whole: usize) -> f64 {
        if whole == 0 {
            0.0
        } else {
            part as f64 * 100.0 / whole as f64
        }
    }

    /// The display's refresh interval as the Presenter's own draw cadence saw
    /// it: the median interval of about a second of continuously requested
    /// frames with no state change. `None` when too few frames arrived.
    struct Refresh {
        ms: Option<f64>,
        samples: usize,
        /// p95 of the probe frames' own cost (render, layout, prepaint, paint).
        cpu_p95_ms: Option<f64>,
    }

    impl Refresh {
        fn from_intervals(mut intervals: Vec<f64>, cpu: &[f64]) -> Self {
            let samples = intervals.len();
            let cpu_p95_ms = (!cpu.is_empty()).then(|| percentile(&sorted(cpu), 95.0));
            intervals.sort_by(|a, b| a.total_cmp(b));
            let median = percentile(&intervals, 50.0);
            Self {
                ms: (samples >= PROBE_MIN_SAMPLES && median > 0.0).then_some(median),
                samples,
                cpu_p95_ms,
            }
        }

        fn hz(&self) -> Option<f64> {
            self.ms.map(|ms| 1000.0 / ms)
        }
    }

    /// `value` over the refresh interval, as JSON (`null` without a refresh).
    fn over_refresh(value: f64, refresh: &Refresh) -> String {
        match refresh.ms {
            Some(ms) => format!("{:.2}", value / ms),
            None => "null".to_owned(),
        }
    }

    fn animation_json(
        script: &Script,
        refresh: &Refresh,
        applicable: bool,
        capture: Capture,
        before: Counters,
        after: Counters,
        jobs_max: usize,
    ) -> String {
        let intervals: Vec<f64> = capture.frames.iter().filter_map(|f| f.interval).collect();
        let over = |limit: f64| intervals.iter().filter(|v| **v > limit).count();
        let cpu: Vec<f64> = capture.frames.iter().map(Frame::cpu).collect();
        let arrivals: Vec<&Frame> = capture.frames.iter().filter(|f| f.arrival).collect();
        let max_of = |values: &mut dyn Iterator<Item = f64>| values.fold(0.0_f64, f64::max);
        let column = |select: fn(&Frame) -> f64| -> String {
            summary(capture.frames.iter().map(select).collect())
        };
        format!(
            "{{\"name\":{},\"what\":{},\"applicable\":{applicable},\"segments\":{},\"frames\":{},\"intervals\":{},\"interval_ms\":{},\"p95_over_refresh\":{},\"cpu_p95_over_refresh\":{},\"over_16_7ms_pct\":{:.1},\"over_25ms\":{},\"dropped_1_5x_frametimer\":{},\"over_33ms\":{},\"first_frame_ms\":{},\"render_ms\":{},\"layout_ms\":{},\"prepaint_ms\":{},\"paint_ms\":{},\"cpu_ms\":{},\"store\":{{\"composed_delta\":{},\"thumb_images_delta\":{},\"full_images_delta\":{},\"jobs_in_flight_max\":{jobs_max},\"frames_with_arrival\":{},\"max_interval_with_arrival_ms\":{:.2},\"max_cpu_with_arrival_ms\":{:.2}}},\"inactive_window_frames\":{}}}",
            json_string(script.name),
            json_string(script.what),
            capture.segments,
            capture.frames.len(),
            intervals.len(),
            summary(intervals.clone()),
            over_refresh(percentile(&sorted(&intervals), 95.0), refresh),
            over_refresh(percentile(&sorted(&cpu), 95.0), refresh),
            pct(over(FRAME_MS), intervals.len()),
            over(DROP_MS),
            capture.dropped_1_5x,
            over(HITCH_MS),
            summary(capture.first_frames.clone()),
            column(|f| f.render),
            column(|f| f.layout),
            column(|f| f.prepaint),
            column(|f| f.paint),
            column(|f| f.cpu()),
            after.composed.saturating_sub(before.composed),
            after.thumb as i64 - before.thumb as i64,
            after.full as i64 - before.full as i64,
            arrivals.len(),
            max_of(&mut arrivals.iter().filter_map(|f| f.interval)),
            max_of(&mut arrivals.iter().map(|f| f.cpu())),
            capture.inactive_frames,
        )
    }

    fn sorted(values: &[f64]) -> Vec<f64> {
        let mut values = values.to_vec();
        values.sort_by(|a, b| a.total_cmp(b));
        values
    }

    /// Ask for a frame after every drawn frame for [`PROBE`] and read the
    /// refresh interval off the draw cadence. No OS display query is made.
    async fn probe_refresh(
        run: &Run,
        view: &Entity<PresenterView>,
        cx: &mut AsyncWindowContext,
    ) -> Result<Refresh, String> {
        let composed = counters(view, cx).composed;
        let _ = run.pacing.take();
        run.pacing.set_probing(true);
        run.pacing.begin_segment(composed);
        view.update(cx, |_, cx| cx.notify());
        cx.background_executor().timer(PROBE).await;
        run.pacing.end_segment();
        run.pacing.set_probing(false);
        let capture = run.pacing.take();
        let cpu: Vec<f64> = capture.frames.iter().map(Frame::cpu).collect();
        Ok(Refresh::from_intervals(
            capture.frames.iter().filter_map(|f| f.interval).collect(),
            &cpu,
        ))
    }

    fn gone<E: std::fmt::Display>(error: E) -> String {
        format!("the Presenter is gone: {error}")
    }

    async fn drive(
        run: &Run,
        view: &Entity<PresenterView>,
        cx: &mut AsyncWindowContext,
    ) -> Result<String, String> {
        let cue_view = if run.config.list_view {
            CueView::List
        } else {
            CueView::Grid
        };
        let (cues, groups, viewport, scale_factor, active, maximized, bounds) = view
            .update_in(cx, |view, window, cx| {
                view.set_cue_view(cue_view, cx);
                view.set_rail_choice(RailChoice::Pinned, cx);
                let snapshot = view.engine().snapshot();
                let viewport = window.viewport_size();
                (
                    snapshot.cue_count(),
                    snapshot.item_count(),
                    (f32::from(viewport.width), f32::from(viewport.height)),
                    window.scale_factor(),
                    window.is_window_active(),
                    window.is_maximized(),
                    format!("{:?}", window.window_bounds()),
                )
            })
            .map_err(gone)?;
        // A measurement is judged at the size it asked for. A Windows setting
        // that opens every app maximized overrides `--size`; the report keeps
        // `window_maximized` so such a run can be told apart.
        if maximized {
            eprintln!(
                "warning: the lab window opened maximized, so --size was not honoured;                  turn off opening apps maximized and measure again"
            );
        }
        let background = cx.background_executor().clone();
        settle(view, cx).await;
        // A few idle frames first, so the first animation is not the first draw.
        background.timer(Duration::from_millis(500)).await;

        let refresh = probe_refresh(run, view, cx).await?;
        background.timer(Duration::from_millis(300)).await;

        let mut animations = Vec::new();
        for script in scripts(groups) {
            // Each animation starts from a settled store at the first Cue.
            view.update_in(cx, |view, window, cx| {
                view.close_rail(window, cx);
                view.set_rail_choice(
                    if script.name == "rail_overlay" {
                        RailChoice::Unpinned
                    } else {
                        RailChoice::Pinned
                    },
                    cx,
                );
                if script.name != "output_fades" && !script.fold_all {
                    view.command(Command::First, cx).ok();
                }
                if script.fold_all {
                    for group in 0..groups {
                        if !view.group_collapsed(group) {
                            view.toggle_group(group, cx);
                        }
                    }
                }
            })
            .map_err(gone)?;
            background.timer(Duration::from_millis(600)).await;
            settle(view, cx).await;
            background.timer(Duration::from_millis(300)).await;

            let before = counters(view, cx);
            let mut jobs_max = 0;
            let mut applicable = script.name != "rail_overlay";
            let _ = run.pacing.take();
            for step in &script.steps {
                run.pacing.begin_segment(counters(view, cx).composed);
                let opened = view
                    .update_in(cx, |view, window, cx| match *step {
                        Step::Command(command) => {
                            view.command(command, cx).ok();
                            false
                        }
                        Step::OpenRail => {
                            view.open_rail(window, cx);
                            true
                        }
                        Step::CloseRail => {
                            view.close_rail(window, cx);
                            false
                        }
                        Step::ToggleGroup(group) => {
                            view.toggle_group(group, cx);
                            false
                        }
                    })
                    .map_err(gone)?;
                let until = Instant::now() + script.hold;
                while Instant::now() < until {
                    background.timer(POLL).await;
                    jobs_max = jobs_max.max(counters(view, cx).jobs);
                }
                if opened && view.read_with(cx, |view, _| view.rail_open()) {
                    applicable = true;
                }
                run.pacing.end_segment();
                background.timer(GAP).await;
            }
            let after = counters(view, cx);
            animations.push(animation_json(
                &script,
                &refresh,
                applicable,
                run.pacing.take(),
                before,
                after,
                jobs_max,
            ));
        }

        Ok(format!(
            "{{\"mode\":\"animation-pacing\",\"issue\":111,\"profile\":{},\"window\":{},\"viewport\":{},\"scale_percent\":{},\"scale_factor\":{scale_factor},\"cue_view\":{},\"cues_requested\":{},\"cue_count\":{cues},\"groups\":{groups},\"window_active_at_start\":{active},\"window_maximized\":{maximized},\"window_bounds\":{},\"limits_ms\":{{\"frame\":{FRAME_MS},\"dropped\":{DROP_MS},\"hitch\":{HITCH_MS},\"refresh\":{},\"p95_bar\":{}}},\"refresh_ms\":{},\"refresh_hz\":{},\"refresh_cpu_p95_ms\":{},\"refresh_samples\":{},\"refresh_note\":{},\"note\":{},\"animations\":[{}]}}",
            json_string(if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }),
            json_string(&format!("{}x{}", run.config.width, run.config.height)),
            json_string(&format!("{}x{}", viewport.0, viewport.1)),
            run.config.scale_percent,
            json_string(if run.config.list_view { "list" } else { "grid" }),
            run.config.cues_requested,
            json_string(&bounds),
            number(refresh.ms, 3),
            number(refresh.ms.map(|ms| P95_BAR * ms), 2),
            number(refresh.ms, 3),
            number(refresh.hz(), 1),
            number(refresh.cpu_p95_ms, 3),
            refresh.samples,
            json_string(&if refresh.ms.is_some() {
                format!(
                    "the median interval of {} frames the Presenter drew back to back for about a second with no state change; equals the display refresh only if an idle frame fits in one vsync; the p95 bar is {P95_BAR} times it",
                    refresh.samples
                )
            } else {
                format!(
                    "refresh not measured: the probe drew {} intervals, fewer than {PROBE_MIN_SAMPLES}",
                    refresh.samples
                )
            }),
            json_string(
                "intervals are between paint stamps of consecutive drawn frames inside one scripted press; first_frame_ms is press to first drawn frame"
            ),
            animations.join(","),
        ))
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn too_few_probe_intervals_give_no_refresh_but_keep_the_count() {
            let refresh = Refresh::from_intervals(vec![16.7; 19], &[2.0; 19]);
            assert_eq!(refresh.ms, None);
            assert_eq!(refresh.samples, 19);
            assert_eq!(refresh.cpu_p95_ms, Some(2.0));
        }

        #[test]
        fn the_refresh_is_the_median_not_the_mean() {
            let mut intervals = vec![8.0; 30];
            intervals.extend([40.0; 9]);
            let refresh = Refresh::from_intervals(intervals, &[]);
            assert_eq!(refresh.ms, Some(8.0));
            assert_eq!(refresh.samples, 39);
            assert_eq!(refresh.cpu_p95_ms, None);
            assert_eq!(refresh.hz(), Some(125.0));
        }
    }
}

/// The church-kit measurement modes: `--display-inventory`,
/// `--measure-presenter` and `--measure-output`. Each is one fresh process that
/// writes one JSON document and quits, reads and writes no preferences, and
/// carries `ok`, the commit, the host and the displays. A failure or a timeout
/// writes `{"ok":false,"error":...}` and the process exits 1.
///
/// The latency method is the one `performance::live` uses: stamp just before
/// `Window::dispatch_keystroke`, stamp each surface's paint with a `canvas`
/// (through `PresenterView::set_paint_probe`, which the labs alone set), settle
/// with `on_next_frame`, and send one command per settled frame. Unlike
/// `live::drive`, the keystroke is sent from a task between frames, so it
/// arrives at an arbitrary point of the frame, as a real key does, and the
/// latency includes the wait for the next frame.
///
/// Ownership and shutdown: `main` arms the timeout before any window opens; the
/// root owns the driver task; `finish` is the only exit and, for the output
/// mode, first takes the output down (Program black, then Disable Output).
mod kit {
    use super::*;
    use gpui::Keystroke;
    use ordo::{
        performance::{
            hot_path,
            latency::{
                CommandKind, KeydownStamp, LatencyBook, LatencyHistogram, PendingKeydown,
                Surface as LatencySurface,
            },
            memory::json_string,
            report::RendererInfo,
        },
        presenter::{PaintProbe, PaintedSurface},
    };
    use platform::display::{
        DisplayAdapter as _, DisplayId, Recognition, Topology,
        gpui_adapter::{
            DisplayResolution, GpuiDisplayAdapter, display_containing_window, topology_with_names,
        },
        monitor_identity, recognize,
    };
    use presentation::state::LiveState;
    use std::collections::{BTreeMap, HashMap};

    /// The seed of every stream, so two runs send the same keys.
    pub const SEED: u64 = 0x0D0_C0DE;
    /// `--commands` default.
    pub const DEFAULT_COMMANDS: usize = 2000;
    /// Most `--commands` accepts.
    pub const MAX_COMMANDS: usize = 200_000;
    /// Unrecorded commands first, so the first frames are not the sample.
    const WARMUP: usize = 30;
    /// The inventory must finish within this.
    const INVENTORY_TIMEOUT: Duration = Duration::from_secs(10);
    /// The output run normally takes about half a minute; this only stops a hang.
    const OUTPUT_TIMEOUT: Duration = Duration::from_secs(90);
    /// A Presenter run: a base plus time per command.
    const PRESENTER_BASE: Duration = Duration::from_secs(60);
    const PRESENTER_PER_COMMAND_MS: u64 = 60;
    /// Longest a command waits for the operator surfaces to paint and settle.
    const PAINT_DEADLINE: Duration = Duration::from_millis(1500);
    /// Longest a command waits for the congregation window to paint.
    const CONGREGATION_DEADLINE: Duration = Duration::from_millis(400);
    /// The 500 ms fades of Take and Blackout end well inside this.
    const FADE_GAP: Duration = Duration::from_millis(750);
    const POLL: Duration = Duration::from_millis(1);
    /// This many unhandled keys in a row mean the keys never reach the Presenter.
    const UNHANDLED_LIMIT: u64 = 20;
    const NAVIGATION_STEPS: usize = 200;
    /// How long the congregation window has to show the Program frame.
    const IDENTITY_DEADLINE: Duration = Duration::from_millis(300);

    const MODE_INVENTORY: &str = "display-inventory";
    const MODE_PRESENTER: &str = "presenter-latency";
    const MODE_OUTPUT: &str = "output-latency";
    pub const MODE_DRILL: &str = "recovery-drill";
    /// The drill is about 70 lines, each waiting at most [`PAINT_DEADLINE`]
    /// (normally two frames); this only stops a hang.
    const DRILL_TIMEOUT: Duration = Duration::from_secs(150);
    /// Time for the effects of a tick line (a timer, a redraw) to settle.
    const DRILL_TICK_GAP: Duration = Duration::from_millis(50);
    /// Mismatches the document quotes in full.
    const DRILL_NOTES: usize = 5;

    // ---------------------------------------------------------------- report

    fn commit() -> String {
        if let Some(commit) = option_env!("ORDO_GIT_COMMIT") {
            return commit.to_owned();
        }
        // A kit can carry the commit beside the exe when it was built without it.
        std::env::current_exe()
            .ok()
            .and_then(|exe| std::fs::read_to_string(exe.with_file_name("commit.txt")).ok())
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    }

    fn text_or_null(value: Option<String>) -> String {
        value.map_or_else(|| "null".to_owned(), |text| json_string(&text))
    }

    fn host_json() -> String {
        format!(
            "{{\"name\":{},\"os\":{},\"os_version\":{},\"arch\":{}}}",
            text_or_null(
                sysinfo::System::host_name().or_else(|| std::env::var("COMPUTERNAME").ok())
            ),
            json_string(std::env::consts::OS),
            text_or_null(sysinfo::System::long_os_version()),
            json_string(std::env::consts::ARCH),
        )
    }

    /// The fields every document starts with, after `{`.
    fn header(mode: &str, ok: bool) -> String {
        format!(
            "\"ok\":{ok},\"mode\":{},\"commit\":{},\"build_identity\":{},\"profile\":{},\"host\":{}",
            json_string(mode),
            json_string(&commit()),
            json_string(ordo::BUILD_IDENTITY),
            json_string(if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }),
            host_json(),
        )
    }

    pub fn error_doc(mode: &str, message: &str) -> String {
        format!(
            "{{{},\"error\":{}}}",
            header(mode, false),
            json_string(message)
        )
    }

    fn number(value: f64) -> String {
        if value.is_finite() {
            format!("{value:.3}")
        } else {
            "null".to_owned()
        }
    }

    fn float(value: f32) -> String {
        number(f64::from(value))
    }

    fn optional_ms(value: Option<Duration>) -> String {
        value.map_or_else(|| "null".to_owned(), |v| number(v.as_secs_f64() * 1000.0))
    }

    /// `"samples":n,"p50_ms":..,"p95_ms":..,"p99_ms":..,"max_ms":..`
    fn tail_fields(histogram: Option<&LatencyHistogram>) -> String {
        let (samples, [p50, p95, p99, max]) = histogram.map_or((0, [None; 4]), |h| {
            let tail = h.percentiles(&[50.0, 95.0, 99.0, 100.0]);
            (h.count(), [tail[0], tail[1], tail[2], tail[3]])
        });
        format!(
            "\"samples\":{samples},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{},\"max_ms\":{}",
            optional_ms(p50),
            optional_ms(p95),
            optional_ms(p99),
            optional_ms(max),
        )
    }

    fn renderer_json(window: &Window) -> String {
        let info = RendererInfo::from_gpu_specs(window.gpu_specs());
        format!(
            "{{\"software_emulated\":{},\"device_name\":{}}}",
            info.software_emulated
                .map_or("null".to_owned(), |v| v.to_string()),
            text_or_null(info.device_name),
        )
    }

    fn rect_json(b: gpui::Bounds<gpui::Pixels>) -> String {
        format!(
            "{{\"x\":{},\"y\":{},\"width\":{},\"height\":{}}}",
            float(b.origin.x.as_f32()),
            float(b.origin.y.as_f32()),
            float(b.size.width.as_f32()),
            float(b.size.height.as_f32()),
        )
    }

    /// One display of the report. GPUI exposes neither a refresh rate nor a
    /// scale per display, so those are `null`; the physical size is filled in
    /// only for the display a window of known scale is on.
    fn display_json(
        display: &Rc<dyn gpui::PlatformDisplay>,
        primary: Option<gpui::DisplayId>,
        operator: Option<gpui::DisplayId>,
        names: &HashMap<String, String>,
        scale_known: Option<f32>,
    ) -> String {
        let id = display.id();
        let uuid = display.uuid().ok().map(|uuid| uuid.to_string());
        let name = uuid.as_ref().and_then(|uuid| names.get(uuid));
        let bounds = display.bounds();
        let physical = |value: f32| scale_known.map_or("null".to_owned(), |s| float(value * s));
        format!(
            "{{\"gpui_id\":{},\"uuid\":{},\"bounds\":{},\"visible_bounds\":{},\"primary\":{},\"operator\":{},\"scale_factor\":{},\"physical_width\":{},\"physical_height\":{},\"refresh_hz\":null,\"monitor_name\":{},\"generic_name\":{}}}",
            u64::from(id),
            text_or_null(uuid),
            rect_json(bounds),
            rect_json(display.visible_bounds()),
            primary == Some(id),
            operator == Some(id),
            scale_known.map_or("null".to_owned(), float),
            physical(bounds.size.width.as_f32()),
            physical(bounds.size.height.as_f32()),
            text_or_null(name.cloned()),
            name.is_none(),
        )
    }

    fn displays_json(
        cx: &App,
        operator: Option<gpui::DisplayId>,
        names: &HashMap<String, String>,
        operator_scale: Option<f32>,
    ) -> String {
        let primary = cx.primary_display().map(|display| display.id());
        let items: Vec<String> = cx
            .displays()
            .iter()
            .map(|display| {
                let scale = if operator == Some(display.id()) {
                    operator_scale
                } else {
                    None
                };
                display_json(display, primary, operator, names, scale)
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    fn names_json(names: &HashMap<String, String>) -> String {
        let sorted: BTreeMap<_, _> = names.iter().collect();
        let items: Vec<String> = sorted
            .into_iter()
            .map(|(uuid, name)| format!("{}:{}", json_string(uuid), json_string(name)))
            .collect();
        format!("{{{}}}", items.join(","))
    }

    // ------------------------------------------------------------- finishing

    type Cleanup = Rc<dyn Fn(&mut App)>;

    thread_local! {
        /// Takes the output down; set by the output mode, run by every exit.
        static CLEANUP: RefCell<Option<Cleanup>> = const { RefCell::new(None) };
    }

    /// Writes the document and quits. For the output mode the output is taken
    /// down first, so no exit leaves the projector lit.
    pub fn finish(cx: &mut App, out: &Path, document: &str, ok: bool) {
        let cleanup = CLEANUP.with(|cleanup| cleanup.borrow().clone());
        if let Some(cleanup) = cleanup {
            cleanup(cx);
        }
        measure::finish(cx, out, document, ok);
    }

    fn arm_timeout(cx: &mut App, out: PathBuf, mode: &'static str, limit: Duration) {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(limit).await;
            cx.update(|cx| {
                let message = format!("timed out after {} s", limit.as_secs());
                finish(cx, &out, &error_doc(mode, &message), false);
            });
        })
        .detach();
    }

    /// A failure before the app loop starts: writes the document and exits 1.
    pub fn fail_early(out: &Path, mode: &str, message: &str) -> ! {
        eprintln!("live-control-lab: {message}");
        if let Err(error) = std::fs::write(out, error_doc(mode, message)) {
            eprintln!("live-control-lab: cannot write {}: {error}", out.display());
        }
        std::process::exit(1);
    }

    /// Ends the run when the last window is closed by someone else.
    fn on_all_windows_closed(cx: &mut App, out: PathBuf, mode: &'static str) {
        cx.on_window_closed(move |cx, _| {
            if cx.windows().is_empty() {
                finish(cx, &out, &error_doc(mode, "the window was closed"), false);
            }
        })
        .detach();
    }

    // ------------------------------------------------------------- fixture

    /// The snapshot a run opens: `--snapshot`, else the first existing of the
    /// candidates (`<exe dir>/fixtures/normal-mass`, then the compile-time path).
    pub fn pick_snapshot(
        given: Option<&Path>,
        candidates: &[PathBuf],
        fixture: &str,
    ) -> Result<PathBuf, String> {
        if let Some(given) = given {
            return if given.is_dir() {
                Ok(given.to_owned())
            } else {
                Err(format!(
                    "the snapshot folder {} does not exist",
                    given.display()
                ))
            };
        }
        candidates
            .iter()
            .find(|path| path.is_dir())
            .cloned()
            .ok_or_else(|| {
                let tried: Vec<String> =
                    candidates.iter().map(|p| p.display().to_string()).collect();
                format!(
                    "no {fixture} fixture found (tried {}); pass --snapshot <dir>",
                    tried.join(", ")
                )
            })
    }

    /// `fixture` is the committed fixture the mode opens: `normal-mass`, or
    /// `recovery-drill` for `--run-drill`.
    pub fn resolve_snapshot(args: &Args, fixture: &str) -> Result<PathBuf, String> {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            candidates.push(exe.with_file_name("fixtures").join(fixture));
        }
        candidates.push(if fixture == "normal-mass" {
            default_snapshot()
        } else {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/presentations")
                .join(fixture)
        });
        pick_snapshot(
            args.snapshot_given.then_some(args.snapshot.as_path()),
            &candidates,
            fixture,
        )
    }

    // ------------------------------------------------------------- inventory

    /// `--display-inventory`: every display GPUI reports, with its monitor name.
    /// It opens no window: GPUI enumerates displays from the application.
    pub fn inventory(cx: &mut App, out: PathBuf) {
        arm_timeout(cx, out.clone(), MODE_INVENTORY, INVENTORY_TIMEOUT);
        let names = monitor_identity::read();
        let primary = cx.primary_display().map(|display| display.id());
        let document = format!(
            "{{{},\"schema\":\"ordo.lab.display-inventory.v1\",\"display_count\":{},\"operator_note\":\"no window is open, so the primary display stands for the operator display\",\"name_note\":\"monitor_identity::read omits blank and Generic PnP names, so generic_name true means no usable name\",\"displays\":{},\"monitor_identity\":{}}}",
            header(MODE_INVENTORY, true),
            cx.displays().len(),
            displays_json(cx, primary, &names, None),
            names_json(&names),
        );
        finish(cx, &out, &document, true);
    }

    // ------------------------------------------------------------- streams

    /// A tiny seeded generator (splitmix64).
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) % bound.max(1)
        }
    }

    /// The Presenter keys the streams send.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Key {
        Right,
        Left,
        Down,
        Up,
        PageDown,
        PageUp,
        Home,
        End,
        Space,
        F,
        B,
    }

    impl Key {
        pub const fn name(self) -> &'static str {
            match self {
                Self::Right => "right",
                Self::Left => "left",
                Self::Down => "down",
                Self::Up => "up",
                Self::PageDown => "pagedown",
                Self::PageUp => "pageup",
                Self::Home => "home",
                Self::End => "end",
                Self::Space => "space",
                Self::F => "f",
                Self::B => "b",
            }
        }

        pub const fn kind(self) -> CommandKind {
            match self {
                Self::Space => CommandKind::Space,
                Self::F => CommandKind::Freeze,
                Self::B => CommandKind::Blackout,
                _ => CommandKind::Navigation,
            }
        }
    }

    /// `n` keys from `seed`: about two thirds navigation, the rest Space, F, B.
    pub fn mixed_stream(seed: u64, n: usize) -> Vec<Key> {
        let mut rng = Rng(seed);
        (0..n)
            .map(|_| match rng.below(100) {
                0..=21 => Key::Right,
                22..=30 => Key::Down,
                31..=42 => Key::Left,
                43..=47 => Key::Up,
                48..=54 => Key::PageDown,
                55..=59 => Key::PageUp,
                60..=62 => Key::Home,
                63..=65 => Key::End,
                66..=79 => Key::Space,
                80..=89 => Key::F,
                _ => Key::B,
            })
            .collect()
    }

    /// `n` navigation keys from `seed`, leaning forward.
    pub fn navigation_stream(seed: u64, n: usize) -> Vec<Key> {
        let mut rng = Rng(seed);
        (0..n)
            .map(|_| match rng.below(100) {
                0..=39 => Key::Right,
                40..=54 => Key::Down,
                55..=69 => Key::Left,
                70..=76 => Key::Up,
                77..=85 => Key::PageDown,
                86..=92 => Key::PageUp,
                93..=96 => Key::Home,
                _ => Key::End,
            })
            .collect()
    }

    // ---------------------------------------------------------------- board

    const SURFACES: [PaintedSurface; 3] = [
        PaintedSurface::Preview,
        PaintedSurface::Program,
        PaintedSurface::Congregation,
    ];

    fn slot(surface: PaintedSurface) -> usize {
        match surface {
            PaintedSurface::Preview => 0,
            PaintedSurface::Program => 1,
            PaintedSurface::Congregation => 2,
        }
    }

    fn surface_name(surface: PaintedSurface) -> &'static str {
        match surface {
            PaintedSurface::Preview => "preview",
            PaintedSurface::Program => "program",
            PaintedSurface::Congregation => "congregation",
        }
    }

    type FrameRef = Option<Arc<rendering::Frame>>;

    /// Frames are the same when both are black or both are the same `Arc`.
    fn same_frame(a: &FrameRef, b: &FrameRef) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }

    /// One command in flight, from keydown until it settles.
    struct Open {
        seq: u64,
        stamp: KeydownStamp,
        scope: hot_path::HotPathScope,
        painted: [Option<Instant>; 3],
        /// A frame after the one that painted Preview and Program has begun.
        settled: bool,
        armed: bool,
    }

    /// What the paint stamps write and the driver reads. GPUI thread only.
    #[derive(Default)]
    pub struct Board {
        seq: u64,
        open: Option<Open>,
        /// The frame each surface drew last.
        last: [FrameRef; 3],
        congregation_paints: u64,
        /// Congregation paints whose frame was not the last Program frame.
        congregation_off_program: u64,
        /// The first congregation paint: when, and whether it was black.
        congregation_first: Option<(Instant, bool)>,
    }

    impl Board {
        fn stamp(
            board: &Rc<RefCell<Self>>,
            surface: PaintedSurface,
            frame: Option<&Arc<rendering::Frame>>,
            window: &Window,
        ) {
            let now = Instant::now();
            let mut b = board.borrow_mut();
            let index = slot(surface);
            b.last[index] = frame.cloned();
            if surface == PaintedSurface::Congregation {
                b.congregation_paints += 1;
                if b.congregation_first.is_none() {
                    b.congregation_first = Some((now, frame.is_none()));
                }
                if !same_frame(&b.last[1], &b.last[2]) {
                    b.congregation_off_program += 1;
                }
            }
            let Some(open) = b.open.as_mut() else {
                return;
            };
            // A congregation paint answers the command only once the Program
            // monitor has painted it: the Presenter's render (which hands the
            // window its frame) and that paint are one draw, so a congregation
            // paint after it shows the new state, and one before it does not.
            let answers = surface != PaintedSurface::Congregation || open.painted[1].is_some();
            if answers && open.painted[index].is_none() {
                open.painted[index] = Some(now);
            }
            let operator_done = open.painted[0].is_some() && open.painted[1].is_some();
            if surface != PaintedSurface::Congregation && operator_done && !open.armed {
                open.armed = true;
                let (board, seq) = (board.clone(), open.seq);
                // Registered during a paint, so it runs at the start of the
                // next frame, after the painted one was presented.
                window.on_next_frame(move |_, _| {
                    if let Some(open) = board.borrow_mut().open.as_mut()
                        && open.seq == seq
                    {
                        open.settled = true;
                    }
                });
            }
        }
    }

    fn probe(board: &Rc<RefCell<Board>>) -> PaintProbe {
        let board = board.clone();
        Rc::new(move |surface, frame, window| Board::stamp(&board, surface, frame, window))
    }

    // ---------------------------------------------------------------- stats

    #[derive(Default)]
    struct Stats {
        /// Keydown to Preview and Program paint, by kind.
        book: LatencyBook,
        /// Keydown to congregation paint, by kind.
        congregation: BTreeMap<CommandKind, LatencyHistogram>,
        /// Keydown to the later of the Preview and Program paints, every kind.
        operator: LatencyHistogram,
        by_key: BTreeMap<&'static str, [LatencyHistogram; 3]>,
        sent: BTreeMap<&'static str, u64>,
        unpainted: [u64; 3],
        unhandled: u64,
        checks: u64,
        /// Checks where the congregation window was behind at first.
        lagged: u64,
        mismatches: u64,
        mismatch_notes: Vec<String>,
    }

    impl Stats {
        fn latency_json(&self) -> String {
            let mut items = Vec::new();
            for kind in CommandKind::ALL {
                for (surface, name) in [
                    (LatencySurface::Preview, "preview"),
                    (LatencySurface::Program, "program"),
                ] {
                    items.push(format!(
                        "{{\"command\":{},\"surface\":{},{}}}",
                        json_string(kind.as_str()),
                        json_string(name),
                        tail_fields(self.book.histogram(kind, surface)),
                    ));
                }
                if !self.congregation.is_empty() {
                    items.push(format!(
                        "{{\"command\":{},\"surface\":\"congregation\",{}}}",
                        json_string(kind.as_str()),
                        tail_fields(self.congregation.get(&kind)),
                    ));
                }
            }
            format!("[{}]", items.join(","))
        }

        fn by_key_json(&self) -> String {
            let items: Vec<String> = self
                .by_key
                .iter()
                .map(|(key, [preview, program, congregation])| {
                    let congregation = if congregation.count() > 0 {
                        format!(",\"congregation\":{{{}}}", tail_fields(Some(congregation)))
                    } else {
                        String::new()
                    };
                    format!(
                        "{}:{{\"sent\":{},\"preview\":{{{}}},\"program\":{{{}}}{congregation}}}",
                        json_string(key),
                        self.sent.get(key).copied().unwrap_or(0),
                        tail_fields(Some(preview)),
                        tail_fields(Some(program)),
                    )
                })
                .collect();
            format!("{{{}}}", items.join(","))
        }

        fn unpainted_json(&self) -> String {
            let items: Vec<String> = SURFACES
                .iter()
                .map(|s| format!("\"{}\":{}", surface_name(*s), self.unpainted[slot(*s)]))
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }

    fn hot_path_json() -> String {
        let violations = hot_path::take_violations();
        let total: u64 = violations.iter().map(|(_, count)| count).sum();
        let kinds: Vec<String> = violations
            .iter()
            .map(|(kind, count)| format!("\"{}\":{count}", kind.as_str()))
            .collect();
        format!("{{\"total\":{total},\"by_kind\":{{{}}}}}", kinds.join(","))
    }

    /// Whether a command should wait for the congregation window to paint.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Wait {
        /// No congregation window exists.
        Never,
        /// Wait only when the Program frame is not the one it last drew.
        IfFrameChanged,
        /// Always wait: the command fades Program or changes its opacity.
        Always,
    }

    fn gone<E: std::fmt::Display>(error: E) -> String {
        format!("the Presenter is gone: {error}")
    }

    /// Sends `key` through the real bindings and records its latency. The
    /// caller applies any gap after it.
    async fn send_key(
        key: Key,
        wait: Wait,
        record: bool,
        board: &Rc<RefCell<Board>>,
        stats: &mut Stats,
        cx: &mut AsyncWindowContext,
    ) -> Result<(), String> {
        let keystroke = Keystroke::parse(key.name()).map_err(|e| format!("bad key: {e}"))?;
        send_input(
            (key.name(), key.kind()),
            |window, app| window.dispatch_keystroke(keystroke, app),
            wait,
            record,
            board,
            stats,
            cx,
        )
        .await
    }

    /// Sends one input and records its latency under `(name, kind)`: `dispatch`
    /// delivers it (a keystroke, or a command straight to the Presenter) and
    /// says whether it was handled. The caller applies any gap after it.
    async fn send_input(
        (name, kind): (&'static str, CommandKind),
        dispatch: impl FnOnce(&mut Window, &mut App) -> bool,
        wait: Wait,
        record: bool,
        board: &Rc<RefCell<Board>>,
        stats: &mut Stats,
        cx: &mut AsyncWindowContext,
    ) -> Result<(), String> {
        let background = cx.background_executor().clone();
        let seq = {
            let mut b = board.borrow_mut();
            b.seq += 1;
            b.seq
        };
        let handled = cx
            .update(|window, app| {
                let scope = hot_path::enter();
                // Immediately before the dispatch, as `live::drive` does.
                let stamp = PendingKeydown::start(kind);
                board.borrow_mut().open = Some(Open {
                    seq,
                    stamp,
                    scope,
                    painted: [None; 3],
                    settled: false,
                    armed: false,
                });
                dispatch(window, app)
            })
            .map_err(gone)?;
        if !handled {
            if let Some(open) = board.borrow_mut().open.take() {
                open.scope.close();
            }
            stats.unhandled += 1;
            return if stats.unhandled >= UNHANDLED_LIMIT {
                Err(format!(
                    "{UNHANDLED_LIMIT} keys were not handled; the Presenter lost key focus"
                ))
            } else {
                Ok(())
            };
        }
        let started = Instant::now();
        let settled = loop {
            background.timer(POLL).await;
            let b = board.borrow();
            let open = b.open.as_ref().ok_or("the command vanished")?;
            if open.settled {
                break true;
            }
            if started.elapsed() > PAINT_DEADLINE {
                break false;
            }
        };
        let waits = match wait {
            Wait::Never => false,
            Wait::Always => true,
            Wait::IfFrameChanged => {
                let b = board.borrow();
                !same_frame(&b.last[1], &b.last[2])
            }
        };
        if waits {
            let started = Instant::now();
            while board
                .borrow()
                .open
                .as_ref()
                .is_some_and(|open| open.painted[2].is_none())
                && started.elapsed() < CONGREGATION_DEADLINE
            {
                background.timer(POLL).await;
            }
        }
        let open = board
            .borrow_mut()
            .open
            .take()
            .ok_or("the command vanished")?;
        let Open {
            stamp,
            scope,
            painted,
            ..
        } = open;
        scope.close();
        if !record {
            return Ok(());
        }
        let kind = stamp.kind();
        if let (Some(preview), Some(program)) = (painted[0], painted[1]) {
            stats
                .operator
                .record(preview.max(program).saturating_duration_since(stamp.at()));
        }
        *stats.sent.entry(name).or_default() += 1;
        let per_key = stats.by_key.entry(name).or_default();
        for surface in SURFACES {
            let index = slot(surface);
            match painted[index] {
                Some(at) => {
                    let elapsed = at.saturating_duration_since(stamp.at());
                    per_key[index].record(elapsed);
                    match surface {
                        PaintedSurface::Preview => {
                            stats.book.record(kind, LatencySurface::Preview, elapsed);
                        }
                        PaintedSurface::Program => {
                            stats.book.record(kind, LatencySurface::Program, elapsed);
                        }
                        PaintedSurface::Congregation => {
                            stats.congregation.entry(kind).or_default().record(elapsed);
                        }
                    }
                }
                None if surface != PaintedSurface::Congregation || waits => {
                    stats.unpainted[index] += 1;
                }
                None => {}
            }
        }
        let _ = settled;
        Ok(())
    }

    /// Compares the last Program frame with the last frame the congregation
    /// window drew, as `Arc` pointers. The two windows draw on their own
    /// frames, so the congregation window may be one frame behind; it is given
    /// [`IDENTITY_DEADLINE`] to catch up before a difference counts.
    async fn check_identity(
        label: &str,
        board: &Rc<RefCell<Board>>,
        stats: &mut Stats,
        cx: &AsyncWindowContext,
    ) {
        let background = cx.background_executor().clone();
        let started = Instant::now();
        let matches = |board: &Rc<RefCell<Board>>| {
            let b = board.borrow();
            same_frame(&b.last[1], &b.last[2])
        };
        let mut caught_up = matches(board);
        if !caught_up {
            stats.lagged += 1;
        }
        while !caught_up && started.elapsed() < IDENTITY_DEADLINE {
            background.timer(POLL).await;
            caught_up = matches(board);
        }
        stats.checks += 1;
        if !caught_up {
            stats.mismatches += 1;
            if stats.mismatch_notes.len() < 10 {
                stats.mismatch_notes.push(format!("after {label}"));
            }
        }
    }

    /// Waits until the frame store has had no job in flight for a few checks.
    async fn settle_frames(view: &Entity<PresenterView>, cx: &mut AsyncWindowContext) {
        let background = cx.background_executor().clone();
        let started = Instant::now();
        let mut quiet = 0;
        while quiet < 4 && started.elapsed() < Duration::from_secs(10) {
            background.timer(Duration::from_millis(50)).await;
            let idle = view.read_with(cx, |view, _| {
                view.frames().jobs_in_flight() == 0 && view.frames().upgrading() == 0
            });
            quiet = if idle { quiet + 1 } else { 0 };
        }
    }

    // ------------------------------------------------------------------ run

    pub enum Kind {
        Presenter {
            commands: usize,
        },
        Output,
        /// The Recovery Drill replay (`--run-drill`).
        Drill,
    }

    /// What the command line asked for, and the board the stamps write.
    pub struct Run {
        pub out: PathBuf,
        pub kind: Kind,
        pub snapshot: PathBuf,
        pub board: Rc<RefCell<Board>>,
        /// The Presenter's clock in the drill, stepped by the driver.
        pub clock: Rc<Cell<u64>>,
        /// The memory peak of the drill, started before the window opened.
        pub watch: Option<MemoryWatch>,
        pub width: f32,
        pub height: f32,
        pub scale_percent: u32,
    }

    impl Run {
        pub fn mode(&self) -> &'static str {
            match self.kind {
                Kind::Presenter { .. } => MODE_PRESENTER,
                Kind::Output => MODE_OUTPUT,
                Kind::Drill => MODE_DRILL,
            }
        }

        /// The stepped clock the Presenter must read, for the drill only.
        pub fn clock(&self) -> Option<Rc<Cell<u64>>> {
            matches!(self.kind, Kind::Drill).then(|| self.clock.clone())
        }

        fn timeout(&self) -> Duration {
            match self.kind {
                Kind::Presenter { commands } => {
                    PRESENTER_BASE
                        + Duration::from_millis(PRESENTER_PER_COMMAND_MS * commands as u64)
                }
                Kind::Output => OUTPUT_TIMEOUT,
                Kind::Drill => DRILL_TIMEOUT,
            }
        }

        pub fn arm(&self, cx: &mut App) {
            arm_timeout(cx, self.out.clone(), self.mode(), self.timeout());
            on_all_windows_closed(cx, self.out.clone(), self.mode());
        }

        pub fn fail(self, message: &str, cx: &mut App) {
            if let Some(watch) = &self.watch {
                watch.finish(None);
            }
            finish(cx, &self.out, &error_doc(self.mode(), message), false);
        }

        pub fn start(
            self,
            view: Entity<PresenterView>,
            window: &mut Window,
            cx: &mut Context<LabRoot>,
        ) -> Task<()> {
            view.update(cx, |view, _| view.set_paint_probe(probe(&self.board)));
            if matches!(self.kind, Kind::Output) {
                let weak = view.downgrade();
                CLEANUP.with(|cleanup| {
                    *cleanup.borrow_mut() = Some(Rc::new(move |cx: &mut App| {
                        weak.update(cx, take_output_down).ok();
                    }));
                });
                match choose_display(window, cx) {
                    Ok(source) => {
                        view.update(cx, |view, cx| {
                            view.install_output(Box::new(source), window, cx);
                        });
                    }
                    Err(reason) => {
                        let document = skipped_doc(&reason);
                        return cx.spawn_in(window, async move |_, cx| {
                            cx.update(|_, cx| finish(cx, &self.out, &document, true))
                                .ok();
                        });
                    }
                }
            }
            cx.spawn_in(window, async move |_, cx| {
                let result = match self.kind {
                    Kind::Presenter { commands } => {
                        drive_presenter(&self, commands, &view, cx).await
                    }
                    Kind::Output => drive_output(&self, &view, cx).await,
                    Kind::Drill => drive_drill(&self, &view, cx).await,
                };
                if let Some(watch) = &self.watch {
                    watch.finish(None);
                }
                cx.update(|_, cx| match &result {
                    Ok(document) => finish(cx, &self.out, document, true),
                    Err(message) => {
                        finish(cx, &self.out, &error_doc(self.mode(), message), false);
                    }
                })
                .ok();
            })
        }
    }

    /// Program black, then the window closed, then the Engine disabled: the
    /// order `close_output_safely` uses, with the public commands.
    fn take_output_down(view: &mut PresenterView, cx: &mut Context<PresenterView>) {
        if matches!(
            view.engine().state().live(),
            LiveState::Live | LiveState::Frozen
        ) {
            view.command(Command::ToggleBlackout, cx).ok();
        }
        if view.engine().state().live() != LiveState::OutputDisabled {
            view.command(Command::DisableOutput, cx).ok();
        }
    }

    // -------------------------------------------------------------- display

    /// Shows the Presenter the operator display and the one display this run
    /// may use, so Enable Output picks it without asking.
    struct OneCongregationDisplay {
        keep: DisplayId,
    }

    impl ordo::output::DisplaySource for OneCongregationDisplay {
        fn snapshot(&self, cx: &App, operator: Option<gpui::DisplayId>) -> Topology {
            let topology = GpuiDisplayAdapter::new(cx, operator).snapshot();
            let operator_id = topology.operator();
            let kept = topology
                .displays()
                .iter()
                .filter(|display| Some(display.id) == operator_id || display.id == self.keep)
                .cloned()
                .collect();
            Topology::new(kept, operator_id)
        }

        fn reported_display(
            &self,
            window: &Window,
            requested: DisplayId,
            cx: &App,
        ) -> Option<DisplayId> {
            ordo::output::DisplaySource::reported_display(
                &ordo::output::GpuiDisplaySource,
                window,
                requested,
                cx,
            )
        }
    }

    /// The display `window` is on, from its bounds and the platform's report.
    fn operator_display(window: &Window, cx: &App) -> Option<gpui::DisplayId> {
        let reported = window.display(cx).map(|display| display.id());
        match display_containing_window(&cx.displays(), window.bounds(), reported) {
            DisplayResolution::Matched(id) => Some(id),
            DisplayResolution::NoMatch => reported,
            DisplayResolution::Ambiguous => None,
        }
    }

    /// The display source for the output run, or the reason there is none.
    fn choose_display(window: &Window, cx: &App) -> Result<OneCongregationDisplay, String> {
        let operator = operator_display(window, cx);
        let keep = cx
            .displays()
            .iter()
            .map(|display| display.id())
            .find(|id| Some(*id) != operator);
        match (operator, keep) {
            (Some(_), Some(keep)) => Ok(OneCongregationDisplay { keep: keep.into() }),
            (None, _) => Err("the operator display could not be told apart".to_owned()),
            _ => Err("no congregation display".to_owned()),
        }
    }

    pub fn skipped_doc(reason: &str) -> String {
        format!(
            "{{{},\"skipped\":{},\"note\":\"the output mode needs a second display\"}}",
            header(MODE_OUTPUT, true),
            json_string(reason)
        )
    }

    // ------------------------------------------------------------ presenter

    struct Setup {
        cues: usize,
        viewport: (f32, f32),
        scale_factor: f32,
        maximized: bool,
        bounds: String,
        renderer: String,
        operator: Option<gpui::DisplayId>,
    }

    fn setup(view: &Entity<PresenterView>, cx: &mut AsyncWindowContext) -> Result<Setup, String> {
        view.update_in(cx, |view, window, cx| {
            let viewport = window.viewport_size();
            Setup {
                cues: view.engine().snapshot().cue_count(),
                viewport: (f32::from(viewport.width), f32::from(viewport.height)),
                scale_factor: window.scale_factor(),
                maximized: window.is_maximized(),
                bounds: format!("{:?}", window.window_bounds()),
                renderer: renderer_json(window),
                operator: operator_display(window, cx),
            }
        })
        .map_err(gone)
    }

    fn window_json(setup: &Setup, run: &Run) -> String {
        format!(
            "{{\"viewport\":{},\"scale_factor\":{},\"requested\":{},\"scale_percent\":{},\"maximized\":{},\"bounds\":{}}}",
            json_string(&format!("{}x{}", setup.viewport.0, setup.viewport.1)),
            float(setup.scale_factor),
            json_string(&format!("{}x{}", run.width, run.height)),
            run.scale_percent,
            setup.maximized,
            json_string(&setup.bounds),
        )
    }

    async fn drive_presenter(
        run: &Run,
        commands: usize,
        view: &Entity<PresenterView>,
        cx: &mut AsyncWindowContext,
    ) -> Result<String, String> {
        let started = Instant::now();
        let setup = setup(view, cx)?;
        let background = cx.background_executor().clone();
        settle_frames(view, cx).await;
        background.timer(Duration::from_millis(300)).await;
        // The scripted Enable Output. No congregation display is installed, so
        // nothing opens a window and nothing can reach a projector.
        view.update(cx, |view, cx| view.command(Command::EnableOutput, cx))
            .map_err(|refusal| format!("Enable Output was refused: {refusal:?}"))?;
        background.timer(Duration::from_millis(300)).await;

        let board = run.board.clone();
        let mut stats = Stats::default();
        hot_path::take_violations();
        for key in navigation_stream(SEED ^ 1, WARMUP) {
            send_key(key, Wait::Never, false, &board, &mut stats, cx).await?;
        }
        for key in mixed_stream(SEED, commands) {
            send_key(key, Wait::Never, true, &board, &mut stats, cx).await?;
        }
        let hot_path = hot_path_json();
        let names = monitor_identity::read();
        let displays = cx
            .update(|_, app| displays_json(app, setup.operator, &names, Some(setup.scale_factor)))
            .map_err(gone)?;
        Ok(format!(
            "{{{},\"schema\":\"ordo.lab.presenter-latency.v1\",\"latency_scope\":\"keydown_to_surface_paint\",\"method\":\"seeded mixed key stream through Window::dispatch_keystroke with output enabled by command and no congregation window; keydown is stamped just before the dispatch, each surface stamps its own paint, one key per settled frame; the key arrives between frames, so the latency includes the wait for the next frame\",\"seed\":{SEED},\"commands\":{commands},\"warmup\":{WARMUP},\"cue_count\":{},\"fixture\":{},\"window\":{},\"renderer\":{},\"displays\":{displays},\"latency\":{},\"latency_by_key\":{},\"unpainted\":{},\"unhandled\":{},\"hot_path_violations\":{hot_path},\"auto_play_cadence\":null,\"auto_play_note\":\"not measured in this mode\",\"elapsed_s\":{}}}",
            header(MODE_PRESENTER, true),
            setup.cues,
            json_string(&run.snapshot.display().to_string()),
            window_json(&setup, run),
            setup.renderer,
            stats.latency_json(),
            stats.by_key_json(),
            stats.unpainted_json(),
            stats.unhandled,
            number(started.elapsed().as_secs_f64()),
        ))
    }

    // ---------------------------------------------------------------- drill

    /// A drill file read on the background executor, not the GPUI thread.
    async fn read_drill_file(path: PathBuf, cx: &AsyncWindowContext) -> Result<String, String> {
        let shown = path.display().to_string();
        cx.background_executor()
            .spawn(async move { std::fs::read_to_string(&path) })
            .await
            .map_err(|error| format!("cannot read {shown}: {error}"))
    }

    /// One process memory reading, taken on the background executor.
    async fn drill_sample(
        cx: &AsyncWindowContext,
    ) -> Result<ordo::performance::process::ProcessSample, String> {
        cx.background_executor()
            .spawn(async { ordo::performance::memory::sample_process() })
            .await
    }

    fn sample_json(at: &str, sample: &ordo::performance::process::ProcessSample) -> String {
        format!(
            "{{\"at\":{},\"rss_bytes\":{},\"private_bytes\":{}}}",
            json_string(at),
            sample.rss_bytes,
            sample
                .private_bytes
                .map_or("null".to_owned(), |bytes| bytes.to_string()),
        )
    }

    /// The latency class of a drill command; it only labels the report.
    fn drill_kind(command: Command) -> CommandKind {
        match command {
            Command::Take => CommandKind::Space,
            Command::ToggleFreeze => CommandKind::Freeze,
            Command::ToggleBlackout => CommandKind::Blackout,
            _ => CommandKind::Navigation,
        }
    }

    /// Replays `commands.txt` through the Presenter's command path, one line at
    /// a time, and compares the Engine state after each with `commands.expect`.
    ///
    /// Time is the stepped clock in `run`: 100 ms before each command, and a
    /// `tick <ms>` line moves it by `<ms>` and ticks the view. The drill's own
    /// `enable-output` line enables output; no congregation display is
    /// installed, so no output window opens. Only command lines are timed
    /// (keydown to Preview and Program paint); a tick line is not an input.
    async fn drive_drill(
        run: &Run,
        view: &Entity<PresenterView>,
        cx: &mut AsyncWindowContext,
    ) -> Result<String, String> {
        use presentation::{
            engine::Outcome,
            test_support::drill::{COMMAND_MS, Step, parse_step, script_lines, state_line},
        };
        let started = Instant::now();
        let script = read_drill_file(run.snapshot.join("commands.txt"), cx).await?;
        let expected = read_drill_file(run.snapshot.join("commands.expect"), cx).await?;
        let lines: Vec<&str> = script_lines(&script).collect();
        let wanted: Vec<&str> = expected.lines().collect();
        if lines.len() != wanted.len() {
            return Err(format!(
                "commands.txt has {} lines and commands.expect has {}",
                lines.len(),
                wanted.len()
            ));
        }
        let steps = lines
            .iter()
            .map(|line| parse_step(line).ok_or_else(|| format!("unknown drill line {line:?}")))
            .collect::<Result<Vec<_>, _>>()?;

        let setup = setup(view, cx)?;
        let background = cx.background_executor().clone();
        settle_frames(view, cx).await;
        background.timer(Duration::from_millis(300)).await;
        let board = run.board.clone();
        let mut stats = Stats::default();
        hot_path::take_violations();
        let mut memory = vec![sample_json("start", &drill_sample(cx).await?)];
        let middle = steps.len() / 2;
        let mut clock = 0u64;
        run.clock.set(clock);

        for (index, (text, step)) in lines.iter().zip(&steps).enumerate() {
            let outcome = match *step {
                Step::Command(command) => {
                    clock += COMMAND_MS;
                    run.clock.set(clock);
                    let result = Cell::new(None);
                    let kind = drill_kind(command);
                    send_input(
                        (kind.as_str(), kind),
                        |_, app| {
                            result
                                .set(Some(view.update(app, |view, cx| view.command(command, cx))));
                            true
                        },
                        Wait::Never,
                        true,
                        &board,
                        &mut stats,
                        cx,
                    )
                    .await?;
                    result.take().ok_or("the command was not sent")?
                }
                Step::Tick(ms) => {
                    clock += ms;
                    run.clock.set(clock);
                    let before = view.read_with(cx, |view, _| view.engine().state().clone());
                    cx.update(|_, app| view.update(app, |view, cx| view.tick(cx)))
                        .map_err(gone)?;
                    let after = view.read_with(cx, |view, _| view.engine().state().clone());
                    background.timer(DRILL_TICK_GAP).await;
                    Ok(if before == after {
                        Outcome::Unchanged
                    } else {
                        Outcome::Changed
                    })
                }
            };
            let actual = view.read_with(cx, |view, _| {
                state_line(index + 1, text, view.engine().state(), outcome)
            });
            if actual != wanted[index] {
                stats.mismatches += 1;
                if stats.mismatch_notes.len() < DRILL_NOTES {
                    stats
                        .mismatch_notes
                        .push(format!("expected `{}`, got `{actual}`", wanted[index]));
                }
            }
            if index == middle {
                memory.push(sample_json("middle", &drill_sample(cx).await?));
            }
        }
        settle_frames(view, cx).await;
        let end = drill_sample(cx).await?;
        memory.push(sample_json("end", &end));
        let peak = run
            .watch
            .as_ref()
            .ok_or("the memory watch was not started")?
            .finish(Some(end));
        let (peak_bytes, peak_source) = match peak.peak_private_bytes {
            Some(bytes) => (bytes, "peak_private_bytes"),
            None => (peak.peak_rss_bytes, "peak_rss_bytes"),
        };
        let violations = hot_path::take_violations();
        let violation_total: u64 = violations.iter().map(|(_, count)| count).sum();
        let by_kind: Vec<String> = violations
            .iter()
            .map(|(kind, count)| format!("\"{}\":{count}", kind.as_str()))
            .collect();
        let notes: Vec<String> = stats
            .mismatch_notes
            .iter()
            .map(|note| json_string(note))
            .collect();
        let ticks = steps
            .iter()
            .filter(|step| matches!(step, Step::Tick(_)))
            .count();
        Ok(format!(
            "{{{},\"schema\":\"ordo.lab.recovery-drill.v1\",\"latency_scope\":\"keydown_to_surface_paint\",\"method\":\"each non-comment line of commands.txt through PresenterView::command on a stepped clock (100 ms per command, tick lines advance it and tick the view), output enabled by the script with no congregation window; the Engine state after every line is compared with commands.expect; latency is stamped just before each command and ends at the later of the Preview and Program paints\",\"fixture\":{},\"commands\":{},\"timed_commands\":{},\"ticks\":{ticks},\"mismatches\":{},\"first_mismatches\":[{}],{},\"unpainted\":{},\"peak_bytes\":{peak_bytes},\"peak_source\":{},\"memory\":[{}],\"hot_path_violations\":{violation_total},\"hot_path_by_kind\":{{{}}},\"window\":{},\"renderer\":{},\"elapsed_s\":{}}}",
            header(MODE_DRILL, true),
            json_string(&run.snapshot.display().to_string()),
            steps.len(),
            stats.operator.count(),
            stats.mismatches,
            notes.join(","),
            tail_fields(Some(&stats.operator)),
            stats.unpainted_json(),
            json_string(peak_source),
            memory.join(","),
            by_kind.join(","),
            window_json(&setup, run),
            setup.renderer,
            number(started.elapsed().as_secs_f64()),
        ))
    }

    // --------------------------------------------------------------- output

    fn recognition_json(verdict: &Recognition) -> String {
        match verdict {
            Recognition::Recognized(id) => {
                format!("{{\"verdict\":\"recognized\",\"display\":{}}}", id.raw())
            }
            Recognition::Ambiguous(ids) => {
                let ids: Vec<String> = ids.iter().map(|id| id.raw().to_string()).collect();
                format!(
                    "{{\"verdict\":\"ambiguous\",\"displays\":[{}]}}",
                    ids.join(",")
                )
            }
            Recognition::Missing => "{\"verdict\":\"missing\"}".to_owned(),
        }
    }

    struct CongregationFacts {
        bounds: String,
        scale_factor: f32,
        display: Option<gpui::DisplayId>,
    }

    async fn drive_output(
        run: &Run,
        view: &Entity<PresenterView>,
        cx: &mut AsyncWindowContext,
    ) -> Result<String, String> {
        let started = Instant::now();
        let setup = setup(view, cx)?;
        let background = cx.background_executor().clone();
        let board = run.board.clone();
        let mut stats = Stats::default();
        settle_frames(view, cx).await;
        background.timer(Duration::from_millis(300)).await;
        hot_path::take_violations();
        let live = |view: &Entity<PresenterView>, cx: &mut AsyncWindowContext| {
            view.read_with(cx, |view, _| view.engine().state().live())
        };

        // Enable Output: the congregation window opens black.
        let enable_at = Instant::now();
        let refused = cx
            .update(|window, app| {
                view.update(app, |view, cx| {
                    view.enable_output(window, cx);
                    view.output_window()
                        .is_none()
                        .then(|| view.status().map(str::to_owned))
                })
            })
            .map_err(gone)?;
        if let Some(status) = refused {
            return Err(format!(
                "Enable Output did not open the congregation window: {}",
                status.unwrap_or_else(|| "no reason given".to_owned())
            ));
        }
        let waited = Instant::now();
        while board.borrow().congregation_first.is_none() {
            if waited.elapsed() > Duration::from_secs(3) {
                return Err("the congregation window never painted".to_owned());
            }
            background.timer(POLL).await;
        }
        let (first_at, first_black) = board.borrow().congregation_first.ok_or("no first paint")?;
        let enable_to_black = first_at.saturating_duration_since(enable_at);
        background.timer(FADE_GAP).await;
        check_identity("enable output", &board, &mut stats, cx).await;

        let window = view
            .read_with(cx, |view, _| view.output_window())
            .ok_or("the congregation window is gone")?;
        let facts = cx
            .update(|_, app| {
                window.update(app, |_, window, app| CongregationFacts {
                    bounds: format!("{:?}", window.bounds()),
                    scale_factor: window.scale_factor(),
                    display: window.display(app).map(|display| display.id()),
                })
            })
            .map_err(gone)?
            .map_err(gone)?;
        let window_id = window.window_id();

        // Take, 200 navigation steps while Live, F, a few steps, F, B, B.
        send_key(Key::Space, Wait::Always, true, &board, &mut stats, cx).await?;
        background.timer(FADE_GAP).await;
        check_identity("take", &board, &mut stats, cx).await;
        for (index, key) in navigation_stream(SEED, NAVIGATION_STEPS)
            .into_iter()
            .enumerate()
        {
            send_key(key, Wait::IfFrameChanged, true, &board, &mut stats, cx).await?;
            check_identity(&format!("navigation {index}"), &board, &mut stats, cx).await;
        }
        send_key(Key::F, Wait::IfFrameChanged, true, &board, &mut stats, cx).await?;
        check_identity("freeze", &board, &mut stats, cx).await;
        for key in navigation_stream(SEED ^ 2, 4) {
            send_key(key, Wait::IfFrameChanged, true, &board, &mut stats, cx).await?;
        }
        send_key(Key::F, Wait::IfFrameChanged, true, &board, &mut stats, cx).await?;
        check_identity("release freeze", &board, &mut stats, cx).await;
        for label in ["blackout", "release blackout"] {
            send_key(Key::B, Wait::Always, true, &board, &mut stats, cx).await?;
            background.timer(FADE_GAP).await;
            check_identity(label, &board, &mut stats, cx).await;
        }

        // Program goes black before the window closes.
        if matches!(live(view, cx), LiveState::Live | LiveState::Frozen) {
            send_key(Key::B, Wait::Always, false, &board, &mut stats, cx).await?;
            background.timer(FADE_GAP).await;
        }
        let state_before_close = live(view, cx);
        let black_before_close = view.read_with(cx, |view, _| view.program_drawn().frame.is_none())
            && matches!(
                state_before_close,
                LiveState::Blackout | LiveState::EnabledBlack
            );

        // Disable Output: the window must go.
        let disable_at = Instant::now();
        cx.update(|_, app| {
            view.update(app, |view, cx| {
                view.command(Command::DisableOutput, cx).ok()
            })
        })
        .map_err(gone)?;
        let gone_after = loop {
            let present = cx
                .update(|_, app| app.windows().iter().any(|w| w.window_id() == window_id))
                .map_err(gone)?;
            if !present {
                break Some(disable_at.elapsed());
            }
            if disable_at.elapsed() > Duration::from_secs(3) {
                break None;
            }
            background.timer(POLL).await;
        };
        let final_state = live(view, cx);
        let disabled = final_state == LiveState::OutputDisabled
            && view.read_with(cx, |view, _| view.output_window().is_none());
        if gone_after.is_none() || !disabled {
            return Err("Disable Output did not close the congregation window".to_owned());
        }

        // The display's identity and the verdict of recognition on it.
        let names = monitor_identity::read();
        let (topology, displays, uuid) = cx
            .update(|_, app| {
                let uuid = facts
                    .display
                    .and_then(|id| app.find_display(id))
                    .and_then(|display| display.uuid().ok().map(|uuid| uuid.to_string()));
                (
                    topology_with_names(&app.displays(), setup.operator, &names),
                    displays_json(app, setup.operator, &names, Some(setup.scale_factor)),
                    uuid,
                )
            })
            .map_err(gone)?;
        let gpui_id = facts.display.map(u64::from);
        let detected = gpui_id.and_then(|id| topology.display(DisplayId::new(id)));
        let (name, verdict) =
            detected.map_or((None, "{\"verdict\":\"unknown\"}".to_owned()), |display| {
                (
                    display.evidence.name.clone(),
                    recognition_json(&recognize(&display.evidence, &topology)),
                )
            });
        let identity = format!(
            "{{\"gpui_id\":{},\"uuid\":{},\"monitor_name\":{},\"generic_name\":{},\"recognition\":{verdict},\"window_bounds\":{},\"window_scale_factor\":{}}}",
            gpui_id.map_or("null".to_owned(), |id| id.to_string()),
            text_or_null(uuid),
            text_or_null(name.clone()),
            name.is_none(),
            json_string(&facts.bounds),
            float(facts.scale_factor),
        );
        let (paints, off_program) = {
            let b = board.borrow();
            (b.congregation_paints, b.congregation_off_program)
        };
        let notes: Vec<String> = stats
            .mismatch_notes
            .iter()
            .map(|n| json_string(n))
            .collect();
        Ok(format!(
            "{{{},\"schema\":\"ordo.lab.output-latency.v1\",\"latency_scope\":\"keydown_to_surface_paint\",\"method\":\"real keys through Window::dispatch_keystroke with a real congregation window on the non-operator display; each surface stamps its own paint; the key arrives between frames, so the latency includes the wait for the next frame\",\"seed\":{SEED},\"fixture\":{},\"window\":{},\"renderer\":{},\"displays\":{displays},\"congregation_display\":{identity},\"latency\":{},\"latency_by_key\":{},\"enable_to_first_black_frame_ms\":{},\"first_frame_was_black\":{first_black},\"disable_to_window_gone_ms\":{},\"frame_identity\":{{\"method\":\"Arc pointer of the frame the congregation window drew against the frame the Program monitor drew last\",\"settled_checks\":{},\"lagging_at_first\":{},\"settled_mismatches\":{},\"mismatch_notes\":[{}],\"congregation_paints\":{paints},\"paints_off_program_frame\":{off_program},\"paints_off_program_note\":\"transient: the window painted before the Program monitor did\",\"frames_match\":{}}},\"program_black_before_close\":{black_before_close},\"state_before_close\":{},\"final_state\":{},\"output_disabled_at_end\":{disabled},\"unpainted\":{},\"unhandled\":{},\"hot_path_violations\":{},\"elapsed_s\":{}}}",
            header(MODE_OUTPUT, true),
            json_string(&run.snapshot.display().to_string()),
            window_json(&setup, run),
            setup.renderer,
            stats.latency_json(),
            stats.by_key_json(),
            optional_ms(Some(enable_to_black)),
            optional_ms(gone_after),
            stats.checks,
            stats.lagged,
            stats.mismatches,
            notes.join(","),
            stats.mismatches == 0,
            json_string(&format!("{state_before_close:?}")),
            json_string(&format!("{final_state:?}")),
            stats.unpainted_json(),
            stats.unhandled,
            hot_path_json(),
            number(started.elapsed().as_secs_f64()),
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_mixed_stream_is_seeded_and_covers_every_key() {
            let a = mixed_stream(SEED, 2000);
            assert_eq!(a, mixed_stream(SEED, 2000));
            assert_ne!(a, mixed_stream(SEED + 1, 2000));
            assert_eq!(a.len(), 2000);
            for key in [
                Key::Right,
                Key::Left,
                Key::Down,
                Key::Up,
                Key::PageDown,
                Key::PageUp,
                Key::Home,
                Key::End,
                Key::Space,
                Key::F,
                Key::B,
            ] {
                assert!(a.contains(&key), "{key:?}");
            }
            for kind in CommandKind::ALL {
                assert!(a.iter().any(|key| key.kind() == kind), "{kind:?}");
            }
        }

        #[test]
        fn the_navigation_stream_has_only_navigation() {
            let stream = navigation_stream(SEED, NAVIGATION_STEPS);
            assert_eq!(stream.len(), NAVIGATION_STEPS);
            assert!(
                stream
                    .iter()
                    .all(|key| key.kind() == CommandKind::Navigation)
            );
        }

        #[test]
        fn every_key_name_is_a_valid_keystroke() {
            for key in mixed_stream(SEED, 500) {
                assert!(Keystroke::parse(key.name()).is_ok(), "{key:?}");
            }
        }

        #[test]
        fn black_frames_are_the_same_frame() {
            assert!(same_frame(&None, &None));
        }

        #[test]
        fn the_snapshot_is_the_given_one_else_the_first_existing_candidate() {
            let dir = std::env::temp_dir().join(format!("ordo-kit-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let missing = dir.join("missing");
            assert_eq!(pick_snapshot(Some(&dir), &[], "normal-mass").unwrap(), dir);
            assert!(
                pick_snapshot(Some(&missing), std::slice::from_ref(&dir), "normal-mass").is_err()
            );
            assert_eq!(
                pick_snapshot(None, &[missing.clone(), dir.clone()], "normal-mass").unwrap(),
                dir
            );
            let error =
                pick_snapshot(None, std::slice::from_ref(&missing), "normal-mass").unwrap_err();
            assert!(
                error.contains("--snapshot") && error.contains("missing"),
                "{error}"
            );
            std::fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn documents_carry_ok_commit_and_host() {
            let error = error_doc(MODE_OUTPUT, "boom \"x\"");
            assert!(error.starts_with("{\"ok\":false,\"mode\":\"output-latency\",\"commit\":"));
            assert!(error.contains("\"host\":{") && error.contains("\"error\":\"boom \\\"x\\\"\""));
            let skipped = skipped_doc("no congregation display");
            assert!(skipped.starts_with("{\"ok\":true,"));
            assert!(skipped.contains("\"skipped\":\"no congregation display\""));
        }

        #[test]
        fn tail_fields_are_null_without_samples_and_ordered_with_them() {
            assert!(tail_fields(None).contains("\"samples\":0,\"p50_ms\":null"));
            let mut histogram = LatencyHistogram::new();
            for ms in 1..=100 {
                histogram.record(Duration::from_millis(ms));
            }
            let text = tail_fields(Some(&histogram));
            assert!(
                text.contains("\"p50_ms\":50.000") && text.contains("\"max_ms\":100.000"),
                "{text}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn defaults_are_the_compact_reference_size_on_the_normal_mass() {
        let args = parse(&[]).unwrap();
        assert_eq!((args.width, args.height), (1366.0, 768.0));
        assert_eq!(args.scale, InterfaceScale::Percent100);
        assert!(
            args.snapshot
                .ends_with("tests/fixtures/presentations/normal-mass")
        );
    }

    #[test]
    fn size_scale_and_directory_are_read() {
        let args = parse(&["--size", "1920x1080", "--scale", "200", "some/dir"]).unwrap();
        assert_eq!((args.width, args.height), (1920.0, 1080.0));
        assert_eq!(args.scale, InterfaceScale::Percent200);
        assert_eq!(args.snapshot, PathBuf::from("some/dir"));
    }

    #[test]
    fn measure_memory_fixes_the_size_and_scale_and_takes_an_optional_snapshot() {
        let args = parse(&[
            "--size",
            "800x600",
            "--scale",
            "200",
            "--measure-memory",
            "out.json",
        ])
        .unwrap();
        assert_eq!(args.measure_memory, Some(PathBuf::from("out.json")));
        assert_eq!((args.width, args.height), (1920.0, 1080.0));
        assert_eq!(args.scale, InterfaceScale::Percent100);
        assert!(!args.snapshot_given);
        let args = parse(&["--measure-memory", "out.json", "--snapshot", "dir"]).unwrap();
        assert!(args.snapshot_given);
        assert_eq!(args.snapshot, PathBuf::from("dir"));
    }

    #[test]
    fn measure_animations_takes_size_scale_cues_and_view() {
        let args = parse(&[
            "--measure-animations",
            "anim.json",
            "--size",
            "1920x1080",
            "--scale",
            "200",
            "--cues",
            "30",
            "--list",
        ])
        .unwrap();
        assert_eq!(args.measure_animations, Some(PathBuf::from("anim.json")));
        assert_eq!((args.width, args.height), (1920.0, 1080.0));
        assert_eq!(args.scale, InterfaceScale::Percent200);
        assert_eq!(args.cues, 30);
        assert!(args.list_view);
        let defaults = parse(&["--measure-animations", "a.json"]).unwrap();
        assert_eq!((defaults.cues, defaults.list_view), (500, false));
        assert_eq!((defaults.width, defaults.height), (1366.0, 768.0));
    }

    #[test]
    fn the_church_kit_modes_are_read() {
        let args = parse(&["--display-inventory", "inv.json"]).unwrap();
        assert_eq!(args.display_inventory, Some(PathBuf::from("inv.json")));
        let args = parse(&["--measure-presenter", "p.json"]).unwrap();
        assert_eq!(args.measure_presenter, Some(PathBuf::from("p.json")));
        assert_eq!(args.commands, 2000);
        assert!(!args.size_given && !args.snapshot_given);
        let args = parse(&[
            "--measure-presenter",
            "p.json",
            "--commands",
            "50",
            "--snapshot",
            "fixtures/normal-mass",
            "--size",
            "1920x1080",
        ])
        .unwrap();
        assert_eq!(args.commands, 50);
        assert!(args.snapshot_given && args.size_given);
        assert_eq!((args.width, args.height), (1920.0, 1080.0));
        let args = parse(&["--measure-output", "o.json"]).unwrap();
        assert_eq!(args.measure_output, Some(PathBuf::from("o.json")));
    }

    #[test]
    fn the_church_kit_modes_are_refused_when_mixed_or_malformed() {
        for bad in [
            &["--display-inventory"][..],
            &["--measure-presenter"],
            &["--measure-output"],
            &["--measure-presenter", "p.json", "--commands"],
            &["--measure-presenter", "p.json", "--commands", "0"],
            &["--measure-presenter", "p.json", "--commands", "many"],
            &["--measure-presenter", "p.json", "--commands", "9999999"],
            &["--commands", "50"],
            &["--measure-output", "o.json", "--commands", "50"],
            &[
                "--measure-presenter",
                "p.json",
                "--measure-output",
                "o.json",
            ],
            &[
                "--display-inventory",
                "i.json",
                "--measure-memory",
                "m.json",
            ],
            &[
                "--measure-output",
                "o.json",
                "--measure-animations",
                "a.json",
            ],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn bad_arguments_are_refused_with_a_reason() {
        for bad in [
            &["--size"][..],
            &["--size", "wide"],
            &["--size", "10x10"],
            &["--scale", "110"],
            &["--frobnicate"],
            &["--measure-memory"],
            &["--snapshot"],
            &["--measure-animations"],
            &["--cues"],
            &["--cues", "40"],
            &[
                "--measure-memory",
                "a.json",
                "--measure-animations",
                "b.json",
            ],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
