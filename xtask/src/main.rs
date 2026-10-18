use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "help".to_owned());
    let result = match command.as_str() {
        "verify" => xtask::verify(),
        "context" => xtask::context(),
        "architecture" => xtask::verify_architecture(&xtask::repo_root()),
        "tokens" => xtask::verify_design_tokens(&xtask::repo_root()),
        "decisions" => xtask::verify_decision_records(&xtask::repo_root()),
        "offline" => xtask::verify_offline_contract(&xtask::repo_root())
            .and_then(|()| xtask::verify_gpui_supply(&xtask::repo_root())),
        "bless-golden-frames" => xtask::bless_golden_frames(&xtask::repo_root()),
        "measure-presenter" => xtask::measure_presenter::measure_presenter(&xtask::repo_root()),
        "qualify" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            xtask::qualify::qualify(&xtask::repo_root(), &args)
        }
        "stage-example" => {
            let mut args = std::env::args().skip(2);
            match (args.next(), args.next()) {
                (Some(example), Some(out_dir)) => xtask::measure_presenter::stage_example(
                    &xtask::repo_root(),
                    &example,
                    std::path::Path::new(&out_dir),
                ),
                _ => Err("usage: cargo xtask stage-example <example> <out-dir>".into()),
            }
        }
        "inspect-pptx" => match std::env::args().nth(2) {
            Some(path) => xtask::inspect_pptx::inspect_pptx(std::path::Path::new(&path)),
            None => Err("usage: cargo xtask inspect-pptx <file.pptx>".into()),
        },
        _ => {
            eprintln!(
                "Usage: cargo xtask <verify|context|architecture|tokens|decisions|offline|bless-golden-frames|measure-presenter|qualify [--skip <step>=<reason>]...|stage-example <example> <out-dir>|inspect-pptx>"
            );
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
