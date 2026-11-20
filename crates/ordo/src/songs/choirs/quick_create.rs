//! Duplicate-safe quick-create of a Choir (MVP-PLAN 6.22).
//!
//! [`ChoirQuickCreate`] is a small reusable entity: a name field that offers
//! one of two actions for the text typed. When the name matches an existing
//! Choir (case and spacing ignored, archived Choirs included) nothing can be
//! created: it offers **Select <existing name>** instead, marked Archived when
//! that Choir is. Otherwise it offers **Add <typed name>**, which creates a
//! name-only Choir through the [`LibraryStore`] and returns the new id through
//! the `done` callback. Authored text is kept verbatim; only the comparison
//! normalizes.

use std::rc::Rc;

use domain::{
    identity::ChoirId,
    song_library::{SongLibrary, match_key},
};
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyDownEvent, ParentElement, SharedString,
    Styled, Subscription, Window, div, prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{
        button::{Button, Tone},
        field::{Field, FieldState},
        list::Badge,
    },
    tokens,
};

use crate::songs::store::LibraryStore;
use domain::song_library::ChoirDraft;

/// What the typed text offers to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Offer {
    /// A Choir with this name exists; select it rather than create another.
    Select {
        /// The existing Choir.
        id: ChoirId,
        /// Its name exactly as authored.
        name: String,
        /// Whether it is archived.
        archived: bool,
    },
    /// No Choir has this name; add one with the typed name.
    Add {
        /// The typed name, verbatim.
        name: String,
    },
}

/// The offer for `typed` over `library`, or `None` while the text is blank.
/// Every Choir is compared, archived ones included.
pub fn offer(library: &SongLibrary, typed: &str) -> Option<Offer> {
    let key = match_key(typed);
    if key.is_empty() {
        return None;
    }
    let existing = library
        .choirs()
        .filter(|choir| match_key(choir.name()) == key)
        // An active Choir wins over an archived one with the same key.
        .min_by_key(|choir| (choir.is_archived(), choir.id()));
    Some(match existing {
        Some(choir) => Offer::Select {
            id: choir.id(),
            name: choir.name().to_owned(),
            archived: choir.is_archived(),
        },
        None => Offer::Add {
            name: typed.to_owned(),
        },
    })
}

/// Called with the Choir that was selected or created.
pub type Done = Rc<dyn Fn(ChoirId, &mut App)>;
/// Called when the operator cancels.
pub type Cancel = Rc<dyn Fn(&mut App)>;

/// The inline name field and its one offered action.
pub struct ChoirQuickCreate {
    store: Entity<LibraryStore>,
    field: FieldState,
    busy: bool,
    error: Option<SharedString>,
    done: Done,
    cancel: Cancel,
    _subscriptions: Vec<Subscription>,
}

impl ChoirQuickCreate {
    /// A quick-create over `store`; the field takes focus at once.
    pub fn new(
        store: Entity<LibraryStore>,
        done: Done,
        cancel: Cancel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = FieldState::new(window, cx);
        let weak = cx.weak_entity();
        let typed = field.subscribe(cx, move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.error = None;
                cx.notify();
            });
        });
        let changed = cx.observe(&store, |_, _, cx| cx.notify());
        field.focus(window, cx);
        Self {
            store,
            field,
            busy: false,
            error: None,
            done,
            cancel,
            _subscriptions: vec![typed, changed],
        }
    }

    /// The name field, so a test or lab can type into it.
    pub fn field(&self) -> &FieldState {
        &self.field
    }

    /// What the current text offers.
    pub fn current_offer(&self, cx: &App) -> Option<Offer> {
        offer(self.store.read(cx).library(), &self.field.value(cx))
    }

    /// Runs the offered action: selects the existing Choir, or creates a
    /// name-only Choir and reports its id. A blank name does nothing.
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        match self.current_offer(cx) {
            None => {}
            Some(Offer::Select { id, .. }) => (self.done)(id, cx),
            Some(Offer::Add { name }) => {
                self.busy = true;
                self.error = None;
                let task = self.store.update(cx, |store, cx| {
                    store.create_choir(ChoirDraft::named(name), cx)
                });
                cx.spawn(async move |this, cx| {
                    let outcome = task.await;
                    let _ = this.update(cx, |this, cx| {
                        this.busy = false;
                        match outcome {
                            Ok(id) => (this.done)(id, cx),
                            Err(message) => this.error = Some(message),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    /// Abandons the quick-create.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        (self.cancel)(cx);
    }

    /// The inline block the list hosts at its top.
    pub fn render(&mut self, environment: UiEnvironment, cx: &mut Context<Self>) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let offer = self.current_offer(cx);
        let weak = cx.weak_entity();
        let act = |run: fn(&mut Self, &mut Context<Self>)| {
            let weak = weak.clone();
            move |_: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, run);
            }
        };
        let (label, archived, tone) = match &offer {
            Some(Offer::Select { name, archived, .. }) => {
                (format!("Select {name}"), *archived, Tone::Standard)
            }
            Some(Offer::Add { name }) => (format!("Add {name}"), false, Tone::Primary),
            None => ("Add".to_owned(), false, Tone::Primary),
        };
        let note: SharedString = match (&offer, &self.error) {
            (_, Some(_)) => "Couldn't save".into(),
            (Some(Offer::Select { name, archived, .. }), None) => if *archived {
                format!("{name} already exists and is archived.")
            } else {
                format!("{name} already exists.")
            }
            .into(),
            _ => "Only a name is needed now.".into(),
        };
        let mut action = Button::new("choir-quick-offer", label, environment)
            .tone(tone)
            .on_surface(palette.surface)
            .on_activate(act(|this, cx| this.submit(cx)));
        if offer.is_none() || self.busy {
            action = action.availability(ui::primitives::Availability::unavailable(
                "Type a name first",
            ));
        }
        div()
            .debug_selector(|| "choir-quick-create".into())
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .w_full()
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .bg(rgb(palette.surface))
            .border_b(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" => {
                        this.submit(cx);
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.cancel(cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(
                Field::new(
                    "choir-quick-name",
                    "New choir name",
                    &self.field,
                    environment,
                )
                .placeholder("Choir name")
                .label_hidden()
                .on_surface(palette.surface),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .child(
                        div()
                            .debug_selector(|| "choir-quick-action".into())
                            .child(action),
                    )
                    .children(archived.then(|| Badge::new("Archived", environment)))
                    .child(
                        Button::new("choir-quick-cancel", "Cancel", environment)
                            .on_surface(palette.surface)
                            .on_activate(act(|this, cx| this.cancel(cx))),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "choir-quick-note".into())
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(note),
            )
            .into_any_element()
    }
}
