//! `cargo xtask measure-presenter`: runs the Presenter performance acceptance
//! in release and leaves its measurement document at
//! `target/ordo-performance/report.json`, where CI uploads it from.
//!
//! The acceptance itself lives in `tests/performance`. This command only chooses
//! where the test writes, so the document and its path are one fixed contract.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::XtaskResult;

/// Where the measurement document is written, relative to the repository root.
pub const REPORT_RELATIVE_PATH: &str = "target/ordo-performance/report.json";

/// The environment variable that tells `performance-tests` where to write.
pub const REPORT_ENV: &str = "ORDO_PERFORMANCE_REPORT";

/// The absolute location of the measurement document under `root`.
pub fn report_path(root: &Path) -> PathBuf {
    root.join(REPORT_RELATIVE_PATH)
}

/// Runs the release acceptance and checks that it wrote the document.
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
    println!("Presenter performance report: {}", report.display());
    Ok(())
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
}
