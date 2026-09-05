//! Menu actions, Select values, and local-view Tabs.
//! A Select is a button trigger plus a popup list. The editable Combobox that
//! `DESIGN.md` distinguishes from it is a separate later primitive.
//! Collections are fixed for the lifetime of their state; recreate state when
//! membership changes. Labels and availability belong to each owning caller.

use crate::{
    environment::UiEnvironment,
    primitives::{Availability, button::Button, focus_visible},
    tokens,
};
use gpui::{
    App, Context, ElementId, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce, Role,
    ScrollHandle, SharedString, Styled, Window, div, prelude::*, px, rgb,
};
use std::rc::Rc;

#[derive(Clone)]
pub struct Choice {
    pub label: SharedString,
    pub availability: Availability,
}

impl Choice {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            availability: Availability::Available,
        }
    }
    pub fn availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }
}

/// Caller-owned selection and focus, independent of any frame or appearance.
#[derive(Clone)]
pub struct SelectionState(Entity<State>);

struct State {
    choices: Vec<Choice>,
    selected: Option<usize>,
    active: Option<usize>,
    open: bool,
    trigger: FocusHandle,
    focus: Vec<FocusHandle>,
    scroll: ScrollHandle,
}

impl State {
    fn new(choices: Vec<Choice>, selected: Option<usize>, cx: &mut Context<Self>) -> Self {
        let selected = selected.filter(|&i| i < choices.len());
        let active = selected
            .filter(|&i| !choices[i].availability.is_disabled())
            .or_else(|| choices.iter().position(|c| !c.availability.is_disabled()));
        Self {
            focus: choices.iter().map(|_| cx.focus_handle()).collect(),
            choices,
            selected,
            active,
            open: false,
            trigger: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.active = self
            .selected
            .filter(|&i| !self.choices[i].availability.is_disabled())
            .or(self.active);
        self.focus_active(window, cx);
        cx.notify();
    }

    /// Closes and returns focus to the trigger. `DESIGN.md` (Focus Navigation)
    /// asks for that when a *focused* surface closes: Escape, Tab, selection.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.trigger.focus(window, cx);
        cx.notify();
    }

    /// Closes without touching focus, for the operator who pointed somewhere
    /// else. Focusing the trigger here would round-trip focus off whatever
    /// they actually clicked.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    fn focus_active(&self, window: &mut Window, cx: &mut App) {
        if let Some(i) = self.active {
            self.focus[i].focus(window, cx);
            self.scroll.scroll_to_item(i);
        }
    }

    fn navigate(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let eligible: Vec<_> = self
            .choices
            .iter()
            .enumerate()
            .filter_map(|(i, c)| (!c.availability.is_disabled()).then_some(i))
            .collect();
        if eligible.is_empty() {
            return;
        }
        let position = eligible
            .iter()
            .position(|i| Some(*i) == self.active)
            .unwrap_or(0);
        let next = match key {
            "home" => 0,
            "end" => eligible.len() - 1,
            "up" | "left" => (position + eligible.len() - 1) % eligible.len(),
            _ => (position + 1) % eligible.len(),
        };
        self.active = Some(eligible[next]);
        self.focus_active(window, cx);
        cx.notify();
    }
}

impl SelectionState {
    pub fn new(choices: Vec<Choice>, selected: Option<usize>, cx: &mut App) -> Self {
        Self(cx.new(|cx| State::new(choices, selected, cx)))
    }
    pub fn keyed(
        id: impl Into<ElementId>,
        choices: Vec<Choice>,
        selected: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self(window.use_keyed_state(id, cx, |_, cx| State::new(choices, selected, cx)))
    }
    pub fn selected(&self, cx: &App) -> Option<usize> {
        self.0.read(cx).selected
    }
    pub fn is_open(&self, cx: &App) -> bool {
        self.0.read(cx).open
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.0.read(cx).trigger.clone().focus(window, cx);
    }
    /// Restore a local tab set's last meaningful target.
    pub fn focus_active(&self, window: &mut Window, cx: &mut App) {
        let state = self.0.read(cx);
        if let Some(i) = state.active {
            state.focus[i].clone().focus(window, cx);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Menu,
    Select,
    Tabs,
}
type OnSelect = Rc<dyn Fn(usize, &mut Window, &mut App)>;

struct Control {
    id: ElementId,
    label: SharedString,
    state: SelectionState,
    environment: UiEnvironment,
    surface: u32,
    kind: Kind,
    on_select: Option<OnSelect>,
}

macro_rules! control {
    ($name:ident, $kind:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(IntoElement)]
        pub struct $name(Control);
        impl $name {
            pub fn new(
                id: impl Into<ElementId>,
                label: impl Into<SharedString>,
                state: &SelectionState,
                environment: UiEnvironment,
            ) -> Self {
                Self(Control {
                    id: id.into(),
                    label: label.into(),
                    state: state.clone(),
                    environment,
                    surface: tokens::palette(environment.appearance()).background,
                    kind: Kind::$kind,
                    on_select: None,
                })
            }
            pub fn on_surface(mut self, surface: u32) -> Self {
                self.0.surface = surface;
                self
            }
            pub fn on_select(
                mut self,
                callback: impl Fn(usize, &mut Window, &mut App) + 'static,
            ) -> Self {
                self.0.on_select = Some(Rc::new(callback));
                self
            }
        }
        impl RenderOnce for $name {
            fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
                self.0.render(window, cx)
            }
        }
    };
}

control!(
    Menu,
    Menu,
    "A popup collection of actions; selection invokes an action and returns focus."
);
control!(
    Select,
    Select,
    "A button trigger plus a popup list. Arrow focus is provisional until Enter or pointer selection."
);
control!(
    Tabs,
    Tabs,
    "Local views only. Left/Right rove focus; Enter/Space selects without moving focus to the panel."
);

impl Control {
    fn focus_item(
        &self,
        i: usize,
    ) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
        let state = self.state.clone();
        move |event, window, cx| {
            state.0.update(cx, |state, cx| {
                if event.button == gpui::MouseButton::Left
                    && !state.choices[i].availability.is_disabled()
                {
                    state.active = Some(i);
                    state.focus_active(window, cx);
                    cx.notify();
                }
            })
        }
    }

    fn tabs(&self) -> gpui_base::Tabs {
        gpui_base::Tabs::new(self.id.clone())
            .aria_label(self.label.clone())
            .text_size(px(self
                .environment
                .preferences()
                .scale
                .apply(tokens::typography::LABEL.size)))
    }
    fn select_root(&self, cx: &App) -> gpui_base::Select {
        let current = self.state.0.read(cx);
        let state = self.state.clone();
        gpui_base::Select::new(self.id.clone())
            .accessibility_label(self.label.clone())
            .open(current.open)
            .focus_handle(&current.trigger)
            .when_some(current.active, |s, i| {
                s.content_focus_handle(&current.focus[i])
            })
            .on_open_change(move |open, w, cx| {
                state.0.update(cx, |s, cx| {
                    if open {
                        s.open(w, cx);
                    } else {
                        s.close(w, cx);
                    }
                })
            })
    }

    fn list(&self) -> gpui::Stateful<gpui::Div> {
        div()
            .id("choices")
            .aria_label(self.label.clone())
            .debug_selector(|| "selection-list".into())
            .when(self.kind != Kind::Tabs, |d| {
                d.role(if self.kind == Kind::Menu {
                    Role::Menu
                } else {
                    Role::ListBox
                })
            })
    }

    fn item(&self, i: usize, cx: &App) -> gpui::Stateful<gpui::Div> {
        let state = self.state.0.read(cx);
        let choice = &state.choices[i];
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let description = choice
            .availability
            .reason()
            .map(|r| format!("Unavailable. {r}"));
        let choose = self.select();
        let item = div()
            .id(i)
            .capture_any_mouse_down(self.focus_item(i))
            .min_w_0()
            .flex_none()
            .debug_selector(move || format!("choice-{i}"))
            .when(!choice.availability.is_disabled(), |d| {
                d.track_focus(&state.focus[i].clone().tab_stop(false))
            })
            .role(if self.kind == Kind::Menu {
                Role::MenuItem
            } else {
                Role::ListBoxOption
            })
            .aria_label(choice.label.clone())
            .when(self.kind != Kind::Menu, |d| {
                d.aria_selected(state.selected == Some(i))
            })
            .when_some(description.clone(), |d, text| d.aria_description(text))
            .when(choice.availability.is_available(), |d| {
                d.on_click(move |_, w, cx| choose(i, w, cx))
            })
            // A block div pins its children to the top, so `min_h` left the
            // label riding high in the row and its hover fill. Column flex
            // centres the label and keeps an Unavailable note beneath it.
            .flex()
            .flex_col()
            .justify_center()
            .min_h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .py(px(scale.apply(4.0)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .text_color(rgb(if choice.availability.is_disabled() {
                palette.text_muted
            } else {
                palette.text
            }))
            .when(state.selected == Some(i) && self.kind != Kind::Menu, |d| {
                d.bg(rgb(palette.selection))
            })
            // An Unavailable choice stays focusable and inspectable but must
            // not advertise itself as clickable, so it gets neither. The
            // hover fill is withheld from the selected row so hovering it
            // cannot flatten the selection it is painted over.
            //
            // ponytail: DESIGN.md asks for a 120ms transition; GPUI has no
            // declarative transition at the pin, as on the Button styles.
            .when(choice.availability.is_available(), |d| {
                d.cursor_pointer().when(
                    !(state.selected == Some(i) && self.kind != Kind::Menu),
                    |d| d.hover(|style| style.bg(rgb(palette.surface_secondary))),
                )
            })
            .child(choice.label.clone())
            .when_some(description, |d, text| {
                d.child(super::note(
                    crate::assets::icon::TRIANGLE_ALERT,
                    palette.warning,
                    text,
                    "choice-unavailable",
                    self.environment,
                ))
            });
        focus_visible(item, self.environment, palette.surface)
    }

    fn tab(&self, i: usize, cx: &App) -> gpui_base::Tab {
        let state = self.state.0.read(cx);
        let choice = &state.choices[i];
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let choose = self.select();
        let tab = gpui_base::Tab::new(i)
            .capture_any_mouse_down(self.focus_item(i))
            .debug_selector(move || format!("tab-{i}"))
            .accessibility_label(choice.label.clone())
            .selected(state.selected == Some(i))
            .set_position(i + 1, state.choices.len())
            .disabled(!choice.availability.is_available())
            .when(!choice.availability.is_disabled(), |t| {
                t.track_focus(&state.focus[i].clone().tab_stop(Some(i) == state.active))
            })
            .on_click(move |_, w, cx| choose(i, w, cx))
            .border_b(px(if state.selected == Some(i) {
                scale.apply(2.0)
            } else {
                0.0
            }))
            .border_color(rgb(palette.text))
            .child(choice.label.clone())
            .px(px(scale.apply(12.0)))
            .py(px(scale.apply(8.0)))
            .when(state.selected == Some(i), |t| t.bg(rgb(palette.selection)))
            .when(choice.availability.is_available(), |t| {
                t.cursor_pointer().when(state.selected != Some(i), |t| {
                    t.hover(|style| style.bg(rgb(palette.surface_secondary)))
                })
            })
            .text_color(rgb(if choice.availability.is_disabled() {
                palette.text_muted
            } else {
                palette.text
            }))
            .when_some(choice.availability.reason(), |t, r| {
                t.aria_description(format!("Unavailable. {r}"))
                    .child(super::note(
                        crate::assets::icon::TRIANGLE_ALERT,
                        palette.warning,
                        format!("Unavailable. {r}"),
                        "choice-unavailable",
                        self.environment,
                    ))
            });
        focus_visible(tab, self.environment, self.surface)
    }

    /// The trigger, which is what carries the accessible name, expanded state,
    /// and value, so it is also what a semantics test inspects.
    fn trigger(&self, window: &Window, cx: &App) -> gpui_base::Button {
        let state = self.state.0.read(cx);
        let (open, handle) = (state.open, state.trigger.clone());
        let value = state
            .selected
            .map(|i| state.choices[i].label.clone())
            // ponytail: a bare literal until a caller needs another word.
            // DESIGN.md makes Not assigned the state word, so it is the right
            // default; a `placeholder(..)` builder is the seam when one does.
            .unwrap_or_else(|| "Not assigned".into());
        let scale = self.environment.preferences().scale;
        // The value is the visible text but not the name: `aria_value` already
        // announces it, and a name of "Language: Cebuano" would say it twice.
        let text = if self.kind == Kind::Select {
            format!("{}: {}", self.label, value)
        } else {
            self.label.to_string()
        };
        Button::new("trigger", text, self.environment)
            .name(self.label.clone())
            // ponytail: `debug_bounds` measures the box, not the glyphs, so no
            // test here can see a label painting outside it — the box was 320px
            // wide while the text ran past both edges. The visible half stays
            // with the [MANUAL] check until a harness can measure text.
            .truncate()
            .on_surface(self.surface)
            .icon(crate::assets::icon::CHEVRON_DOWN)
            .on_activate({
                let state = self.state.clone();
                move |w, cx| {
                    state.0.update(cx, |s, cx| {
                        if s.open {
                            s.close(w, cx);
                        } else {
                            s.open(w, cx);
                        }
                    })
                }
            })
            .control()
            .track_focus(&handle)
            .aria_expanded(open)
            .when(self.kind == Kind::Select, |b| b.aria_value(value))
            .h_auto()
            .min_h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .max_w(px(scale
                .apply(320.0)
                .min(f32::from(window.viewport_size().width) - 32.0)
                .max(0.0)))
    }

    fn select(&self) -> OnSelect {
        let state = self.state.clone();
        let kind = self.kind;
        let callback = self.on_select.clone();
        Rc::new(move |i, window, cx| {
            let available = state
                .0
                .read(cx)
                .choices
                .get(i)
                .is_some_and(|c| c.availability.is_available());
            if !available {
                return;
            }
            state.0.update(cx, |state, cx| {
                state.active = Some(i);
                if kind != Kind::Menu {
                    state.selected = Some(i);
                }
                if kind != Kind::Tabs {
                    state.close(window, cx);
                } else {
                    state.focus_active(window, cx);
                    cx.notify();
                }
            });
            if let Some(callback) = &callback {
                callback(i, window, cx);
            }
        })
    }

    fn render(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let state = self.state.0.read(cx);
        let open = state.open;
        let choice_count = state.choices.len();
        let scroll = state.scroll.clone();
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let kind = self.kind;
        let select = self.select();
        let mut list = self
            .list()
            .flex()
            .gap(px(scale.apply(4.0)))
            .when(kind != Kind::Tabs, |d| d.flex_col())
            .on_key_down({
                let state = self.state.clone();
                let select = select.clone();
                move |event, window, cx| {
                    let key = event.keystroke.key.as_str();
                    let arrow = if kind == Kind::Tabs {
                        matches!(key, "left" | "right")
                    } else {
                        matches!(key, "up" | "down")
                    };
                    if arrow || matches!(key, "home" | "end") {
                        state.0.update(cx, |s, cx| s.navigate(key, window, cx));
                    } else if matches!(key, "enter" | "space") {
                        if let Some(i) = state.0.read(cx).active {
                            select(i, window, cx);
                        }
                    } else if key == "escape" && kind != Kind::Tabs {
                        state.0.update(cx, |s, cx| s.close(window, cx));
                    } else if key == "tab" && kind != Kind::Tabs {
                        state.0.update(cx, |s, cx| s.close(window, cx));
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                    } else {
                        return;
                    }
                    cx.stop_propagation();
                    window.prevent_default();
                }
            });
        for i in 0..choice_count {
            if kind == Kind::Tabs {
                list = list.child(self.tab(i, cx));
            } else {
                list = list.child(self.item(i, cx));
            }
        }
        if kind == Kind::Tabs {
            return self.tabs().child(list.flex_wrap()).into_any_element();
        }

        let list = list
            .track_scroll(&scroll)
            .overflow_y_scroll()
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .p(px(scale.apply(8.0)))
            .w(px(scale
                .apply(320.0)
                .min(f32::from(window.viewport_size().width) - 32.0)
                .max(0.0)))
            .max_h(px((f32::from(window.viewport_size().height) - 32.0)
                .max(0.0)
                .min(scale.apply(240.0))))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border_strong))
            .rounded(px(scale.apply(tokens::radius::OVERLAY)))
            .shadow(vec![tokens::elevation::floating(
                self.environment.appearance(),
                scale,
            )])
            // The click reaches whatever it was aimed at. `DESIGN.md` gives
            // pointer capture to the scrim, and a Floating popup has none, so
            // swallowing the click would leave no visible reason it did
            // nothing.
            .on_mouse_down_out({
                let state = self.state.clone();
                move |_, _, cx| state.0.update(cx, |s, cx| s.dismiss(cx))
            });
        let popup = gpui::deferred(
            // `Anchored` is already positioned at the trigger's bottom-left, so
            // this offset is the deliberate gap, not the trigger's own height.
            gpui::anchored()
                .offset(gpui::point(
                    px(0.0),
                    px(scale.apply(tokens::spacing::RELATED_TIGHT)),
                ))
                .snap_to_window_with_margin(px(16.0))
                .child(list),
        )
        .with_priority(1);
        let trigger_element =
            focus_visible(self.trigger(window, cx), self.environment, self.surface);
        if kind == Kind::Select {
            // The base Select is the qualified popup root. Popup key handling
            // consumes Confirm before base can refocus a now-closed option.
            self.select_root(cx)
                .child(trigger_element)
                .when(open, |s| s.child(popup))
                .into_any_element()
        } else {
            div()
                .id(self.id)
                .child(trigger_element)
                .on_key_down({
                    let state = self.state.clone();
                    move |e, w, cx| {
                        if matches!(e.keystroke.key.as_str(), "up" | "down") {
                            state.0.update(cx, |s, cx| {
                                s.open(w, cx);
                                if e.keystroke.key == "up" {
                                    s.navigate("end", w, cx);
                                }
                            });
                            cx.stop_propagation();
                            w.prevent_default();
                        } else if e.keystroke.key == "escape" && state.is_open(cx) {
                            state.0.update(cx, |s, cx| s.close(w, cx));
                            cx.stop_propagation();
                            w.prevent_default();
                        }
                    }
                })
                .when(open, |s| s.child(popup))
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{KeyDownEvent, KeyUpEvent, Keystroke, Render, TestAppContext};

    struct Harness {
        state: SelectionState,
        kind: Kind,
        activations: Rc<std::cell::Cell<usize>>,
        elsewhere: Rc<std::cell::Cell<usize>>,
    }
    impl Render for Harness {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(
                    Control {
                        id: "test".into(),
                        label: "Languages".into(),
                        state: self.state.clone(),
                        kind: self.kind,
                        on_select: Some({
                            let activations = self.activations.clone();
                            Rc::new(move |_, _, _| activations.set(activations.get() + 1))
                        }),
                        environment: UiEnvironment::new(
                            UiPreferences::default(),
                            ResolvedAppearance::Light,
                        ),
                        surface: tokens::color::PREPARATION_SURFACE,
                    }
                    .render(window, cx),
                )
                .child(
                    Button::new(
                        "after",
                        "After",
                        UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light),
                    )
                    .on_activate({
                        let elsewhere = self.elsewhere.clone();
                        move |_, _| elsewhere.set(elsewhere.get() + 1)
                    }),
                )
        }
    }
    fn open(
        cx: &mut TestAppContext,
        kind: Kind,
    ) -> (Entity<Harness>, &mut gpui::VisualTestContext) {
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(move |w, cx| {
            let state = SelectionState::new(
                vec![
                    Choice::new("English"),
                    Choice::new("Disabled").availability(Availability::Disabled),
                    Choice::new("Blocked").availability(Availability::unavailable("Missing")),
                    Choice::new("Cebuano"),
                ],
                Some(0),
                cx,
            );
            if kind == Kind::Tabs {
                state.0.read(cx).focus[0].clone().focus(w, cx);
            } else {
                state.focus(w, cx);
            }
            Harness {
                state,
                kind,
                activations: Rc::new(std::cell::Cell::new(0)),
                elsewhere: Rc::new(std::cell::Cell::new(0)),
            }
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        (view, cx)
    }
    fn press(cx: &mut gpui::VisualTestContext, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    #[gpui::test]
    fn popup_keyboard_selection_escape_and_focus_return(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select] {
            let (view, cx) = open(cx, kind);
            for key in ["enter", "space"] {
                press(cx, key);
                view.read_with(cx, |v, cx| {
                    assert!(v.state.is_open(cx), "{kind:?} opens with {key}")
                });
                press(cx, "escape");
            }
            press(cx, "down");
            view.read_with(cx, |v, cx| assert!(v.state.is_open(cx)));
            press(cx, "down");
            view.read_with(cx, |v, cx| {
                assert_eq!(
                    v.state.0.read(cx).active,
                    Some(2),
                    "disabled is skipped, unavailable stays inspectable"
                );
                assert_eq!(v.state.selected(cx), Some(0));
            });
            press(cx, "enter");
            view.read_with(cx, |v, cx| {
                assert!(v.state.is_open(cx), "unavailable cannot select")
            });
            press(cx, "down");
            press(cx, "enter");
            cx.update(|w, cx| {
                let s = &view.read(cx).state;
                assert!(!s.is_open(cx));
                assert!(s.0.read(cx).trigger.is_focused(w));
                assert_eq!(
                    s.selected(cx),
                    Some(if kind == Kind::Select { 3 } else { 0 })
                );
            });
            press(cx, "down");
            press(cx, "home");
            press(cx, "escape");
            cx.update(|w, cx| {
                let s = &view.read(cx).state;
                assert!(!s.is_open(cx));
                assert!(s.0.read(cx).trigger.is_focused(w));
                assert_eq!(
                    s.selected(cx),
                    Some(if kind == Kind::Select { 3 } else { 0 })
                );
            });
        }
    }

    #[gpui::test]
    fn local_tabs_roving_focus_does_not_select_until_activation(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, Kind::Tabs);
        press(cx, "right");
        cx.update(|w, cx| {
            let s = view.read(cx).state.0.read(cx);
            assert_eq!(s.selected, Some(0));
            assert!(s.focus[2].is_focused(w));
        });
        press(cx, "end");
        press(cx, "space");
        cx.update(|w, cx| {
            let s = view.read(cx).state.0.read(cx);
            assert_eq!(s.selected, Some(3));
            assert!(s.focus[3].is_focused(w));
        });
        press(cx, "right");
        press(cx, "enter");
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(0)));
    }

    #[gpui::test]
    fn final_accesskit_nodes_report_roles_selection_and_blocked_actions(cx: &mut TestAppContext) {
        use gpui::{Element, accesskit};
        fn node(element: impl Element) -> accesskit::Node {
            let mut node = accesskit::Node::new(element.a11y_role().expect("semantic element"));
            element.write_a11y_info(&mut node);
            node
        }
        let (view, cx) = open(cx, Kind::Menu);
        cx.update(|w, cx| {
            let mut control = Control {
                id: "semantics".into(),
                label: "Languages".into(),
                state: view.read(cx).state.clone(),
                kind: Kind::Menu,
                on_select: None,
                environment: UiEnvironment::new(
                    UiPreferences::default(),
                    ResolvedAppearance::Light,
                ),
                surface: 0,
            };
            assert_eq!(node(control.list()).role(), Role::Menu);
            for kind in [Kind::Menu, Kind::Select, Kind::Tabs] {
                control.kind = kind;
                for i in 0..4 {
                    let item = if kind == Kind::Tabs {
                        node(control.tab(i, cx).render(w, cx).into_element())
                    } else {
                        node(control.item(i, cx))
                    };
                    assert_eq!(
                        item.role(),
                        match kind {
                            Kind::Menu => Role::MenuItem,
                            Kind::Select => Role::ListBoxOption,
                            Kind::Tabs => Role::Tab,
                        }
                    );
                    assert_eq!(
                        item.supports_action(accesskit::Action::Click),
                        i == 0 || i == 3
                    );
                    if kind != Kind::Menu {
                        assert_eq!(item.is_selected(), Some(i == 0));
                    }
                    if i == 2 {
                        assert_eq!(item.description(), Some("Unavailable. Missing"));
                    }
                    if i == 0 {
                        assert_eq!(item.label(), Some("English"));
                    }
                }
            }
            control.kind = Kind::Select;
            // The trigger names the control and carries the value once. A name
            // of "Languages: English" would have a screen reader read the
            // value twice on the collapsed control.
            let trigger = node(control.trigger(w, cx).render(w, cx).into_element());
            assert_eq!(trigger.label(), Some("Languages"));
            assert_eq!(trigger.value(), Some("English"));
            control.kind = Kind::Menu;
            assert_eq!(
                node(control.trigger(w, cx).render(w, cx).into_element()).value(),
                None
            );
            control.kind = Kind::Select;
            let root = node(control.select_root(cx).render(w, cx).into_element());
            assert_eq!(root.role(), Role::ComboBox);
            assert_eq!(root.label(), Some("Languages"));
            assert_eq!(root.is_expanded(), Some(false));
            assert_eq!(node(control.list()).role(), Role::ListBox);
            let tabs = node(control.tabs().render(w, cx).into_element());
            assert_eq!(tabs.role(), Role::TabList);
            assert_eq!(tabs.label(), Some("Languages"));
        });
    }

    #[gpui::test]
    fn popup_tab_exits_and_pointer_selection_invokes_once(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select] {
            let (view, cx) = open(cx, kind);
            press(cx, "down");
            press(cx, "tab");
            cx.update(|w, cx| {
                let s = view.read(cx).state.0.read(cx);
                assert!(!s.open);
                assert!(!s.trigger.is_focused(w), "Tab leaves popup");
            });
            view.update_in(cx, |v, w, cx| v.state.focus(w, cx));
            press(cx, "down");
            let bounds = cx.debug_bounds("choice-3").unwrap();
            cx.simulate_click(bounds.center(), gpui::Modifiers::default());
            view.read_with(cx, |v, cx| {
                assert!(!v.state.is_open(cx));
                assert_eq!(v.activations.get(), 1);
                if kind == Kind::Select {
                    assert_eq!(v.state.selected(cx), Some(3));
                }
            });
        }
    }

    #[gpui::test]
    fn pointer_dismissal_reaches_the_control_the_operator_aimed_at(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select] {
            let (view, cx) = open(cx, kind);
            press(cx, "down");
            view.read_with(cx, |v, cx| assert!(v.state.is_open(cx)));
            let elsewhere = cx.debug_bounds("button-After").unwrap();
            cx.simulate_click(elsewhere.center(), gpui::Modifiers::default());
            cx.update(|w, cx| {
                let view = view.read(cx);
                let s = view.state.0.read(cx);
                assert!(!s.open, "{kind:?} dismisses");
                assert!(
                    !s.trigger.is_focused(w),
                    "{kind:?} does not pull focus back off the clicked control"
                );
                assert_eq!(view.elsewhere.get(), 1, "{kind:?} click-through activation");
            });
        }
    }

    #[gpui::test]
    fn pointer_focus_on_unavailable_items_becomes_the_arrow_origin(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select, Kind::Tabs] {
            let (view, cx) = open(cx, kind);
            if kind != Kind::Tabs {
                press(cx, "down");
            }
            let target = cx
                .debug_bounds(if kind == Kind::Tabs {
                    "tab-2"
                } else {
                    "choice-2"
                })
                .unwrap();
            cx.simulate_click(target.center(), gpui::Modifiers::default());
            press(cx, if kind == Kind::Tabs { "right" } else { "down" });
            view.read_with(cx, |v, cx| {
                assert_eq!(v.state.0.read(cx).active, Some(3), "{kind:?}");
                assert_eq!(v.activations.get(), 0);
            });
        }
    }
}
