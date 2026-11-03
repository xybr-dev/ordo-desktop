//! Replays a snapshot directory's `commands.txt` against the Presentation Engine
//! and prints one state line per command, in the format of `commands.expect`:
//! `cargo run -p presentation --example engine-replay -- <snapshot directory>`.
//!
//! The clock starts at 0 ms and advances 100 ms per line; `tick <ms>` advances
//! it by `<ms>` and ticks the Engine instead of applying a command. After an
//! Engine change, regenerate the expected file with
//! `... > <directory>/commands.expect` and review the diff by hand.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use presentation::commands::Command;
use presentation::engine::{AutoPlayRefusal, AutoPlayScope, Engine, Outcome, Refusal};
use presentation::snapshot_reader::{DirectorySource, read_snapshot};
use presentation::state::{LiveState, Millis, ProgramContent};

fn parse(line: &str) -> Option<Result<Command, u64>> {
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

fn refusal(refusal: Refusal) -> String {
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

fn line(number: usize, text: &str, engine: &Engine, outcome: Result<Outcome, Refusal>) -> String {
    let state = engine.state();
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
        Err(error) => format!("refused:{}", refusal(error)),
    };
    format!(
        "{number} {text} -> live={live} preview={} program={program} unavailable_black={} autoplay={auto_play} outcome={outcome}",
        state.preview(),
        state.unavailable_black(),
    )
}

fn replay(dir: &Path) -> Result<String, String> {
    let loaded = read_snapshot(&DirectorySource::new(dir))
        .map_err(|error| format!("snapshot rejected: {error}"))?;
    let script = fs::read_to_string(dir.join("commands.txt")).map_err(|error| error.to_string())?;
    let mut engine = Engine::new(Arc::new(loaded.snapshot().clone()));
    let mut clock = 0u64;
    let mut out = String::new();
    let mut number = 0;
    for text in script.lines().filter(|text| !text.starts_with('#')) {
        number += 1;
        let step = parse(text).ok_or_else(|| format!("unknown command: {text:?}"))?;
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
        let _ = writeln!(out, "{}", line(number, text, &engine, outcome));
    }
    Ok(out)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(dir), None) = (args.next(), args.next()) else {
        eprintln!("usage: engine-replay <snapshot directory with commands.txt>");
        return ExitCode::from(2);
    };
    match replay(Path::new(&dir)) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
