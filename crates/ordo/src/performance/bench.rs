//! The Presenter engine bench (T0.6 WP4).
//!
//! The bench drives a [`PresenterEngine`] with a deterministic mixed command
//! stream, one `apply` per command, exactly as the live hot path does. Each
//! command is stamped at keydown and timed to state-ready. Nothing else runs in
//! the measured loop: no storage, filesystem, worker, pagination, image decode,
//! or paint. That makes it an honest measurement of the engine's share of the
//! keydown-to-paint budget and a floor under it, not a substitute for the
//! window-level paint measurement that needs a live Presenter window.
//!
//! Both surfaces are stamped from the same state-ready instant, because the
//! engine returns Preview and Program in one state. The bench also checks the
//! final state against an oracle engine fed the coalesced stream.
//!
//! The memory baselines are process RSS samples taken at three phases of each
//! run. They are the closest approximation available before the real shell and
//! library exist; [`BaselineRun`] documents what each phase holds.

use std::time::{Duration, Instant};

use presentation::presenter::{PresenterCommand, PresenterEngine, PresenterState, coalesce};

use super::{
    latency::{CommandKind, LatencyBook, PendingKeydown, Surface},
    probe::{HotPathCounts, HotPathProbe},
    process::{ProcessSample, ProcessSampler},
    report::{BudgetLimits, BudgetVerdict, PerformanceReport, REPORT_SCHEMA, RendererInfo},
};

/// Parameters of one bench run. The defaults are the adopted T0.6 fixture: a
/// 500-Cue deck and 2,000 mixed commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BenchConfig {
    pub cues: usize,
    pub commands: usize,
    pub seed: u64,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            cues: 500,
            commands: 2000,
            seed: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

/// Everything one bench run produced.
#[derive(Clone, Debug)]
pub struct BenchOutcome {
    pub config: BenchConfig,
    /// Commands applied to the engine, one `apply` each.
    pub commands_applied: usize,
    /// Length of the coalesced stream the oracle was fed.
    pub coalesced_commands: usize,
    /// Whether the engine's final state equals the oracle's, ignoring
    /// `revision`, which a collapsed run changes by design.
    pub oracle_match: bool,
    /// Hot-path-forbidden operations observed during the measured loop.
    pub hot_path: HotPathCounts,
    pub process_before: Option<ProcessSample>,
    /// The latency book, with the process sample taken after the loop.
    pub report: PerformanceReport,
    /// The adopted budget checked against `report`.
    pub verdict: BudgetVerdict,
    /// Wall-clock time of the measured loop.
    pub wall: Duration,
}

impl BenchOutcome {
    /// Whether the run meets every bench requirement: oracle match, a clean
    /// hot path, and a passing budget verdict.
    pub fn passed(&self) -> bool {
        self.oracle_match && self.hot_path.is_clean() && self.verdict.is_pass()
    }

    /// Lines for the Diagnostics panel and the CLI: the report's summary, then
    /// the oracle, hot-path, and budget verdicts.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = self.report.summary_lines();
        lines.push(format!(
            "Oracle match: {}",
            if self.oracle_match { "yes" } else { "NO" }
        ));
        lines.push(format!(
            "Hot path: {} worker send(s) to preferences, {} to canonical",
            self.hot_path.preference_sends, self.hot_path.canonical_sends
        ));
        lines.push(verdict_line(&self.verdict));
        lines
    }
}

/// One-line description of a budget verdict.
pub fn verdict_line(verdict: &BudgetVerdict) -> String {
    match verdict {
        BudgetVerdict::Pass => "Budget: PASS (p95 under 50 ms, memory within 512 MiB)".to_owned(),
        BudgetVerdict::Fail(violations) => format!(
            "Budget: FAIL ({} violation(s); first: {:?})",
            violations.len(),
            violations.first()
        ),
    }
}

/// Maps a Presenter command to the operator action it measures.
fn kind_of(command: PresenterCommand) -> CommandKind {
    match command {
        PresenterCommand::Next | PresenterCommand::Previous | PresenterCommand::GoTo(_) => {
            CommandKind::Navigation
        }
        PresenterCommand::TakeLive => CommandKind::Space,
        PresenterCommand::ToggleFreeze => CommandKind::Freeze,
        PresenterCommand::ToggleBlackout => CommandKind::Blackout,
    }
}

/// A small xorshift generator. Dependency-free and fully determined by `seed`.
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        // xorshift has a fixed point at zero, so a zero seed is remapped.
        Self(if seed == 0 {
            0x2545_F491_4F6C_DD1D
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

/// A deterministic mixed stream: mostly navigation, with Space, Freeze, and
/// Blackout interleaved. GoTo targets run past the deck end on purpose, so the
/// clamping path is exercised too.
pub fn command_stream(seed: u64, cue_count: usize, length: usize) -> Vec<PresenterCommand> {
    let mut rng = XorShift::new(seed);
    let cues = cue_count.max(1) as u64;
    (0..length)
        .map(|_| match rng.below(100) {
            0..=34 => PresenterCommand::Next,
            35..=44 => PresenterCommand::Previous,
            45..=59 => PresenterCommand::GoTo(rng.below(cues + 5) as usize),
            60..=74 => PresenterCommand::TakeLive,
            75..=84 => PresenterCommand::ToggleFreeze,
            _ => PresenterCommand::ToggleBlackout,
        })
        .collect()
}

/// Whether two states agree on every field except `revision`.
fn same_observable_state(a: &PresenterState, b: &PresenterState) -> bool {
    a.preview() == b.preview()
        && a.program() == b.program()
        && a.frozen() == b.frozen()
        && a.blackout() == b.blackout()
}

fn sample_process() -> Option<ProcessSample> {
    ProcessSampler::new().ok()?.sample()
}

/// Runs the bench and returns its outcome. Blocking and CPU-bound: call it on a
/// background executor, never on the GPUI thread.
pub fn run_presenter_bench(config: BenchConfig) -> BenchOutcome {
    let commands = command_stream(config.seed, config.cues, config.commands);
    let process_before = sample_process();

    HotPathProbe::reset();
    let mut engine = PresenterEngine::new(config.cues);
    let mut book = LatencyBook::new();
    let started = Instant::now();
    for &command in &commands {
        let kind = kind_of(command);
        let stamp = PendingKeydown::start(kind);
        engine.apply(command);
        PendingKeydown::finish(stamp, Surface::Preview, &mut book);
        PendingKeydown::finish(stamp, Surface::Program, &mut book);
    }
    let wall = started.elapsed();
    let hot_path = HotPathProbe::counts();

    let coalesced = coalesce(&commands, config.cues, 0);
    let mut oracle = PresenterEngine::new(config.cues);
    for &command in &coalesced {
        oracle.apply(command);
    }
    let oracle_match = same_observable_state(engine.state(), oracle.state());

    let mut report = PerformanceReport::new(RendererInfo::default());
    report.latency = book;
    report.process = sample_process();
    let verdict = report.budget_check(BudgetLimits::default());

    BenchOutcome {
        config,
        commands_applied: commands.len(),
        coalesced_commands: coalesced.len(),
        oracle_match,
        hot_path,
        process_before,
        report,
        verdict,
        wall,
    }
}

/// One run's three memory samples. Each sample is process RSS, so later phases
/// include the fixtures earlier phases left resident in the same run.
///
/// - `empty_shell`: the process before any fixture is built in the run. This is
///   not the GPUI shell: a window needs the application, which the bench does
///   not open.
/// - `preparation`: a synthetic large library, 10,000 entries of about 512
///   bytes each, held resident. It stands in for the Preparation data set until
///   the real library exists.
/// - `presenter`: the synthetic library plus a 500-Cue deck of text, with the
///   bench's engine, latency book, and report built and held. No Program frames,
///   thumbnails, or GPU resources are created, so this is not a full Presenter
///   window.
#[derive(Clone, Debug)]
pub struct BaselineRun {
    pub run: usize,
    pub empty_shell: Option<ProcessSample>,
    pub preparation: Option<ProcessSample>,
    pub presenter: Option<ProcessSample>,
}

const SYNTHETIC_LIBRARY_ENTRIES: usize = 10_000;
const SYNTHETIC_ENTRY_BYTES: usize = 512;

fn synthetic_library() -> Vec<String> {
    (0..SYNTHETIC_LIBRARY_ENTRIES)
        .map(|index| {
            let mut entry = format!("entry {index} ");
            entry.extend(std::iter::repeat_n('a', SYNTHETIC_ENTRY_BYTES));
            entry
        })
        .collect()
}

fn synthetic_deck(cues: usize) -> Vec<String> {
    (0..cues)
        .map(|index| format!("cue {index}: {}", "verse text ".repeat(32)))
        .collect()
}

/// Three independent runs of the three-phase memory baseline.
pub fn three_run_baselines(config: BenchConfig) -> [BaselineRun; 3] {
    [1, 2, 3].map(|run| baseline_run(run, config))
}

fn baseline_run(run: usize, config: BenchConfig) -> BaselineRun {
    let empty_shell = sample_process();
    let library = synthetic_library();
    let preparation = sample_process();
    let deck = synthetic_deck(config.cues);
    let outcome = run_presenter_bench(config);
    let presenter = sample_process();
    // Keep both fixtures and the outcome alive until the final sample.
    drop((library, deck, outcome));
    BaselineRun {
        run,
        empty_shell,
        preparation,
        presenter,
    }
}

fn json_sample(sample: Option<ProcessSample>) -> String {
    match sample {
        Some(sample) => format!(
            "{{\"rss_bytes\":{},\"virtual_bytes\":{}}}",
            sample.rss_bytes, sample.virtual_bytes
        ),
        None => "null".to_owned(),
    }
}

/// The three-run baseline as a JSON array.
pub fn baselines_json(runs: &[BaselineRun]) -> String {
    let items: Vec<String> = runs
        .iter()
        .map(|run| {
            format!(
                "{{\"run\":{},\"empty_shell\":{},\"preparation\":{},\"presenter\":{}}}",
                run.run,
                json_sample(run.empty_shell),
                json_sample(run.preparation),
                json_sample(run.presenter)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// The complete measurement document `cargo xtask measure-presenter` writes:
/// the bench summary, the report, and the baselines.
pub fn measurement_json(outcome: &BenchOutcome, baselines: &[BaselineRun]) -> String {
    format!(
        "{{\"schema\":\"{REPORT_SCHEMA}\",\"bench\":{{\"cues\":{},\"commands\":{},\"seed\":{},\"wall_ms\":{:.3},\"oracle_match\":{},\"hot_path_clean\":{},\"budget_pass\":{}}},\"report\":{},\"baselines\":{}}}",
        outcome.config.cues,
        outcome.config.commands,
        outcome.config.seed,
        outcome.wall.as_secs_f64() * 1000.0,
        outcome.oracle_match,
        outcome.hot_path.is_clean(),
        outcome.verdict.is_pass(),
        outcome.report.to_json(),
        baselines_json(baselines),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_is_deterministic_for_a_seed() {
        assert_eq!(command_stream(7, 500, 300), command_stream(7, 500, 300));
        assert_ne!(command_stream(7, 500, 300), command_stream(8, 500, 300));
    }

    #[test]
    fn the_stream_covers_every_command_kind() {
        let stream = command_stream(BenchConfig::default().seed, 500, 2000);
        let kinds: std::collections::BTreeSet<_> = stream.iter().map(|c| kind_of(*c)).collect();
        assert_eq!(kinds.len(), CommandKind::ALL.len());
        assert!(
            stream
                .iter()
                .any(|c| matches!(c, PresenterCommand::GoTo(i) if *i >= 500))
        );
    }

    #[test]
    fn a_zero_seed_still_produces_a_varied_stream() {
        let stream = command_stream(0, 10, 100);
        assert!(stream.iter().any(|c| *c != stream[0]));
    }

    #[test]
    fn a_small_bench_matches_the_oracle_and_keeps_the_hot_path_clean() {
        let outcome = run_presenter_bench(BenchConfig {
            cues: 20,
            commands: 300,
            seed: 11,
        });
        assert_eq!(outcome.commands_applied, 300);
        assert!(outcome.coalesced_commands <= outcome.commands_applied);
        assert!(outcome.oracle_match);
        assert!(outcome.hot_path.is_clean());
        for command in CommandKind::ALL {
            for surface in Surface::ALL {
                let histogram = outcome
                    .report
                    .latency
                    .histogram(command, surface)
                    .expect("every pair is recorded");
                assert!(histogram.count() > 0, "{command:?} {surface:?} unmeasured");
            }
        }
    }

    #[test]
    fn the_oracle_comparison_detects_a_divergent_state() {
        let mut a = PresenterEngine::new(5);
        let mut b = PresenterEngine::new(5);
        a.apply(PresenterCommand::Next);
        b.apply(PresenterCommand::Previous);
        assert!(!same_observable_state(a.state(), b.state()));
        assert!(same_observable_state(a.state(), a.state()));
    }

    #[test]
    fn baseline_json_lists_every_phase_of_every_run() {
        let run = BaselineRun {
            run: 1,
            empty_shell: None,
            preparation: None,
            presenter: Some(ProcessSample {
                rss_bytes: 10,
                virtual_bytes: 20,
                private_bytes: None,
                cpu_percent: 0.0,
            }),
        };
        let json = baselines_json(&[run]);
        assert_eq!(
            json,
            "[{\"run\":1,\"empty_shell\":null,\"preparation\":null,\"presenter\":{\"rss_bytes\":10,\"virtual_bytes\":20}}]"
        );
    }
}
