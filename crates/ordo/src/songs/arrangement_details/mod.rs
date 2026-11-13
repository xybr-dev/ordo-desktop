//! The Arrangement details panel (MVP-PLAN 6.15): one slideover that creates
//! an Arrangement of a Song (**New Arrangement**) or edits the details of an
//! existing one (**Arrangement details**).
//!
//! [`ArrangementDetailsPanel`] is an entity the opener keeps while the panel is
//! open. It draws a slideover over the screen beneath it, so the workspace's
//! query, filters, selection and scroll are untouched. Everything the operator
//! changes is staged in the panel; nothing is stored until the primary action,
//! except a Choir created from the Choir picker, which is saved at once and
//! stays saved if the Arrangement is cancelled.
//!
//! The panel decides nothing about the screens: it closes its own surface when
//! a write stood and reports an [`Outcome`], and the opener navigates.

mod form;
mod pickers;
#[cfg(test)]
pub(crate) mod tests;

use std::rc::Rc;

use domain::{
    identity::{ArrangementId, ChoirId, SongId},
    song_library::{ArrangementDetails, ChoirDraft},
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, SharedString,
    Subscription, Task, Window,
};
use ui::{
    environment::UiEnvironment,
    overlay::{Overlay, OverlayHost, Toast},
    primitives::{
        Availability,
        field::{FieldEvent, FieldState},
    },
};

use crate::songs::store::LibraryStore;

pub use form::{build_details, derived_label};

/// What the panel is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// A new Arrangement of this Song.
    New(SongId),
    /// The details of this Arrangement.
    Edit(ArrangementId),
}

/// What the operator decided. The panel has already closed its surface when
/// it reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// A new Arrangement was created.
    Created(ArrangementId),
    /// An Arrangement's details were saved (or left as they were).
    Saved(ArrangementId),
    /// The stored Arrangement was duplicated; this is the copy.
    Duplicated(ArrangementId),
}

/// Called with the outcome, from outside any entity update.
pub type Report = Rc<dyn Fn(Outcome, &mut Window, &mut App)>;
/// Called when the panel's surface has gone, however it was dismissed.
pub type Closed = Rc<dyn Fn(&mut App)>;

/// Which search is open under its chips.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Picker {
    /// The built-in language list.
    Language,
    /// Active Choirs, with a create row.
    Choir,
}

/// The staged details form.
pub struct ArrangementDetailsPanel {
    store: Entity<LibraryStore>,
    host: OverlayHost,
    mode: Mode,
    title: FieldState,
    translator: FieldState,
    notes: FieldState,
    language_search: FieldState,
    choir_search: FieldState,
    languages: Vec<String>,
    /// Languages picked from the search during this edit, kept as chips even
    /// after they are switched off.
    offered: Vec<String>,
    choirs: Vec<ChoirId>,
    occasions: Vec<String>,
    picker: Option<Picker>,
    active: usize,
    add_language_focus: FocusHandle,
    add_choir_focus: FocusHandle,
    /// Set once the primary action was tried, so the language error shows.
    tried: bool,
    /// The name of the Choir created from the picker, for its note.
    made: Option<String>,
    /// A failed write: what failed, and the store's reason.
    error: Option<(SharedString, SharedString)>,
    busy: bool,
    creating_choir: bool,
    dismissed: bool,
    pending: Option<Task<()>>,
    choir_task: Option<Task<()>>,
    report: Report,
    _subscriptions: Vec<Subscription>,
}

impl ArrangementDetailsPanel {
    /// Opens the panel for a new Arrangement of `song`.
    pub fn open_new(
        store: Entity<LibraryStore>,
        song: SongId,
        host: OverlayHost,
        report: Report,
        closed: Closed,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::open(store, Mode::New(song), host, report, closed, window, cx)
    }

    /// Opens the panel on the details of `arrangement`.
    pub fn open_edit(
        store: Entity<LibraryStore>,
        arrangement: ArrangementId,
        host: OverlayHost,
        report: Report,
        closed: Closed,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::open(
            store,
            Mode::Edit(arrangement),
            host,
            report,
            closed,
            window,
            cx,
        )
    }

    fn open(
        store: Entity<LibraryStore>,
        mode: Mode,
        host: OverlayHost,
        report: Report,
        closed: Closed,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let panel = cx.new(|cx| Self::new(store, mode, host.clone(), report, window, cx));
        let edit = matches!(mode, Mode::Edit(_));
        let body = panel.clone();
        let gate = panel.clone();
        let submit = panel.clone();
        let close = panel.clone();
        let (title, primary) = if edit {
            (form::EDIT_TITLE, form::SAVE)
        } else {
            (form::NEW_TITLE, form::CREATE)
        };
        let mut overlay = Overlay::slideover(title)
            .body(move |environment, window, cx| {
                body.update(cx, |panel, cx| panel.render_form(environment, window, cx))
            })
            .primary_gated(
                primary,
                move |cx| {
                    if gate.read(cx).busy {
                        Availability::unavailable(form::BUSY_REASON)
                    } else {
                        Availability::Available
                    }
                },
                move |window, cx| {
                    submit.update(cx, |panel, cx| panel.submit(window, cx));
                },
            )
            .secondary("Cancel", |_, _| {})
            .on_close(move |_, cx| {
                close.update(cx, |panel, _| panel.dismiss());
                closed(cx);
            });
        if edit {
            let gate = panel.clone();
            let duplicate = panel.clone();
            overlay = overlay.leading_gated(
                form::DUPLICATE,
                move |cx| {
                    if gate.read(cx).busy {
                        Availability::unavailable(form::BUSY_REASON)
                    } else {
                        Availability::Available
                    }
                },
                move |window, cx| {
                    duplicate.update(cx, |panel, cx| panel.duplicate(window, cx));
                },
            );
        }
        host.open(overlay, window, cx);
        let first = panel.read(cx).title.clone();
        first.focus(window, cx);
        panel
    }

    fn new(
        store: Entity<LibraryStore>,
        mode: Mode,
        host: OverlayHost,
        report: Report,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fields = [
            FieldState::new(window, cx),
            FieldState::new(window, cx),
            FieldState::new(window, cx),
            FieldState::new(window, cx),
            FieldState::new(window, cx),
        ];
        let mut subscriptions = Vec::new();
        for field in &fields {
            let weak = cx.weak_entity();
            subscriptions.push(field.subscribe(cx, move |event, cx| {
                if event == FieldEvent::Changed {
                    let _ = weak.update(cx, |panel, cx| {
                        panel.error = None;
                        panel.active = 0;
                        cx.notify();
                    });
                }
            }));
        }
        subscriptions.push(cx.observe(&store, |_, _, cx| cx.notify()));
        let [title, translator, notes, language_search, choir_search] = fields;
        let mut panel = Self {
            store,
            host,
            mode,
            title,
            translator,
            notes,
            language_search,
            choir_search,
            languages: Vec::new(),
            offered: Vec::new(),
            choirs: Vec::new(),
            occasions: Vec::new(),
            picker: None,
            active: 0,
            add_language_focus: cx.focus_handle().tab_stop(true),
            add_choir_focus: cx.focus_handle().tab_stop(true),
            tried: false,
            made: None,
            error: None,
            busy: false,
            creating_choir: false,
            dismissed: false,
            pending: None,
            choir_task: None,
            report,
            _subscriptions: subscriptions,
        };
        if let Mode::Edit(id) = mode {
            let stored = panel
                .store
                .read(cx)
                .library()
                .arrangement(id)
                .ok()
                .map(domain::song_library::Arrangement::details);
            if let Some(details) = stored {
                panel
                    .title
                    .set_value(details.title.unwrap_or_default(), window, cx);
                panel.translator.set_value(
                    details.rendition_attribution.unwrap_or_default(),
                    window,
                    cx,
                );
                panel
                    .notes
                    .set_value(details.notes.unwrap_or_default(), window, cx);
                panel.languages = details.languages;
                panel.choirs = details.choirs;
                panel.occasions = details.occasions;
            }
        }
        panel
    }

    /// What the panel is for.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The Display title field.
    pub fn title_field(&self) -> &FieldState {
        &self.title
    }

    /// The Translator or adapter field.
    pub fn translator_field(&self) -> &FieldState {
        &self.translator
    }

    /// The Source and rights notes field.
    pub fn notes_field(&self) -> &FieldState {
        &self.notes
    }

    /// The staged languages, in authored order.
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// The staged Choirs.
    pub fn choirs(&self) -> &[ChoirId] {
        &self.choirs
    }

    /// The staged occasions.
    pub fn occasions(&self) -> &[String] {
        &self.occasions
    }

    /// The open picker, if any.
    pub fn picker(&self) -> Option<Picker> {
        self.picker
    }

    /// Whether a write is in flight.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// The reason the last write failed, while it is shown.
    pub fn error(&self) -> Option<&SharedString> {
        self.error.as_ref().map(|(_, reason)| reason)
    }

    /// The staged details: what the primary action would commit.
    pub fn details(&self, cx: &App) -> ArrangementDetails {
        build_details(
            &self.title.value(cx),
            &self.languages,
            &self.choirs,
            &self.translator.value(cx),
            &self.occasions,
            &self.notes.value(cx),
        )
    }

    /// The Choir names, in label order, when the STORED Arrangement is shared
    /// by more than one Choir (Edit only).
    pub fn shared_notice(&self, cx: &App) -> Option<Vec<String>> {
        let Mode::Edit(id) = self.mode else {
            return None;
        };
        let library = self.store.read(cx).library();
        let stored: Vec<ChoirId> = library
            .arrangement(id)
            .ok()?
            .choirs()
            .iter()
            .copied()
            .collect();
        (stored.len() > 1).then(|| form::choir_names_in_label_order(library, &stored))
    }

    /// Switches a language on or off.
    pub fn toggle_language(&mut self, name: &str, cx: &mut Context<Self>) {
        match self
            .languages
            .iter()
            .position(|staged| form::same_language(staged, name))
        {
            Some(at) => {
                self.languages.remove(at);
            }
            None => self.languages.push(name.to_owned()),
        }
        self.error = None;
        cx.notify();
    }

    /// Switches a Choir on or off.
    pub fn toggle_choir(&mut self, id: ChoirId, cx: &mut Context<Self>) {
        match self.choirs.iter().position(|staged| *staged == id) {
            Some(at) => {
                self.choirs.remove(at);
            }
            None => self.choirs.push(id),
        }
        self.error = None;
        cx.notify();
    }

    /// Switches an occasion on or off.
    pub fn toggle_occasion(&mut self, name: &str, cx: &mut Context<Self>) {
        match self.occasions.iter().position(|staged| staged == name) {
            Some(at) => {
                self.occasions.remove(at);
            }
            None => self.occasions.push(name.to_owned()),
        }
        self.error = None;
        cx.notify();
    }

    /// Saves a name-only Choir at once and stages it. The Arrangement stays
    /// unsaved; the Choir stays saved if the panel is cancelled.
    pub(super) fn create_choir(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.creating_choir {
            return;
        }
        self.creating_choir = true;
        let name = name.trim().to_owned();
        let draft = ChoirDraft::named(name.clone());
        let task = self
            .store
            .update(cx, |store, cx| store.create_choir(draft, cx));
        let panel = cx.weak_entity();
        self.choir_task = Some(window.spawn(cx, async move |cx| {
            let outcome = task.await;
            cx.update(|window, cx| {
                let _ = panel.update(cx, |panel, cx| {
                    panel.creating_choir = false;
                    match outcome {
                        Ok(id) => {
                            if !panel.dismissed {
                                panel.choirs.push(id);
                                panel.made = Some(name);
                                panel.close_picker(window, cx);
                            }
                        }
                        Err(message) => {
                            panel.error = Some((form::FAILED_CHOIR.into(), message));
                        }
                    }
                    cx.notify();
                });
            })
            .ok();
        }));
    }

    /// Stores the staged details through the store's write queue. With no
    /// language it only shows the error. On failure the panel stays open with
    /// its form and says why. Does nothing while a write is in flight.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.languages.is_empty() {
            self.tried = true;
            cx.notify();
            return;
        }
        let details = self.details(cx);
        match self.mode {
            Mode::New(song) => {
                let task = self
                    .store
                    .update(cx, |store, cx| store.create_arrangement(song, details, cx));
                self.run(
                    task,
                    form::FAILED_CREATE,
                    Outcome::Created,
                    false,
                    window,
                    cx,
                );
            }
            Mode::Edit(id) => {
                let unchanged = self
                    .store
                    .read(cx)
                    .library()
                    .arrangement(id)
                    .is_ok_and(|stored| form::same_details(&stored.details(), &details));
                if unchanged {
                    // Nothing to store: just let go, with no toast.
                    self.run(
                        Task::ready(Ok(id)),
                        form::FAILED_SAVE,
                        Outcome::Saved,
                        false,
                        window,
                        cx,
                    );
                    return;
                }
                let task = self.store.update(cx, |store, cx| {
                    store.save_arrangement_details(id, details, cx)
                });
                self.run(task, form::FAILED_SAVE, Outcome::Saved, true, window, cx);
            }
        }
    }

    /// Duplicates the STORED Arrangement (not the staged edit), closes the
    /// panel and reports the copy.
    pub fn duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Edit(id) = self.mode else {
            return;
        };
        if self.busy {
            return;
        }
        let task = self
            .store
            .update(cx, |store, cx| store.duplicate_arrangement(id, cx));
        self.run(
            task,
            form::FAILED_DUPLICATE,
            Outcome::Duplicated,
            false,
            window,
            cx,
        );
    }

    fn run(
        &mut self,
        task: Task<Result<ArrangementId, SharedString>>,
        failed: &'static str,
        outcome: fn(ArrangementId) -> Outcome,
        toast: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.error = None;
        let panel = cx.weak_entity();
        let report = self.report.clone();
        let host = self.host.clone();
        let own = self.surface_title();
        self.pending = Some(window.spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|window, cx| {
                let Some(panel) = panel.upgrade() else {
                    return;
                };
                let done = panel.update(cx, |panel, cx| {
                    panel.busy = false;
                    cx.notify();
                    if panel.dismissed {
                        return None;
                    }
                    match result {
                        Ok(id) => Some(id),
                        Err(message) => {
                            panel.error = Some((failed.into(), message));
                            None
                        }
                    }
                });
                if let Some(id) = done {
                    // Outside the panel's own update: closing runs its on_close.
                    if host.top(cx).is_some_and(|top| top == own) {
                        host.close_top(window, cx);
                    }
                    if toast {
                        host.notify(Toast::confirmation(form::SAVED_TOAST), cx);
                    }
                    report(outcome(id), window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// The title of the panel's own surface, which is how it finds it on the
    /// host: a surface another part of the app stacked above it is not the
    /// panel's to close.
    fn surface_title(&self) -> &'static str {
        if matches!(self.mode, Mode::Edit(_)) {
            form::EDIT_TITLE
        } else {
            form::NEW_TITLE
        }
    }

    /// The panel's surface has gone: forget any in-flight result.
    fn dismiss(&mut self) {
        self.dismissed = true;
        self.pending = None;
    }

    fn render_form(
        &mut self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        form::render(self, environment, window, cx).into_any_element()
    }
}
