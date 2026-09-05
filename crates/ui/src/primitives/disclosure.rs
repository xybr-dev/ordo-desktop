//! A named expandable region with a persistent Button invoker.
use crate::{
    environment::UiEnvironment,
    primitives::{button::Button, focus_visible},
    tokens,
};
use gpui::{
    AnyElement, App, Context, ElementId, Entity, FocusHandle, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, Styled, Window, div, prelude::*, px,
};

#[derive(Clone)]
pub struct DisclosureState(Entity<State>);
struct State {
    expanded: bool,
    trigger: FocusHandle,
}
impl State {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            expanded: false,
            trigger: cx.focus_handle().tab_stop(true),
        }
    }
}
impl DisclosureState {
    pub fn new(cx: &mut App) -> Self {
        Self(cx.new(State::new))
    }
    pub fn keyed(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Self {
        Self(window.use_keyed_state(id, cx, |_, cx| State::new(cx)))
    }
    pub fn expanded(&self, cx: &App) -> bool {
        self.0.read(cx).expanded
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.0.read(cx).trigger.clone().focus(window, cx);
    }
    /// A pure state setter. Focus return belongs to the invoker's own
    /// activation, not to a caller collapsing a section for its own reasons —
    /// a changed selection, restored layout, or the constrained-layout rules —
    /// with the operator's focus somewhere else entirely.
    pub fn set_expanded(&self, expanded: bool, cx: &mut App) {
        self.0.update(cx, |s, cx| {
            s.expanded = expanded;
            cx.notify();
        });
    }
}

#[derive(IntoElement)]
pub struct Disclosure {
    id: ElementId,
    label: SharedString,
    state: DisclosureState,
    environment: UiEnvironment,
    surface: u32,
    content: AnyElement,
}
impl Disclosure {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        state: &DisclosureState,
        environment: UiEnvironment,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: state.clone(),
            environment,
            surface: tokens::palette(environment.appearance()).background,
            content: content.into_any_element(),
        }
    }
    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }
    fn trigger(&self, cx: &App) -> gpui_base::Button {
        let expanded = self.state.expanded(cx);
        let focus = self.state.0.read(cx).trigger.clone();
        let state = self.state.clone();
        Button::new("trigger", self.label.clone(), self.environment)
            .icon(if expanded {
                crate::assets::icon::CHEVRON_DOWN
            } else {
                crate::assets::icon::CHEVRON_RIGHT
            })
            .on_activate(move |w, cx| {
                let expanded = !state.expanded(cx);
                state.set_expanded(expanded, cx);
                // Collapsing removes the content the operator was in, so the
                // invoker they used takes focus back.
                if !expanded {
                    state.focus(w, cx);
                }
            })
            .control()
            .track_focus(&focus)
            .aria_expanded(expanded)
    }
}
impl RenderOnce for Disclosure {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let expanded = self.state.expanded(cx);
        let trigger = self.trigger(cx);
        div().id(self.id).child(
            gpui_base::Collapsible::new()
                .open(expanded)
                .flex()
                .flex_col()
                // A disclosure invoker is intrinsically sized. Stretching it
                // makes a full-width bar with its label marooned in the middle.
                .items_start()
                .gap(px(self
                    .environment
                    .preferences()
                    .scale
                    .apply(tokens::spacing::RELATED)))
                .child(focus_visible(trigger, self.environment, self.surface))
                .content(
                    div()
                        .id("region")
                        // A flex item's `min-width: auto` lets a long unbroken
                        // run size the line instead of wrapping into it, so the
                        // region needs both to fill its owner and to shrink.
                        .w_full()
                        .min_w_0()
                        .role(Role::Region)
                        .aria_label(self.label)
                        .debug_selector(|| "disclosure-content".into())
                        .child(self.content),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{Render, TestAppContext};
    struct Harness(DisclosureState);
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let env = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
            Disclosure::new(
                "details",
                "Details",
                &self.0,
                env,
                Button::new("inside", "Inspect", env),
            )
        }
    }
    #[gpui::test]
    fn keyboard_toggle_and_collapse_return_to_invoker(cx: &mut TestAppContext) {
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(|w, cx| {
            let state = DisclosureState::new(cx);
            state.focus(w, cx);
            Harness(state)
        });
        assert!(cx.debug_bounds("disclosure-content").is_none());
        let keystroke = gpui::Keystroke::parse("enter").unwrap();
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
        assert!(cx.debug_bounds("disclosure-content").is_some());
        cx.update(|w, cx| {
            use gpui::Element;
            let state = view.read(cx).0.clone();
            let env = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
            let disclosure = Disclosure::new("probe", "Details", &state, env, div());
            let element = disclosure.trigger(cx).render(w, cx).into_element();
            assert_eq!(element.a11y_role(), Some(Role::Button));
            let mut node = gpui::accesskit::Node::new(Role::Button);
            element.write_a11y_info(&mut node);
            assert_eq!(node.label(), Some("Details"));
            assert_eq!(node.is_expanded(), Some(true));
            assert!(node.supports_action(gpui::accesskit::Action::Click));
            // A caller collapsing for its own reasons only changes state.
            state.set_expanded(false, cx);
            assert!(!state.expanded(cx));
            state.set_expanded(true, cx);
        });
        assert!(cx.debug_bounds("disclosure-content").is_some());

        // Collapsing from the invoker does return focus to it.
        let keystroke = gpui::Keystroke::parse("enter").unwrap();
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
        assert!(cx.debug_bounds("disclosure-content").is_none());
        cx.update(|w, cx| {
            let state = view.read(cx).0.clone();
            assert!(!state.expanded(cx));
            assert!(state.0.read(cx).trigger.is_focused(w));
        });
    }
}
