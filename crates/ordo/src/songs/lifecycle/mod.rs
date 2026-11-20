//! Lifecycle of Songs and Arrangements in the interface: the overflow menu,
//! the archive dialog, restore, and the reference-safety deletion dialog.
//!
//! Which actions a record offers is the domain's decision
//! (`song_lifecycle_actions`, `arrangement_lifecycle_actions`); this module
//! only draws them and runs the chosen one through the `LibraryStore`. Every
//! write runs off the GPUI thread, a refusal is shown as operator copy and
//! changes nothing, and each surface this module opens is closed by the id
//! the overlay host handed back.

mod archive;
mod delete;
mod menu;
#[cfg(test)]
pub(crate) mod tests;

use std::{future::Future, pin::Pin, rc::Rc};

use domain::{
    identity::{ArrangementId, SongId},
    song_library::LifecycleAction,
};
use gpui::{App, Entity, SharedString, Task, Window};
use ui::overlay::{OverlayHost, Toast};

use super::store::LibraryStore;
use crate::{edit_session::NavigationGuard, history::COULDNT_SAVE};

pub use archive::{archive_impact, confirm_archive, restore};
pub use delete::{blocked_description, erase_summary, review_deletion};
pub use menu::{action_label, overflow};

/// The record a lifecycle action is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleTarget {
    /// A Song work.
    Song(SongId),
    /// One Arrangement of a Song.
    Arrangement(ArrangementId),
}

/// What a lifecycle action did, reported to the screen that asked for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleOutcome {
    /// The record is archived.
    Archived(LifecycleTarget),
    /// The record is active again.
    Restored(LifecycleTarget),
    /// The record and its History are gone.
    Deleted(LifecycleTarget),
}

type AfterHook = Rc<dyn Fn(LifecycleOutcome, &mut Window, &mut App)>;
type FlushHook = Rc<dyn Fn(&mut Window, &mut App) -> Task<NavigationGuard>>;
type InspectHook = Rc<dyn Fn(ArrangementId, &mut Window, &mut App)>;

/// What the caller wants done around a lifecycle write: pending edits settled
/// first, the screen updated after, and an Arrangement opened from the
/// deletion dialog's blocker list.
#[derive(Clone)]
pub struct AfterLifecycle {
    after: AfterHook,
    flush: FlushHook,
    inspect: InspectHook,
}

impl AfterLifecycle {
    /// A hook with nothing to flush and nothing to inspect.
    pub fn new(after: impl Fn(LifecycleOutcome, &mut Window, &mut App) + 'static) -> Self {
        Self {
            after: Rc::new(after),
            flush: Rc::new(|_, _| Task::ready(NavigationGuard::Proceed)),
            inspect: Rc::new(|_, _, _| {}),
        }
    }

    /// Settles pending edits first; a `Blocked` flush stops the write.
    pub fn flushing(
        mut self,
        flush: impl Fn(&mut Window, &mut App) -> Task<NavigationGuard> + 'static,
    ) -> Self {
        self.flush = Rc::new(flush);
        self
    }

    /// What **Inspect** does for an Arrangement named by the deletion dialog.
    pub fn inspecting(
        mut self,
        inspect: impl Fn(ArrangementId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.inspect = Rc::new(inspect);
        self
    }

    fn run(&self, outcome: LifecycleOutcome, window: &mut Window, cx: &mut App) {
        (self.after)(outcome, window, cx);
    }
}

/// The toast when a pending edit cannot be saved first, so nothing was done.
pub const FLUSH_BLOCKED: &str = "Couldn\u{2019}t save, so nothing changed. Retry save first.";

/// Everything one lifecycle write needs, so a toast's Undo can run it again.
#[derive(Clone)]
struct Request {
    target: LifecycleTarget,
    name: SharedString,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after: AfterLifecycle,
}

impl Request {
    fn new(
        target: LifecycleTarget,
        store: Entity<LibraryStore>,
        host: OverlayHost,
        after: AfterLifecycle,
        cx: &App,
    ) -> Self {
        let name = target_name(&store, target, cx);
        Self {
            target,
            name,
            store,
            host,
            after,
        }
    }
}

/// The name a record is shown under: a Song's title or an Arrangement's label.
pub(crate) fn target_name(
    store: &Entity<LibraryStore>,
    target: LifecycleTarget,
    cx: &App,
) -> SharedString {
    let library = store.read(cx).library();
    match target {
        LifecycleTarget::Song(id) => library
            .song(id)
            .map_or_else(|_| "Song".to_owned(), |song| song.title().to_owned()),
        LifecycleTarget::Arrangement(id) => library
            .arrangement_label(id)
            .unwrap_or_else(|_| "Arrangement".to_owned()),
    }
    .into()
}

type Pending = Pin<Box<dyn Future<Output = Result<(), SharedString>>>>;

/// Starts the store write for `action`, dropping what it returns.
fn write(
    action: LifecycleAction,
    target: LifecycleTarget,
    store: &Entity<LibraryStore>,
    cx: &mut App,
) -> Pending {
    store.update(cx, |store, cx| -> Pending {
        match (action, target) {
            (LifecycleAction::Archive, LifecycleTarget::Song(id)) => {
                let task = store.archive_song(id, cx);
                Box::pin(async move { task.await.map(|_| ()) })
            }
            (LifecycleAction::Archive, LifecycleTarget::Arrangement(id)) => {
                let task = store.archive_arrangement(id, cx);
                Box::pin(async move { task.await.map(|_| ()) })
            }
            (LifecycleAction::Restore, LifecycleTarget::Song(id)) => {
                let task = store.restore_song(id, cx);
                Box::pin(async move { task.await.map(|_| ()) })
            }
            (LifecycleAction::Restore, LifecycleTarget::Arrangement(id)) => {
                let task = store.restore_arrangement(id, cx);
                Box::pin(async move { task.await.map(|_| ()) })
            }
            (LifecycleAction::DeletePermanently, LifecycleTarget::Song(id)) => {
                let task = store.delete_song(id, cx);
                Box::pin(async move { task.await.map(|_| ()) })
            }
            (LifecycleAction::DeletePermanently, LifecycleTarget::Arrangement(id)) => {
                let task = store.delete_arrangement(id, cx);
                Box::pin(async move { task.await.map(|_| ()) })
            }
        }
    })
}

/// Settles pending edits, runs the write, then reports what happened: a
/// confirmation toast and the caller's hook, or the refusal as a failure toast
/// with nothing changed.
fn perform(action: LifecycleAction, request: Request, window: &mut Window, cx: &mut App) {
    // The store queue owns the write once it is sent, so the task is
    // detached: dropping it must not cancel a write that began.
    window
        .spawn(cx, async move |cx| {
            // Asked from inside the task: the caller may be mid-update of the
            // entity the hook reads (a click handler on the shell).
            let Ok(flush) = cx.update(|window, cx| (request.after.flush)(window, cx)) else {
                return;
            };
            if flush.await == NavigationGuard::Blocked {
                cx.update(|_, cx| request.host.notify(Toast::failure(FLUSH_BLOCKED), cx))
                    .ok();
                return;
            }
            let Ok(pending) = cx.update(|_, cx| write(action, request.target, &request.store, cx))
            else {
                return;
            };
            let result = pending.await;
            cx.update(|window, cx| finish(action, request, result, window, cx))
                .ok();
        })
        .detach();
}

fn finish(
    action: LifecycleAction,
    request: Request,
    result: Result<(), SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    let Request {
        target,
        name,
        store,
        host,
        after,
    } = request.clone();
    match result {
        Ok(()) => match action {
            LifecycleAction::Archive => {
                let undo = request.clone();
                host.notify(
                    Toast::confirmation(format!(
                        "Archived {name}. Mass plans that use it keep it."
                    ))
                    .undo("Undo", move |window, cx| {
                        perform(LifecycleAction::Restore, undo.clone(), window, cx);
                    }),
                    cx,
                );
                after.run(LifecycleOutcome::Archived(target), window, cx);
            }
            LifecycleAction::Restore => {
                host.notify(Toast::confirmation(format!("Restored {name}.")), cx);
                after.run(LifecycleOutcome::Restored(target), window, cx);
            }
            LifecycleAction::DeletePermanently => {
                host.notify(
                    Toast::confirmation(format!("Deleted {name} permanently.")),
                    cx,
                );
                after.run(LifecycleOutcome::Deleted(target), window, cx);
            }
        },
        // Something started using it after the review: show the blocked state
        // again instead of a bare refusal.
        Err(message)
            if action == LifecycleAction::DeletePermanently
                && crate::songs::store::is_deletion_blocked(&message) =>
        {
            delete::review(
                Request {
                    target,
                    name,
                    store,
                    host,
                    after,
                },
                window,
                cx,
            );
        }
        Err(message) => {
            let text = if message.is_empty() {
                SharedString::from(COULDNT_SAVE)
            } else {
                message
            };
            host.notify(Toast::failure(text), cx);
        }
    }
}
