//! The Recovery Drill script format: parsing `commands.txt` and formatting the
//! state lines of `commands.expect`.
//!
//! The Engine replay test (`tests/presentation/tests/state_model.rs`) and the
//! `engine-replay` example carry their own copies of [`parse_step`] and
//! [`state_line`]; this one serves the callers that already enable
//! `test-support` (the Presenter's drill test and `live-control-lab
//! --run-drill`). All three must format identically, and the test below holds
//! this copy to the committed `commands.expect`.

use crate::commands::Command;
use crate::engine::{AutoPlayRefusal, AutoPlayScope, Outcome, Refusal};
use crate::state::{EngineState, LiveState, ProgramContent};

/// Milliseconds the drill clock advances before each command line.
pub const COMMAND_MS: u64 = 100;

/// One non-comment line of `commands.txt`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Apply this command, [`COMMAND_MS`] after the previous line.
    Command(Command),
    /// Let this many milliseconds pass and tick the Engine.
    Tick(u64),
}

/// The non-comment lines of a drill script, in order.
pub fn script_lines(script: &str) -> impl Iterator<Item = &str> {
    script.lines().filter(|line| !line.starts_with('#'))
}

/// The step a script line names, or `None` for an unknown line.
pub fn parse_step(line: &str) -> Option<Step> {
    let command = match line {
        "next" => Command::Next,
        "previous" => Command::Previous,
        "next-item" => Command::NextItem,
        "previous-item" => Command::PreviousItem,
        "first" => Command::First,
        "last" => Command::Last,
        "take" => Command::Take,
        "freeze" => Command::ToggleFreeze,
        "blackout" => Command::ToggleBlackout,
        "enable-output" => Command::EnableOutput,
        "disable-output" => Command::DisableOutput,
        "disconnect" => Command::DisplayLost,
        "display-back" => Command::DisplayReturned { recognized: true },
        "display-back-other" => Command::DisplayReturned { recognized: false },
        "autoplay" => Command::ToggleAutoPlay,
        "loop-section" => Command::LoopSection,
        "stop-autoplay" => Command::StopAutoPlay,
        "recover" => Command::Recover,
        other => return Some(Step::Tick(other.strip_prefix("tick ")?.parse().ok()?)),
    };
    Some(Step::Command(command))
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

/// One line of `commands.expect`: the state after script line `number`.
pub fn state_line(
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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use super::*;
    use crate::engine::Engine;
    use crate::snapshot_reader::{DirectorySource, read_snapshot};
    use crate::state::Millis;

    fn drill_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/presentations/recovery-drill")
    }

    #[test]
    fn every_committed_line_parses_and_matches_the_expected_file_in_count() {
        let script = std::fs::read_to_string(drill_dir().join("commands.txt")).unwrap();
        let expected = std::fs::read_to_string(drill_dir().join("commands.expect")).unwrap();
        let lines: Vec<&str> = script_lines(&script).collect();
        for line in &lines {
            assert!(parse_step(line).is_some(), "unknown drill line {line:?}");
        }
        assert_eq!(lines.len(), expected.lines().count());
        assert!(parse_step("tick").is_none() && parse_step("tick x").is_none());
        assert!(parse_step("unknown").is_none());
    }

    #[test]
    fn this_copy_formats_the_committed_expected_states() {
        let loaded = read_snapshot(&DirectorySource::new(drill_dir())).unwrap();
        let script = std::fs::read_to_string(drill_dir().join("commands.txt")).unwrap();
        let expected = std::fs::read_to_string(drill_dir().join("commands.expect")).unwrap();
        let mut engine = Engine::new(Arc::new(loaded.snapshot().clone()));
        let mut clock = 0;
        let mut actual = String::new();
        for (index, text) in script_lines(&script).enumerate() {
            let outcome = match parse_step(text).unwrap() {
                Step::Command(command) => {
                    clock += COMMAND_MS;
                    engine.apply(command, Millis(clock))
                }
                Step::Tick(ms) => {
                    clock += ms;
                    Ok(engine.tick(Millis(clock)))
                }
            };
            actual.push_str(&state_line(index + 1, text, engine.state(), outcome));
            actual.push('\n');
        }
        assert_eq!(actual, expected);
    }
}
