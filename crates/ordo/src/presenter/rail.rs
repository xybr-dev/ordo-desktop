//! The Ordo rail of the Presenter (T1.4 WP7, Decisions 171 and 237).
//!
//! The rail lists the Mass items under their six sections and scrolls the
//! timeline to a group; it never filters, so every group stays present, and it
//! never touches the Engine. It is drawn in one of three places, chosen by the
//! layout model:
//!
//! * pinned: a column beside the timeline, with a pin toggle that unpins it;
//! * the `ordo-tab` column, which opens the rail as an overlay on a click
//!   (hover never opens it);
//! * overlay-only (Constrained): the same overlay with no pin toggle.
//!
//! The overlay is a left-anchored panel over a dim scrim. The scrim consumes
//! the click that closes it, so nothing behind it is pressed. The pin choice
//! lives in `PresenterView::rail_choice` for the session only (Decision 237);
//! this module only draws it and reports what was pressed.
//!
//! The overlay is not the application `OverlayHost`: that host owns modal
//! dialogs and right-hand slideovers with their own focus trap and stack,
//! whereas the rail is part of the Presenter surface and must leave the Output
//! Dock reachable. It takes its slide timing from [`ui::motion::entrance`]
//! instead (a fade only under reduced motion).

use std::rc::Rc;

use gpui::{
    AnyElement, App, FocusHandle, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Window, deferred, div, prelude::*, px, rgb,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    motion::{self, Entrance},
    primitives::{self, button::Button},
    scroll::Scroll,
    tokens,
};

/// Called with a Mass item's index.
pub type OnJump = Rc<dyn Fn(usize, &mut Window, &mut App)>;
/// Called by a button with no argument.
pub type OnPress = Rc<dyn Fn(&mut Window, &mut App)>;

/// Lucide `pin` and `pin-off`, which the bundled icon set does not carry yet.
const PIN_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5" /><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z" /></svg>"#;
const PIN_OFF_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5" /><path d="M15 9.34V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H7.89" /><path d="m2 2 20 20" /><path d="M9 9v1.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h11" /></svg>"#;

/// Accessible name and selector word of the pin toggle when the rail is
/// pinned (pressing it unpins).
pub const UNPIN: &str = "Unpin Ordo";
/// The same when the rail is in the overlay (pressing it pins).
pub const PIN: &str = "Pin Ordo";
/// The overlay's close button.
pub const CLOSE: &str = "Close Ordo";

/// One Mass item of the rail.
pub struct RailItem {
    /// Index among all Mass items; the argument to the jump callback.
    pub index: usize,
    pub number: SharedString,
    pub title: SharedString,
    /// The item that owns the Preview Cue.
    pub current: bool,
}

/// One of the Mass sections.
pub struct RailSection {
    pub title: SharedString,
    pub items: Vec<RailItem>,
}

/// What the rail's pin control does, or that there is none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinControl {
    /// The rail is pinned; the control unpins it.
    Unpin,
    /// The rail is in the overlay; the control pins it.
    Pin,
    /// Constrained: overlay only, so there is no choice to offer.
    Hidden,
}

/// Everything the panel needs.
pub struct RailModel {
    pub sections: Vec<RailSection>,
    pub pin: PinControl,
}

/// The callbacks the rail reports to.
#[derive(Clone)]
pub struct Handlers {
    pub jump: OnJump,
    pub toggle_pin: OnPress,
    pub close: OnPress,
}

/// Where and how the panel is drawn.
pub struct Placement<'a> {
    pub width: f32,
    /// In the overlay: adds the close button and Escape handling.
    pub overlay: bool,
    /// The handle the owner moves focus to when the overlay opens. Escape
    /// is heard while focus is inside the panel.
    pub focus: Option<&'a FocusHandle>,
}

/// The rail panel: header, then the sections and their items in a scrolling
/// body.
pub fn panel(
    model: RailModel,
    placement: Placement<'_>,
    scroll: &Scroll,
    handlers: &Handlers,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Placement {
        width,
        overlay,
        focus,
    } = placement;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;

    let pin = match model.pin {
        PinControl::Unpin => Some((UNPIN, PIN_OFF_ICON)),
        PinControl::Pin => Some((PIN, PIN_ICON)),
        PinControl::Hidden => None,
    };
    let header = div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .px(px(scale.apply(tokens::spacing::RELATED)))
        .h(px(scale.apply(tokens::density::DEFAULT_CONTROL + 8.0)))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Ordo"),
        )
        .when(model.pin == PinControl::Hidden, |header| {
            header.child(
                div()
                    .flex_none()
                    .text_color(rgb(palette.text_muted))
                    .text_size(px(scale.apply(tokens::typography::CAPTION.size)))
                    .child("Overlay only"),
            )
        })
        .children(pin.map(|(label, glyph)| {
            let toggle_pin = handlers.toggle_pin.clone();
            Button::new(label, label, environment)
                .icon(glyph)
                .icon_only()
                .quiet()
                .on_surface(palette.surface)
                .on_activate(move |window, cx| toggle_pin(window, cx))
        }))
        .children(overlay.then(|| {
            let close = handlers.close.clone();
            Button::new(CLOSE, CLOSE, environment)
                .icon(icon::X)
                .icon_only()
                .quiet()
                .on_surface(palette.surface)
                .on_activate(move |window, cx| close(window, cx))
        }));

    let mut body = Vec::new();
    for section in model.sections {
        body.push(
            div()
                .flex_none()
                .px(px(scale.apply(tokens::spacing::RELATED)))
                .pt(px(scale.apply(tokens::spacing::RELATED)))
                .pb(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .truncate()
                .text_color(rgb(palette.text_muted))
                .text_size(px(scale.apply(tokens::typography::CAPTION.size)))
                .child(section.title)
                .into_any_element(),
        );
        for item in section.items {
            body.push(item_row(item, handlers, environment));
        }
    }

    let panel = div()
        .id(if overlay {
            "rail-overlay-panel"
        } else {
            "rail-pinned-panel"
        })
        .debug_selector(|| "presenter-rail".into())
        .flex_none()
        .w(px(width))
        .h_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(palette.surface))
        .text_color(rgb(palette.text))
        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
        .border_r(px(tokens::border::SEPARATOR))
        .border_color(rgb(if overlay {
            palette.border_strong
        } else {
            palette.border
        }))
        .when_some(focus, |panel, focus| {
            panel.track_focus(focus).on_key_down({
                let close = handlers.close.clone();
                move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        close(window, cx);
                        cx.stop_propagation();
                    }
                }
            })
        })
        .child(header)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    scroll
                        .bounded(div().id("rail-scroll"))
                        .size_full()
                        .flex()
                        .flex_col()
                        .children(body),
                )
                .children(scroll.indicator("rail", 0.0, window, cx, environment)),
        );
    panel.into_any_element()
}

fn item_row(item: RailItem, handlers: &Handlers, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let index = item.index;
    let jump = handlers.jump.clone();
    let name: SharedString = if item.current {
        format!("{} {}, current", item.number, item.title).into()
    } else {
        format!("{} {}", item.number, item.title).into()
    };
    let row = div()
        .id(("rail-item", index))
        .debug_selector(move || format!("rail-item-{index}"))
        .relative()
        .flex_none()
        .w_full()
        .min_h(px(scale.apply(tokens::density::COMPACT_ROW)))
        .px(px(scale.apply(tokens::spacing::RELATED)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .when(item.current, |row| {
            row.bg(rgb(palette.selection))
                .font_weight(FontWeight::SEMIBOLD)
        })
        .hover(|style| style.bg(rgb(palette.surface_secondary)))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(name.clone())
        .on_click(move |_, window, cx| jump(index, window, cx))
        // The current item's neutral leading bar (Decision 295), so the mark
        // is a shape and not a fill alone.
        .children(item.current.then(|| {
            div()
                .absolute()
                .left_0()
                .top_0()
                .h_full()
                .w(px(scale.apply(3.0)))
                .bg(rgb(palette.text))
        }))
        .child(
            div()
                .flex_none()
                .text_color(rgb(palette.text_muted))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .child(item.number),
        )
        .child(div().flex_1().min_w_0().truncate().child(item.title));
    primitives::truncated(row, name, environment).into_any_element()
}

/// The collapsed ORDO column. A click opens the rail; hover does nothing.
pub fn tab(width: f32, on_open: OnPress, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .id("ordo-tab")
        .debug_selector(|| "ordo-tab".into())
        .flex_none()
        .w(px(width))
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(scale.apply(tokens::spacing::GROUP)))
        .bg(rgb(palette.surface))
        .border_r(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .text_color(rgb(palette.text_muted))
        .text_size(px(scale.apply(tokens::typography::CAPTION.size)))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .role(Role::Button)
        .aria_label("Open Ordo")
        .on_click(move |_, window, cx| on_open(window, cx))
        .children(["O", "R", "D", "O"].map(|letter| div().child(letter)))
        .into_any_element()
}

/// The entrance of the overlay for this opening. Keyed state lives only while
/// it is asked for every frame, so the owner calls this on every render and
/// bumps `generation` on each opening: the first opening then slides in
/// instead of being adopted as already arrived.
pub fn entrance(generation: u64, distance: f32, window: &mut Window, cx: &mut App) -> Entrance {
    motion::entrance("rail-overlay", generation, distance, window, cx)
}

/// The overlay: a dim scrim over the whole window and the panel anchored at
/// the left edge, below the header. A press on the scrim is consumed here and
/// closes the overlay; nothing behind it receives it.
pub fn overlay(
    panel: AnyElement,
    width: f32,
    top: f32,
    entrance: Entrance,
    on_close: OnPress,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let scrim = div()
        .id("rail-scrim")
        .debug_selector(|| "rail-scrim".into())
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        // Occlusion stops the hit test here, so the press never reaches the
        // timeline, the dock or the header behind the dim.
        .occlude()
        .bg(tokens::elevation::scrim(entrance.opacity))
        .on_click(move |_, window, cx| {
            on_close(window, cx);
            cx.stop_propagation();
        });
    let slid = div()
        .absolute()
        .top(px(top))
        .bottom_0()
        .left(px(-entrance.offset))
        .w(px(width))
        .opacity(entrance.opacity)
        .shadow(vec![tokens::elevation::overlay(
            environment.appearance(),
            scale,
        )])
        // The panel takes its own presses: it sits over the scrim.
        .occlude()
        .child(panel);
    deferred(
        div()
            .id("rail-overlay")
            .debug_selector(|| "rail-overlay".into())
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(scrim)
            .child(slid),
    )
    .with_priority(OVERLAY_PRIORITY)
    .into_any_element()
}

/// Above the Presenter's own content and below a popup; the same layer the
/// application overlays use.
const OVERLAY_PRIORITY: usize = 20;
