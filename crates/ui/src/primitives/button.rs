//! Ordo's Button Interface.

use std::rc::Rc;

type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

use gpui::{
    AnyView, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::*, px, rgb, svg,
};

use crate::{
    assets,
    environment::UiEnvironment,
    primitives::{Availability, focus_visible, note},
    tokens,
};

/// How long the pointer rests on a control before its tooltip appears. Ordo
/// owns the delay rather than the base or GPUI default so a test can advance
/// exactly this far and see the tooltip the operator would see.
pub const TOOLTIP_SHOW_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

/// A labeled action.
///
/// The label is always the accessible name, including for an icon-only button,
/// where it also becomes the tooltip. Behavior — focus, Enter/Space activation,
/// and the Button role — comes from the qualified base Button; everything
/// visible here is Ordo's.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    name: Option<SharedString>,
    truncate: bool,
    icon: Option<&'static str>,
    icon_trailing: bool,
    icon_only: bool,
    quiet: bool,
    tooltip: Option<SharedString>,
    availability: Availability,
    environment: UiEnvironment,
    surface: u32,
    on_activate: Option<Activate>,
}

impl Button {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            name: None,
            truncate: false,
            icon: None,
            icon_trailing: false,
            icon_only: false,
            quiet: false,
            tooltip: None,
            availability: Availability::default(),
            environment,
            surface: tokens::palette(environment.appearance()).background,
            on_activate: None,
        }
    }

    /// The accessible name, when the visible label carries more than the
    /// control's identity. A Select trigger shows `"Language: Cebuano"` but is
    /// named `"Language"`, so its value is announced once, by `aria_value`.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Ellipsizes the visible label instead of letting it paint outside the
    /// control. Truncation is per-context — a title, a file name, and a value
    /// do not shorten the same way — so the owning control asks for it and
    /// bounds the width itself. The accessible name keeps the full text.
    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self
    }

    /// One bundled Lucide icon, from [`crate::assets::icon`].
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Places the icon after the label. An icon that describes the action
    /// leads; an icon that says a surface opens below this control — a Select
    /// or Menu chevron — trails, pointing where the popup appears. A
    /// Disclosure's chevron is a twisty and keeps the leading edge.
    pub fn icon_trailing(mut self) -> Self {
        self.icon_trailing = true;
        self
    }

    /// Renders the icon alone. The label stays the accessible name and becomes
    /// the tooltip, so the control is never nameless.
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// Subordinate treatment, for an action that sits inside another control
    /// or beside a primary one: no border and no fill of its own, so it cannot
    /// double an edge its surroundings already draw. `DESIGN.md` keeps a quiet
    /// action subordinate without making it undiscoverable, so hover and press
    /// still answer with the tonal fill.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }

    /// The color immediately behind the button, which its focus separation ring
    /// has to match. Defaults to the shell's own ground, which is what a
    /// control placed without a container sits on.
    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }

    pub fn on_activate(mut self, activate: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(activate));
        self
    }

    /// The control itself, without the explanation that may sit under it. This
    /// is the element that carries the accessible name, role, and action, so it
    /// is also what a semantics test inspects.
    pub(crate) fn control(self) -> gpui_base::Button {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let disabled = self.availability.is_disabled();
        // Nothing binds `icon_only()` to `icon()`, and a square with neither a
        // glyph nor a label is a named empty box. The label wins.
        let icon_only = self.icon_only && self.icon.is_some();
        let height = scale.apply(tokens::density::DEFAULT_CONTROL);
        let icon_size = scale.apply(tokens::icon::CONTROL_SIZE);
        let label = self.label.clone();
        let name = self.name.clone().unwrap_or_else(|| label.clone());
        // A tooltip may repeat the reason but never carry it alone, so the
        // Unavailable Cue below the control stays the real explanation. An
        // icon-only control also needs its name here, since nothing on screen
        // spells it.
        let tooltip =
            self.tooltip
                .clone()
                .or_else(|| match (self.availability.reason(), icon_only) {
                    (Some(reason), true) => {
                        Some(format!("{name}. {}. {reason}", Availability::UNAVAILABLE).into())
                    }
                    (Some(reason), false) => {
                        Some(format!("{}. {reason}", Availability::UNAVAILABLE).into())
                    }
                    (None, true) => Some(name.clone()),
                    (None, false) => None,
                });
        // Activation is dropped rather than intercepted for both blocked
        // states, so a consequentially blocked action keeps its focus and its
        // name while doing nothing.
        let activate = self
            .availability
            .is_available()
            .then_some(self.on_activate)
            .flatten();

        let glyph = self.icon.map(|icon| {
            svg()
                .debug_selector({
                    let label = label.clone();
                    move || format!("icon-{label}")
                })
                .flex_none()
                .size(px(icon_size))
                .text_color(rgb(if disabled {
                    palette.text_muted
                } else {
                    palette.text
                }))
                .data(icon.as_bytes())
                .into_any_element()
        });
        let text = (!icon_only).then({
            let label = label.clone();
            move || {
                if self.truncate {
                    // The row clips and the label ellipsizes: without both, a
                    // bare text child ignores `max_w` and paints past the box.
                    div().min_w_0().truncate().child(label).into_any_element()
                } else {
                    label.into_any_element()
                }
            }
        });
        let icon_and_label = if self.icon_trailing {
            [text, glyph]
        } else {
            [glyph, text]
        };

        gpui_base::Button::new(self.id)
            .debug_selector({
                let label = label.clone();
                move || format!("button-{label}")
            })
            .disabled(disabled)
            .accessibility_label(name)
            // ponytail: the pinned GPUI exposes no disabled flag to AccessKit
            // (see docs/qualification/GPUI-BASE-T0.3.md), so the blocked state
            // rides the description. Move it to the flag when a revision has one.
            .when_some(self.availability.reason(), |control, reason| {
                control.aria_description(format!("{}. {reason}", Availability::UNAVAILABLE))
            })
            .when_some(activate, |control, activate| {
                control.on_click(move |_, window, cx| activate(window, cx))
            })
            .when_some(tooltip, |control, tooltip| {
                control
                    .tooltip_show_delay(TOOLTIP_SHOW_DELAY)
                    .tooltip(move |_, cx| tooltip_view(tooltip.clone(), self.environment, cx))
            })
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .h(px(height))
            .when(icon_only, |control| control.w(px(height)))
            .when(!icon_only, |control| {
                control.px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            })
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .when(!self.quiet, |control| {
                control
                    .border(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.surface_secondary))
            })
            // Only a usable control reacts to the pointer, so hover is itself
            // the cheapest signal that an action is live. Hover strengthens the
            // border and pressing steps the fill to `border`, which is darker
            // on the light appearance and lighter on the dark one — the
            // direction each reads as "pressed".
            //
            // ponytail: DESIGN.md asks for a 120ms transition on both; GPUI has
            // no declarative transition, so these are instant until the motion
            // work in PR 7 gives Ordo somewhere to put one.
            .when(self.availability.is_available(), |control| {
                let quiet = self.quiet;
                control
                    .cursor_pointer()
                    // A quiet control has no border to strengthen, so the fill
                    // it does not otherwise have is what answers the pointer.
                    .hover(move |style| {
                        if quiet {
                            style.bg(rgb(palette.surface_secondary))
                        } else {
                            style.border_color(rgb(palette.border_strong))
                        }
                    })
                    .active(move |style| {
                        let style = style.bg(rgb(palette.border));
                        if quiet {
                            style
                        } else {
                            style.border_color(rgb(palette.border_strong))
                        }
                    })
            })
            // Disabled is neutral and subdued: never warning or danger color.
            .text_color(rgb(if disabled {
                palette.text_muted
            } else {
                palette.text
            }))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .when(self.truncate, |control| control.overflow_hidden())
            .children(icon_and_label.into_iter().flatten())
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let environment = self.environment;
        let surface = self.surface;
        let reason = self.availability.reason().cloned();

        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .child(focus_visible(self.control(), environment, surface))
            // The Unavailable Cue: the word, an icon, and the reason, in the
            // warning color that keeps it from reading as an inert disabled
            // control.
            .when_some(reason, |wrapper, reason| {
                wrapper.child(note(
                    assets::icon::TRIANGLE_ALERT,
                    palette.warning,
                    format!("{}. {reason}", Availability::UNAVAILABLE),
                    "control-unavailable",
                    environment,
                ))
            })
    }
}

fn tooltip_view(text: SharedString, environment: UiEnvironment, cx: &mut App) -> AnyView {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    cx.new(move |_| TooltipView {
        text: text.clone(),
        palette,
        scale,
    })
    .into()
}

struct TooltipView {
    text: SharedString,
    palette: tokens::Palette,
    scale: crate::environment::InterfaceScale,
}

impl gpui::Render for TooltipView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "control-tooltip".into())
            .px(px(self.scale.apply(tokens::spacing::RELATED)))
            .py(px(self.scale.apply(tokens::spacing::RELATED_TIGHT)))
            .rounded(px(self.scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(self.palette.border))
            .bg(rgb(self.palette.surface))
            .text_color(rgb(self.palette.text))
            .text_size(px(self.scale.apply(tokens::typography::LABEL.size)))
            .child(self.text.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{
        Context, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, Render, TestAppContext, accesskit,
        canvas,
    };
    use std::{
        cell::Cell,
        rc::Rc,
        sync::{Arc, Mutex},
    };

    fn environment() -> UiEnvironment {
        UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light)
    }

    struct Harness {
        button: Box<dyn Fn(UiEnvironment) -> Button>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child((self.button)(environment()))
        }
    }

    fn open(
        cx: &mut TestAppContext,
        button: impl Fn(UiEnvironment) -> Button + 'static,
    ) -> &mut gpui::VisualTestContext {
        cx.update(crate::init).unwrap();
        let (_, cx) = cx.add_window_view(move |_, _| Harness {
            button: Box::new(button),
        });
        cx.run_until_parked();
        cx
    }

    /// Native keyboard activation is a key press, not an action dispatch, so it
    /// has to arrive as the raw pair the platform would send.
    fn press(cx: &mut gpui::VisualTestContext, key: &str) {
        let keystroke = Keystroke::parse(key).expect("a parseable key");
        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    #[gpui::test]
    fn a_popup_chevron_trails_its_label_and_an_action_icon_leads(cx: &mut TestAppContext) {
        for trailing in [false, true] {
            let cx = open(cx, move |environment| {
                let button =
                    Button::new("open", "Language", environment).icon(assets::icon::CHEVRON_DOWN);
                if trailing {
                    button.icon_trailing()
                } else {
                    button
                }
            });
            let control = cx.debug_bounds("button-Language").expect("the button");
            let icon = cx.debug_bounds("icon-Language").expect("the icon");
            if trailing {
                assert!(
                    icon.origin.x > control.center().x,
                    "a popup chevron points at where the popup opens"
                );
            } else {
                assert!(
                    icon.origin.x < control.center().x,
                    "an icon that describes the action leads its label"
                );
            }
        }
    }

    #[gpui::test]
    fn pointer_and_keyboard_both_activate_an_available_button(cx: &mut TestAppContext) {
        let activations = Rc::new(Cell::new(0));
        let cx = open(cx, {
            let activations = activations.clone();
            move |environment| {
                let activations = activations.clone();
                Button::new("save", "Save", environment)
                    .on_activate(move |_, _| activations.set(activations.get() + 1))
            }
        });

        let bounds = cx.debug_bounds("button-Save").expect("the button renders");
        cx.simulate_click(bounds.center(), Modifiers::default());
        assert_eq!(activations.get(), 1);

        press(cx, "enter");
        press(cx, "space");
        assert_eq!(
            activations.get(),
            3,
            "Enter and Space each activate the focused button once"
        );
    }

    #[gpui::test]
    fn neither_blocked_state_activates_and_only_unavailable_explains_itself(
        cx: &mut TestAppContext,
    ) {
        for availability in [
            Availability::Disabled,
            Availability::unavailable("The Mass has no Celebration yet"),
        ] {
            let activations = Rc::new(Cell::new(0));
            let cx = open(cx, {
                let activations = activations.clone();
                let availability = availability.clone();
                move |environment| {
                    let activations = activations.clone();
                    Button::new("start", "Start", environment)
                        .availability(availability.clone())
                        .on_activate(move |_, _| activations.set(activations.get() + 1))
                }
            });

            let bounds = cx.debug_bounds("button-Start").expect("the button renders");
            cx.simulate_click(bounds.center(), Modifiers::default());

            // The whole point of the two states being distinct: a
            // consequentially blocked action stays reachable and named, so it
            // keeps its focus handle. Only an obviously inapplicable one
            // leaves the tab order. Without this the keyboard half below is
            // vacuous for Disabled, and a regression to `.disabled(true)` for
            // Unavailable would pass unnoticed.
            assert_eq!(
                cx.update(|window, cx| window.focused(cx).is_some()),
                availability.reason().is_some(),
                "{availability:?} must decide focusability, not activation"
            );

            press(cx, "enter");
            press(cx, "space");

            assert_eq!(activations.get(), 0, "{availability:?} must not activate");
            assert_eq!(
                cx.debug_bounds("control-unavailable").is_some(),
                availability.reason().is_some(),
                "only a consequentially blocked action owes a persistent explanation"
            );
        }
    }

    #[gpui::test]
    fn an_icon_only_button_still_names_itself_on_hover(cx: &mut TestAppContext) {
        let cx = open(cx, |environment| {
            Button::new("more", "More actions", environment)
                .icon(crate::assets::icon::ELLIPSIS)
                .icon_only()
        });

        assert!(
            cx.debug_bounds("control-tooltip").is_none(),
            "a tooltip is not shown before the pointer rests on the control"
        );
        let bounds = cx
            .debug_bounds("button-More actions")
            .expect("the button renders");
        cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
        cx.executor().advance_clock(TOOLTIP_SHOW_DELAY);
        cx.run_until_parked();

        assert!(
            cx.debug_bounds("control-tooltip").is_some(),
            "an icon-only button names itself through its tooltip"
        );
    }

    /// The accessible node the base Button produces once Ordo has configured
    /// it. `gpui-base`'s own tests cover the base defaults; this covers the
    /// name, description, and action Ordo is responsible for.
    #[gpui::test]
    fn every_availability_keeps_the_name_and_reports_its_own_actions(cx: &mut TestAppContext) {
        type Captured = Arc<Mutex<Vec<accesskit::Node>>>;

        struct Probe {
            captured: Captured,
        }

        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let captured = self.captured.clone();
                canvas(
                    move |_, window, cx| {
                        let nodes = [
                            Availability::Available,
                            Availability::Disabled,
                            Availability::unavailable("No output is connected"),
                        ]
                        .map(|availability| {
                            let element = Button::new("go", "Go Live", environment())
                                .availability(availability)
                                .on_activate(|_, _| {})
                                .control()
                                .render(window, cx)
                                .into_element();
                            // The role is reported separately from the node's
                            // other properties, so both halves are captured.
                            let mut node = accesskit::Node::new(
                                element.a11y_role().expect("the control is exposed"),
                            );
                            element.write_a11y_info(&mut node);
                            node
                        });
                        *captured.lock().unwrap() = nodes.to_vec();
                    },
                    |_, _, _, _| {},
                )
            }
        }

        let captured: Captured = Arc::default();
        let (_, cx) = cx.add_window_view({
            let captured = captured.clone();
            move |_, _| Probe { captured }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        let nodes = captured.lock().unwrap();
        let [available, disabled, unavailable] = &nodes[..] else {
            panic!("three nodes are captured");
        };
        for node in [available, disabled, unavailable] {
            assert_eq!(node.role(), accesskit::Role::Button);
            assert_eq!(node.label(), Some("Go Live"));
        }
        assert!(available.supports_action(accesskit::Action::Click));
        assert!(!disabled.supports_action(accesskit::Action::Click));
        assert_eq!(available.description(), None);
        assert_eq!(disabled.description(), None);
        // ponytail: the pinned GPUI has no aria-disabled setter, so the blocked
        // state is only announced as a description; see the note on `control`.
        assert_eq!(
            unavailable.description(),
            Some("Unavailable. No output is connected")
        );
    }
}
