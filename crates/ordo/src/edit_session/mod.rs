//! Coalesced autosave, editor Undo and save states for one editable entity
//! (T2.2; Decisions 138 to 141).
//!
//! [`EditSession`] is a GPUI entity that owns the working copy of an
//! entity's content. It never persists anything itself: every committed point
//! goes through a [`RevisionPort`] future that is awaited inside `cx.spawn`,
//! so the GPUI thread never blocks on storage. The pure mechanics (text
//! coalescing, the Undo stack, History sessions) live in `domain::revisions`
//! and are reused here unchanged.
//!
//! Rules a caller can rely on:
//!
//! - Commits are serialized: at most one is in flight. Typing during a commit
//!   stays pending and commits after it settles; it is never dropped.
//! - A commit whose content equals the last committed content is skipped.
//! - While a save is in flight, or after one failed, Undo, Redo and
//!   structural changes are refused (they return `false`). Typing is still
//!   accepted and stays pending. A failed save is not retried automatically;
//!   [`EditSession::retry_save`] retries it.
//! - Undo and Redo are `Edit` revisions in the current History session. If
//!   text is pending when Undo is asked for, that text is committed first and
//!   Undo goes back from it. Redo with changed pending text is refused,
//!   because that text becomes a new point and ends the redo chain.
//! - Time comes from the GPUI executor clock, so tests that advance it see
//!   the same timing the operator does.
//!
//! A focused text field consumes Ctrl+Z and Ctrl+Y itself (native Undo), so
//! the session's Undo is reachable only when no text field has focus.

pub mod harness;
pub mod port;
#[cfg(test)]
mod tests;

use std::{collections::VecDeque, rc::Rc, time::Duration, time::Instant};

use domain::{
    identity::{EntityId, HistorySessionId},
    revisions::{
        EditGeneration, EditPoint, RevisionEntity, RevisionKind, SessionClock, TextCoalescer,
        UndoStack,
    },
};
use gpui::{Context, EventEmitter, Task};

pub use harness::init;
pub use port::{CommitPoint, RevisionPort, SaveFailure};

/// Where the working copy stands relative to durable storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveState {
    /// Everything accepted so far is durable (typing may still be settling).
    Saved,
    /// A commit is in flight or queued.
    Saving,
    /// A commit failed; the content is retained and nothing retries by itself.
    CouldntSave,
}

impl SaveState {
    /// The header copy for this state.
    pub fn label(self) -> &'static str {
        match self {
            Self::Saved => "Saved",
            Self::Saving => "Saving\u{2026}",
            Self::CouldntSave => "Couldn't save",
        }
    }
}

/// What the caller decides when leaving the entity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationGuard {
    /// Everything is durable; leave.
    Proceed,
    /// The flush failed. Offer Retry save, Keep editing and Leave without saving.
    Blocked,
}

/// What an [`EditSession`] tells its views.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditSessionEvent {
    /// [`EditSession::save_state`] changed.
    StateChanged,
    /// Undo or Redo replaced the content; views reload their fields from
    /// [`EditSession::content`] (with `set_value`, never `replace_all`).
    ContentReplaced,
}

impl<C: Clone + Eq + 'static> EventEmitter<EditSessionEvent> for EditSession<C> {}

#[derive(Clone)]
enum Queued<C> {
    /// A snapshot of pending text.
    Text {
        content: C,
        generation: EditGeneration,
    },
    /// A structural change or semantic replacement, already applied to the
    /// working copy; `content` is the snapshot to commit.
    Structural {
        content: C,
        kind: RevisionKind,
    },
    Undo,
    Redo,
}

/// The working copy of one entity with coalesced autosave and editor Undo.
pub struct EditSession<C: Clone + Eq + 'static> {
    entity: RevisionEntity,
    content: C,
    committed: C,
    port: Rc<dyn RevisionPort<C>>,
    coalescer: TextCoalescer,
    undo: UndoStack<C>,
    clock: SessionClock,
    epoch: Instant,
    queue: VecDeque<Queued<C>>,
    in_flight: bool,
    failed: bool,
    last_state: SaveState,
    timer: Option<Task<()>>,
    waiters: Vec<async_channel::Sender<NavigationGuard>>,
}

impl<C: Clone + Eq + 'static> EditSession<C> {
    /// A session over `entity` whose last durable content is `committed`.
    /// No History session is open: the first edit starts one.
    pub fn new(
        entity: RevisionEntity,
        committed: C,
        port: Rc<dyn RevisionPort<C>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            entity,
            content: committed.clone(),
            undo: UndoStack::new(committed.clone()),
            committed,
            port,
            coalescer: TextCoalescer::new(),
            clock: SessionClock::new(),
            epoch: cx.background_executor().now(),
            queue: VecDeque::new(),
            in_flight: false,
            failed: false,
            last_state: SaveState::Saved,
            timer: None,
            waiters: Vec::new(),
        }
    }

    /// The latest content, pending or committed.
    pub fn content(&self) -> &C {
        &self.content
    }

    /// The last durably committed content.
    pub fn committed(&self) -> &C {
        &self.committed
    }

    /// Where the working copy stands relative to durable storage.
    pub fn save_state(&self) -> SaveState {
        if self.failed {
            SaveState::CouldntSave
        } else if self.in_flight || !self.queue.is_empty() {
            SaveState::Saving
        } else {
            SaveState::Saved
        }
    }

    /// Whether Undo would do something now: nothing saving, failed or
    /// pending, and an older committed state exists.
    pub fn can_undo(&self) -> bool {
        !self.busy() && !self.coalescer.is_pending() && self.undo.can_undo()
    }

    /// Whether Redo would do something now; same conditions as [`Self::can_undo`].
    pub fn can_redo(&self) -> bool {
        !self.busy() && !self.coalescer.is_pending() && self.undo.can_redo()
    }

    /// A text field changed; `apply` updates the working content. Starts or
    /// extends the 1 s coalescing timer. Always accepted, even while saving
    /// or after a failed save.
    pub fn text_edited(&mut self, apply: impl FnOnce(&mut C), cx: &mut Context<Self>) {
        apply(&mut self.content);
        let now = self.now_ms(cx);
        self.coalescer.edited(now);
        self.after_change(cx);
    }

    /// Focus left an edited field: commit any pending text edit now.
    pub fn focus_left(&mut self, cx: &mut Context<Self>) {
        if self.coalescer.focus_left() {
            self.pump(cx);
        }
    }

    /// A structural add, remove or reorder, or a semantic replacement.
    /// Pending text is flushed first as its own point, then this commits as
    /// another. Returns `true` when the change was accepted; `false` when it
    /// was refused (a save is in flight or failed) or changed nothing.
    /// [`EditPoint::TextSession`] is treated as `Structural`.
    pub fn structural(
        &mut self,
        point: EditPoint,
        apply: impl FnOnce(&mut C),
        cx: &mut Context<Self>,
    ) -> bool {
        if self.busy() {
            return false;
        }
        self.queue_pending_text();
        let before = self.content.clone();
        apply(&mut self.content);
        let changed = self.content != before;
        if changed {
            let kind = match point {
                EditPoint::SemanticReplacement => RevisionKind::Replacement,
                EditPoint::TextSession | EditPoint::Structural => RevisionKind::Edit,
            };
            self.queue.push_back(Queued::Structural {
                content: self.content.clone(),
                kind,
            });
        }
        self.pump(cx);
        changed
    }

    /// Editor Undo: commits the previous committed state as an `Edit` in the
    /// current History session, and moves the stack only once that commit
    /// succeeded. Pending text is committed first. Returns `false` when
    /// refused (saving or failed) or there is nothing to undo.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        self.history_step(true, cx)
    }

    /// Editor Redo; mirror of [`Self::undo`]. Refused while changed text is
    /// pending, because that text is a new point and ends the redo chain.
    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        self.history_step(false, cx)
    }

    /// Retries after [`SaveState::CouldntSave`]. If only a text snapshot is
    /// queued, it is refreshed to the latest content first.
    pub fn retry_save(&mut self, cx: &mut Context<Self>) {
        if !self.failed {
            return;
        }
        self.failed = false;
        if self.queue.len() == 1
            && let (
                Some(Queued::Text {
                    content,
                    generation,
                }),
                Some(latest),
            ) = (self.queue.front_mut(), self.coalescer.generation())
        {
            *content = self.content.clone();
            *generation = latest;
        }
        self.pump(cx);
    }

    /// Leaving the entity (navigation, window close, quit): flushes pending
    /// text and resolves to `Proceed` once everything is durable, or `Blocked`
    /// when a commit failed.
    pub fn request_navigation(&mut self, cx: &mut Context<Self>) -> Task<NavigationGuard> {
        let (tx, rx) = async_channel::bounded(1);
        self.waiters.push(tx);
        self.coalescer.focus_left();
        self.pump(cx);
        cx.spawn(async move |_, _| rx.recv().await.unwrap_or(NavigationGuard::Blocked))
    }

    /// Leave without saving (only after `Blocked`): drops pending content,
    /// clears Undo and ends the History session. The content reverts to the
    /// last committed state ([`EditSessionEvent::ContentReplaced`]).
    pub fn leave_without_saving(&mut self, cx: &mut Context<Self>) {
        self.queue.clear();
        self.failed = false;
        if let Some(generation) = self.coalescer.generation() {
            self.coalescer.settle(generation);
        }
        self.content = self.committed.clone();
        self.undo.clear();
        self.clock.end();
        self.after_change(cx);
        cx.emit(EditSessionEvent::ContentReplaced);
    }

    /// Departure after `Proceed`: clears the Undo stack and ends the History
    /// session, so the next edit starts a new one.
    pub fn leave(&mut self, cx: &mut Context<Self>) {
        self.undo.clear();
        self.clock.end();
        cx.notify();
    }

    fn busy(&self) -> bool {
        self.in_flight || self.failed || !self.queue.is_empty()
    }

    fn now_ms(&self, cx: &Context<Self>) -> i64 {
        let elapsed = cx
            .background_executor()
            .now()
            .saturating_duration_since(self.epoch);
        i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
    }

    /// Queues a snapshot of pending text, or discards the pending edit when
    /// typing returned the content to the committed state.
    fn queue_pending_text(&mut self) {
        let Some(generation) = self.coalescer.generation() else {
            return;
        };
        if self.content == self.committed {
            self.coalescer.settle(generation);
        } else {
            self.queue.push_back(Queued::Text {
                content: self.content.clone(),
                generation,
            });
        }
    }

    fn history_step(&mut self, undo: bool, cx: &mut Context<Self>) -> bool {
        if self.busy() {
            return false;
        }
        let mut flushed = false;
        if let Some(generation) = self.coalescer.generation() {
            if self.content == self.committed {
                self.coalescer.settle(generation);
            } else if undo {
                self.queue_pending_text();
                flushed = true;
            } else {
                return false;
            }
        }
        let available = if undo {
            self.undo.can_undo()
        } else {
            self.undo.can_redo()
        };
        if !flushed && !available {
            return false;
        }
        self.queue
            .push_back(if undo { Queued::Undo } else { Queued::Redo });
        self.pump(cx);
        true
    }

    /// Starts the next commit when none is in flight, then refreshes the
    /// timer, the save state and any navigation waiters.
    fn pump(&mut self, cx: &mut Context<Self>) {
        if !self.in_flight && !self.failed {
            self.start_next(cx);
        }
        self.after_change(cx);
    }

    fn start_next(&mut self, cx: &mut Context<Self>) {
        loop {
            if self.queue.is_empty() {
                let now = self.now_ms(cx);
                if !self.coalescer.is_due(now) {
                    return;
                }
                self.queue_pending_text();
                if self.queue.is_empty() {
                    return;
                }
            }
            let Some(front) = self.queue.front().cloned() else {
                return;
            };
            let (content, kind) = match front {
                Queued::Text {
                    content,
                    generation,
                } => {
                    if content == self.committed {
                        self.queue.pop_front();
                        self.coalescer.settle(generation);
                        continue;
                    }
                    (content, RevisionKind::Edit)
                }
                Queued::Structural { content, kind } => {
                    if content == self.committed {
                        self.queue.pop_front();
                        continue;
                    }
                    (content, kind)
                }
                Queued::Undo | Queued::Redo => {
                    // Typing since the request wins over the Undo.
                    let target = if self.coalescer.is_pending() {
                        None
                    } else if matches!(front, Queued::Undo) {
                        self.undo.peek_undo().cloned()
                    } else {
                        self.undo.peek_redo().cloned()
                    };
                    match target {
                        Some(target) => (target, RevisionKind::Edit),
                        None => {
                            self.queue.pop_front();
                            continue;
                        }
                    }
                }
            };
            let session = (kind == RevisionKind::Edit).then(|| {
                let now = self.now_ms(cx);
                self.clock.on_edit(self.entity, now, fresh_session())
            });
            let future = self.port.commit(content, CommitPoint { kind, session });
            self.in_flight = true;
            cx.spawn(async move |this, cx| {
                let result = future.await;
                this.update(cx, |session, cx| session.commit_finished(result, cx))
                    .ok();
            })
            .detach();
            return;
        }
    }

    fn commit_finished(&mut self, result: Result<(), SaveFailure>, cx: &mut Context<Self>) {
        self.in_flight = false;
        if result.is_err() {
            // The front entry stays queued for `retry_save`.
            self.failed = true;
            self.pump(cx);
            return;
        }
        match self.queue.pop_front() {
            Some(Queued::Text {
                content,
                generation,
            }) => {
                self.coalescer.settle(generation);
                self.record_committed(content);
            }
            Some(Queued::Structural { content, .. }) => self.record_committed(content),
            Some(entry @ (Queued::Undo | Queued::Redo)) => {
                let target = if matches!(entry, Queued::Undo) {
                    self.undo.undo()
                } else {
                    self.undo.redo()
                };
                if let Some(target) = target {
                    self.committed = target.clone();
                    // Typing that arrived during the commit supersedes the
                    // restored state in the working copy.
                    if !self.coalescer.is_pending() {
                        self.content = target;
                        cx.emit(EditSessionEvent::ContentReplaced);
                    }
                }
            }
            None => {}
        }
        self.pump(cx);
    }

    fn record_committed(&mut self, content: C) {
        self.undo.commit(content.clone());
        self.committed = content;
    }

    /// Reschedules the idle timer, refreshes the save state and answers
    /// navigation waiters.
    fn after_change(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        if !self.failed && !self.in_flight && self.queue.is_empty() {
            let now = self.now_ms(cx);
            if let Some(due) = self.coalescer.due_in(now) {
                let wait = Duration::from_millis(u64::try_from(due).unwrap_or(0));
                self.timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    this.update(cx, |session, cx| session.pump(cx)).ok();
                }));
            }
        }
        let state = self.save_state();
        if state != self.last_state {
            self.last_state = state;
            cx.emit(EditSessionEvent::StateChanged);
        }
        if !self.waiters.is_empty() {
            let verdict = if self.failed {
                Some(NavigationGuard::Blocked)
            } else if !self.in_flight && self.queue.is_empty() && !self.coalescer.is_pending() {
                Some(NavigationGuard::Proceed)
            } else {
                None
            };
            if let Some(verdict) = verdict {
                for waiter in self.waiters.drain(..) {
                    waiter.try_send(verdict).ok();
                }
            }
        }
        cx.notify();
    }
}

/// A fresh History-session id from OS randomness.
fn fresh_session() -> HistorySessionId {
    HistorySessionId::from_entity(EntityId::from_random_bytes(
        *uuid::Uuid::new_v4().as_bytes(),
    ))
}
