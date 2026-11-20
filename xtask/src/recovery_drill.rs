//! `cargo xtask recovery-drill`: kills the real, packaged `ordo.exe` mid
//! Presentation and relaunches it, to check that it reopens in the recovered
//! Presenter (Phase 1 gate, T1.7).
//!
//! `recovery_e2e.rs` restarts inside the GPUI test host. Nothing there ends a
//! process, so this command closes the gap: it builds `ordo` in release,
//! stages the executable with the `normal-mass` fixture beside it (the
//! Presenter prefers `fixtures/normal-mass` beside the executable), and runs
//! every launch with `APPDATA` pointing at a temporary directory. The
//! installation preferences and the recovery journal live under `APPDATA`, so
//! no product change is needed and no real Parish data is read or written.
//!
//! Each case drives the real window (Present and Enable Output through the
//! accessibility tree, Space, F and B through synthetic keyboard input, see
//! `recovery_drill.ps1`), waits until the journal on disk shows the state,
//! and only then hard-kills the process. The relaunch must open a new session
//! whose first point carries the interrupted state, with no rejected journal,
//! and a Space must take the recovered Preview live.
//!
//! What it does not check: the congregation display staying black on
//! relaunch. `recovery_e2e.rs` proves that in-process and a projector cannot
//! be screenshotted from here.
//!
//! Enable Output is refused with no external display (T1.6), so Live, Frozen
//! and Blackout need two displays and this is a local or church-kit check,
//! not a CI job. With `--single-display` it runs the one case a single display
//! allows: a session with output off, whose Preview is moved by a key before
//! the kill.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{XtaskResult, measure_presenter};

/// Where the report is written, relative to the repository root.
pub const REPORT_RELATIVE_PATH: &str = "target/ordo-recovery-drill/report.json";

/// The PowerShell helper that drives the window.
const DRIVER_SCRIPT: &str = include_str!("recovery_drill.ps1");

/// Journal file name and the name a refused journal is moved to. They mirror
/// `crates/ordo/src/recovery.rs`; the drill reads files, not the crate.
const JOURNAL_FILE: &str = "session-recovery.json";
const REJECTED_FILE: &str = "session-recovery.rejected.json";

/// How long a state may take to reach the journal file.
const JOURNAL_TIMEOUT: Duration = Duration::from_secs(20);
/// How long a relaunch may take to journal its recovered session.
const RELAUNCH_TIMEOUT: Duration = Duration::from_secs(40);
const POLL: Duration = Duration::from_millis(100);

/// One journal point: the fields the drill judges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub session_id: String,
    pub seq: u64,
    pub state: String,
    pub preview: String,
    pub program: Option<String>,
    pub ended: bool,
}

/// Reads the fields the drill judges from a journal file's text. `None` for
/// anything that is not a complete record, so a poll that races a write just
/// tries again. The drill does not verify the record hash: the product's own
/// `decode` does that when it reopens, and a bad record shows up as a
/// rejected journal.
pub fn parse_point(text: &str) -> Option<Point> {
    let value: Value = serde_json::from_str(text).ok()?;
    Some(Point {
        session_id: value["session_id"].as_str()?.to_owned(),
        seq: value["seq"].as_u64()?,
        state: value["state"].as_str()?.to_owned(),
        preview: value["preview"]["cue_id"].as_str()?.to_owned(),
        program: match &value["program"] {
            Value::Null => None,
            program => Some(program["cue_id"].as_str()?.to_owned()),
        },
        ended: value["ended"].as_bool()?,
    })
}

/// What the display count allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Two or more displays and no `--single-display`: every case runs.
    Full,
    /// `--single-display`, whatever is attached: only the output-off case runs.
    SingleOnly,
}

/// Decides what to run from the number of attached displays. A single display
/// is refused unless the caller asked for the reduced run, because it cannot
/// exercise Enable Output and a pass would say less than it appears to.
pub fn display_gate(displays: usize, single_display: bool) -> Result<Gate, String> {
    match (displays, single_display) {
        (0, _) => Err("the drill found no attached display".to_owned()),
        (_, true) => Ok(Gate::SingleOnly),
        (2.., false) => Ok(Gate::Full),
        (1, false) => Err(
            "the drill needs two displays: Enable Output is refused with only \
             one. Attach a second display, or pass --single-display to run the output-off \
             case only"
                .to_owned(),
        ),
    }
}

/// The cases a gate runs.
fn cases(gate: Gate) -> &'static [Case] {
    match gate {
        Gate::Full => &[Case::Live, Case::Frozen, Case::Blackout],
        Gate::SingleOnly => &[Case::OutputOff],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Live,
    Frozen,
    Blackout,
    OutputOff,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Frozen => "frozen",
            Self::Blackout => "blackout",
            Self::OutputOff => "output-off",
        }
    }

    /// The journal state the kill must interrupt.
    pub fn interrupted_state(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Frozen => "frozen",
            Self::Blackout => "blackout",
            Self::OutputOff => "output-disabled",
        }
    }
}

/// Judges the first point the relaunch journaled: a new session, carrying the
/// interrupted state and the Preview the kill left, with no rejected journal.
pub fn check_reopened(
    before: &Point,
    after: &Point,
    rejected_exists: bool,
    expected_state: &str,
) -> Result<(), String> {
    if rejected_exists {
        return Err(format!(
            "the relaunch rejected the journal ({REJECTED_FILE} exists)"
        ));
    }
    if after.session_id == before.session_id {
        return Err("the relaunch did not start a new session".to_owned());
    }
    if after.ended {
        return Err("the relaunch's first point is an ended session".to_owned());
    }
    if after.state != expected_state {
        return Err(format!(
            "the recovered session's first point is '{}', expected the interrupted '{expected_state}'",
            after.state
        ));
    }
    if after.preview != before.preview {
        return Err("the recovered Preview is not the Cue the kill left".to_owned());
    }
    Ok(())
}

/// Judges the point after Space on a recovered output: Live on the recovered
/// Preview Cue.
pub fn check_resumed(recovered: &Point, resumed: &Point) -> Result<(), String> {
    if resumed.session_id != recovered.session_id {
        return Err("Space changed the session".to_owned());
    }
    if resumed.state != "live" {
        return Err(format!(
            "after Space the state is '{}', not live",
            resumed.state
        ));
    }
    if resumed.program.as_deref() != Some(recovered.preview.as_str()) {
        return Err("after Space Program is not the recovered Preview Cue".to_owned());
    }
    Ok(())
}

/// One case's result.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub case: &'static str,
    pub result: Result<String, String>,
}

/// The one line printed per case.
pub fn summary_line(outcome: &Outcome) -> String {
    match &outcome.result {
        Ok(detail) => format!("recovery-drill {}: {detail}", outcome.case),
        Err(reason) => format!("recovery-drill {}: FAILED, {reason}", outcome.case),
    }
}

/// The report document: one entry per case, and a verdict that is true only
/// when at least one case ran and all of them passed.
pub fn report(gate: Gate, displays: usize, outcomes: &[Outcome]) -> Value {
    let passed = !outcomes.is_empty() && outcomes.iter().all(|o| o.result.is_ok());
    json!({
        "mode": match gate { Gate::Full => "two-displays", Gate::SingleOnly => "single-display" },
        "displays": displays,
        "passed": passed,
        "cases": outcomes.iter().map(|o| match &o.result {
            Ok(detail) => json!({"case": o.case, "passed": true, "detail": detail}),
            Err(reason) => json!({"case": o.case, "passed": false, "detail": reason}),
        }).collect::<Vec<_>>(),
    })
}

/// Fails unless every case passed.
pub fn judge(outcomes: &[Outcome]) -> Result<(), String> {
    if outcomes.is_empty() {
        return Err("no case ran".to_owned());
    }
    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|o| o.result.is_err())
        .map(|o| o.case)
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("recovery drill failed: {}", failed.join(", ")))
    }
}

/// Runs the drill.
pub fn recovery_drill(root: &Path, args: &[String]) -> XtaskResult<()> {
    let mut single_display = false;
    for arg in args {
        match arg.as_str() {
            "--single-display" => single_display = true,
            other => {
                return Err(format!(
                    "unknown argument '{other}'; usage: cargo xtask recovery-drill [--single-display]"
                )
                .into());
            }
        }
    }
    if !cfg!(windows) {
        return Err("recovery-drill drives the Windows window and runs on Windows only".into());
    }

    let work = Work::new()?;
    let driver = Driver::new(&work)?;
    let displays = driver.display_count()?;
    let gate = display_gate(displays, single_display)?;
    println!("recovery-drill: {displays} display(s), running {gate:?}");

    let built = build_ordo(root)?;
    let app = work.dir.join("app");
    fs::create_dir_all(app.join("fixtures"))?;
    let executable = app.join("ordo.exe");
    fs::copy(&built, &executable)?;
    copy_dir(
        &root.join("tests/fixtures/presentations/normal-mass"),
        &app.join("fixtures/normal-mass"),
    )?;

    let mut outcomes = Vec::new();
    for &case in cases(gate) {
        let appdata = work.dir.join(format!("appdata-{}", case.name()));
        fs::create_dir_all(&appdata)?;
        let session = Session {
            executable: &executable,
            appdata: &appdata,
            driver: &driver,
        };
        let outcome = Outcome {
            case: case.name(),
            result: session.run(case),
        };
        println!("{}", summary_line(&outcome));
        outcomes.push(outcome);
    }

    let document = report(gate, displays, &outcomes);
    let report_path = root.join(REPORT_RELATIVE_PATH);
    if let Some(directory) = report_path.parent() {
        fs::create_dir_all(directory)?;
    }
    fs::write(&report_path, serde_json::to_string_pretty(&document)?)?;
    println!("recovery-drill: report at {}", report_path.display());
    judge(&outcomes)?;
    Ok(())
}

/// A temporary directory removed on drop.
struct Work {
    dir: PathBuf,
}

impl Work {
    fn new() -> XtaskResult<Self> {
        let dir = std::env::temp_dir().join(format!("ordo-recovery-drill-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Runs the PowerShell helper.
struct Driver {
    script: PathBuf,
}

impl Driver {
    fn new(work: &Work) -> XtaskResult<Self> {
        let script = work.dir.join("recovery_drill.ps1");
        fs::write(&script, DRIVER_SCRIPT)?;
        Ok(Self { script })
    }

    fn command(&self, action: &str) -> Command {
        let mut command = Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&self.script)
            .args(["-Action", action]);
        command
    }

    fn display_count(&self) -> XtaskResult<usize> {
        let output = self.command("displays").stdin(Stdio::null()).output()?;
        if !output.status.success() {
            return Err("the display count helper failed".into());
        }
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .map_err(|_| "the display count helper printed no number".into())
    }

    fn drive(&self, action: &str, pid: u32, argument: &str) -> Result<(), String> {
        let output = self
            .command(action)
            .args(["-ProcessId", &pid.to_string(), "-Argument", argument])
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("could not start PowerShell: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{action} '{argument}' failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn invoke(&self, pid: u32, button: &str) -> Result<(), String> {
        self.drive("invoke", pid, button)
    }

    fn keys(&self, pid: u32, keys: &str) -> Result<(), String> {
        self.drive("keys", pid, keys)
    }
}

/// A launched `ordo.exe`. Dropping it kills and reaps it, so no path leaves a
/// child running; it only ever touches the process this drill started.
struct Running {
    child: Child,
}

impl Running {
    fn pid(&self) -> u32 {
        self.child.id()
    }

    /// A hard kill (`TerminateProcess`), not a graceful close.
    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.kill();
    }
}

struct Session<'a> {
    executable: &'a Path,
    appdata: &'a Path,
    driver: &'a Driver,
}

impl Session<'_> {
    fn launch(&self) -> Result<Running, String> {
        let child = Command::new(self.executable)
            .env("APPDATA", self.appdata)
            .stdin(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not launch ordo.exe: {error}"))?;
        Ok(Running { child })
    }

    fn journal_path(&self) -> PathBuf {
        self.appdata.join("ordo").join(JOURNAL_FILE)
    }

    fn current(&self) -> Option<Point> {
        parse_point(&fs::read_to_string(self.journal_path()).ok()?)
    }

    /// Polls until a point satisfies `wanted`, or fails with `what`.
    fn wait_for(
        &self,
        timeout: Duration,
        what: &str,
        wanted: impl Fn(&Point) -> bool,
    ) -> Result<Point, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(point) = self.current()
                && wanted(&point)
            {
                return Ok(point);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "the journal did not show {what} within {} s (last: {:?})",
                    timeout.as_secs(),
                    self.current()
                ));
            }
            thread::sleep(POLL);
        }
    }

    fn run(&self, case: Case) -> Result<String, String> {
        // The state that must be on disk before the kill, and the session
        // that wrote it.
        let mut first = self.launch()?;
        let pid = first.pid();
        self.driver.invoke(pid, "Present")?;
        let opened = self.wait_for(JOURNAL_TIMEOUT, "the Presenter session", |p| {
            p.state == "output-disabled" && !p.ended
        })?;

        let before = match case {
            Case::OutputOff => {
                // A key moves Preview, so the kill leaves a Cue worth
                // recovering rather than the opening one.
                self.driver.keys(pid, "{RIGHT}")?;
                self.wait_for(JOURNAL_TIMEOUT, "Preview moved", |p| {
                    p.session_id == opened.session_id && p.preview != opened.preview
                })?
            }
            Case::Live | Case::Frozen | Case::Blackout => {
                self.driver.invoke(pid, "Enable Output")?;
                self.wait_for(JOURNAL_TIMEOUT, "output enabled", |p| {
                    p.session_id == opened.session_id && p.state == "enabled-black"
                })?;
                self.driver.keys(pid, "space")?;
                self.wait_for(JOURNAL_TIMEOUT, "live", |p| {
                    p.session_id == opened.session_id && p.state == "live"
                })?;
                match case {
                    Case::Frozen => {
                        self.driver.keys(pid, "f")?;
                        self.wait_for(JOURNAL_TIMEOUT, "frozen", |p| {
                            p.session_id == opened.session_id && p.state == "frozen"
                        })?
                    }
                    Case::Blackout => {
                        self.driver.keys(pid, "b")?;
                        self.wait_for(JOURNAL_TIMEOUT, "blackout", |p| {
                            p.session_id == opened.session_id && p.state == "blackout"
                        })?
                    }
                    _ => self.wait_for(JOURNAL_TIMEOUT, "live", |p| {
                        p.session_id == opened.session_id && p.state == "live"
                    })?,
                }
            }
        };
        // Let any in-flight write settle: the point on disk must be the one
        // the relaunch will read, so read it again after the wait.
        let before = self
            .current()
            .filter(|p| p.session_id == before.session_id)
            .unwrap_or(before);
        first.kill();

        let mut second = self.launch()?;
        let recovered = self.wait_for(RELAUNCH_TIMEOUT, "a recovered session", |p| {
            p.session_id != before.session_id
        })?;
        let rejected = self.appdata.join("ordo").join(REJECTED_FILE).exists();
        check_reopened(&before, &recovered, rejected, case.interrupted_state())?;

        let detail = match case {
            Case::OutputOff => {
                // Output is off, so Space is refused: a key proves the
                // recovered Presenter takes input instead.
                let pid = second.pid();
                self.driver.keys(pid, "{RIGHT}")?;
                self.wait_for(JOURNAL_TIMEOUT, "Preview moved after recovery", |p| {
                    p.session_id == recovered.session_id && p.preview != recovered.preview
                })?;
                "reopened recovered, Right moved Preview (output off, no Space)"
            }
            Case::Live | Case::Frozen | Case::Blackout => {
                self.driver.keys(second.pid(), "space")?;
                let resumed =
                    self.wait_for(JOURNAL_TIMEOUT, "live on the recovered Preview", |p| {
                        p.session_id == recovered.session_id
                            && p.state == "live"
                            && p.program.as_deref() == Some(recovered.preview.as_str())
                    })?;
                check_resumed(&recovered, &resumed)?;
                "reopened recovered, Space resumed"
            }
        };
        second.kill();
        Ok(detail.to_owned())
    }
}

/// Builds `ordo` in release with the workspace's `--locked` pin and returns
/// the executable cargo reports.
fn build_ordo(root: &Path) -> XtaskResult<PathBuf> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "ordo",
            "--bin",
            "ordo",
            "--locked",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err("command failed: cargo build --release -p ordo --bin ordo --locked".into());
    }
    measure_presenter::example_executable(&String::from_utf8_lossy(&output.stdout), "ordo")
        .ok_or_else(|| "cargo build did not report the ordo executable".into())
}

fn copy_dir(from: &Path, to: &Path) -> XtaskResult<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(session: &str, state: &str, preview: &str, program: Option<&str>) -> Point {
        Point {
            session_id: session.to_owned(),
            seq: 1,
            state: state.to_owned(),
            preview: preview.to_owned(),
            program: program.map(str::to_owned),
            ended: false,
        }
    }

    #[test]
    fn parses_a_journal_record_and_ignores_a_torn_one() {
        let text = r#"{"schema_version":2,"session_id":"s1","seq":3,"state":"live","preview":{"cue_id":"c2"},"program":{"cue_id":"c1"},"output":{"enabled":true,"display":null},"ended":false,"record_sha256":"x"}"#;
        let parsed = parse_point(text).unwrap();
        assert_eq!(
            parsed,
            Point {
                seq: 3,
                ..point("s1", "live", "c2", Some("c1"))
            }
        );
        assert!(parse_point(&text[..40]).is_none());
        assert!(parse_point("").is_none());
        let black = text.replace(r#"{"cue_id":"c1"}"#, "null");
        assert_eq!(parse_point(&black).unwrap().program, None);
    }

    #[test]
    fn one_display_is_refused_unless_asked_for() {
        assert_eq!(display_gate(2, false), Ok(Gate::Full));
        assert_eq!(display_gate(3, true), Ok(Gate::SingleOnly));
        assert_eq!(display_gate(1, true), Ok(Gate::SingleOnly));
        assert!(display_gate(1, false).unwrap_err().contains("two displays"));
        assert!(display_gate(0, true).is_err());
    }

    #[test]
    fn a_single_display_runs_only_the_output_off_case() {
        assert_eq!(cases(Gate::SingleOnly), &[Case::OutputOff]);
        assert_eq!(cases(Gate::Full).len(), 3);
    }

    #[test]
    fn a_reopen_needs_a_new_session_in_the_interrupted_state() {
        let before = point("s1", "frozen", "c2", Some("c1"));
        let good = point("s2", "frozen", "c2", None);
        assert!(check_reopened(&before, &good, false, "frozen").is_ok());
        assert!(check_reopened(&before, &good, true, "frozen").is_err());
        assert!(check_reopened(&before, &before, false, "frozen").is_err());
        assert!(
            check_reopened(
                &before,
                &point("s2", "enabled-black", "c2", None),
                false,
                "frozen"
            )
            .is_err()
        );
        assert!(
            check_reopened(&before, &point("s2", "frozen", "c3", None), false, "frozen").is_err()
        );
        let ended = Point {
            ended: true,
            ..good
        };
        assert!(check_reopened(&before, &ended, false, "frozen").is_err());
    }

    #[test]
    fn space_must_take_the_recovered_preview_live() {
        let recovered = point("s2", "blackout", "c2", None);
        assert!(check_resumed(&recovered, &point("s2", "live", "c3", Some("c2"))).is_ok());
        assert!(check_resumed(&recovered, &point("s2", "blackout", "c2", None)).is_err());
        assert!(check_resumed(&recovered, &point("s2", "live", "c3", Some("c1"))).is_err());
        assert!(check_resumed(&recovered, &point("s3", "live", "c3", Some("c2"))).is_err());
    }

    #[test]
    fn the_report_passes_only_when_every_case_passed() {
        let ok = Outcome {
            case: "live",
            result: Ok("reopened recovered, Space resumed".to_owned()),
        };
        let bad = Outcome {
            case: "frozen",
            result: Err("no".to_owned()),
        };
        assert!(judge(std::slice::from_ref(&ok)).is_ok());
        assert!(
            judge(&[ok.clone(), bad.clone()])
                .unwrap_err()
                .contains("frozen")
        );
        assert!(judge(&[]).is_err());
        assert_eq!(
            report(Gate::Full, 2, std::slice::from_ref(&ok))["passed"],
            true
        );
        assert_eq!(report(Gate::Full, 2, &[ok.clone(), bad])["passed"], false);
        assert_eq!(report(Gate::SingleOnly, 1, &[])["passed"], false);
        assert_eq!(
            summary_line(&ok),
            "recovery-drill live: reopened recovered, Space resumed"
        );
    }
}
