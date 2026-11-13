//! One visit to an Arrangement's editor.
//!
//! The editor entity is created when the shell opens an Arrangement and
//! dropped when it closes, so a visit owns exactly one
//! [`EditSession`] and its History session id (Decision 15). An archived
//! Arrangement or Song gets no session at all: the frame is read-only and
//! nothing can be written.

use std::rc::Rc;

use domain::{identity::ArrangementId, revisions::RevisionEntity, song_library::ArrangementLyrics};
use gpui::{App, AppContext, Context, Entity, Subscription, Task, Window};
use ui::{
    environment::UiEnvironment,
    primitives::selection::{Choice, SelectionState, Tabs},
};

use super::{
    panes::{self, Panes},
    port::LyricsPort,
};
use crate::{
    edit_session::{EditSession, EditSessionEvent, NavigationGuard, RevisionPort, SaveState},
    songs::LibraryStore,
};

/// The Arrangement editor's task tabs. Metadata is not a task: it lives in
/// Arrangement details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskTab {
    /// Writing the sections (the default).
    Lyrics,
    /// The performance order, read-only until a later task.
    Sequence,
}

impl TaskTab {
    /// Every tab, in display order.
    pub const ALL: [TaskTab; 2] = [TaskTab::Lyrics, TaskTab::Sequence];

    /// The tab's visible name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Lyrics => "Lyrics",
            Self::Sequence => "Sequence",
        }
    }
}

/// The editor of one Arrangement for one visit.
pub struct ArrangementEditor {
    arrangement: ArrangementId,
    store: Entity<LibraryStore>,
    /// `None` when the Arrangement or its Song is archived, or gone.
    session: Option<Entity<EditSession<ArrangementLyrics>>>,
    tabs: SelectionState,
    /// Redraw on every session event; dropped with the editor.
    _session_events: Option<Subscription>,
}

impl ArrangementEditor {
    /// An editor over `arrangement`, with a session when it is editable.
    pub fn new(
        arrangement: ArrangementId,
        store: Entity<LibraryStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let tabs = SelectionState::new(
            TaskTab::ALL
                .iter()
                .map(|tab| Choice::new(tab.label()))
                .collect(),
            Some(0),
            cx,
        );
        let committed = editable_lyrics(&store, arrangement, cx);
        let session = committed.map(|committed| {
            let port: Rc<dyn RevisionPort<ArrangementLyrics>> = Rc::new(LyricsPort::new(
                store.downgrade(),
                arrangement,
                cx.to_async(),
            ));
            cx.new(|cx| {
                EditSession::new(
                    RevisionEntity::Arrangement(arrangement),
                    committed,
                    port,
                    cx,
                )
            })
        });
        let events = session
            .as_ref()
            .map(|session| cx.subscribe(session, |_, _, _: &EditSessionEvent, cx| cx.notify()));
        Self {
            arrangement,
            store,
            session,
            tabs,
            _session_events: events,
        }
    }

    /// The Arrangement being edited.
    pub fn arrangement(&self) -> ArrangementId {
        self.arrangement
    }

    /// The edit session, when the Arrangement is editable. A Lyrics view
    /// issues `text_edited`, `structural` and `focus_left` on it, and reloads
    /// its fields only on [`EditSessionEvent::ContentReplaced`].
    pub fn session(&self) -> Option<&Entity<EditSession<ArrangementLyrics>>> {
        self.session.as_ref()
    }

    /// Whether the frame is read-only (no session exists).
    pub fn is_read_only(&self) -> bool {
        self.session.is_none()
    }

    /// The header's save state; `None` for a read-only frame.
    pub fn save_state(&self, cx: &App) -> Option<SaveState> {
        self.session
            .as_ref()
            .map(|session| session.read(cx).save_state())
    }

    /// The selected task tab.
    pub fn tab(&self, cx: &App) -> TaskTab {
        match self.tabs.selected(cx) {
            Some(1) => TaskTab::Sequence,
            _ => TaskTab::Lyrics,
        }
    }

    /// The task tabs element; the caller supplies the surface it sits on and
    /// the redraw seam for what the tab shows.
    pub fn tabs(
        &self,
        surface: u32,
        environment: UiEnvironment,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Tabs {
        Tabs::new("editor-tabs", "Arrangement tasks", &self.tabs, environment)
            .on_surface(surface)
            .on_select(move |_, window, cx| on_select(window, cx))
    }

    /// The working content: the session's, or the stored content when the
    /// frame is read-only. `None` when the Arrangement is gone.
    pub fn content(&self, cx: &App) -> Option<ArrangementLyrics> {
        match &self.session {
            Some(session) => Some(session.read(cx).content().clone()),
            None => self
                .store
                .read(cx)
                .library()
                .arrangement(self.arrangement)
                .ok()
                .map(|arrangement| arrangement.lyrics()),
        }
    }

    /// The three pane slots for the selected tab. `compact` omits the
    /// Sections and Inspector slots.
    pub fn panes(&self, compact: bool, environment: UiEnvironment, cx: &App) -> Panes {
        panes::build(
            self.tab(cx),
            self.content(cx).as_ref(),
            self.is_read_only(),
            compact,
            environment,
        )
    }

    /// Flushes pending text and resolves once everything is durable
    /// (`Proceed`) or a commit failed (`Blocked`). A read-only frame
    /// proceeds at once.
    pub fn request_navigation(&self, cx: &mut App) -> Task<NavigationGuard> {
        match &self.session {
            Some(session) => session.update(cx, |session, cx| session.request_navigation(cx)),
            None => Task::ready(NavigationGuard::Proceed),
        }
    }

    /// Retries a failed save.
    pub fn retry_save(&self, cx: &mut App) {
        if let Some(session) = &self.session {
            session.update(cx, |session, cx| session.retry_save(cx));
        }
    }

    /// Departure after a `Proceed`.
    pub fn leave(&self, cx: &mut App) {
        if let Some(session) = &self.session {
            session.update(cx, |session, cx| session.leave(cx));
        }
    }

    /// Departure after a `Blocked`, dropping what could not be saved.
    pub fn leave_without_saving(&self, cx: &mut App) {
        if let Some(session) = &self.session {
            session.update(cx, |session, cx| session.leave_without_saving(cx));
        }
    }

    /// Editor Undo (the registry's `Ctrl+Z`).
    pub fn undo(&self, cx: &mut App) {
        if let Some(session) = &self.session {
            session.update(cx, |session, cx| {
                session.undo(cx);
            });
        }
    }

    /// Editor Redo (the registry's `Ctrl+Y`).
    pub fn redo(&self, cx: &mut App) {
        if let Some(session) = &self.session {
            session.update(cx, |session, cx| {
                session.redo(cx);
            });
        }
    }
}

/// The stored lyrics when the Arrangement and its Song are both active.
fn editable_lyrics(
    store: &Entity<LibraryStore>,
    arrangement: ArrangementId,
    cx: &App,
) -> Option<ArrangementLyrics> {
    let library = store.read(cx).library();
    let found = library.arrangement(arrangement).ok()?;
    let song_active = library
        .song(found.song())
        .is_ok_and(|song| !song.is_archived());
    (!found.is_archived() && song_active).then(|| found.lyrics())
}
