//! Ordo's TextArea Interface: an ordinary multi-line editable value.
//!
//! The multi-line sibling of [`Field`](super::field::Field): the same editing
//! engine (`gpui-base`'s `TextareaState`), the same tokens, two-layer focus ring
//! and accessible label/value, wrapped so no `gpui-base` type crosses this
//! Interface. Qualification evidence is in `docs/qualification/GPUI-BASE-T2.7.md`.

use std::{cell::Cell, ops::Range, rc::Rc};

use gpui::{
    App, ElementId, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, Styled, Subscription, Window, div, prelude::*, px, rgb,
};
use gpui_base::input::{InputEditorStyle, InputEvent, Textarea, TextareaState};

use crate::{
    assets,
    environment::UiEnvironment,
    primitives::{Availability, focus_rings, note},
    tokens,
};

/// How many rows an empty TextArea shows until the caller says otherwise.
const DEFAULT_MIN_ROWS: usize = 3;
/// How many rows a TextArea grows to before it scrolls inside, by default.
const DEFAULT_MAX_ROWS: usize = 8;
/// The narrowest a TextArea may be, in logical pixels. It does not scale with
/// the Interface scale: a larger scale wraps the text onto more rows instead of
/// forcing the column wider.
const MIN_WIDTH: f32 = 240.0;

/// What a [`TextAreaState`] reports to a caller that subscribes to it.
///
/// Ordo's own vocabulary: the base input's events never cross this Interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAreaEvent {
    /// The operator changed the value (typing, paste, delete, or the field's
    /// own native Undo/Redo), once per edit. [`TextAreaState::set_value`] never
    /// reports this.
    Changed,
    /// Keyboard focus left the text area.
    FocusLeft,
}

/// The value, caret, selection, and native edit history behind a [`TextArea`].
///
/// The caller owns it so the value outlives any one frame. Every offset it
/// takes or returns is a UTF-8 byte offset.
#[derive(Clone)]
pub struct TextAreaState {
    input: Entity<TextareaState>,
    /// The row limits last pushed into the base, so a render writes them only
    /// when they changed (the base notifies on every write).
    rows: Rc<Cell<(usize, usize)>>,
}

impl TextAreaState {
    /// An empty text area. It always uses the base's auto-growing layout, which
    /// is also what lets `Tab` leave the field: the base indents on `Tab` only
    /// in its fixed-height layout.
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            input: cx.new(|cx| {
                TextareaState::new(window, cx).auto_grow(DEFAULT_MIN_ROWS, DEFAULT_MAX_ROWS)
            }),
            rows: Rc::new(Cell::new((DEFAULT_MIN_ROWS, DEFAULT_MAX_ROWS))),
        }
    }

    /// Window-keyed state, for a surface that renders a TextArea without owning
    /// an entity of its own. The same key returns the same state every frame.
    pub fn keyed(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Self {
        let id: ElementId = id.into();
        let input = window.use_keyed_state((id.clone(), "input"), cx, |window, cx| {
            TextareaState::new(window, cx).auto_grow(DEFAULT_MIN_ROWS, DEFAULT_MAX_ROWS)
        });
        let rows = window.use_keyed_state((id, "rows"), cx, |_, _| {
            Rc::new(Cell::new((DEFAULT_MIN_ROWS, DEFAULT_MAX_ROWS)))
        });
        let rows = rows.read(cx).clone();
        Self { input, rows }
    }

    /// The whole text, as typed.
    pub fn value(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }

    /// Replaces the whole text without reporting [`TextAreaEvent::Changed`], for
    /// a caller that reloads the field from its own model (after Undo or a
    /// Split).
    ///
    /// It clears the field's native Undo/Redo history, so a native `Ctrl+Z`
    /// right after cannot bring back the text from before the call, and puts
    /// the caret at the start with nothing selected.
    pub fn set_value(&self, value: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        let value: SharedString = value.into();
        self.input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    /// The caret, as a byte offset on a character boundary. With a selection it
    /// is the end the operator is extending.
    pub fn cursor(&self, cx: &App) -> usize {
        self.input.read(cx).cursor()
    }

    /// The selection as a byte range; empty (`start == end`) at the caret.
    pub fn selected_range(&self, cx: &App) -> Range<usize> {
        self.input.read(cx).selected_range()
    }

    /// Selects a byte range. A non-empty range widens to character boundaries;
    /// an empty one (a caret) clips back to the preceding boundary. Offsets
    /// past the end clip to the end.
    pub fn set_selected_range(&self, range: Range<usize>, cx: &mut App) {
        self.input
            .update(cx, |input, cx| input.set_selected_range(range, cx));
    }

    /// Calls `on` for every [`TextAreaEvent`] until the returned subscription is
    /// dropped.
    pub fn subscribe(
        &self,
        cx: &mut App,
        mut on: impl FnMut(TextAreaEvent, &mut App) + 'static,
    ) -> Subscription {
        cx.subscribe(&self.input, move |_, event: &InputEvent, cx| match event {
            InputEvent::Change => on(TextAreaEvent::Changed, cx),
            InputEvent::Blur => on(TextAreaEvent::FocusLeft, cx),
            InputEvent::PressEnter { .. } | InputEvent::Focus => {}
        })
    }

    /// Calls `on` whenever the value, caret or selection may have changed,
    /// including moves that report no [`TextAreaEvent`], until the returned
    /// subscription is dropped. It also fires for [`Self::set_value`].
    pub fn observe(&self, cx: &mut App, mut on: impl FnMut(&mut App) + 'static) -> Subscription {
        cx.observe(&self.input, move |_, cx| on(cx))
    }

    /// The handle keyboard focus lands on.
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).presentation().focus_handle().clone()
    }

    /// Moves keyboard focus into the text area.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle(cx), cx);
    }
}

/// A named, editable multi-line value.
///
/// Enter inserts a newline and never submits. `Tab` and `Shift+Tab` leave the
/// field and `Escape` is not taken, so an enclosing overlay still closes. The
/// text soft-wraps and is never ellipsized: past its tallest row count it
/// scrolls inside.
#[derive(IntoElement)]
pub struct TextArea {
    id: ElementId,
    label: SharedString,
    state: TextAreaState,
    availability: Availability,
    read_only: Option<SharedString>,
    placeholder: Option<SharedString>,
    rows: Option<(usize, usize)>,
    environment: UiEnvironment,
    surface: u32,
}

impl TextArea {
    /// A text area named `label`, which stays its accessible name.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        state: &TextAreaState,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: state.clone(),
            availability: Availability::default(),
            read_only: None,
            placeholder: None,
            rows: None,
            environment,
            surface: tokens::palette(environment.appearance()).background,
        }
    }

    /// Whether the text area can be used, and why not when it cannot.
    pub fn availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }

    /// Inspectable but not editable, with the reason stated beside the value.
    /// It stays focusable and selectable, and its text stays copyable.
    pub fn read_only(mut self, reason: impl Into<SharedString>) -> Self {
        self.read_only = Some(reason.into());
        self
    }

    /// A hint shown while empty. It is never the name: the label stays that.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Shows at least `min` rows, grows with the text up to `max` rows, then
    /// scrolls inside. `max` is raised to `min` when smaller.
    pub fn rows(mut self, min: usize, max: usize) -> Self {
        let min = min.max(1);
        self.rows = Some((min, max.max(min)));
        self
    }

    /// The color immediately outside the control, which the focus ring's
    /// separation layer matches.
    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }

    /// The description both the screen and the accessibility tree carry.
    fn reason(&self) -> Option<String> {
        [
            self.read_only
                .as_ref()
                .map(|reason| format!("Read-only. {reason}")),
            self.availability
                .reason()
                .map(|reason| format!("{}. {reason}", Availability::UNAVAILABLE)),
        ]
        .into_iter()
        .flatten()
        .reduce(|described, also| format!("{described} {also}"))
    }
}

impl TextArea {
    /// The framed control, in whichever of its states applies.
    fn control(&self, window: &mut Window, cx: &mut App) -> gpui_base::input::InputBase {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let disabled = self.availability.is_disabled();
        // Read-only and Unavailable keep a real control so selecting and
        // copying survive; only the mutating actions are refused (see Field).
        let read_only = self.read_only.is_some() || !self.availability.is_available();
        let reason = self.reason();
        let placeholder = self.placeholder.clone().unwrap_or_default();
        let rows = self.rows.unwrap_or(self.state.rows.get());

        // Declarative properties pushed into the state the base owns. The base
        // notifies on `set_placeholder` and `set_auto_grow` unconditionally, so
        // both are guarded: an unguarded write here would schedule the next
        // frame from inside this one, forever.
        let remembered = self.state.rows.clone();
        self.state.input.update(cx, |input, cx| {
            input.set_disabled(disabled, cx);
            input.set_readonly(read_only, cx);
            if input.presentation().placeholder() != &placeholder {
                input.set_placeholder(placeholder, window, cx);
            }
            if remembered.get() != rows {
                remembered.set(rows);
                input.set_auto_grow(rows.0, rows.1, cx);
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
        let focus_visible = focused && window.last_input_was_keyboard();
        let aria_placeholder = self
            .state
            .input
            .read(cx)
            .presentation()
            .placeholder()
            .clone();

        gpui_base::input::InputBase::new(self.id.clone())
            .debug_selector(|| "text-area-control".into())
            .role(Role::MultilineTextInput)
            .focused(focused)
            .disabled(disabled)
            .accessibility_label(self.label.clone())
            .aria_value(self.state.value(cx))
            .aria_placeholder(aria_placeholder)
            .when_some(reason, |frame, reason| frame.aria_description(reason))
            .w_full()
            .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
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
            .child(Textarea::new(&self.state.input))
    }
}

impl RenderOnce for TextArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let control = self.control(window, cx);

        div()
            .flex()
            .flex_col()
            .w_full()
            .min_w(px(MIN_WIDTH))
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .child(
                div()
                    .text_color(rgb(palette.text_muted))
                    .child(self.label.clone()),
            )
            .child(control)
            .when_some(self.read_only, |area, reason| {
                area.child(note(
                    assets::icon::INFO,
                    palette.text_muted,
                    format!("Read-only. {reason}"),
                    "text-area-read-only",
                    self.environment,
                ))
            })
            .when_some(self.availability.reason().cloned(), |area, reason| {
                area.child(note(
                    assets::icon::TRIANGLE_ALERT,
                    palette.warning,
                    format!("{}. {reason}", Availability::UNAVAILABLE),
                    "control-unavailable",
                    self.environment,
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FocusNext, FocusPrevious,
        environment::{InterfaceScale, ResolvedAppearance, UiPreferences},
        overlay::{Overlay, OverlayHost},
        primitives::button::Button,
    };
    use gpui::{Context, Render, TestAppContext, VisualTestContext};
    use std::cell::RefCell;

    type Log = Rc<RefCell<Vec<TextAreaEvent>>>;

    struct Harness {
        state: TextAreaState,
        configure: fn(TextArea) -> TextArea,
        width: f32,
        environment: UiEnvironment,
        _events: Subscription,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
                .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
                .child(Button::new("before", "Before", self.environment))
                .child(
                    div()
                        .w(px(self.width))
                        .child((self.configure)(TextArea::new(
                            "lyrics",
                            "Lyrics",
                            &self.state,
                            self.environment,
                        ))),
                )
                .child(Button::new("after", "After", self.environment))
        }
    }

    fn environment(scale: InterfaceScale) -> UiEnvironment {
        UiEnvironment::new(
            UiPreferences {
                scale,
                ..UiPreferences::default()
            },
            ResolvedAppearance::Light,
        )
    }

    fn open_with(
        cx: &mut TestAppContext,
        scale: InterfaceScale,
        width: f32,
        configure: fn(TextArea) -> TextArea,
    ) -> (TextAreaState, Log, &mut VisualTestContext) {
        cx.update(crate::init).unwrap();
        let log = Log::default();
        let (harness, cx) = cx.add_window_view({
            let log = log.clone();
            move |window, cx| {
                let state = TextAreaState::new(window, cx);
                let events = state.subscribe(cx, move |event, _| log.borrow_mut().push(event));
                Harness {
                    state,
                    configure,
                    width,
                    environment: environment(scale),
                    _events: events,
                }
            }
        });
        let state = harness.read_with(cx, |harness, _| harness.state.clone());
        cx.update(|window, cx| state.focus(window, cx));
        cx.run_until_parked();
        (state, log, cx)
    }

    fn open(cx: &mut TestAppContext) -> (TextAreaState, Log, &mut VisualTestContext) {
        open_with(cx, InterfaceScale::Percent100, 400.0, |area| area)
    }

    fn value(state: &TextAreaState, cx: &mut VisualTestContext) -> String {
        cx.update(|_, cx| state.value(cx).to_string())
    }

    fn count(log: &Log, wanted: TextAreaEvent) -> usize {
        log.borrow()
            .iter()
            .filter(|event| **event == wanted)
            .count()
    }

    fn control_size(cx: &mut VisualTestContext) -> gpui::Size<gpui::Pixels> {
        cx.debug_bounds("text-area-control")
            .expect("the control renders")
            .size
    }

    /// Reloads the text and lets the next frame lay it out.
    fn seed(state: &TextAreaState, text: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| state.set_value(text.to_owned(), window, cx));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn height_of(state: &TextAreaState, text: &str, cx: &mut VisualTestContext) -> f32 {
        seed(state, text, cx);
        f32::from(control_size(cx).height)
    }

    #[gpui::test]
    fn offsets_are_utf8_bytes_on_character_boundaries(cx: &mut TestAppContext) {
        let (state, _, cx) = open(cx);
        // "Ginoó" is 6 bytes, the newline is byte 6, "Niño" is bytes 7..12.
        seed(&state, "Ginoó\nNiño", cx);
        let (cursor, range) = cx.update(|_, cx| (state.cursor(cx), state.selected_range(cx)));
        assert_eq!((cursor, range), (0, 0..0), "set_value puts the caret first");

        cx.update(|_, cx| state.set_selected_range(6..6, cx));
        assert_eq!(cx.update(|_, cx| state.cursor(cx)), 6, "after the ó");
        cx.update(|_, cx| state.set_selected_range(10..10, cx));
        assert_eq!(
            cx.update(|_, cx| state.cursor(cx)),
            9,
            "inside the ñ, a caret clips back to its start"
        );
        cx.update(|_, cx| state.set_selected_range(7..11, cx));
        assert_eq!(cx.update(|_, cx| state.selected_range(cx)), 7..11);
        cx.update(|_, cx| state.set_selected_range(7..10, cx));
        assert_eq!(
            cx.update(|_, cx| state.selected_range(cx)),
            7..11,
            "a range ending inside the ñ widens to include it"
        );
        cx.update(|_, cx| state.set_selected_range(0..99, cx));
        assert_eq!(cx.update(|_, cx| state.selected_range(cx)), 0..12);

        // Moving with the keyboard reports bytes too, not characters.
        cx.simulate_keystrokes("ctrl-end");
        assert_eq!(cx.update(|_, cx| state.cursor(cx)), 12, "text is 12 bytes");
        cx.simulate_keystrokes("left left");
        assert_eq!(
            cx.update(|_, cx| state.cursor(cx)),
            9,
            "two characters back from the end is byte 9"
        );
    }

    #[gpui::test]
    fn set_value_is_silent_and_native_undo_cannot_bring_back_older_text(cx: &mut TestAppContext) {
        let (state, log, cx) = open(cx);
        cx.simulate_input("Kyrie");
        assert_eq!(value(&state, cx), "Kyrie");
        let typed = log.borrow().len();
        assert!(typed > 0, "typing reports Changed");

        seed(&state, "Gloria\n\nSanctus", cx);
        assert_eq!(log.borrow().len(), typed, "set_value reports nothing");

        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(
            value(&state, cx),
            "Gloria\n\nSanctus",
            "native Undo has nothing from before set_value to bring back"
        );
        assert_eq!(log.borrow().len(), typed, "a refused Undo is not an edit");

        // History is cleared on every reload, including one after more typing.
        cx.simulate_input("!");
        seed(&state, "Agnus Dei", cx);
        cx.simulate_keystrokes("ctrl-z ctrl-z");
        assert_eq!(value(&state, cx), "Agnus Dei");
    }

    #[gpui::test]
    fn changed_fires_once_per_edit_and_native_undo_is_an_edit(cx: &mut TestAppContext) {
        let (state, log, cx) = open(cx);

        cx.simulate_input("a");
        assert_eq!(
            count(&log, TextAreaEvent::Changed),
            1,
            "one typed character"
        );

        cx.simulate_keystrokes("backspace");
        assert_eq!(count(&log, TextAreaEvent::Changed), 2, "one delete");
        assert_eq!(value(&state, cx), "");

        cx.write_to_clipboard(gpui::ClipboardItem::new_string("Pater\nnoster".into()));
        cx.simulate_keystrokes("ctrl-v");
        assert_eq!(count(&log, TextAreaEvent::Changed), 3, "one paste");
        assert_eq!(value(&state, cx), "Pater\nnoster");

        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(count(&log, TextAreaEvent::Changed), 4, "native Undo");
        assert_eq!(value(&state, cx), "");
        cx.simulate_keystrokes("ctrl-y");
        assert_eq!(count(&log, TextAreaEvent::Changed), 5, "native Redo");
        assert_eq!(value(&state, cx), "Pater\nnoster");
    }

    #[gpui::test]
    fn focus_left_fires_when_focus_moves_elsewhere(cx: &mut TestAppContext) {
        let (state, log, cx) = open(cx);
        // Focus listeners only see a window that is active.
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(count(&log, TextAreaEvent::FocusLeft), 0);

        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));

        assert_eq!(count(&log, TextAreaEvent::FocusLeft), 1);
        assert!(cx.update(|window, cx| !state.focus_handle(cx).is_focused(window)));
    }

    #[gpui::test]
    fn window_deactivation_reports_focus_left_exactly_as_a_field_does(cx: &mut TestAppContext) {
        let (_, log, cx) = open(cx);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));

        cx.deactivate_window();
        cx.update(|window, cx| window.draw(cx).clear(cx));

        // Field maps the base's Blur straight to FocusLeft and the base emits
        // Blur when the window deactivates, so a TextArea does the same: a
        // caller that commits on FocusLeft also commits when the operator
        // switches to another application, which loses nothing.
        assert_eq!(count(&log, TextAreaEvent::FocusLeft), 1);
    }

    #[gpui::test]
    fn enter_inserts_a_newline_and_never_submits(cx: &mut TestAppContext) {
        let (state, log, cx) = open(cx);

        cx.simulate_input("Holy");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("Holy");

        assert_eq!(value(&state, cx), "Holy\nHoly");
        assert!(
            cx.update(|window, cx| state.focus_handle(cx).is_focused(window)),
            "Enter keeps focus in the field"
        );
        assert_eq!(count(&log, TextAreaEvent::FocusLeft), 0);
    }

    #[gpui::test]
    fn tab_and_shift_tab_leave_the_field_without_editing_it(cx: &mut TestAppContext) {
        let (state, _, cx) = open(cx);
        cx.simulate_input("Kyrie");

        cx.simulate_keystrokes("tab");
        assert!(
            cx.update(|window, cx| !state.focus_handle(cx).is_focused(window)),
            "Tab leaves the field"
        );
        assert_eq!(value(&state, cx), "Kyrie", "Tab inserted nothing");

        cx.update(|window, cx| state.focus(window, cx));
        cx.simulate_keystrokes("shift-tab");
        assert!(
            cx.update(|window, cx| !state.focus_handle(cx).is_focused(window)),
            "Shift+Tab leaves the field"
        );
        assert_eq!(value(&state, cx), "Kyrie", "Shift+Tab changed nothing");
    }

    #[gpui::test]
    fn escape_is_not_swallowed_so_an_enclosing_overlay_closes(cx: &mut TestAppContext) {
        cx.update(crate::init).unwrap();
        struct Host {
            overlays: OverlayHost,
            state: TextAreaState,
            environment: UiEnvironment,
        }
        impl Render for Host {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .size_full()
                    .child(Button::new("invoker", "Open", self.environment))
                    .child(self.overlays.layer(self.environment, window, cx))
            }
        }
        let (host, cx) = cx.add_window_view(move |window, cx| Host {
            overlays: OverlayHost::new(cx),
            state: TextAreaState::new(window, cx),
            environment: environment(InterfaceScale::Percent100),
        });
        let (overlays, state) =
            host.read_with(cx, |host, _| (host.overlays.clone(), host.state.clone()));
        cx.update(|window, cx| {
            let state = state.clone();
            overlays.open(
                Overlay::dialog("Edit lyrics").body(move |environment, _, _| {
                    TextArea::new("overlay-lyrics", "Lyrics", &state, environment)
                        .into_any_element()
                }),
                window,
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.update(|window, cx| state.focus(window, cx));
        cx.simulate_input("Kyrie");
        assert_eq!(cx.update(|_, cx| overlays.depth(cx)), 1);

        cx.simulate_keystrokes("escape");

        assert_eq!(
            cx.update(|_, cx| overlays.depth(cx)),
            0,
            "Escape reached the overlay from inside the text area"
        );
        assert_eq!(cx.update(|_, cx| state.value(cx)), "Kyrie");
    }

    #[gpui::test]
    fn it_soft_wraps_and_never_ellipsizes(cx: &mut TestAppContext) {
        let (state, _, cx) = open_with(cx, InterfaceScale::Percent100, 260.0, |area| {
            area.rows(1, 12)
        });
        let one_line = height_of(&state, "Ang", cx);

        let long =
            "Maghimaya ang Dios sa kahitas-an ug sa yuta pag-ukon sa tanang mga tawo ".repeat(3);
        let wrapped = height_of(&state, &long, cx);

        assert!(
            wrapped > one_line * 1.5,
            "one long line wraps onto more rows: {one_line} vs {wrapped}"
        );
        assert_eq!(value(&state, cx), long, "wrapping is not truncation");
    }

    #[gpui::test]
    fn rows_grow_with_the_text_then_scroll_inside(cx: &mut TestAppContext) {
        let (state, _, cx) = open_with(cx, InterfaceScale::Percent100, 400.0, |area| {
            area.rows(2, 5)
        });
        let empty = height_of(&state, "", cx);
        let two = height_of(&state, "a\nb", cx);
        let four = height_of(&state, "a\nb\nc\nd", cx);
        let five = height_of(&state, "a\nb\nc\nd\ne", cx);
        let twenty = height_of(&state, &"x\n".repeat(20), cx);

        assert_eq!(empty, two, "the minimum is two rows");
        assert!(four > two, "it grows with the text");
        assert!(five > four, "up to the maximum");
        assert_eq!(twenty, five, "past the maximum it stops growing");
    }

    #[gpui::test]
    fn wrapping_holds_at_the_largest_interface_scale(cx: &mut TestAppContext) {
        let (state, _, cx) = open_with(cx, InterfaceScale::Percent200, 360.0, |area| {
            area.rows(1, 20)
        });
        let short = height_of(&state, "Ang", cx);
        let long = "Maghimaya ang Dios sa kahitas-an ".repeat(4);
        let wrapped = height_of(&state, &long, cx);

        let width = f32::from(control_size(cx).width);
        assert!(
            width <= 360.5,
            "the control stays inside its column at 200% instead of widening: {width}"
        );
        assert!(
            wrapped > short * 2.0,
            "a long line wraps onto several rows at 200%: {short} vs {wrapped}"
        );
    }

    #[gpui::test]
    fn a_read_only_text_area_stays_focusable_selectable_and_says_why(cx: &mut TestAppContext) {
        let (state, log, cx) = open_with(cx, InterfaceScale::Percent100, 400.0, |area| {
            area.read_only("The Arrangement is archived")
        });
        seed(&state, "Gloria", cx);
        cx.update(|window, cx| state.focus(window, cx));

        assert!(
            cx.update(|window, cx| state.focus_handle(cx).is_focused(window)),
            "still focusable"
        );
        cx.simulate_input("edited");
        cx.simulate_keystrokes("enter");
        assert_eq!(value(&state, cx), "Gloria", "not editable");
        assert!(log.borrow().is_empty(), "a refused edit is not Changed");

        cx.simulate_keystrokes("ctrl-a");
        assert_eq!(cx.update(|_, cx| state.selected_range(cx)), 0..6);
        cx.simulate_keystrokes("ctrl-c");
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("Gloria".to_owned()),
            "selectable and copyable"
        );
        assert!(
            cx.debug_bounds("text-area-read-only").is_some(),
            "the reason is stated, not implied by fading"
        );
    }

    #[gpui::test]
    fn the_node_is_a_named_multi_line_text_input_with_its_value_and_reason(
        cx: &mut TestAppContext,
    ) {
        use gpui::{Element, accesskit};
        let (state, _, cx) = open(cx);
        seed(&state, "Gloria\n\nSanctus", cx);
        cx.update(|window, cx| {
            let area = TextArea::new(
                "lyrics",
                "Lyrics",
                &state,
                environment(InterfaceScale::Percent100),
            )
            .placeholder("Type the lyrics")
            .read_only("Archived");
            let element = area.control(window, cx).render(window, cx).into_element();
            let mut node = accesskit::Node::new(element.a11y_role().unwrap());
            element.write_a11y_info(&mut node);
            assert_eq!(node.role(), accesskit::Role::MultilineTextInput);
            assert_eq!(node.label(), Some("Lyrics"));
            assert_eq!(node.value(), Some("Gloria\n\nSanctus"));
            assert_eq!(node.placeholder(), Some("Type the lyrics"));
            assert_eq!(node.description(), Some("Read-only. Archived"));
        });
    }
}
