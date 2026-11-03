//! One entry in the global navigation rail.
//!
//! A rail entry is a stacked icon over its label (Decision 300). The current
//! entry fills its icon and shows it alone, centred: its label fades out but
//! stays in the layout, so the entry keeps its height, its accessible name is
//! still the label, and the label is its tooltip. Arriving and leaving are the
//! `160ms` crossing Decision 301 describes; reduced motion makes it a jump.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::*, px, rgb, svg,
};
use gpui_base::motion::transition;

use crate::{
    environment::UiEnvironment,
    motion,
    primitives::{button::TOOLTIP_SHOW_DELAY, button::tooltip_view, focus_visible},
    tokens,
};

/// The mockup's rail entry: at least one label tall, with its content centred.
pub const ENTRY_MIN_HEIGHT: f32 = 54.0;
/// The gap between an entry's icon and its label.
const ICON_GAP: f32 = 2.0;
/// The rail label: 12px at the label weight, on a 1.25 line.
const LABEL_SIZE: f32 = 12.0;
const LABEL_LINE_HEIGHT: f32 = 1.25;
/// How far the label sinks as it fades, so it reads as leaving under the icon.
const LABEL_SINK: f32 = 4.0;

type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct NavEntry {
    id: ElementId,
    label: SharedString,
    icon: &'static str,
    filled: &'static str,
    current: bool,
    compact: bool,
    environment: UiEnvironment,
    on_activate: Option<Activate>,
}

impl NavEntry {
    /// `icon` is the Lucide outline and `filled` its filled form from
    /// [`crate::assets::icon::filled`], shown while the entry is current.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: &'static str,
        filled: &'static str,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            filled,
            current: false,
            compact: false,
            environment,
            on_activate: None,
        }
    }

    /// The destination the operator is in. It is announced as selected, and
    /// drawn filled, alone, and on the selection color, so it never relies on
    /// color alone.
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// The collapsed rail: one square icon control per entry, named by its
    /// tooltip, for when the workspace needs the width (`MILESTONES.md`).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn on_activate(mut self, activate: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(activate));
        self
    }
}

impl RenderOnce for NavEntry {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let label = self.label.clone();
        let current = self.current;

        // 0 while the entry shows its label, 1 once it shows its icon alone.
        let alone: f32 = transition(
            (self.id.clone(), "current"),
            if current { 1.0 } else { 0.0 },
            motion::standard(),
            window,
            cx,
        );
        // The icon travels half of what the label and the gap take below it,
        // which lands it in the middle of the entry. A label that wraps (as
        // "Transfer & Backup" does) is taller, so its height is measured.
        let one_line = scale.apply(LABEL_SIZE * LABEL_LINE_HEIGHT);
        let measured =
            window.use_keyed_state((self.id.clone(), "label-height"), cx, |_, _| one_line);
        let label_height = *measured.read(cx);
        let shift = (label_height + scale.apply(ICON_GAP)) / 2.0;

        let text = if current {
            palette.text
        } else {
            palette.text_muted
        };
        let icon = svg()
            .debug_selector({
                let label = label.clone();
                move || format!("icon-{label}")
            })
            .flex_none()
            .size(px(scale.apply(tokens::icon::NAVIGATION_SIZE)))
            .text_color(rgb(text))
            .data(if current { self.filled } else { self.icon }.as_bytes())
            .when(!self.compact, |icon| icon.relative().top(px(shift * alone)));

        let content = if self.compact {
            div().child(icon)
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .w_full()
                .gap(px(scale.apply(ICON_GAP)))
                .on_children_prepainted(move |bounds, window, cx| {
                    let Some(height) = bounds.get(1).map(|label| f32::from(label.size.height))
                    else {
                        return;
                    };
                    if (*measured.read(cx) - height).abs() > 0.5 {
                        measured.update(cx, |measured, _| *measured = height);
                        window.refresh();
                    }
                })
                .child(icon)
                .child(
                    div()
                        .debug_selector({
                            let label = label.clone();
                            move || format!("rail-label-{label}")
                        })
                        .w_full()
                        .text_center()
                        .text_size(px(scale.apply(LABEL_SIZE)))
                        .line_height(px(one_line))
                        .opacity(1.0 - alone)
                        .relative()
                        .top(px(scale.apply(LABEL_SINK) * alone))
                        .child(label.clone()),
                )
        };

        let activate = self.on_activate;
        let tooltip = (current || self.compact).then(|| label.clone());
        let environment = self.environment;
        let control = gpui_base::Button::new(self.id)
            .debug_selector({
                let label = label.clone();
                move || format!("button-{label}")
            })
            .accessibility_label(label)
            .when(current, |control| control.aria_selected(true))
            .when_some(activate, |control, activate| {
                control.on_click(move |_, window, cx| activate(window, cx))
            })
            .when_some(tooltip, |control, tooltip| {
                control
                    .tooltip_show_delay(TOOLTIP_SHOW_DELAY)
                    .tooltip(move |_, cx| tooltip_view(tooltip.clone(), environment, cx))
            })
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .text_color(rgb(text))
            .cursor_pointer()
            .when(self.compact, |control| {
                control.size(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            })
            .when(!self.compact, |control| {
                control
                    .w_full()
                    .min_h(px(scale.apply(ENTRY_MIN_HEIGHT)))
                    .py(px(scale.apply(tokens::spacing::RELATED)))
            })
            .when(current, |control| control.bg(rgb(palette.selection)))
            .when(!current, |control| {
                control.hover(move |style| style.bg(rgb(palette.surface_secondary)))
            })
            .child(content);
        focus_visible(
            control,
            self.environment,
            if current {
                palette.selection
            } else {
                palette.surface
            },
        )
    }
}
