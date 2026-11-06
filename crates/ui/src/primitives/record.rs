//! A record row with its own columns, and the status label it carries.
//!
//! A [`ListRow`](super::list::ListRow) is a title over a detail line. A
//! grouped index such as Masses → Upcoming reads across instead: time,
//! language, choir, presider, readiness, each in its own column, so that the
//! rows of a group line up and can be compared down the page. The row is one
//! control, named by everything it shows, and opens the record it names.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::*, px, rgb, svg,
};

use crate::{
    environment::UiEnvironment,
    primitives::{button, focus_visible},
    tokens,
};

type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

/// The mockup's Mass row is at least this tall, so a row with an Anticipated
/// chip beside its time is not taller than one without.
const MIN_HEIGHT: f32 = 44.0;

#[derive(IntoElement)]
pub struct RecordRow {
    id: ElementId,
    name: SharedString,
    cells: Vec<(f32, AnyElement)>,
    trailing: Vec<AnyElement>,
    inset: f32,
    environment: UiEnvironment,
    surface: u32,
    on_activate: Option<Activate>,
}

impl RecordRow {
    /// `name` is the accessible name: everything that tells this record from
    /// its neighbours, since none of its cells is a control of its own.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cells: Vec::new(),
            trailing: Vec::new(),
            inset: 0.0,
            environment,
            surface: tokens::palette(environment.appearance()).surface,
            on_activate: None,
        }
    }

    /// A column taking `share` of the row's flexible width. Every row of an
    /// index passes the same shares, so the columns line up down the page.
    pub fn cell(mut self, share: f32, content: impl IntoElement) -> Self {
        self.cells.push((share, content.into_any_element()));
        self
    }

    /// Content at the trailing end, at its own width.
    pub fn trailing(mut self, content: impl IntoElement) -> Self {
        self.trailing.push(content.into_any_element());
        self
    }

    /// The leading inset, in unscaled logical pixels, so rows under a group
    /// header start where the header's title does.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    pub fn on_surface(mut self, surface: u32) -> Self {
        self.surface = surface;
        self
    }

    pub fn on_activate(mut self, activate: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(activate));
        self
    }
}

impl RenderOnce for RecordRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let name = self.name.clone();
        let environment = self.environment;
        let control = gpui_base::Button::new(self.id)
            .debug_selector({
                let name = name.clone();
                move || format!("record-{name}")
            })
            .accessibility_label(name.clone())
            .tooltip_show_delay(button::TOOLTIP_SHOW_DELAY)
            .tooltip(move |_, cx| button::tooltip_view(name.clone(), environment, cx))
            .when_some(self.on_activate, |row, activate| {
                row.on_click(move |_, window, cx| activate(window, cx))
            })
            .flex()
            .items_center()
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(
                scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
            ))
            .min_h(px(scale.apply(MIN_HEIGHT)))
            .pl(px(scale.apply(self.inset)))
            .pr(px(
                scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
            ))
            .border_t(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .hover(move |style| style.bg(rgb(palette.selection)))
            .active(move |style| style.bg(rgb(palette.border)))
            .cursor_pointer()
            .text_color(rgb(palette.text))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .children(self.cells.into_iter().map(|(share, cell)| {
                // Every cell shares the flexible width in proportion and
                // ellipsizes rather than pushing its neighbours along.
                div()
                    .flex_grow(share)
                    .flex_shrink(1.0)
                    .flex_basis(px(0.0))
                    .min_w_0()
                    .overflow_hidden()
                    .truncate()
                    .child(cell)
            }))
            .children(self.trailing);
        focus_visible(control, self.environment, self.surface)
    }
}

/// What a status is about, which decides its color. The marker's shape says
/// the same thing without the color (`DESIGN.md`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusTone {
    Live,
    Warning,
    Danger,
}

/// The status marker shapes `DESIGN.md` pairs with each state, so a state is
/// never told by color alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Marker {
    Dot,
    Square,
    Diamond,
}

impl Marker {
    /// The marker as an inline SVG, filled in `currentColor`.
    pub fn svg(self) -> &'static str {
        match self {
            Self::Dot => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="4" fill="currentColor"/></svg>"#
            }
            Self::Square => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect x="1.5" y="1.5" width="7" height="7" rx="1" fill="currentColor"/></svg>"#
            }
            Self::Diamond => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M5 0.5 9.5 5 5 9.5 0.5 5Z" fill="currentColor"/></svg>"#
            }
        }
    }
}

/// A status label: a shaped marker and a short phrase, in the state's color.
#[derive(IntoElement)]
pub struct Status {
    text: SharedString,
    tone: StatusTone,
    marker: Marker,
    environment: UiEnvironment,
}

impl Status {
    pub fn new(
        text: impl Into<SharedString>,
        tone: StatusTone,
        marker: Marker,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            text: text.into(),
            tone,
            marker,
            environment,
        }
    }
}

impl RenderOnce for Status {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let color = match self.tone {
            StatusTone::Live => palette.live,
            StatusTone::Warning => palette.warning,
            StatusTone::Danger => palette.danger,
        };
        let text = self.text.clone();
        div()
            .debug_selector(move || format!("status-{text}"))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT + 2.0)))
            .text_color(rgb(color))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .whitespace_nowrap()
            .child(
                svg()
                    .flex_none()
                    .size(px(scale.apply(10.0)))
                    .text_color(rgb(color))
                    .data(self.marker.svg().as_bytes()),
            )
            .child(self.text)
    }
}

/// A small neutral chip beside a value, such as "Anticipated" beside a Mass's
/// time. It classifies; it is never a control.
#[derive(IntoElement)]
pub struct Chip {
    label: SharedString,
    icon: Option<&'static str>,
    environment: UiEnvironment,
}

impl Chip {
    pub fn new(label: impl Into<SharedString>, environment: UiEnvironment) -> Self {
        Self {
            label: label.into(),
            icon: None,
            environment,
        }
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let label = self.label.clone();
        div()
            .debug_selector(move || format!("chip-{label}"))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .px(px(scale.apply(tokens::spacing::RELATED - 2.0)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .bg(rgb(palette.surface_secondary))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .text_color(rgb(palette.text_muted))
            .text_size(px(scale.apply(tokens::typography::CAPTION.size)))
            .whitespace_nowrap()
            .children(self.icon.map(|icon| {
                svg()
                    .flex_none()
                    .size(px(scale.apply(12.0)))
                    .text_color(rgb(palette.text_muted))
                    .data(icon.as_bytes())
            }))
            .child(self.label)
    }
}
