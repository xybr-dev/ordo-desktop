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
//! * Preferences (the Grid/List choice) are read from and written under
//!   `target/live-control-lab/`, never `%APPDATA%`. Saves go to one worker
//!   thread, owned by `main`, which drops the sender when the window closes
//!   and joins the thread after the app loop ends.

use std::{
    cell::RefCell,
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
        .filter(|entry| entry.scope != Scope::Surface)
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
    /// `--cues 30|500`: the fixture the animation measurement runs over.
    cues: usize,
    /// `--list` / `--grid`: the Cue view the animation measurement runs in.
    list_view: bool,
}

fn default_snapshot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/presentations/normal-mass")
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
    if parsed.measure_memory.is_some() && parsed.measure_animations.is_some() {
        return Err("--measure-memory and --measure-animations are separate runs".into());
    }
    if parsed.measure_memory.is_some() {
        // The measurement is defined at one size and scale.
        parsed.width = measure::WIDTH;
        parsed.height = measure::HEIGHT;
        parsed.scale = InterfaceScale::Percent100;
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
}

impl LabRoot {
    fn new(
        snapshot_dir: PathBuf,
        environment: UiEnvironment,
        save: SavePreferences,
        measure: Option<measure::Run>,
        animations: Option<animations::Run>,
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
                self.stage = Stage::Failed(message.into());
            }
            Ok(snapshot) => {
                let snapshot = Arc::new(snapshot);
                // The measurement skips the image check: its transient decodes
                // would be counted in the memory it is measuring.
                if self.measure.is_none() && self.animations.is_none() {
                    self.check_images(snapshot.clone(), window, cx);
                }
                let started = self.started;
                let clock: Rc<dyn Fn() -> Millis> =
                    Rc::new(move || Millis(started.elapsed().as_millis() as u64));
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
                "usage: live-control-lab [--size 1366x768] [--scale 100|125|150|175|200] [snapshot-dir] | --measure-memory <out.json> [--snapshot <dir>] | --measure-animations <out.json> [--size WxH] [--scale N] [--cues 30|500] [--list|--grid]"
            );
            std::process::exit(2);
        }
    };

    // The measurement writes and reads no preferences, so a run depends only on
    // its own arguments.
    let measuring = args.measure_memory.is_some() || args.measure_animations.is_some();
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
    let mut args = args;
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
            .or_else(|| args.measure_animations.clone());
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
                window_bounds: Some(WindowBounds::Windowed(bounds)),
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
                    let root =
                        LabRoot::new(snapshot, environment, save, measure, animations, window, cx);
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
        cx.on_window_closed(move |cx, _| {
            sender.borrow_mut().take();
            cx.quit();
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
    pub const TIMEOUT: Duration = Duration::from_secs(90);
    /// Idle time between two scripted presses, with nothing recorded.
    const GAP: Duration = Duration::from_millis(250);
    /// How often a hold samples the frame store.
    const POLL: Duration = Duration::from_millis(8);
    const SETTLE_POLL: Duration = Duration::from_millis(50);
    const SETTLE_QUIET_POLLS: usize = 4;
    const SETTLE_LIMIT: Duration = Duration::from_secs(10);
    /// The 60 Hz interval the limits are judged against.
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

    fn pct(part: usize, whole: usize) -> f64 {
        if whole == 0 {
            0.0
        } else {
            part as f64 * 100.0 / whole as f64
        }
    }

    fn animation_json(
        script: &Script,
        applicable: bool,
        capture: Capture,
        before: Counters,
        after: Counters,
        jobs_max: usize,
    ) -> String {
        let intervals: Vec<f64> = capture.frames.iter().filter_map(|f| f.interval).collect();
        let over = |limit: f64| intervals.iter().filter(|v| **v > limit).count();
        let arrivals: Vec<&Frame> = capture.frames.iter().filter(|f| f.arrival).collect();
        let max_of = |values: &mut dyn Iterator<Item = f64>| values.fold(0.0_f64, f64::max);
        let column = |select: fn(&Frame) -> f64| -> String {
            summary(capture.frames.iter().map(select).collect())
        };
        format!(
            "{{\"name\":{},\"what\":{},\"applicable\":{applicable},\"segments\":{},\"frames\":{},\"intervals\":{},\"interval_ms\":{},\"over_16_7ms_pct\":{:.1},\"over_25ms\":{},\"dropped_1_5x_frametimer\":{},\"over_33ms\":{},\"first_frame_ms\":{},\"render_ms\":{},\"layout_ms\":{},\"prepaint_ms\":{},\"paint_ms\":{},\"cpu_ms\":{},\"store\":{{\"composed_delta\":{},\"thumb_images_delta\":{},\"full_images_delta\":{},\"jobs_in_flight_max\":{jobs_max},\"frames_with_arrival\":{},\"max_interval_with_arrival_ms\":{:.2},\"max_cpu_with_arrival_ms\":{:.2}}},\"inactive_window_frames\":{}}}",
            json_string(script.name),
            json_string(script.what),
            capture.segments,
            capture.frames.len(),
            intervals.len(),
            summary(intervals.clone()),
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
                applicable,
                run.pacing.take(),
                before,
                after,
                jobs_max,
            ));
        }

        Ok(format!(
            "{{\"mode\":\"animation-pacing\",\"issue\":111,\"profile\":{},\"window\":{},\"viewport\":{},\"scale_percent\":{},\"scale_factor\":{scale_factor},\"cue_view\":{},\"cues_requested\":{},\"cue_count\":{cues},\"groups\":{groups},\"window_active_at_start\":{active},\"window_maximized\":{maximized},\"window_bounds\":{},\"limits_ms\":{{\"frame\":{FRAME_MS},\"dropped\":{DROP_MS},\"hitch\":{HITCH_MS}}},\"note\":{},\"animations\":[{}]}}",
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
            json_string(
                "intervals are between paint stamps of consecutive drawn frames inside one scripted press; first_frame_ms is press to first drawn frame"
            ),
            animations.join(","),
        ))
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
