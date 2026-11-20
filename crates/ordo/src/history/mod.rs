//! History: a slideover listing an entity's sessions of edits, a nested
//! read-only comparison of one revision with the current state, and the
//! whole-entity restore that comparison offers (T2.9, Decisions 6-12).
//!
//! The store read, the row building and the frame composition all run off the
//! GPUI thread; the view shows a quiet loading line meanwhile.

pub mod compare;
mod list;
pub mod model;
pub mod slides;
pub mod time;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use domain::{
    identity::{ArrangementId, ChoirId, RevisionId, SongId},
    revisions::RevisionEntity,
    song_library::{
        SongDraft, SongWork, can_restore_arrangement_revision, can_restore_song_revision,
    },
};
use gpui::{App, AppContext as _, Context, Entity, FocusHandle, SharedString, Task, Window};
use storage::{HistoryRevision, RevisionContent};
use ui::overlay::{Overlay, OverlayHost, Toast, TwoStage};

use self::{
    compare::{ArrangementSide, CompareView, Sides},
    list::Item,
    model::SessionRow,
    slides::SlideFrames,
};
use crate::{edit_session::NavigationGuard, songs::LibraryStore};

/// What History is of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryTarget {
    /// A Song work.
    Song(SongId),
    /// One Arrangement.
    Arrangement(ArrangementId),
}

impl HistoryTarget {
    fn entity(self) -> RevisionEntity {
        match self {
            Self::Song(id) => RevisionEntity::Song(id),
            Self::Arrangement(id) => RevisionEntity::Arrangement(id),
        }
    }
}

/// What the shell lends History: a hook that runs after a restore (or its
/// Undo) stood and both surfaces closed, so the shell can rebuild whatever
/// shows the entity, and a flush that settles any pending edit before an Undo
/// writes (an Undo must never overwrite text the operator typed since).
#[derive(Clone)]
pub struct AfterRestore {
    after: AfterHook,
    flush: FlushHook,
}

type AfterHook = Rc<dyn Fn(HistoryTarget, &mut Window, &mut App)>;
type FlushHook = Rc<dyn Fn(&mut Window, &mut App) -> Task<NavigationGuard>>;

impl AfterRestore {
    /// A hook with nothing to flush.
    pub fn new(after: impl Fn(HistoryTarget, &mut Window, &mut App) + 'static) -> Self {
        Self {
            after: Rc::new(after),
            flush: Rc::new(|_, _| Task::ready(NavigationGuard::Proceed)),
        }
    }

    /// Settles pending edits first; a `Blocked` flush stops the Undo.
    pub fn flushing(
        mut self,
        flush: impl Fn(&mut Window, &mut App) -> Task<NavigationGuard> + 'static,
    ) -> Self {
        self.flush = Rc::new(flush);
        self
    }

    fn run(&self, target: HistoryTarget, window: &mut Window, cx: &mut App) {
        (self.after)(target, window, cx);
    }
}

/// The title of the History surface.
pub const TITLE: &str = "History";
/// The title of the comparison surface.
pub const COMPARE_TITLE: &str = "Compare revision";
/// The accessible name of the comparison's Back button.
pub const BACK_LABEL: &str = "Back to History";
/// The footer's Back button.
pub const BACK: &str = "Back";
/// The footer's first-stage action.
pub const RESTORE_THIS: &str = "Restore this revision";
/// The confirmation's safe answer.
pub const KEEP: &str = "Keep current";
/// The shown for the Current column when the head is under a minute old.
pub const SAVED_JUST_NOW: &str = "Saved just now";
/// "Choir A and Choir B", or "no choir": how both the restore and the archive
/// confirmation say which Choirs use an Arrangement.
pub(crate) fn choir_list(names: &[&str]) -> String {
    if names.is_empty() {
        "no choir".to_owned()
    } else {
        names.join(" and ")
    }
}
/// The refusal when nothing more specific is known.
pub const COULDNT_SAVE: &str = "Couldn't save.";
/// The toast when an Undo is held back by an edit that cannot be saved.
pub const UNDO_BLOCKED: &str = "Couldn\u{2019}t save, so Undo didn\u{2019}t run. Retry save first.";
/// The refusal when History itself could not be read.
pub const COULDNT_OPEN: &str = "Couldn't open History.";

/// Where the read stands.
enum Load {
    Loading,
    Ready,
    Failed(SharedString),
}

/// The History surface's state.
pub struct HistoryView {
    target: HistoryTarget,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after_restore: AfterRestore,
    load: Load,
    revisions: Vec<HistoryRevision>,
    sessions: Vec<SessionRow>,
    expanded: HashSet<usize>,
    /// The row the arrow keys are on; a row that vanished falls back to its session.
    active: Item,
    selected: Option<(usize, usize)>,
    focus: FocusHandle,
    frames: Entity<SlideFrames>,
    compare: Option<Entity<CompareView>>,
    /// The open comparison's footer; its confirmation restores through this view.
    stage: Option<TwoStage>,
    /// The overlay ids of this surface and of its comparison.
    history_id: usize,
    compare_id: Option<usize>,
    task: Option<Task<()>>,
    /// The restore in flight; the view owns it and drops it with itself.
    restore_task: Option<Task<()>>,
    restoring: bool,
    dismissed: bool,
}

/// Opens History for one entity on the shell's overlay layer.
pub fn open_history(
    target: HistoryTarget,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after_restore: AfterRestore,
    window: &mut Window,
    cx: &mut App,
) {
    open(target, store, host, after_restore, window, cx);
}

/// [`open_history`], handing back the view so a test can read it.
fn open(
    target: HistoryTarget,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after_restore: AfterRestore,
    window: &mut Window,
    cx: &mut App,
) -> Entity<HistoryView> {
    let meta = meta_line(target, &store, cx);
    let view = cx.new(|cx| HistoryView {
        target,
        store: store.clone(),
        host: host.clone(),
        after_restore,
        load: Load::Loading,
        revisions: Vec::new(),
        sessions: Vec::new(),
        expanded: HashSet::new(),
        active: Item::Session(0),
        selected: None,
        focus: cx.focus_handle().tab_stop(true),
        frames: cx.new(|_| SlideFrames::new()),
        compare: None,
        stage: None,
        history_id: 0,
        compare_id: None,
        task: None,
        restore_task: None,
        restoring: false,
        dismissed: false,
    });
    let body = view.clone();
    let close = view.clone();
    let overlay = Overlay::slideover(TITLE)
        .description(meta)
        .body(move |environment, window, cx| {
            body.update(cx, |view, cx| view.render_list(environment, window, cx))
        })
        .on_close(move |_, cx| {
            close.update(cx, |view, _| view.dismissed = true);
        });
    let history_id = host.open(overlay, window, cx);
    view.update(cx, |view, _| view.history_id = history_id);
    let read = store.update(cx, |store, cx| store.history(target.entity(), cx));
    let weak = view.downgrade();
    let task = window.spawn(cx, async move |cx| {
        let loaded = read.await;
        let built = match loaded {
            Ok(history) => {
                let now = ::time::OffsetDateTime::now_utc();
                let offset = time::local_offset();
                Ok(cx
                    .background_executor()
                    .spawn(async move {
                        let rows = model::rows(&history, now, offset);
                        (history, rows)
                    })
                    .await)
            }
            Err(message) => Err(message),
        };
        cx.update(|window, cx| {
            let _ = weak.update(cx, |view, cx| {
                match built {
                    Ok((history, rows)) => {
                        view.revisions = history;
                        view.sessions = rows;
                        view.expanded = HashSet::from([0]);
                        view.load = Load::Ready;
                    }
                    Err(message) => {
                        view.load = Load::Failed(if message.is_empty() {
                            COULDNT_OPEN.into()
                        } else {
                            message
                        });
                    }
                }
                // The list takes focus only once it is drawn: a refusal or a
                // loading line has no key handler, and focus on the list's
                // handle would leave Escape with nothing to dispatch from.
                if matches!(view.load, Load::Ready) && view.host.depth(cx) == 1 && !view.dismissed {
                    view.focus.focus(window, cx);
                }
                cx.notify();
            });
        })
        .ok();
    });
    view.update(cx, |view, _| view.task = Some(task));
    view
}

/// "Arrangement · {label}" or "Song · {title}".
fn meta_line(target: HistoryTarget, store: &Entity<LibraryStore>, cx: &App) -> String {
    let library = store.read(cx).library();
    match target {
        HistoryTarget::Arrangement(id) => format!(
            "Arrangement \u{b7} {}",
            library.arrangement_label(id).unwrap_or_default()
        ),
        HistoryTarget::Song(id) => format!(
            "Song \u{b7} {}",
            library
                .song(id)
                .map(|song| song.title().to_owned())
                .unwrap_or_default()
        ),
    }
}

/// The Song's authored fields as a draft, for the Overview columns.
fn song_draft(song: &SongWork) -> SongDraft {
    SongDraft {
        title: song.title().to_owned(),
        aliases: song.aliases().to_vec(),
        original_attribution: song.original_attribution().map(str::to_owned),
        provenance: song.provenance().map(str::to_owned),
        rights_notes: song.rights_notes().map(str::to_owned),
        liturgical_moments: song.liturgical_moments().iter().cloned().collect(),
        occasions: song.occasions().iter().cloned().collect(),
    }
}

impl HistoryView {
    fn song(&self) -> bool {
        matches!(self.target, HistoryTarget::Song(_))
    }

    /// Opens the nested comparison for one revision.
    fn open_compare(&mut self, si: usize, ri: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self
            .sessions
            .get(si)
            .and_then(|session| session.revisions.get(ri))
            .map(|row| row.id)
        else {
            return;
        };
        let (Some(revision), Some(head)) = (
            self.revisions
                .iter()
                .find(|revision| revision.record.id == id),
            self.revisions.last(),
        ) else {
            return;
        };
        let (revision_id, head_id) = (revision.record.id, head.record.id);
        let (revision_created, head_created) = (revision.record.created_at, head.record.created_at);
        let now = ::time::OffsetDateTime::now_utc();
        let offset = time::local_offset();
        let at = time::when(revision_created, now, offset);
        let what = self.sessions[si].revisions[ri].what.clone();
        let day = time::day(revision_created, now, offset);
        let meta = format!(
            "{day} {} \u{b7} {what}",
            time::clock(revision_created, offset)
        );
        let current_time = if now.unix_timestamp() * 1_000 - head_created.millis() < 60_000 {
            SAVED_JUST_NOW.to_owned()
        } else {
            time::stamp(head_created, now, offset)
        };
        let revision_time = time::stamp(revision_created, now, offset);
        let (sides, choirs, can_restore) = {
            let library = self.store.read(cx).library();
            let choirs: HashMap<ChoirId, String> = library
                .choirs()
                .map(|choir| (choir.id(), choir.name().to_owned()))
                .collect();
            match (self.target, &revision.content) {
                (HistoryTarget::Arrangement(id), RevisionContent::Arrangement(content)) => {
                    let Ok(arrangement) = library.arrangement(id) else {
                        return;
                    };
                    let can = library
                        .arrangement_visibility(id)
                        .is_ok_and(can_restore_arrangement_revision);
                    (
                        Sides::Arrangement {
                            current: ArrangementSide {
                                details: arrangement.details(),
                                lyrics: arrangement.lyrics(),
                                recorded: true,
                            },
                            revision: ArrangementSide {
                                details: content.details(),
                                lyrics: content.lyrics(),
                                recorded: revision.pagination_recorded,
                            },
                        },
                        choirs,
                        can,
                    )
                }
                (HistoryTarget::Song(id), RevisionContent::Song(content)) => {
                    let Ok(song) = library.song(id) else {
                        return;
                    };
                    (
                        Sides::Song {
                            current: song_draft(song),
                            revision: content.draft(),
                        },
                        choirs,
                        can_restore_song_revision(song),
                    )
                }
                _ => return,
            }
        };
        self.selected = Some((si, ri));
        let choir_text = match &sides {
            Sides::Arrangement { current, .. } => {
                let names: Vec<&str> = current
                    .details
                    .choirs
                    .iter()
                    .filter_map(|id| choirs.get(id).map(String::as_str))
                    .collect();
                choir_list(&names)
            }
            Sides::Song { .. } => String::new(),
        };
        let whole = if self.song() { "Song" } else { "Arrangement" };
        let message = if self.song() {
            "Only the Song\u{2019}s identity and metadata change; its Arrangements stay as they are. It becomes a new revision, so nothing in History is lost.".to_owned()
        } else {
            format!(
                "Used by {choir_text}. Mass-local copies and past Presentation snapshots don\u{2019}t change. It becomes a new revision, so nothing in History is lost."
            )
        };
        let compare = CompareView::new(
            sides,
            choirs,
            (current_time, revision_time),
            self.frames.clone(),
            cx,
        );
        let weak = cx.weak_entity();
        let confirmed_at = at.clone();
        let mut stage = TwoStage::new(
            format!("Read-only comparison \u{b7} restores the whole {whole}"),
            BACK,
            RESTORE_THIS,
            cx,
        )
        .confirmation(
            format!("Restore the whole {whole} to {at}?"),
            message,
            KEEP,
            format!("Restore entire {whole}"),
            move |window, cx| {
                let at = confirmed_at.clone();
                let _ = weak.update(cx, |view, cx| {
                    let Some(stage) = view.stage.clone() else {
                        return;
                    };
                    view.restore(revision_id, head_id, at, stage, window, cx);
                });
            },
        );
        if !can_restore {
            stage = stage.without_ask();
        }
        self.compare = Some(compare.clone());
        self.stage = Some(stage.clone());
        let closed = cx.weak_entity();
        let body = compare;
        let overlay = Overlay::slideover(COMPARE_TITLE)
            .wide()
            .back(BACK_LABEL)
            .description(meta)
            .body(move |environment, window, cx| {
                body.update(cx, |compare, cx| {
                    compare.render_body(environment, window, cx)
                })
            })
            .on_close(move |_, cx| {
                let _ = closed.update(cx, |view, cx| {
                    view.compare = None;
                    view.stage = None;
                    view.compare_id = None;
                    cx.notify();
                });
            })
            .two_stage(stage);
        self.compare_id = Some(self.host.open(overlay, window, cx));
        cx.notify();
    }

    /// Restores `revision` over `head`; on success closes both surfaces,
    /// posts the toast with Undo and calls the shell's hook, on refusal keeps
    /// the footer and posts nothing.
    fn restore(
        &mut self,
        revision: RevisionId,
        head: RevisionId,
        at: String,
        stage: TwoStage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.restoring {
            return;
        }
        self.restoring = true;
        let task = self
            .store
            .update(cx, |store, cx| store.restore_revision(revision, head, cx));
        let weak = cx.weak_entity();
        self.restore_task = Some(window.spawn(cx, async move |cx| {
            let outcome = task.await;
            cx.update(|window, cx| {
                let Some(view) = weak.upgrade() else {
                    return;
                };
                view.update(cx, |view, _| view.restoring = false);
                match outcome {
                    Ok(new_head) => {
                        // Closing the surfaces runs their dismiss hooks, which
                        // update this view, so this must not run inside an update.
                        let done = {
                            let view = view.read(cx);
                            Restored {
                                host: view.host.clone(),
                                surfaces: view
                                    .compare_id
                                    .into_iter()
                                    .chain([view.history_id])
                                    .collect(),
                                target: view.target,
                                store: view.store.clone(),
                                hook: view.after_restore.clone(),
                                previous: head,
                                new_head,
                                at,
                            }
                        };
                        restored(done, window, cx);
                    }
                    Err(message) => {
                        let message = if message.is_empty() {
                            SharedString::from(COULDNT_SAVE)
                        } else {
                            message
                        };
                        stage.set_refusal(Some(message), cx);
                    }
                }
            })
            .ok();
        }));
    }
}

/// Everything the finished restore needs once the view is out of the way.
struct Restored {
    host: OverlayHost,
    /// The surfaces to close, the comparison first.
    surfaces: Vec<usize>,
    target: HistoryTarget,
    store: Entity<LibraryStore>,
    hook: AfterRestore,
    previous: RevisionId,
    new_head: RevisionId,
    at: String,
}

/// The restore stood: close both surfaces, post the toast, call the hook.
fn restored(done: Restored, window: &mut Window, cx: &mut App) {
    let Restored {
        host,
        surfaces,
        target,
        store,
        hook,
        previous,
        new_head,
        at,
    } = done;
    for id in surfaces {
        host.close(id, window, cx);
    }
    let undo_host = host.clone();
    let undo_hook = hook.clone();
    host.notify(
        Toast::confirmation(format!(
            "Restored the revision from {at}. Every earlier revision is still in History."
        ))
        .undo("Undo", move |window, cx| {
            let (host, hook, store) = (undo_host.clone(), undo_hook.clone(), store.clone());
            let flush = (hook.flush)(window, cx);
            // The store queue owns the one-shot write once it is sent, so the
            // task is detached: dropping it must not cancel an Undo that began.
            window
                .spawn(cx, async move |cx| {
                    if flush.await == NavigationGuard::Blocked {
                        cx.update(|_, cx| host.notify(Toast::failure(UNDO_BLOCKED), cx))
                            .ok();
                        return;
                    }
                    let Ok(task) = cx.update(|_, cx| {
                        store.update(cx, |store, cx| {
                            store.restore_revision(previous, new_head, cx)
                        })
                    }) else {
                        return;
                    };
                    let outcome = task.await;
                    cx.update(|window, cx| match outcome {
                        Ok(_) => hook.run(target, window, cx),
                        Err(message) => host.notify(
                            Toast::failure(if message.is_empty() {
                                SharedString::from(COULDNT_SAVE)
                            } else {
                                message
                            }),
                            cx,
                        ),
                    })
                    .ok();
                })
                .detach();
        }),
        cx,
    );
    hook.run(target, window, cx);
}
