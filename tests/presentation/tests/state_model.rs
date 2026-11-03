//! Engine property tests and the Recovery Drill replay (T1.2), through the
//! public Engine interface over the committed fixtures.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::presentation::{CueBehavior, FrameId, PresentationSnapshot};
use presentation::commands::Command;
use presentation::engine::{
    AutoPlayRefusal, AutoPlayScope, Availability, Engine, Outcome, Refusal, UnavailableSource,
};
use presentation::snapshot_reader::{
    DirectorySource, LoadedSnapshot, SnapshotLoadError, read_snapshot,
};
use presentation::state::{EngineState, LiveState, Millis, ProgramContent};
use rendering::UnavailableReason;

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/presentations")
        .join(name)
}

fn load(name: &str) -> LoadedSnapshot {
    read_snapshot(&DirectorySource::new(fixture_dir(name))).expect("fixture loads")
}

struct XorShift(u64);

impl XorShift {
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

/// One generated step: a command or a tick, at an absolute time.
#[derive(Clone, Copy, Debug)]
enum Step {
    Command(Command, Millis),
    Tick(Millis),
}

impl Step {
    fn time(&self) -> Millis {
        match *self {
            Self::Command(_, now) | Self::Tick(now) => now,
        }
    }
}

/// `frames` are the snapshot's declared frames; about 1% of steps report one
/// of them as failed at runtime.
fn generate(rng: &mut XorShift, length: usize, cue_count: usize, frames: &[FrameId]) -> Vec<Step> {
    let mut clock = 0u64;
    (0..length)
        .map(|_| {
            // Time mostly creeps forward, sometimes jumps past auto-play and
            // fade durations. Step times never decrease.
            clock += match rng.below(4) {
                0 => 0,
                1 => rng.below(120),
                2 => rng.below(700),
                _ => rng.below(9_000),
            };
            let now = Millis(clock);
            if !frames.is_empty() && rng.below(100) == 0 {
                let frame = frames[rng.below(frames.len() as u64) as usize];
                let reason = if rng.below(2) == 0 {
                    UnavailableReason::DecodeFailed
                } else {
                    UnavailableReason::MissingAsset
                };
                return Step::Command(Command::FrameUnavailable { frame, reason }, now);
            }
            let command = match rng.below(26) {
                0 | 1 => Command::Next,
                2 => Command::Previous,
                3 => Command::NextItem,
                4 => Command::PreviousItem,
                5 => Command::First,
                6 => Command::Last,
                7 => Command::GoTo(rng.below(cue_count as u64 + 6) as usize),
                8..=10 => Command::Take,
                11 => Command::ToggleFreeze,
                12 => Command::ToggleBlackout,
                13 | 14 => Command::EnableOutput,
                15 => Command::DisableOutput,
                16 => Command::DisplayLost,
                17 => Command::DisplayReturned { recognized: true },
                18 => Command::DisplayReturned { recognized: false },
                19 | 20 => Command::ToggleAutoPlay,
                21 => Command::LoopSection,
                22 => Command::StopAutoPlay,
                23 => Command::Recover,
                _ => return Step::Tick(now),
            };
            Step::Command(command, now)
        })
        .collect()
}

fn after_program(engine: &Engine) -> ProgramContent {
    engine.state().program()
}

type Trace = Vec<(Result<Outcome, Refusal>, EngineState)>;

/// Whether Cue `cue` is a Project Cue whose frame is loaded (and equals
/// `frame` when one is given).
fn is_loaded_project_frame(loaded: &LoadedSnapshot, cue: usize, frame: Option<FrameId>) -> bool {
    let Some(CueBehavior::Project {
        frame: cue_frame, ..
    }) = loaded.snapshot().cue(cue).map(|cue| cue.behavior())
    else {
        return false;
    };
    loaded.frame(*cue_frame).is_some() && frame.is_none_or(|frame| frame == *cue_frame)
}

/// Whether the Cue at `cue` failed at runtime (Command::FrameUnavailable).
fn runtime_unavailable(engine: &Engine, cue: usize) -> bool {
    matches!(
        engine.cue_availability(cue),
        Availability::Unavailable(UnavailableSource::Runtime(_))
    )
}

/// Every invariant that must hold after every step.
///
/// `grandfathered` is the one frame allowed to stay on Program although its
/// Cues failed at runtime: the frame that was already on Program when it was
/// marked (it is complete and pinned; the next navigation follows the new
/// availability).
fn assert_invariants(
    loaded: &LoadedSnapshot,
    engine: &Engine,
    clock: Millis,
    grandfathered: Option<FrameId>,
    context: &str,
) {
    let snapshot = loaded.snapshot();
    let state = engine.state();
    let live = state.live();
    match state.program() {
        ProgramContent::Black => {}
        ProgramContent::Frame { frame, cue } => {
            assert!(
                matches!(live, LiveState::Live | LiveState::Frozen),
                "{context}: Program shows a frame in {live:?}"
            );
            assert!(
                is_loaded_project_frame(loaded, cue, Some(frame)),
                "{context}: Program frame {frame:?} is not the loaded frame of Project Cue {cue}"
            );
            // Program never shows a frame whose Cue failed at runtime, except
            // the frame that was already on Program when it was marked.
            assert!(
                !runtime_unavailable(engine, cue) || grandfathered == Some(frame),
                "{context}: Program shows runtime-unavailable frame {frame:?} of Cue {cue}"
            );
        }
    }
    if !matches!(live, LiveState::Live | LiveState::Frozen) {
        assert_eq!(
            state.program(),
            ProgramContent::Black,
            "{context}: {live:?}"
        );
    }
    if state.unavailable_black() {
        assert_eq!(live, LiveState::Live, "{context}");
        assert_eq!(state.program(), ProgramContent::Black, "{context}");
    }
    assert!(state.preview() < snapshot.cue_count(), "{context}");

    // Linked Live: Program is exactly what the Preview Cue dictates.
    if live == LiveState::Live {
        let preview = state.preview();
        match snapshot.cue(preview).unwrap().behavior() {
            CueBehavior::Project { frame, .. } if runtime_unavailable(engine, preview) => {
                if grandfathered == Some(*frame)
                    && state.program()
                        == (ProgramContent::Frame {
                            frame: *frame,
                            cue: preview,
                        })
                {
                    // The marked frame stays where it already was.
                } else {
                    assert_eq!(state.program(), ProgramContent::Black, "{context}");
                    assert!(state.unavailable_black(), "{context}");
                }
            }
            CueBehavior::Project { frame, .. } => assert_eq!(
                state.program(),
                ProgramContent::Frame {
                    frame: *frame,
                    cue: preview
                },
                "{context}"
            ),
            CueBehavior::Unavailable { .. } => {
                assert_eq!(state.program(), ProgramContent::Black, "{context}");
                assert!(state.unavailable_black(), "{context}");
            }
            CueBehavior::OperatorTrigger | CueBehavior::Hold => {
                assert_eq!(state.program(), ProgramContent::Black, "{context}");
                assert!(!state.unavailable_black(), "{context}");
            }
            // A Wait leaves Program and the flag as they were (a Wait after an
            // unavailable Cue, including one that failed at runtime, keeps the
            // "Cue unavailable" black); the flag implies black, asserted above.
            CueBehavior::Wait => {}
        }
    }

    // Auto-play is bounded to its scope and never runs while Program is held,
    // blacked out, or the display is disconnected. Rehearsal (output disabled
    // or black) may run it (Decision 54), so those states are not asserted
    // against.
    if let Some(run) = state.auto_play() {
        assert!(
            !matches!(
                live,
                LiveState::Frozen | LiveState::Blackout | LiveState::Disconnected { .. }
            ),
            "{context}: auto-play in {live:?}"
        );
        let position = snapshot.positions()[state.preview()];
        match run.scope {
            AutoPlayScope::Item(item) => assert_eq!(position.item_id, item, "{context}"),
            AutoPlayScope::Section(section) => assert_eq!(
                snapshot.sections()[position.section].id(),
                section,
                "{context}"
            ),
        }
        // The scope is the whole item or section, so Preview may sit on a
        // Cue that cannot advance; then the run has no deadline. On a
        // projectable Cue it has one, and the owner of the clock is told.
        let projectable = matches!(
            snapshot.cue(state.preview()).unwrap().behavior(),
            CueBehavior::Project { .. }
        ) && !runtime_unavailable(engine, state.preview());
        assert_eq!(run.duration_ms.is_some(), projectable, "{context}");
        if let Some(duration_ms) = run.duration_ms {
            let due = Millis(run.cue_started.0 + duration_ms);
            assert!(
                engine.next_deadline().is_some_and(|next| next <= due),
                "{context}: next_deadline misses the auto-play due time"
            );
        }
    }

    // The monitors never draw anything but black or a loaded Project frame.
    for (name, view) in [
        ("program", engine.program_view(clock)),
        ("preview", engine.preview_view(clock)),
    ] {
        assert!(
            (0.0..=1.0).contains(&view.opacity),
            "{context}: {name} opacity {}",
            view.opacity
        );
        match view.shown {
            None => assert_eq!(view.opacity, 0.0, "{context}: {name}"),
            Some(frame) => {
                let drawable = snapshot
                    .positions()
                    .iter()
                    .any(|position| is_loaded_project_frame(loaded, position.flat, Some(frame)));
                assert!(drawable, "{context}: {name} shows {frame:?}");
            }
        }
    }
}

fn run(loaded: &LoadedSnapshot, steps: &[Step], check: bool, label: &str) -> Trace {
    let mut engine = Engine::new(Arc::new(loaded.snapshot().clone()));
    let mut trace = Vec::with_capacity(steps.len());
    let mut grandfathered: Option<FrameId> = None;
    for (index, step) in steps.iter().enumerate() {
        let context = format!("{label} step {index} {step:?}");
        let before = engine.state().clone();
        let before_now = engine.last_now();
        let (outcome, dry_run) = match *step {
            Step::Command(command, now) => {
                let dry_run = engine.check(command, now);
                (engine.apply(command, now), Some(dry_run))
            }
            Step::Tick(now) => (Ok(engine.tick(now)), None),
        };
        if check {
            if let Some(dry_run) = dry_run {
                assert_eq!(outcome, dry_run, "{context}: check disagrees with apply");
            }
            let after = engine.state();
            match outcome {
                Err(_) => {
                    assert_eq!(*after, before, "{context}: a refusal changed state");
                    assert_eq!(
                        engine.last_now(),
                        before_now,
                        "{context}: a refusal moved the clock"
                    );
                }
                Ok(Outcome::Changed) => {
                    assert_eq!(after.revision(), before.revision() + 1, "{context}");
                }
                Ok(Outcome::Unchanged) => {
                    assert_eq!(after.revision(), before.revision(), "{context}");
                    assert_eq!(
                        (
                            after.live(),
                            after.preview(),
                            after.program(),
                            after.unavailable_black()
                        ),
                        (
                            before.live(),
                            before.preview(),
                            before.program(),
                            before.unavailable_black()
                        ),
                        "{context}: Unchanged altered observable state"
                    );
                    assert_eq!(
                        after.auto_play().map(|run| run.scope),
                        before.auto_play().map(|run| run.scope),
                        "{context}"
                    );
                }
            }
            assert!(
                engine.last_now() >= before_now,
                "{context}: the clock ran backwards"
            );
            // A frame marked while on Program stays there; the exception ends
            // as soon as Program shows anything else.
            if let Step::Command(Command::FrameUnavailable { frame, .. }, _) = *step
                && matches!(before.program(), ProgramContent::Frame { frame: shown, .. } if shown == frame)
            {
                grandfathered = Some(frame);
            }
            grandfathered = grandfathered.filter(|frame| {
                matches!(after_program(&engine), ProgramContent::Frame { frame: shown, .. } if shown == *frame)
            });
            assert_invariants(loaded, &engine, step.time(), grandfathered, &context);
        }
        trace.push((outcome, engine.state().clone()));
    }
    trace
}

fn property_run(name: &str, seed: u64) {
    let loaded = load(name);
    let count = loaded.snapshot().cue_count();
    let frames: Vec<FrameId> = loaded.snapshot().frame_ids().into_iter().collect();
    let mut rng = XorShift(seed);
    for sequence in 0..200 {
        let steps = generate(&mut rng, 500, count, &frames);
        let label = format!("{name} sequence {sequence}");
        let first = run(&loaded, &steps, true, &label);
        let second = run(&loaded, &steps, false, &label);
        assert_eq!(first, second, "{label}: replay is not deterministic");
    }
}

#[test]
fn recovery_drill_holds_the_state_invariants_under_random_commands() {
    property_run("recovery-drill", 0x9E37_79B9_7F4A_7C15);
}

#[test]
fn normal_mass_holds_the_state_invariants_under_random_commands() {
    property_run("normal-mass", 0xD1B5_4A32_D192_ED03);
}

/// A dry run changes nothing, not even the clock.
#[test]
fn check_never_changes_the_engine() {
    let loaded = load("recovery-drill");
    let mut rng = XorShift(0x1234_5678_9ABC_DEF1);
    let frames: Vec<FrameId> = loaded.snapshot().frame_ids().into_iter().collect();
    let steps = generate(&mut rng, 500, loaded.snapshot().cue_count(), &frames);
    let mut engine = Engine::new(Arc::new(loaded.snapshot().clone()));
    for step in steps {
        match step {
            Step::Command(command, now) => {
                let before = engine.state().clone();
                let before_now = engine.last_now();
                let _ = engine.check(command, now);
                assert_eq!(*engine.state(), before);
                assert_eq!(engine.last_now(), before_now);
                let _ = engine.apply(command, now);
            }
            Step::Tick(now) => {
                engine.tick(now);
            }
        }
    }
}

// ---- Recovery Drill replay -------------------------------------------------

fn parse_step(line: &str) -> Option<Result<Command, u64>> {
    Some(match line {
        "next" => Ok(Command::Next),
        "previous" => Ok(Command::Previous),
        "next-item" => Ok(Command::NextItem),
        "previous-item" => Ok(Command::PreviousItem),
        "first" => Ok(Command::First),
        "last" => Ok(Command::Last),
        "take" => Ok(Command::Take),
        "freeze" => Ok(Command::ToggleFreeze),
        "blackout" => Ok(Command::ToggleBlackout),
        "enable-output" => Ok(Command::EnableOutput),
        "disable-output" => Ok(Command::DisableOutput),
        "disconnect" => Ok(Command::DisplayLost),
        "display-back" => Ok(Command::DisplayReturned { recognized: true }),
        "display-back-other" => Ok(Command::DisplayReturned { recognized: false }),
        "autoplay" => Ok(Command::ToggleAutoPlay),
        "loop-section" => Ok(Command::LoopSection),
        "stop-autoplay" => Ok(Command::StopAutoPlay),
        "recover" => Ok(Command::Recover),
        other => Err(other.strip_prefix("tick ")?.parse().ok()?),
    })
}

fn refusal_name(refusal: Refusal) -> String {
    match refusal {
        Refusal::UnavailablePreview { .. } => "UnavailablePreview".to_owned(),
        Refusal::AutoPlayIneligible { reason } => format!(
            "AutoPlayIneligible({})",
            match reason {
                AutoPlayRefusal::NotGeneralPresentation => "NotGeneralPresentation",
                AutoPlayRefusal::NoDurations => "NoDurations",
                AutoPlayRefusal::NothingProjectable => "NothingProjectable",
                AutoPlayRefusal::SectionHasIneligibleItem => "SectionHasIneligibleItem",
            }
        ),
        other => format!("{other:?}"),
    }
}

/// One expected-file line; the `engine-replay` example prints the same format.
fn state_line(
    number: usize,
    text: &str,
    state: &EngineState,
    outcome: Result<Outcome, Refusal>,
) -> String {
    let live = match state.live() {
        LiveState::Disconnected { display_back } => format!("Disconnected(back={display_back})"),
        other => format!("{other:?}"),
    };
    let program = match state.program() {
        ProgramContent::Black => "black".to_owned(),
        ProgramContent::Frame { cue, .. } => format!("frame:{cue}"),
    };
    let auto_play = match state.auto_play().map(|run| run.scope) {
        None => "none",
        Some(AutoPlayScope::Item(_)) => "item",
        Some(AutoPlayScope::Section(_)) => "section",
    };
    let outcome = match outcome {
        Ok(Outcome::Changed) => "changed".to_owned(),
        Ok(Outcome::Unchanged) => "unchanged".to_owned(),
        Err(error) => format!("refused:{}", refusal_name(error)),
    };
    format!(
        "{number} {text} -> live={live} preview={} program={program} unavailable_black={} autoplay={auto_play} outcome={outcome}",
        state.preview(),
        state.unavailable_black(),
    )
}

fn replay_lines(snapshot: &PresentationSnapshot, script: &str) -> Vec<String> {
    let mut engine = Engine::new(Arc::new(snapshot.clone()));
    let mut clock = 0u64;
    let mut lines = Vec::new();
    for (index, text) in script
        .lines()
        .filter(|line| !line.starts_with('#'))
        .enumerate()
    {
        let step = parse_step(text).unwrap_or_else(|| panic!("unknown command {text:?}"));
        let outcome = match step {
            Ok(command) => {
                clock += 100;
                engine.apply(command, Millis(clock))
            }
            Err(ms) => {
                clock += ms;
                Ok(engine.tick(Millis(clock)))
            }
        };
        lines.push(state_line(index + 1, text, engine.state(), outcome));
    }
    lines
}

#[test]
fn the_recovery_drill_replays_to_the_committed_states() {
    let dir = fixture_dir("recovery-drill");
    let loaded = load("recovery-drill");
    let script = fs::read_to_string(dir.join("commands.txt")).unwrap();
    let expected = fs::read_to_string(dir.join("commands.expect")).unwrap();
    assert!(expected.ends_with('\n') && !expected.contains('\r'));
    let actual = replay_lines(loaded.snapshot(), &script);
    let expected: Vec<&str> = expected.lines().collect();
    for (index, line) in actual.iter().enumerate() {
        assert_eq!(
            Some(line.as_str()),
            expected.get(index).copied(),
            "first differing line: {}",
            index + 1
        );
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "expected file has extra lines"
    );
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A frame that only a later Cue needs, once missing, leaves no snapshot to
/// drive an Engine at all: it can never surface as an error frame mid-service.
#[test]
fn a_missing_future_frame_is_rejected_before_any_engine_exists() {
    let temp = tempfile::tempdir().unwrap();
    let copy = temp.path().join("recovery-drill");
    copy_dir(&fixture_dir("recovery-drill"), &copy);
    // `lyrics.txt` is the frame of the Closing lyric Cue (flat 15) and of no other.
    fs::remove_file(copy.join("frames/lyrics.txt")).unwrap();
    let result = read_snapshot(&DirectorySource::new(&copy));
    assert!(
        matches!(result, Err(SnapshotLoadError::MissingDependency { .. })),
        "{:?}",
        result.map(|_| ())
    );
}
