//! Presenter bench acceptance (T0.6 WP4), through the `ordo` performance
//! Module Interface only.
//!
//! Both tests take one lock, because they sample the whole test process's
//! resident memory and must not run beside each other.
//!
//! `ORDO_PERFORMANCE_REPORT`, when set to an absolute path, makes the baseline
//! test write the measurement document there. `cargo xtask measure-presenter`
//! sets it to `target/ordo-performance/report.json`.

use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use ordo::performance::{
    BenchConfig, ProcessSample,
    bench::{BaselineRun, measurement_json},
    latency::{CommandKind, Surface},
    run_presenter_bench, three_run_baselines,
};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const P95_LIMIT: Duration = Duration::from_millis(50);
const QUALIFICATION_BYTES: u64 = 512 * MIB;
const CEILING_BYTES: u64 = GIB;
const REPORT_ENV: &str = "ORDO_PERFORMANCE_REPORT";

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn resident_bytes(sample: ProcessSample) -> u64 {
    sample.private_bytes.unwrap_or(sample.rss_bytes)
}

fn assert_within_memory_budget(label: &str, sample: Option<ProcessSample>) {
    let sample = sample.unwrap_or_else(|| panic!("{label}: process memory was not sampled"));
    let bytes = resident_bytes(sample);
    assert!(
        bytes <= QUALIFICATION_BYTES,
        "{label}: {bytes} bytes exceeds the 512 MiB qualification budget"
    );
    assert!(
        bytes <= CEILING_BYTES,
        "{label}: {bytes} bytes exceeds the 1 GiB absolute ceiling"
    );
}

/// The adopted bench: 500 Cues, 2,000 mixed commands. The final state matches
/// the oracle, the hot path is clean, every command and surface pair has a p95
/// under 50 ms, and process memory is within both memory limits.
#[test]
fn the_500_cue_bench_meets_the_adopted_budget() {
    let _serial = serial();
    let outcome = run_presenter_bench(BenchConfig::default());

    assert_eq!(outcome.commands_applied, 2000);
    assert!(
        outcome.oracle_match,
        "engine final state diverged from the oracle fed the coalesced stream"
    );
    assert!(
        outcome.hot_path.is_clean(),
        "hot path performed forbidden work: {:?}",
        outcome.hot_path
    );

    for command in CommandKind::ALL {
        for surface in Surface::ALL {
            let histogram = outcome
                .report
                .latency
                .histogram(command, surface)
                .unwrap_or_else(|| panic!("{command:?}/{surface:?} was not recorded"));
            assert!(
                histogram.count() > 0,
                "{command:?}/{surface:?} has no samples"
            );
            let p95 = histogram.p95().expect("a sampled pair has a p95");
            assert!(
                p95 < P95_LIMIT,
                "{command:?}/{surface:?} p95 {p95:?} is not under {P95_LIMIT:?}"
            );
        }
    }

    assert_within_memory_budget("500-Cue bench", outcome.report.process);
    assert!(
        outcome.verdict.is_pass(),
        "budget verdict: {:?}",
        outcome.verdict
    );
}

/// Three independent runs of the empty-shell, Preparation, and 500-Cue Presenter
/// memory phases. Every sample stays within the qualification budget and the
/// absolute ceiling. When `ORDO_PERFORMANCE_REPORT` is set, this test also
/// writes the measurement document, with one bench run beside the baselines.
#[test]
fn three_run_baselines_stay_within_the_memory_budget() {
    let _serial = serial();
    let config = BenchConfig::default();
    let runs: [BaselineRun; 3] = three_run_baselines(config);

    for run in &runs {
        assert_within_memory_budget(&format!("run {} empty shell", run.run), run.empty_shell);
        assert_within_memory_budget(&format!("run {} Preparation", run.run), run.preparation);
        assert_within_memory_budget(&format!("run {} Presenter", run.run), run.presenter);
    }

    if let Some(path) = std::env::var_os(REPORT_ENV) {
        let outcome = run_presenter_bench(config);
        let document = measurement_json(&outcome, &runs);
        std::fs::write(&path, document)
            .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.to_string_lossy()));
        println!("wrote performance report to {}", path.to_string_lossy());
    }
}
