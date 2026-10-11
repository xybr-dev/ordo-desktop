//! No blocking worker wait outside worker threads and shutdown (T0.6 hot path).
//!
//! The runtime hot-path probe counts work by kind, but a blocking wait has no
//! single choke point to note. This test closes that gap statically: every
//! blocking receive, blocking send, `block_on`, or thread join in production
//! `ordo` source must be one of the reviewed sites below, each of which runs
//! on a worker thread or during shutdown, never between keydown and paint.
//! A new site fails the test until it is reviewed and listed here.

use std::path::{Path, PathBuf};

/// Calls that park the calling thread until another thread answers.
const BLOCKING: [&str; 6] = [
    ".recv()",
    ".recv_timeout(",
    ".recv_blocking(",
    ".send_blocking(",
    "block_on(",
    ".join()",
];

/// Reviewed production sites: (file relative to `src/`, trimmed line, why).
const REVIEWED: [(&str, &str, &str); 6] = [
    (
        "canonical.rs",
        "while let Ok(command) = requests.recv() {",
        "canonical worker thread loop",
    ),
    (
        "lib.rs",
        "let _ = restored_preferences.send_blocking(preferences.read());",
        "preference worker thread, startup restore",
    ),
    (
        "lib.rs",
        "while let Ok(PreferenceCommand::Save(contents)) = commands.recv() {",
        "preference worker thread loop",
    ),
    (
        "lib.rs",
        "if let Err(failure) = worker.join() {",
        "runtime shutdown",
    ),
    (
        "performance/live.rs",
        "let _ = thread.join();",
        "thumbnail worker drop at probe shutdown",
    ),
    (
        "performance/live.rs",
        "while let Ok(command) = requests.recv_blocking() {",
        "thumbnail worker thread loop",
    ),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source directory must be readable") {
        let path = entry.expect("directory entry must be readable").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Production lines only: everything before the first `#[cfg(test)]` or
/// `#[cfg(all(test` attribute. Test modules sit at the end of each file.
fn production_lines(source: &str) -> impl Iterator<Item = &str> {
    source
        .lines()
        .take_while(|line| {
            let line = line.trim_start();
            !line.starts_with("#[cfg(test)]") && !line.starts_with("#[cfg(all(test")
        })
        .map(str::trim)
}

#[test]
fn every_blocking_wait_in_production_source_is_reviewed() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    files.sort();

    let mut found = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("source file must be readable");
        let relative = file
            .strip_prefix(&src)
            .expect("file is under src")
            .to_string_lossy()
            .replace('\\', "/");
        for line in production_lines(&source) {
            let is_comment = line.starts_with("//");
            let is_async = line.contains(".await");
            if !is_comment && !is_async && BLOCKING.iter().any(|call| line.contains(call)) {
                found.push((relative.clone(), line.to_owned()));
            }
        }
    }

    let unreviewed: Vec<_> = found
        .iter()
        .filter(|(file, line)| !REVIEWED.iter().any(|(f, l, _)| f == file && l == line))
        .collect();
    assert!(
        unreviewed.is_empty(),
        "blocking waits need review before they can run near the hot path: {unreviewed:#?}"
    );

    let stale: Vec<_> = REVIEWED
        .iter()
        .filter(|(f, l, _)| !found.iter().any(|(file, line)| file == f && line == l))
        .collect();
    assert!(
        stale.is_empty(),
        "reviewed sites no longer exist; remove them: {stale:#?}"
    );
}
