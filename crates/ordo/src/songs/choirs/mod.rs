//! The Choirs workspace: the permanent list-detail screen of Library → Choirs.
//!
//! One [`ChoirsWorkspace`] entity lives as long as the shell, so the selection,
//! the archived toggle and the list's scroll survive opening an Arrangement
//! from a Choir's repertoire and coming back. The shell hosts the two panes
//! ([`ChoirsWorkspace::render_list`], [`ChoirsWorkspace::render_detail`]) and
//! keeps the chrome.
//!
//! Choirs are optional. Adding one needs only a name, through
//! [`ChoirQuickCreate`], which never creates a second Choir whose name differs
//! only in case or spacing. A Choir's details (name, short label, preferred
//! languages, private notes) save as the operator types: a
//! `domain::revisions::TextCoalescer` makes an edit due after one idle second
//! or when focus leaves the field, and the header says Saving…, Saved or
//! Couldn't save. Every write goes through the [`LibraryStore`], which
//! validates, writes off the GPUI thread and only then updates memory.

mod detail;
mod list;
pub mod quick_create;
#[cfg(test)]
pub(crate) mod tests;

use std::{rc::Rc, time::Duration, time::Instant};

use domain::{identity::ChoirId, revisions::TextCoalescer, song_library::ChoirDraft};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Pixels, Point, Subscription, Task, Window,
};
use ui::{
    environment::UiEnvironment,
    primitives::field::{FieldEvent, FieldState},
    scroll::Scroll,
};

use super::{
    library::OpenArrangement,
    store::{LibraryEvent, LibraryStore},
};
use crate::edit_session::SaveState;
pub use quick_create::{ChoirQuickCreate, Offer, offer};

/// The editable fields of the selected Choir.
struct Form {
    name: FieldState,
    short_label: FieldState,
    notes: FieldState,
    /// Where a new preferred language is typed before Enter adds it.
    language: FieldState,
    languages: Vec<String>,
}

/// Selection, archived toggle, inline add and details autosave of Library →
/// Choirs.
pub struct ChoirsWorkspace {
    store: Entity<LibraryStore>,
    selected: Option<ChoirId>,
    show_archived: bool,
    quick: Option<Entity<ChoirQuickCreate>>,
    form: Form,
    /// The Choir the form fields currently hold.
    loaded: Option<ChoirId>,
    /// Whether the title is showing its inline name field.
    renaming: bool,
    coalescer: TextCoalescer,
    epoch: Instant,
    in_flight: bool,
    failed: bool,
    timer: Option<Task<()>>,
    /// Why the last archive or restore was refused, shown in the header.
    message: Option<gpui::SharedString>,
    list_scroll: Scroll,
    detail_scroll: Scroll,
    restore_scroll: Option<Point<Pixels>>,
    _subscriptions: Vec<Subscription>,
}

impl ChoirsWorkspace {
    /// A workspace over `store`.
    pub fn new(store: Entity<LibraryStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = Form {
            name: FieldState::new(window, cx),
            short_label: FieldState::new(window, cx),
            notes: FieldState::new(window, cx),
            language: FieldState::new(window, cx),
            languages: Vec::new(),
        };
        let mut subscriptions = Vec::new();
        for (is_name, field) in [
            (true, &form.name),
            (false, &form.short_label),
            (false, &form.notes),
        ] {
            let weak = cx.weak_entity();
            subscriptions.push(field.subscribe(cx, move |event, cx| {
                let _ = weak.update(cx, |workspace, cx| {
                    // Leaving the name field ends the inline rename.
                    if is_name && event == FieldEvent::FocusLeft {
                        workspace.renaming = false;
                    }
                    workspace.field_event(event, cx);
                });
            }));
        }
        subscriptions.push(cx.subscribe(&store, |_, _, _: &LibraryEvent, cx| cx.notify()));
        Self {
            store,
            selected: None,
            show_archived: false,
            quick: None,
            form,
            loaded: None,
            renaming: false,
            coalescer: TextCoalescer::new(),
            epoch: cx.background_executor().now(),
            in_flight: false,
            failed: false,
            timer: None,
            message: None,
            list_scroll: Scroll::new(),
            detail_scroll: Scroll::new(),
            restore_scroll: None,
            _subscriptions: subscriptions,
        }
    }

    /// The store this workspace reads and writes.
    pub fn store(&self) -> &Entity<LibraryStore> {
        &self.store
    }

    /// The selected Choir, if any.
    pub fn selected(&self) -> Option<ChoirId> {
        self.selected
    }

    /// Whether archived Choirs are listed.
    pub fn show_archived(&self) -> bool {
        self.show_archived
    }

    /// The name field of the detail pane, for a test or lab.
    pub fn name_field(&self) -> &FieldState {
        &self.form.name
    }

    /// The short label field.
    pub fn short_label_field(&self) -> &FieldState {
        &self.form.short_label
    }

    /// The private notes field.
    pub fn notes_field(&self) -> &FieldState {
        &self.form.notes
    }

    /// The field a new preferred language is typed into.
    pub fn language_field(&self) -> &FieldState {
        &self.form.language
    }

    /// The inline add-choir block while it is open.
    pub fn quick_create(&self) -> Option<&Entity<ChoirQuickCreate>> {
        self.quick.as_ref()
    }

    /// Where the list is scrolled.
    pub fn list_offset(&self) -> Point<Pixels> {
        self.list_scroll.handle().offset()
    }

    /// Where the details save stands.
    pub fn save_state(&self) -> SaveState {
        if self.failed {
            SaveState::CouldntSave
        } else if self.in_flight || self.coalescer.is_pending() {
            SaveState::Saving
        } else {
            SaveState::Saved
        }
    }

    /// Selects a Choir. Details typed for the previous one are written first.
    pub fn select(&mut self, id: ChoirId, cx: &mut Context<Self>) {
        self.flush(cx);
        self.selected = Some(id);
        self.renaming = false;
        self.message = None;
        cx.notify();
    }

    /// Shows or hides archived Choirs.
    pub fn set_show_archived(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_archived = show;
        cx.notify();
    }

    /// Opens the inline add-choir block at the top of the list.
    pub fn begin_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(quick) = &self.quick {
            let field = quick.read(cx).field().clone();
            field.focus(window, cx);
            return;
        }
        let weak = cx.weak_entity();
        let done: quick_create::Done = {
            let weak = weak.clone();
            Rc::new(move |id, cx: &mut App| {
                let _ = weak.update(cx, |workspace, cx| {
                    workspace.quick = None;
                    workspace.select(id, cx);
                });
            })
        };
        let cancel: quick_create::Cancel = Rc::new(move |cx: &mut App| {
            let _ = weak.update(cx, |workspace, cx| {
                workspace.quick = None;
                cx.notify();
            });
        });
        let store = self.store.clone();
        let quick = cx.new(|cx| ChoirQuickCreate::new(store, done, cancel, window, cx));
        self.quick = Some(watch_quick(quick, cx));
        cx.notify();
    }

    /// Archives the selected Choir.
    pub fn archive_selected(&mut self, cx: &mut Context<Self>) {
        self.change_archive(true, cx);
    }

    /// Restores the selected Choir.
    pub fn restore_selected(&mut self, cx: &mut Context<Self>) {
        self.change_archive(false, cx);
    }

    fn change_archive(&mut self, archive: bool, cx: &mut Context<Self>) {
        let Some(id) = self.selected else { return };
        // Typed details go first; the store runs writes in order.
        self.flush(cx);
        self.message = None;
        let task = self.store.update(cx, |store, cx| {
            if archive {
                store.archive_choir(id, cx)
            } else {
                store.restore_choir(id, cx)
            }
        });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            let _ = this.update(cx, |workspace, cx| {
                if let Err(message) = outcome {
                    workspace.message = Some(message);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Remembers the list position before the shell replaces the screen with
    /// an Arrangement.
    pub fn remember_scroll(&mut self) {
        self.restore_scroll = Some(self.list_scroll.handle().offset());
    }

    /// Adds the language typed in the language field and saves.
    pub fn add_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.form.language.value(cx).trim().to_owned();
        if typed.is_empty() || self.is_read_only(cx) {
            return;
        }
        let key = domain::song_library::match_key(&typed);
        if !self
            .form
            .languages
            .iter()
            .any(|language| domain::song_library::match_key(language) == key)
        {
            self.form.languages.push(typed);
            self.edited(cx);
            self.flush(cx);
        }
        self.form.language.set_value("", window, cx);
        cx.notify();
    }

    /// Removes one preferred language and saves.
    pub fn remove_language(&mut self, at: usize, cx: &mut Context<Self>) {
        if at < self.form.languages.len() && !self.is_read_only(cx) {
            self.form.languages.remove(at);
            self.edited(cx);
            self.flush(cx);
            cx.notify();
        }
    }

    /// Whether the loaded Choir is archived (inspect-only).
    fn is_read_only(&self, cx: &App) -> bool {
        self.loaded
            .and_then(|id| self.store.read(cx).library().choir(id).ok())
            .is_none_or(|choir| choir.is_archived())
    }

    /// Fills the form from the library when the selection changed. Run at the
    /// start of rendering the detail pane, where the window is at hand.
    fn sync_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loaded == self.selected {
            return;
        }
        self.loaded = self.selected;
        self.renaming = false;
        let (name, label, notes, languages) = self
            .selected
            .and_then(|id| self.store.read(cx).library().choir(id).ok())
            .map(|choir| {
                (
                    choir.name().to_owned(),
                    choir.short_label().unwrap_or_default().to_owned(),
                    choir.notes().unwrap_or_default().to_owned(),
                    choir.languages().to_vec(),
                )
            })
            .unwrap_or_default();
        self.form.name.set_value(name, window, cx);
        self.form.short_label.set_value(label, window, cx);
        self.form.notes.set_value(notes, window, cx);
        self.form.language.set_value("", window, cx);
        self.form.languages = languages;
    }

    fn field_event(&mut self, event: FieldEvent, cx: &mut Context<Self>) {
        match event {
            FieldEvent::Changed => self.edited(cx),
            FieldEvent::FocusLeft => self.flush(cx),
        }
    }

    fn now_ms(&self, cx: &App) -> i64 {
        let elapsed = cx
            .background_executor()
            .now()
            .saturating_duration_since(self.epoch);
        i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
    }

    /// The operator changed a detail: it becomes due after one idle second.
    fn edited(&mut self, cx: &mut Context<Self>) {
        if self.loaded.is_none() || self.is_read_only(cx) {
            return;
        }
        let now = self.now_ms(cx);
        self.coalescer.edited(now);
        self.failed = false;
        self.schedule(cx);
    }

    /// Makes any pending edit due now and writes it.
    fn flush(&mut self, cx: &mut Context<Self>) {
        if self.coalescer.focus_left() {
            self.pump(cx);
        }
    }

    /// Arms the idle timer when an edit waits and nothing blocks it.
    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        if !self.failed && !self.in_flight {
            let now = self.now_ms(cx);
            if let Some(due) = self.coalescer.due_in(now) {
                let wait = Duration::from_millis(u64::try_from(due).unwrap_or(0));
                self.timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    let _ = this.update(cx, |workspace, cx| workspace.pump(cx));
                }));
            }
        }
        cx.notify();
    }

    /// The details as a draft: text verbatim, blank optional text absent.
    fn draft(&self, cx: &App) -> ChoirDraft {
        let optional = |field: &FieldState| {
            let text = field.value(cx);
            (!text.trim().is_empty()).then(|| text.to_string())
        };
        ChoirDraft {
            name: self.form.name.value(cx).to_string(),
            short_label: optional(&self.form.short_label),
            languages: self.form.languages.clone(),
            notes: optional(&self.form.notes),
        }
    }

    /// Writes the pending edit when it is due and no write is in flight.
    fn pump(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        let now = self.now_ms(cx);
        if self.in_flight || self.failed || !self.coalescer.is_due(now) {
            self.schedule(cx);
            return;
        }
        let generation = self.coalescer.generation();
        let settle = |this: &mut Self| {
            if let Some(generation) = generation {
                this.coalescer.settle(generation);
            }
        };
        let Some(target) = self.loaded else {
            settle(self);
            self.schedule(cx);
            return;
        };
        let draft = self.draft(cx);
        let unchanged = self
            .store
            .read(cx)
            .library()
            .choir(target)
            .is_ok_and(|choir| {
                choir.name() == draft.name
                    && choir.short_label() == draft.short_label.as_deref()
                    && choir.languages() == draft.languages.as_slice()
                    && choir.notes() == draft.notes.as_deref()
            });
        if unchanged {
            settle(self);
            self.schedule(cx);
            return;
        }
        self.in_flight = true;
        let task = self
            .store
            .update(cx, |store, cx| store.edit_choir(target, draft, cx));
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            let _ = this.update(cx, |workspace, cx| {
                workspace.in_flight = false;
                match outcome {
                    Ok(_) => {
                        workspace.failed = false;
                        if let Some(generation) = generation {
                            workspace.coalescer.settle(generation);
                        }
                    }
                    Err(_) => workspace.failed = true,
                }
                // Typing during the write stays pending and goes next.
                workspace.pump(cx);
            });
        })
        .detach();
        cx.notify();
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
        self.sync_form(window, cx);
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

/// Makes the workspace redraw whenever its quick-create block changes.
fn watch_quick(
    quick: Entity<ChoirQuickCreate>,
    cx: &mut Context<ChoirsWorkspace>,
) -> Entity<ChoirQuickCreate> {
    cx.observe(&quick, |_, _, cx| cx.notify()).detach();
    quick
}
