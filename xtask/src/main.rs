use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "help".to_owned());
    let result = match command.as_str() {
        "verify" => xtask::verify(),
        "context" => xtask::context(),
        "architecture" => xtask::verify_architecture(&xtask::repo_root()),
        "offline" => xtask::verify_offline_contract(&xtask::repo_root())
            .and_then(|()| xtask::verify_git_pins(&xtask::repo_root())),
        _ => {
            eprintln!("Usage: cargo xtask <verify|context|architecture|offline>");
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
