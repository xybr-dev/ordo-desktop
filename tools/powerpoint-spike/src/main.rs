use powerpoint_spike::{export, fixture_plan};
use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, out] if command.as_str() == "fixture" => {
            let plan = fixture_plan();
            match export(Path::new(out), &plan) {
                Ok(()) => {
                    // Two safety slides plus one slide per Cue.
                    let slides = 2 + plan.items.iter().map(|item| item.cues.len()).sum::<usize>();
                    println!("wrote {slides} slides to {out}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("export failed: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: powerpoint-spike fixture <out.pptx>");
            ExitCode::from(2)
        }
    }
}
