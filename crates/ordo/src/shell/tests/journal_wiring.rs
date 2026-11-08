//! The recovery journal through the real shell wiring: the shell hands a
//! writer to each Presenter session, points reach a temp-dir file, and every
//! clean end (confirmed quit, Exit with output disabled, End presentation)
//! leaves no journal behind.

use super::*;
use crate::recovery::{JOURNAL_FILE, JournalShutdown, JournalWriter, spawn_journal_writer};
use platform::{
    display::{
        DetectedDisplay, DisplayBounds, DisplayEvidence, DisplayId, InMemoryDisplayAdapter,
        PlacementBounds,
    },
    tasks::OwnedWorker,
};
use presentation::journal::{LiveIntent, RecoveryPoint, decode};
use presentation::state::LiveState;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

/// A unique empty directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ordo-shell-journal-{}-{}",
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

struct Journal {
    dir: TempDir,
    worker: OwnedWorker<()>,
    shutdown: JournalShutdown,
}

impl Journal {
    /// Stop the writer after it has handled everything queued, and join it.
    fn drain(mut self) -> TempDir {
        self.shutdown.send();
        self.worker
            .cancel_and_join()
            .expect("the journal worker joins");
        self.dir
    }
}

fn start_journal() -> (Journal, JournalWriter) {
    let dir = TempDir::new();
    let (writer, worker, shutdown) = spawn_journal_writer(dir.journal(), |failure| {
        panic!("journal failure: {failure:?}")
    })
    .unwrap();
    (
        Journal {
            dir,
            worker,
            shutdown,
        },
        writer,
    )
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

fn assert_nothing_left(dir: &TempDir) {
    assert!(!dir.journal().exists(), "the journal is deleted");
    assert!(
        !dir.0.join(format!("{JOURNAL_FILE}.tmp")).exists(),
        "no staging file is left"
    );
}

/// Enter the Presenter with a journal writer set; optionally with a projector.
fn enter(
    cx: &mut TestAppContext,
    with_output: bool,
) -> (gpui::Entity<ShellView>, &mut VisualTestContext, Journal) {
    let (shell, cx) = open(cx);
    let (journal, writer) = start_journal();
    shell.update(cx, |shell, _| shell.set_journal_writer(writer));
    click(cx, "button-Present");
    if with_output {
        let presenter = shell.read_with(cx, |shell, _| shell.presenter().cloned().unwrap());
        let adapter = Rc::new(RefCell::new(InMemoryDisplayAdapter::new(
            vec![display(1, 0, 1920, 1080), display(2, 1920, 1280, 720)],
            Some(DisplayId::new(1)),
        )));
        presenter.update_in(cx, |view, window, cx| {
            view.install_output(Box::new(adapter), window, cx)
        });
        cx.run_until_parked();
        click(cx, "primary-action");
        keys(cx, "space");
        let live = presenter.read_with(cx, |view, _| view.engine().state().live());
        assert_eq!(live, LiveState::Live);
    }
    (shell, cx, journal)
}

#[gpui::test]
fn a_live_session_is_journaled_and_a_confirmed_quit_deletes_the_journal(cx: &mut TestAppContext) {
    let (shell, cx, journal) = enter(cx, true);
    let point = wait_for_point(&journal.dir.journal(), |point| {
        point.state == LiveIntent::Live && !point.ended
    });
    assert!(point.output.enabled);

    let allowed = shell.update_in(cx, |shell, window, cx| shell.allow_close(window, cx));
    assert!(!allowed);
    cx.run_until_parked();
    keys(cx, "enter");
    cx.executor().advance_clock(Duration::from_millis(
        ui::tokens::motion::OVERLAY_CLOSE_DURATION_MS.into(),
    ));
    cx.run_until_parked();

    assert_nothing_left(&journal.drain());
}

#[gpui::test]
fn exit_with_output_disabled_deletes_the_journal(cx: &mut TestAppContext) {
    let (_shell, cx, journal) = enter(cx, false);
    keys(cx, "right");
    let point = wait_for_point(&journal.dir.journal(), |point| !point.ended);
    assert_eq!(point.state, LiveIntent::OutputDisabled);

    click(cx, "header-exit-presenter");

    assert_nothing_left(&journal.drain());
}

#[gpui::test]
fn a_confirmed_end_presentation_deletes_the_journal(cx: &mut TestAppContext) {
    let (_shell, cx, journal) = enter(cx, true);
    wait_for_point(&journal.dir.journal(), |point| {
        point.state == LiveIntent::Live && !point.ended
    });

    click(cx, "header-exit-presenter");
    click(cx, "button-End presentation");

    assert_nothing_left(&journal.drain());
}
