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

use std::{path::PathBuf, rc::Rc};

use domain::{
    identity::{ArrangementId, SongId},
    song_library::{
        AfterArchive, LibraryQuery, LifecycleAction, SongHit, search, selection_after_archive,
    },
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, Pixels, Point, Subscription,
    Task, Window,
};
use ui::{
    environment::UiEnvironment,
    overlay::{OverlayHost, Toast},
    primitives::field::{FieldEvent, FieldState},
    scroll::Scroll,
};

use crate::history::{AfterRestore, HistoryTarget, open_history};

use super::{
    arrangement_details::{self, ArrangementDetailsPanel},
    import_review::{FilePicker, Picked, SystemPicker, route},
    lifecycle::{
        AfterLifecycle, LifecycleOutcome, LifecycleTarget, confirm_archive, restore,
        review_deletion,
    },
    new_song::{NewSongPanel, Outcome},
    store::{ImportStaging, LibraryEvent, LibraryState, LibraryStore},
};
pub use filters::FilterKind;

/// The toast for a pick no import path accepts (Decision 21).
pub(crate) const CHOOSE_FILES: &str = "Choose .ordo files, or one Markdown (.md or .txt) song.";
/// The named progress indicator while files are read (DESIGN 620).
pub(crate) const READING_FILES: &str = "Reading files\u{2026}";

/// What the workspace asks the shell to open: a full-workspace frame over the
/// screen, built from what the operator picked.
pub enum SongsEvent {
    /// Review Song import over a staged batch.
    OpenReview(ImportStaging),
    /// The Markdown draft over one picked Markdown or text file.
    OpenDraft {
        /// The file's name, for the frame's meta line.
        file: String,
        /// The file's bytes, as read.
        bytes: Vec<u8>,
        /// The parse of those bytes, with fresh identities.
        draft: Box<domain::song_library::import::MarkdownDraft>,
    },
}

/// Focus the next draw of the list should place, once the control is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingFocus {
    /// The header's **Import .ordo**, after the frame that import opened closes.
    Import,
    /// The selected Song's row, after an import was committed.
    Row,
}

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
    /// How files are picked; the system dialog unless a test installs another.
    picker: Rc<dyn FilePicker>,
    /// The header's **Import .ordo**, where focus returns when the frame it
    /// opened closes.
    import_focus: gpui::FocusHandle,
    /// Whether an import is between the pick and the frame it opens.
    importing: bool,
    /// Whether the picked files are being read, which the list says by name.
    reading: bool,
    /// The pick and the read, owned here so a closed workspace drops them.
    import_task: Option<Task<()>>,
    pending_focus: Option<PendingFocus>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SongsEvent> for SongsWorkspace {}

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
            focus_arrangements_pending: false,
            open_filter: None,
            restore_scroll: None,
            overlays: None,
            new_song: None,
            new_arrangement: None,
            picker: Rc::new(SystemPicker),
            import_focus: cx.focus_handle(),
            importing: false,
            reading: false,
            import_task: None,
            pending_focus: None,
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

    /// Replaces how files are picked, for a test.
    pub fn set_picker(&mut self, picker: Rc<dyn FilePicker>) {
        self.picker = picker;
    }

    /// How files are picked.
    pub fn picker(&self) -> Rc<dyn FilePicker> {
        self.picker.clone()
    }

    /// Whether the picked files are being read.
    pub fn reading(&self) -> bool {
        self.reading
    }

    /// The header's **Import .ordo**, which takes focus back when the frame it
    /// opened closes.
    pub fn import_focus(&self) -> &gpui::FocusHandle {
        &self.import_focus
    }

    /// Asks for files and routes them (Decision 21): `.ordo` files become one
    /// Review Song import, one Markdown or text file becomes the draft, and
    /// anything else is refused with a toast. Reading and classifying run off
    /// the GPUI thread; the shell opens the frame on the [`SongsEvent`].
    /// Cancelling the pick changes nothing. Does nothing while the library is
    /// not readable, another import is under way, or a surface is open.
    pub fn import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.importing
            || *self.store.read(cx).state() != LibraryState::Ready
            || self.overlay_depth(cx) > 0
        {
            return;
        }
        self.importing = true;
        let picked = self.picker.pick(true, cx);
        self.import_task = Some(cx.spawn(async move |this, cx| {
            let paths = picked.await.unwrap_or_default();
            let event = match route(&paths) {
                _ if paths.is_empty() => None,
                Picked::Refused => {
                    let _ = this.update(cx, |workspace, cx| {
                        workspace.toast_failure(CHOOSE_FILES, cx)
                    });
                    None
                }
                Picked::Ordo(paths) => {
                    let Ok(task) = this.update(cx, |workspace, cx| {
                        workspace.reading = true;
                        cx.notify();
                        workspace
                            .store
                            .update(cx, |store, cx| store.stage_import(paths, cx))
                    }) else {
                        return;
                    };
                    Some(SongsEvent::OpenReview(task.await))
                }
                Picked::Markdown(path) => {
                    let Ok((draft, bytes)) = this.update(cx, |workspace, cx| {
                        workspace.reading = true;
                        cx.notify();
                        let draft = workspace
                            .store
                            .update(cx, |store, cx| store.read_markdown(path.clone(), cx));
                        (draft, cx.background_spawn(read_bytes(path)))
                    }) else {
                        return;
                    };
                    match (draft.await, bytes.await) {
                        (Ok(draft), Ok((file, bytes))) => Some(SongsEvent::OpenDraft {
                            file,
                            bytes,
                            draft: Box::new(draft),
                        }),
                        (Err(message), _) | (_, Err(message)) => {
                            let _ = this.update(cx, |workspace, cx| {
                                workspace.toast_failure(message, cx);
                            });
                            None
                        }
                    }
                }
            };
            let _ = this.update(cx, |workspace, cx| {
                workspace.importing = false;
                workspace.reading = false;
                workspace.import_task = None;
                if let Some(event) = event {
                    cx.emit(event);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Says `message` in a failure toast, when the overlay layer is there.
    fn toast_failure(&self, message: impl Into<gpui::SharedString>, cx: &mut App) {
        if let Some(host) = &self.overlays {
            host.notify(Toast::failure(message), cx);
        }
    }

    /// Puts focus on **Import .ordo** once the list draws it: the frame the
    /// import opened has closed without writing.
    pub fn focus_import(&mut self, cx: &mut Context<Self>) {
        self.pending_focus = Some(PendingFocus::Import);
        cx.notify();
    }

    /// Selects `song` and puts focus on its row once the list draws it: the
    /// import was committed and `song` is the first it created or updated.
    pub fn show_imported(&mut self, song: SongId, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal(song, window, cx);
        self.focus_song = Some(song);
        self.pending_focus = Some(PendingFocus::Row);
        cx.notify();
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
        // Each request carries its own snapshot, so a Cancel or a refusal
        // leaves nothing behind for the next one.
        let removal = Removal {
            visible,
            include_archived: self.query.include_archived,
        };
        let weak = cx.weak_entity();
        let inspect_open = open.clone();
        let after = AfterLifecycle::new(move |outcome, window, cx| {
            let _ = weak.update(cx, |workspace, cx| {
                workspace.after_lifecycle(outcome, &removal, window, cx);
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
        removal: &Removal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            LifecycleOutcome::Archived(LifecycleTarget::Song(song))
            | LifecycleOutcome::Deleted(LifecycleTarget::Song(song)) => {
                let deleted = matches!(outcome, LifecycleOutcome::Deleted(_));
                self.refresh(cx);
                match selection_after_archive(
                    &removal.visible,
                    song,
                    removal.include_archived && !deleted,
                ) {
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
                        // Focus goes to a control that is drawn: the first
                        // remaining row (nothing is selected, the domain rule
                        // stands), else the empty or no-match state's action,
                        // which is **Create song** for an empty library.
                        if let Some(first) = self.hits.first() {
                            self.focus_song = Some(first.song);
                            self.row_focus.focus(window, cx);
                        } else {
                            self.empty_focus.focus(window, cx);
                        }
                        cx.notify();
                    }
                }
            }
            LifecycleOutcome::Restored(LifecycleTarget::Song(song)) => {
                // The control that held focus (the empty state's action, the
                // toast's Undo) is gone with the restore, so the restored
                // Song's row takes it; `reveal` put the Song in the list.
                self.reveal(song, window, cx);
                self.focus_song = Some(song);
                self.row_focus.focus(window, cx);
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
    pub(crate) fn reveal(&mut self, song: SongId, window: &mut Window, cx: &mut Context<Self>) {
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
        if let Some(pending) = self.pending_focus.take() {
            // The control is drawn by this very render, so focus moves once
            // it is in the frame.
            let handle = match pending {
                PendingFocus::Import => self.import_focus.clone(),
                PendingFocus::Row => self.row_focus.clone(),
            };
            window
                .spawn(cx, async move |cx| {
                    cx.update(|window, cx| handle.focus(window, cx)).ok();
                })
                .detach();
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

/// Reads a picked file's bytes and name; the error is the operator copy.
async fn read_bytes(path: PathBuf) -> Result<(String, Vec<u8>), gpui::SharedString> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    match std::fs::read(&path) {
        Ok(bytes) => Ok((name, bytes)),
        Err(_) => Err(format!("Couldn't read {name}.").into()),
    }
}
