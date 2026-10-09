//! `cargo xtask measure-presenter`: runs the Presenter performance acceptance
//! in release and leaves its measurement document at
//! `target/ordo-performance/report.json`, where CI uploads it from.
//!
//! The engine bench (`tests/performance`) runs first and writes the document.
//! Then the native measurement runs the `presenter-lab` example, a real GPUI
//! window, as one fresh release process per measurement: the keydown-to-paint
//! run, the display-path comparison, and three runs of each memory baseline.
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

/// Every keydown-to-paint p95 must be strictly below this.
const P95_LIMIT_MS: f64 = 50.0;
/// Process memory above this fails the qualification budget.
const QUALIFICATION_BYTES: u64 = 512 * MIB;
/// Process memory above this fails the absolute release ceiling.
const CEILING_BYTES: u64 = GIB;
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Baseline runs, each a fresh process per mode.
const BASELINE_RUNS: usize = 3;
/// The three baseline modes, in the order the document lists them. Each pair
/// is the lab's mode name and the key it is stored under.
const BASELINE_MODES: [(&str, &str); 3] = [
    ("empty", "empty_shell"),
    ("preparation", "preparation"),
    ("presenter", "presenter"),
];

/// A lab process still running after this is killed and recorded as timed out.
/// The lab's own 120 s guard fires first in every normal failure.
const PROCESS_TIMEOUT: Duration = Duration::from_secs(180);
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
    build_lab(root)?;
    let executable = lab_executable(root);
    let scratch =
        std::env::temp_dir().join(format!("ordo-measure-presenter-{}", std::process::id()));
    fs::create_dir_all(&scratch)?;

    let measure = run_lab(&executable, &["--measure"], &scratch, "measure");
    let display = run_lab(&executable, &["--display-path"], &scratch, "display-path");
    let display_path = display.get("display_path").cloned().unwrap_or(display);

    let mut baselines = Vec::with_capacity(BASELINE_RUNS);
    for run in 1..=BASELINE_RUNS {
        let mut entry = serde_json::Map::new();
        entry.insert("run".to_owned(), json!(run));
        for (mode, key) in BASELINE_MODES {
            let label = format!("baseline-{mode}-{run}");
            let document = run_lab(&executable, &["--baseline", mode], &scratch, &label);
            entry.insert(key.to_owned(), document);
        }
        baselines.push(Value::Object(entry));
    }

    // Best effort: the scratch files are not part of the document.
    let _ = fs::remove_dir_all(&scratch);
    Ok(json!({
        "measure": measure,
        "display_path": display_path,
        "baselines": baselines,
    }))
}

/// Builds the lab example in release, with the same `--locked` pin as the
/// rest of the workspace.
fn build_lab(root: &Path) -> XtaskResult<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "ordo",
            "--example",
            LAB_EXAMPLE,
            "--locked",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err(format!(
            "command failed: cargo build --release -p ordo --example {LAB_EXAMPLE} --locked"
        )
        .into());
    }
    Ok(())
}

/// Where cargo put the release example. Honours `CARGO_TARGET_DIR`, which
/// cargo resolves against the directory it runs in, the repository root.
fn lab_executable(root: &Path) -> PathBuf {
    let target = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => root.join(dir),
        None => root.join("target"),
    };
    target
        .join("release")
        .join("examples")
        .join(format!("{LAB_EXAMPLE}{}", std::env::consts::EXE_SUFFIX))
}

/// Runs one lab process and returns its document. A process that times out is
/// killed, and one that fails without a readable document yields an
/// `{"error": ...}` document, so every run has a result to report.
fn run_lab(executable: &Path, args: &[&str], scratch: &Path, label: &str) -> Value {
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
            Ok(None) if started.elapsed() < PROCESS_TIMEOUT => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return json!({
                    "error": format!("timed out after {} s and was killed", PROCESS_TIMEOUT.as_secs())
                });
            }
            Err(error) => {
                let _ = child.kill();
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

/// Process memory the budget judges: private bytes when the platform reports
/// them, RSS otherwise. This is the rule `PerformanceReport::budget_check`
/// applies, so the document and the engine bench agree.
fn resident_bytes(sample: &Value) -> Option<u64> {
    sample
        .get("private_bytes")
        .and_then(Value::as_u64)
        .or_else(|| sample.get("rss_bytes").and_then(Value::as_u64))
}

/// Failures for one memory sample against the qualification budget and the
/// absolute ceiling. A missing sample fails, because an unmeasured budget is
/// not a passing one.
fn memory_failures(label: &str, bytes: Option<u64>) -> Vec<String> {
    match bytes {
        None => vec![format!("{label}: process memory was not sampled")],
        Some(bytes) => {
            let mut failures = Vec::new();
            if bytes > QUALIFICATION_BYTES {
                failures.push(format!(
                    "{label}: {} MiB is over the 512 MiB qualification budget",
                    bytes / MIB
                ));
            }
            if bytes > CEILING_BYTES {
                failures.push(format!(
                    "{label}: {} MiB is over the 1 GiB absolute ceiling",
                    bytes / MIB
                ));
            }
            failures
        }
    }
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
                        None => failures.extend(memory_failures(&label, resident_bytes(sample))),
                    }
                }
            }
        }
        None => failures.push("baselines: no runs were recorded".to_owned()),
    }
    failures
}

/// Failures for the keydown-to-paint run: an error, a divergent final state,
/// a p95 at or over the limit, memory over budget, or a hot-path violation.
fn measure_failures(measure: &Value) -> Vec<String> {
    if let Some(message) = error_message(measure) {
        return vec![format!("keydown-to-paint run: {message}")];
    }
    let mut failures = Vec::new();
    if measure["oracle_match"].as_bool() != Some(true) {
        failures.push("keydown-to-paint run: final state diverged from the oracle".to_owned());
    }

    let report = &measure["measure"];
    match report["latency"].as_array() {
        Some(pairs) => {
            for pair in pairs {
                let label = format!(
                    "keydown-to-paint {} {}",
                    pair["command"].as_str().unwrap_or("?"),
                    pair["surface"].as_str().unwrap_or("?")
                );
                match pair["p95_ms"].as_f64() {
                    Some(p95) if p95 < P95_LIMIT_MS => {}
                    Some(p95) => failures.push(format!(
                        "{label}: p95 {p95:.3} ms is at or over the 50 ms limit"
                    )),
                    None => failures.push(format!("{label}: no p95 was recorded")),
                }
            }
        }
        None => failures.push("keydown-to-paint run: no latency table was recorded".to_owned()),
    }

    failures.extend(memory_failures(
        "keydown-to-paint run",
        resident_bytes(&report["memory"]),
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

/// Adds the native results under the `native` key, leaving every existing
/// key of the engine document as it is.
fn merge_native(document: &mut Value, native: Value) -> XtaskResult<()> {
    let object = document
        .as_object_mut()
        .ok_or("the engine measurement document is not a JSON object")?;
    object.insert("native".to_owned(), native);
    Ok(())
}

/// Human-readable lines for the console: per-command p95, the display-path
/// medians, the baseline memory, and the renderer.
fn summary_lines(native: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    let report = &native["measure"]["measure"];
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
                "baseline run {}: empty {} MiB, preparation {} MiB, presenter {} MiB (RSS)",
                run["run"],
                mib_of(&run["empty_shell"]["rss_bytes"]),
                mib_of(&run["preparation"]["rss_bytes"]),
                mib_of(&run["presenter"]["rss_bytes"]),
            ));
        }
    }
    lines
}

fn mib_of(bytes: &Value) -> String {
    match bytes.as_u64() {
        Some(bytes) => format!("{:.1}", bytes as f64 / MIB as f64),
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

    /// A passing keydown-to-paint document with every command and surface
    /// under budget and memory within the qualification budget.
    fn passing_measure() -> Value {
        let pairs: Vec<Value> = ["navigation", "space", "freeze", "blackout"]
            .iter()
            .flat_map(|command| {
                ["preview", "program"].iter().map(move |surface| {
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

    fn passing_native() -> Value {
        let sample = |rss: u64| json!({"mode": "x", "rss_bytes": rss, "virtual_bytes": 1});
        json!({
            "measure": passing_measure(),
            "display_path": {"passes": 30, "cpu": {"median_ms": 50.0, "held_bytes": 8}, "retained": {"median_ms": 12.0, "held_bytes": 4}},
            "baselines": [
                {"run": 1, "empty_shell": sample(40 * MIB), "preparation": sample(120 * MIB), "presenter": sample(90 * MIB)},
                {"run": 2, "empty_shell": sample(40 * MIB), "preparation": sample(120 * MIB), "presenter": sample(90 * MIB)},
                {"run": 3, "empty_shell": sample(40 * MIB), "preparation": sample(120 * MIB), "presenter": sample(90 * MIB)},
            ]
        })
    }

    #[test]
    fn a_passing_native_measurement_has_no_failures() {
        assert_eq!(native_failures(&passing_native()), Vec::<String>::new());
    }

    #[test]
    fn a_p95_at_the_limit_fails() {
        let mut native = passing_native();
        native["measure"]["measure"]["latency"][3]["p95_ms"] = json!(50.0);
        let failures = native_failures(&native);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("p95 50.000 ms"));
    }

    #[test]
    fn a_missing_p95_fails_rather_than_passing() {
        let mut native = passing_native();
        native["measure"]["measure"]["latency"][0]["p95_ms"] = Value::Null;
        assert!(
            native_failures(&native)
                .iter()
                .any(|failure| failure.contains("no p95 was recorded"))
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
    fn memory_over_the_budget_and_over_the_ceiling_fail_separately() {
        assert!(memory_failures("run", Some(512 * MIB)).is_empty());
        let over_budget = memory_failures("run", Some(600 * MIB));
        assert_eq!(over_budget.len(), 1, "{over_budget:?}");
        assert!(over_budget[0].contains("qualification budget"));
        let over_ceiling = memory_failures("run", Some(GIB + MIB));
        assert_eq!(over_ceiling.len(), 2, "{over_ceiling:?}");
        assert!(over_ceiling[1].contains("absolute ceiling"));
        assert_eq!(
            memory_failures("run", None),
            vec!["run: process memory was not sampled".to_owned()]
        );
    }

    #[test]
    fn private_bytes_are_judged_before_rss() {
        let sample = json!({"rss_bytes": 100 * MIB, "private_bytes": 600 * MIB});
        assert_eq!(resident_bytes(&sample), Some(600 * MIB));
        assert_eq!(resident_bytes(&json!({"rss_bytes": 7})), Some(7));
        assert_eq!(resident_bytes(&json!({})), None);
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
        native["measure"] = json!({"error": "timed out after 120 s"});
        assert_eq!(
            native_failures(&native),
            vec!["keydown-to-paint run: timed out after 120 s".to_owned()]
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
    fn a_missing_lab_executable_yields_an_error_document() {
        let scratch = std::env::temp_dir();
        let document = run_lab(
            Path::new("definitely/not/a/presenter-lab.exe"),
            &["--measure"],
            &scratch,
            "unit-missing",
        );
        assert!(
            error_message(&document).is_some_and(|message| message.starts_with("cannot start")),
            "{document}"
        );
    }

    #[test]
    fn the_summary_names_the_p95_and_the_renderer() {
        let lines = summary_lines(&passing_native());
        assert!(
            lines
                .iter()
                .any(|line| line.contains("navigation preview: p50 13.0"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("renderer: \"GPU\""))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("baseline run 1: empty 40.0 MiB"))
        );
    }
}
