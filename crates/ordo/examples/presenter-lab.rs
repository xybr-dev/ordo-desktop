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
//! - `--measure <out.json>` drives the seeded 2,000-command stream through the
//!   probe, idles, and writes the report with the oracle check.
//! - `--display-path <out.json> [passes]` compares the CPU raster and retained
//!   display paths for one canonical frame in a real window.
//! - `--baseline <empty|preparation|presenter> <out.json>` samples process
//!   memory of the empty operator shell, the shell plus a synthetic 10,000-entry
//!   library, or the probe window with a 500-Cue deck after 200 commands.
//!
//! Every measuring mode quits itself. A mode that has not finished within 120
//! seconds writes an error document and exits nonzero, so CI cannot hang. Only
//! the output path the caller names is written. The operator runtime's
//! installation preferences go to a temporary file, so the lab never touches
//! real Parish data.

use std::{
    any::Any,
    cell::Cell,
    fmt::Write as _,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    time::{Duration, Instant},
};

use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowHandle, WindowOptions, px, size};
use gpui_platform::application;
use ordo::{
    open_native_runtime_at,
    performance::{
        DriveError, PresenterProbe, ProbeOutcome, ProcessSample, ProcessSampler, StartupClock,
        bench::{BenchConfig, command_stream},
        drive,
        native_display_path::{DisplayPathProbe, measure},
        open_probe,
    },
};
use presentation::presenter::{PresenterEngine, PresenterState};

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
/// A measuring mode that has not finished by then writes an error and exits.
const MEASURE_TIMEOUT: Duration = Duration::from_secs(120);
/// Entries in the synthetic library the Preparation baseline holds.
const LIBRARY_ENTRIES: usize = 10_000;
const LIBRARY_ENTRY_BYTES: usize = 512;

/// Set by the first `finish`, so a late timeout cannot overwrite a result.
static FINISHED: AtomicBool = AtomicBool::new(false);
/// The process exit status, set when a measurement fails.
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Copy, Debug)]
enum Baseline {
    Empty,
    Preparation,
    Presenter,
}

impl Baseline {
    fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Preparation => "preparation",
            Self::Presenter => "presenter",
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
/// process exit nonzero.
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

/// Fails the run with an error document if it has not finished in time.
fn arm_timeout(cx: &mut App, out: PathBuf) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(MEASURE_TIMEOUT).await;
        cx.update(|cx| {
            let message = format!("timed out after {} s", MEASURE_TIMEOUT.as_secs());
            finish(cx, &out, error_json(&message), false);
        });
    })
    .detach();
}

/// Whether two states show the same position. Revision is excluded because a
/// `GoTo` expands into one arrow key per Cue step.
fn same_position(a: &PresenterState, b: &PresenterState) -> bool {
    a.preview() == b.preview()
        && a.program() == b.program()
        && a.frozen() == b.frozen()
        && a.blackout() == b.blackout()
}

fn oracle(cues: usize, commands: &[presentation::presenter::PresenterCommand]) -> PresenterState {
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

/// A probe window and the cell its first frame's time from process start is
/// written into.
type TimedProbe = (WindowHandle<PresenterProbe>, Rc<Cell<Option<Duration>>>);

/// Opens the probe and records when its first frame is drawn, measured from
/// process start.
fn open_timed_probe(cx: &mut App) -> Result<TimedProbe, String> {
    let window = open_probe(cx, probe_window_options(cx), PROBE_CUES)
        .map_err(|error| format!("cannot open the Presenter probe: {error:?}"))?;
    let first_frame = Rc::new(Cell::new(None));
    let slot = first_frame.clone();
    window
        .update(cx, |_, window, _| {
            window.on_next_frame(move |_, _| {
                let mut clock = StartupClock::new();
                slot.set(Some(clock.record_first_operator_paint(Instant::now())));
            });
        })
        .map_err(|error| format!("the Presenter probe closed at once: {error:?}"))?;
    // A window the process does not own the foreground for can be throttled, so
    // the measured frames would not be the operator's frames.
    cx.activate(true);
    Ok((window, first_frame))
}

fn measure_run(cx: &mut App, out: PathBuf) {
    arm_timeout(cx, out.clone());
    let (window, first_frame) = match open_timed_probe(cx) {
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
        move |outcome: ProbeOutcome, cx: &mut App| {
            let oracle_match = same_position(&outcome.final_state, &expected);
            let document = format!(
                "{{\"measure\":{},\"oracle_match\":{},\"startup_ms\":{}}}",
                outcome.report.to_json(),
                oracle_match,
                milliseconds_json(first_frame.get()),
            );
            finish(cx, &done_out, document, oracle_match);
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
    }
}

fn display_path_run(cx: &mut App, out: PathBuf, passes: usize) {
    arm_timeout(cx, out.clone());
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
    let _ = cx.update_window(window.into(), |_, window, cx| {
        measure(&entity, window, cx, passes, move |result| {
            let result = result
                .map(|measured| measured.to_json())
                .map_err(|error| error.to_string());
            let _ = sender.try_send(result);
        });
    });
}

fn baseline_run(cx: &mut App, kind: Baseline, out: PathBuf) {
    arm_timeout(cx, out.clone());
    match kind {
        Baseline::Empty => {
            let runtime = match open_runtime(cx) {
                Ok(runtime) => runtime,
                Err(message) => return finish(cx, &out, error_json(&message), false),
            };
            sample_after_settle(cx, out, kind, runtime);
        }
        Baseline::Preparation => {
            let runtime = match open_runtime(cx) {
                Ok(runtime) => runtime,
                Err(message) => return finish(cx, &out, error_json(&message), false),
            };
            sample_after_settle(cx, out, kind, (runtime, synthetic_library()));
        }
        Baseline::Presenter => {
            let (window, _) = match open_timed_probe(cx) {
                Ok(opened) => opened,
                Err(message) => return finish(cx, &out, error_json(&message), false),
            };
            let commands =
                command_stream(BenchConfig::default().seed, PROBE_CUES, BASELINE_COMMANDS);
            let done_out = out.clone();
            let started = drive(
                window,
                commands,
                Duration::ZERO,
                move |_: ProbeOutcome, cx: &mut App| {
                    sample_after_settle(cx, done_out, kind, ());
                },
                cx,
            );
            if let Err(error) = started {
                finish(cx, &out, error_json(&drive_message(&error)), false);
            }
        }
    }
}

/// Opens the real operator runtime against a temporary preferences file.
fn open_runtime(cx: &mut App) -> Result<ordo::NativeRuntime, String> {
    let preferences = std::env::temp_dir().join(format!(
        "ordo-presenter-lab-{}-preferences",
        std::process::id()
    ));
    open_native_runtime_at(cx, &preferences)
        .map_err(|error| format!("cannot open the operator runtime: {error:?}"))
}

/// The synthetic library the Preparation baseline holds. The real library
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

/// Waits for the settle time, then samples process memory on a background
/// task. `held` stays alive until the sample is taken, so the baseline counts
/// what the mode holds.
fn sample_after_settle(cx: &mut App, out: PathBuf, kind: Baseline, held: impl Any + 'static) {
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let _held = held;
        background.timer(SETTLE).await;
        let sample = background.spawn(async { sample_process() }).await;
        cx.update(|cx| match sample {
            Ok(sample) => {
                let document = format!(
                    "{{\"mode\":{},\"rss_bytes\":{},\"virtual_bytes\":{}}}",
                    json_string(kind.name()),
                    sample.rss_bytes,
                    sample.virtual_bytes,
                );
                finish(cx, &out, document, true);
            }
            Err(message) => finish(cx, &out, error_json(&message), false),
        });
    })
    .detach();
}

/// One process memory sample. The sampler is created and used on the calling
/// background task, because it is not thread-safe to share.
fn sample_process() -> Result<ProcessSample, String> {
    let mut sampler =
        ProcessSampler::new().map_err(|error| format!("cannot sample process memory: {error}"))?;
    sampler
        .sample()
        .ok_or_else(|| "process memory was not sampled".to_owned())
}
