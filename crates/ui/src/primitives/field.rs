//! Ordo's Field Interface: an ordinary single-line editable value.

use gpui::{
    App, ElementId, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::*, px, rgb,
};
use gpui_base::input::{Input, InputEditorStyle, InputState};
use std::rc::Rc;

use crate::{
    assets,
    environment::UiEnvironment,
    primitives::{Availability, button::Button, focus_rings, focus_visible, note},
    tokens,
};

/// What the Field's Clear control runs. What clearing means belongs to the
/// caller: emptying the text is all a plain field owes, while a Combobox also
/// has a selection to give up.
type Clear = Rc<dyn Fn(&mut Window, &mut App)>;

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
    /// entity of its own. The same key returns the same state every frame, so
    /// `value` seeds it once and every later edit — including back to empty —
    /// sticks.
    pub fn keyed(
        id: impl Into<ElementId>,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let value = value.into();
        Self {
            input: window.use_keyed_state(id, cx, |window, cx| {
                InputState::new(window, cx).default_value(value)
            }),
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
    on_clear: Option<Clear>,
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
            on_clear: None,
            environment,
            surface: tokens::palette(environment.appearance()).background,
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

    /// Shows a quiet Clear control inside the field while it holds a value it
    /// can still edit, and runs this when the operator uses it.
    pub fn on_clear(mut self, on_clear: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(on_clear));
        self
    }

    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }

    /// The Clear control, when there is something to clear and the field is
    /// allowed to clear it. A Read-only or Unavailable value stays inspectable,
    /// and emptying it is an edit like any other.
    fn clear_control(
        &self,
        disabled: bool,
        read_only: bool,
        cx: &App,
    ) -> Option<impl IntoElement + use<>> {
        let on_clear = self.on_clear.clone()?;
        let scale = self.environment.preferences().scale;
        (!disabled && !read_only && !self.state.value(cx).is_empty()).then(|| {
            focus_visible(
                Button::new("clear", "Clear", self.environment)
                    .icon(assets::icon::X)
                    .icon_only()
                    .quiet()
                    .on_activate(move |window, cx| on_clear(window, cx))
                    .control()
                    .flex_none()
                    .size(px(scale.apply(tokens::density::COMPACT_CONTROL))),
                self.environment,
                tokens::palette(self.environment.appearance()).surface,
            )
        })
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
        // Both states can be set at once, and both render a note, so the
        // description carries both too: an accessible name that mentioned only
        // the first would disagree with the screen.
        let reason = [
            self.read_only
                .as_ref()
                .map(|reason| format!("Read-only. {reason}")),
            self.availability
                .reason()
                .map(|reason| format!("{}. {reason}", Availability::UNAVAILABLE)),
        ]
        .into_iter()
        .flatten()
        .reduce(|described, also| format!("{described} {also}"));
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
                self.clear_control(disabled, read_only, cx),
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
        clear: Option<impl IntoElement>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui_base::input::InputBase {
        let scale = self.environment.preferences().scale;

        // Declarative properties pushed into the state the base owns.
        // `set_disabled` and `set_readonly` early-return when unchanged, but
        // `set_placeholder` notifies unconditionally, so that one is guarded:
        // an unguarded write here would schedule the next frame from inside
        // this one, forever.
        self.state.input.update(cx, |input, cx| {
            input.set_disabled(disabled, cx);
            input.set_readonly(read_only, cx);
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
            .aria_value(self.state.value(cx))
            .aria_placeholder(
                self.state
                    .input
                    .read(cx)
                    .presentation()
                    .placeholder()
                    .clone(),
            )
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
            .when_some(clear, |frame, clear| {
                // The control sits inside the box, so it takes the inset the
                // text would have had rather than adding one of its own.
                frame
                    .pr(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                    .child(clear)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{Context, Render, TestAppContext};

    struct Harness {
        state: FieldState,
        field: fn(Field, &FieldState) -> Field,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let environment =
                UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
            div().size_full().child((self.field)(
                Field::new("title", "Title", &self.state, environment),
                &self.state,
            ))
        }
    }

    fn open(
        cx: &mut TestAppContext,
        field: fn(Field, &FieldState) -> Field,
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
    fn field_node_exposes_name_value_placeholder_and_description(cx: &mut TestAppContext) {
        use gpui::{Element, accesskit};
        let (state, cx) = open(cx, |field, _| field);
        cx.update(|window, cx| {
            state.set_value("Gloria", window, cx);
            let environment =
                UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
            let field = Field::new("title", "Title", &state, environment);
            let element = field
                .control(
                    false,
                    true,
                    Some("Read-only. Published".into()),
                    Field::NOT_ASSIGNED.into(),
                    tokens::palette(environment.appearance()),
                    None::<gpui::Div>,
                    window,
                    cx,
                )
                .render(window, cx)
                .into_element();
            let mut node = accesskit::Node::new(element.a11y_role().unwrap());
            element.write_a11y_info(&mut node);
            assert_eq!(node.role(), accesskit::Role::TextInput);
            assert_eq!(node.label(), Some("Title"));
            assert_eq!(node.value(), Some("Gloria"));
            assert_eq!(node.placeholder(), Some("Not assigned"));
            assert_eq!(node.description(), Some("Read-only. Published"));
        });
    }

    #[gpui::test]
    fn typing_edits_the_value_and_the_field_reports_it(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, |field, _| field);

        cx.simulate_input("Kyrie");

        assert_eq!(cx.update(|_, cx| state.value(cx)), "Kyrie");
        assert!(cx.debug_bounds("field-control").is_some());
    }

    #[gpui::test]
    fn a_read_only_field_stays_selectable_and_copyable_and_says_why(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, |field, _| {
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
        let (state, cx) = open(cx, |field, _| field.not_assigned());

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
    fn a_clear_control_appears_only_where_there_is_a_value_it_may_empty(cx: &mut TestAppContext) {
        let clearable = |field: Field, state: &FieldState| {
            let state = state.clone();
            field.on_clear(move |window, cx| state.set_value("", window, cx))
        };
        let (state, cx) = open(cx, clearable);
        assert!(
            cx.debug_bounds("button-Clear").is_none(),
            "an empty field has nothing to clear"
        );

        cx.simulate_input("Gloria");
        let clear = cx
            .debug_bounds("button-Clear")
            .expect("a value may be cleared");
        cx.simulate_click(clear.center(), gpui::Modifiers::default());
        assert_eq!(cx.update(|_, cx| state.value(cx)), "");
        assert!(cx.debug_bounds("button-Clear").is_none());

        // Read-only and Unavailable stay inspectable, and emptying a value is
        // an edit like any other, so neither offers the control.
        let (state, cx) = open(cx, clearable_read_only);
        cx.update(|window, cx| state.set_value("Gloria", window, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("button-Clear").is_none());
    }

    fn clearable_read_only(field: Field, state: &FieldState) -> Field {
        let state = state.clone();
        field
            .on_clear(move |window, cx| state.set_value("", window, cx))
            .read_only("The Mass has already been celebrated")
    }

    #[gpui::test]
    fn an_unavailable_field_blocks_editing_and_explains_itself(cx: &mut TestAppContext) {
        let (state, cx) = open(cx, |field, _| {
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
