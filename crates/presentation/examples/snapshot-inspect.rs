//! Prints what the snapshot reader accepts: `cargo run -p presentation --example
//! snapshot-inspect -- <snapshot directory>`.
//!
//! On a load failure the typed error is printed to stderr and the exit code is
//! 1, so the example doubles as the way to learn which file to fix (and which
//! digest to paste) after editing a fixture.

use std::process::ExitCode;

use domain::presentation::ProgramEffect;
use presentation::snapshot_reader::{DirectorySource, read_snapshot};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(dir), None) = (args.next(), args.next()) else {
        eprintln!("usage: snapshot-inspect <snapshot directory>");
        return ExitCode::from(2);
    };
    let loaded = match read_snapshot(&DirectorySource::new(dir)) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("snapshot rejected: {error}");
            return ExitCode::FAILURE;
        }
    };

    let snapshot = loaded.snapshot();
    println!("title: {}", snapshot.title());
    println!("fingerprint: {}", loaded.fingerprint());
    for position in snapshot.positions() {
        let section = &snapshot.sections()[position.section];
        let item = &section.items()[position.item];
        let cue = &item.cues()[position.cue];
        let effect = match cue.program_effect() {
            ProgramEffect::Frame(frame) => format!("frame {frame}"),
            ProgramEffect::Black => "black".to_owned(),
            ProgramEffect::Unchanged => "unchanged".to_owned(),
            ProgramEffect::NotPresentable => "not presentable".to_owned(),
        };
        println!(
            "{} {} / {} / {} -> {effect}",
            position.flat,
            section.title(),
            item.title(),
            cue.label()
        );
        if let Some(private) = cue.private_context() {
            println!("    private: {private}");
        }
    }
    ExitCode::SUCCESS
}
