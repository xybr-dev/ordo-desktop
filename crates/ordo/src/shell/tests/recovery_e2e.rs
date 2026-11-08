//! Recovery E2E (T1.7e WP8): an interrupted Presenter session is reopened by
//! a fresh start with the same operator context and a black congregation
//! display, and nothing goes live until one explicit valid Space.
//!
//! One test each for the Live, Frozen and Blackout interruptions, all through
//! `drive`:
//!
//! 1. Session one runs on the real wiring: a journal writer on a temporary
//!    file (as the runtime spawns it, with its startup channel, which finds
//!    no journal), an in-memory display adapter with a 1280x720 projector,
//!    Present, Enable Output, Space, and the arrow keys. The interrupted
//!    state is reached (Live as is, `f` for Frozen, `b` for Blackout) and
//!    Preview is moved on. What the live Engine holds is recorded.
//! 2. The session is cut off without any exit path: the writer is stopped by
//!    its shutdown message and joined, and the windows are dropped. No
//!    process has to be killed: the journal file is the only state that
//!    survives a termination, so dropping the session without `end_session`
//!    hands the next start exactly the input a killed process would
//!    (plan WP8). The journal must still exist and not be marked ended.
//! 3. A fresh writer on the same file delivers `StartupRecovery::Found`, and
//!    a new shell on a new adapter with the same projector is given it
//!    through `finish_startup_recovery`, the call the runtime makes.
//! 4. The operator context equals what was recorded (Presenter, full-screen,
//!    Preview, prior Program Cue, interrupted state, output display) with
//!    Engine `EnabledBlack` and the recovery strip showing.
//! 5. The congregation window exists but draws no frame and carries no strip.
//! 6. One navigation key changes nothing live. Then one Space takes Preview
//!    live (`Live`, never `Frozen` or `Blackout`), the output draws that
//!    frame, and the recovery context and strip are gone.
//! 7. Exiting through the normal path leaves no journal.

use super::*;
use crate::{
    output::DisplaySource,
    recovery::{JOURNAL_FILE, JournalShutdown, StartupRecovery, spawn_journal_writer},
};
use platform::{
    display::{
        DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
        PlacementBounds,
    },
    tasks::OwnedWorker,
};
use presentation::{
    journal::{LiveIntent, RecoveryPoint, decode},
    state::{LiveState, ProgramContent},
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

type Adapter = Rc<RefCell<InMemoryDisplayAdapter>>;
type Presenter = gpui::Entity<crate::presenter::PresenterView>;

/// A unique empty directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ordo-shell-recovery-e2e-{}-{}",
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

/// An operator display and a 1280x720 projector. A new adapter per session.
fn with_projector() -> Adapter {
    Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
        vec![display(1, 0, 1920, 1080), display(2, 1920, 1280, 720)],
        Some(DisplayId::new(1)),
    )))
}

/// The shell as the runtime builds it: displays on the adapter.
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

fn presenter_of(shell: &gpui::Entity<ShellView>, cx: &VisualTestContext) -> Presenter {
    shell.read_with(cx, |shell, _| {
        shell.presenter().cloned().expect("the Presenter is open")
    })
}

fn live(presenter: &Presenter, cx: &VisualTestContext) -> LiveState {
    presenter.read_with(cx, |view, _| view.engine().state().live())
}

fn preview(presenter: &Presenter, cx: &VisualTestContext) -> usize {
    presenter.read_with(cx, |view, _| view.engine().state().preview())
}

/// The Cue on Program, `None` while it is black.
fn program_cue(presenter: &Presenter, cx: &VisualTestContext) -> Option<usize> {
    presenter.read_with(cx, |view, _| match view.engine().state().program() {
        ProgramContent::Frame { cue, .. } => Some(cue),
        ProgramContent::Black => None,
    })
}

/// What session one held at the moment it was cut off.
struct Recorded {
    preview: usize,
    prior_program: usize,
    interrupted: LiveIntent,
    evidence: DisplayEvidence,
}

/// Wait (bounded) until the journal decodes to a point satisfying `wanted`.
fn wait_for_point(path: &Path, wanted: impl Fn(&RecoveryPoint) -> bool) -> RecoveryPoint {
    for _ in 0..200 {
        if let Some(point) = std::fs::read(path)
            .ok()
            .and_then(|bytes| decode(&bytes).ok())
            && wanted(&point)
        {
            return point;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("the journal never reached the wanted point");
}

/// Waits (bounded) for what the writer sends on its startup channel.
fn startup_result(received: &async_channel::Receiver<StartupRecovery>) -> StartupRecovery {
    for _ in 0..200 {
        if let Ok(result) = received.try_recv() {
            return result;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("the journal worker never reported its startup read");
}

/// Stops a writer after it has handled everything queued, and joins it.
fn stop(shutdown: JournalShutdown, mut worker: OwnedWorker<()>) {
    shutdown.send();
    worker.cancel_and_join().expect("the journal worker joins");
}

/// Session one, up to and including the cut-off. Nothing here borrows the
/// test context after it returns, so session two can open its own windows.
fn interrupted_session(cx: &mut TestAppContext, dir: &TempDir, how: LiveIntent) -> Recorded {
    let (startup, received) = async_channel::bounded(1);
    let (writer, worker, shutdown) = spawn_journal_writer(dir.journal(), startup, |failure| {
        panic!("journal failure: {failure:?}")
    })
    .unwrap();
    assert!(
        matches!(startup_result(&received), StartupRecovery::Absent),
        "a first run finds no journal"
    );

    let (shell, cx) = shell_on(cx, &with_projector());
    shell.update(cx, |shell, _| shell.set_journal_writer(writer));
    click(cx, "button-Present");
    click(cx, "primary-action");
    let presenter = presenter_of(&shell, cx);
    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    keys(cx, "space");
    assert_eq!(live(&presenter, cx), LiveState::Live);
    keys(cx, "right right right");
    assert_eq!(live(&presenter, cx), LiveState::Live);
    assert_eq!(
        program_cue(&presenter, cx),
        Some(preview(&presenter, cx)),
        "linked Live: Program moved with Preview"
    );

    // The last valid Program Cue is the one on Program before it is left.
    let prior_program = program_cue(&presenter, cx).expect("a Cue is live");
    match how {
        LiveIntent::Live => {}
        LiveIntent::Frozen => {
            keys(cx, "f");
            assert_eq!(live(&presenter, cx), LiveState::Frozen);
            keys(cx, "right right");
        }
        LiveIntent::Blackout => {
            keys(cx, "b");
            assert_eq!(live(&presenter, cx), LiveState::Blackout);
            keys(cx, "right right");
        }
        other => panic!("not an interruption this test drives: {other:?}"),
    }
    let recorded = Recorded {
        preview: preview(&presenter, cx),
        prior_program,
        interrupted: how,
        evidence: presenter
            .read_with(cx, |view, _| view.output_evidence())
            .expect("output is placed on the projector"),
    };
    if how != LiveIntent::Live {
        assert_ne!(
            recorded.preview, prior_program,
            "Preview moved away from the held Cue"
        );
    }

    // Wait until the journal holds this very point.
    let wanted_cue = presenter.read_with(cx, |view, _| {
        view.engine().snapshot().positions()[recorded.preview].cue_id
    });
    let before = wait_for_point(&dir.journal(), |point| {
        point.preview == wanted_cue && point.state == how && !point.ended
    });
    assert!(before.output.enabled);

    // Cut off: no exit path, no end_session, nothing deleted.
    stop(shutdown, worker);
    // The congregation window first, then the operator window itself.
    let operator = cx.update(|window, _| window.window_handle());
    let windows = cx.update(|_, cx| cx.windows());
    for window in windows {
        if window != operator {
            cx.update(|_, cx| {
                window
                    .update(cx, |_, window, _| window.remove_window())
                    .ok();
            });
        }
    }
    cx.run_until_parked();
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 0, "session one's windows are gone");
    let left = std::fs::read(dir.journal()).expect("the journal survives the cut-off");
    assert!(!decode(&left).expect("it still decodes").ended);
    recorded
}

fn drive(cx: &mut TestAppContext, how: LiveIntent) {
    let dir = TempDir::new();
    let recorded = interrupted_session(cx, &dir, how);

    // Fresh start on the same directory.
    let (startup, received) = async_channel::bounded(1);
    let (writer, worker, shutdown) = spawn_journal_writer(dir.journal(), startup, |failure| {
        panic!("journal failure: {failure:?}")
    })
    .unwrap();
    let found = match startup_result(&received) {
        StartupRecovery::Found(found) => *found,
        other => panic!("expected a recovered session, got {other:?}"),
    };
    let (shell, cx) = shell_on(cx, &with_projector());
    shell.update(cx, |shell, _| shell.set_journal_writer(writer));
    shell.update_in(cx, |shell, window, cx| {
        shell.finish_startup_recovery(found, window, cx)
    });
    cx.run_until_parked();

    // Identical operator context.
    assert!(shell.read_with(cx, |shell, _| shell.presenter_mode().is_presenter()));
    assert!(fullscreen(cx));
    let presenter = presenter_of(&shell, cx);
    assert_eq!(preview(&presenter, cx), recorded.preview);
    presenter.read_with(cx, |view, _| {
        let context = view.engine().state().recovered().expect("context is kept");
        assert_eq!(context.program(), Some(recorded.prior_program));
        assert_eq!(context.interrupted(), recorded.interrupted);
        assert_eq!(view.output_evidence(), Some(recorded.evidence.clone()));
    });
    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    let (announcement, meta) = presenter
        .read_with(cx, |view, _| view.recovery_strip_texts())
        .expect("the recovery strip shows");
    assert!(
        announcement.starts_with("Presentation recovered"),
        "{announcement}"
    );
    let word = match how {
        LiveIntent::Live => "LIVE on ",
        LiveIntent::Frozen => "FROZEN on ",
        _ => "BLACKOUT on ",
    };
    assert!(
        meta.as_deref().is_some_and(|meta| meta.starts_with(word)),
        "{meta:?}"
    );
    assert!(cx.debug_bounds("recovery-strip").is_some());

    // External Program is black.
    assert!(presenter.read_with(cx, |view, _| view.program_drawn().frame.is_none()));
    assert_eq!(windows_open(cx), 2, "operator and congregation windows");
    let handle = presenter
        .read_with(cx, |view, _| view.output_window())
        .expect("the congregation window is open");
    let mut on_output = VisualTestContext::from_window(handle.into(), cx);
    on_output.run_until_parked();
    assert!(on_output.debug_bounds("recovery-strip").is_none());
    assert!(
        handle
            .update(cx, |view, _, _| view.drawn_frame().is_none())
            .unwrap(),
        "the congregation window draws no frame"
    );

    // Navigation alone puts nothing live.
    keys(cx, "right");
    assert_eq!(live(&presenter, cx), LiveState::EnabledBlack);
    assert_eq!(program_cue(&presenter, cx), None);
    assert!(presenter.read_with(cx, |view, _| view.program_drawn().frame.is_none()));
    assert!(cx.debug_bounds("recovery-strip").is_some());

    // One explicit Space takes Preview live, whatever was interrupted.
    keys(cx, "space");
    assert_eq!(live(&presenter, cx), LiveState::Live);
    assert_eq!(
        program_cue(&presenter, cx),
        Some(preview(&presenter, cx)),
        "Program shows the new Preview"
    );
    presenter.read_with(cx, |view, _| {
        assert!(view.engine().state().recovered().is_none());
    });
    assert!(cx.debug_bounds("recovery-strip").is_none());
    on_output.run_until_parked();
    assert!(
        handle
            .update(cx, |view, _, _| view.drawn_frame().is_some())
            .unwrap(),
        "the output draws the live frame"
    );

    // A normal exit leaves no journal.
    click(cx, "header-exit-presenter");
    if presenter.read_with(cx, |view, cx| view.exit_confirmation_open(cx)) {
        click(cx, "button-End presentation");
    }
    assert!(shell.read_with(cx, |shell, _| shell.presenter().is_none()));
    stop(shutdown, worker);
    assert!(!dir.journal().exists(), "a clean exit leaves no journal");
}

#[gpui::test]
fn a_live_session_is_restored_black_and_resumed_by_one_space(cx: &mut TestAppContext) {
    drive(cx, LiveIntent::Live);
}

#[gpui::test]
fn a_frozen_session_is_restored_black_and_resumed_by_one_space(cx: &mut TestAppContext) {
    drive(cx, LiveIntent::Frozen);
}

#[gpui::test]
fn a_blackout_session_is_restored_black_and_resumed_by_one_space(cx: &mut TestAppContext) {
    drive(cx, LiveIntent::Blackout);
}
