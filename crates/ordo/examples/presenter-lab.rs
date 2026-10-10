//! Presenter lab for the T0.6 native measurement (WP7).
//!
//! `cargo run --release -p ordo --example presenter-lab` opens the Presenter
//! probe over a 500-Cue deck for review on real hardware. Right and Left move
//! Preview, Space takes Preview live, F freezes Program, and B blacks it out.
//! The footer shows live keydown-to-paint p50, p95, and p99 per command and
//! surface.
//!
//! The same binary runs the measurements `cargo xtask measure-presenter`
//! collects. Each one is a fresh process, so every memory baseline is clean:
//!
//! - `--measure <out.json>` opens the real operator runtime, then the probe
//!   beside it, drives the seeded 2,000-command stream, idles, and writes the
//!   report with the oracle check. Peak memory is the OS-recorded peak on
//!   Windows, with a 250 ms sampler as the fallback elsewhere.
//! - `--display-path <out.json> [passes]` compares the CPU raster and retained
//!   display paths for one canonical frame in a real window.
//! - `--baseline <empty|preparation|presenter> <out.json>` samples process
//!   memory of three modes, each with the real operator runtime:
//!   - `empty` (`"empty_shell"`): the runtime only.
//!   - `preparation` (`"preparation_stand_in"`): the runtime plus a synthetic
//!     10,000-entry library held in memory. No Preparation UI or storage exists
//!     yet.
//!   - `presenter` (`"presenter_probe_in_runtime"`): the runtime plus the probe
//!     with 500 Cues. 500 canonical 1920x1080 frames pass through a bounded
//!     `FrameCache` and a `ThumbnailCache` (visible window only), then 200
//!     commands are driven before the sample.
//!
//! Every measuring mode quits itself. A mode gets 30 s plus 100 ms per command
//! before it writes an error document and exits nonzero, so CI cannot hang.
//! Only the output path the caller names is written. The runtime's
//! installation preferences go to a temporary file, which every exit path
//! removes, so the lab never touches real Parish data.

use std::{
    any::Any,
    cell::{Cell, RefCell},
    fmt::Write as _,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowHandle, WindowOptions, px, size};
use gpui_platform::application;
use ordo::{
    NativeRuntime, open_native_runtime_at,
    performance::{
        DriveError, PresenterProbe, ProbeOutcome, StartupClock,
        bench::{BenchConfig, command_stream},
        drive,
        native_display_path::{DisplayPathProbe, measure},
        open_probe,
        process::{PeakMemory, ProcessSample, ProcessSampler},
    },
};
use presentation::{
    ProgramFrame,
    cache::{FrameCache, ThumbnailCache},
    presenter::{PresenterCommand, PresenterEngine, PresenterState},
};

/// Cues in the probe deck: the adopted 500-Cue fixture.
const PROBE_CUES: usize = 500;
/// Commands in the measured run: the adopted 2,000-command stream.
const MEASURE_COMMANDS: usize = 2000;
/// Commands driven before the Presenter baseline is sampled.
const BASELINE_COMMANDS: usize = 200;
/// Idle window after a measured run, with no input.
const IDLE: Duration = Duration::from_secs(3);
/// Settle time before a memory baseline is sampled.
const SETTLE: Duration = Duration::from_secs(2);
/// Measured passes per display path when none is given.
const DEFAULT_PASSES: usize = 30;
/// A measuring mode gets this long plus `TIMEOUT_PER_COMMAND` per command.
/// The 2,000-command run gets 230 s, under xtask's 600 s kill.
const BASE_TIMEOUT: Duration = Duration::from_secs(30);
const TIMEOUT_PER_COMMAND: Duration = Duration::from_millis(100);
/// Interval of the background memory sampler that feeds the peak.
const MEMORY_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
/// Entries in the synthetic library the Preparation stand-in holds.
const LIBRARY_ENTRIES: usize = 10_000;
const LIBRARY_ENTRY_BYTES: usize = 512;
/// Canonical frame size, and the number of Cue frames the Presenter baseline
/// pushes through the caches.
const CANONICAL_WIDTH: u32 = 1920;
const CANONICAL_HEIGHT: u32 = 1080;
const CANONICAL_FRAME_BYTES: usize = CANONICAL_WIDTH as usize * CANONICAL_HEIGHT as usize * 4;
const CACHED_FRAMES: usize = 500;
/// The full-resolution cache holds 16 canonical frames (about 127 MiB), so
/// the 500 inserts evict all but the last 16.
const FRAME_CACHE_BUDGET: usize = 16 * CANONICAL_FRAME_BYTES;
/// Thumbnails are downscaled to fit this box.
const THUMBNAIL_WIDTH: u32 = 320;
const THUMBNAIL_HEIGHT: u32 = 180;
/// The thumbnails inside the operator's visible strip. The rest are released.
const VISIBLE_FIRST: usize = 240;
const VISIBLE_THUMBNAILS: usize = 12;
/// What the report's `startup_scope` names: the first frame after the first
/// draw, timed from process start.
const STARTUP_SCOPE: &str = "process_start_to_next_frame_after_first_present";
/// What `private_bytes` and the peaks mean in a baseline document.
const PRIVATE_BYTES_NOTE: &str = "Windows PrivateUsage; peaks are OS-recorded since process start (Decision 269); null off Windows";

/// Set by the first `finish`, so a late timeout cannot overwrite a result.
static FINISHED: AtomicBool = AtomicBool::new(false);
/// The process exit status, set when a measurement fails.
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);
/// The temporary installation preferences file this process opened. Removed
/// by `finish`, on every exit path.
static TEMP_PREFERENCES: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
enum Baseline {
    Empty,
    Preparation,
    Presenter,
}

impl Baseline {
    /// The mode name written into the baseline document.
    fn mode(self) -> &'static str {
        match self {
            Self::Empty => "empty_shell",
            Self::Preparation => "preparation_stand_in",
            Self::Presenter => "presenter_probe_in_runtime",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Empty => "real operator runtime only",
            Self::Preparation => {
                "real operator runtime plus a synthetic 10,000-entry library held in memory; no Preparation UI or storage exists yet"
            }
            Self::Presenter => {
                "real operator runtime plus a 500-Cue Presenter probe window; 500 canonical 1920x1080 frames through the bounded FrameCache and ThumbnailCache (visible window only); 200 commands driven"
            }
        }
    }

    /// Commands the mode drives before it samples.
    fn commands(self) -> usize {
        match self {
            Self::Presenter => BASELINE_COMMANDS,
            Self::Empty | Self::Preparation => 0,
        }
    }
}

enum Mode {
    Interactive,
    Measure(PathBuf),
    DisplayPath { out: PathBuf, passes: usize },
    Baseline { kind: Baseline, out: PathBuf },
}

const USAGE: &str = "usage: presenter-lab [--measure <out.json> | --display-path <out.json> [passes] | --baseline <empty|preparation|presenter> <out.json>]";

fn parse_args(args: &[String]) -> Result<Mode, String> {
    match args {
        [] => Ok(Mode::Interactive),
        [flag, out] if flag == "--measure" => Ok(Mode::Measure(out.into())),
        [flag, out] if flag == "--display-path" => Ok(Mode::DisplayPath {
            out: out.into(),
            passes: DEFAULT_PASSES,
        }),
        [flag, out, passes] if flag == "--display-path" => {
            let passes = passes
                .parse()
                .map_err(|_| format!("passes must be a whole number, not {passes:?}\n{USAGE}"))?;
            Ok(Mode::DisplayPath {
                out: out.into(),
                passes,
            })
        }
        [flag, kind, out] if flag == "--baseline" => {
            let kind = match kind.as_str() {
                "empty" => Baseline::Empty,
                "preparation" => Baseline::Preparation,
                "presenter" => Baseline::Presenter,
                other => return Err(format!("unknown baseline {other:?}\n{USAGE}")),
            };
            Ok(Mode::Baseline {
                kind,
                out: out.into(),
            })
        }
        _ => Err(USAGE.to_owned()),
    }
}

fn main() {
    // Must be the first statement: startup time is measured from here.
    StartupClock::capture_process_start();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match parse_args(&args) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    application().run(move |cx: &mut App| match mode {
        Mode::Interactive => {
            if let Err(error) = open_probe(cx, probe_window_options(cx), PROBE_CUES) {
                eprintln!("cannot open the Presenter probe: {error:?}");
                EXIT_CODE.store(1, Ordering::SeqCst);
                cx.quit();
                return;
            }
            cx.activate(true);
        }
        Mode::Measure(out) => measure_run(cx, out),
        Mode::DisplayPath { out, passes } => display_path_run(cx, out, passes),
        Mode::Baseline { kind, out } => baseline_run(cx, kind, out),
    });
    std::process::exit(EXIT_CODE.load(Ordering::SeqCst));
}

fn probe_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.0), px(720.0)),
            cx,
        ))),
        ..Default::default()
    }
}

/// Writes `document` to `out` and quits. Only the first call has an effect.
/// `ok` is false when the document describes a failure, which makes the
/// process exit nonzero. The temporary preferences file is removed here, so
/// every exit path cleans it up.
fn finish(cx: &mut App, out: &Path, document: String, ok: bool) {
    if FINISHED.swap(true, Ordering::SeqCst) {
        return;
    }
    if !ok {
        EXIT_CODE.store(1, Ordering::SeqCst);
    }
    if let Err(error) = std::fs::write(out, document) {
        eprintln!("cannot write {}: {error}", out.display());
        EXIT_CODE.store(1, Ordering::SeqCst);
    }
    if let Some(path) = TEMP_PREFERENCES.get() {
        // Best effort: a file the runtime never wrote is simply absent.
        let _ = std::fs::remove_file(path);
    }
    cx.quit();
}

fn error_json(message: &str) -> String {
    format!("{{\"error\":{}}}", json_string(message))
}

/// A quoted JSON string. Control characters are escaped, so any message is
/// a valid document.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            control if control.is_control() => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The timeout for a mode that drives `steps` commands (or passes).
fn timeout_for(steps: usize) -> Duration {
    let steps = u32::try_from(steps).unwrap_or(u32::MAX);
    BASE_TIMEOUT + TIMEOUT_PER_COMMAND * steps
}

/// Fails the run with an error document if it has not finished in time.
fn arm_timeout(cx: &mut App, out: PathBuf, timeout: Duration) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(timeout).await;
        cx.update(|cx| {
            let message = format!("timed out after {} s", timeout.as_secs());
            finish(cx, &out, error_json(&message), false);
        });
    })
    .detach();
}

fn oracle(cues: usize, commands: &[PresenterCommand]) -> PresenterState {
    let mut engine = PresenterEngine::new(cues);
    for &command in commands {
        engine.apply(command);
    }
    *engine.state()
}

fn milliseconds_json(value: Option<Duration>) -> String {
    match value {
        Some(duration) => format!("{:.3}", duration.as_secs_f64() * 1000.0),
        None => "null".to_owned(),
    }
}

fn optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

/// A probe window and the cell its startup time is written into.
type TimedProbe = (WindowHandle<PresenterProbe>, Rc<Cell<Option<Duration>>>);

/// Opens the probe and records the startup time: the first frame drawn after
/// the first draw, measured from process start. The first `on_next_frame`
/// runs before that frame's draw, so it cannot see the draw. The callback it
/// registers runs at the start of the following frame, after the first draw
/// was presented.
fn open_timed_probe(cx: &mut App) -> Result<TimedProbe, String> {
    let window = open_probe(cx, probe_window_options(cx), PROBE_CUES)
        .map_err(|error| format!("cannot open the Presenter probe: {error:?}"))?;
    let startup = Rc::new(Cell::new(None));
    let slot = startup.clone();
    window
        .update(cx, |_, window, _| {
            window.on_next_frame(move |window, _cx| {
                window.on_next_frame(move |_, _| {
                    let mut clock = StartupClock::new();
                    slot.set(Some(clock.record_first_operator_paint(Instant::now())));
                });
            });
        })
        .map_err(|error| format!("the Presenter probe closed at once: {error:?}"))?;
    // A window the process does not own the foreground for can be throttled, so
    // the measured frames would not be the operator's frames.
    cx.activate(true);
    Ok((window, startup))
}

/// A background memory sampler. It samples every `MEMORY_SAMPLE_INTERVAL` on
/// the background executor, and folds each result into a peak on the GPUI
/// thread. It keeps sampling until `finish`.
struct MemoryWatch {
    peak: Rc<RefCell<PeakMemory>>,
    stop: Rc<Cell<bool>>,
}

impl MemoryWatch {
    fn start(cx: &mut App) -> Result<Self, String> {
        let mut sampler = ProcessSampler::new().map_err(|error| error.to_string())?;
        let watch = Self {
            peak: Rc::default(),
            stop: Rc::default(),
        };
        let (peak, stop) = (watch.peak.clone(), watch.stop.clone());
        let background = cx.background_executor().clone();
        cx.spawn(async move |_| {
            while !stop.get() {
                // The sampler is not shared: it moves to the background task
                // for one sample and comes back with the result.
                let (returned, sample) = background
                    .spawn(async move {
                        let sample = sampler.sample();
                        (sampler, sample)
                    })
                    .await;
                sampler = returned;
                if let Some(sample) = sample {
                    peak.borrow_mut().observe(sample);
                }
                background.timer(MEMORY_SAMPLE_INTERVAL).await;
            }
        })
        .detach();
        Ok(watch)
    }

    /// Stops sampling and returns the peak, folding in `last` if the caller
    /// already took a sample.
    fn finish(&self, last: Option<ProcessSample>) -> PeakMemory {
        self.stop.set(true);
        if let Some(sample) = last {
            self.peak.borrow_mut().observe(sample);
        }
        *self.peak.borrow()
    }
}

fn measure_run(cx: &mut App, out: PathBuf) {
    arm_timeout(cx, out.clone(), timeout_for(MEASURE_COMMANDS));
    // The real operator runtime comes first; the probe opens beside it.
    let runtime = match open_runtime(cx) {
        Ok(runtime) => runtime,
        Err(message) => return finish(cx, &out, error_json(&message), false),
    };
    let watch = match MemoryWatch::start(cx) {
        Ok(watch) => watch,
        Err(message) => return finish(cx, &out, error_json(&message), false),
    };
    let (window, startup) = match open_timed_probe(cx) {
        Ok(opened) => opened,
        Err(message) => return finish(cx, &out, error_json(&message), false),
    };
    let commands = command_stream(BenchConfig::default().seed, PROBE_CUES, MEASURE_COMMANDS);
    let expected = oracle(PROBE_CUES, &commands);
    let done_out = out.clone();
    let started = drive(
        window,
        commands,
        IDLE,
        move |outcome: Result<ProbeOutcome, DriveError>, cx: &mut App| {
            let (document, ok) = match outcome {
                Ok(outcome) => {
                    let peak = watch.finish(outcome.report.process);
                    let oracle_match = outcome.final_state == expected;
                    let document = format!(
                        "{{\"measure\":{},\"oracle_match\":{},\"startup_ms\":{},\"startup_scope\":{}}}",
                        outcome.report.to_json_with_peak(Some(peak)),
                        oracle_match,
                        milliseconds_json(startup.get()),
                        json_string(STARTUP_SCOPE),
                    );
                    (document, oracle_match)
                }
                Err(error) => (error_json(&drive_message(&error)), false),
            };
            finish(cx, &done_out, document, ok);
            // The runtime stays open until the outcome is written.
            drop(runtime);
        },
        cx,
    );
    if let Err(error) = started {
        finish(cx, &out, error_json(&drive_message(&error)), false);
    }
}

fn drive_message(error: &DriveError) -> String {
    match error {
        DriveError::Sampler(error) => format!("cannot sample the process: {error}"),
        DriveError::WindowClosed => "the Presenter probe window is closed".to_owned(),
        DriveError::Unhandled => "a Presenter command never applied".to_owned(),
        DriveError::NotPainted => "a Presenter surface did not paint a command".to_owned(),
        DriveError::Superseded => "the Presenter drive was replaced".to_owned(),
    }
}

fn display_path_run(cx: &mut App, out: PathBuf, passes: usize) {
    arm_timeout(cx, out.clone(), timeout_for(passes));
    let window = match cx.open_window(probe_window_options(cx), |_, cx| {
        cx.new(|_| DisplayPathProbe::default())
    }) {
        Ok(window) => window,
        Err(error) => {
            let message = format!("cannot open the display-path window: {error:?}");
            let document = format!("{{\"display_path\":{}}}", error_json(&message));
            return finish(cx, &out, document, false);
        }
    };
    cx.activate(true);
    let entity = match window.entity(&*cx) {
        Ok(entity) => entity,
        Err(error) => {
            let message = format!("the display-path window closed at once: {error:?}");
            let document = format!("{{\"display_path\":{}}}", error_json(&message));
            return finish(cx, &out, document, false);
        }
    };
    let (sender, results) = async_channel::bounded::<Result<String, String>>(1);
    let done_out = out.clone();
    cx.spawn(async move |cx| {
        if let Ok(result) = results.recv().await {
            cx.update(|cx| match result {
                Ok(json) => finish(cx, &done_out, json, true),
                Err(message) => {
                    let document = format!("{{\"display_path\":{}}}", error_json(&message));
                    finish(cx, &done_out, document, false);
                }
            });
        }
    })
    .detach();
    // `update_window` does not lease the probe entity, which `measure` reads.
    let measured = cx.update_window(window.into(), |_, window, cx| {
        measure(&entity, window, cx, passes, move |result| {
            let result = result
                .map(|measured| measured.to_json())
                .map_err(|error| error.to_string());
            let _ = sender.try_send(result);
        });
    });
    if let Err(error) = measured {
        let message = format!("the display-path window could not be measured: {error:?}");
        let document = format!("{{\"display_path\":{}}}", error_json(&message));
        finish(cx, &out, document, false);
    }
}

fn baseline_run(cx: &mut App, kind: Baseline, out: PathBuf) {
    arm_timeout(cx, out.clone(), timeout_for(kind.commands()));
    let runtime = match open_runtime(cx) {
        Ok(runtime) => runtime,
        Err(message) => return finish(cx, &out, error_json(&message), false),
    };
    let watch = match MemoryWatch::start(cx) {
        Ok(watch) => watch,
        Err(message) => return finish(cx, &out, error_json(&message), false),
    };
    match kind {
        Baseline::Empty => sample_after_settle(cx, out, kind, watch, runtime, String::new()),
        Baseline::Preparation => build_on_background(cx, synthetic_library, move |library, cx| {
            sample_after_settle(cx, out, kind, watch, (runtime, library), String::new());
        }),
        Baseline::Presenter => presenter_baseline(cx, out, kind, runtime, watch),
    }
}

/// Opens the probe, builds the cache contents off the GPUI thread, then drives
/// the baseline commands and samples.
fn presenter_baseline(
    cx: &mut App,
    out: PathBuf,
    kind: Baseline,
    runtime: NativeRuntime,
    watch: MemoryWatch,
) {
    let (window, _) = match open_timed_probe(cx) {
        Ok(opened) => opened,
        Err(message) => return finish(cx, &out, error_json(&message), false),
    };
    build_on_background(cx, build_frame_caches, move |caches, cx| {
        let caches = match caches {
            Ok(caches) => caches,
            Err(message) => return finish(cx, &out, error_json(&message), false),
        };
        let commands = command_stream(BenchConfig::default().seed, PROBE_CUES, BASELINE_COMMANDS);
        let done_out = out.clone();
        let detail = caches.summary_json();
        let started = drive(
            window,
            commands,
            Duration::ZERO,
            move |result: Result<ProbeOutcome, DriveError>, cx: &mut App| match result {
                Ok(_) => sample_after_settle(cx, done_out, kind, watch, (runtime, caches), detail),
                Err(error) => finish(cx, &done_out, error_json(&drive_message(&error)), false),
            },
            cx,
        );
        if let Err(error) = started {
            finish(cx, &out, error_json(&drive_message(&error)), false);
        }
    });
}

/// Runs `build` on the background executor, then `next` on the GPUI thread
/// with its result. The GPUI thread never builds the value.
fn build_on_background<T: Send + 'static>(
    cx: &mut App,
    build: impl FnOnce() -> T + Send + 'static,
    next: impl FnOnce(T, &mut App) + 'static,
) {
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let built = background.spawn(async move { build() }).await;
        cx.update(move |cx| next(built, cx));
    })
    .detach();
}

/// Opens the real operator runtime against a temporary preferences file. The
/// file is removed when the lab finishes.
fn open_runtime(cx: &mut App) -> Result<NativeRuntime, String> {
    let path = std::env::temp_dir().join(format!(
        "ordo-presenter-lab-{}-preferences",
        std::process::id()
    ));
    // Recorded before the runtime opens, so every exit path removes it.
    let _ = TEMP_PREFERENCES.set(path.clone());
    open_native_runtime_at(cx, &path)
        .map_err(|error| format!("cannot open the operator runtime: {error:?}"))
}

/// The synthetic library the Preparation stand-in holds. The real library
/// does not exist yet, so this stands in for its resident size.
fn synthetic_library() -> Vec<String> {
    (0..LIBRARY_ENTRIES)
        .map(|index| {
            let mut entry = format!("entry {index} ");
            entry.extend(std::iter::repeat_n('a', LIBRARY_ENTRY_BYTES));
            entry
        })
        .collect()
}

/// The caches the Presenter baseline holds.
struct HeldFrames {
    frames: FrameCache<usize>,
    thumbnails: ThumbnailCache<usize>,
}

impl HeldFrames {
    /// The cache fields of the baseline document, as a JSON fragment that
    /// starts with a comma. The held bytes show the bounds in effect.
    fn summary_json(&self) -> String {
        format!(
            ",\"caches\":{{\"cued_frames\":{CACHED_FRAMES},\"frame_cache_budget_bytes\":{},\"frame_cache_held_bytes\":{},\"frames_resident\":{},\"thumbnail_cache_held_bytes\":{},\"thumbnails_resident\":{}}}",
            self.frames.budget(),
            self.frames.held_bytes(),
            self.frames.len(),
            self.thumbnails.held_bytes(),
            self.thumbnails.len(),
        )
    }
}

/// Pushes the Cue frames through the bounded full-resolution cache and the
/// thumbnail cache, then releases the thumbnails outside the visible window.
fn build_frame_caches() -> Result<HeldFrames, String> {
    let mut frames = FrameCache::with_budget(FRAME_CACHE_BUDGET);
    let mut thumbnails = ThumbnailCache::new();
    for cue in 0..CACHED_FRAMES {
        let frame = synthetic_canonical_frame(cue)?;
        thumbnails.insert(cue, &frame, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
        frames
            .insert(cue, frame)
            .map_err(|error| format!("cannot cache Cue {cue}: {error:?}"))?;
    }
    thumbnails.retain_visible(VISIBLE_FIRST..VISIBLE_FIRST + VISIBLE_THUMBNAILS);
    Ok(HeldFrames { frames, thumbnails })
}

/// A cheap synthetic canonical frame: a flat fill whose value depends on the
/// Cue, so each frame is distinct.
fn synthetic_canonical_frame(cue: usize) -> Result<ProgramFrame, String> {
    let value = (cue % 256) as u8;
    ProgramFrame::new(
        cue as u64,
        CANONICAL_WIDTH,
        CANONICAL_HEIGHT,
        vec![value; CANONICAL_FRAME_BYTES],
    )
    .map_err(|error| format!("cannot build canonical frame {cue}: {error:?}"))
}

/// Waits for the settle time, then samples process memory on a background
/// task. `held` stays alive until the sample is taken, so the baseline counts
/// what the mode holds. The watch's peak is folded with that sample.
fn sample_after_settle(
    cx: &mut App,
    out: PathBuf,
    kind: Baseline,
    watch: MemoryWatch,
    held: impl Any + 'static,
    detail: String,
) {
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let _held = held;
        background.timer(SETTLE).await;
        let sample = background.spawn(async { sample_process() }).await;
        cx.update(move |cx| match sample {
            Ok(sample) => {
                let peak = watch.finish(Some(sample));
                finish(
                    cx,
                    &out,
                    baseline_document(kind, sample, peak, &detail),
                    true,
                );
            }
            Err(message) => finish(cx, &out, error_json(&message), false),
        });
    })
    .detach();
}

/// The baseline document. `detail` is a JSON fragment that starts with a
/// comma, for the mode's own fields.
fn baseline_document(
    kind: Baseline,
    sample: ProcessSample,
    peak: PeakMemory,
    detail: &str,
) -> String {
    format!(
        "{{\"mode\":{},\"description\":{},\"rss_bytes\":{},\"private_bytes\":{},\"private_bytes_note\":{},\"virtual_bytes\":{},\"peak_rss_bytes\":{},\"peak_private_bytes\":{}{detail}}}",
        json_string(kind.mode()),
        json_string(kind.description()),
        sample.rss_bytes,
        optional_u64(sample.private_bytes),
        json_string(PRIVATE_BYTES_NOTE),
        sample.virtual_bytes,
        peak.peak_rss_bytes,
        optional_u64(peak.peak_private_bytes),
    )
}

/// One process memory sample, taken on the calling background task. The
/// sampler is created there, because it is not thread-safe to share.
fn sample_process() -> Result<ProcessSample, String> {
    let mut sampler =
        ProcessSampler::new().map_err(|error| format!("cannot sample process memory: {error}"))?;
    sampler
        .sample()
        .ok_or_else(|| "process memory was not sampled".to_owned())
}
