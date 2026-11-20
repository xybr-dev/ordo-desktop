//! `[E2E]` T2.2: the edit session over a real `storage::Storage`.
//!
//! Type several fields, reorder twice, wait for autosave, Undo and Redo
//! through the expected points, restart, and restore a prior durable
//! revision, asserting current content and immutable history at each step.
//!
//! The storage worker is a real OS thread, so its replies wake GPUI tasks from
//! a foreign thread: every storage future is awaited inside a GPUI foreground
//! task, and the loops re-run `run_until_parked` after short real sleeps. The
//! GPUI clock stays manual, so the 1 s idle window is crossed with
//! `advance_clock`.

#[path = "support/storage_port.rs"]
mod storage_port;

use std::{
    future::Future,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use domain::{
    identity::{ArrangementId, RevisionId},
    revisions::{IDLE_MS, RevisionEntity, RevisionKind},
};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use ordo::edit_session::{
    EditSession, NavigationGuard, RevisionPort, SaveState,
    harness::{EditHarness, HarnessContent},
};
use storage::{
    NewArrangement, NewSection, NewSong, RevisionContent, RevisionRecord, ShutdownOutcome, Storage,
    StorageConfig,
};
use storage_port::{SharedStorage, StoragePort};
use ui::environment::{ResolvedAppearance, UiEnvironment, UiPreferences};

const DEADLINE: Duration = Duration::from_secs(10);

/// A unique temporary directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "ordo-edit-session-e2e-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn db(&self) -> PathBuf {
        self.0.join("parish.sqlite3")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        for _ in 0..5 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

trait Parked {
    fn park(&mut self);
}

impl Parked for TestAppContext {
    fn park(&mut self) {
        self.run_until_parked();
    }
}

impl Parked for VisualTestContext {
    fn park(&mut self) {
        self.run_until_parked();
    }
}

fn wait<T>(cx: &mut impl Parked, mut poll: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + DEADLINE;
    loop {
        cx.park();
        if let Some(value) = poll() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "did not settle within {DEADLINE:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Awaits `future` in a foreground task of the app and returns its output.
fn drive<T: 'static>(cx: &mut TestAppContext, future: impl Future<Output = T> + 'static) -> T {
    cx.executor().allow_parking();
    let (tx, rx) = async_channel::bounded(1);
    cx.spawn(async move |_| {
        tx.send(future.await).await.ok();
    })
    .detach();
    wait(cx, || rx.try_recv().ok())
}

/// [`drive`] from inside a window.
fn drive_in<T: 'static>(
    cx: &mut VisualTestContext,
    future: impl Future<Output = T> + 'static,
) -> T {
    cx.executor().allow_parking();
    let (tx, rx) = async_channel::bounded(1);
    cx.update(|_, app| {
        app.spawn(async move |_| {
            tx.send(future.await).await.ok();
        })
        .detach();
    });
    wait(cx, || rx.try_recv().ok())
}

fn open_storage(cx: &mut TestAppContext, dir: &TempDir) -> SharedStorage {
    let storage = Storage::spawn(StorageConfig::new(dir.db()));
    let ready = storage.ready();
    drive(cx, ready).expect("storage ready");
    SharedStorage::new(storage)
}

fn shut_down(cx: &mut TestAppContext, storage: &SharedStorage) {
    let storage = storage.take().expect("storage is still running");
    assert_eq!(drive(cx, storage.shutdown()), ShutdownOutcome::Clean);
}

type History = Vec<(RevisionRecord, RevisionContent)>;

/// Every revision of the Arrangement, oldest first, with its content.
fn history(
    cx: &mut VisualTestContext,
    storage: &SharedStorage,
    arrangement: ArrangementId,
) -> History {
    let list = storage.with(|s| s.revisions(RevisionEntity::Arrangement(arrangement)));
    let records = drive_in(cx, list).expect("revisions");
    records
        .into_iter()
        .map(|record| {
            let content =
                drive_in(cx, storage.with(|s| s.revision_content(record.id))).expect("content");
            (record, content)
        })
        .collect()
}

fn arrangement_content(content: &RevisionContent) -> &storage::ArrangementContent {
    let RevisionContent::Arrangement(content) = content else {
        panic!("an Arrangement revision holds Arrangement content");
    };
    content
}

/// The harness view of a stored revision (sections in Sequence order).
fn harness_of(content: &RevisionContent) -> HarnessContent {
    let content = arrangement_content(content);
    HarnessContent {
        title: content.title.clone().unwrap_or_default(),
        notes: content.rendition_attribution.clone().unwrap_or_default(),
        items: content
            .sequence
            .iter()
            .map(|occurrence| {
                let section = content
                    .sections
                    .iter()
                    .find(|section| section.id == occurrence.section)
                    .expect("every occurrence places a stored section");
                section.label.clone().expect("the adapter labels sections")
            })
            .collect(),
    }
}

/// Section and occurrence ids by label, to prove identities survive reorders.
fn identities(content: &RevisionContent) -> Vec<(String, String, String)> {
    let content = arrangement_content(content);
    let mut ids: Vec<_> = content
        .sequence
        .iter()
        .map(|occurrence| {
            let section = content
                .sections
                .iter()
                .find(|section| section.id == occurrence.section)
                .expect("a stored section");
            (
                section.label.clone().unwrap_or_default(),
                section.id.to_string(),
                occurrence.id.to_string(),
            )
        })
        .collect();
    ids.sort();
    ids
}

struct Rig {
    harness: Entity<EditHarness>,
    session: Entity<EditSession<HarnessContent>>,
}

fn open_window<'a>(
    cx: &'a mut TestAppContext,
    storage: &SharedStorage,
    arrangement: ArrangementId,
) -> (Rig, &'a mut VisualTestContext) {
    cx.update(|cx| {
        // `ui::init` may already have run for an earlier window of this app.
        ui::init(cx).ok();
        ordo::edit_session::init(cx);
    });
    let opened = StoragePort::open(storage.clone(), arrangement);
    let (port, content) = drive(cx, opened).expect("port opens");
    let port: Rc<dyn RevisionPort<HarnessContent>> = Rc::new(port);
    let (harness, cx) = cx.add_window_view(move |window, cx| {
        let session = cx.new(|cx| {
            EditSession::new(RevisionEntity::Arrangement(arrangement), content, port, cx)
        });
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        EditHarness::new(session, environment, window, cx)
    });
    let session = harness.read_with(cx, |harness, _| harness.session().clone());
    // Blur is only reported for an active window.
    cx.update(|window, _| window.activate_window());
    refresh(cx);
    (Rig { harness, session }, cx)
}

/// Focus moves are applied when the window draws; a resize forces a frame.
fn refresh(cx: &mut VisualTestContext) {
    cx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(900.0)));
    cx.run_until_parked();
}

fn focus_title(rig: &Rig, cx: &mut VisualTestContext) {
    cx.update(|window, cx| rig.harness.read(cx).title().clone().focus(window, cx));
    refresh(cx);
}

fn focus_notes(rig: &Rig, cx: &mut VisualTestContext) {
    cx.update(|window, cx| rig.harness.read(cx).notes().clone().focus(window, cx));
    refresh(cx);
}

fn focus_container(rig: &Rig, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = rig.harness.read(cx).container_focus().clone();
        window.focus(&handle, cx);
    });
    refresh(cx);
}

fn save_state(rig: &Rig, cx: &mut VisualTestContext) -> SaveState {
    rig.session.read_with(cx, |s, _| s.save_state())
}

/// Waits until every accepted edit is durable.
fn settle(rig: &Rig, cx: &mut VisualTestContext) {
    cx.executor().allow_parking();
    let deadline = Instant::now() + DEADLINE;
    loop {
        cx.run_until_parked();
        if save_state(rig, cx) == SaveState::Saved {
            return;
        }
        assert!(Instant::now() < deadline, "autosave did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn idle(rig: &Rig, cx: &mut VisualTestContext) {
    let idle = Duration::from_millis(u64::try_from(IDLE_MS).expect("a positive idle window"));
    cx.executor().advance_clock(idle);
    settle(rig, cx);
}

fn session_content(rig: &Rig, cx: &mut VisualTestContext) -> HarnessContent {
    rig.session.read_with(cx, |s, _| s.content().clone())
}

/// Reads the history and checks the earlier part is unchanged and chained.
fn extend(
    cx: &mut VisualTestContext,
    storage: &SharedStorage,
    arrangement: ArrangementId,
    before: &History,
    added: usize,
) -> History {
    let after = history(cx, storage, arrangement);
    assert_eq!(after.len(), before.len() + added, "revisions appended");
    assert_eq!(
        &after[..before.len()],
        &before[..],
        "earlier history is immutable"
    );
    for pair in after.windows(2) {
        assert_eq!(pair[1].0.parent, Some(pair[0].0.id), "an unbroken chain");
    }
    after
}

fn assert_last_edit(history: &History, expected: &HarnessContent) {
    let (record, content) = history.last().expect("a revision");
    assert_eq!(record.kind, RevisionKind::Edit);
    assert!(
        record.session.is_some(),
        "an Edit carries its History session"
    );
    assert_eq!(&harness_of(content), expected);
}

#[gpui::test]
fn edit_session_over_real_storage(cx: &mut TestAppContext) {
    let dir = TempDir::new();
    let storage = open_storage(cx, &dir);

    // 1. A song with an Arrangement of three sections, one occurrence each.
    let song = drive(
        cx,
        storage.with(|s| {
            s.create_song(NewSong {
                title: "Kyrie".to_owned(),
                aliases: Vec::new(),
                original_attribution: None,
                provenance: None,
                rights_notes: None,
                liturgical_moments: Vec::new(),
                occasions: Vec::new(),
            })
        }),
    )
    .expect("song");
    let created = drive(
        cx,
        storage.with(|s| {
            s.create_arrangement(NewArrangement {
                song: song.id,
                title: Some("SATB".to_owned()),
                notes: None,
                languages: vec!["en".to_owned()],
                choirs: Vec::new(),
                rendition_attribution: None,
                occasions: Vec::new(),
                sections: ["A", "B", "C"]
                    .map(|label| NewSection {
                        label: Some(label.to_owned()),
                        lyrics: label.to_owned(),
                    })
                    .to_vec(),
                sequence: vec![0, 1, 2],
            })
        }),
    )
    .expect("arrangement");
    let arrangement = created.id;

    let final_content: HarnessContent;
    let first_title_revision: RevisionId;
    let last_committed: RevisionId;
    {
        let (rig, cx) = open_window(cx, &storage, arrangement);
        let mut log = history(cx, &storage, arrangement);
        let base = log.len();
        assert!(base >= 1, "the arrangement has its creation revision");
        assert_eq!(session_content(&rig, cx).items, ["A", "B", "C"]);

        // 2. Type into the title, then the notes: one Edit revision per field.
        focus_title(&rig, cx);
        cx.simulate_input("Hello");
        assert_eq!(save_state(&rig, cx), SaveState::Saving);
        idle(&rig, cx);
        log = extend(cx, &storage, arrangement, &log, 1);
        let after_title = session_content(&rig, cx);
        assert!(after_title.title.contains("Hello"), "{after_title:?}");
        assert_last_edit(&log, &after_title);
        first_title_revision = log.last().unwrap().0.id;

        focus_notes(&rig, cx);
        cx.simulate_input("Hi");
        idle(&rig, cx);
        log = extend(cx, &storage, arrangement, &log, 1);
        let after_notes = session_content(&rig, cx);
        assert_eq!(after_notes.notes, "Hi");
        assert_eq!(after_notes.title, after_title.title);
        assert_last_edit(&log, &after_notes);
        assert_eq!(
            log.last().unwrap().0.session,
            log[log.len() - 2].0.session,
            "both commits are in one History session"
        );

        // 3. Two reorders through the row buttons; identities are preserved.
        let ids_before = identities(&log.last().unwrap().1);
        let mut expected = after_notes.clone();
        for (selector, swap) in [("edit-item-0-down", (0, 1)), ("edit-item-1-down", (1, 2))] {
            let button = cx.debug_bounds(selector).expect("the row renders a button");
            cx.simulate_click(button.center(), gpui::Modifiers::default());
            settle(&rig, cx);
            expected.items.swap(swap.0, swap.1);
            log = extend(cx, &storage, arrangement, &log, 1);
            assert_eq!(session_content(&rig, cx), expected);
            assert_last_edit(&log, &expected);
            assert_eq!(
                identities(&log.last().unwrap().1),
                ids_before,
                "reordering keeps every section and occurrence identity"
            );
        }
        assert_eq!(expected.items, ["B", "C", "A"]);
        let reordered_once = HarnessContent {
            items: vec!["B".into(), "A".into(), "C".into()],
            ..after_notes.clone()
        };

        // 4. Undo twice and Redo once with no field focused.
        focus_container(&rig, cx);
        for expected in [&reordered_once, &after_notes] {
            cx.simulate_keystrokes("ctrl-z");
            settle(&rig, cx);
            log = extend(cx, &storage, arrangement, &log, 1);
            assert_eq!(&session_content(&rig, cx), expected);
            assert_last_edit(&log, expected);
        }
        assert!(rig.session.read_with(cx, |s, _| s.can_redo()));
        cx.simulate_keystrokes("ctrl-y");
        settle(&rig, cx);
        log = extend(cx, &storage, arrangement, &log, 1);
        assert_eq!(session_content(&rig, cx), reordered_once);
        assert_last_edit(&log, &reordered_once);
        assert_eq!(log.len(), base + 7, "seven Edit revisions in all");
        let sessions: Vec<_> = log[base..].iter().map(|(r, _)| r.session).collect();
        assert!(sessions.iter().all(|s| *s == sessions[0]), "one session");

        // 5. Leave: flush, end the session, close the window.
        let navigation = rig
            .session
            .update(cx, |session, cx| session.request_navigation(cx));
        assert_eq!(drive_in(cx, navigation), NavigationGuard::Proceed);
        rig.session.update(cx, |session, cx| session.leave(cx));
        final_content = reordered_once;
        last_committed = log.last().unwrap().0.id;
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }
    cx.run_until_parked();
    shut_down(cx, &storage);

    // Restart: a new worker over the same file.
    let storage = open_storage(cx, &dir);
    {
        let (rig, cx) = open_window(cx, &storage, arrangement);
        assert_eq!(session_content(&rig, cx), final_content, "durable content");
        let loaded =
            drive_in(cx, storage.with(|s| StoragePort::load(s, arrangement))).expect("load");
        assert_eq!(loaded, final_content);
        assert!(
            !rig.session.read_with(cx, |s, _| s.can_undo()),
            "Undo does not survive a restart"
        );
        assert_eq!(save_state(&rig, cx), SaveState::Saved);
        let before = history(cx, &storage, arrangement);
        assert_eq!(before.last().unwrap().0.id, last_committed);

        // 6. Restore the revision after the first title commit.
        let target_content = before
            .iter()
            .find(|(record, _)| record.id == first_title_revision)
            .expect("the title revision is still there")
            .1
            .clone();
        let (restored, record) = drive_in(
            cx,
            storage.with(|s| {
                s.restore_revision(
                    first_title_revision,
                    last_committed,
                    domain::song_library::PaginationState::Draft,
                )
            }),
        )
        .expect("restore");
        assert_eq!(restored, target_content);
        assert_eq!(record.kind, RevisionKind::Restoration);
        assert_eq!(record.restored_from, Some(first_title_revision));
        assert_eq!(record.parent, Some(last_committed));

        let after = extend(cx, &storage, arrangement, &before, 1);
        let (last, content) = after.last().unwrap();
        assert_eq!(last.id, record.id);
        assert_eq!(
            content, &target_content,
            "current content is the restored one"
        );
        let reloaded =
            drive_in(cx, storage.with(|s| StoragePort::load(s, arrangement))).expect("load");
        assert_eq!(reloaded, harness_of(&target_content));
        assert_eq!(reloaded.items, ["A", "B", "C"]);
        assert!(reloaded.title.contains("Hello"));
        assert!(reloaded.notes.is_empty(), "the notes edit came later");

        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }
    cx.run_until_parked();
    shut_down(cx, &storage);
}
