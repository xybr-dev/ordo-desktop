//! A radio group: one choice out of a few, each with its own explanation.
//!
//! A Select hides its options behind a trigger; a radio group keeps every
//! option and the words that tell them apart on screen, which is what a
//! decision with consequences (an import outcome, say) needs. It reuses
//! [`SelectionState`] so a caller reads the choice the way it reads a Select's or
//! Tabs': `selected()`. Which options are Available comes from the state's
//! [`Choice`s](super::selection::Choice), so an option is disabled the way a
//! Select's is.

use crate::{
    environment::UiEnvironment,
    primitives::{
        focus_visible,
        record::Chip,
        selection::{RadioView, SelectionState},
    },
    tokens,
};
use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use std::rc::Rc;

/// One option of a [`RadioGroup`]: its label, the sentence under it and a
/// classifier beside it.
#[derive(Clone, Debug)]
pub struct RadioOption {
    /// The visible and accessible name.
    pub label: SharedString,
    /// What choosing the option does; also its accessible description.
    pub description: Option<SharedString>,
    /// A neutral chip after the label, such as "Recommended".
    pub badge: Option<SharedString>,
}

/// Reports the index the operator chose, by click, Space or an arrow.
type OnSelect = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// One Tab stop; arrows move and select (WAI radio group); no option is
/// selected until one is chosen.
///
/// The Tab stop is the chosen option, or the first Available one while none is
/// chosen. Up/Left and Down/Right (and Home/End) move to the previous or next
/// Available option, wrapping, and choose it. Disabled and Unavailable options
/// are skipped.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    name: SharedString,
    options: Vec<RadioOption>,
    state: SelectionState,
    environment: UiEnvironment,
    surface: u32,
    on_select: Option<OnSelect>,
}

impl RadioGroup {
    /// A group named `name` for assistive technology. `state` must hold one
    /// [`Choice`](super::selection::Choice) per option, in the same order, with the
    /// same labels.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        options: Vec<RadioOption>,
        state: &SelectionState,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            options,
            state: state.clone(),
            environment,
            surface: tokens::palette(environment.appearance()).background,
            on_select: None,
        }
    }

    /// The color immediately outside the group, which its focus ring separates
    /// itself from.
    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }

    /// Reports an option the operator chose through this group. Choosing from
    /// code with [`SelectionState::select`] does not call it.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The group root: the named `RadioGroup` role, plus the arrow keys.
    fn root(&self) -> gpui_base::RadioGroup {
        let state = self.state.clone();
        let on_select = self.on_select.clone();
        gpui_base::RadioGroup::new(self.id.clone())
            .aria_label(self.name.clone())
            .flex()
            .flex_col()
            .gap(px(self
                .environment
                .preferences()
                .scale
                .apply(tokens::spacing::RELATED)))
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                if !matches!(key, "up" | "down" | "left" | "right" | "home" | "end") {
                    return;
                }
                if let Some(index) = state.radio_step(key, window, cx)
                    && let Some(on_select) = &on_select
                {
                    on_select(index, window, cx);
                }
                cx.stop_propagation();
                window.prevent_default();
            })
    }

    /// Option `i`: a bordered row with its indicator, label, badge and
    /// description, carrying the `Radio` role with its checked state.
    fn radio(&self, i: usize, view: &RadioView) -> gpui_base::Radio {
        let option = &self.options[i];
        let availability = view.availability.get(i).cloned().unwrap_or_default();
        let checked = view.selected == Some(i);
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let reason = availability.reason().map(|r| format!("Unavailable. {r}"));
        let description = match (&option.description, &reason) {
            (Some(text), Some(reason)) => Some(format!("{text} {reason}")),
            (Some(text), None) => Some(text.to_string()),
            (None, reason) => reason.clone(),
        };
        let stop = match view.stop {
            Some(stop) => stop == i,
            None => i == 0,
        };
        let state = self.state.clone();
        let on_select = self.on_select.clone();
        let indicator = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .mt(px(scale.apply(2.0)))
            .size(px(scale.apply(16.0)))
            .rounded_full()
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(if checked {
                palette.primary_foreground
            } else {
                palette.border_strong
            }))
            .children(checked.then(|| {
                div()
                    .size(px(scale.apply(8.0)))
                    .rounded_full()
                    .bg(rgb(palette.primary_foreground))
            }));
        let heading = div()
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(option.label.clone())
            .children(
                option
                    .badge
                    .clone()
                    .map(|badge| Chip::new(badge, self.environment)),
            );
        let text = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(heading)
            .children(option.description.clone().map(|description| {
                div()
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(description)
            }))
            .children(reason.map(|reason| {
                div()
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .text_color(rgb(palette.warning))
                    .child(reason)
            }));
        let radio = gpui_base::Radio::new(i)
            .checked(checked)
            .disabled(availability.is_disabled())
            .accessibility_label(option.label.clone())
            .set_position(i + 1, self.options.len())
            .when_some(view.focus.get(i), |radio, handle| {
                radio.track_focus(handle).tab_stop(stop)
            })
            .when(availability.is_available(), |radio| {
                radio.on_change(move |_, _, window, cx| {
                    if state.radio_choose(i, window, cx)
                        && let Some(on_select) = &on_select
                    {
                        on_select(i, window, cx);
                    }
                })
            })
            .when_some(description, |radio, text| radio.aria_description(text))
            .debug_selector(move || format!("radio-{i}"))
            .flex()
            .items_start()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .py(px(scale.apply(tokens::spacing::RELATED)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(if checked {
                palette.primary_foreground
            } else {
                palette.border
            }))
            .when(checked, |radio| radio.bg(rgb(palette.selection)))
            .text_color(rgb(if availability.is_disabled() {
                palette.text_muted
            } else {
                palette.text
            }))
            .when(availability.is_available(), |radio| {
                radio.cursor_pointer().when(!checked, |radio| {
                    radio.hover(|style| style.bg(rgb(palette.surface_secondary)))
                })
            })
            .child(indicator)
            .child(text);
        focus_visible(radio, self.environment, self.surface)
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = self.state.radio_view(cx);
        let radios: Vec<_> = (0..self.options.len())
            .map(|i| self.radio(i, &view))
            .collect();
        self.root().children(radios)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        environment::{ResolvedAppearance, UiPreferences},
        primitives::{Availability, selection::Choice},
    };
    use gpui::{
        Context, FocusHandle, KeyDownEvent, KeyUpEvent, Keystroke, Render, Role, TestAppContext,
        Toggled, VisualTestContext, accesskit, canvas,
    };
    use std::{
        cell::RefCell,
        sync::{Arc, Mutex},
    };

    fn environment() -> UiEnvironment {
        UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light)
    }

    fn options() -> Vec<RadioOption> {
        vec![
            RadioOption {
                label: "Add as a new Arrangement".into(),
                description: Some("Keeps both renditions side by side.".into()),
                badge: Some("Recommended".into()),
            },
            RadioOption {
                label: "Update the existing Arrangement".into(),
                description: Some("Replaces it as a new revision.".into()),
                badge: None,
            },
            RadioOption {
                label: "Skip".into(),
                description: Some("Leaves the Library as it is.".into()),
                badge: None,
            },
        ]
    }

    fn choices(options: &[RadioOption], disabled: &[usize]) -> Vec<Choice> {
        options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                Choice::new(o.label.clone()).availability(if disabled.contains(&i) {
                    Availability::Disabled
                } else {
                    Availability::Available
                })
            })
            .collect()
    }

    /// A group between two ordinary Tab stops, so one stop shows as a single
    /// step across it.
    struct Harness {
        state: SelectionState,
        before: FocusHandle,
        after: FocusHandle,
        chosen: Rc<RefCell<Vec<usize>>>,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();
            div()
                .size_full()
                .flex()
                .flex_col()
                .on_action(|_: &crate::FocusNext, window, cx| window.focus_next(cx))
                .on_action(|_: &crate::FocusPrevious, window, cx| window.focus_prev(cx))
                .child(div().id("before").size(px(20.0)).track_focus(&self.before))
                .child(
                    RadioGroup::new("outcome", "Outcome", options(), &self.state, environment())
                        .on_select(move |i, _, _| chosen.borrow_mut().push(i)),
                )
                .child(div().id("after").size(px(20.0)).track_focus(&self.after))
        }
    }

    fn open<'a>(
        cx: &'a mut TestAppContext,
        disabled: &'static [usize],
    ) -> (gpui::Entity<Harness>, &'a mut VisualTestContext) {
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(move |_, cx| Harness {
            state: SelectionState::new(choices(&options(), disabled), None, cx),
            before: cx.focus_handle().tab_stop(true),
            after: cx.focus_handle().tab_stop(true),
            chosen: Rc::default(),
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        (view, cx)
    }

    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    /// Which option holds focus, if any.
    fn focused(view: &gpui::Entity<Harness>, cx: &mut VisualTestContext) -> Option<usize> {
        let state = view.read_with(cx, |v, _| v.state.clone());
        cx.update(|w, cx| {
            state
                .radio_view(cx)
                .focus
                .iter()
                .position(|handle| handle.is_focused(w))
        })
    }

    #[gpui::test]
    fn radio_group_starts_with_nothing_selected(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, &[]);
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), None, "nothing is chosen by default");
            assert!(v.chosen.borrow().is_empty());
        });
        // Tabbing in lands on the first option without choosing it.
        let before = view.read_with(cx, |v, _| v.before.clone());
        cx.update(|w, cx| before.focus(w, cx));
        press(cx, "tab");
        assert_eq!(focused(&view, cx), Some(0));
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), None, "focus alone does not choose");
            assert!(v.chosen.borrow().is_empty());
        });
    }

    #[gpui::test]
    fn radio_group_is_one_tab_stop_with_arrow_selection(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, &[]);
        cx.update(|w, cx| view.read(cx).before.clone().focus(w, cx));
        press(cx, "tab");
        assert_eq!(focused(&view, cx), Some(0));

        press(cx, "down");
        assert_eq!(focused(&view, cx), Some(1), "Down moves focus");
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(1)));
        press(cx, "right");
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(2)));
        press(cx, "down");
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), Some(0), "the arrows wrap");
        });
        press(cx, "up");
        assert_eq!(focused(&view, cx), Some(2), "Up wraps backwards");
        press(cx, "home");
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(0)));
        press(cx, "end");
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), Some(2));
            assert_eq!(*v.chosen.borrow(), vec![1, 2, 0, 2, 0, 2]);
        });

        // One Tab stop: a single Tab leaves the group, and Shift-Tab comes
        // back to the chosen option rather than to its neighbour.
        press(cx, "tab");
        assert_eq!(focused(&view, cx), None, "Tab leaves the group");
        assert!(cx.update(|w, cx| view.read(cx).after.is_focused(w)));
        press(cx, "shift-tab");
        assert_eq!(focused(&view, cx), Some(2), "back on the chosen option");
    }

    #[gpui::test]
    fn a_disabled_option_is_skipped_by_arrows(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, &[1]);
        let state = view.read_with(cx, |v, _| v.state.clone());
        cx.update(|_, cx| state.select(0, cx));
        cx.update(|w, cx| {
            view.read(cx).state.radio_view(cx).focus[0].focus(w, cx);
        });
        press(cx, "down");
        assert_eq!(
            focused(&view, cx),
            Some(2),
            "Down skips the disabled option"
        );
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(2)));
        press(cx, "up");
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), Some(0), "Up skips it too");
        });
        // A disabled option cannot be chosen from code either.
        cx.update(|_, cx| state.select(1, cx));
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(0)));
    }

    #[gpui::test]
    fn radio_roles_names_descriptions_and_checked_state(cx: &mut TestAppContext) {
        type Captured = Arc<Mutex<Vec<accesskit::Node>>>;
        struct Probe {
            state: SelectionState,
            captured: Captured,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let captured = self.captured.clone();
                let state = self.state.clone();
                canvas(
                    move |_, window, cx| {
                        let group =
                            RadioGroup::new("outcome", "Outcome", options(), &state, environment());
                        let view = state.radio_view(cx);
                        let mut nodes = Vec::new();
                        let element = group.root().render(window, cx).into_element();
                        let mut node = accesskit::Node::new(
                            element.a11y_role().expect("the group is exposed"),
                        );
                        element.write_a11y_info(&mut node);
                        nodes.push(node);
                        for i in 0..3 {
                            let element = group.radio(i, &view).render(window, cx).into_element();
                            let mut node = accesskit::Node::new(
                                element.a11y_role().expect("the option is exposed"),
                            );
                            element.write_a11y_info(&mut node);
                            nodes.push(node);
                        }
                        *captured.lock().unwrap() = nodes;
                    },
                    |_, _, _, _| {},
                )
            }
        }

        cx.update(crate::init).unwrap();
        let captured: Captured = Arc::default();
        let (_, cx) = cx.add_window_view({
            let captured = captured.clone();
            move |_, cx| Probe {
                state: SelectionState::new(choices(&options(), &[]), Some(1), cx),
                captured,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        let nodes = captured.lock().unwrap();
        let [group, first, second, third] = &nodes[..] else {
            panic!("a group and three options are captured");
        };
        assert_eq!(group.role(), Role::RadioGroup);
        assert_eq!(group.label(), Some("Outcome"));
        let expected = [
            (
                "Add as a new Arrangement",
                "Keeps both renditions side by side.",
            ),
            (
                "Update the existing Arrangement",
                "Replaces it as a new revision.",
            ),
            ("Skip", "Leaves the Library as it is."),
        ];
        for (node, (label, description)) in [first, second, third].into_iter().zip(expected) {
            assert_eq!(node.role(), Role::RadioButton);
            assert_eq!(node.label(), Some(label));
            assert_eq!(node.description(), Some(description));
        }
        assert_eq!(first.toggled(), Some(Toggled::False));
        assert_eq!(
            second.toggled(),
            Some(Toggled::True),
            "the chosen option is checked"
        );
        assert_eq!(third.toggled(), Some(Toggled::False));
        assert_eq!(second.is_selected(), Some(true));
        assert_eq!(first.position_in_set(), Some(1));
        assert_eq!(first.size_of_set(), Some(3));
    }

    #[gpui::test]
    fn a_click_chooses_an_option(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, &[]);
        assert!(
            cx.debug_bounds("chip-Recommended").is_some(),
            "the badge is a chip after the label"
        );
        let bounds = cx.debug_bounds("radio-2").expect("the option is drawn");
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), Some(2));
            assert_eq!(*v.chosen.borrow(), vec![2]);
        });
        assert_eq!(focused(&view, cx), Some(2));
    }
}
