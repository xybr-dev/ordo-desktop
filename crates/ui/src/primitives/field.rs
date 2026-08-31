//! Ordo's Field Interface: an ordinary single-line editable value.

use gpui::{
    App, ElementId, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::*, px, rgb,
};
use gpui_base::input::{Input, InputEditorStyle, InputState};

use crate::{
    assets,
    environment::UiEnvironment,
    primitives::{Availability, focus_rings, note},
    tokens,
};

/// The value, caret, selection, and edit history behind a [`Field`].
///
/// The caller owns it so the value outlives any one frame; the base editing
/// state it wraps stays private, so no `gpui-base` type crosses this Interface.
#[derive(Clone)]
pub struct FieldState {
    input: Entity<InputState>,
}

impl FieldState {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            input: cx.new(|cx| InputState::new(window, cx)),
        }
    }

    /// Window-keyed state, for a surface that renders a Field without owning an
    /// entity of its own. The same key returns the same state every frame.
    pub fn keyed(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Self {
        Self {
            input: window.use_keyed_state(id, cx, InputState::new),
        }
    }

    pub fn value(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }

    pub fn set_value(&self, value: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        self.input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).presentation().focus_handle().clone()
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle(cx), cx);
    }
}

/// A named, editable value.
///
/// Editing, selection, clipboard, and Undo/Redo come from the qualified base
/// input; the name, the states, and every visible detail are Ordo's.
#[derive(IntoElement)]
pub struct Field {
    id: ElementId,
    label: SharedString,
    state: FieldState,
    availability: Availability,
    read_only: Option<SharedString>,
    not_assigned: bool,
    environment: UiEnvironment,
    surface: u32,
}

impl Field {
    /// The ordinary neutral value `DESIGN.md` reserves for a deliberate empty
    /// choice, which is not the same thing as an Unavailable value.
    pub const NOT_ASSIGNED: &str = "Not assigned";

    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        state: &FieldState,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: state.clone(),
            availability: Availability::default(),
            read_only: None,
            not_assigned: false,
            environment,
            surface: tokens::palette(environment.appearance()).surface,
        }
    }

    pub fn availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }

    /// Inspectable but not editable, with the reason stated beside the value:
    /// `DESIGN.md` forbids approximating Read-only by fading the form.
    pub fn read_only(mut self, reason: impl Into<SharedString>) -> Self {
        self.read_only = Some(reason.into());
        self
    }

    /// Shows the neutral **Not assigned** placeholder while the value is empty.
    pub fn not_assigned(mut self) -> Self {
        self.not_assigned = true;
        self
    }

    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let disabled = self.availability.is_disabled();
        // DESIGN.md: omit editing controls where practical rather than leave an
        // inert one behind. A Read-only or Unavailable value is therefore not a
        // text box at all — no caret promising an edit, no tab stop, nothing to
        // click. Disabled keeps the native disabled control, because that is
        // the obvious affordance for an obviously inapplicable one.
        let editable = self.read_only.is_none() && self.availability.is_available();
        let placeholder = if self.not_assigned {
            SharedString::new_static(Self::NOT_ASSIGNED)
        } else {
            SharedString::default()
        };

        div()
            .flex()
            .flex_col()
            // Without this a field collapses to the width of an empty value,
            // which is no width at all.
            .min_w(px(scale.apply(MIN_WIDTH)))
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .child(
                div()
                    .text_color(rgb(palette.text_muted))
                    .child(self.label.clone()),
            )
            .child(if editable {
                self.control(disabled, placeholder, palette, window, cx)
                    .into_any_element()
            } else {
                value(self.state.value(cx), placeholder, palette, self.environment)
                    .into_any_element()
            })
            .when_some(self.read_only, |field, reason| {
                field.child(note(
                    assets::icon::INFO,
                    palette.text_muted,
                    format!("Read-only. {reason}"),
                    "field-read-only",
                    self.environment,
                ))
            })
            .when_some(self.availability.reason().cloned(), |field, reason| {
                field.child(note(
                    assets::icon::TRIANGLE_ALERT,
                    palette.warning,
                    format!("{}. {reason}", Availability::UNAVAILABLE),
                    "control-unavailable",
                    self.environment,
                ))
            })
    }
}

/// The narrowest a field may be before an empty value stops looking like one.
/// `DESIGN.md` sizes controls by height, not width, so this is Ordo's own.
const MIN_WIDTH: f32 = 240.0;

impl Field {
    /// The editable control, for the one state that has one.
    fn control(
        &self,
        disabled: bool,
        placeholder: SharedString,
        palette: tokens::Palette,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui_base::input::InputBase {
        let scale = self.environment.preferences().scale;

        // Declarative properties pushed into the state the base owns. Each
        // setter notifies, so each is guarded: an unguarded write here would
        // schedule the next frame from inside this one, forever.
        self.state.input.update(cx, |input, cx| {
            if input.presentation().is_disabled() != disabled {
                input.set_disabled(disabled, cx);
            }
            if input.presentation().placeholder() != &placeholder {
                input.set_placeholder(placeholder, window, cx);
            }
            input.set_editor_style(InputEditorStyle {
                foreground: rgb(palette.text).into(),
                muted_foreground: rgb(palette.text_muted).into(),
                background: rgb(palette.surface).into(),
                border: rgb(palette.border).into(),
                selection: rgb(palette.selection).into(),
                caret: rgb(palette.text).into(),
                ..Default::default()
            });
        });

        let focused = self.state.focus_handle(cx).is_focused(window);
        // The base input owns the focus handle, so the wrapper that carries the
        // treatment resolves keyboard modality itself instead of through
        // GPUI's `focus_visible`, which needs the element to track the handle.
        let focus_visible = focused && window.last_input_was_keyboard();

        gpui_base::input::InputBase::new(self.id.clone())
            .debug_selector(|| "field-control".into())
            .focused(focused)
            .disabled(disabled)
            .accessibility_label(self.label.clone())
            .flex()
            .items_center()
            .w_full()
            .h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(if focused {
                palette.border_strong
            } else {
                palette.border
            }))
            .bg(rgb(if disabled {
                palette.surface_secondary
            } else {
                palette.surface
            }))
            .text_color(rgb(if disabled {
                palette.text_muted
            } else {
                palette.text
            }))
            .when(focus_visible, |frame| {
                frame.shadow(focus_rings(self.environment, self.surface))
            })
            .child(Input::new(&self.state.input))
    }
}

/// A value with no editing control: the same reading position and height as the
/// control it replaces, so a form does not jump when a field becomes
/// inspectable rather than editable.
fn value(
    value: SharedString,
    placeholder: SharedString,
    palette: tokens::Palette,
    environment: UiEnvironment,
) -> impl IntoElement {
    let scale = environment.preferences().scale;
    let empty = value.is_empty();
    div()
        .debug_selector(|| "field-value".into())
        .flex()
        .items_center()
        .h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
        .text_color(rgb(if empty {
            palette.text_muted
        } else {
            palette.text
        }))
        .child(if empty { placeholder } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{Context, Render, TestAppContext};

    struct Harness {
        state: FieldState,
        field: fn(Field) -> Field,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let environment =
                UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
            div().size_full().child((self.field)(Field::new(
                "title",
                "Title",
                &self.state,
                environment,
            )))
        }
    }

    fn open(
        cx: &mut TestAppContext,
        field: fn(Field) -> Field,
    ) -> (FieldState, &mut gpui::VisualTestContext) {
        cx.update(crate::init).unwrap();
        let (harness, cx) = cx.add_window_view(move |window, cx| Harness {
            state: FieldState::new(window, cx),
            field,
        });
        let state = harness.read_with(cx, |harness, _| harness.state.clone());
        cx.update(|window, cx| state.focus(window, cx));
        cx.run_until_parked();
        (state, cx)
    }

    #[gpui::test]
    fn typing_edits_the_value_and_the_field_reports_it(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, |field| field);

        cx.simulate_input("Kyrie");

        assert_eq!(cx.update(|_, cx| state.value(cx)), "Kyrie");
        assert!(cx.debug_bounds("field-control").is_some());
    }

    #[gpui::test]
    fn a_read_only_field_shows_the_value_without_a_control_and_says_why(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, |field| {
            field.read_only("The Mass has already been celebrated")
        });
        cx.update(|window, cx| state.set_value("Gloria", window, cx));
        cx.run_until_parked();

        cx.simulate_input("edited");

        assert_eq!(
            cx.update(|_, cx| state.value(cx)),
            "Gloria",
            "the value stays readable and unedited"
        );
        assert!(
            cx.debug_bounds("field-control").is_none(),
            "DESIGN.md omits the editing control rather than leaving an inert one"
        );
        assert!(cx.debug_bounds("field-value").is_some());
        assert!(
            cx.debug_bounds("field-read-only").is_some(),
            "DESIGN.md requires the reason to be stated, not implied by fading"
        );
    }

    #[gpui::test]
    fn not_assigned_is_an_ordinary_empty_value_not_an_unavailable_one(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, Field::not_assigned);

        assert_eq!(cx.update(|_, cx| state.value(cx)), "");
        assert!(
            cx.debug_bounds("control-unavailable").is_none(),
            "Not assigned is a valid choice and owes no Unavailable Cue"
        );
        // Not assigned stays an ordinary editable field: the operator can
        // still assign one. The placeholder is the whole presentation.
        assert!(cx.debug_bounds("field-control").is_some());
        assert_eq!(
            cx.update(|_, cx| state.input.read(cx).presentation().placeholder().clone()),
            Field::NOT_ASSIGNED
        );
    }

    #[gpui::test]
    fn an_unavailable_field_blocks_editing_and_explains_itself(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, |field| {
            field.availability(Availability::unavailable("The Song file is missing"))
        });

        cx.simulate_input("edited");

        assert_eq!(cx.update(|_, cx| state.value(cx)), "");
        assert!(
            cx.debug_bounds("field-control").is_none(),
            "an Unavailable value stays inspectable, not editable"
        );
        assert!(cx.debug_bounds("control-unavailable").is_some());
    }
}
