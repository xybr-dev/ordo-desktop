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
        // Read-only and Unavailable keep a real control, because the base
        // registers Copy and every selection action outside its editable gate:
        // taking the control away would take selecting and copying the value
        // with it. Only the mutating actions are refused. This is what the web
        // does for `readonly`, and it is what separates it from `disabled`,
        // which gives up focus, selection, and Copy together.
        let read_only = self.read_only.is_some() || !self.availability.is_available();
        let reason = self
            .read_only
            .clone()
            .map(|reason| format!("Read-only. {reason}"))
            .or_else(|| {
                self.availability
                    .reason()
                    .map(|reason| format!("{}. {reason}", Availability::UNAVAILABLE))
            });
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
            .child(self.control(
                disabled,
                read_only,
                reason,
                placeholder,
                palette,
                window,
                cx,
            ))
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
    /// The control, in whichever of its states applies.
    #[allow(clippy::too_many_arguments)]
    fn control(
        &self,
        disabled: bool,
        read_only: bool,
        reason: Option<String>,
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
            if input.presentation().is_readonly() != read_only {
                input.set_readonly(read_only, cx);
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
            // ponytail: AccessKit can represent read-only, but the pinned GPUI
            // exposes no `aria_read_only` setter any more than it does a
            // disabled one, so the state rides the description. Same upgrade
            // path as `Button::control`.
            .when_some(reason, |frame, reason| frame.aria_description(reason))
            .flex()
            .items_center()
            .w_full()
            .h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            // A field you cannot type into says so with a fainter edge, not by
            // fading its value: DESIGN.md forbids approximating Read-only by
            // dimming content that is perfectly valid.
            .border_color(rgb(match (focused, read_only) {
                (true, _) => palette.border_strong,
                (false, true) => palette.surface_secondary,
                (false, false) => palette.border,
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
    fn a_read_only_field_stays_selectable_and_copyable_and_says_why(cx: &mut TestAppContext) {
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
        // Inspection is the whole point of Read-only, so the value must still
        // reach the clipboard.
        cx.update(|window, cx| {
            state
                .input
                .update(cx, |input, cx| input.select_all(window, cx));
        });
        // The base binds Copy per platform, so the test presses what an
        // operator on this platform would actually press.
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        });
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("Gloria".to_owned()),
            "a read-only value stays selectable and copyable"
        );
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
            cx.update(|_, cx| state.input.read(cx).presentation().is_readonly()),
            "an Unavailable value stays inspectable, not editable"
        );
        assert!(cx.debug_bounds("control-unavailable").is_some());
    }
}
