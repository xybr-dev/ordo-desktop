//! Edit-safety mechanics: text coalescing, editor Undo and History sessions.
//!
//! Everything here is pure and deterministic. Callers pass time in as
//! milliseconds and supply fresh identities, so the domain reads no clock and
//! draws no randomness (Decisions 138, 139, 140, 141).

use crate::identity::{ArrangementId, HistorySessionId, RevisionId, SongId, Timestamp};

/// The entity one revision chain belongs to. Song-work edits and Arrangement
/// edits never share a session (Decision 141).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RevisionEntity {
    /// A Song work.
    Song(SongId),
    /// An Arrangement of a Song work.
    Arrangement(ArrangementId),
}

/// Why a revision exists. Only `Edit` revisions group into History sessions;
/// every other kind is a standalone History event (Decision 140/141).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RevisionKind {
    /// The entity was created.
    Creation,
    /// An ordinary committed edit.
    Edit,
    /// Content arrived through an import.
    Import,
    /// Content was replaced wholesale.
    Replacement,
    /// An earlier revision was restored.
    Restoration,
}

impl RevisionKind {
    /// Whether this kind is its own History event rather than part of a session.
    pub const fn is_standalone(self) -> bool {
        !matches!(self, Self::Edit)
    }

    /// The stable stored text for this kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Creation => "creation",
            Self::Edit => "edit",
            Self::Import => "import",
            Self::Replacement => "replacement",
            Self::Restoration => "restoration",
        }
    }

    /// Inverse of [`RevisionKind::as_str`]; exact match only.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "creation" => Some(Self::Creation),
            "edit" => Some(Self::Edit),
            "import" => Some(Self::Import),
            "replacement" => Some(Self::Replacement),
            "restoration" => Some(Self::Restoration),
            _ => None,
        }
    }
}

/// How an edit point was reached (Decision 138).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditPoint {
    /// Continuous typing settled by idle time or focus departure.
    TextSession,
    /// A structural change such as reordering or deleting.
    Structural,
    /// A semantic replacement of content.
    SemanticReplacement,
}

/// Idle time after the last keystroke before a pending text edit commits.
pub const IDLE_MS: i64 = 1_000;

/// Identifies one recorded text edit. Generations increase monotonically for
/// the life of a [`TextCoalescer`], so a save can tell whether typing
/// continued after the snapshot it saved was taken.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EditGeneration(u64);

/// Coalesces continuous typing into one committed point (Decision 138):
/// a pending text edit commits after `IDLE_MS` without further edits, or as
/// soon as focus leaves the edited field.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextCoalescer {
    last_edit_ms: Option<i64>,
    focus_left: bool,
    latest: u64,
}

impl TextCoalescer {
    /// A coalescer with nothing pending.
    pub const fn new() -> Self {
        Self {
            last_edit_ms: None,
            focus_left: false,
            latest: 0,
        }
    }

    /// Records an edit at `now_ms`; it stays pending until due. Returns the
    /// edit's generation, which the caller passes to [`TextCoalescer::settle`]
    /// once the snapshot taken at this point has been saved.
    pub fn edited(&mut self, now_ms: i64) -> EditGeneration {
        self.latest = self.latest.saturating_add(1);
        self.last_edit_ms = Some(now_ms);
        self.focus_left = false;
        EditGeneration(self.latest)
    }

    /// The generation of the pending edit, or `None` when nothing is pending.
    pub fn generation(&self) -> Option<EditGeneration> {
        self.last_edit_ms.map(|_| EditGeneration(self.latest))
    }

    /// True when a pending edit exists and it is due: focus has left, or
    /// `now_ms - last_edit >= IDLE_MS`. A clock that went backwards is never
    /// due by time.
    pub fn is_due(&self, now_ms: i64) -> bool {
        match self.last_edit_ms {
            None => false,
            Some(last) => self.focus_left || now_ms.saturating_sub(last) >= IDLE_MS,
        }
    }

    /// Focus left the field: a pending edit is due immediately. Returns
    /// whether one was pending.
    pub fn focus_left(&mut self) -> bool {
        if self.last_edit_ms.is_some() {
            self.focus_left = true;
            true
        } else {
            false
        }
    }

    /// The snapshot taken at generation `saved` was committed (or discarded).
    /// Clears the pending edit only when no newer edit was recorded since;
    /// otherwise the newer edit stays pending with its own timing.
    pub fn settle(&mut self, saved: EditGeneration) {
        if saved.0 >= self.latest {
            self.last_edit_ms = None;
            self.focus_left = false;
        }
    }

    /// Whether an edit is waiting to commit.
    pub fn is_pending(&self) -> bool {
        self.last_edit_ms.is_some()
    }

    /// Milliseconds until due, for scheduling a timer; `None` when nothing is
    /// pending. Saturates at 0; a backwards clock reports `Some(IDLE_MS)`.
    pub fn due_in(&self, now_ms: i64) -> Option<i64> {
        let last = self.last_edit_ms?;
        if self.focus_left {
            return Some(0);
        }
        if now_ms < last {
            return Some(IDLE_MS);
        }
        Some((IDLE_MS - now_ms.saturating_sub(last)).max(0))
    }
}

/// Editor-local immediate Undo/Redo over committed content states
/// (Decision 139). In memory only: dropped on entity departure or restart.
#[derive(Clone, Debug)]
pub struct UndoStack<C> {
    states: Vec<C>,
    cursor: usize,
}

impl<C: Clone + Eq> UndoStack<C> {
    /// Starts at the entity's current committed content.
    pub fn new(current: C) -> Self {
        Self {
            states: vec![current],
            cursor: 0,
        }
    }

    /// The state at the cursor.
    pub fn current(&self) -> &C {
        &self.states[self.cursor]
    }

    /// Records a newly committed state and discards any redo states. A state
    /// equal to the current one is ignored and returns false.
    pub fn commit(&mut self, state: C) -> bool {
        if *self.current() == state {
            return false;
        }
        self.states.truncate(self.cursor + 1);
        self.states.push(state);
        self.cursor += 1;
        true
    }

    /// The state [`UndoStack::undo`] would move to, without moving.
    pub fn peek_undo(&self) -> Option<&C> {
        self.cursor.checked_sub(1).map(|index| &self.states[index])
    }

    /// The state [`UndoStack::redo`] would move to, without moving.
    pub fn peek_redo(&self) -> Option<&C> {
        self.states.get(self.cursor + 1)
    }

    /// Moves back one state and returns it; `None` at the oldest state.
    ///
    /// Call this only after the durable commit of the state returned by
    /// [`UndoStack::peek_undo`] succeeded, so a failed save leaves the stack
    /// unchanged.
    pub fn undo(&mut self) -> Option<C> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        Some(self.states[self.cursor].clone())
    }

    /// Moves forward one state and returns it; `None` at the newest state.
    ///
    /// Call this only after the durable commit of the state returned by
    /// [`UndoStack::peek_redo`] succeeded.
    pub fn redo(&mut self) -> Option<C> {
        if self.cursor + 1 >= self.states.len() {
            return None;
        }
        self.cursor += 1;
        Some(self.states[self.cursor].clone())
    }

    /// Whether an older state exists.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether a newer state exists.
    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.states.len()
    }

    /// Departure from the entity: keep only the current state.
    pub fn clear(&mut self) {
        let current = self.states.swap_remove(self.cursor);
        self.states.clear();
        self.states.push(current);
        self.cursor = 0;
    }
}

/// Idle gap after which the next Edit commit starts a new History session.
pub const SESSION_IDLE_MS: i64 = 30 * 60 * 1_000;

/// Assigns History-session ids to Edit commits (Decision 141).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionClock {
    open: Option<OpenSession>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenSession {
    entity: RevisionEntity,
    session: HistorySessionId,
    last_commit_ms: i64,
}

impl SessionClock {
    /// A clock with no open session.
    pub const fn new() -> Self {
        Self { open: None }
    }

    /// The session for an Edit commit on `entity` at `now_ms`. Starts a new
    /// session (using `fresh`) on the first commit, on a different entity, or
    /// when more than `SESSION_IDLE_MS` passed since the previous commit
    /// (exactly `SESSION_IDLE_MS` still continues). A clock that goes
    /// backwards continues the session.
    pub fn on_edit(
        &mut self,
        entity: RevisionEntity,
        now_ms: i64,
        fresh: HistorySessionId,
    ) -> HistorySessionId {
        let (session, last_commit_ms) = match self.open {
            Some(open)
                if open.entity == entity
                    && now_ms.saturating_sub(open.last_commit_ms) <= SESSION_IDLE_MS =>
            {
                // A backwards clock never moves the idle reference earlier.
                (open.session, open.last_commit_ms.max(now_ms))
            }
            _ => (fresh, now_ms),
        };
        self.open = Some(OpenSession {
            entity,
            session,
            last_commit_ms,
        });
        session
    }

    /// Navigation away or application close: the next edit starts a new session.
    pub fn end(&mut self) {
        self.open = None;
    }
}

/// The facts History needs about one stored revision, oldest first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionSummary {
    /// The revision's identity.
    pub id: RevisionId,
    /// The entity whose chain it belongs to.
    pub entity: RevisionEntity,
    /// Why the revision exists.
    pub kind: RevisionKind,
    /// The History session, for Edit revisions that have one.
    pub session: Option<HistorySessionId>,
    /// When the revision was created.
    pub created_at: Timestamp,
}

/// One History row: a standalone event or a group of Edit revisions from one session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryEntry {
    /// A standalone event.
    Event(RevisionSummary),
    /// Edit revisions of one session, oldest first and non-empty.
    Session {
        /// The shared session id.
        session: HistorySessionId,
        /// The grouped revisions, oldest first.
        revisions: Vec<RevisionSummary>,
    },
}

/// Groups consecutive Edit revisions that share an entity and a session id;
/// every standalone kind, and every Edit without a session, is its own entry.
/// Order is preserved (oldest first). Never merges across entities.
pub fn group_history(revisions: &[RevisionSummary]) -> Vec<HistoryEntry> {
    let mut entries: Vec<HistoryEntry> = Vec::new();
    for revision in revisions {
        let session = match (revision.kind, revision.session) {
            (RevisionKind::Edit, Some(session)) => session,
            _ => {
                entries.push(HistoryEntry::Event(revision.clone()));
                continue;
            }
        };
        if let Some(HistoryEntry::Session {
            session: open,
            revisions: group,
        }) = entries.last_mut()
            && *open == session
            && group
                .last()
                .is_some_and(|last| last.entity == revision.entity)
        {
            group.push(revision.clone());
            continue;
        }
        entries.push(HistoryEntry::Session {
            session,
            revisions: vec![revision.clone()],
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::EntityId;

    fn eid(n: u8) -> EntityId {
        EntityId::from_random_bytes([n; 16])
    }

    fn session(n: u8) -> HistorySessionId {
        HistorySessionId::from_entity(eid(n))
    }

    fn song(n: u8) -> RevisionEntity {
        RevisionEntity::Song(SongId::from_entity(eid(n)))
    }

    fn arrangement(n: u8) -> RevisionEntity {
        RevisionEntity::Arrangement(ArrangementId::from_entity(eid(n)))
    }

    #[test]
    fn coalescer_is_due_at_exactly_the_idle_boundary() {
        let mut c = TextCoalescer::new();
        assert!(!c.is_pending());
        assert!(!c.is_due(10_000));
        c.edited(0);
        assert!(c.is_pending());
        assert!(!c.is_due(999));
        assert!(c.is_due(1_000));
        assert!(c.is_due(5_000));
    }

    #[test]
    fn coalescer_second_edit_pushes_due_time() {
        let mut c = TextCoalescer::new();
        c.edited(0);
        c.edited(900);
        assert!(!c.is_due(1_000));
        assert!(!c.is_due(1_899));
        assert!(c.is_due(1_900));
    }

    #[test]
    fn coalescer_focus_left_makes_pending_edit_due_at_once() {
        let mut c = TextCoalescer::new();
        let g = c.edited(0);
        assert!(!c.is_due(10));
        assert!(c.focus_left());
        assert!(c.is_due(10));
        assert!(c.is_due(-5));
        assert_eq!(c.due_in(10), Some(0));
        c.settle(g);
        assert!(!c.is_due(10));
    }

    #[test]
    fn coalescer_focus_left_without_pending_returns_false() {
        let mut c = TextCoalescer::new();
        assert!(!c.focus_left());
        assert!(!c.is_due(0));
        c.edited(0);
        assert!(!c.is_due(10), "earlier focus_left must not linger");
    }

    #[test]
    fn coalescer_settle_clears_pending() {
        let mut c = TextCoalescer::new();
        let g = c.edited(0);
        assert_eq!(c.generation(), Some(g));
        c.settle(g);
        assert_eq!(c.generation(), None);
        assert!(!c.is_pending());
        assert!(!c.is_due(5_000));
        assert_eq!(c.due_in(5_000), None);
    }

    #[test]
    fn coalescer_settle_keeps_typing_recorded_during_a_save() {
        let mut c = TextCoalescer::new();
        let saved = c.edited(0);
        assert!(c.is_due(1_000));
        // The save of that snapshot starts; the user types again.
        let newer = c.edited(1_200);
        assert!(newer > saved);
        // The first save finishes: the newer edit must stay pending.
        c.settle(saved);
        assert!(c.is_pending());
        assert_eq!(c.generation(), Some(newer));
        assert!(!c.is_due(2_199), "newer edit keeps its own timing");
        assert!(c.is_due(2_200));
        assert_eq!(c.due_in(1_700), Some(500));
        c.settle(newer);
        assert!(!c.is_pending());
    }

    #[test]
    fn coalescer_settle_of_stale_generation_keeps_focus_departure_of_newer_edit() {
        let mut c = TextCoalescer::new();
        let saved = c.edited(0);
        let newer = c.edited(10);
        assert!(c.focus_left());
        c.settle(saved);
        assert_eq!(c.generation(), Some(newer));
        assert!(c.is_due(10));
    }

    #[test]
    fn coalescer_backwards_clock_is_never_due_by_time() {
        let mut c = TextCoalescer::new();
        c.edited(5_000);
        assert!(!c.is_due(4_000));
        assert!(!c.is_due(i64::MIN));
        assert_eq!(c.due_in(4_000), Some(IDLE_MS));
        assert!(c.focus_left());
        assert!(c.is_due(4_000));
    }

    #[test]
    fn coalescer_due_in_counts_down_and_saturates() {
        let mut c = TextCoalescer::new();
        assert_eq!(c.due_in(0), None);
        c.edited(100);
        assert_eq!(c.due_in(100), Some(1_000));
        assert_eq!(c.due_in(600), Some(500));
        assert_eq!(c.due_in(1_099), Some(1));
        assert_eq!(c.due_in(1_100), Some(0));
        assert_eq!(c.due_in(9_999), Some(0));
    }

    #[test]
    fn undo_stack_walks_back_and_forward() {
        let mut s = UndoStack::new("a".to_owned());
        assert_eq!(s.current(), "a");
        assert!(!s.can_undo());
        assert!(!s.can_redo());
        assert!(s.commit("b".to_owned()));
        assert!(s.commit("c".to_owned()));
        assert_eq!(s.current(), "c");
        assert_eq!(s.undo(), Some("b".to_owned()));
        assert_eq!(s.undo(), Some("a".to_owned()));
        assert_eq!(s.current(), "a");
        assert!(!s.can_undo());
        assert!(s.can_redo());
        assert_eq!(s.redo(), Some("b".to_owned()));
        assert_eq!(s.redo(), Some("c".to_owned()));
        assert_eq!(s.redo(), None);
        assert!(!s.can_redo());
        assert!(s.can_undo());
    }

    #[test]
    fn undo_stack_new_commit_discards_redo() {
        let mut s = UndoStack::new(1);
        s.commit(2);
        s.commit(3);
        assert_eq!(s.undo(), Some(2));
        assert!(s.can_redo());
        assert!(s.commit(9));
        assert!(!s.can_redo());
        assert_eq!(s.redo(), None);
        assert_eq!(s.current(), &9);
        assert_eq!(s.undo(), Some(2));
        assert_eq!(s.undo(), Some(1));
    }

    #[test]
    fn undo_stack_ignores_equal_state() {
        let mut s = UndoStack::new(1);
        assert!(!s.commit(1));
        assert!(!s.can_undo());
        s.commit(2);
        s.undo();
        assert!(!s.commit(1), "equal to current ignored, redo kept");
        assert!(s.can_redo());
    }

    #[test]
    fn undo_stack_peek_leaves_stack_unchanged_until_undo_or_redo() {
        let mut s = UndoStack::new(1);
        assert_eq!(s.peek_undo(), None);
        assert_eq!(s.peek_redo(), None);
        s.commit(2);
        s.commit(3);
        // A failed save: peek, then do not move.
        assert_eq!(s.peek_undo(), Some(&2));
        assert_eq!(s.peek_undo(), Some(&2));
        assert_eq!(s.current(), &3);
        assert!(!s.can_redo());
        assert_eq!(s.peek_redo(), None);
        // A successful save moves.
        assert_eq!(s.undo(), Some(2));
        assert_eq!(s.peek_redo(), Some(&3));
        assert_eq!(s.peek_redo(), Some(&3));
        assert_eq!(s.current(), &2);
        assert_eq!(s.redo(), Some(3));
        assert_eq!(s.current(), &3);
    }

    #[test]
    fn undo_stack_undo_at_oldest_is_none() {
        let mut s = UndoStack::new(1);
        assert_eq!(s.undo(), None);
        assert_eq!(s.current(), &1);
    }

    #[test]
    fn undo_stack_clear_keeps_only_current() {
        let mut s = UndoStack::new(1);
        s.commit(2);
        s.commit(3);
        s.undo();
        s.clear();
        assert_eq!(s.current(), &2);
        assert!(!s.can_undo());
        assert!(!s.can_redo());
        assert_eq!(s.undo(), None);
        assert_eq!(s.redo(), None);
    }

    #[test]
    fn session_clock_groups_by_idle_gap_entity_and_end() {
        let mut clock = SessionClock::new();
        let e = song(1);
        let a = clock.on_edit(e, 0, session(10));
        assert_eq!(a, session(10));
        assert_eq!(clock.on_edit(e, 100, session(11)), a);
        assert_eq!(
            clock.on_edit(e, 100 + SESSION_IDLE_MS, session(12)),
            a,
            "exactly SESSION_IDLE_MS still continues"
        );
        let last = 100 + SESSION_IDLE_MS;
        let b = clock.on_edit(e, last + SESSION_IDLE_MS + 1, session(13));
        assert_eq!(b, session(13));
        let now = last + SESSION_IDLE_MS + 1;
        let c = clock.on_edit(song(2), now + 1, session(14));
        assert_eq!(c, session(14));
        clock.end();
        let d = clock.on_edit(song(2), now + 2, session(15));
        assert_eq!(d, session(15));
    }

    #[test]
    fn session_clock_backwards_clock_keeps_the_latest_commit_as_reference() {
        let mut clock = SessionClock::new();
        let e = song(1);
        let a = clock.on_edit(e, 10_000, session(1));
        assert_eq!(
            clock.on_edit(e, 5_000, session(2)),
            a,
            "backwards continues"
        );
        // Reference stays 10_000, not 5_000.
        assert_eq!(
            clock.on_edit(e, 10_000 + SESSION_IDLE_MS, session(3)),
            a,
            "exactly the idle gap from the latest commit continues"
        );
        let b = clock.on_edit(e, 10_000 + 2 * SESSION_IDLE_MS + 1, session(4));
        assert_eq!(b, session(4));
    }

    #[test]
    fn session_clock_never_shares_between_song_and_arrangement() {
        let mut clock = SessionClock::new();
        let same = eid(7);
        let s = RevisionEntity::Song(SongId::from_entity(same));
        let a = RevisionEntity::Arrangement(ArrangementId::from_entity(same));
        assert_eq!(clock.on_edit(s, 0, session(1)), session(1));
        assert_eq!(clock.on_edit(a, 1, session(2)), session(2));
        assert_eq!(clock.on_edit(s, 2, session(3)), session(3));
    }

    fn rev(
        n: u8,
        entity: RevisionEntity,
        kind: RevisionKind,
        sess: Option<HistorySessionId>,
    ) -> RevisionSummary {
        RevisionSummary {
            id: RevisionId::from_entity(eid(n)),
            entity,
            kind,
            session: sess,
            created_at: Timestamp::from_millis(i64::from(n)),
        }
    }

    #[test]
    fn group_history_groups_edits_and_keeps_standalone_events() {
        let e = song(1);
        let (s1, s2) = (session(100), session(101));
        let r = [
            rev(1, e, RevisionKind::Creation, None),
            rev(2, e, RevisionKind::Edit, Some(s1)),
            rev(3, e, RevisionKind::Edit, Some(s1)),
            rev(4, e, RevisionKind::Restoration, None),
            rev(5, e, RevisionKind::Edit, Some(s1)),
            rev(6, e, RevisionKind::Edit, Some(s2)),
        ];
        let got = group_history(&r);
        assert_eq!(
            got,
            vec![
                HistoryEntry::Event(r[0].clone()),
                HistoryEntry::Session {
                    session: s1,
                    revisions: vec![r[1].clone(), r[2].clone()]
                },
                HistoryEntry::Event(r[3].clone()),
                HistoryEntry::Session {
                    session: s1,
                    revisions: vec![r[4].clone()]
                },
                HistoryEntry::Session {
                    session: s2,
                    revisions: vec![r[5].clone()]
                },
            ]
        );
    }

    #[test]
    fn group_history_never_merges_across_entities() {
        let s = session(100);
        let r = [
            rev(1, song(1), RevisionKind::Edit, Some(s)),
            rev(2, arrangement(2), RevisionKind::Edit, Some(s)),
            rev(3, song(1), RevisionKind::Edit, Some(s)),
        ];
        let got = group_history(&r);
        assert_eq!(got.len(), 3);
        for (entry, revision) in got.iter().zip(&r) {
            assert_eq!(
                entry,
                &HistoryEntry::Session {
                    session: s,
                    revisions: vec![revision.clone()]
                }
            );
        }
    }

    #[test]
    fn group_history_edit_without_session_is_an_event() {
        let r = [rev(1, song(1), RevisionKind::Edit, None)];
        assert_eq!(group_history(&r), vec![HistoryEntry::Event(r[0].clone())]);
        assert!(group_history(&[]).is_empty());
    }

    #[test]
    fn revision_kind_text_round_trips() {
        let all = [
            (RevisionKind::Creation, "creation"),
            (RevisionKind::Edit, "edit"),
            (RevisionKind::Import, "import"),
            (RevisionKind::Replacement, "replacement"),
            (RevisionKind::Restoration, "restoration"),
        ];
        for (kind, text) in all {
            assert_eq!(kind.as_str(), text);
            assert_eq!(RevisionKind::parse(text), Some(kind));
            assert_eq!(kind.is_standalone(), kind != RevisionKind::Edit);
        }
        assert_eq!(RevisionKind::parse("Edit"), None);
        assert_eq!(RevisionKind::parse("edit "), None);
        assert_eq!(RevisionKind::parse(""), None);
        assert_eq!(RevisionKind::parse("bogus"), None);
    }
}
