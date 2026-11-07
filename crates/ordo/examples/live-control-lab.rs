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
    if parsed.measure_memory.is_some() {
        // The measurement is defined at one size and scale.
        parsed.width = measure::WIDTH;
        parsed.height = measure::HEIGHT;
        parsed.scale = InterfaceScale::Percent100;
    }
    Ok(parsed)
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
}

impl LabRoot {
    fn new(
        snapshot_dir: PathBuf,
        environment: UiEnvironment,
        save: SavePreferences,
        measure: Option<measure::Run>,
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
        };
        match verified {
            Err(error) => {
                let message = format!(
                    "The bundled fonts did not verify, so the Presenter will not open: {error}"
                );
                if let Some(run) = root.measure.take() {
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
                self.stage = Stage::Failed(message.into());
            }
            Ok(snapshot) => {
                let snapshot = Arc::new(snapshot);
                // The measurement skips the image check: its transient decodes
                // would be counted in the memory it is measuring.
                if self.measure.is_none() {
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
            .child(body)
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
                "usage: live-control-lab [--size 1366x768] [--scale 100|125|150|175|200] [snapshot-dir] | --measure-memory <out.json> [--snapshot <dir>]"
            );
            std::process::exit(2);
        }
    };

    // The measurement writes and reads no preferences, so a run depends only on
    // its own arguments.
    let measuring = args.measure_memory.is_some();
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
    if args.measure_memory.is_some() && !args.snapshot_given {
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
        let measure_out = args.measure_memory.clone();
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
                    let root = LabRoot::new(snapshot, environment, save, measure, window, cx);
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
        cx.spawn(async move |cx| {
            cx.background_executor().timer(TIMEOUT).await;
            cx.update(|cx| {
                let message = format!("timed out after {} s", TIMEOUT.as_secs());
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
    fn bad_arguments_are_refused_with_a_reason() {
        for bad in [
            &["--size"][..],
            &["--size", "wide"],
            &["--size", "10x10"],
            &["--scale", "110"],
            &["--frobnicate"],
            &["--measure-memory"],
            &["--snapshot"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
