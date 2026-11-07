//! `cargo xtask measure-presenter`: runs the Presenter performance acceptance
//! in release and leaves its measurement document at
//! `target/ordo-performance/report.json`, where CI uploads it from.
//!
//! The engine bench (`tests/performance`) runs first and writes the document.
//! Then the native measurement runs the `presenter-lab` example, a real GPUI
//! window, as one fresh release process per measurement: the keydown-to-paint
//! run, the display-path comparison, and three runs of each memory baseline.
//! The `live-control-lab` example then measures the real Presenter at 500 Cues
//! in three more fresh processes (T1.3d), judged against the Phase 0 envelope.
//! One more run of the same example measures the animation frame pacing
//! (`--measure-animations`, #111) against the display's measured refresh.
//! Its results are merged under the document's `native` key, and the command
//! fails if any native budget is missed.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::XtaskResult;

/// Where the measurement document is written, relative to the repository root.
pub const REPORT_RELATIVE_PATH: &str = "target/ordo-performance/report.json";

/// The environment variable that tells `performance-tests` where to write.
pub const REPORT_ENV: &str = "ORDO_PERFORMANCE_REPORT";

/// The example the native measurement runs.
const LAB_EXAMPLE: &str = "presenter-lab";
/// The example that measures the real Presenter (T1.3d), one fresh process per
/// run, with `--measure-memory`.
const LIVE_LAB_EXAMPLE: &str = "live-control-lab";

/// Every keydown-to-paint p95 must be strictly below this.
const P95_LIMIT_MS: f64 = 50.0;
/// Process memory above this fails the qualification budget. Baselines only.
const QUALIFICATION_BYTES: u64 = 512 * MIB;
/// Process memory above this fails the absolute release ceiling. Both the
/// keydown-to-paint run and the baselines are judged against it.
const CEILING_BYTES: u64 = GIB;
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// The documented run-to-run noise tolerance, in percent: the envelope is the
/// same-run Phase 0 figure times 1.10.
const ENVELOPE_TOLERANCE_PERCENT: u64 = 10;

/// `value` plus [`ENVELOPE_TOLERANCE_PERCENT`].
const fn with_tolerance(value: u64) -> u64 {
    value + value * ENVELOPE_TOLERANCE_PERCENT / 100
}

/// The same-run Phase 0 envelope for the real Presenter at 500 Cues.
///
/// The origin of the Phase 0 probe is `docs/plans/T0.6-implementation-plan.md`
/// (the old `presenter` baseline). Owner decision, 2026-11-04: the T1.3
/// acceptance "A 500-Cue fixture remains within the Phase 0 measured memory
/// envelope" compares against that same probe measured in the same report, not
/// the fixed T0.6 numbers, because they come from another day and setup (the
/// probe read 273.5 MiB on the owner's machine and about 194.5 MiB on the CI
/// runner with the software renderer, against 198.5 MiB recorded). The
/// envelope is the highest `private_bytes` and the highest `peak_private_bytes`
/// across the `presenter` baseline runs, each times 1.10. A report without
/// every presenter baseline sampled has no envelope and fails; the bound is
/// never loosened to fit a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Envelope {
    private: u64,
    peak: u64,
}

const NO_BASELINE_MESSAGE: &str = "no same-run Phase-0 presenter baseline to compare against";

/// Derives the [`Envelope`] from the `presenter` baseline runs of `native`.
/// `Err` carries the failure message when any run is missing, errored or
/// unsampled.
fn same_run_envelope(native: &Value) -> Result<Envelope, String> {
    let runs = native["baselines"]
        .as_array()
        .filter(|runs| runs.len() >= BASELINE_RUNS)
        .ok_or_else(|| NO_BASELINE_MESSAGE.to_owned())?;
    let (mut private, mut peak) = (0, 0);
    for run in runs {
        let sample = &run["presenter"];
        if error_message(sample).is_some() {
            return Err(NO_BASELINE_MESSAGE.to_owned());
        }
        let (Some(run_private), Some(run_peak)) = (
            sample[LIVE_PRIVATE_KEY].as_u64(),
            sample[LIVE_PEAK_KEY].as_u64(),
        ) else {
            return Err(NO_BASELINE_MESSAGE.to_owned());
        };
        private = private.max(run_private);
        peak = peak.max(run_peak);
    }
    Ok(Envelope {
        private: with_tolerance(private),
        peak: with_tolerance(peak),
    })
}

/// Each live-presenter run is its own process, like the baselines.
const LIVE_PRESENTER_RUNS: usize = 3;
/// Memory fields the live-presenter runs are judged on.
const LIVE_PRIVATE_KEY: &str = "private_bytes";
const LIVE_PEAK_KEY: &str = "peak_private_bytes";
/// A real run has forward, reverse and random phases.
const LIVE_MIN_PHASES: usize = 3;
/// The spec: at least this many Cues, and at least this many commands in
/// every phase, so a run that did nothing cannot pass.
const LIVE_MIN_CUES: u64 = 500;
const LIVE_MIN_PHASE_COMMANDS: u64 = 500;

/// The commands and surfaces every keydown-to-paint run must measure. A pair
/// missing from the document is a failure, not a pass by omission.
const LATENCY_COMMANDS: [&str; 4] = ["navigation", "freeze", "blackout", "space"];
const LATENCY_SURFACES: [&str; 2] = ["preview", "program"];

/// Baseline runs, each a fresh process per mode.
const BASELINE_RUNS: usize = 3;
/// The three baseline modes, in the order the document lists them. Each pair
/// is the lab's mode name and the key it is stored under.
const BASELINE_MODES: [(&str, &str); 3] = [
    ("empty", "empty_shell"),
    ("preparation", "preparation"),
    ("presenter", "presenter"),
];

/// Memory fields the keydown-to-paint run is judged on, in preference order.
/// Peak values win when the document has them, so a spike between the
/// current readings still counts.
const MEASURE_MEMORY_KEYS: [&str; 4] = [
    "peak_private_bytes",
    "peak_rss_bytes",
    "private_bytes",
    "rss_bytes",
];
/// Memory fields a baseline sample is judged on, in preference order.
const BASELINE_MEMORY_KEYS: [&str; 2] = ["private_bytes", "rss_bytes"];

/// The keydown-to-paint run gets this long. Its 2,000 commands at about two
/// frames each take about 70 s at 60 Hz, and CI runners can be slow.
const MEASURE_TIMEOUT: Duration = Duration::from_secs(600);
/// Every other lab process gets this long.
const LAB_TIMEOUT: Duration = Duration::from_secs(180);
/// A live-presenter run drives about 2,700 commands, 4 ms apart or more, and
/// settles several times; the lab's own 420 s timeout fires before this kill.
const LIVE_TIMEOUT: Duration = Duration::from_secs(480);
/// The animation run takes about 50 s; the lab's own 90 s timeout fires before
/// this kill.
const ANIMATIONS_TIMEOUT: Duration = Duration::from_secs(100);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The absolute location of the measurement document under `root`.
pub fn report_path(root: &Path) -> PathBuf {
    root.join(REPORT_RELATIVE_PATH)
}

/// Runs the release acceptance, then the native measurement, and checks both.
pub fn measure_presenter(root: &Path) -> XtaskResult<()> {
    let report = report_path(root);
    if let Some(directory) = report.parent() {
        fs::create_dir_all(directory)?;
    }
    if report.is_file() {
        fs::remove_file(&report)?;
    }

    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "performance-tests",
            "--release",
            "--locked",
            "--",
            "--nocapture",
        ])
        .env(REPORT_ENV, &report)
        .current_dir(root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err("command failed: cargo test -p performance-tests --release --locked".into());
    }
    if !report.is_file() {
        return Err(format!("the acceptance did not write {}", report.display()).into());
    }

    let native = measure_native(root)?;
    for line in summary_lines(&native) {
        println!("{line}");
    }
    let mut document: Value = serde_json::from_str(&fs::read_to_string(&report)?)?;
    merge_native(&mut document, native.clone())?;
    fs::write(&report, serde_json::to_string_pretty(&document)?)?;
    println!("Presenter performance report: {}", report.display());

    let failures = native_failures(&native);
    if failures.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Presenter native measurement missed its budget or did not finish:\n  - {}",
        failures.join("\n  - ")
    )
    .into())
}

/// Builds the lab in release, then runs each native measurement as its own
/// process. Every result is kept, including failures, so one failed run does
/// not hide the others.
fn measure_native(root: &Path) -> XtaskResult<Value> {
    let executable = build_example(root, LAB_EXAMPLE)?;
    let scratch =
        std::env::temp_dir().join(format!("ordo-measure-presenter-{}", std::process::id()));
    fs::create_dir_all(&scratch)?;

    let measure = run_lab(
        &executable,
        &["--measure"],
        &scratch,
        "measure",
        MEASURE_TIMEOUT,
    );
    let display = run_lab(
        &executable,
        &["--display-path"],
        &scratch,
        "display-path",
        LAB_TIMEOUT,
    );
    let display_path = display.get("display_path").cloned().unwrap_or(display);

    let mut baselines = Vec::with_capacity(BASELINE_RUNS);
    for run in 1..=BASELINE_RUNS {
        let mut entry = serde_json::Map::new();
        entry.insert("run".to_owned(), json!(run));
        for (mode, key) in BASELINE_MODES {
            let label = format!("baseline-{mode}-{run}");
            let document = run_lab(
                &executable,
                &["--baseline", mode],
                &scratch,
                &label,
                LAB_TIMEOUT,
            );
            entry.insert(key.to_owned(), document);
        }
        baselines.push(Value::Object(entry));
    }

    // A build failure is a recorded failed run, not an abort: the presenter-lab
    // results stay in the document and the scratch directory is still cleaned.
    let live_presenter = measure_live_presenter(root, &scratch)
        .unwrap_or_else(|error| json!([{"run": 1, "error": error.to_string()}]));

    let animations = measure_animations(root, &scratch)
        .unwrap_or_else(|error| json!({"error": error.to_string()}));

    // Best effort: the scratch files are not part of the document.
    let _ = fs::remove_dir_all(&scratch);
    Ok(json!({
        "measure": measure,
        "display_path": display_path,
        "baselines": baselines,
        "live_presenter": live_presenter,
        "animations": animations,
    }))
}

/// Runs `live-control-lab --measure-animations` once at 1366x768, 100%, 500
/// Cues in the Grid view. The lab measures the display refresh first, then
/// each animation. A build or run failure is an `{"error": ...}` document.
fn measure_animations(root: &Path, scratch: &Path) -> XtaskResult<Value> {
    let executable = build_example(root, LIVE_LAB_EXAMPLE)?;
    // The output path is appended last, so the flag that takes it ends the list.
    Ok(run_lab(
        &executable,
        &[
            "--size",
            "1366x768",
            "--scale",
            "100",
            "--cues",
            "500",
            "--measure-animations",
        ],
        scratch,
        "animations",
        ANIMATIONS_TIMEOUT,
    ))
}

/// Builds `live-control-lab` in release and runs `--measure-memory` in
/// [`LIVE_PRESENTER_RUNS`] fresh processes. Each document keeps its `run`
/// number, and a failed run is an `{"error": ...}` document.
fn measure_live_presenter(root: &Path, scratch: &Path) -> XtaskResult<Value> {
    let executable = build_example(root, LIVE_LAB_EXAMPLE)?;
    let mut runs = Vec::with_capacity(LIVE_PRESENTER_RUNS);
    for run in 1..=LIVE_PRESENTER_RUNS {
        let label = format!("live-presenter-{run}");
        let mut document = run_lab(
            &executable,
            &["--measure-memory"],
            scratch,
            &label,
            LIVE_TIMEOUT,
        );
        if let Some(object) = document.as_object_mut() {
            object.insert("run".to_owned(), json!(run));
        }
        runs.push(document);
    }
    Ok(Value::Array(runs))
}

/// Builds an `ordo` example in release, with the same `--locked` pin as the
/// rest of the workspace, and returns the executable cargo reports. Asking
/// cargo honours every source of the target directory: `CARGO_TARGET_DIR`,
/// the repository's `.cargo/config.toml`, and the user's cargo config.
pub(crate) fn build_example(root: &Path, example: &str) -> XtaskResult<PathBuf> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "ordo",
            "--example",
            example,
            "--locked",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(root)
        .stderr(std::process::Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "command failed: cargo build --release -p ordo --example {example} --locked"
        )
        .into());
    }
    example_executable(&String::from_utf8_lossy(&output.stdout), example)
        .ok_or_else(|| format!("cargo build did not report the {example} executable").into())
}

/// The named example's executable in cargo's JSON build messages, if any.
fn example_executable(messages: &str, example: &str) -> Option<PathBuf> {
    messages
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| message["target"]["name"] == example)
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
}

/// Runs one lab process and returns its document. A process still running
/// after `timeout` is killed, and one that fails without a readable document
/// yields an `{"error": ...}` document, so every run has a result to report.
/// Every path that leaves the child running is followed by a kill and a wait,
/// so the child is always reaped.
fn run_lab(
    executable: &Path,
    args: &[&str],
    scratch: &Path,
    label: &str,
    timeout: Duration,
) -> Value {
    let out = scratch.join(format!("{label}.json"));
    if out.is_file() {
        let _ = fs::remove_file(&out);
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .arg(&out)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return json!({"error": format!("cannot start {}: {error}", executable.display())});
        }
    };

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return json!({
                    "error": format!("timed out after {} s and was killed", timeout.as_secs())
                });
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return json!({"error": format!("cannot wait for the process: {error}")});
            }
        }
    };

    // The lab writes its document before it exits, even when it fails, so a
    // readable document is the result whatever the exit status.
    let parsed = fs::read_to_string(&out)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|error| error.to_string()));
    match parsed {
        Ok(document) => document,
        Err(message) => json!({"error": format!("exited with {status}; no document: {message}")}),
    }
}

/// The message of an `{"error": ...}` document, if it is one.
fn error_message(document: &Value) -> Option<&str> {
    document.get("error").and_then(Value::as_str)
}

/// One memory reading and the document field it came from, so every message
/// and summary line says which counter was judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Reading {
    bytes: u64,
    source: &'static str,
}

/// The first of `keys` the sample reports, with its field name.
fn read_memory(sample: &Value, keys: &[&'static str]) -> Option<Reading> {
    keys.iter().find_map(|&key| {
        sample
            .get(key)
            .and_then(Value::as_u64)
            .map(|bytes| Reading { bytes, source: key })
    })
}

/// Failures for one reading against the 1 GiB absolute ceiling. A missing
/// reading fails, because an unmeasured ceiling is not a passing one.
fn ceiling_failures(label: &str, reading: Option<Reading>) -> Vec<String> {
    match reading {
        None => vec![format!("{label}: process memory was not sampled")],
        Some(Reading { bytes, source }) if bytes > CEILING_BYTES => vec![format!(
            "{label}: {} MiB ({source}) is over the 1 GiB absolute ceiling",
            bytes / MIB
        )],
        Some(_) => Vec::new(),
    }
}

/// Failures for one baseline reading against the 512 MiB qualification budget
/// and the 1 GiB absolute ceiling. A missing reading fails once.
fn memory_failures(label: &str, reading: Option<Reading>) -> Vec<String> {
    let Some(Reading { bytes, source }) = reading else {
        return ceiling_failures(label, None);
    };
    let mut failures = Vec::new();
    if bytes > QUALIFICATION_BYTES {
        failures.push(format!(
            "{label}: {} MiB ({source}) is over the 512 MiB qualification budget",
            bytes / MIB
        ));
    }
    failures.extend(ceiling_failures(label, Some(Reading { bytes, source })));
    failures
}

/// Every reason the native results miss the budget, or are missing. Empty
/// when the measurement passes.
pub fn native_failures(native: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    failures.extend(measure_failures(&native["measure"]));

    match error_message(&native["display_path"]) {
        Some(message) => failures.push(format!("display path: {message}")),
        None if native["display_path"].is_null() => {
            failures.push("display path: no result was recorded".to_owned());
        }
        None => {}
    }

    match native["baselines"].as_array() {
        Some(runs) => {
            for run in runs {
                let number = run["run"].as_u64().unwrap_or(0);
                for (_, key) in BASELINE_MODES {
                    let label = format!("baseline {key} run {number}");
                    let sample = &run[key];
                    match error_message(sample) {
                        Some(message) => failures.push(format!("{label}: {message}")),
                        None => failures.extend(memory_failures(
                            &label,
                            read_memory(sample, &BASELINE_MEMORY_KEYS),
                        )),
                    }
                }
            }
        }
        None => failures.push("baselines: no runs were recorded".to_owned()),
    }
    failures.extend(live_presenter_failures(native));
    failures.extend(animation_failures(native));
    failures
}

/// Interval p95 may be at most this many refresh intervals.
const ANIMATION_P95_REFRESH_FACTOR: f64 = 1.5;
/// No interval of an animation may be longer than this (a hitch).
const ANIMATION_HITCH_MS: f64 = 33.0;

/// Failures for the animation frame pacing (#111). Each animation that applied
/// is judged against the display refresh the lab measured in the same run:
/// no interval over 33 ms, and an interval p95 of at most 1.5 refresh
/// intervals. A missing or errored document, a missing refresh, or an
/// animation without intervals fails, because an unmeasured run is not a
/// passing one. The p95 frame cost against the refresh is reported in the
/// summary and not judged: the remaining per-frame cost is tracked in #111.
fn animation_failures(native: &Value) -> Vec<String> {
    let document = &native["animations"];
    if document.is_null() {
        return vec!["animations: no result was recorded".to_owned()];
    }
    if let Some(message) = error_message(document) {
        return vec![format!("animations: {message}")];
    }
    let Some(refresh) = document["refresh_ms"].as_f64().filter(|ms| *ms > 0.0) else {
        return vec![format!(
            "animations: the display refresh was not measured ({})",
            document["refresh_note"].as_str().unwrap_or("no note")
        )];
    };
    let Some(animations) = document["animations"]
        .as_array()
        .filter(|animations| !animations.is_empty())
    else {
        return vec!["animations: no animation was recorded".to_owned()];
    };
    let mut failures = Vec::new();
    for animation in animations {
        if animation["applicable"].as_bool() != Some(true) {
            continue;
        }
        let name = animation["name"].as_str().unwrap_or("unnamed");
        let label = format!("animation {name}");
        if animation["intervals"].as_u64().unwrap_or(0) == 0 {
            failures.push(format!("{label}: no frame intervals were measured"));
            continue;
        }
        match animation["over_33ms"].as_u64() {
            Some(0) => {}
            Some(count) => failures.push(format!(
                "{label}: {count} frame intervals over {ANIMATION_HITCH_MS} ms"
            )),
            None => failures.push(format!("{label}: no over_33ms count")),
        }
        let bar = ANIMATION_P95_REFRESH_FACTOR * refresh;
        match animation["interval_ms"]["p95"].as_f64() {
            Some(p95) if p95 <= bar => {}
            Some(p95) => failures.push(format!(
                "{label}: interval p95 {p95:.2} ms is over {bar:.2} ms ({ANIMATION_P95_REFRESH_FACTOR} x the {refresh:.2} ms refresh)"
            )),
            None => failures.push(format!("{label}: no interval p95")),
        }
    }
    failures
}

/// Failures for the real Presenter at 500 Cues (T1.3d). Each run is judged
/// in private bytes against the 512 MiB qualification budget and the Phase 0
/// same-run Phase 0 envelope, at every phase sample (not just the last), and
/// its peak private bytes against the budget and the same-run peak envelope. The envelope is a
/// private-memory figure, so a missing figure fails rather than falling back
/// to RSS. The run must also prove it did the work: live-presenter mode, 500
/// Cues, three phases of at least 500 commands each, every phase settled, and
/// full-size images decoded.
fn live_presenter_failures(native: &Value) -> Vec<String> {
    let Some(runs) = native["live_presenter"].as_array() else {
        return vec!["live presenter: no runs were recorded".to_owned()];
    };
    if runs.is_empty() {
        return vec!["live presenter: no runs were recorded".to_owned()];
    }
    let mut failures = Vec::new();
    let envelope = match same_run_envelope(native) {
        Ok(envelope) => Some(envelope),
        Err(message) => {
            failures.push(format!("live presenter: {message}"));
            None
        }
    };
    for run in runs {
        let label = format!("live presenter run {}", run["run"].as_u64().unwrap_or(0));
        if let Some(message) = error_message(run) {
            failures.push(format!("{label}: {message}"));
            continue;
        }
        if run["mode"].as_str() != Some("live-presenter") {
            failures.push(format!("{label}: mode is not live-presenter"));
        }
        if run["cue_count"].as_u64().unwrap_or(0) < LIVE_MIN_CUES {
            failures.push(format!("{label}: fewer than {LIVE_MIN_CUES} Cues ran"));
        }
        if run["max_full_images"].as_u64().unwrap_or(0) == 0 {
            failures.push(format!(
                "{label}: no full-size image was ever decoded, so image memory was not exercised"
            ));
        }
        for (key, envelope, name) in [
            (
                LIVE_PRIVATE_KEY,
                envelope.map(|envelope| envelope.private),
                "same-run Phase 0 private envelope",
            ),
            (
                LIVE_PEAK_KEY,
                envelope.map(|envelope| envelope.peak),
                "same-run Phase 0 peak private envelope",
            ),
        ] {
            failures.extend(bound_failures(&label, run.get(key), key, envelope, name));
        }
        match run["phases"].as_array() {
            Some(phases) if phases.len() >= LIVE_MIN_PHASES => {
                for (index, phase) in phases.iter().enumerate() {
                    let phase_name = phase["phase"].as_str().unwrap_or("unnamed");
                    let phase_label = format!("{label}, phase {} ({phase_name})", index + 1);
                    if phase["settled"].as_bool() != Some(true) {
                        failures.push(format!("{phase_label}: the frame store did not settle"));
                    }
                    if phase["commands"].as_u64().unwrap_or(0) < LIVE_MIN_PHASE_COMMANDS {
                        failures.push(format!(
                            "{phase_label}: fewer than {LIVE_MIN_PHASE_COMMANDS} commands ran"
                        ));
                    }
                    failures.extend(bound_failures(
                        &phase_label,
                        phase.get(LIVE_PRIVATE_KEY),
                        LIVE_PRIVATE_KEY,
                        envelope.map(|envelope| envelope.private),
                        "same-run Phase 0 private envelope",
                    ));
                }
            }
            _ => failures.push(format!(
                "{label}: fewer than {LIVE_MIN_PHASES} phases were recorded"
            )),
        }
    }
    failures
}

/// Failures for one sampled figure against the qualification budget and one
/// envelope. A figure that is missing or not a number fails.
fn bound_failures(
    label: &str,
    value: Option<&Value>,
    key: &str,
    envelope: Option<u64>,
    name: &str,
) -> Vec<String> {
    let Some(bytes) = value.and_then(Value::as_u64) else {
        return vec![format!("{label}: {key} was not sampled")];
    };
    let mut failures = Vec::new();
    if bytes > QUALIFICATION_BYTES {
        failures.push(format!(
            "{label}: {:.1} MiB ({key}) is over the 512 MiB qualification budget",
            bytes as f64 / MIB as f64
        ));
    }
    if let Some(envelope) = envelope.filter(|envelope| bytes > *envelope) {
        failures.push(format!(
            "{label}: {:.1} MiB ({key}) is over the {name} of {:.1} MiB",
            bytes as f64 / MIB as f64,
            envelope as f64 / MIB as f64
        ));
    }
    failures
}

/// Failures for the keydown-to-paint run: an error, a divergent final state,
/// a missing or unsampled pair, a p95 at or over the limit, memory over the
/// ceiling, or a hot-path violation. The run is judged on the 1 GiB ceiling
/// alone; the 512 MiB budget applies to the baselines.
fn measure_failures(measure: &Value) -> Vec<String> {
    if let Some(message) = error_message(measure) {
        return vec![format!("keydown-to-paint run: {message}")];
    }
    let mut failures = Vec::new();
    if measure["oracle_match"].as_bool() != Some(true) {
        failures.push("keydown-to-paint run: final state diverged from the oracle".to_owned());
    }

    let report = &measure["measure"];
    failures.extend(latency_failures(report));
    failures.extend(ceiling_failures(
        "keydown-to-paint run",
        read_memory(&report["memory"], &MEASURE_MEMORY_KEYS),
    ));
    match report["hot_path_violations"].as_u64() {
        Some(0) => {}
        Some(count) => failures.push(format!(
            "keydown-to-paint run: {count} hot-path violation(s)"
        )),
        None => failures.push("keydown-to-paint run: hot-path violations not recorded".to_owned()),
    }
    failures
}

/// Failures for the latency table: every expected command and surface must be
/// present with at least one sample and a p95 under the limit. The latency
/// scope string is the probe's to define, so it is printed, not checked.
fn latency_failures(report: &Value) -> Vec<String> {
    let Some(pairs) = report["latency"].as_array() else {
        return vec!["keydown-to-paint run: no latency table was recorded".to_owned()];
    };
    let mut failures = Vec::new();
    for command in LATENCY_COMMANDS {
        for surface in LATENCY_SURFACES {
            let label = format!("keydown-to-paint {command} {surface}");
            let found = pairs.iter().find(|pair| {
                pair["command"].as_str() == Some(command)
                    && pair["surface"].as_str() == Some(surface)
            });
            let Some(pair) = found else {
                failures.push(format!("{label}: missing from the latency table"));
                continue;
            };
            if pair["samples"].as_u64().unwrap_or(0) == 0 {
                failures.push(format!("{label}: no samples were recorded"));
                continue;
            }
            match pair["p95_ms"].as_f64() {
                Some(p95) if p95 < P95_LIMIT_MS => {}
                Some(p95) => failures.push(format!(
                    "{label}: p95 {p95:.3} ms is at or over the 50 ms limit"
                )),
                None => failures.push(format!("{label}: no p95 was recorded")),
            }
        }
    }
    failures
}

/// Adds the native results under the `native` key, leaving every existing
/// key of the engine document as it is.
fn merge_native(document: &mut Value, native: Value) -> XtaskResult<()> {
    let object = document
        .as_object_mut()
        .ok_or("the engine measurement document is not a JSON object")?;
    object.insert("native".to_owned(), native);
    Ok(())
}

/// Human-readable lines for the console: the latency scope, per-command p95,
/// the display-path medians, the memory judged, and the renderer.
fn summary_lines(native: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    let report = &native["measure"]["measure"];
    lines.push(format!(
        "keydown-to-paint latency scope: {}",
        report["latency_scope"].as_str().unwrap_or("?")
    ));
    if let Some(pairs) = report["latency"].as_array() {
        for pair in pairs {
            lines.push(format!(
                "keydown-to-paint {} {}: p50 {} ms, p95 {} ms, p99 {} ms over {} samples",
                pair["command"].as_str().unwrap_or("?"),
                pair["surface"].as_str().unwrap_or("?"),
                pair["p50_ms"],
                pair["p95_ms"],
                pair["p99_ms"],
                pair["samples"],
            ));
        }
    }
    lines.push(format!(
        "frames {}, dropped {}, worker queue p95 {} ms, idle CPU {}%, hot-path violations {}",
        report["frames"]["count"],
        report["frames"]["dropped"],
        report["worker_queue"]["p95_ms"],
        report["cpu"]["idle_percent"],
        report["hot_path_violations"],
    ));
    lines.push(format!(
        "keydown-to-paint memory: {}",
        memory_text(&report["memory"], &MEASURE_MEMORY_KEYS)
    ));
    lines.push(format!(
        "renderer: {} (software emulated: {})",
        report["renderer"]["device_name"], report["renderer"]["software_emulated"],
    ));
    lines.push(format!(
        "startup to first probe frame: {} ms; oracle match: {}",
        native["measure"]["startup_ms"], native["measure"]["oracle_match"],
    ));
    let display = &native["display_path"];
    lines.push(format!(
        "display path ({} passes): CPU median {} ms, held {} bytes; retained median {} ms, held {} bytes",
        display["passes"],
        display["cpu"]["median_ms"],
        display["cpu"]["held_bytes"],
        display["retained"]["median_ms"],
        display["retained"]["held_bytes"],
    ));
    if let Some(runs) = native["baselines"].as_array() {
        for run in runs {
            lines.push(format!(
                "baseline run {}: empty {}, preparation {}, presenter {}",
                run["run"],
                memory_text(&run["empty_shell"], &BASELINE_MEMORY_KEYS),
                memory_text(&run["preparation"], &BASELINE_MEMORY_KEYS),
                memory_text(&run["presenter"], &BASELINE_MEMORY_KEYS),
            ));
        }
    }
    if let Some(runs) = native["live_presenter"].as_array() {
        let limits = match same_run_envelope(native) {
            Ok(envelope) => format!(
                "envelope {:.1} / {:.1} MiB = same-run Phase-0 presenter \u{d7} 1.10",
                envelope.private as f64 / MIB as f64,
                envelope.peak as f64 / MIB as f64
            ),
            Err(message) => message,
        };
        for run in runs {
            lines.push(match error_message(run) {
                Some(message) => format!("live presenter run {}: {message}", run["run"]),
                None => format!(
                    "live presenter run {}: private {}, peak private {}, {} Cues, {} frames composed, max cache {}, max images {} full / {} thumbnail ({limits})",
                    run["run"],
                    memory_text(run, &[LIVE_PRIVATE_KEY]),
                    memory_text(run, &[LIVE_PEAK_KEY]),
                    run["cue_count"],
                    run["frames_composed"],
                    run["max_cache_len"],
                    run["max_full_images"],
                    run["max_thumb_images"],
                ),
            });
        }
    }
    lines.push(animation_summary(&native["animations"]));
    lines
}

/// One line for the animation run: the refresh, the worst interval p95 and the
/// animation it belongs to, the intervals over 33 ms, and the worst frame cost
/// p95 (reported, not judged).
fn animation_summary(document: &Value) -> String {
    if document.is_null() {
        return "animations: no result".to_owned();
    }
    if let Some(message) = error_message(document) {
        return format!("animations: {message}");
    }
    let Some(refresh) = document["refresh_ms"].as_f64() else {
        return "animations: refresh not measured".to_owned();
    };
    let applicable: Vec<&Value> = document["animations"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter(|a| a["applicable"].as_bool() == Some(true))
                .collect()
        })
        .unwrap_or_default();
    let worst = |select: fn(&Value) -> Option<f64>| {
        applicable
            .iter()
            .filter_map(|a| select(a).map(|value| (value, a["name"].as_str().unwrap_or("?"))))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    };
    let (p95, name) = worst(|a| a["interval_ms"]["p95"].as_f64()).unwrap_or((0.0, "none"));
    let (cpu, _) = worst(|a| a["cpu_ms"]["p95"].as_f64()).unwrap_or((0.0, "none"));
    let over: u64 = applicable
        .iter()
        .filter_map(|a| a["over_33ms"].as_u64())
        .sum();
    format!(
        "animations: refresh {refresh:.2} ms ({} Hz), worst interval p95 {p95:.2} ms ({name}, bar {:.2} ms), {over} intervals over {ANIMATION_HITCH_MS} ms, worst frame cost p95 {cpu:.2} ms (reported)",
        document["refresh_hz"],
        ANIMATION_P95_REFRESH_FACTOR * refresh
    )
}

/// The reading the gate would judge, in MiB with its field name, or `n/a`.
fn memory_text(sample: &Value, keys: &[&'static str]) -> String {
    match read_memory(sample, keys) {
        Some(Reading { bytes, source }) => {
            format!("{:.1} MiB ({source})", bytes as f64 / MIB as f64)
        }
        None => "n/a".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_lives_under_the_target_directory() {
        let root = Path::new("repo");
        assert_eq!(
            report_path(root),
            Path::new("repo")
                .join("target")
                .join("ordo-performance")
                .join("report.json")
        );
    }

    /// A passing keydown-to-paint document with every expected pair under
    /// budget and memory within the ceiling.
    fn passing_measure() -> Value {
        let pairs: Vec<Value> = LATENCY_COMMANDS
            .iter()
            .flat_map(|command| {
                LATENCY_SURFACES.iter().map(move |surface| {
                    json!({
                        "command": command,
                        "surface": surface,
                        "samples": 10,
                        "p50_ms": 13.0,
                        "p95_ms": 14.0,
                        "p99_ms": 16.0,
                    })
                })
            })
            .collect();
        json!({
            "measure": {
                "latency_scope": "keydown_to_paint",
                "latency": pairs,
                "memory": {"rss_bytes": 300 * MIB, "virtual_bytes": 1, "private_bytes": null},
                "hot_path_violations": 0,
                "frames": {"count": 5, "dropped": 0},
                "worker_queue": {"p95_ms": 0.2},
                "cpu": {"process_percent": 1.0, "idle_percent": 0.5},
                "renderer": {"software_emulated": false, "device_name": "GPU"}
            },
            "oracle_match": true,
            "startup_ms": 900.0
        })
    }

    fn live_phase(name: &str, private: u64) -> Value {
        json!({
            "phase": name,
            "commands": 900,
            "settled": true,
            "private_bytes": private,
        })
    }

    /// A passing run: the final and peak figures as given, three settled
    /// phases comfortably inside the envelope.
    fn live_run(run: u64, private: u64, peak: u64) -> Value {
        json!({
            "run": run,
            "mode": "live-presenter",
            "private_bytes": private,
            "peak_private_bytes": peak,
            "phases": [
                live_phase("forward", 190 * MIB),
                live_phase("reverse", 190 * MIB),
                live_phase("random", 190 * MIB),
            ],
            "cue_count": 500,
            "frames_composed": 120,
            "max_cache_len": 40,
            "max_full_images": 2,
            "max_thumb_images": 3,
        })
    }

    fn passing_native() -> Value {
        let sample = |rss: u64| json!({"mode": "x", "rss_bytes": rss, "virtual_bytes": 1});
        // The same-run Phase 0 presenter: private 180 / 190 / 200 MiB and peak
        // 300 / 310 / 320 MiB, so the envelope is 220 / 352 MiB.
        let presenter = |private: u64, peak: u64| json!({"mode": "x", "rss_bytes": 90 * MIB, "private_bytes": private, "peak_private_bytes": peak, "virtual_bytes": 1});
        json!({
            "measure": passing_measure(),
            "display_path": {"passes": 30, "cpu": {"median_ms": 50.0, "held_bytes": 8}, "retained": {"median_ms": 12.0, "held_bytes": 4}},
            "baselines": [
                {"run": 1, "empty_shell": sample(40 * MIB), "preparation": sample(120 * MIB), "presenter": presenter(180 * MIB, 300 * MIB)},
                {"run": 2, "empty_shell": sample(40 * MIB), "preparation": sample(120 * MIB), "presenter": presenter(190 * MIB, 310 * MIB)},
                {"run": 3, "empty_shell": sample(40 * MIB), "preparation": sample(120 * MIB), "presenter": presenter(200 * MIB, 320 * MIB)},
            ],
            "live_presenter": [
                live_run(1, 190 * MIB, 330 * MIB),
                live_run(2, 190 * MIB, 330 * MIB),
                live_run(3, 190 * MIB, 330 * MIB),
            ],
            "animations": passing_animations()
        })
    }

    fn animation(name: &str, applicable: bool, p95: f64, cpu: f64, over: u64) -> Value {
        json!({"name": name, "applicable": applicable, "intervals": 100, "over_33ms": over,
            "interval_ms": {"p50": 13.3, "p95": p95, "p99": p95, "max": p95},
            "cpu_ms": {"p50": 5.0, "p95": cpu, "p99": cpu, "max": cpu}})
    }

    /// A 75 Hz display: refresh 13.3 ms, so the p95 bar is 19.95 ms.
    fn passing_animations() -> Value {
        json!({"mode": "animation-pacing", "refresh_ms": 13.3, "refresh_hz": 75.0,
        "refresh_samples": 70, "animations": [
            animation("timeline_glide", true, 16.7, 17.1, 0),
            animation("rail_overlay", false, 90.0, 60.0, 9),
            animation("group_fold", true, 14.5, 12.1, 0),
        ]})
    }

    fn pair_mut<'a>(native: &'a mut Value, command: &str, surface: &str) -> &'a mut Value {
        native["measure"]["measure"]["latency"]
            .as_array_mut()
            .expect("latency is an array")
            .iter_mut()
            .find(|pair| pair["command"] == command && pair["surface"] == surface)
            .expect("the pair is in the fixture")
    }

    fn measure_memory_mut(native: &mut Value) -> &mut Value {
        &mut native["measure"]["measure"]["memory"]
    }

    #[test]
    fn a_passing_native_measurement_has_no_failures() {
        assert_eq!(native_failures(&passing_native()), Vec::<String>::new());
    }

    #[test]
    fn a_p95_at_the_limit_fails() {
        let mut native = passing_native();
        pair_mut(&mut native, "navigation", "preview")["p95_ms"] = json!(50.0);
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("p95 50.000 ms"));
    }

    #[test]
    fn a_missing_p95_fails_rather_than_passing() {
        let mut native = passing_native();
        pair_mut(&mut native, "navigation", "preview")["p95_ms"] = Value::Null;
        assert!(
            native_failures(&native)
                .iter()
                .any(|failure| failure.contains("no p95 was recorded"))
        );
    }

    #[test]
    fn every_one_of_the_eight_pairs_is_required() {
        for command in LATENCY_COMMANDS {
            for surface in LATENCY_SURFACES {
                let mut native = passing_native();
                let pairs = native["measure"]["measure"]["latency"]
                    .as_array_mut()
                    .expect("latency is an array");
                pairs.retain(|pair| !(pair["command"] == command && pair["surface"] == surface));
                let failures = native_failures(&native);
                assert_eq!(
                    failures,
                    vec![format!(
                        "keydown-to-paint {command} {surface}: missing from the latency table"
                    )],
                    "dropping {command} {surface}"
                );
            }
        }
    }

    #[test]
    fn a_pair_with_no_samples_fails_even_if_present() {
        let mut native = passing_native();
        let pair = pair_mut(&mut native, "space", "program");
        pair["samples"] = json!(0);
        pair["p50_ms"] = Value::Null;
        pair["p95_ms"] = Value::Null;
        pair["p99_ms"] = Value::Null;
        assert_eq!(
            native_failures(&native),
            vec!["keydown-to-paint space program: no samples were recorded".to_owned()]
        );
    }

    #[test]
    fn the_latency_scope_is_printed_not_judged() {
        let mut native = passing_native();
        native["measure"]["measure"]["latency_scope"] = json!("some_other_scope");
        assert_eq!(native_failures(&native), Vec::<String>::new());
        assert!(
            summary_lines(&native)
                .iter()
                .any(|line| line == "keydown-to-paint latency scope: some_other_scope")
        );
    }

    #[test]
    fn an_oracle_divergence_or_a_hot_path_violation_fails() {
        let mut native = passing_native();
        native["measure"]["oracle_match"] = json!(false);
        native["measure"]["measure"]["hot_path_violations"] = json!(2);
        let failures = native_failures(&native);
        assert!(
            failures.iter().any(|f| f.contains("oracle")),
            "{failures:?}"
        );
        assert!(
            failures.iter().any(|f| f.contains("2 hot-path")),
            "{failures:?}"
        );
    }

    #[test]
    fn a_measure_run_over_the_qualification_budget_but_under_the_ceiling_passes() {
        let mut native = passing_native();
        *measure_memory_mut(&mut native) = json!({"rss_bytes": 600 * MIB});
        assert_eq!(native_failures(&native), Vec::<String>::new());
    }

    #[test]
    fn a_measure_run_over_the_ceiling_fails_on_the_peak_when_reported() {
        let mut native = passing_native();
        *measure_memory_mut(&mut native) =
            json!({"rss_bytes": 300 * MIB, "peak_rss_bytes": GIB + MIB});
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("peak_rss_bytes"));
        assert!(failures[0].contains("absolute ceiling"));
    }

    #[test]
    fn a_measure_peak_private_bytes_outranks_peak_rss() {
        let mut native = passing_native();
        *measure_memory_mut(&mut native) = json!({
            "rss_bytes": 100 * MIB,
            "peak_rss_bytes": 200 * MIB,
            "peak_private_bytes": GIB + MIB,
        });
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("peak_private_bytes"));
    }

    #[test]
    fn a_measure_run_without_a_memory_reading_fails() {
        let mut native = passing_native();
        *measure_memory_mut(&mut native) = json!({"virtual_bytes": 1});
        assert_eq!(
            native_failures(&native),
            vec!["keydown-to-paint run: process memory was not sampled".to_owned()]
        );
    }

    #[test]
    fn baseline_memory_over_the_budget_and_over_the_ceiling_fail_separately() {
        assert!(
            memory_failures(
                "run",
                read_memory(&json!({"rss_bytes": 512 * MIB}), &BASELINE_MEMORY_KEYS)
            )
            .is_empty()
        );
        let over_budget = memory_failures(
            "run",
            read_memory(&json!({"rss_bytes": 600 * MIB}), &BASELINE_MEMORY_KEYS),
        );
        assert_eq!(over_budget.len(), 1, "{over_budget:?}");
        assert!(over_budget[0].contains("qualification budget"));
        assert!(over_budget[0].contains("(rss_bytes)"));
        let over_ceiling = memory_failures(
            "run",
            read_memory(&json!({"rss_bytes": GIB + MIB}), &BASELINE_MEMORY_KEYS),
        );
        assert_eq!(over_ceiling.len(), 2, "{over_ceiling:?}");
        assert!(over_ceiling[1].contains("absolute ceiling"));
        assert_eq!(
            memory_failures("run", None),
            vec!["run: process memory was not sampled".to_owned()]
        );
    }

    #[test]
    fn a_baseline_over_the_budget_fails_the_native_measurement() {
        let mut native = passing_native();
        native["baselines"][2]["presenter"] = json!({"rss_bytes": 600 * MIB, "virtual_bytes": 1});
        let failures = native_failures(&native);
        // The unsampled private figure also leaves no same-run envelope.
        assert_eq!(failures.len(), 2, "{failures:?}");
        assert!(failures[0].starts_with("baseline presenter run 3:"));
        assert!(failures[0].contains("qualification budget"));
        assert!(failures[1].contains("no same-run Phase-0 presenter baseline"));
    }

    #[test]
    fn baseline_private_bytes_are_judged_before_rss() {
        let sample = json!({"rss_bytes": 100 * MIB, "private_bytes": 600 * MIB});
        assert_eq!(
            read_memory(&sample, &BASELINE_MEMORY_KEYS),
            Some(Reading {
                bytes: 600 * MIB,
                source: "private_bytes",
            })
        );
        assert_eq!(
            read_memory(&json!({"rss_bytes": 7}), &BASELINE_MEMORY_KEYS),
            Some(Reading {
                bytes: 7,
                source: "rss_bytes",
            })
        );
        assert_eq!(read_memory(&json!({}), &BASELINE_MEMORY_KEYS), None);
    }

    #[test]
    fn an_errored_mode_or_display_path_fails() {
        let mut native = passing_native();
        native["display_path"] = json!({"error": "retained paint failed"});
        native["baselines"][1]["presenter"] =
            json!({"error": "timed out after 180 s and was killed"});
        let failures = native_failures(&native);
        assert!(failures.iter().any(|f| f.contains("retained paint failed")));
        assert!(
            failures
                .iter()
                .any(|f| f.contains("baseline presenter run 2"))
        );
    }

    #[test]
    fn a_measure_error_document_fails_once() {
        let mut native = passing_native();
        native["measure"] = json!({"error": "timed out after 600 s"});
        assert_eq!(
            native_failures(&native),
            vec!["keydown-to-paint run: timed out after 600 s".to_owned()]
        );
    }

    #[test]
    fn merging_keeps_the_engine_keys_and_adds_native() {
        let mut document = json!({"schema": "ordo.performance.v1", "report": {}, "baselines": []});
        merge_native(&mut document, json!({"measure": {}})).expect("object merges");
        assert_eq!(document["schema"], "ordo.performance.v1");
        assert_eq!(document["baselines"], json!([]));
        assert_eq!(document["native"], json!({"measure": {}}));
    }

    #[test]
    fn merging_into_a_non_object_is_an_error() {
        let mut document = json!([1, 2]);
        assert!(merge_native(&mut document, json!({})).is_err());
    }

    #[test]
    fn the_lab_executable_comes_from_cargo_build_messages() {
        let messages = concat!(
            "{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"ordo\"},\"executable\":null}\n",
            "not json\n",
            "{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"presenter-lab\"},",
            "\"executable\":\"X:/cargo-target/release/examples/presenter-lab.exe\"}\n",
            "{\"reason\":\"build-finished\",\"success\":true}\n",
        );
        assert_eq!(
            example_executable(messages, "presenter-lab"),
            Some(PathBuf::from(
                "X:/cargo-target/release/examples/presenter-lab.exe"
            ))
        );
        assert_eq!(example_executable(messages, "walking-skeleton-lab"), None);
        assert_eq!(
            example_executable("{\"reason\":\"build-finished\"}", "presenter-lab"),
            None
        );
    }

    #[test]
    fn a_missing_lab_executable_yields_an_error_document() {
        let scratch = std::env::temp_dir();
        let document = run_lab(
            Path::new("definitely/not/a/presenter-lab.exe"),
            &["--measure"],
            &scratch,
            "unit-missing",
            LAB_TIMEOUT,
        );
        assert!(
            error_message(&document).is_some_and(|message| message.starts_with("cannot start")),
            "{document}"
        );
    }

    #[test]
    fn the_summary_names_the_p95_the_memory_and_the_renderer() {
        let lines = summary_lines(&passing_native());
        assert!(
            lines
                .iter()
                .any(|line| line.contains("navigation preview: p50 13.0"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "keydown-to-paint memory: 300.0 MiB (rss_bytes)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("renderer: \"GPU\""))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "baseline run 1: empty 40.0 MiB (rss_bytes), preparation 120.0 MiB (rss_bytes), presenter 180.0 MiB (private_bytes)")
        );
    }

    const ENVELOPE_PRIVATE: u64 = 220 * MIB;
    const ENVELOPE_PEAK: u64 = 352 * MIB;

    #[test]
    fn the_envelope_is_the_highest_same_run_presenter_baseline_plus_ten_percent() {
        let envelope = same_run_envelope(&passing_native()).unwrap();
        assert_eq!(
            envelope,
            Envelope {
                private: ENVELOPE_PRIVATE,
                peak: ENVELOPE_PEAK
            }
        );
    }

    #[test]
    fn live_presenter_runs_at_the_envelope_pass() {
        let mut native = passing_native();
        native["live_presenter"][0] = live_run(1, ENVELOPE_PRIVATE, ENVELOPE_PEAK);
        native["live_presenter"][0]["phases"][1]["private_bytes"] = json!(ENVELOPE_PRIVATE);
        assert_eq!(native_failures(&native), Vec::<String>::new());
    }

    #[test]
    fn a_live_presenter_run_over_the_private_envelope_fails() {
        let mut native = passing_native();
        native["live_presenter"][1] = live_run(2, ENVELOPE_PRIVATE + 1, 300 * MIB);
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].starts_with("live presenter run 2:"));
        assert!(failures[0].contains("(private_bytes)"));
        assert!(failures[0].contains("same-run Phase 0 private envelope"));
    }

    #[test]
    fn a_live_presenter_peak_over_the_peak_envelope_fails() {
        let mut native = passing_native();
        native["live_presenter"][2] = live_run(3, 190 * MIB, ENVELOPE_PEAK + 1);
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("(peak_private_bytes)"));
        assert!(failures[0].contains("same-run Phase 0 peak private envelope"));
    }

    #[test]
    fn a_missing_errored_or_unsampled_presenter_baseline_fails() {
        let message = "live presenter: no same-run Phase-0 presenter baseline to compare against";
        let mutations: [fn(&mut Value); 4] = [
            |native| {
                native["baselines"].as_array_mut().unwrap().pop();
            },
            |native| native["baselines"][1]["presenter"] = json!({"error": "timed out"}),
            |native| {
                native["baselines"][2]["presenter"]
                    .as_object_mut()
                    .unwrap()
                    .remove("peak_private_bytes");
            },
            |native| {
                native["baselines"][0]["presenter"]
                    .as_object_mut()
                    .unwrap()
                    .remove("private_bytes");
            },
        ];
        for mutate in mutations {
            let mut native = passing_native();
            mutate(&mut native);
            assert!(
                native_failures(&native)
                    .iter()
                    .any(|failure| failure == message),
                "{:?}",
                native_failures(&native)
            );
        }
    }

    #[test]
    fn a_live_presenter_run_over_the_qualification_budget_fails_both_gates() {
        let mut native = passing_native();
        native["live_presenter"][0] = live_run(1, 600 * MIB, 600 * MIB);
        let failures = native_failures(&native);
        // Private and peak each miss the budget and their envelope.
        assert_eq!(failures.len(), 4, "{failures:?}");
        assert!(failures[0].contains("qualification budget"));
        assert!(failures[1].contains("same-run Phase 0 private envelope"));
        assert!(failures[2].contains("qualification budget"));
        assert!(failures[3].contains("same-run Phase 0 peak private envelope"));
    }

    #[test]
    fn a_mid_phase_sample_over_the_bound_fails_even_when_the_last_is_fine() {
        let mut native = passing_native();
        native["live_presenter"][0]["phases"][1]["private_bytes"] = json!(ENVELOPE_PRIVATE + 1);
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(
            failures[0].contains("run 1, phase 2 (reverse)"),
            "{failures:?}"
        );
        assert!(failures[0].contains("same-run Phase 0 private envelope"));
    }

    fn one_live_failure(mutate: impl FnOnce(&mut Value), expected: &str) {
        let mut native = passing_native();
        mutate(&mut native["live_presenter"][0]);
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains(expected), "{failures:?}");
    }

    #[test]
    fn a_live_presenter_run_that_did_not_settle_or_do_the_work_fails() {
        one_live_failure(
            |run| run["phases"][2]["settled"] = json!(false),
            "did not settle",
        );
        one_live_failure(
            |run| {
                run["phases"][0].as_object_mut().unwrap().remove("settled");
            },
            "did not settle",
        );
        one_live_failure(|run| run["cue_count"] = json!(499), "fewer than 500 Cues");
        one_live_failure(
            |run| run["phases"][1]["commands"] = json!(499),
            "fewer than 500 commands",
        );
        one_live_failure(
            |run| run["mode"] = json!("other"),
            "mode is not live-presenter",
        );
        one_live_failure(
            |run| run["max_full_images"] = json!(0),
            "no full-size image",
        );
    }

    #[test]
    fn a_live_presenter_run_with_fewer_than_three_phases_fails() {
        one_live_failure(
            |run| {
                run["phases"].as_array_mut().unwrap().pop();
            },
            "fewer than 3 phases",
        );
        one_live_failure(
            |run| {
                run.as_object_mut().unwrap().remove("phases");
            },
            "fewer than 3 phases",
        );
    }

    #[test]
    fn an_unsampled_errored_or_missing_live_presenter_fails() {
        let mut native = passing_native();
        native["live_presenter"][0] = json!({"run": 1, "rss_bytes": 10 * MIB});
        native["live_presenter"][1] = json!({"run": 2, "error": "timed out after 300 s"});
        let failures = native_failures(&native);
        for expected in [
            "run 1: private_bytes was not sampled",
            "run 1: peak_private_bytes was not sampled",
            "run 2: timed out",
        ] {
            assert!(
                failures.iter().any(|line| line.contains(expected)),
                "{expected}: {failures:?}"
            );
        }

        let mut native = passing_native();
        native["live_presenter"] = json!([]);
        assert_eq!(
            native_failures(&native),
            vec!["live presenter: no runs were recorded".to_owned()]
        );
        native.as_object_mut().unwrap().remove("live_presenter");
        assert_eq!(native_failures(&native).len(), 1);
    }

    #[test]
    fn passing_animations_have_no_failures_even_with_a_high_frame_cost() {
        // The glide's cpu p95 (17.1 ms) is over the refresh: reported only.
        assert!(animation_failures(&passing_native()).is_empty());
        let mut native = passing_native();
        native["animations"]["animations"][0]["interval_ms"]["p95"] = json!(19.9);
        assert!(animation_failures(&native).is_empty());
    }

    fn one_animation_failure(mutate: impl FnOnce(&mut Value), expected: &str) {
        let mut native = passing_native();
        mutate(&mut native["animations"]);
        let failures = native_failures(&native);
        assert!(
            failures.iter().any(|failure| failure.contains(expected)),
            "{expected}: {failures:?}"
        );
    }

    #[test]
    fn an_interval_over_33_ms_fails() {
        one_animation_failure(
            |a| a["animations"][2]["over_33ms"] = json!(2),
            "animation group_fold: 2 frame intervals over 33 ms",
        );
    }

    #[test]
    fn an_interval_p95_over_one_and_a_half_refreshes_fails() {
        one_animation_failure(
            |a| a["animations"][0]["interval_ms"]["p95"] = json!(20.1),
            "animation timeline_glide: interval p95 20.10 ms is over 19.95 ms",
        );
    }

    #[test]
    fn the_p95_bar_follows_the_measured_refresh() {
        // At 60 Hz the bar is 25 ms, so 22 ms passes; at 144 Hz it is 10.4 ms.
        let mut native = passing_native();
        native["animations"]["animations"][0]["interval_ms"]["p95"] = json!(22.0);
        native["animations"]["refresh_ms"] = json!(16.67);
        assert!(animation_failures(&native).is_empty());
        native["animations"]["refresh_ms"] = json!(6.94);
        assert_eq!(animation_failures(&native).len(), 2);
    }

    #[test]
    fn an_unmeasured_refresh_or_missing_document_fails() {
        one_animation_failure(
            |a| {
                a["refresh_ms"] = Value::Null;
                a["refresh_note"] = json!("probe drew 3 intervals");
            },
            "the display refresh was not measured (probe drew 3 intervals)",
        );
        one_animation_failure(
            |a| *a = json!({"error": "timed out"}),
            "animations: timed out",
        );
        let mut native = passing_native();
        native.as_object_mut().unwrap().remove("animations");
        assert_eq!(
            animation_failures(&native),
            ["animations: no result was recorded"]
        );
        one_animation_failure(
            |a| a["animations"] = json!([]),
            "animations: no animation was recorded",
        );
    }

    #[test]
    fn an_animation_without_intervals_or_fields_fails() {
        one_animation_failure(
            |a| a["animations"][0]["intervals"] = json!(0),
            "animation timeline_glide: no frame intervals were measured",
        );
        one_animation_failure(
            |a| a["animations"][0]["over_33ms"] = Value::Null,
            "animation timeline_glide: no over_33ms count",
        );
        one_animation_failure(
            |a| a["animations"][0]["interval_ms"] = json!({}),
            "animation timeline_glide: no interval p95",
        );
    }

    #[test]
    fn a_non_applicable_animation_is_ignored() {
        // rail_overlay has 9 hitches and a 90 ms p95 in the passing document.
        assert!(animation_failures(&passing_native()).is_empty());
    }

    #[test]
    fn the_summary_names_the_refresh_the_worst_p95_and_the_cpu() {
        let lines = summary_lines(&passing_native());
        assert!(
            lines.iter().any(|line| line
                == "animations: refresh 13.30 ms (75.0 Hz), worst interval p95 16.70 ms (timeline_glide, bar 19.95 ms), 0 intervals over 33 ms, worst frame cost p95 17.10 ms (reported)"),
            "{lines:?}"
        );
    }

    #[test]
    fn the_summary_names_each_live_presenter_run() {
        let lines = summary_lines(&passing_native());
        assert!(lines.iter().any(|line| line
            == "live presenter run 1: private 190.0 MiB (private_bytes), peak private 330.0 MiB (peak_private_bytes), 500 Cues, 120 frames composed, max cache 40, max images 2 full / 3 thumbnail (envelope 220.0 / 352.0 MiB = same-run Phase-0 presenter \u{d7} 1.10)"));
    }

    #[test]
    fn the_summary_reports_an_errored_live_presenter_run() {
        let mut native = passing_native();
        native["live_presenter"][1] = json!({"run": 2, "error": "timed out after 300 s"});
        let lines = summary_lines(&native);
        assert!(
            lines
                .iter()
                .any(|line| line == "live presenter run 2: timed out after 300 s"),
            "{lines:?}"
        );
    }

    #[test]
    fn measure_memory_prefers_peaks_then_private_then_rss() {
        let keys = &MEASURE_MEMORY_KEYS;
        let source = |sample: Value| read_memory(&sample, keys).map(|reading| reading.source);
        assert_eq!(
            source(json!({"rss_bytes": 1, "private_bytes": 2, "peak_rss_bytes": 3})),
            Some("peak_rss_bytes")
        );
        assert_eq!(
            source(json!({"rss_bytes": 1, "private_bytes": 2})),
            Some("private_bytes")
        );
        assert_eq!(source(json!({"rss_bytes": 1})), Some("rss_bytes"));
        assert_eq!(source(json!({})), None);
    }
}
