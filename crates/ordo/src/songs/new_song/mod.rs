//! The New song panel (MVP-PLAN 6.15): create a Song work on its own, before
//! any Arrangement exists.
//!
//! [`NewSongPanel`] is an entity the Songs workspace owns while the panel is
//! open. It draws a slideover over the Songs screens, so the workspace's query,
//! filters, selection and scroll are untouched underneath it. The operator
//! types a primary title and optional identity (composer or original author,
//! aliases, usual moments); while typing, Ordo shows a collapsed count of
//! possible existing Songs ranked by the domain. Choosing one opens a nested,
//! identity-only preview with **Back** and **Use this existing Song**. Nothing
//! is ever selected or merged automatically.
//!
//! The panel decides nothing about the workspace: it reports an [`Outcome`]
//! and the workspace selects the Song and closes the surfaces.

mod form;
mod preview;
#[cfg(test)]
pub(crate) mod tests;

use std::rc::Rc;

use domain::{
    identity::SongId,
    song_library::{PossibleSong, SongDraft},
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, SharedString, Subscription,
    Task, Window,
};
use ui::{
    environment::UiEnvironment,
    overlay::{Overlay, OverlayHost},
    primitives::{
        Availability,
        disclosure::DisclosureState,
        field::{FieldEvent, FieldState},
    },
};

use crate::songs::store::LibraryStore;

pub use form::build_draft;

/// What the operator decided. The panel reports it once and does not close
/// itself: the receiver selects the Song and then closes every surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// A new Song was created.
    Created(SongId),
    /// The operator chose an existing Song instead; nothing was created.
    UseExisting(SongId),
}

/// Called with the outcome, from outside any entity update.
pub type Report = Rc<dyn Fn(Outcome, &mut Window, &mut App)>;
/// Called when the panel's surface has gone, however it was dismissed.
pub type Closed = Rc<dyn Fn(&mut App)>;

/// The panel's form state and its possible-existing-Song evidence.
pub struct NewSongPanel {
    store: Entity<LibraryStore>,
    host: OverlayHost,
    title: FieldState,
    composer: FieldState,
    aliases: FieldState,
    moments: FieldState,
    disclosure: DisclosureState,
    /// Ranked possible existing Songs for the staged draft, recomputed when a
    /// field or the library changes, never per frame.
    candidates: Vec<PossibleSong>,
    busy: bool,
    error: Option<SharedString>,
    /// The candidate whose preview is open.
    previewed: Option<SongId>,
    /// Set when the panel's surface closes, so a create that finishes later
    /// does not select a Song the operator no longer expects.
    dismissed: bool,
    /// The in-flight create. The store finishes the write whether or not this
    /// is dropped; dropping it only drops the panel's interest.
    pending: Option<Task<()>>,
    report: Report,
    _subscriptions: Vec<Subscription>,
}

impl NewSongPanel {
    /// Opens the panel as a slideover on `host` over `store`, with focus in the
    /// title. `report` hears the outcome; `closed` hears the surface go.
    pub fn open(
        store: Entity<LibraryStore>,
        host: OverlayHost,
        report: Report,
        closed: Closed,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let panel = cx.new(|cx| Self::new(store, host.clone(), report, window, cx));
        let body = panel.clone();
        let gate = panel.clone();
        let submit = panel.clone();
        let close = panel.clone();
        let overlay = Overlay::slideover(form::TITLE)
            .description(form::DESCRIPTION)
            .body(move |environment, window, cx| {
                body.update(cx, |panel, cx| panel.render_form(environment, window, cx))
            })
            .primary_gated(
                form::CREATE,
                move |cx| {
                    if gate.read(cx).can_create(cx) {
                        Availability::Available
                    } else {
                        Availability::unavailable(form::CREATE_REASON)
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
        host.open(overlay, window, cx);
        let title = panel.read(cx).title.clone();
        title.focus(window, cx);
        panel
    }

    fn new(
        store: Entity<LibraryStore>,
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
        ];
        let mut subscriptions = Vec::new();
        for field in &fields {
            let weak = cx.weak_entity();
            subscriptions.push(field.subscribe(cx, move |event, cx| {
                if event == FieldEvent::Changed {
                    let _ = weak.update(cx, |panel, cx| {
                        panel.error = None;
                        panel.recompute(cx);
                    });
                }
            }));
        }
        subscriptions.push(cx.observe(&store, |panel, _, cx| panel.recompute(cx)));
        let [title, composer, aliases, moments] = fields;
        Self {
            store,
            host,
            title,
            composer,
            aliases,
            moments,
            disclosure: DisclosureState::new(cx),
            candidates: Vec::new(),
            busy: false,
            error: None,
            previewed: None,
            dismissed: false,
            pending: None,
            report,
            _subscriptions: subscriptions,
        }
    }

    /// The Primary title field, so a test or lab can type into it.
    pub fn title_field(&self) -> &FieldState {
        &self.title
    }

    /// The Composer or original author field.
    pub fn composer_field(&self) -> &FieldState {
        &self.composer
    }

    /// The Aliases field.
    pub fn aliases_field(&self) -> &FieldState {
        &self.aliases
    }

    /// The Usual moment field.
    pub fn moments_field(&self) -> &FieldState {
        &self.moments
    }

    /// The possible existing Songs for what is typed, strongest first.
    pub fn candidates(&self) -> &[PossibleSong] {
        &self.candidates
    }

    /// Whether the possible-existing disclosure is expanded.
    pub fn candidates_expanded(&self, cx: &App) -> bool {
        self.disclosure.expanded(cx)
    }

    /// The candidate whose preview is open, if any.
    pub fn previewed(&self) -> Option<SongId> {
        self.previewed
    }

    /// Whether a create is in flight.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// The reason the last create failed, while it is shown.
    pub fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    /// The staged draft: what Create song would commit.
    pub fn draft(&self, cx: &App) -> SongDraft {
        build_draft(
            &self.title.value(cx),
            &self.composer.value(cx),
            &self.aliases.value(cx),
            &self.moments.value(cx),
        )
    }

    /// Whether Create song can be taken: a title is typed and no create is
    /// already in flight, so one activation commits once.
    pub fn can_create(&self, cx: &App) -> bool {
        !self.busy && !self.title.value(cx).trim().is_empty()
    }

    fn recompute(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft(cx);
        self.candidates = self
            .store
            .read(cx)
            .library()
            .possible_existing_songs(&draft);
        if self.candidates.is_empty() {
            // Collapsed, so it does not reappear already expanded.
            self.disclosure.set_expanded(false, cx);
        }
        cx.notify();
    }

    /// Creates the Song through the store's write queue. On success the
    /// outcome is reported; on failure the panel stays open with its form and
    /// says why. Does nothing while blank or while a create is in flight.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_create(cx) {
            return;
        }
        self.busy = true;
        self.error = None;
        let draft = self.draft(cx);
        let task = self
            .store
            .update(cx, |store, cx| store.create_song(draft, cx));
        let panel = cx.weak_entity();
        let report = self.report.clone();
        self.pending = Some(window.spawn(cx, async move |cx| {
            let outcome = task.await;
            cx.update(|window, cx| {
                let Some(panel) = panel.upgrade() else {
                    return;
                };
                let created = panel.update(cx, |panel, cx| {
                    panel.busy = false;
                    cx.notify();
                    if panel.dismissed {
                        return None;
                    }
                    match outcome {
                        Ok(id) => Some(id),
                        Err(message) => {
                            panel.error = Some(message);
                            None
                        }
                    }
                });
                if let Some(id) = created {
                    report(Outcome::Created(id), window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// The panel's surface has gone: forget the in-flight create's result.
    fn dismiss(&mut self) {
        self.dismissed = true;
        self.pending = None;
    }

    /// Opens the nested preview of a candidate over the form.
    fn preview(&mut self, song: SongId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(overlay) = preview::overlay(self, song, cx) else {
            return;
        };
        self.previewed = Some(song);
        self.host.clone().open(overlay, window, cx);
        cx.notify();
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
