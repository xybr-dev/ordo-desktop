//! `cargo xtask qualify`: runs every automated Phase 0 proof that a Windows
//! runner can execute and writes one indexed qualification bundle under
//! `target/qualification/milestone-0/` (T0.10, `docs/plans/T0.10-implementation-plan.md`).
//!
//! Each run gets its own `runs/<UTC timestamp>-<short commit>/` directory, so
//! failures and reruns are retained. `latest.txt` names the newest run. Steps
//! run in the order of `STEPS`; a failing step does not stop the later ones.
//! Steps given with `--skip <step>=<reason>` are recorded as skipped and do
//! not fail the exit code, but the bundle is then not `complete`.
//!
//! Step output goes to `logs/<step>.log`. Both stdout and stderr are kept in
//! arrival order, which can interleave differently between runs.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fmt::Write as _,
    fs,
    io::{self, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cargo_metadata::MetadataCommand;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{XtaskResult, measure_presenter::REPORT_RELATIVE_PATH};

/// The bundle root, relative to the repository root.
pub const BUNDLE_RELATIVE_PATH: &str = "target/qualification/milestone-0";

/// Trees the provenance fingerprints (the T0.8 rule), relative to the root.
const FIXTURE_TREES: [&str; 5] = [
    "tests/fixtures",
    "tests/golden-frames",
    "tests/native-runtime/golden",
    "crates/rendering/tests/fixtures",
    "contracts",
];

/// The GPUI crates whose versions the provenance reads from `Cargo.lock`.
const GPUI_LOCK_PACKAGES: [&str; 3] = ["gpui-pre", "gpui-pre-platform", "gpui-base"];

/// Top-level provenance fields. Each must be present and non-empty.
const PROVENANCE_FIELDS: [&str; 10] = [
    "commit",
    "dirty",
    "rustc",
    "cargo",
    "toolchain_file",
    "gpui",
    "dependencies",
    "os",
    "gpu",
    "fixtures",
];

/// The schema of `index.json`. Bump when its shape changes.
const INDEX_SCHEMA: u32 = 2;

/// The longest one step may run before its command is killed and the step is
/// recorded as failed. Measurement and verify steps are long, so this is
/// generous.
const STEP_TIMEOUT: Duration = Duration::from_secs(120 * 60);

/// How long each output reader may keep running after its command has ended.
/// A grandchild that inherited the pipes can hold them open; the reader is
/// then detached instead of joined.
const READER_GRACE: Duration = Duration::from_secs(5);

/// How often the runner checks whether a command has exited.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The prefix of a provenance value that could not be read.
const UNKNOWN_PREFIX: &str = "unknown:";

/// The provenance problem for a dirty working tree.
const DIRTY_PROBLEM: &str = "working tree is dirty: qualification needs one clean commit";

/// One qualification step. `commands` run in order, and the step stops at
/// the first one that fails. `{artifacts}` in an argument is replaced with the
/// run's absolute `artifacts/` directory. `{xtask}` is replaced by the running
/// xtask executable, so no step rebuilds it.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub name: &'static str,
    pub commands: &'static [&'static [&'static str]],
    /// Repo-relative files copied into `artifacts/` under the given name.
    pub copies: &'static [(&'static str, &'static str)],
    /// Names under `artifacts/` that the commands write. A passing step must
    /// have written each one.
    pub produces: &'static [&'static str],
}

/// The walking-skeleton lab's file name under `artifacts/`, with the platform
/// executable suffix, as cargo names it.
const WALKING_SKELETON_LAB_ARTIFACT: &str = if cfg!(windows) {
    "walking-skeleton-lab.exe"
} else {
    "walking-skeleton-lab"
};

/// The Phase 0 proofs, in run order. Later phases append here.
pub const STEPS: &[Step] = &[
    Step {
        name: "verify",
        commands: &[&["{xtask}", "verify"]],
        copies: &[],
        produces: &[],
    },
    Step {
        name: "build-release",
        commands: &[&["cargo", "build", "-p", "ordo", "--release", "--locked"]],
        copies: &[],
        produces: &[],
    },
    Step {
        name: "canonical-invariance",
        commands: &[&[
            "cargo",
            "test",
            "-p",
            "native-runtime-tests",
            "--locked",
            "--test",
            "canonical_invariance",
        ]],
        copies: &[],
        produces: &[],
    },
    Step {
        name: "native-display",
        commands: &[&[
            "cargo",
            "test",
            "-p",
            "display-tests",
            "--locked",
            "--test",
            "native_display",
        ]],
        copies: &[],
        produces: &[],
    },
    Step {
        name: "shaping-probe",
        commands: &[&[
            "cargo",
            "test",
            "-p",
            "rendering",
            "--locked",
            "--test",
            "shaping_probe",
        ]],
        copies: &[],
        produces: &[],
    },
    Step {
        name: "measure-presenter",
        commands: &[&["{xtask}", "measure-presenter"]],
        copies: &[(REPORT_RELATIVE_PATH, "report.json")],
        produces: &[],
    },
    Step {
        name: "powerpoint-fixture",
        commands: &[
            &[
                "cargo",
                "run",
                "-p",
                "powerpoint-spike",
                "--locked",
                "--",
                "fixture",
                "{artifacts}/emergency-fixture.pptx",
            ],
            &[
                "{xtask}",
                "inspect-pptx",
                "{artifacts}/emergency-fixture.pptx",
            ],
        ],
        copies: &[],
        produces: &["emergency-fixture.pptx"],
    },
    Step {
        name: "walking-skeleton-lab",
        commands: &[&[
            "{xtask}",
            "stage-example",
            "walking-skeleton-lab",
            "{artifacts}",
        ]],
        copies: &[],
        produces: &[WALKING_SKELETON_LAB_ARTIFACT],
    },
];

/// The outcome of one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Fail,
    Missing,
    Skipped,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Missing => "missing",
            Self::Skipped => "skipped",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// One copied or produced artifact, with its SHA-256.
#[derive(Clone, Debug, Serialize)]
pub struct ArtifactRecord {
    pub path: String,
    pub sha256: String,
}

/// One step's entry in `index.json`.
#[derive(Clone, Debug, Serialize)]
pub struct StepRecord {
    pub name: String,
    pub status: Status,
    pub reason: Option<String>,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub log: String,
    pub artifacts: Vec<ArtifactRecord>,
}

/// The bundle index for one run.
#[derive(Clone, Debug, Serialize)]
pub struct Index {
    pub schema: u32,
    pub run: String,
    pub started_unix: u64,
    pub complete: bool,
    pub provenance: Value,
    pub provenance_problems: Vec<String>,
    /// Problems that do not fail the exit code, such as a dirty tree under
    /// `--allow-dirty`.
    pub warnings: Vec<String>,
    pub steps: Vec<StepRecord>,
}

/// Runs every step that is not skipped, writes the bundle, and fails if any
/// step failed or went missing, or if the provenance is incomplete.
pub fn qualify(root: &Path, args: &[String]) -> XtaskResult<()> {
    let options = parse_args(args)?;
    let provenance = provenance(root);
    let (provenance_problems, warnings) =
        split_dirty(provenance_problems(&provenance), options.allow_dirty);
    let started_unix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let run_name = format!(
        "{}-{}",
        utc_timestamp(started_unix),
        short_commit(&provenance)
    );

    let bundle = root.join(BUNDLE_RELATIVE_PATH);
    let run_dir = bundle.join("runs").join(&run_name);
    fs::create_dir_all(run_dir.join("logs"))?;
    fs::create_dir_all(run_dir.join("artifacts"))?;

    let xtask = env::current_exe()?.display().to_string();
    let context = Context {
        root,
        run_dir: &run_dir,
        run: &run_name,
        provenance: &provenance,
        xtask: &xtask,
    };
    let mut steps = Vec::with_capacity(STEPS.len());
    for step in STEPS {
        let record = match options.skips.get(step.name) {
            Some(reason) => skipped_record(&context, step.name, reason),
            None => {
                println!("==> qualify step {}", step.name);
                run_step(&context, step)
            }
        };
        println!("    {}: {}", record.name, record.status);
        steps.push(record);
    }

    let complete = is_complete(&steps, &provenance_problems, &provenance);
    let index = Index {
        schema: INDEX_SCHEMA,
        run: run_name,
        started_unix,
        complete,
        provenance,
        provenance_problems,
        warnings,
        steps,
    };
    fs::write(
        run_dir.join("index.json"),
        serde_json::to_string_pretty(&index)?,
    )?;
    fs::write(run_dir.join("index.md"), render_markdown(&index))?;
    fs::create_dir_all(&bundle)?;
    fs::write(
        bundle.join("latest.txt"),
        format!("{}\n", run_dir.display()),
    )?;
    println!("qualification bundle: {}", run_dir.display());
    for warning in &index.warnings {
        println!("warning: {warning}");
    }
    println!("complete: {complete}");

    let failing: Vec<String> = index
        .steps
        .iter()
        .filter(|step| matches!(step.status, Status::Fail | Status::Missing))
        .map(|step| format!("{} ({})", step.name, step.status))
        .collect();
    let mut problems = Vec::new();
    if !failing.is_empty() {
        problems.push(format!("failed or missing steps: {}", failing.join(", ")));
    }
    if !index.provenance_problems.is_empty() {
        problems.push(format!(
            "incomplete provenance: {}",
            index.provenance_problems.join("; ")
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("qualification failed: {}", problems.join("; ")).into())
    }
}

/// A bundle is complete only when every step passed, no provenance problem
/// is recorded, and the working tree was clean.
fn is_complete(steps: &[StepRecord], problems: &[String], provenance: &Value) -> bool {
    steps.iter().all(|step| step.status == Status::Pass)
        && problems.is_empty()
        && provenance["dirty"] == json!(false)
}

/// Splits the dirty-tree problem out of the provenance problems, returning
/// `(problems, warnings)`. With `allow_dirty` it becomes a warning and no
/// longer fails the exit code; `is_complete` still requires a clean tree.
fn split_dirty(problems: Vec<String>, allow_dirty: bool) -> (Vec<String>, Vec<String>) {
    if !allow_dirty {
        return (problems, Vec::new());
    }
    let (warnings, kept): (Vec<String>, Vec<String>) = problems
        .into_iter()
        .partition(|problem| problem == DIRTY_PROBLEM);
    (kept, warnings)
}

/// The parsed `cargo xtask qualify` arguments.
#[derive(Debug, Default)]
pub struct Options {
    /// Steps to skip, with the reason for each.
    pub skips: BTreeMap<String, String>,
    /// Record a dirty working tree as a warning instead of a failure.
    pub allow_dirty: bool,
}

/// Parses `--skip <step>=<reason>` (or `--skip=<step>=<reason>`) and
/// `--allow-dirty`. Unknown steps, missing reasons and other arguments are
/// errors.
pub fn parse_args(args: &[String]) -> XtaskResult<Options> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        if argument == "--allow-dirty" {
            options.allow_dirty = true;
            continue;
        }
        let spec = if argument == "--skip" {
            iter.next()
                .ok_or_else(|| "--skip needs <step>=<reason>".to_owned())?
                .as_str()
        } else if let Some(spec) = argument.strip_prefix("--skip=") {
            spec
        } else {
            return Err(format!(
                "unknown argument `{argument}`; usage: cargo xtask qualify [--allow-dirty] [--skip <step>=<reason>]..."
            )
            .into());
        };
        let (name, reason) = spec
            .split_once('=')
            .ok_or_else(|| format!("--skip `{spec}` must be <step>=<reason>"))?;
        let (name, reason) = (name.trim(), reason.trim());
        if !STEPS.iter().any(|step| step.name == name) {
            let known: Vec<&str> = STEPS.iter().map(|step| step.name).collect();
            return Err(format!(
                "--skip names unknown step `{name}`; steps are: {}",
                known.join(", ")
            )
            .into());
        }
        if reason.is_empty() {
            return Err(format!("--skip {name}= needs a reason").into());
        }
        options.skips.insert(name.to_owned(), reason.to_owned());
    }
    Ok(options)
}

/// Collects the provenance block. Each field is a value, or `unknown: <reason>`
/// when it cannot be read. It is never an empty string, and it never fails the
/// run by itself; `provenance_problems` decides that.
pub fn provenance(root: &Path) -> Value {
    let commit = text_field(capture("git", &["rev-parse", "HEAD"], root));
    let dirty = match capture("git", &["status", "--porcelain"], root) {
        Ok(text) => json!(!text.is_empty()),
        Err(error) => json!(format!("unknown: {error}")),
    };
    let rustc = match capture("rustc", &["-vV"], root).and_then(|text| {
        parse_rustc_version(&text).ok_or_else(|| "rustc -vV had no release or host line".to_owned())
    }) {
        Ok(version) => json!({"release": version.release, "host": version.host}),
        Err(error) => json!(format!("unknown: {error}")),
    };
    let cargo = text_field(capture("cargo", &["-V"], root));
    let toolchain_file = match fs::read_to_string(root.join("rust-toolchain.toml")) {
        Ok(text) => match parse_toolchain_channel(&text) {
            Some(channel) => json!(channel),
            None => json!("unknown: rust-toolchain.toml has no channel"),
        },
        Err(error) => json!(format!("unknown: {error}")),
    };
    let lock_text = fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default();

    json!({
        "commit": commit,
        "dirty": dirty,
        "rustc": rustc,
        "cargo": cargo,
        "toolchain_file": toolchain_file,
        "gpui": gpui_provenance(root, &lock_text),
        "dependencies": {
            "cargo_lock_sha256": text_field(
                sha256_file(&root.join("Cargo.lock")).map_err(|error| error.to_string())
            ),
        },
        "os": os_provenance(root),
        "gpu": gpu_provenance(root),
        "fixtures": fixtures_provenance(root),
    })
}

/// Every empty, missing, unknown or dirty provenance field, as a path. Empty
/// when the block is complete. Nulls, empty strings and empty containers all
/// count as empty, and so does any string that starts with `unknown:`.
pub fn provenance_problems(provenance: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    for field in PROVENANCE_FIELDS {
        if provenance.get(field).is_none() {
            problems.push(format!("provenance.{field} is missing"));
        }
    }
    collect_empty(provenance, "provenance", &mut problems);
    match provenance.get("dirty") {
        Some(Value::Bool(false)) => {}
        // An unreadable dirty state is already reported as unknown above.
        Some(Value::String(text)) if text.trim_start().starts_with(UNKNOWN_PREFIX) => {}
        _ => problems.push(DIRTY_PROBLEM.to_owned()),
    }
    problems
}

fn collect_empty(value: &Value, path: &str, problems: &mut Vec<String>) {
    match value {
        Value::Null => problems.push(format!("{path} is null")),
        Value::String(text) if text.trim().is_empty() => {
            problems.push(format!("{path} is empty"));
        }
        Value::String(text) if text.trim_start().starts_with(UNKNOWN_PREFIX) => {
            problems.push(format!("{path} is {}", text.trim()));
        }
        Value::Array(items) if items.is_empty() => problems.push(format!("{path} is empty")),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_empty(item, &format!("{path}[{index}]"), problems);
            }
        }
        Value::Object(map) if map.is_empty() => problems.push(format!("{path} is empty")),
        Value::Object(map) => {
            for (key, item) in map {
                collect_empty(item, &format!("{path}.{key}"), problems);
            }
        }
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// The 7-character commit prefix for the run directory name, or `unknown`
/// when the commit is not a full hash.
fn short_commit(provenance: &Value) -> String {
    match provenance["commit"].as_str() {
        Some(commit) if commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()) => {
            commit[..7].to_owned()
        }
        _ => "unknown".to_owned(),
    }
}

/// Everything a step needs from the run around it.
struct Context<'a> {
    root: &'a Path,
    run_dir: &'a Path,
    run: &'a str,
    provenance: &'a Value,
    /// The running xtask executable, which `{xtask}` expands to.
    xtask: &'a str,
}

/// The header that starts every step log: the facts that tie the log to one
/// commit and one run.
fn log_header(context: &Context<'_>, step: &str) -> String {
    format!(
        "# commit: {}\n# dirty: {}\n# run: {}\n# step: {}\n\n",
        text_of(&context.provenance["commit"]),
        text_of(&context.provenance["dirty"]),
        context.run,
        step
    )
}

/// A provenance value as plain text, without JSON quotes.
fn text_of(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

/// Expands one command template. `{xtask}` becomes `xtask` (the executable
/// path, or a display name), and `{artifacts}` inside an argument becomes the
/// artifacts directory.
fn expand(template: &[&str], artifacts: &str, xtask: &str) -> Vec<String> {
    template
        .iter()
        .map(|argument| match *argument {
            "{xtask}" => xtask.to_owned(),
            other => other.replace("{artifacts}", artifacts),
        })
        .collect()
}

/// Writes `artifacts/<name>.provenance.json` for one artifact and returns its
/// index record. The sidecar names the artifact, its SHA-256, the step and
/// run, and the whole provenance block.
fn write_sidecar(
    context: &Context<'_>,
    step: &str,
    name: &str,
    sha256: &str,
) -> io::Result<ArtifactRecord> {
    let path = format!("artifacts/{name}.provenance.json");
    let sidecar = json!({
        "artifact": format!("artifacts/{name}"),
        "sha256": sha256,
        "step": step,
        "run": context.run,
        "provenance": context.provenance,
    });
    let destination = context.run_dir.join(&path);
    fs::write(
        &destination,
        serde_json::to_string_pretty(&sidecar).map_err(io::Error::other)?,
    )?;
    Ok(ArtifactRecord {
        path,
        sha256: sha256_file(&destination)?,
    })
}

/// The index records for one artifact a step wrote: the artifact itself and
/// its provenance sidecar.
fn artifact_records(
    context: &Context<'_>,
    step: &str,
    name: &str,
) -> Result<Vec<ArtifactRecord>, String> {
    let sha256 = sha256_file(&context.run_dir.join("artifacts").join(name))
        .map_err(|error| format!("cannot hash {name}: {error}"))?;
    let sidecar = write_sidecar(context, step, name, &sha256)
        .map_err(|error| format!("cannot write the provenance sidecar for {name}: {error}"))?;
    Ok(vec![
        ArtifactRecord {
            path: format!("artifacts/{name}"),
            sha256,
        },
        sidecar,
    ])
}

/// Runs one step and records everything about it. Commands run in order and
/// the step stops at the first failure. A step that cannot start its command
/// is `missing`, and so is a passing step whose expected artifact is absent.
fn run_step(context: &Context<'_>, step: &Step) -> StepRecord {
    let started = Instant::now();
    let artifacts_dir = context.run_dir.join("artifacts");
    let artifacts_text = artifacts_dir.display().to_string();
    let log = Arc::new(Mutex::new(Vec::new()));

    let mut status = Status::Pass;
    let mut reason = None;
    let mut command_lines = Vec::new();
    let mut exit_code = None;
    for template in step.commands {
        let argv = expand(template, &artifacts_text, context.xtask);
        let display = expand(template, &artifacts_text, "cargo xtask").join(" ");
        command_lines.push(display.clone());
        match run_logged(context.root, &argv, &display, &log, STEP_TIMEOUT) {
            Err(error) => {
                status = Status::Missing;
                reason = Some(error);
                exit_code = None;
                break;
            }
            Ok(Outcome::TimedOut) => {
                status = Status::Fail;
                reason = Some(format!(
                    "timed out after {} minutes",
                    STEP_TIMEOUT.as_secs() / 60
                ));
                exit_code = None;
                break;
            }
            Ok(Outcome::Exited(code)) => {
                exit_code = code;
                if code != Some(0) {
                    status = Status::Fail;
                    reason = Some(match code {
                        Some(code) => format!("exit code {code}"),
                        None => "terminated without an exit code".to_owned(),
                    });
                    break;
                }
            }
        }
    }

    let mut artifacts = Vec::new();
    for (source, name) in step.copies {
        let source_path = context.root.join(source);
        let destination = artifacts_dir.join(name);
        if source_path.is_file() {
            let copied = fs::copy(&source_path, &destination)
                .map_err(|error| format!("cannot copy {source}: {error}"));
            match copied.and_then(|_| artifact_records(context, step.name, name)) {
                Ok(records) => artifacts.extend(records),
                Err(error) => {
                    status = Status::Fail;
                    reason = Some(error);
                }
            }
        } else if status == Status::Pass {
            status = Status::Missing;
            reason = Some(format!("expected {source} was not written"));
        }
    }
    for name in step.produces {
        if artifacts_dir.join(name).is_file() {
            match artifact_records(context, step.name, name) {
                Ok(records) => artifacts.extend(records),
                Err(error) => {
                    status = Status::Fail;
                    reason = Some(error);
                }
            }
        } else if status == Status::Pass {
            status = Status::Missing;
            reason = Some(format!("expected artifacts/{name} was not written"));
        }
    }

    let log_path = context
        .run_dir
        .join("logs")
        .join(format!("{}.log", step.name));
    let mut log_bytes = log_header(context, step.name).into_bytes();
    log_bytes.extend(log.lock().map(|bytes| bytes.clone()).unwrap_or_default());
    if let Err(error) = fs::write(&log_path, log_bytes) {
        status = Status::Fail;
        reason = Some(format!("cannot write the log: {error}"));
    }

    StepRecord {
        name: step.name.to_owned(),
        status,
        reason,
        command: command_lines,
        exit_code,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        log: format!("logs/{}.log", step.name),
        artifacts,
    }
}

/// The record for a step given with `--skip`. It still gets a log, which
/// holds the header and the reason. A log that cannot be written fails the
/// step rather than the run, as in `run_step`.
fn skipped_record(context: &Context<'_>, name: &str, reason: &str) -> StepRecord {
    let log_path = context.run_dir.join("logs").join(format!("{name}.log"));
    let written = fs::write(
        &log_path,
        format!("{}skipped: {reason}\n", log_header(context, name)),
    );
    let (status, reason) = match written {
        Ok(()) => (Status::Skipped, Some(reason.to_owned())),
        Err(error) => (Status::Fail, Some(format!("cannot write the log: {error}"))),
    };
    StepRecord {
        name: name.to_owned(),
        status,
        reason,
        command: Vec::new(),
        exit_code: None,
        duration_ms: 0,
        log: format!("logs/{name}.log"),
        artifacts: Vec::new(),
    }
}

/// How one command ended.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The command exited. `None` means it ended without an exit code.
    Exited(Option<i32>),
    /// The command outlived its timeout and was killed.
    TimedOut,
}

/// Runs one command with stdout and stderr both echoed to the console and
/// appended to `log`. `Err` means the command could not be started or waited
/// on. A command still running after `timeout` is killed. Its output readers
/// then get `READER_GRACE` to finish and are detached if they do not, so a
/// grandchild that keeps the pipes open cannot block the run.
fn run_logged(
    root: &Path,
    argv: &[String],
    display: &str,
    log: &Arc<Mutex<Vec<u8>>>,
    timeout: Duration,
) -> Result<Outcome, String> {
    append(log, &format!("$ {display}\n"));
    let (program, arguments) = argv
        .split_first()
        .ok_or_else(|| "empty command".to_owned())?;
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start {program}: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .map(|pipe| pump(pipe, Arc::clone(log), false));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| pump(pipe, Arc::clone(log), true));
    let started = Instant::now();
    let outcome = loop {
        match child
            .try_wait()
            .map_err(|error| format!("cannot wait for {program}: {error}"))?
        {
            Some(status) => break Outcome::Exited(status.code()),
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break Outcome::TimedOut;
            }
            None => thread::sleep(POLL_INTERVAL),
        }
    };

    let grace_ends = Instant::now() + READER_GRACE;
    for reader in [stdout, stderr].into_iter().flatten() {
        while !reader.is_finished() && Instant::now() < grace_ends {
            thread::sleep(POLL_INTERVAL);
        }
        if reader.is_finished() {
            let _ = reader.join();
        }
    }

    let note = match outcome {
        Outcome::Exited(code) => format!(
            "[exit {}]\n",
            code.map_or_else(|| "none".to_owned(), |code| code.to_string())
        ),
        Outcome::TimedOut => format!("[timed out after {} s; killed]\n", timeout.as_secs()),
    };
    append(log, &note);
    Ok(outcome)
}

/// Copies one pipe into the log and the console until it closes.
fn pump<R: Read + Send + 'static>(
    mut reader: R,
    log: Arc<Mutex<Vec<u8>>>,
    to_stderr: bool,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut chunk = [0_u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    let bytes = &chunk[..count];
                    if let Ok(mut buffer) = log.lock() {
                        buffer.extend_from_slice(bytes);
                    }
                    let _ = if to_stderr {
                        io::stderr().write_all(bytes)
                    } else {
                        io::stdout().write_all(bytes)
                    };
                }
            }
        }
    })
}

fn append(log: &Arc<Mutex<Vec<u8>>>, text: &str) {
    if let Ok(mut buffer) = log.lock() {
        buffer.extend_from_slice(text.as_bytes());
    }
}

/// Runs a short query command and returns its trimmed stdout.
fn capture(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} exited with {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// A non-empty text field, or `unknown: <reason>`.
fn text_field(result: Result<String, String>) -> Value {
    match result {
        Ok(text) if !text.trim().is_empty() => json!(text.trim()),
        Ok(_) => json!("unknown: empty output"),
        Err(error) => json!(format!("unknown: {error}")),
    }
}

fn gpui_provenance(root: &Path, lock_text: &str) -> Value {
    let supply = MetadataCommand::new()
        .current_dir(root)
        .no_deps()
        .other_options(vec!["--locked".to_owned()])
        .exec()
        .map_err(|error| error.to_string())
        .and_then(|metadata| {
            metadata
                .workspace_metadata
                .get("ordo")
                .and_then(|ordo| ordo.get("gpui-supply"))
                .cloned()
                .ok_or_else(|| "workspace.metadata.ordo.gpui-supply is missing".to_owned())
        });
    let supply = match supply {
        Ok(supply) => supply,
        Err(error) => json!(format!("unknown: {error}")),
    };

    let versions = parse_lock_versions(lock_text);
    let mut lock = serde_json::Map::new();
    for package in GPUI_LOCK_PACKAGES {
        // An absent package is recorded as an empty string, which the
        // validator then rejects.
        let version = versions
            .get(package)
            .map(|found| found.join(", "))
            .unwrap_or_default();
        lock.insert(package.to_owned(), json!(version));
    }
    json!({"supply": supply, "lock": lock})
}

fn os_provenance(root: &Path) -> Value {
    let version = if cfg!(windows) {
        capture("cmd", &["/C", "ver"], root)
    } else {
        Err("not a Windows host".to_owned())
    };
    json!({
        "ver": text_field(version),
        "architecture": text_field(
            env::var("PROCESSOR_ARCHITECTURE").map_err(|error| error.to_string())
        ),
    })
}

fn gpu_provenance(root: &Path) -> Value {
    let script = "Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion | ConvertTo-Json -Compress";
    match capture("powershell", &["-NoProfile", "-Command", script], root) {
        Ok(text) => match parse_video_controllers(&text) {
            Some(gpus) => json!(gpus),
            None => json!("unknown: Win32_VideoController reported no adapter"),
        },
        Err(error) => json!(format!("unknown: {error}")),
    }
}

fn fixtures_provenance(root: &Path) -> Value {
    let mut trees = serde_json::Map::new();
    let mut entries = Vec::new();
    for tree in FIXTURE_TREES {
        match tree_fingerprint(&root.join(tree)) {
            Ok(Some(hash)) => {
                trees.insert(tree.to_owned(), json!(hash));
                entries.push((tree.to_owned(), hash));
            }
            Ok(None) => {
                // An absent expected tree is a provenance problem, not a value.
                let text = format!("unknown: fixture tree {tree} is absent");
                trees.insert(tree.to_owned(), json!(text));
                entries.push((tree.to_owned(), text));
            }
            Err(error) => {
                let text = format!("unknown: {error}");
                trees.insert(tree.to_owned(), json!(text));
                entries.push((tree.to_owned(), text));
            }
        }
    }
    json!({
        "trees": trees,
        "aggregate_sha256": fingerprint(&entries),
    })
}

/// The T0.8 fingerprint rule: the hash of the lines `path\0sha256hex\n`, with
/// the paths sorted bytewise. The input order does not matter.
pub fn fingerprint(entries: &[(String, String)]) -> String {
    let sorted: BTreeMap<&str, &str> = entries
        .iter()
        .map(|(path, hash)| (path.as_str(), hash.as_str()))
        .collect();
    let mut hasher = Sha256::new();
    for (path, hash) in sorted {
        hasher.update(path.as_bytes());
        hasher.update([0_u8]);
        hasher.update(hash.as_bytes());
        hasher.update(b"\n");
    }
    hex(&hasher.finalize())
}

/// The fingerprint of every file under `dir`, with `/` separators. `None`
/// when the directory is absent. Symbolic links are not followed.
fn tree_fingerprint(dir: &Path) -> io::Result<Option<String>> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    collect_files(dir, dir, &mut entries)?;
    Ok(Some(fingerprint(&entries)))
}

fn collect_files(base: &Path, dir: &Path, entries: &mut Vec<(String, String)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(base, &path, entries)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(|error| io::Error::other(error.to_string()))?;
            let relative = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            entries.push((relative, sha256_file(&path)?));
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The release and host lines of `rustc -vV`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustcVersion {
    pub release: String,
    pub host: String,
}

/// Parses the `release:` and `host:` lines of `rustc -vV`. Both must be
/// present and non-empty.
pub fn parse_rustc_version(text: &str) -> Option<RustcVersion> {
    let mut release = None;
    let mut host = None;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.trim() {
                "release" if release.is_none() => release = Some(value.to_owned()),
                "host" if host.is_none() => host = Some(value.to_owned()),
                _ => {}
            }
        }
    }
    Some(RustcVersion {
        release: release?,
        host: host?,
    })
}

/// The `channel` value of `rust-toolchain.toml`.
pub fn parse_toolchain_channel(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key.trim() != "channel" {
            return None;
        }
        let value = value.trim().trim_matches('"').trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// The versions of each package in a `Cargo.lock`, by name. A package may
/// appear at more than one version, so each name maps to a list.
pub fn parse_lock_versions(lock: &str) -> BTreeMap<String, Vec<String>> {
    let mut versions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for block in lock.split("[[package]]").skip(1) {
        let mut name = None;
        let mut version = None;
        for line in block.lines() {
            let line = line.trim();
            if line.starts_with('[') && !line.starts_with("[[") {
                break;
            }
            if name.is_none() {
                name = quoted_value(line, "name").map(str::to_owned);
            }
            if version.is_none() {
                version = quoted_value(line, "version").map(str::to_owned);
            }
        }
        if let (Some(name), Some(version)) = (name, version) {
            versions.entry(name).or_default().push(version);
        }
    }
    versions
}

fn quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim()
        .strip_prefix('"')?
        .strip_suffix('"')
}

/// The `Name (driver version)` of each adapter in `ConvertTo-Json` output, or
/// `None` when there is none.
pub fn parse_video_controllers(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let items: Vec<&Value> = match &value {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    let described: Vec<String> = items
        .into_iter()
        .filter_map(|item| {
            let name = item.get("Name")?.as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            let driver = item
                .get("DriverVersion")
                .and_then(Value::as_str)
                .filter(|driver| !driver.trim().is_empty())
                .unwrap_or("unknown driver version");
            Some(format!("{name} (driver {driver})"))
        })
        .collect();
    (!described.is_empty()).then(|| described.join("; "))
}

/// The UTC `yyyymmddThhmmssZ` form of a Unix timestamp.
pub fn utc_timestamp(unix_seconds: u64) -> String {
    let days = i64::try_from(unix_seconds / 86_400).unwrap_or(i64::MAX);
    let seconds_of_day = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        seconds_of_day / 3600,
        seconds_of_day % 3600 / 60,
        seconds_of_day % 60
    )
}

/// The civil (year, month, day) of a day count since 1970-01-01, using
/// Howard Hinnant's proleptic Gregorian algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// The Markdown form of an index: skipped steps first, then the provenance
/// table, then the steps table.
pub fn render_markdown(index: &Index) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Milestone 0 qualification run\n");
    let _ = writeln!(out, "- Run: `{}`", index.run);
    let _ = writeln!(out, "- Schema: {}", index.schema);
    let _ = writeln!(out, "- **complete: {}**\n", index.complete);

    let skipped: Vec<&StepRecord> = index
        .steps
        .iter()
        .filter(|step| step.status == Status::Skipped)
        .collect();
    if !skipped.is_empty() {
        let _ = writeln!(
            out,
            "## Skipped steps\n\n> **{} step(s) skipped. Skipped steps are not proven by this run, so the bundle is not complete.**\n",
            skipped.len()
        );
        for step in skipped {
            let _ = writeln!(
                out,
                "- **{}**: {}",
                step.name,
                step.reason.as_deref().unwrap_or("no reason recorded")
            );
        }
        out.push('\n');
    }

    if !index.provenance_problems.is_empty() {
        let _ = writeln!(out, "## Provenance problems\n");
        for problem in &index.provenance_problems {
            let _ = writeln!(out, "- {}", md_cell(problem));
        }
        out.push('\n');
    }

    if !index.warnings.is_empty() {
        let _ = writeln!(out, "## Warnings\n");
        for warning in &index.warnings {
            let _ = writeln!(out, "- {}", md_cell(warning));
        }
        out.push('\n');
    }

    let mut rows = Vec::new();
    flatten(&index.provenance, "", &mut rows);
    let _ = writeln!(out, "## Provenance\n\n| Field | Value |\n| --- | --- |");
    for (field, value) in rows {
        let _ = writeln!(out, "| `{}` | {} |", md_cell(&field), md_cell(&value));
    }

    let _ = writeln!(
        out,
        "\n## Steps\n\n| Step | Status | Duration (ms) | Exit | Reason | Log | Artifacts |\n| --- | --- | --- | --- | --- | --- | --- |"
    );
    for step in &index.steps {
        let exit = step
            .exit_code
            .map_or_else(|| "-".to_owned(), |code| code.to_string());
        let artifacts = if step.artifacts.is_empty() {
            "-".to_owned()
        } else {
            step.artifacts
                .iter()
                .map(|artifact| format!("`{}` ({})", artifact.path, &artifact.sha256[..12]))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | [{}]({}) | {} |",
            md_cell(step.name.as_str()),
            step.status,
            step.duration_ms,
            exit,
            md_cell(step.reason.as_deref().unwrap_or("-")),
            step.log,
            step.log,
            artifacts
        );
    }
    out
}

/// Flattens nested provenance objects into dotted `(field, value)` rows.
fn flatten(value: &Value, prefix: &str, rows: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, item) in map {
                let field = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(item, &field, rows);
            }
        }
        Value::String(text) => rows.push((prefix.to_owned(), text.clone())),
        other => rows.push((prefix.to_owned(), other.to_string())),
    }
}

/// Makes a value safe for one Markdown table cell.
fn md_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// The steps named in `STEPS`, for error messages and tests.
pub fn step_names() -> BTreeSet<&'static str> {
    STEPS.iter().map(|step| step.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        env::temp_dir().join(format!("ordo-qualify-test-{}-{name}", std::process::id()))
    }

    fn sample_provenance() -> Value {
        json!({
            "commit": "0123456789abcdef0123456789abcdef01234567",
            "dirty": false,
            "rustc": {"release": "1.96.0", "host": "x86_64-pc-windows-msvc"},
            "cargo": "cargo 1.96.0",
            "toolchain_file": "1.96.0",
            "gpui": {
                "supply": {"gpui-pre": "0.3.8"},
                "lock": {"gpui-base": "0.7.1", "gpui-pre": "0.3.8", "gpui-pre-platform": "0.3.8"}
            },
            "dependencies": {"cargo_lock_sha256": "ab"},
            "os": {"ver": "Microsoft Windows", "architecture": "AMD64"},
            "gpu": "NVIDIA RTX (driver 555.85)",
            "fixtures": {"trees": {"contracts": "cd"}, "aggregate_sha256": "cd"}
        })
    }

    fn step(name: &str, status: Status, reason: Option<&str>) -> StepRecord {
        StepRecord {
            name: name.to_owned(),
            status,
            reason: reason.map(str::to_owned),
            command: Vec::new(),
            exit_code: None,
            duration_ms: 0,
            log: format!("logs/{name}.log"),
            artifacts: Vec::new(),
        }
    }

    #[test]
    fn fingerprint_is_deterministic_and_order_independent() {
        let entries = vec![
            ("b.txt".to_owned(), "22".to_owned()),
            ("a/c.txt".to_owned(), "11".to_owned()),
            ("a.txt".to_owned(), "33".to_owned()),
        ];
        let mut reversed = entries.clone();
        reversed.reverse();

        let first = fingerprint(&entries);
        assert_eq!(first, fingerprint(&entries));
        assert_eq!(first, fingerprint(&reversed));
    }

    #[test]
    fn fingerprint_depends_on_paths_and_hashes() {
        let base = vec![("a.txt".to_owned(), "11".to_owned())];
        let renamed = vec![("b.txt".to_owned(), "11".to_owned())];
        let changed = vec![("a.txt".to_owned(), "12".to_owned())];
        assert_ne!(fingerprint(&base), fingerprint(&renamed));
        assert_ne!(fingerprint(&base), fingerprint(&changed));
    }

    #[test]
    fn fingerprint_uses_the_documented_line_format() {
        let entries = vec![("a".to_owned(), "h".to_owned())];
        let mut hasher = Sha256::new();
        hasher.update(b"a\0h\n");
        assert_eq!(fingerprint(&entries), hex(&hasher.finalize()));
    }

    #[test]
    fn tree_fingerprint_uses_slash_separators_and_reports_absence() {
        let dir = scratch("tree");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("top.txt"), b"top").unwrap();
        fs::write(dir.join("sub").join("inner.txt"), b"inner").unwrap();

        let expected = fingerprint(&[
            ("sub/inner.txt".to_owned(), hex(&Sha256::digest(b"inner"))),
            ("top.txt".to_owned(), hex(&Sha256::digest(b"top"))),
        ]);
        assert_eq!(tree_fingerprint(&dir).unwrap(), Some(expected));
        assert_eq!(tree_fingerprint(&dir.join("missing")).unwrap(), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_rustc_release_and_host_lines() {
        let text = "rustc 1.96.0 (abcdef 2026-05-01)\nbinary: rustc\ncommit-hash: abcdef\nrelease: 1.96.0\nhost: x86_64-pc-windows-msvc\nLLVM version: 20.1\n";
        assert_eq!(
            parse_rustc_version(text),
            Some(RustcVersion {
                release: "1.96.0".to_owned(),
                host: "x86_64-pc-windows-msvc".to_owned(),
            })
        );
        assert_eq!(parse_rustc_version("rustc 1.96.0\nrelease: 1.96.0\n"), None);
        assert_eq!(parse_rustc_version("release: \nhost: x\n"), None);
    }

    #[test]
    fn parses_toolchain_channel() {
        let text = "[toolchain]\nchannel = \"1.96.0\"\nprofile = \"minimal\"\n";
        assert_eq!(parse_toolchain_channel(text), Some("1.96.0".to_owned()));
        assert_eq!(
            parse_toolchain_channel("[toolchain]\nprofile = \"x\"\n"),
            None
        );
    }

    #[test]
    fn parses_cargo_lock_versions_per_package() {
        let lock = "version = 4\n\n[[package]]\nname = \"gpui-pre\"\nversion = \"0.3.8\"\nsource = \"registry+x\"\ndependencies = [\n \"gpui-pre-macros 0.3.8\",\n]\n\n[[package]]\nname = \"other\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"other\"\nversion = \"2.0.0\"\n";
        let versions = parse_lock_versions(lock);
        assert_eq!(versions["gpui-pre"], vec!["0.3.8".to_owned()]);
        assert_eq!(
            versions["other"],
            vec!["1.0.0".to_owned(), "2.0.0".to_owned()]
        );
        assert!(!versions.contains_key("gpui-base"));
    }

    #[test]
    fn parses_video_controllers_from_array_or_single_object() {
        let array = r#"[{"Name":"NVIDIA RTX","DriverVersion":"555.85"},{"Name":"Basic Display","DriverVersion":null}]"#;
        assert_eq!(
            parse_video_controllers(array),
            Some(
                "NVIDIA RTX (driver 555.85); Basic Display (driver unknown driver version)"
                    .to_owned()
            )
        );
        assert_eq!(
            parse_video_controllers(r#"{"Name":"Intel","DriverVersion":"1.2"}"#),
            Some("Intel (driver 1.2)".to_owned())
        );
        assert_eq!(parse_video_controllers(""), None);
    }

    #[test]
    fn civil_dates_match_known_epochs() {
        assert_eq!(utc_timestamp(0), "19700101T000000Z");
        assert_eq!(utc_timestamp(951_782_400), "20000229T000000Z");
        assert_eq!(utc_timestamp(1_000_000_000), "20010909T014640Z");
        assert_eq!(utc_timestamp(1_700_000_000), "20231114T221320Z");
        assert_eq!(utc_timestamp(1_709_251_199), "20240229T235959Z");
        assert_eq!(utc_timestamp(1_709_251_200), "20240301T000000Z");
    }

    #[test]
    fn index_marks_incomplete_when_a_step_is_skipped() {
        let index = Index {
            schema: INDEX_SCHEMA,
            run: "run".to_owned(),
            started_unix: 0,
            complete: false,
            provenance: sample_provenance(),
            provenance_problems: Vec::new(),
            warnings: Vec::new(),
            steps: vec![
                step("verify", Status::Skipped, Some("covered by the verify job")),
                step("build-release", Status::Pass, None),
            ],
        };
        let markdown = render_markdown(&index);
        assert!(markdown.contains("**complete: false**"));
        assert!(markdown.contains("## Skipped steps"));
        assert!(markdown.contains("**verify**: covered by the verify job"));
        assert!(markdown.contains("| `gpui.lock.gpui-pre` | 0.3.8 |"));
    }

    #[test]
    fn provenance_validator_rejects_empty_fields() {
        assert!(provenance_problems(&sample_provenance()).is_empty());

        let mut empty_commit = sample_provenance();
        empty_commit["commit"] = json!("");
        assert_eq!(
            provenance_problems(&empty_commit),
            vec!["provenance.commit is empty".to_owned()]
        );

        let mut empty_lock = sample_provenance();
        empty_lock["gpui"]["lock"]["gpui-base"] = json!("");
        assert_eq!(
            provenance_problems(&empty_lock),
            vec!["provenance.gpui.lock.gpui-base is empty".to_owned()]
        );

        let mut missing = sample_provenance();
        missing.as_object_mut().unwrap().remove("os");
        assert_eq!(
            provenance_problems(&missing),
            vec!["provenance.os is missing".to_owned()]
        );
    }

    #[test]
    fn skip_arguments_are_validated() {
        let skips = parse_args(&[
            "--skip".to_owned(),
            "verify=covered by the verify job".to_owned(),
            "--skip=measure-presenter=covered elsewhere".to_owned(),
        ])
        .unwrap()
        .skips;
        assert_eq!(skips["verify"], "covered by the verify job");
        assert_eq!(skips["measure-presenter"], "covered elsewhere");

        assert!(parse_args(&["--skip".to_owned(), "nonsense=x".to_owned()]).is_err());
        assert!(parse_args(&["--skip".to_owned(), "verify=".to_owned()]).is_err());
        assert!(parse_args(&["--skip".to_owned(), "verify".to_owned()]).is_err());
        assert!(parse_args(&["--other".to_owned()]).is_err());
    }

    #[test]
    fn unknown_provenance_values_are_problems() {
        let mut unknown = sample_provenance();
        unknown["gpu"] = json!("unknown: test");
        assert_eq!(
            provenance_problems(&unknown),
            vec!["provenance.gpu is unknown: test".to_owned()]
        );
        // The value is still recorded in the bundle; only the problem list grows.
        assert_eq!(unknown["gpu"], json!("unknown: test"));
    }

    #[test]
    fn dirty_tree_fails_unless_allowed_and_never_completes() {
        let mut dirty = sample_provenance();
        dirty["dirty"] = json!(true);
        let problems = provenance_problems(&dirty);
        assert_eq!(problems, vec![DIRTY_PROBLEM.to_owned()]);

        let passing = [step("verify", Status::Pass, None)];
        let (kept, warnings) = split_dirty(problems.clone(), false);
        assert_eq!(kept, problems);
        assert!(warnings.is_empty());
        assert!(!is_complete(&passing, &kept, &dirty));

        let (kept, warnings) = split_dirty(problems, true);
        assert!(kept.is_empty());
        assert_eq!(warnings, vec![DIRTY_PROBLEM.to_owned()]);
        assert!(!is_complete(&passing, &kept, &dirty));

        let clean = sample_provenance();
        assert!(provenance_problems(&clean).is_empty());
        assert!(is_complete(&passing, &[], &clean));
        assert!(!is_complete(
            &[step("verify", Status::Fail, Some("exit code 1"))],
            &[],
            &clean
        ));

        assert!(
            parse_args(&["--allow-dirty".to_owned()])
                .unwrap()
                .allow_dirty
        );
        assert!(!parse_args(&[]).unwrap().allow_dirty);
    }

    #[test]
    fn sidecar_records_full_provenance_and_artifact_hash() {
        let dir = scratch("sidecar");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("artifacts")).unwrap();
        fs::write(dir.join("artifacts").join("report.json"), b"{}").unwrap();
        let sha = sha256_file(&dir.join("artifacts").join("report.json")).unwrap();

        let provenance = sample_provenance();
        let context = Context {
            root: &dir,
            run_dir: &dir,
            run: "run-1",
            provenance: &provenance,
            xtask: "xtask",
        };
        let record = write_sidecar(&context, "measure-presenter", "report.json", &sha).unwrap();
        assert_eq!(record.path, "artifacts/report.json.provenance.json");

        let text = fs::read_to_string(dir.join(&record.path)).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["artifact"], json!("artifacts/report.json"));
        assert_eq!(value["sha256"], json!(sha));
        assert_eq!(value["step"], json!("measure-presenter"));
        assert_eq!(value["run"], json!("run-1"));
        assert_eq!(value["provenance"], provenance);
        assert_eq!(record.sha256, sha256_file(&dir.join(&record.path)).unwrap());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_header_names_commit_dirty_run_and_step() {
        let provenance = sample_provenance();
        let context = Context {
            root: Path::new("."),
            run_dir: Path::new("."),
            run: "run-1",
            provenance: &provenance,
            xtask: "xtask",
        };
        let header = log_header(&context, "verify");
        assert!(header.contains("# commit: 0123456789abcdef0123456789abcdef01234567\n"));
        assert!(header.contains("# dirty: false\n"));
        assert!(header.contains("# run: run-1\n"));
        assert!(header.contains("# step: verify\n"));
    }

    #[test]
    fn expand_replaces_xtask_and_artifacts_tokens() {
        let argv = expand(
            &["{xtask}", "inspect-pptx", "{artifacts}/deck.pptx"],
            "/runs/artifacts",
            "/target/xtask.exe",
        );
        assert_eq!(
            argv,
            vec![
                "/target/xtask.exe".to_owned(),
                "inspect-pptx".to_owned(),
                "/runs/artifacts/deck.pptx".to_owned(),
            ]
        );
    }

    #[test]
    fn run_logged_kills_a_command_that_outlives_its_timeout() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let argv: Vec<String> = if cfg!(windows) {
            vec![
                "cmd".to_owned(),
                "/C".to_owned(),
                "ping -n 30 127.0.0.1 >NUL".to_owned(),
            ]
        } else {
            vec!["sleep".to_owned(), "30".to_owned()]
        };
        let outcome = run_logged(
            &env::temp_dir(),
            &argv,
            "sleep",
            &log,
            Duration::from_millis(300),
        )
        .unwrap();
        assert_eq!(outcome, Outcome::TimedOut);
        let text = String::from_utf8_lossy(&log.lock().unwrap()).into_owned();
        assert!(text.contains("[timed out after 0 s; killed]"));
    }

    #[test]
    fn step_names_are_unique() {
        assert_eq!(step_names().len(), STEPS.len());
    }
}
