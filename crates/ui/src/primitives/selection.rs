//! Menu actions, Select values, Combobox values, and local-view Tabs.
//! A Select is a button trigger plus a popup list; a Combobox is an editable
//! Field trigger plus the same list, filtered by what the operator types.
//! The collection is fixed for the lifetime of its state — recreate state when
//! membership changes — but a Combobox's *visible* subset is not, so a choice
//! is addressed by its index in the whole collection and never by its position
//! in the filtered view. Labels and availability belong to each owning caller.

use crate::{
    environment::UiEnvironment,
    primitives::{
        Availability,
        button::Button,
        field::{Field, FieldState},
        focus_visible,
    },
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
    /// The editable trigger a Combobox renders in place of a Button. Its text
    /// is the visible value, and while it differs from the committed one it is
    /// also the filter. `None` for every other kind.
    field: Option<FieldState>,
    /// What the Field held when the popup opened, which is what Escape puts
    /// back. Leaving any other way keeps what the operator typed.
    restore_to: SharedString,
    /// Whether an unmatched name is offered as one the operator may keep. Off
    /// by default: a closed collection must not invite a value it will refuse.
    creatable: bool,
    /// Set by the trigger's capture-phase press, which runs before the popup's
    /// own outside-press handler. Without it the press dismisses and the
    /// release reopens, so the control that opens the popup cannot close it.
    press_on_trigger: bool,
}

impl State {
    fn new(
        choices: Vec<Choice>,
        selected: Option<usize>,
        field: Option<FieldState>,
        cx: &mut Context<Self>,
    ) -> Self {
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
            // A Combobox is triggered by its Field, so the Field's own handle
            // is the trigger: everything that returns focus to the trigger
            // returns it to the caret without knowing which kind it has.
            trigger: match &field {
                Some(field) => field.focus_handle(cx),
                None => cx.focus_handle().tab_stop(true),
            },
            scroll: ScrollHandle::new(),
            field,
            restore_to: SharedString::default(),
            creatable: false,
            press_on_trigger: false,
        }
    }

    /// What the operator has typed, while it still names no choice. Arriving
    /// at one ends the filter, so the collection opens back up around it.
    fn query(&self, cx: &App) -> Option<String> {
        let typed = self.field.as_ref()?.value(cx);
        self.current(cx).is_none().then(|| typed.to_lowercase())
    }

    /// A Combobox's selection is whichever choice its Field names, and none
    /// while the text names none. That is the whole rule: a half-typed or
    /// cleared value is not a selection, and a name the collection does not
    /// contain leaves the caller to read [`SelectionState::text`] and decide —
    /// which is what `MVP-PLAN.md` asks of the Presider, Location, and Choir
    /// selectors.
    ///
    /// The name has to be whole, but not in the operator's exact case, because
    /// the filter already folds case: `english` filtering to `English` while
    /// reporting no selection would offer to add the choice it is showing, and
    /// hand a Choir or Presider caller its own duplicate to reject. Two choices
    /// that differ only in case are one name here; a collection holding both is
    /// a collection the operator cannot tell apart either.
    fn current(&self, cx: &App) -> Option<usize> {
        let Some(field) = self.field.as_ref() else {
            return self.selected;
        };
        let value = field.value(cx).to_lowercase();
        self.choices
            .iter()
            .position(|choice| choice.label.to_lowercase() == value)
    }

    /// Everything one frame needs from the collection, resolved once.
    ///
    /// Each part walks every choice and lowercases its label, and a frame wants
    /// them for the loop, for each row it draws, and for the offered name. Two
    /// reads in one frame could also disagree, since the Field is free to
    /// change between them.
    fn frame(&self, cx: &App) -> Frame {
        Frame {
            selected: self.current(cx),
            active: self.active_choice(cx),
            create: self.create_row(cx),
            visible: self.visible(cx),
        }
    }

    /// The row that offers the typed name, addressed one past the collection.
    /// It is a row the arrows and Enter reach like any other, so it needs an
    /// index; nothing else may hand this one out, and every lookup that could
    /// meet it asks `choices.get` rather than indexing.
    fn create_row(&self, cx: &App) -> Option<usize> {
        let query = self.query(cx)?;
        (self.creatable && !query.is_empty()).then_some(self.choices.len())
    }

    /// Gives up the value entirely, which is the ordinary **Not assigned**
    /// state rather than a filter that happens to match everything.
    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.active = None;
        if let Some(field) = self.field.clone() {
            field.set_value("", window, cx);
        }
        cx.notify();
    }

    /// The choices the operator can currently see, by their index in the whole
    /// collection. Everything but a filtered Combobox sees all of them.
    fn visible(&self, cx: &App) -> Vec<usize> {
        let query = self.query(cx);
        (0..self.choices.len())
            .filter(|&i| match &query {
                Some(query) => self.choices[i].label.to_lowercase().contains(query),
                None => true,
            })
            .chain(self.create_row(cx))
            .collect()
    }

    /// The choice the arrows are resting on, which a filter may have hidden
    /// since it was chosen. A hidden or ineligible one falls back to the first
    /// choice that is neither, so Enter never commits what is off screen.
    fn active_choice(&self, cx: &App) -> Option<usize> {
        let eligible = |i: &usize| {
            self.choices
                .get(*i)
                .is_none_or(|c| !c.availability.is_disabled())
        };
        let visible = self.visible(cx);
        self.active
            .filter(|i| visible.contains(i) && eligible(i))
            .or_else(|| visible.iter().copied().find(eligible))
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        if let Some(field) = self.field.as_ref() {
            self.restore_to = field.value(cx);
        }
        // The press that opened this is spent. A press that never became a
        // click — pressed on the trigger, released elsewhere — would otherwise
        // leave the flag set and swallow one later outside press.
        self.press_on_trigger = false;
        self.active = self
            .current(cx)
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
        let Some(i) = self.active_choice(cx) else {
            return;
        };
        // A Combobox keeps focus — and the caret — in its Field, so its active
        // choice is marked rather than focused. Everything else moves focus.
        if self.field.is_none() {
            self.focus[i].focus(window, cx);
        }
        if let Some(position) = self.visible(cx).iter().position(|&visible| visible == i) {
            self.scroll.scroll_to_item(position);
        }
    }

    /// One arrow press on a trigger: the first opens, later ones move.
    fn step(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.navigate(key, window, cx);
        } else {
            self.open(window, cx);
            if key == "up" {
                self.navigate("end", window, cx);
            }
        }
    }

    /// Cancels the edit: the Field goes back to what it held when the popup
    /// opened. Arrow movement is provisional, so it is discarded with it.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(field) = self.field.clone() {
            field.set_value(self.restore_to.clone(), window, cx);
        }
        self.active = None;
    }

    fn navigate(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let eligible: Vec<_> = self
            .visible(cx)
            .into_iter()
            .filter(|&i| {
                self.choices
                    .get(i)
                    .is_none_or(|c| !c.availability.is_disabled())
            })
            .collect();
        if eligible.is_empty() {
            return;
        }
        let active = self.active_choice(cx);
        let position = eligible
            .iter()
            .position(|i| Some(*i) == active)
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
        Self(cx.new(|cx| State::new(choices, selected, None, cx)))
    }
    pub fn keyed(
        id: impl Into<ElementId>,
        choices: Vec<Choice>,
        selected: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self(window.use_keyed_state(id, cx, |_, cx| State::new(choices, selected, None, cx)))
    }
    /// The same collection plus the editable Field a [`Combobox`] triggers and
    /// filters with, seeded with the committed value.
    pub fn combobox(
        choices: Vec<Choice>,
        selected: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let field = seed(&choices, selected, window, cx);
        Self(cx.new(|cx| State::new(choices, selected, Some(field), cx)))
    }
    pub fn combobox_keyed(
        id: impl Into<ElementId>,
        choices: Vec<Choice>,
        selected: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self(window.use_keyed_state(id, cx, |window, cx| {
            let field = seed(&choices, selected, window, cx);
            State::new(choices, selected, Some(field), cx)
        }))
    }
    /// What the operator has typed into a Combobox, which may be a value the
    /// collection does not contain. Accepting one is the caller's decision.
    pub fn text(&self, cx: &App) -> Option<SharedString> {
        self.0.read(cx).field.as_ref().map(|field| field.value(cx))
    }
    /// Ends an unmatched filter with a row that offers the typed name, so a
    /// name the collection does not hold reads as one the operator may keep
    /// rather than one the control refused. For the open sets `MVP-PLAN.md`
    /// asks for: a visiting Presider, a Location, a Choir not yet created.
    ///
    /// Keeping it is all the row does — the same as leaving with Tab — so the
    /// caller still reads [`Self::text`] and decides what a new name means.
    pub fn accepting_new_values(self, cx: &mut App) -> Self {
        self.0.update(cx, |state, _| state.creatable = true);
        self
    }
    pub fn selected(&self, cx: &App) -> Option<usize> {
        self.0.read(cx).current(cx)
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
        // `active` can be the offered name's index, which has no handle. No
        // Combobox calls this — it restores a tab set — but the state is
        // shared, so the lookup asks like the rest.
        if let Some(handle) = state.active.and_then(|i| state.focus.get(i)) {
            handle.clone().focus(window, cx);
        }
    }
}

/// A Combobox trigger's Field, showing the committed value it starts on.
fn seed(
    choices: &[Choice],
    selected: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) -> FieldState {
    let field = FieldState::new(window, cx);
    if let Some(choice) = selected.and_then(|i| choices.get(i)) {
        field.set_value(choice.label.clone(), window, cx);
    }
    field
}

/// One frame's view of the collection. See [`State::frame`].
struct Frame {
    visible: Vec<usize>,
    selected: Option<usize>,
    active: Option<usize>,
    create: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Menu,
    Select,
    Combobox,
    Tabs,
}
/// Reports the selection a choice or the Clear control just produced. A Menu
/// only ever invokes an action, so it only ever reports `Some`.
type OnSelect = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;
/// Commits one choice, the only path that both changes the value and tells the
/// caller. Clearing is the other, and reports `None`.
type Choose = Rc<dyn Fn(usize, &mut Window, &mut App)>;

struct Control {
    id: ElementId,
    label: SharedString,
    state: SelectionState,
    environment: UiEnvironment,
    surface: u32,
    kind: Kind,
    clearable: bool,
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
                    clearable: true,
                    on_select: None,
                })
            }
            pub fn on_surface(mut self, surface: u32) -> Self {
                self.0.surface = surface;
                self
            }
            pub fn on_select(
                mut self,
                callback: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
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
    Combobox,
    Combobox,
    "An editable Field trigger plus the Select's popup list, filtered by what the operator types."
);
impl Combobox {
    /// The Clear control inside the Field. On by default, because an empty
    /// Combobox is the ordinary **Not assigned** value; turn it off where
    /// giving up the value is not a choice the operator may make.
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.0.clearable = clearable;
        self
    }
}

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
                // The offered name is one past the collection, so this asks
                // rather than indexes, as every other lookup that can meet it
                // does. It is always eligible: there is nothing to disable.
                if event.button == gpui::MouseButton::Left
                    && state
                        .choices
                        .get(i)
                        .is_none_or(|c| !c.availability.is_disabled())
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
            .when(self.kind == Kind::Select, |s| {
                s.when_some(current.active, |s, i| {
                    s.content_focus_handle(&current.focus[i])
                })
            })
            .when(self.kind == Kind::Combobox, |s| {
                let choose = self.select();
                let active = self.state.clone();
                let revert = self.state.clone();
                // Enter and Escape reach the trigger, not the popup, because a
                // Combobox keeps focus in its Field. The qualified base root
                // already dispatches both; only what they mean is Ordo's.
                s.on_confirm(move |window, cx| {
                    if let Some(i) = active.0.read(cx).active_choice(cx) {
                        choose(i, window, cx);
                    }
                })
                .on_dismiss(move |window, cx| {
                    revert.0.update(cx, |state, cx| state.restore(window, cx))
                })
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

    /// The row that keeps the typed name. It sits in the same list, carries
    /// the same option role, and is reached the same way, because from the
    /// operator's side it is one more thing they can pick.
    fn create_item(&self, frame: &Frame, cx: &App) -> gpui::Stateful<gpui::Div> {
        let state = self.state.0.read(cx);
        let typed = state
            .field
            .as_ref()
            .map(|f| f.value(cx))
            .unwrap_or_default();
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        // ponytail: a third bare literal, with the other two.
        let label = SharedString::from(format!("Add \"{typed}\""));
        let choose = self.select();
        let index = state.choices.len();
        let item = div()
            .id("create")
            .capture_any_mouse_down(self.focus_item(index))
            .min_w_0()
            .flex_none()
            .debug_selector(|| "choice-create".into())
            .role(Role::ListBoxOption)
            .aria_label(label.clone())
            .aria_selected(false)
            .on_click(move |_, w, cx| choose(index, w, cx))
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .min_h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .text_color(rgb(palette.text))
            .when(frame.active == Some(index), |d| {
                d.aria_active_descendant()
                    .bg(rgb(palette.surface_secondary))
            })
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.surface_secondary)))
            .child(
                gpui::svg()
                    .flex_none()
                    .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                    .text_color(rgb(palette.text))
                    .data(crate::assets::icon::PLUS.as_bytes()),
            )
            .child(div().min_w_0().truncate().child(label));
        focus_visible(item, self.environment, palette.surface)
    }

    fn item(&self, i: usize, frame: &Frame, cx: &App) -> gpui::Stateful<gpui::Div> {
        // A tab renders through `tab()`, which carries the Tab role and its
        // position. Reaching a popup row with `Kind::Tabs` would silently name
        // it a ListBoxOption instead.
        debug_assert!(self.kind != Kind::Tabs, "a tab is not a popup choice");
        let state = self.state.0.read(cx);
        let choice = &state.choices[i];
        let selected = frame.selected == Some(i);
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let description = choice
            .availability
            .reason()
            .map(|r| format!("Unavailable. {r}"));
        let choose = self.select();
        // The Combobox's caret stays in its Field, so the choice the arrows
        // rest on is marked here instead of being focused.
        let active = self.kind == Kind::Combobox && frame.active == Some(i);
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
            .when(self.kind != Kind::Menu, |d| d.aria_selected(selected))
            .when_some(description.clone(), |d, text| d.aria_description(text))
            .when(active, |d| {
                d.aria_active_descendant()
                    .bg(rgb(palette.surface_secondary))
            })
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
            .when(selected && self.kind != Kind::Menu, |d| {
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
                d.cursor_pointer()
                    .when(!(selected && self.kind != Kind::Menu), |d| {
                        d.hover(|style| style.bg(rgb(palette.surface_secondary)))
                    })
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

    fn tab(&self, i: usize, frame: &Frame, cx: &App) -> gpui_base::Tab {
        let state = self.state.0.read(cx);
        let choice = &state.choices[i];
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let choose = self.select();
        let tab = gpui_base::Tab::new(i)
            .capture_any_mouse_down(self.focus_item(i))
            .debug_selector(move || format!("tab-{i}"))
            .accessibility_label(choice.label.clone())
            .selected(frame.selected == Some(i))
            .set_position(i + 1, state.choices.len())
            // Disabled and Unavailable are different states, as on the Button:
            // only Disabled is disabled, and an Unavailable tab stays
            // focusable and inspectable while its activation is simply not
            // attached.
            .disabled(choice.availability.is_disabled())
            .when(!choice.availability.is_disabled(), |t| {
                t.track_focus(&state.focus[i].clone().tab_stop(Some(i) == state.active))
            })
            .when(choice.availability.is_available(), |t| {
                t.on_click(move |_, w, cx| choose(i, w, cx))
            })
            .border_b(px(if frame.selected == Some(i) {
                scale.apply(2.0)
            } else {
                0.0
            }))
            .border_color(rgb(palette.text))
            .child(choice.label.clone())
            .px(px(scale.apply(12.0)))
            .py(px(scale.apply(8.0)))
            .when(frame.selected == Some(i), |t| t.bg(rgb(palette.selection)))
            .when(choice.availability.is_available(), |t| {
                t.cursor_pointer().when(frame.selected != Some(i), |t| {
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
        //
        // ponytail: a second untranslatable literal, and it bakes the label
        // into the trigger, so a Select cannot yet sit under a separate visible
        // label the way a Field does. Same seam as the placeholder above: give
        // the caller the composition when one needs either.
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
            // The chevron says a popup opens below, so it points at where the
            // popup appears rather than leading the label.
            .icon(crate::assets::icon::CHEVRON_DOWN)
            .icon_trailing()
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
            .capture_any_mouse_down({
                let state = self.state.clone();
                move |_, _, cx| state.0.update(cx, |s, _| s.press_on_trigger = true)
            })
            .track_focus(&handle)
            .aria_expanded(open)
            .when(self.kind == Kind::Select, |b| b.aria_value(value))
            .max_w(px(scale
                .apply(320.0)
                .min(f32::from(window.viewport_size().width) - 32.0)
                .max(0.0)))
    }

    /// The Combobox's trigger: an Ordo Field, plus the key and pointer
    /// handling a Field alone has no reason to own.
    ///
    /// The base binds Up/Down/Enter/Escape in the popup root's context, and the
    /// single-line input leaves all four to propagate, so Enter and Escape are
    /// answered through the root's own Confirm/Cancel seams. Up and Down are
    /// intercepted in the capture phase instead: the base would move focus into
    /// the popup, and this control's caret has to stay in the Field.
    fn field_trigger(&self, cx: &App) -> impl IntoElement {
        let state = self.state.clone();
        let field = state
            .0
            .read(cx)
            .field
            .clone()
            .expect("a Combobox is constructed with its Field");
        fn arrow<A: gpui::Action>(
            state: SelectionState,
            key: &'static str,
        ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
            move |_, window, cx| {
                state.0.update(cx, |s, cx| s.step(key, window, cx));
                cx.stop_propagation();
            }
        }
        div()
            .id("combobox-trigger")
            // Pressing into the Field opens the collection it filters, and
            // marks the press so the popup's outside-press handler leaves the
            // control that owns it alone.
            .capture_any_mouse_down({
                let state = state.clone();
                move |_, window, cx| {
                    state.0.update(cx, |s, cx| {
                        if s.open {
                            // The open popup's outside-press handler is about
                            // to see this press, and it belongs to the trigger.
                            // Claiming it while nothing is open would leave the
                            // flag to swallow a later dismissal instead.
                            s.press_on_trigger = true;
                        } else {
                            s.open(window, cx);
                        }
                    })
                }
            })
            .capture_action(arrow::<gpui_base::actions::SelectUp>(state.clone(), "up"))
            .capture_action(arrow::<gpui_base::actions::SelectDown>(
                state.clone(),
                "down",
            ))
            .capture_key_down({
                let state = state.clone();
                move |event, window, cx| {
                    let open = state.is_open(cx);
                    if event.keystroke.key == "tab" && open {
                        // Leaving is not cancelling: the Field keeps what the
                        // operator typed, which for an open set is the point.
                        // Only Escape puts the earlier value back.
                        state.0.update(cx, |s, cx| s.close(window, cx));
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                        window.prevent_default();
                    } else if !open && event.keystroke.key_char.is_some() {
                        // Typing is a filter, and a filter with nothing to show
                        // is not one.
                        state.0.update(cx, |s, cx| s.open(window, cx));
                    }
                }
            })
            .child(
                Field::new("field", self.label.clone(), &field, self.environment)
                    // An empty Combobox is the ordinary neutral empty value,
                    // so no collection needs a pseudo-choice that spells it.
                    .not_assigned()
                    .when(self.clearable, |trigger| {
                        let state = state.clone();
                        let cleared = self.on_select.clone();
                        trigger.on_clear(move |window, cx| {
                            state.0.update(cx, |s, cx| {
                                s.clear(window, cx);
                                // Clearing is picking again, not leaving, so
                                // the collection stays open around the caret.
                                if !s.open {
                                    s.open(window, cx);
                                }
                            });
                            state.focus(window, cx);
                            if let Some(cleared) = &cleared {
                                cleared(None, window, cx);
                            }
                        })
                    })
                    .on_surface(self.surface),
            )
    }

    /// What a filter that matches nothing shows, so an empty popup does not
    /// read as a control that has broken.
    fn no_matches(&self) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        div()
            .debug_selector(|| "choice-none".into())
            .flex()
            .items_center()
            .min_h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .text_color(rgb(palette.text_muted))
            // ponytail: a bare literal, as on the Select trigger's value.
            .child("No matches")
    }

    fn select(&self) -> Choose {
        let state = self.state.clone();
        let kind = self.kind;
        let callback = self.on_select.clone();
        Rc::new(move |i, window, cx| {
            let (selectable, creating) = {
                let current = state.0.read(cx);
                let creating = current.create_row(cx) == Some(i);
                (
                    (creating
                        || current
                            .choices
                            .get(i)
                            .is_some_and(|c| c.availability.is_available()))
                        // A filter the operator cannot see through is not a
                        // collection they can commit from.
                        && current.visible(cx).contains(&i),
                    creating,
                )
            };
            if !selectable {
                return;
            }
            if creating {
                // Keeping the typed name is all this row does, so there is
                // nothing to write: closing is the whole commitment. It still
                // reports, because the operator did decide something.
                state.0.update(cx, |state, cx| state.close(window, cx));
                if let Some(callback) = &callback {
                    callback(None, window, cx);
                }
                return;
            }
            state.0.update(cx, |state, cx| {
                state.active = Some(i);
                if kind != Kind::Menu {
                    state.selected = Some(i);
                }
                if let Some(field) = state.field.clone() {
                    field.set_value(state.choices[i].label.clone(), window, cx);
                }
                if kind != Kind::Tabs {
                    state.close(window, cx);
                } else {
                    state.focus_active(window, cx);
                    cx.notify();
                }
            });
            if let Some(callback) = &callback {
                callback(Some(i), window, cx);
            }
        })
    }

    fn render(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let state = self.state.0.read(cx);
        let open = state.open;
        let choice_count = state.choices.len();
        let frame = state.frame(cx);
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
        if kind == Kind::Tabs {
            for i in 0..choice_count {
                list = list.child(self.tab(i, &frame, cx));
            }
            return self.tabs().child(list.flex_wrap()).into_any_element();
        }
        for &i in &frame.visible {
            if Some(i) == frame.create {
                list = list.child(self.create_item(&frame, cx));
            } else {
                list = list.child(self.item(i, &frame, cx));
            }
        }
        if frame.visible.is_empty() {
            list = list.child(self.no_matches());
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
                move |_, _, cx| {
                    state.0.update(cx, |s, cx| {
                        // The trigger sits outside the popup, so its own press
                        // arrives here too. That one belongs to the trigger,
                        // which toggles on release.
                        if !std::mem::take(&mut s.press_on_trigger) {
                            s.dismiss(cx);
                        }
                    })
                }
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
        let trigger_element = if kind == Kind::Combobox {
            self.field_trigger(cx).into_any_element()
        } else {
            focus_visible(self.trigger(window, cx), self.environment, self.surface)
                .into_any_element()
        };
        if kind == Kind::Select || kind == Kind::Combobox {
            // The base Select is the qualified popup root for both. Select's
            // popup key handling consumes Confirm before base can refocus a
            // now-closed option; the Combobox's arrives at the trigger.
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
        cleared: Rc<std::cell::Cell<usize>>,
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
                        clearable: true,
                        on_select: Some({
                            let activations = self.activations.clone();
                            let cleared = self.cleared.clone();
                            Rc::new(move |choice, _, _| {
                                let counter = if choice.is_some() {
                                    &activations
                                } else {
                                    &cleared
                                };
                                counter.set(counter.get() + 1)
                            })
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
        open_with(
            cx,
            kind,
            vec![
                Choice::new("English"),
                Choice::new("Disabled").availability(Availability::Disabled),
                Choice::new("Blocked").availability(Availability::unavailable("Missing")),
                Choice::new("Cebuano"),
            ],
            Some(0),
        )
    }
    fn open_with(
        cx: &mut TestAppContext,
        kind: Kind,
        choices: Vec<Choice>,
        selected: Option<usize>,
    ) -> (Entity<Harness>, &mut gpui::VisualTestContext) {
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(move |w, cx| {
            let state = if kind == Kind::Combobox {
                SelectionState::combobox(choices, selected, w, cx)
            } else {
                SelectionState::new(choices, selected, cx)
            };
            if kind == Kind::Tabs {
                state.0.read(cx).focus[0].clone().focus(w, cx);
            } else {
                state.focus(w, cx);
            }
            Harness {
                state,
                kind,
                activations: Rc::new(std::cell::Cell::new(0)),
                cleared: Rc::new(std::cell::Cell::new(0)),
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
                clearable: true,
                on_select: None,
                environment: UiEnvironment::new(
                    UiPreferences::default(),
                    ResolvedAppearance::Light,
                ),
                surface: 0,
            };
            assert_eq!(node(control.list()).role(), Role::Menu);
            let frame = control.state.0.read(cx).frame(cx);
            for kind in [Kind::Menu, Kind::Select, Kind::Combobox, Kind::Tabs] {
                control.kind = kind;
                for i in 0..4 {
                    let item = if kind == Kind::Tabs {
                        node(control.tab(i, &frame, cx).render(w, cx).into_element())
                    } else {
                        node(control.item(i, &frame, cx))
                    };
                    assert_eq!(
                        item.role(),
                        match kind {
                            Kind::Menu => Role::MenuItem,
                            Kind::Select | Kind::Combobox => Role::ListBoxOption,
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
        for kind in [Kind::Menu, Kind::Select, Kind::Combobox] {
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
    fn a_collection_with_nothing_eligible_opens_inert(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select] {
            let (view, cx) = open_with(
                cx,
                kind,
                vec![
                    Choice::new("One").availability(Availability::Disabled),
                    Choice::new("Two").availability(Availability::Disabled),
                ],
                Some(0),
            );
            // Nothing is eligible, so there is no active choice to focus and
            // none of the arrow, activation, or dismissal paths may reach for
            // one.
            view.read_with(cx, |v, cx| assert_eq!(v.state.0.read(cx).active, None));
            for key in ["enter", "down", "up", "home", "end", "enter", "space"] {
                press(cx, key);
            }
            cx.update(|w, cx| {
                let v = view.read(cx);
                let s = v.state.0.read(cx);
                assert!(s.open, "{kind:?} still opens");
                assert_eq!(s.active, None, "{kind:?} keeps no active choice");
                assert_eq!(s.selected, Some(0), "{kind:?} activates nothing");
                assert_eq!(v.activations.get(), 0);
                assert!(s.trigger.is_focused(w), "focus stays where it can act");
            });
            press(cx, "escape");
            view.read_with(cx, |v, cx| assert!(!v.state.is_open(cx)));
        }
    }

    #[gpui::test]
    fn a_disabled_selection_never_becomes_the_active_choice(cx: &mut TestAppContext) {
        // A caller may hand over a selection that is now Disabled — a Choir
        // that was archived since the plan was saved. The selection stands,
        // because it is what the record says, but arrows and Enter start from
        // a choice they are allowed to act on.
        let (view, cx) = open_with(
            cx,
            Kind::Select,
            vec![
                Choice::new("Archived").availability(Availability::Disabled),
                Choice::new("Blocked").availability(Availability::unavailable("Missing")),
                Choice::new("Available"),
            ],
            Some(0),
        );
        view.read_with(cx, |v, cx| {
            let s = v.state.0.read(cx);
            assert_eq!(s.selected, Some(0));
            assert_eq!(s.active, Some(1), "the first choice that may be focused");
        });
        // Down opens on that choice rather than advancing past it, and it is
        // Unavailable, so Enter there commits nothing.
        press(cx, "down");
        press(cx, "enter");
        cx.update(|w, cx| {
            let s = view.read(cx).state.0.read(cx);
            assert!(s.open);
            assert_eq!(s.active, Some(1));
            assert_eq!(s.selected, Some(0));
            assert!(s.focus[1].is_focused(w));
        });
        press(cx, "down");
        press(cx, "enter");
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(2)));
    }

    #[gpui::test]
    fn the_trigger_closes_the_popup_it_opened(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select] {
            let (view, cx) = open(cx, kind);
            press(cx, "down");
            view.read_with(cx, |v, cx| assert!(v.state.is_open(cx)));
            let trigger = cx
                .debug_bounds(if kind == Kind::Select {
                    "button-Languages: English"
                } else {
                    "button-Languages"
                })
                .unwrap();
            cx.simulate_click(trigger.center(), gpui::Modifiers::default());
            view.read_with(cx, |v, cx| {
                assert!(!v.state.is_open(cx), "{kind:?} closes from its trigger")
            });
            cx.simulate_click(trigger.center(), gpui::Modifiers::default());
            view.read_with(cx, |v, cx| {
                assert!(v.state.is_open(cx), "{kind:?} reopens from its trigger")
            });
            // A press the trigger swallowed must not also swallow the next
            // press somewhere else.
            let elsewhere = cx.debug_bounds("button-After").unwrap();
            cx.simulate_click(elsewhere.center(), gpui::Modifiers::default());
            view.read_with(cx, |v, cx| {
                assert!(!v.state.is_open(cx), "{kind:?} still dismisses");
                assert_eq!(v.elsewhere.get(), 1);
            });
        }
    }

    fn combobox(cx: &mut TestAppContext) -> (Entity<Harness>, &mut gpui::VisualTestContext) {
        open_with(
            cx,
            Kind::Combobox,
            vec![
                Choice::new("English"),
                Choice::new("Disabled").availability(Availability::Disabled),
                Choice::new("Blocked").availability(Availability::unavailable("Missing")),
                Choice::new("Cebuano"),
            ],
            None,
        )
    }

    fn caret(view: &Entity<Harness>, cx: &mut gpui::VisualTestContext) -> (bool, SharedString) {
        cx.update(|window, cx| {
            let state = view.read(cx).state.0.read(cx);
            let field = state.field.clone().unwrap();
            (field.focus_handle(cx).is_focused(window), field.value(cx))
        })
    }

    #[gpui::test]
    fn combobox_typing_filters_while_the_caret_stays_in_the_field(cx: &mut TestAppContext) {
        let (view, cx) = combobox(cx);
        press(cx, "down");
        view.read_with(cx, |v, cx| {
            assert!(v.state.is_open(cx));
            assert_eq!(v.state.0.read(cx).active, Some(0));
        });

        cx.simulate_input("ceb");
        assert_eq!(caret(&view, cx), (true, "ceb".into()));
        assert!(cx.debug_bounds("choice-3").is_some(), "the one match shows");
        assert!(
            cx.debug_bounds("choice-0").is_none(),
            "and the rest are filtered away"
        );
        // The stored active choice is now filtered out, so the arrows and Enter
        // work from the first choice the operator can actually see.
        cx.update(|_, cx| {
            let state = view.read(cx).state.0.read(cx);
            assert_eq!(state.active, Some(0));
            assert_eq!(state.active_choice(cx), Some(3));
        });

        // Arrows move the active choice; they neither type nor take the caret.
        press(cx, "down");
        press(cx, "up");
        assert_eq!(caret(&view, cx), (true, "ceb".into()));
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.0.read(cx).active_choice(cx), Some(3));
            assert_eq!(v.state.selected(cx), None, "movement commits nothing");
            assert_eq!(v.activations.get(), 0);
        });

        cx.simulate_input("zzz");
        assert!(cx.debug_bounds("choice-none").is_some());
        assert!(cx.debug_bounds("choice-3").is_none());
        press(cx, "enter");
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), None, "nothing visible to commit");
            assert!(v.state.is_open(cx));
        });
    }

    #[gpui::test]
    fn combobox_enter_commits_escape_cancels_and_tab_keeps(cx: &mut TestAppContext) {
        let (view, cx) = combobox(cx);
        press(cx, "down");
        cx.simulate_input("ceb");
        press(cx, "enter");
        assert_eq!(caret(&view, cx), (true, "Cebuano".into()));
        cx.update(|window, cx| {
            let v = view.read(cx);
            assert!(!v.state.is_open(cx));
            assert_eq!(v.state.selected(cx), Some(3));
            assert_eq!(v.activations.get(), 1);
            assert!(v.state.0.read(cx).trigger.is_focused(window));
        });

        // The Field names a choice, so it is no longer a filter: reopening on
        // that value shows the whole collection again.
        press(cx, "down");
        assert!(cx.debug_bounds("choice-0").is_some());
        // Editing away from that name deselects it. What the operator typed is
        // theirs to keep — `MVP-PLAN.md`'s Presider and Location accept a name
        // the collection does not contain — so only Escape takes it back.
        cx.simulate_input("eng");
        assert_eq!(caret(&view, cx), (true, "Cebuanoeng".into()));
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), None, "editing deselects");
            assert_eq!(v.state.text(cx), Some("Cebuanoeng".into()));
        });
        press(cx, "escape");
        assert_eq!(caret(&view, cx), (true, "Cebuano".into()));
        cx.update(|window, cx| {
            let v = view.read(cx);
            assert!(!v.state.is_open(cx));
            assert_eq!(v.state.selected(cx), Some(3), "Escape cancels the edit");
            assert!(v.state.0.read(cx).trigger.is_focused(window));
        });

        // Typing on a closed Combobox opens the collection it filters, and
        // Tab leaves with what was typed rather than cancelling it.
        cx.simulate_input("x");
        view.read_with(cx, |v, cx| assert!(v.state.is_open(cx)));
        press(cx, "tab");
        assert_eq!(caret(&view, cx).1, "Cebuanox");
        cx.update(|window, cx| {
            let v = view.read(cx);
            assert!(!v.state.is_open(cx));
            assert_eq!(v.state.selected(cx), None, "and it names no choice");
            assert!(!v.state.0.read(cx).trigger.is_focused(window), "Tab leaves");
        });
    }

    #[gpui::test]
    fn typing_a_whole_name_selects_it_without_committing_through_the_popup(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = combobox(cx);
        // Offering new values is what makes the case rule matter: a collection
        // that folds case in its filter but not in its selection would offer
        // to add the very choice it is showing.
        view.update(cx, |v, cx| {
            v.state = v.state.clone().accepting_new_values(cx);
        });
        cx.simulate_input("English");
        view.read_with(cx, |v, cx| {
            assert_eq!(
                v.state.selected(cx),
                Some(0),
                "the Field names a choice, so that choice is the selection"
            );
            assert_eq!(v.activations.get(), 0, "no popup activation happened");
        });
        // The filter folds case, so the selection does too: a name typed in a
        // hurry is the choice it names, not a new one to be created beside it.
        cx.simulate_keystrokes("ctrl-a");
        cx.simulate_input("english");
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), Some(0));
            assert_eq!(v.state.text(cx), Some("english".into()));
        });
        cx.update(|_, cx| {
            let state = view.read(cx).state.0.read(cx);
            assert_eq!(state.query(cx), None, "an arrived-at name is no filter");
            assert_eq!(state.create_row(cx), None, "and is not a new one either");
        });

        // A name the collection does not contain is not a selection, and the
        // caller reads it to decide whether to accept it.
        cx.simulate_keystrokes("ctrl-a");
        cx.simulate_input("English Choir");
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), None);
            assert_eq!(v.state.text(cx), Some("English Choir".into()));
        });
        cx.update(|_, cx| {
            let state = view.read(cx).state.0.read(cx);
            assert!(state.create_row(cx).is_some(), "this one is new");
        });
    }

    #[gpui::test]
    fn clearing_gives_up_the_value_and_keeps_the_collection_open(cx: &mut TestAppContext) {
        let (view, cx) = combobox(cx);
        press(cx, "down");
        cx.simulate_input("ceb");
        press(cx, "enter");
        view.read_with(cx, |v, cx| assert_eq!(v.state.selected(cx), Some(3)));

        let clear = cx
            .debug_bounds("button-Clear")
            .expect("a Combobox holding a value offers to clear it");
        cx.simulate_click(clear.center(), gpui::Modifiers::default());
        assert_eq!(caret(&view, cx), (true, "".into()));
        view.read_with(cx, |v, cx| {
            assert_eq!(v.state.selected(cx), None);
            assert!(
                v.state.is_open(cx),
                "clearing is picking again, not leaving"
            );
            assert_eq!(v.cleared.get(), 1, "and the caller is told");
        });
        // Nothing left to clear, so nothing offers to.
        assert!(cx.debug_bounds("button-Clear").is_none());
        // Clearing is a value, not an abandoned filter: leaving keeps it.
        press(cx, "tab");
        assert_eq!(caret(&view, cx).1, "");
    }

    #[gpui::test]
    fn an_unmatched_name_is_offered_rather_than_refused(cx: &mut TestAppContext) {
        let (view, cx) = combobox(cx);
        view.update(cx, |v, cx| {
            v.state = v.state.clone().accepting_new_values(cx);
        });
        press(cx, "down");
        cx.simulate_input("Most Rev. Juan");
        // Nothing matches, but the collection does not say so: it offers the
        // name, because keeping it is a thing the operator may do.
        assert!(cx.debug_bounds("choice-none").is_none());
        cx.update(|_, cx| {
            use gpui::{Element, accesskit};
            let control = Control {
                id: "semantics".into(),
                label: "Presider".into(),
                state: view.read(cx).state.clone(),
                kind: Kind::Combobox,
                clearable: true,
                on_select: None,
                environment: UiEnvironment::new(
                    UiPreferences::default(),
                    ResolvedAppearance::Light,
                ),
                surface: 0,
            };
            let row = control.create_item(&control.state.0.read(cx).frame(cx), cx);
            let mut node = accesskit::Node::new(row.a11y_role().expect("semantic element"));
            row.write_a11y_info(&mut node);
            // The row says what taking it does, in the operator's own words,
            // rather than reporting that the collection has nothing.
            assert_eq!(node.label(), Some(r#"Add "Most Rev. Juan""#));
            assert_eq!(node.role(), Role::ListBoxOption);
        });
        // And it is reached like any other row.
        press(cx, "down");
        cx.update(|_, cx| {
            let state = view.read(cx).state.0.read(cx);
            assert_eq!(state.active_choice(cx), Some(4), "one past the collection");
        });
        // The pointer reaches the row the same way it reaches any other.
        let offered = cx.debug_bounds("choice-create").unwrap();
        cx.simulate_click(offered.center(), gpui::Modifiers::default());
        view.read_with(cx, |v, cx| {
            assert!(!v.state.is_open(cx));
            assert_eq!(v.state.text(cx), Some("Most Rev. Juan".into()));
            assert_eq!(v.cleared.get(), 1);
        });
        // And the keyboard reaches it the same way.
        press(cx, "down");
        press(cx, "enter");
        cx.update(|window, cx| {
            let v = view.read(cx);
            assert!(!v.state.is_open(cx));
            assert_eq!(v.state.text(cx), Some("Most Rev. Juan".into()));
            assert_eq!(v.state.selected(cx), None, "a new name is not a choice");
            assert_eq!(v.cleared.get(), 2, "and the caller is told to look");
            assert!(v.state.0.read(cx).trigger.is_focused(window));
        });

        // A name the collection does hold is not a new one, and an empty
        // Combobox has no name to offer.
        press(cx, "down");
        cx.simulate_keystrokes("ctrl-a");
        cx.simulate_input("English");
        assert!(cx.debug_bounds("choice-create").is_none());
        cx.simulate_keystrokes("ctrl-a backspace");
        assert!(cx.debug_bounds("choice-create").is_none());
    }

    #[gpui::test]
    fn a_closed_collection_still_says_it_has_no_match(cx: &mut TestAppContext) {
        let (_, cx) = combobox(cx);
        press(cx, "down");
        cx.simulate_input("Most Rev. Juan");
        assert!(cx.debug_bounds("choice-create").is_none());
        assert!(cx.debug_bounds("choice-none").is_some());
    }

    #[gpui::test]
    fn a_combobox_reports_its_collapsed_semantics_and_active_choice(cx: &mut TestAppContext) {
        use gpui::{Element, accesskit};
        fn node(element: impl Element) -> accesskit::Node {
            let mut node = accesskit::Node::new(element.a11y_role().expect("semantic element"));
            element.write_a11y_info(&mut node);
            node
        }
        let (view, cx) = combobox(cx);
        press(cx, "down");
        cx.simulate_input("ceb");
        cx.update(|window, cx| {
            let control = Control {
                id: "semantics".into(),
                label: "Languages".into(),
                state: view.read(cx).state.clone(),
                kind: Kind::Combobox,
                clearable: true,
                on_select: None,
                environment: UiEnvironment::new(
                    UiPreferences::default(),
                    ResolvedAppearance::Light,
                ),
                surface: 0,
            };
            // A collapsed Combobox reports the same ComboBox root a Select
            // does; what differs is that its trigger is an editable Field.
            let root = node(control.select_root(cx).render(window, cx).into_element());
            assert_eq!(root.role(), Role::ComboBox);
            assert_eq!(root.label(), Some("Languages"));
            assert_eq!(root.is_expanded(), Some(true));
            assert_eq!(node(control.list()).role(), Role::ListBox);
            let frame = control.state.0.read(cx).frame(cx);
            for i in [0, 3] {
                let item = node(control.item(i, &frame, cx));
                assert_eq!(item.role(), Role::ListBoxOption);
                assert_eq!(item.is_selected(), Some(false));
            }
            // The choice the arrows rest on is marked as the active descendant
            // rather than focused, because the caret never leaves the Field.
            // The pin sets that through interactivity rather than the node, so
            // it is not visible to an element-node probe; the behavior test
            // covers it instead.
        });
    }

    #[gpui::test]
    fn a_popup_opened_by_pointer_dismisses_on_the_first_outside_press(cx: &mut TestAppContext) {
        for kind in [Kind::Menu, Kind::Select, Kind::Combobox] {
            let (view, cx) = open(cx, kind);
            let trigger = cx
                .debug_bounds(match kind {
                    Kind::Combobox => "field-control",
                    Kind::Select => "button-Languages: English",
                    _ => "button-Languages",
                })
                .unwrap();
            cx.simulate_click(trigger.center(), gpui::Modifiers::default());
            view.read_with(cx, |v, cx| {
                assert!(v.state.is_open(cx), "{kind:?} opens from its trigger")
            });
            // The press that opened this is spent. Claiming it as the
            // trigger's would leave it to swallow this dismissal instead.
            let elsewhere = cx.debug_bounds("button-After").unwrap();
            cx.simulate_click(elsewhere.center(), gpui::Modifiers::default());
            view.read_with(cx, |v, cx| {
                assert!(!v.state.is_open(cx), "{kind:?} dismisses on one press")
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
