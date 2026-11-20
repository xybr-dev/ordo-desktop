//! A small editor over an [`EditSession`]: two Fields and a reorderable list.
//!
//! It is the reference wiring for the session (what a Song or Arrangement
//! editor does) and the host the tests and the edit-session lab drive. Debug
//! selectors: `edit-title`, `edit-notes`, `edit-item-<i>-up`,
//! `edit-item-<i>-down`, `edit-save-state`.

use domain::revisions::EditPoint;
use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyBinding, ParentElement,
    Render, Styled, Subscription, Window, actions, div, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{
        button::Button,
        field::{Field, FieldEvent, FieldState},
    },
    tokens,
};

use super::{EditSession, EditSessionEvent};

actions!(edit_session, [Undo, Redo]);

/// The key context of the editor container; the Undo and Redo keys bind here.
pub const KEY_CONTEXT: &str = "EditSession";

/// Binds Ctrl+Z and Ctrl+Y to the editor-level Undo and Redo. A focused
/// text field handles both keys itself first (native Undo), so these fire only
/// when no field has focus.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-z", Undo, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-y", Redo, Some(KEY_CONTEXT)),
    ]);
}

/// The editable content of the harness.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HarnessContent {
    /// The title field.
    pub title: String,
    /// The notes field.
    pub notes: String,
    /// The reorderable list.
    pub items: Vec<String>,
}

/// The harness view.
pub struct EditHarness {
    session: Entity<EditSession<HarnessContent>>,
    title: FieldState,
    notes: FieldState,
    focus: FocusHandle,
    environment: UiEnvironment,
    _subscriptions: Vec<Subscription>,
}

impl EditHarness {
    /// A harness showing `session`'s current content.
    pub fn new(
        session: Entity<EditSession<HarnessContent>>,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = FieldState::new(window, cx);
        let notes = FieldState::new(window, cx);
        let content = session.read(cx).content().clone();
        title.set_value(content.title, window, cx);
        notes.set_value(content.notes, window, cx);

        let mut subscriptions = Vec::new();
        for (field, apply) in [
            (
                title.clone(),
                (|content: &mut HarnessContent, text: String| content.title = text)
                    as fn(&mut HarnessContent, String),
            ),
            (notes.clone(), |content, text| content.notes = text),
        ] {
            let watched = field.clone();
            let session = session.clone();
            subscriptions.push(field.subscribe(cx, move |event, cx| {
                let text = watched.value(cx).to_string();
                session.update(cx, |session, cx| match event {
                    FieldEvent::Changed => session.text_edited(|content| apply(content, text), cx),
                    FieldEvent::FocusLeft => session.focus_left(cx),
                });
            }));
        }
        subscriptions.push(cx.subscribe_in(
            &session,
            window,
            |this, session, event, window, cx| match event {
                EditSessionEvent::StateChanged => cx.notify(),
                EditSessionEvent::ContentReplaced => {
                    let content = session.read(cx).content().clone();
                    this.title.set_value(content.title, window, cx);
                    this.notes.set_value(content.notes, window, cx);
                    cx.notify();
                }
            },
        ));

        Self {
            session,
            title,
            notes,
            focus: cx.focus_handle(),
            environment,
            _subscriptions: subscriptions,
        }
    }

    /// The session this harness edits.
    pub fn session(&self) -> &Entity<EditSession<HarnessContent>> {
        &self.session
    }

    /// The title field's state, for tests that focus it.
    pub fn title(&self) -> &FieldState {
        &self.title
    }

    /// The notes field's state.
    pub fn notes(&self) -> &FieldState {
        &self.notes
    }

    /// The container's focus handle: focusing it takes focus off every field.
    pub fn container_focus(&self) -> &FocusHandle {
        &self.focus
    }

    fn move_item(&self, index: usize, down: bool, cx: &mut App) {
        self.session.update(cx, |session, cx| {
            session.structural(
                EditPoint::Structural,
                |content| {
                    let other = if down {
                        index + 1
                    } else {
                        index.wrapping_sub(1)
                    };
                    if index < content.items.len() && other < content.items.len() {
                        content.items.swap(index, other);
                    }
                },
                cx,
            );
        });
    }
}

impl Render for EditHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let (label, items) = {
            let session = self.session.read(cx);
            (
                session.save_state().label(),
                session.content().items.clone(),
            )
        };
        let environment = self.environment;
        let rows = items.into_iter().enumerate().map(|(index, item)| {
            let move_button = |down: bool| {
                let this = cx.entity();
                let (id, text) = if down {
                    ("edit-item-down", "Move down")
                } else {
                    ("edit-item-up", "Move up")
                };
                div()
                    .debug_selector(move || {
                        format!("edit-item-{index}-{}", if down { "down" } else { "up" })
                    })
                    .child(
                        Button::new((id, index), text, environment).on_activate(move |_, cx| {
                            this.update(cx, |harness, cx| harness.move_item(index, down, cx));
                        }),
                    )
            };
            div()
                .id(("edit-item", index))
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(px(160.0)).child(item))
                .child(move_button(false))
                .child(move_button(true))
        });
        div()
            .id("edit-harness")
            .track_focus(&self.focus)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &Undo, _, cx| {
                this.session.update(cx, |session, cx| session.undo(cx));
            }))
            .on_action(cx.listener(|this, _: &Redo, _, cx| {
                this.session.update(cx, |session, cx| session.redo(cx));
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .debug_selector(|| "edit-save-state".into())
                    .child(label),
            )
            .child(
                div()
                    .debug_selector(|| "edit-title".into())
                    .child(Field::new(
                        "edit-title-field",
                        "Title",
                        &self.title,
                        environment,
                    )),
            )
            .child(
                div()
                    .debug_selector(|| "edit-notes".into())
                    .child(Field::new(
                        "edit-notes-field",
                        "Notes",
                        &self.notes,
                        environment,
                    )),
            )
            .children(rows)
    }
}
