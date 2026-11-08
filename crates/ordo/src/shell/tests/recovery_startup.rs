//! Startup recovery through the real shell wiring (T1.7d): a journal written
//! by the encoder is read by the startup read, handed to the shell, and the
//! Presenter opens directly into the recovered state with Program black.
//!
//! Displays are the in-memory adapter installed through the shell's
//! display-source factory, so the Presenter is built on it before it opens
//! anything. Every journal lives in a temporary directory.

use super::*;
use crate::{
    output::DisplaySource,
    recovery::{
        JOURNAL_FILE, REJECTED_FILE, RecoveredSession, StartupRecovery, read_startup_recovery,
        spawn_journal_writer,
    },
};
use platform::display::{
    DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
    PlacementBounds,
};
use presentation::{
    journal::{
        BoundsRecord, DisplayRecord, LiveIntent, OutputEvidence, RecoveryPoint, SessionId,
        WrittenAt, decode, encode,
    },
    snapshot_reader::{DirectorySource, read_snapshot},
    state::{LiveState, ProgramContent},
};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

type Adapter = Rc<RefCell<InMemoryDisplayAdapter>>;

/// A unique empty directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ordo-shell-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn journal(&self) -> PathBuf {
        self.0.join(JOURNAL_FILE)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn display(id: u64, x: i32, width: u32, height: u32) -> DetectedDisplay {
    let bounds = DisplayBounds::new(x, 0, width, height);
    DetectedDisplay {
        id: DisplayId::new(id),
        evidence: DisplayEvidence {
            name: None,
            uuid: None,
            bounds,
        },
        placement: PlacementBounds::from(bounds),
    }
}

/// An operator display and a 1280x720 projector.
fn with_projector() -> Adapter {
    Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 1920, 1080), display(2, 1920, 1280, 720)],
        Some(DisplayId::new(1)),
    )))
}

/// The journaled evidence of the 1280x720 projector.
fn projector_record() -> DisplayRecord {
    DisplayRecord {
        name: None,
        uuid: None,
        bounds: BoundsRecord {
            x: 1920,
            y: 0,
            width: 1280,
            height: 720,
        },
    }
}

fn old_session() -> SessionId {
    SessionId::parse("00000000-0000-4000-8000-000000000901").unwrap()
}

/// What the interrupted session left: Preview on flat Cue 2, Program's last
/// valid Cue was flat Cue 0.
fn interrupted_point(
    output: OutputEvidence,
    state: LiveIntent,
    ended: bool,
    program: Option<usize>,
) -> RecoveryPoint {
    let directory = crate::presenter::fixture_snapshot_dir();
    let loaded = read_snapshot(&DirectorySource::new(&directory)).unwrap();
    let positions = loaded.snapshot().positions();
    assert!(positions.len() > 2, "the fixture has a few Cues");
    RecoveryPoint {
        session_id: old_session(),
        seq: 7,
        written_at: WrittenAt::parse("2026-11-08T09:31:00Z").unwrap(),
        snapshot_id: loaded.snapshot().id(),
        manifest_hash: loaded.manifest_hash(),
        source_dir: directory.to_str().unwrap().to_owned(),
        preview: positions[2].cue_id,
        program: program.map(|flat| positions[flat].cue_id),
        state,
        output,
        ended,
    }
}

fn live_on_projector() -> RecoveryPoint {
    interrupted_point(
        OutputEvidence {
            enabled: true,
            display: Some(projector_record()),
        },
        LiveIntent::Live,
        false,
        Some(0),
    )
}

fn write(dir: &TempDir, point: &RecoveryPoint) {
    std::fs::write(dir.journal(), encode(point)).unwrap();
}

fn found(dir: &TempDir) -> RecoveredSession {
    match read_startup_recovery(&dir.journal()) {
        StartupRecovery::Found(found) => *found,
        other => panic!("expected a recovered session, got {other:?}"),
    }
}

/// The shell as the runtime builds it: displays on the in-memory adapter.
fn shell_on<'a>(
    cx: &'a mut TestAppContext,
    adapter: &Adapter,
) -> (gpui::Entity<ShellView>, &'a mut VisualTestContext) {
    let (shell, cx) = open(cx);
    let source = adapter.clone();
    shell.update(cx, |shell, _| {
        shell.set_display_source(Rc::new(move || -> Box<dyn DisplaySource> {
            Box::new(source.clone())
        }))
    });
    (shell, cx)
}

fn recover(shell: &gpui::Entity<ShellView>, cx: &mut VisualTestContext, found: RecoveredSession) {
    shell.update_in(cx, |shell, window, cx| {
        shell.finish_startup_recovery(found, window, cx)
    });
    cx.run_until_parked();
}

fn presenter(
    shell: &gpui::Entity<ShellView>,
    cx: &VisualTestContext,
) -> gpui::Entity<crate::presenter::PresenterView> {
    shell.read_with(cx, |shell, _| {
        shell.presenter().cloned().expect("the Presenter is open")
    })
}

fn live(
    presenter: &gpui::Entity<crate::presenter::PresenterView>,
    cx: &VisualTestContext,
) -> LiveState {
    presenter.read_with(cx, |view, _| view.engine().state().live())
}

fn display_changed(
    presenter: &gpui::Entity<crate::presenter::PresenterView>,
    cx: &mut VisualTestContext,
) {
    presenter.update_in(cx, |view, window, cx| {
        let operator = crate::output::window_display(window, &cx.displays(), cx);
        view.display_changed(operator, cx)
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_live_interruption_reopens_black_on_the_recognized_projector(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    let preview = found.recovery.preview();
    assert!(!found.closed_at.is_empty());
    let (shell, cx) = shell_on(cx, &with_projector());

    recover(&shell, cx, found);

    assert!(shell.read_with(cx, |shell, _| shell.presenter_mode().is_presenter()));
    assert!(fullscreen(cx));
    assert_eq!(
        windows_open(cx),
        2,
        "operator window and one congregation window"
    );
    let presenter = presenter(&shell, cx);
    assert!(presenter.read_with(cx, |view, _| view.output_window().is_some()));
    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    presenter.read_with(cx, |view, _| {
        let state = view.engine().state();
        assert_eq!(state.preview(), preview);
        assert_eq!(state.program(), ProgramContent::Black);
        let context = state.recovered().expect("the recovery context is kept");
        assert_eq!(context.program(), Some(0));
        assert_eq!(context.interrupted(), LiveIntent::Live);
        assert!(view.program_drawn().frame.is_none(), "Program draws black");
        let notice = view.recovery_notice().expect("the notice is kept");
        assert!(!notice.closed_at.is_empty());
        assert_eq!(notice.display_name, None);
    });
    assert!(
        presenter.read_with(cx, |view, _| view.program_drawn().frame.is_none()),
        "nothing is on the congregation display"
    );

    keys(cx, "space");

    assert_eq!(live(&presenter, cx), LiveState::Live);
    presenter.read_with(cx, |view, _| {
        let state = view.engine().state();
        assert!(state.recovered().is_none(), "Space ends the context");
        assert!(
            matches!(state.program(), ProgramContent::Frame { cue, .. } if cue == state.preview()),
            "Program shows Preview's Cue"
        );
    });
}

#[gpui::test]
fn a_different_display_leaves_output_disconnected_until_the_projector_returns(
    cx: &mut TestAppContext,
) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    // The only other display has another resolution.
    let adapter: Adapter = Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 1920, 1080), display(2, 1920, 1024, 768)],
        Some(DisplayId::new(1)),
    )));
    let (shell, cx) = shell_on(cx, &adapter);

    recover(&shell, cx, found);

    let presenter = presenter(&shell, cx);
    assert_eq!(
        live(&presenter, cx),
        LiveState::Disconnected {
            display_back: false
        }
    );
    assert_eq!(windows_open(cx), 1, "no congregation window opened");
    assert!(presenter.read_with(cx, |view, _| view.engine().state().recovered().is_some()));
    assert!(presenter.read_with(cx, |view, _| {
        view.recovery_notice()
            .is_some_and(|notice| !notice.display_recognized)
    }));

    // The recognized display is connected and the displays are re-read.
    adapter.borrow_mut().connect(display(3, 3000, 1280, 720));
    display_changed(&presenter, cx);
    assert_eq!(
        live(&presenter, cx),
        LiveState::Disconnected { display_back: true }
    );
    assert_eq!(windows_open(cx), 1, "a return never opens output by itself");
    assert!(
        presenter.read_with(cx, |view, _| view.program_drawn().frame.is_none()),
        "Program is still black"
    );
    let (announcement, _) = texts(&presenter, cx).expect("the strip is still shown");
    assert!(
        announcement.ends_with(
            " The congregation display is back. Space resumes output and takes Preview live."
        ),
        "{announcement}"
    );

    keys(cx, "space");

    assert_eq!(live(&presenter, cx), LiveState::Live);
    assert_eq!(windows_open(cx), 2);
    assert!(presenter.read_with(cx, |view, _| view.output_window().is_some()));
    assert!(presenter.read_with(cx, |view, _| view.engine().state().recovered().is_none()));
    assert!(!strip_present(cx), "Space clears the strip");
}

#[gpui::test]
fn output_that_was_disabled_reopens_with_output_disabled(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(
        &dir,
        &interrupted_point(
            OutputEvidence {
                enabled: false,
                display: None,
            },
            LiveIntent::OutputDisabled,
            false,
            None,
        ),
    );
    let found = found(&dir);
    let preview = found.recovery.preview();
    let (shell, cx) = shell_on(cx, &with_projector());

    recover(&shell, cx, found);

    let presenter = presenter(&shell, cx);
    assert_eq!(live(&presenter, cx), LiveState::OutputDisabled);
    assert_eq!(windows_open(cx), 1);
    assert_eq!(
        presenter.read_with(cx, |view, _| view.engine().state().preview()),
        preview
    );
}

#[gpui::test]
fn a_rejected_journal_leaves_preparation_and_is_set_aside(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    std::fs::write(dir.journal(), b"{\"schema_version\": 2, \"se").unwrap();
    let (shell, cx) = shell_on(cx, &with_projector());

    match read_startup_recovery(&dir.journal()) {
        StartupRecovery::Rejected(_) => {}
        other => panic!("expected a rejection, got {other:?}"),
    }
    cx.run_until_parked();

    assert!(!shell.read_with(cx, |shell, _| shell.presenter_mode().is_presenter()));
    assert!(shell.read_with(cx, |shell, _| shell.presenter().is_none()));
    assert!(cx.debug_bounds("workspace-header").is_some());
    assert!(dir.0.join(REJECTED_FILE).exists());
    assert!(!dir.journal().exists());
}

#[gpui::test]
fn a_recovery_that_arrives_while_the_presenter_is_open_is_dropped(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    let (shell, cx) = shell_on(cx, &with_projector());
    click(cx, "button-Present");
    let open = presenter(&shell, cx);

    recover(&shell, cx, found);

    let after = presenter(&shell, cx);
    assert_eq!(after.entity_id(), open.entity_id(), "the Presenter is kept");
    assert!(after.read_with(cx, |view, _| view.recovery_notice().is_none()));
    assert!(after.read_with(cx, |view, _| view.engine().state().recovered().is_none()));
    assert_eq!(live(&after, cx), LiveState::OutputDisabled);
}

/// A recovered session with the journal writer wired as the runtime does, over
/// the same file the interrupted session left.
fn recovered_with_writer(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<ShellView>,
    &mut VisualTestContext,
    TempDir,
    crate::recovery::JournalShutdown,
    platform::tasks::OwnedWorker<()>,
) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    let (writer, worker, shutdown) =
        spawn_journal_writer(dir.journal(), async_channel::bounded(1).0, |failure| {
            panic!("journal failure: {failure:?}")
        })
        .unwrap();
    let (shell, cx) = shell_on(cx, &with_projector());
    shell.update(cx, |shell, _| shell.set_journal_writer(writer));
    recover(&shell, cx, found);
    (shell, cx, dir, shutdown, worker)
}

#[gpui::test]
fn the_first_journal_point_of_the_new_session_keeps_the_interrupted_intent(
    cx: &mut TestAppContext,
) {
    let (_shell, _cx, dir, shutdown, mut worker) = recovered_with_writer(cx);
    // The journal is replaced at once, without any operator action.
    let mut written = None;
    for _ in 0..200 {
        if let Some(point) = std::fs::read(dir.journal())
            .ok()
            .and_then(|bytes| decode(&bytes).ok())
            && point.session_id != old_session()
        {
            written = Some(point);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    shutdown.send();
    worker.cancel_and_join().expect("the journal worker joins");
    let point = written.expect("the recovered session journals its first point");
    assert_ne!(point.session_id, old_session(), "a new session id");
    assert_eq!(point.program, interrupted_point_program());
    assert_eq!(point.state, LiveIntent::Live);
    assert!(point.output.enabled);
    assert!(!point.ended);
}

fn interrupted_point_program() -> Option<domain::presentation::CueId> {
    live_on_projector().program
}

#[gpui::test]
fn exiting_from_the_recovered_state_takes_the_normal_path_and_leaves_no_journal(
    cx: &mut TestAppContext,
) {
    let (shell, cx, dir, shutdown, mut worker) = recovered_with_writer(cx);

    click(cx, "header-exit-presenter");
    if presenter(&shell, cx).read_with(cx, |view, cx| view.exit_confirmation_open(cx)) {
        click(cx, "button-End presentation");
    }

    assert!(shell.read_with(cx, |shell, _| shell.presenter().is_none()));
    assert!(!fullscreen(cx));
    assert_eq!(windows_open(cx), 1, "the congregation window is gone");
    shutdown.send();
    worker.cancel_and_join().expect("the journal worker joins");
    assert!(!dir.journal().exists(), "a clean exit leaves no journal");
    assert!(!dir.0.join(format!("{JOURNAL_FILE}.tmp")).exists());
}

fn strip_present(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("recovery-strip").is_some()
}

fn texts(
    presenter: &gpui::Entity<crate::presenter::PresenterView>,
    cx: &VisualTestContext,
) -> Option<(String, Option<String>)> {
    presenter.read_with(cx, |view, _| view.recovery_strip_texts())
}

#[gpui::test]
fn the_strip_shows_prior_program_privately_and_space_clears_it(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    let (shell, cx) = shell_on(cx, &with_projector());

    recover(&shell, cx, found);

    let presenter = presenter(&shell, cx);
    assert!(strip_present(cx), "the strip is on screen");
    assert!(cx.debug_bounds("recovery-prior").is_some());
    let title = presenter.read_with(cx, |view, _| {
        view.engine()
            .snapshot()
            .item_of(0)
            .unwrap()
            .title()
            .to_owned()
    });
    let (announcement, meta) = texts(&presenter, cx).expect("the strip has text");
    assert!(announcement.starts_with("Presentation recovered · Congregation output is black. "));
    assert!(announcement.ends_with(" Space takes Preview live."));
    assert_eq!(
        meta.as_deref(),
        Some(format!("LIVE on {title} · 1.1 · not on screen now").as_str())
    );
    assert!(
        presenter.read_with(cx, |view, _| view.prior_frame_ready()),
        "the prior frame is requested and composed"
    );

    // Private: Program draws black and the congregation window has no strip.
    assert!(presenter.read_with(cx, |view, _| view.program_drawn().frame.is_none()));
    let handle = presenter
        .read_with(cx, |view, _| view.output_window())
        .expect("the congregation window is open");
    let mut on_output = VisualTestContext::from_window(handle.into(), cx);
    on_output.run_until_parked();
    assert!(on_output.debug_bounds("recovery-strip").is_none());
    assert!(on_output.debug_bounds("recovery-prior").is_none());
    let drawn = handle
        .update(cx, |view, _, _| view.drawn_frame().cloned())
        .unwrap();
    assert!(drawn.is_none(), "the output draws no frame");

    keys(cx, "space");

    assert!(!strip_present(cx), "Space clears the strip");
    assert!(cx.debug_bounds("recovery-prior").is_none());
    assert!(texts(&presenter, cx).is_none());
    assert!(presenter.read_with(cx, |view, _| view.program_drawn().frame.is_some()));
}

#[gpui::test]
fn a_refused_space_keeps_the_strip_and_names_the_display(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    let mut point = live_on_projector();
    point.output.display.as_mut().unwrap().name = Some("Hall projector".to_owned());
    write(&dir, &point);
    let found = found(&dir);
    let adapter: Adapter = Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 1920, 1080), display(2, 1920, 1024, 768)],
        Some(DisplayId::new(1)),
    )));
    let (shell, cx) = shell_on(cx, &adapter);

    recover(&shell, cx, found);

    let presenter = presenter(&shell, cx);
    let (announcement, meta) = texts(&presenter, cx).expect("the strip has text");
    assert!(announcement.ends_with("Hall projector wasn’t recognized, so output is disconnected."));
    assert!(meta.is_some_and(|meta| meta.starts_with("LIVE on ")));
    keys(cx, "space");
    assert!(
        presenter.read_with(cx, |view, _| view.engine().state().recovered().is_some()),
        "the refused Space changed nothing"
    );
    assert!(strip_present(cx));
}

#[gpui::test]
fn without_a_prior_program_or_output_the_strip_has_no_prior_block(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(
        &dir,
        &interrupted_point(
            OutputEvidence {
                enabled: false,
                display: None,
            },
            LiveIntent::OutputDisabled,
            false,
            None,
        ),
    );
    let found = found(&dir);
    let (shell, cx) = shell_on(cx, &with_projector());

    recover(&shell, cx, found);

    let presenter = presenter(&shell, cx);
    assert!(strip_present(cx));
    assert!(cx.debug_bounds("recovery-prior").is_none());
    let (announcement, meta) = texts(&presenter, cx).unwrap();
    assert!(announcement.ends_with(" Enable Output, then Space takes Preview live."));
    assert_eq!(meta, None);
}

#[gpui::test]
fn exiting_from_the_recovered_state_never_brings_the_strip_back(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    let (shell, cx) = shell_on(cx, &with_projector());
    recover(&shell, cx, found);
    assert!(strip_present(cx));

    click(cx, "header-exit-presenter");
    if presenter(&shell, cx).read_with(cx, |view, cx| view.exit_confirmation_open(cx)) {
        assert!(strip_present(cx), "the confirmation does not clear it");
        click(cx, "button-End presentation");
    }

    assert!(shell.read_with(cx, |shell, _| shell.presenter().is_none()));
    assert!(!strip_present(cx));
    cx.run_until_parked();
    assert!(!strip_present(cx), "Preparation never shows the strip");
}

#[gpui::test]
fn a_refused_space_on_an_unavailable_preview_keeps_the_strip_and_the_black(
    cx: &mut TestAppContext,
) {
    let dir = TempDir::new();
    write(&dir, &live_on_projector());
    let found = found(&dir);
    let (shell, cx) = shell_on(cx, &with_projector());
    recover(&shell, cx, found);
    let presenter = presenter(&shell, cx);
    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    let windows = windows_open(cx);

    // Preview's own frame fails to compose.
    let frame = presenter.read_with(cx, |view, _| {
        let snapshot = view.engine().snapshot();
        match snapshot.cue(2).unwrap().behavior() {
            domain::presentation::CueBehavior::Project { frame, .. } => *frame,
            other => panic!("the fixture's third Cue projects, got {other:?}"),
        }
    });
    presenter.update(cx, |view, cx| {
        view.command(
            presentation::commands::Command::FrameUnavailable {
                frame,
                reason: rendering::UnavailableReason::DecodeFailed,
            },
            cx,
        )
        .unwrap();
    });
    cx.run_until_parked();

    keys(cx, "space");

    assert!(strip_present(cx), "the refused Space leaves the strip");
    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    assert!(presenter.read_with(cx, |view, _| {
        view.engine().state().recovered().is_some() && view.program_drawn().frame.is_none()
    }));
    assert_eq!(windows_open(cx), windows, "nothing opened or closed");
}

#[gpui::test]
fn enabling_output_after_a_disabled_recovery_switches_the_strip_to_space(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    write(
        &dir,
        &interrupted_point(
            OutputEvidence {
                enabled: false,
                display: None,
            },
            LiveIntent::OutputDisabled,
            false,
            None,
        ),
    );
    let found = found(&dir);
    let (shell, cx) = shell_on(cx, &with_projector());
    recover(&shell, cx, found);
    let presenter = presenter(&shell, cx);
    assert!(
        texts(&presenter, cx)
            .unwrap()
            .0
            .ends_with(" Enable Output, then Space takes Preview live.")
    );

    presenter.update_in(cx, |view, window, cx| view.enable_output(window, cx));
    cx.run_until_parked();

    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    assert_eq!(windows_open(cx), 2, "the congregation window opened");
    assert!(presenter.read_with(cx, |view, _| view.program_drawn().frame.is_none()));
    assert!(strip_present(cx), "Enable Output does not clear the strip");
    let (announcement, _) = texts(&presenter, cx).unwrap();
    assert!(announcement.ends_with(" Space takes Preview live."));
    assert!(!announcement.contains("Enable Output"), "{announcement}");

    keys(cx, "space");

    assert_eq!(live(&presenter, cx), LiveState::Live);
    assert!(!strip_present(cx), "Space clears the strip");
}
