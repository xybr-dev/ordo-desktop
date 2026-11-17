//! The Songs workspace: the permanent master-detail screen of Library → Songs.
//!
//! One [`SongsWorkspace`] entity lives as long as the shell, so the query, the
//! filters, the selection and the list's scroll survive leaving the screen for
//! an Arrangement and coming back. The shell calls [`SongsWorkspace::render_list`]
//! and [`SongsWorkspace::render_detail`] for the two panes it hosts and keeps
//! the chrome; everything inside the panes is drawn here.
//!
//! Search is the pure `domain::song_library::search` over the hydrated
//! library. It runs synchronously when the query, a filter, the archived
//! toggle or the library changes, never per frame, and never writes.

pub(crate) mod detail;
mod filters;
mod list;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use domain::{
    identity::{ArrangementId, SongId},
    song_library::{
        AfterArchive, LibraryQuery, LifecycleAction, SongHit, search, selection_after_archive,
    },
};
use gpui::{AnyElement, App, Context, Entity, Pixels, Point, Subscription, Window};
use ui::{
    environment::UiEnvironment,
    overlay::OverlayHost,
    primitives::field::{FieldEvent, FieldState},
    scroll::Scroll,
};

use crate::history::{AfterRestore, HistoryTarget, open_history};

use super::{
    arrangement_details::{self, ArrangementDetailsPanel},
    lifecycle::{
        AfterLifecycle, LifecycleOutcome, LifecycleTarget, confirm_archive, restore,
        review_deletion,
    },
    new_song::{NewSongPanel, Outcome},
    store::{LibraryEvent, LibraryState, LibraryStore},
};
pub use filters::FilterKind;

/// How the detail pane asks the shell to open an Arrangement's focused view.
pub type OpenArrangement = Rc<dyn Fn(ArrangementId, &mut Window, &mut App)>;

/// The list as it was shown when an archive or deletion began: what the
/// selection that follows is decided from, because the store's own refresh
/// has already dropped the Song by the time the write is answered.
struct Removal {
    visible: Vec<(SongId, bool)>,
    include_archived: bool,
}

/// Query, filters, selection and scroll of Library → Songs.
pub struct SongsWorkspace {
    store: Entity<LibraryStore>,
    search: FieldState,
    query: LibraryQuery,
    hits: Vec<SongHit>,
    selected: Option<SongId>,
    list_scroll: Scroll,
    detail_scroll: Scroll,
    /// The detail header's History button, so closing History hands focus
    /// back to it.
    history_focus: gpui::FocusHandle,
    /// The list row that follows an archive or deletion, carried by the row of
    /// `focus_song`.
    row_focus: gpui::FocusHandle,
    focus_song: Option<SongId>,
    /// The first action of the list's empty or no-match state.
    empty_focus: gpui::FocusHandle,
    /// The first control of the Arrangement list (its first row, or the Add
    /// first Arrangement button), where focus lands after an Arrangement is
    /// archived or deleted.
    arrangement_focus: gpui::FocusHandle,
    removal: Option<Removal>,
    /// Set until the detail pane has drawn the Arrangement list that
    /// `arrangement_focus` belongs to.
    focus_arrangements_pending: bool,
    /// Which filter's option panel is open under the filter row.
    open_filter: Option<FilterKind>,
    /// The list offset to put back the next time the list is drawn, saved when
    /// an Arrangement was opened. The scroll handle keeps its offset while the
    /// list is unmounted, but the list re-measures on return, so the offset is
    /// restored explicitly rather than trusted to survive.
    restore_scroll: Option<Point<Pixels>>,
    /// The shell's overlay layer, which hosts the New song panel.
    overlays: Option<OverlayHost>,
    /// The New song panel while it is open.
    new_song: Option<Entity<NewSongPanel>>,
    /// The New Arrangement panel while it is open.
    new_arrangement: Option<Entity<ArrangementDetailsPanel>>,
    _subscriptions: Vec<Subscription>,
}

impl SongsWorkspace {
    /// A workspace over `store`, searching as the operator types.
    pub fn new(store: Entity<LibraryStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = FieldState::new(window, cx);
        let weak = cx.weak_entity();
        let typed = search.subscribe(cx, move |event, cx| {
            // Only a change of the text searches; losing focus does not.
            if event == FieldEvent::Changed {
                let _ = weak.update(cx, |workspace, cx| workspace.refresh(cx));
            }
        });
        let changed = cx.subscribe(&store, |workspace, _, _: &LibraryEvent, cx| {
            workspace.refresh(cx);
        });
        let mut workspace = Self {
            store,
            search,
            query: LibraryQuery::default(),
            hits: Vec::new(),
            selected: None,
            list_scroll: Scroll::new(),
            detail_scroll: Scroll::new(),
            history_focus: cx.focus_handle(),
            row_focus: cx.focus_handle(),
            focus_song: None,
            empty_focus: cx.focus_handle(),
            arrangement_focus: cx.focus_handle(),
            removal: None,
            focus_arrangements_pending: false,
            open_filter: None,
            restore_scroll: None,
            overlays: None,
            new_song: None,
            new_arrangement: None,
            _subscriptions: vec![typed, changed],
        };
        workspace.refresh(cx);
        workspace
    }

    /// Hands the workspace the shell's overlay layer, which hosts the New song
    /// panel. Without it the panel cannot open.
    pub fn set_overlays(&mut self, overlays: OverlayHost) {
        self.overlays = Some(overlays);
    }

    /// The New song panel, while it is open.
    pub fn new_song(&self) -> Option<&Entity<NewSongPanel>> {
        self.new_song.as_ref()
    }

    /// The New Arrangement panel, while it is open.
    pub fn new_arrangement(&self) -> Option<&Entity<ArrangementDetailsPanel>> {
        self.new_arrangement.as_ref()
    }

    /// How many temporary surfaces the shell's overlay layer holds open.
    pub fn overlay_depth(&self, cx: &App) -> usize {
        self.overlays.as_ref().map_or(0, |host| host.depth(cx))
    }

    /// The title of the topmost open surface, if any.
    pub fn overlay_top(&self, cx: &App) -> Option<gpui::SharedString> {
        self.overlays.as_ref().and_then(|host| host.top(cx))
    }

    /// Opens the New song panel over the workspace. The query, filters,
    /// selection and scroll stay exactly as they are underneath it. Does
    /// nothing while the library is not readable or a panel is already open.
    pub fn open_new_song(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.overlays.clone() else {
            return;
        };
        if self.new_song.is_some() || *self.store.read(cx).state() != LibraryState::Ready {
            return;
        }
        let weak = cx.weak_entity();
        let report = {
            let weak = weak.clone();
            let host = host.clone();
            Rc::new(move |outcome: Outcome, window: &mut Window, cx: &mut App| {
                let preview = match outcome {
                    Outcome::UseExisting(song) => weak
                        .update(cx, |workspace, cx| {
                            workspace
                                .store
                                .read(cx)
                                .library()
                                .song(song)
                                .ok()
                                .map(|work| gpui::SharedString::from(work.title().to_owned()))
                        })
                        .ok()
                        .flatten(),
                    Outcome::Created(_) => None,
                };
                let _ = weak.update(cx, |workspace, cx| workspace.apply(outcome, window, cx));
                // Close only the panel's own surfaces: a dialog another part of
                // the app stacked above them is not the panel's to dismiss.
                while host
                    .top(cx)
                    .is_some_and(|top| top == "New song" || Some(&top) == preview.as_ref())
                {
                    host.close_top(window, cx);
                }
            })
        };
        let closed = Rc::new(move |cx: &mut App| {
            let _ = weak.update(cx, |workspace, cx| {
                workspace.new_song = None;
                cx.notify();
            });
        });
        let panel = NewSongPanel::open(self.store.clone(), host, report, closed, window, cx);
        self.new_song = Some(panel);
        cx.notify();
    }

    /// Opens the New Arrangement panel for `song` over the workspace, leaving
    /// the query, filters, selection and scroll as they are. A created
    /// Arrangement is opened through `open`. Does nothing while the library is
    /// not readable, the Song is archived or gone, or a panel is already open.
    pub fn open_new_arrangement(
        &mut self,
        song: SongId,
        open: OpenArrangement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.overlays.clone() else {
            return;
        };
        let addable = self
            .store
            .read(cx)
            .library()
            .song(song)
            .is_ok_and(|work| !work.is_archived());
        if self.new_arrangement.is_some()
            || !addable
            || *self.store.read(cx).state() != LibraryState::Ready
        {
            return;
        }
        let weak = cx.weak_entity();
        let report = Rc::new(
            move |outcome: arrangement_details::Outcome, window: &mut Window, cx: &mut App| {
                if let arrangement_details::Outcome::Created(id)
                | arrangement_details::Outcome::Duplicated(id) = outcome
                {
                    open(id, window, cx);
                }
            },
        );
        let closed = Rc::new(move |cx: &mut App| {
            let _ = weak.update(cx, |workspace, cx| {
                workspace.new_arrangement = None;
                cx.notify();
            });
        });
        let panel = ArrangementDetailsPanel::open_new(
            self.store.clone(),
            song,
            host,
            report,
            closed,
            window,
            cx,
        );
        self.new_arrangement = Some(panel);
        cx.notify();
    }

    /// The detail header's History button focus handle.
    #[cfg(test)]
    pub(crate) fn history_focus(&self) -> &gpui::FocusHandle {
        &self.history_focus
    }

    /// Opens History for `song` as a slideover over the workspace, leaving
    /// the query, filters, selection and scroll as they are. Song detail has
    /// no edit session, so nothing is flushed first. After a restore the
    /// hits and the detail are recomputed from the store. Does nothing while
    /// the library is not readable, the Song is gone or another surface is
    /// open.
    pub fn open_song_history(&mut self, song: SongId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.overlays.clone() else {
            return;
        };
        let known = self.store.read(cx).library().song(song).is_ok();
        if !known || host.depth(cx) > 0 || *self.store.read(cx).state() != LibraryState::Ready {
            return;
        }
        let weak = cx.weak_entity();
        let after = AfterRestore::new(move |_, _, cx| {
            let _ = weak.update(cx, |workspace, cx| workspace.refresh(cx));
        });
        open_history(
            HistoryTarget::Song(song),
            self.store.clone(),
            host,
            after,
            window,
            cx,
        );
    }

    /// Runs a lifecycle action on the selected Song: Archive asks first,
    /// Restore is immediate, Delete permanently reviews what still uses it.
    /// Does nothing while the library is not readable or another surface is
    /// open.
    pub(crate) fn run_song_lifecycle(
        &mut self,
        song: SongId,
        action: LifecycleAction,
        open: OpenArrangement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.overlays.clone() else {
            return;
        };
        if host.depth(cx) > 0 || *self.store.read(cx).state() != LibraryState::Ready {
            return;
        }
        let library = self.store.read(cx).library();
        let visible = self
            .hits
            .iter()
            .map(|hit| {
                let active = library.song(hit.song).is_ok_and(|work| !work.is_archived());
                (hit.song, active)
            })
            .collect();
        self.removal = Some(Removal {
            visible,
            include_archived: self.query.include_archived,
        });
        let weak = cx.weak_entity();
        let inspect_open = open.clone();
        let after = AfterLifecycle::new(move |outcome, window, cx| {
            let _ = weak.update(cx, |workspace, cx| {
                workspace.after_lifecycle(outcome, window, cx);
            });
        })
        .inspecting(move |id, window, cx| inspect_open(id, window, cx));
        let target = LifecycleTarget::Song(song);
        let store = self.store.clone();
        match action {
            LifecycleAction::Archive => confirm_archive(target, store, host, after, window, cx),
            LifecycleAction::Restore => restore(target, store, host, after, window, cx),
            LifecycleAction::DeletePermanently => {
                review_deletion(target, store, host, after, window, cx);
            }
        }
    }

    /// Settles the list and the selection after a lifecycle write answered.
    fn after_lifecycle(
        &mut self,
        outcome: LifecycleOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            LifecycleOutcome::Archived(LifecycleTarget::Song(song))
            | LifecycleOutcome::Deleted(LifecycleTarget::Song(song)) => {
                let deleted = matches!(outcome, LifecycleOutcome::Deleted(_));
                let removal = self.removal.take();
                self.refresh(cx);
                let (visible, include) = removal
                    .map(|removal| (removal.visible, removal.include_archived))
                    .unwrap_or_default();
                match selection_after_archive(&visible, song, include && !deleted) {
                    AfterArchive::Select(next) => {
                        self.select(next, cx);
                        self.focus_song = Some(next);
                        self.row_focus.focus(window, cx);
                    }
                    AfterArchive::Keep => {
                        if !deleted {
                            self.select(song, cx);
                        }
                    }
                    AfterArchive::EmptyState => {
                        self.selected = None;
                        self.empty_focus.focus(window, cx);
                        cx.notify();
                    }
                }
            }
            LifecycleOutcome::Restored(LifecycleTarget::Song(song)) => {
                self.removal = None;
                self.reveal(song, window, cx);
            }
            _ => self.refresh(cx),
        }
    }

    /// The list row that carries focus after an archive or deletion.
    #[cfg(test)]
    pub(crate) fn row_focus(&self) -> &gpui::FocusHandle {
        &self.row_focus
    }

    /// The Song whose row carries [`Self::row_focus`].
    #[cfg(test)]
    pub(crate) fn focus_song(&self) -> Option<SongId> {
        self.focus_song
    }

    /// The first action of the list's empty state.
    #[cfg(test)]
    pub(crate) fn empty_focus(&self) -> &gpui::FocusHandle {
        &self.empty_focus
    }

    /// The first control of the Arrangement list.
    #[cfg(test)]
    pub(crate) fn arrangement_focus(&self) -> &gpui::FocusHandle {
        &self.arrangement_focus
    }

    /// Puts focus on the Arrangement list of the selected Song, once the
    /// detail pane draws it. The shell calls this after it closes an editor
    /// whose Arrangement was archived or deleted.
    pub fn focus_arrangements(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        self.focus_arrangements_pending = true;
        cx.notify();
    }

    /// Carries out what the panel decided: both outcomes select a Song, and a
    /// Song that is archived is only visible with Include archived on.
    fn apply(&mut self, outcome: Outcome, window: &mut Window, cx: &mut Context<Self>) {
        let song = match outcome {
            Outcome::Created(song) | Outcome::UseExisting(song) => song,
        };
        self.reveal(song, window, cx);
    }

    /// Selects `song`, making it visible: an archived Song turns Include
    /// archived on, and a Song the search or filters hide makes them give way.
    fn reveal(&mut self, song: SongId, window: &mut Window, cx: &mut Context<Self>) {
        let archived = self
            .store
            .read(cx)
            .library()
            .song(song)
            .is_ok_and(|work| work.is_archived());
        if archived {
            self.set_include_archived(true, cx);
        } else {
            self.refresh(cx);
        }
        // A Song the search or filters hide would be dropped by the next
        // refresh, so the operator's search and filters give way to it. The
        // archived toggle is left as it is.
        if !self.hits.iter().any(|hit| hit.song == song) {
            self.clear_search_and_filters(window, cx);
        }
        self.select(song, cx);
    }

    /// The store this workspace reads.
    pub fn store(&self) -> &Entity<LibraryStore> {
        &self.store
    }

    /// The search field, so a test or lab can type into it.
    pub fn search_field(&self) -> &FieldState {
        &self.search
    }

    /// The query as last computed.
    pub fn query(&self) -> &LibraryQuery {
        &self.query
    }

    /// The Songs the current query found, in rank order.
    pub fn hits(&self) -> &[SongHit] {
        &self.hits
    }

    /// The selected Song, if any.
    pub fn selected(&self) -> Option<SongId> {
        self.selected
    }

    /// Where the list is scrolled, in pixels from the top.
    pub fn list_offset(&self) -> Point<Pixels> {
        self.list_scroll.handle().offset()
    }

    /// Scrolls the list, for a test or lab.
    pub fn scroll_list_to(&self, offset: Point<Pixels>) {
        self.list_scroll.handle().set_offset(offset);
    }

    /// Selects a Song and refreshes the detail pane.
    pub fn select(&mut self, song: SongId, cx: &mut Context<Self>) {
        self.selected = Some(song);
        cx.notify();
    }

    /// Recomputes the hits from the field's text and the filters. Called when
    /// the text, a filter, the archived toggle or the library changes.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.query.text = self.search.value(cx).to_string();
        self.hits = search(self.store.read(cx).library(), &self.query);
        if let Some(selected) = self.selected
            && !self.hits.iter().any(|hit| hit.song == selected)
        {
            self.selected = None;
        }
        cx.notify();
    }

    /// Turns every filter, the search text and nothing else back to empty. The
    /// archived toggle is a view choice of its own and stays.
    pub fn clear_search_and_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.set_value("", window, cx);
        self.query.choirs.clear();
        self.query.languages.clear();
        self.query.occasions.clear();
        self.open_filter = None;
        self.refresh(cx);
    }

    /// Sets the archived toggle.
    pub fn set_include_archived(&mut self, include: bool, cx: &mut Context<Self>) {
        self.query.include_archived = include;
        self.refresh(cx);
    }

    /// Remembers the list position before the shell replaces the screen with an
    /// Arrangement.
    pub fn remember_scroll(&mut self) {
        self.restore_scroll = Some(self.list_scroll.handle().offset());
    }

    /// The pane the shell hosts in its list region.
    pub fn render_list(
        &mut self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(offset) = self.restore_scroll.take() {
            self.list_scroll.handle().set_offset(offset);
        }
        list::render(self, environment, window, cx)
    }

    /// The pane the shell hosts in its viewer region.
    pub fn render_detail(
        &mut self,
        environment: UiEnvironment,
        open: OpenArrangement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if std::mem::take(&mut self.focus_arrangements_pending) {
            // The handle is carried by controls this very render draws, so
            // focus moves once they are in the frame.
            let handle = self.arrangement_focus.clone();
            window
                .spawn(cx, async move |cx| {
                    cx.update(|window, cx| handle.focus(window, cx)).ok();
                })
                .detach();
        }
        detail::render(self, environment, open, window, cx)
    }

    /// A handler for a control, run against this entity.
    fn act(
        cx: &Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let weak = cx.weak_entity();
        move |window, cx| {
            let _ = weak.update(cx, |workspace, cx| run(workspace, window, cx));
        }
    }
}
