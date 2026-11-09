//! `[UI-AUTO]` tests of the edit session through the harness view: coalesced
//! autosave, native-first Undo, editor Undo, save states and navigation.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    time::Duration,
};

use domain::{
    identity::{EntityId, SongId},
    revisions::{EditPoint, RevisionEntity, RevisionKind},
};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use ui::environment::{ResolvedAppearance, UiEnvironment, UiPreferences};

use super::{
    CommitPoint, EditSession, NavigationGuard, RevisionPort, SaveFailure, SaveState,
    harness::{EditHarness, HarnessContent},
};

#[derive(Default)]
struct FakePort {
    log: RefCell<Vec<(HarnessContent, CommitPoint)>>,
    fail: Rc<Cell<bool>>,
    hold: Cell<bool>,
    gates: RefCell<VecDeque<async_channel::Sender<()>>>,
}

impl FakePort {
    fn log(&self) -> Vec<(HarnessContent, CommitPoint)> {
        self.log.borrow().clone()
    }

    fn release(&self) {
        let gate = self.gates.borrow_mut().pop_front();
        if let Some(gate) = gate {
            gate.try_send(()).unwrap();
        }
    }
}

impl RevisionPort<HarnessContent> for FakePort {
    fn commit(
        &self,
        content: HarnessContent,
        point: CommitPoint,
    ) -> Pin<Box<dyn Future<Output = Result<(), SaveFailure>>>> {
        self.log.borrow_mut().push((content, point));
        let fail = self.fail.clone();
        let gate = self.hold.get().then(|| {
            let (tx, rx) = async_channel::bounded(1);
            self.gates.borrow_mut().push_back(tx);
            rx
        });
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.recv().await.ok();
            }
            if fail.get() {
                Err(SaveFailure {
                    message: "disk is full".into(),
                })
            } else {
                Ok(())
            }
        })
    }
}

struct Rig {
    harness: Entity<EditHarness>,
    session: Entity<EditSession<HarnessContent>>,
    port: Rc<FakePort>,
}

fn initial() -> HarnessContent {
    HarnessContent {
        title: String::new(),
        notes: String::new(),
        items: vec!["a".into(), "b".into(), "c".into()],
    }
}

fn open(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    cx.update(|cx| {
        ui::init(cx).unwrap();
        super::init(cx);
    });
    let port = Rc::new(FakePort::default());
    let for_window = port.clone();
    let (harness, cx) = cx.add_window_view(move |window, cx| {
        let entity =
            RevisionEntity::Song(SongId::from_entity(EntityId::from_random_bytes([7; 16])));
        let port: Rc<dyn RevisionPort<HarnessContent>> = for_window;
        let session = cx.new(|cx| EditSession::new(entity, initial(), port, cx));
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        EditHarness::new(session, environment, window, cx)
    });
    let session = harness.read_with(cx, |harness, _| harness.session().clone());
    // Blur is only reported for an active window.
    cx.update(|window, _| window.activate_window());
    cx.update(|window, cx| {
        harness.read(cx).title().clone().focus(window, cx);
    });
    cx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(900.0)));
    cx.run_until_parked();
    (
        Rig {
            harness,
            session,
            port,
        },
        cx,
    )
}

fn advance(cx: &mut VisualTestContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

fn type_slowly(cx: &mut VisualTestContext, text: &str, gap_ms: u64) {
    for character in text.chars() {
        cx.simulate_input(&character.to_string());
        advance(cx, Duration::from_millis(gap_ms));
    }
}

fn focus_container(rig: &Rig, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = rig.harness.read(cx).container_focus().clone();
        window.focus(&handle, cx);
    });
    // Focus moves are applied when the window draws; a resize forces a frame.
    cx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(900.0)));
    cx.run_until_parked();
}

fn structural(
    rig: &Rig,
    cx: &mut VisualTestContext,
    point: EditPoint,
    swap: (usize, usize),
) -> bool {
    let accepted = rig.session.update(cx, |session, cx| {
        session.structural(point, |content| content.items.swap(swap.0, swap.1), cx)
    });
    cx.run_until_parked();
    accepted
}

fn state(rig: &Rig, cx: &mut VisualTestContext) -> SaveState {
    rig.session.read_with(cx, |session, _| session.save_state())
}

fn content(rig: &Rig, cx: &mut VisualTestContext) -> HarnessContent {
    rig.session
        .read_with(cx, |session, _| session.content().clone())
}

fn title_field(rig: &Rig, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| rig.harness.read(cx).title().value(cx).to_string())
}

#[gpui::test]
fn typing_coalesces_into_one_edit_commit_after_a_second_of_idle(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);

    type_slowly(cx, "ab", 300);
    cx.simulate_input("c");
    advance(cx, Duration::from_millis(999));
    cx.run_until_parked();
    assert!(
        rig.port.log().is_empty(),
        "nothing commits before 1000 ms of idle"
    );

    advance(cx, Duration::from_millis(1));
    cx.run_until_parked();
    let log = rig.port.log();
    assert_eq!(log.len(), 1, "typing is one point");
    assert_eq!(log[0].0.title, "abc");
    assert_eq!(log[0].1.kind, RevisionKind::Edit);
    assert!(log[0].1.session.is_some());
    assert_eq!(state(&rig, cx), SaveState::Saved);
}

#[gpui::test]
fn focus_departure_commits_at_once(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);

    cx.simulate_input("hi");
    assert!(rig.port.log().is_empty());
    focus_container(&rig, cx);

    let log = rig.port.log();
    assert_eq!(log.len(), 1, "blurring the field commits without waiting");
    assert_eq!(log[0].0.title, "hi");
}

#[gpui::test]
fn a_structural_change_flushes_pending_text_first(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);

    cx.simulate_input("x");
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));

    let log = rig.port.log();
    assert_eq!(log.len(), 2, "text first, then the reorder");
    assert_eq!(log[0].0.title, "x");
    assert_eq!(
        log[0].0.items,
        initial().items,
        "the text point predates the reorder"
    );
    assert_eq!(log[1].0.title, "x");
    assert_eq!(log[1].0.items, vec!["b", "a", "c"]);
    assert_eq!(log[0].1.session, log[1].1.session, "one History session");
    assert!(log[0].1.session.is_some());
}

#[gpui::test]
fn the_buttons_reorder_and_native_undo_wins_while_a_field_is_focused(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);

    // The per-row buttons reorder through the session.
    let down = cx
        .debug_bounds("edit-item-0-down")
        .expect("row 0 renders Move down");
    cx.simulate_click(down.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    assert_eq!(content(&rig, cx).items, vec!["b", "a", "c"]);
    assert_eq!(rig.port.log().len(), 1);

    // Title focused: type, commit, type more, commit.
    cx.update(|window, cx| rig.harness.read(cx).title().clone().focus(window, cx));
    cx.run_until_parked();
    cx.simulate_input("abc");
    advance(cx, Duration::from_millis(1_000));
    cx.run_until_parked();
    cx.simulate_input("d");
    advance(cx, Duration::from_millis(1_000));
    cx.run_until_parked();
    assert_eq!(content(&rig, cx).title, "abcd");
    let commits = rig.port.log().len();

    // Native Undo: the field reverts, the session does not undo anything.
    cx.simulate_keystrokes("ctrl-z");
    assert_ne!(title_field(&rig, cx), "abcd", "the field reverted natively");
    assert_eq!(
        rig.port.log().len(),
        commits,
        "no session Undo commit happened"
    );

    // Leaving the field commits the reverted text as an ordinary point.
    focus_container(&rig, cx);
    assert_eq!(rig.port.log().len(), commits + 1);
    assert_eq!(content(&rig, cx).title, title_field(&rig, cx));

    // Container focused: Ctrl+Z is the session Undo and the fields follow.
    cx.simulate_keystrokes("ctrl-z");
    cx.run_until_parked();
    let log = rig.port.log();
    assert_eq!(log.len(), commits + 2);
    let (undone, point) = log.last().unwrap();
    assert_eq!(point.kind, RevisionKind::Edit);
    assert!(point.session.is_some());
    assert_eq!(
        undone.title, "abcd",
        "back to the state before the reverted text"
    );
    assert_eq!(
        title_field(&rig, cx),
        "abcd",
        "ContentReplaced reloads the Field"
    );
}

#[gpui::test]
fn undo_and_redo_walk_committed_points_and_a_new_edit_clears_redo(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);
    focus_container(&rig, cx);

    let p0 = content(&rig, cx);
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));
    let p1 = content(&rig, cx);
    assert!(structural(&rig, cx, EditPoint::Structural, (1, 2)));
    let p2 = content(&rig, cx);
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 2)));
    let p3 = content(&rig, cx);
    assert_eq!(rig.port.log().len(), 3);

    for expected in [&p2, &p1, &p0] {
        assert!(rig.session.update(cx, |s, cx| s.undo(cx)));
        cx.run_until_parked();
        assert_eq!(&content(&rig, cx), expected);
        assert_eq!(rig.port.log().last().unwrap().0, *expected);
    }
    assert!(!rig.session.update(cx, |s, cx| s.undo(cx)), "oldest state");
    assert!(!rig.session.read_with(cx, |s, _| s.can_undo()));

    for expected in [&p1, &p2, &p3] {
        assert!(rig.session.read_with(cx, |s, _| s.can_redo()));
        assert!(rig.session.update(cx, |s, cx| s.redo(cx)));
        cx.run_until_parked();
        assert_eq!(&content(&rig, cx), expected);
    }
    assert!(!rig.session.update(cx, |s, cx| s.redo(cx)), "newest state");

    // Undo twice, then a new edit ends the redo chain.
    assert!(rig.session.update(cx, |s, cx| s.undo(cx)));
    cx.run_until_parked();
    assert!(rig.session.update(cx, |s, cx| s.undo(cx)));
    cx.run_until_parked();
    assert!(rig.session.read_with(cx, |s, _| s.can_redo()));
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));
    assert!(!rig.session.read_with(cx, |s, _| s.can_redo()));
    assert!(!rig.session.update(cx, |s, cx| s.redo(cx)));
    let sessions: std::collections::HashSet<_> = rig
        .port
        .log()
        .iter()
        .map(|(_, point)| point.session)
        .collect();
    assert_eq!(sessions.len(), 1, "all of it is one History session");
}

#[gpui::test]
fn save_states_follow_the_commit_and_a_failure_blocks_history(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);
    focus_container(&rig, cx);
    assert_eq!(state(&rig, cx), SaveState::Saved);
    assert_eq!(SaveState::Saved.label(), "Saved");
    assert_eq!(SaveState::Saving.label(), "Saving\u{2026}");
    assert_eq!(SaveState::CouldntSave.label(), "Couldn't save");

    // Held: Saving, and history is refused until it settles.
    rig.port.hold.set(true);
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));
    assert_eq!(state(&rig, cx), SaveState::Saving);
    assert!(
        !structural(&rig, cx, EditPoint::Structural, (1, 2)),
        "refused while saving"
    );
    assert!(!rig.session.update(cx, |s, cx| s.undo(cx)));
    assert!(!rig.session.read_with(cx, |s, _| s.can_undo()));
    rig.port.hold.set(false);
    rig.port.release();
    cx.run_until_parked();
    assert_eq!(state(&rig, cx), SaveState::Saved);

    // Failure: content retained, history refused, retry succeeds.
    rig.port.fail.set(true);
    assert!(structural(&rig, cx, EditPoint::Structural, (1, 2)));
    assert_eq!(state(&rig, cx), SaveState::CouldntSave);
    let kept = content(&rig, cx);
    assert_eq!(kept.items, vec!["b", "c", "a"]);
    assert_ne!(
        rig.session.read_with(cx, |s, _| s.committed().clone()),
        kept
    );
    assert!(!rig.session.update(cx, |s, cx| s.undo(cx)));
    assert!(!structural(&rig, cx, EditPoint::Structural, (0, 1)));
    advance(cx, Duration::from_secs(10));
    cx.run_until_parked();
    assert_eq!(rig.port.log().len(), 2, "no automatic retry loop");

    rig.port.fail.set(false);
    rig.session.update(cx, |s, cx| s.retry_save(cx));
    cx.run_until_parked();
    assert_eq!(state(&rig, cx), SaveState::Saved);
    assert_eq!(
        rig.session.read_with(cx, |s, _| s.committed().clone()),
        kept
    );
    assert_eq!(rig.port.log().last().unwrap().0, kept);
}

#[gpui::test]
fn typing_during_an_in_flight_commit_commits_after_it_settles(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);

    rig.port.hold.set(true);
    cx.simulate_input("a");
    advance(cx, Duration::from_millis(1_000));
    cx.run_until_parked();
    assert_eq!(rig.port.log().len(), 1, "first commit is in flight");
    assert_eq!(state(&rig, cx), SaveState::Saving);

    cx.simulate_input("b");
    advance(cx, Duration::from_millis(1_000));
    cx.run_until_parked();
    assert_eq!(rig.port.log().len(), 1, "commits are serialized");

    rig.port.hold.set(false);
    rig.port.release();
    cx.run_until_parked();
    advance(cx, Duration::from_millis(1_000));
    cx.run_until_parked();
    let log = rig.port.log();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].0.title, "a");
    assert_eq!(log[1].0.title, "ab", "the newer typing was not dropped");
    assert_eq!(state(&rig, cx), SaveState::Saved);
}

fn navigate(rig: &Rig, cx: &mut VisualTestContext) -> NavigationGuard {
    let task = rig.session.update(cx, |s, cx| s.request_navigation(cx));
    let answer = Rc::new(Cell::new(None));
    let slot = answer.clone();
    cx.update(|_, cx| cx.spawn(async move |_| slot.set(Some(task.await))).detach());
    cx.run_until_parked();
    answer.take().expect("the navigation guard resolved")
}

#[gpui::test]
fn navigation_proceeds_when_saved_and_blocks_when_the_flush_fails(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);

    cx.simulate_input("x");
    assert_eq!(
        navigate(&rig, cx),
        NavigationGuard::Proceed,
        "flushes pending text"
    );
    assert_eq!(rig.port.log().len(), 1);
    assert_eq!(rig.port.log()[0].0.title, "x");

    focus_container(&rig, cx);
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));
    assert!(rig.session.read_with(cx, |s, _| s.can_undo()));

    rig.port.fail.set(true);
    assert!(structural(&rig, cx, EditPoint::Structural, (1, 2)));
    let kept = content(&rig, cx);
    assert_eq!(navigate(&rig, cx), NavigationGuard::Blocked);
    assert_eq!(content(&rig, cx), kept, "Blocked keeps the content");
    assert_eq!(state(&rig, cx), SaveState::CouldntSave);

    rig.session.update(cx, |s, cx| s.leave_without_saving(cx));
    cx.run_until_parked();
    assert!(
        !rig.session.read_with(cx, |s, _| s.can_undo()),
        "Undo is cleared"
    );
    assert_eq!(state(&rig, cx), SaveState::Saved);
    assert_eq!(
        content(&rig, cx).items,
        vec!["b", "a", "c"],
        "back to committed"
    );
}

#[gpui::test]
fn history_sessions_split_after_thirty_minutes_and_after_leaving(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);
    focus_container(&rig, cx);

    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));
    advance(cx, Duration::from_secs(29 * 60));
    assert!(structural(&rig, cx, EditPoint::Structural, (1, 2)));
    advance(cx, Duration::from_secs(31 * 60));
    assert!(structural(&rig, cx, EditPoint::Structural, (0, 1)));

    let ids: Vec<_> = rig
        .port
        .log()
        .iter()
        .map(|(_, point)| point.session)
        .collect();
    assert_eq!(ids[0], ids[1], "29 minutes apart is one session");
    assert_ne!(ids[1], ids[2], "31 minutes apart is a new session");

    rig.session.update(cx, |s, cx| s.leave(cx));
    assert!(structural(&rig, cx, EditPoint::Structural, (1, 2)));
    let last = rig.port.log().last().unwrap().1.session;
    assert_ne!(last, ids[2], "leaving ends the session");
    assert!(last.is_some());
}

#[gpui::test]
fn a_semantic_replacement_is_a_standalone_replacement(cx: &mut TestAppContext) {
    let (rig, cx) = open(cx);
    focus_container(&rig, cx);

    let accepted = rig.session.update(cx, |session, cx| {
        session.structural(
            EditPoint::SemanticReplacement,
            |content| content.items = vec!["z".into()],
            cx,
        )
    });
    cx.run_until_parked();

    assert!(accepted);
    let log = rig.port.log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].1.kind, RevisionKind::Replacement);
    assert_eq!(log[0].1.session, None);
}
