//! An ordinary bordered list row, and the compact labels it carries.
//!
//! `DESIGN.md` reaches for bordered rows before cards, and asks for one shared
//! edge rather than a border around every item, so a row draws only its own
//! bottom edge. A column of them reads the way a mail list does — a continuous
//! ruled column — instead of a stack of detached boxes with doubled borders
//! between them.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};

use crate::{
    environment::UiEnvironment,
    primitives::{focus_visible, note},
    tokens,
};

/// A short label beside a row's own text: what kind of thing the row is, or
/// what state it is in. It is never the only thing that says so — the row's
/// title and its detail line carry the identity — so a badge stays a
/// classifier rather than a control.
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    tone: Option<u32>,
    environment: UiEnvironment,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>, environment: UiEnvironment) -> Self {
        Self {
            label: label.into(),
            tone: None,
            environment,
        }
    }

    /// A domain color, for a badge that carries real domain metadata — a
    /// liturgical season, say. `DESIGN.md` reserves the liturgical colors for
    /// exactly that and forbids them as application accents, so pass a token
    /// from [`crate::tokens::color`] and nothing else.
    pub fn tone(mut self, tone: u32) -> Self {
        self.tone = Some(tone);
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let tone = self.tone.unwrap_or(palette.text_muted);
        // An outline rather than a fill: the row underneath a badge is already
        // a surface, and a filled chip on it would read as a second control.
        // `DESIGN.md` also keeps pills for status labels alone, so this uses
        // the ordinary control radius.
        div()
            .debug_selector({
                let label = self.label.clone();
                move || format!("badge-{label}")
            })
            .flex_none()
            .px(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(tone))
            .text_color(rgb(tone))
            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
            .child(self.label)
    }
}

type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

/// One row of a primary list: a title, an optional detail line, and the badges
/// that classify it.
///
/// Behavior — focus, Enter/Space activation, and the accessible name — comes
/// from the same qualified base Button every other Ordo control uses, so a row
/// is reached by `Tab` in visible reading order and activated like anything
/// else.
///
/// ponytail: Tab order only. `DESIGN.md` also asks a selectable collection for
/// arrow-key behavior and ListBox/option semantics, which need the collection
/// itself — a roving-focus owner like `primitives::selection`'s, over rows that
/// carry content rather than one label. That lands with the first list that
/// owns real records; until then a row is an ordinary named, focusable,
/// activatable control and nothing claims otherwise.
#[derive(IntoElement)]
pub struct ListRow {
    id: ElementId,
    title: SharedString,
    detail: Option<SharedString>,
    badges: Vec<Badge>,
    selected: bool,
    environment: UiEnvironment,
    surface: u32,
    on_activate: Option<Activate>,
}

impl ListRow {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        environment: UiEnvironment,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            badges: Vec::new(),
            selected: false,
            environment,
            surface: tokens::palette(environment.appearance()).surface,
            on_activate: None,
        }
    }

    /// The secondary line beneath the title. `DESIGN.md` moves secondary
    /// metadata out before it compresses primary identity, so this is the
    /// first thing a narrow row gives up — it truncates on its own line.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn badges(mut self, badges: impl IntoIterator<Item = Badge>) -> Self {
        self.badges = badges.into_iter().collect();
        self
    }

    /// Selection is the forest-green tinted fill, and stays independent of
    /// focus: both may be on the same row at once.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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

    pub(crate) fn control(self) -> gpui_base::Button {
        let palette = tokens::palette(self.environment.appearance());
        let scale = self.environment.preferences().scale;
        let selected = self.selected;
        let title = self.title.clone();
        // A row's accessible name is everything that identifies it, because
        // the detail line and the badges are what tell one Mass from another
        // and neither is a control of its own.
        let name = [Some(self.title.clone()), self.detail.clone()]
            .into_iter()
            .flatten()
            .map(|part| part.to_string())
            .chain(self.badges.iter().map(|badge| badge.label.to_string()))
            .collect::<Vec<_>>()
            .join(". ");
        let detail = self.detail.map(|detail| {
            div()
                .min_w_0()
                .truncate()
                .text_color(rgb(palette.text_muted))
                .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                .child(detail)
        });
        let secondary = (detail.is_some() || !self.badges.is_empty()).then(|| {
            div()
                .flex()
                .items_center()
                .w_full()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .children(detail)
                .children(self.badges)
        });

        gpui_base::Button::new(self.id)
            .debug_selector(move || format!("row-{title}"))
            .accessibility_label(name)
            .aria_selected(selected)
            .when_some(self.on_activate, |row, activate| {
                row.on_click(move |_, window, cx| activate(window, cx))
            })
            .flex()
            .flex_col()
            .items_start()
            .justify_center()
            .w_full()
            .min_w_0()
            // A row is the height of what it holds and nothing less. Without
            // this it is an ordinary flex child of its column, so a short
            // window squeezes every row down to fit instead of overflowing —
            // the titles close up, and the scroll indicator has nothing to
            // report because the list was never allowed to be too long.
            .flex_none()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .min_h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .py(px(scale.apply(tokens::spacing::RELATED)))
            // The one shared edge. A row draws its own bottom border and
            // nothing else, so adjacent rows rule a continuous column instead
            // of doubling a border between every pair.
            .border_b(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .when(selected, |row| row.bg(rgb(palette.selection)))
            .when(!selected, |row| {
                row.hover(|style| style.bg(rgb(palette.surface_secondary)))
            })
            .active(|style| style.bg(rgb(palette.border)))
            .cursor_pointer()
            .text_color(rgb(palette.text))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .truncate()
                    .child(self.title.clone()),
            )
            .children(secondary)
    }
}

impl RenderOnce for ListRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let environment = self.environment;
        let surface = self.surface;
        focus_visible(self.control(), environment, surface)
    }
}

/// A list that has nothing in it says so, rather than leaving the operator in
/// front of an empty column wondering whether it is still loading.
pub fn empty(reason: impl Into<String>, environment: UiEnvironment) -> impl IntoElement {
    note(
        crate::assets::icon::INFO,
        tokens::palette(environment.appearance()).text_muted,
        reason.into(),
        "list-empty",
        environment,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{Context, Element, Render, TestAppContext, accesskit, canvas};
    use std::sync::{Arc, Mutex};

    fn environment() -> UiEnvironment {
        UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light)
    }

    /// A row's identity is spread across its title, its detail line, and its
    /// badges, and only one of the three is its title. All of it has to reach
    /// the accessible name, or the row announces "6:00 AM" and nothing about
    /// which Mass that is.
    #[gpui::test]
    fn a_rows_name_carries_its_detail_and_every_badge(cx: &mut TestAppContext) {
        type Captured = Arc<Mutex<Vec<accesskit::Node>>>;

        struct Probe {
            captured: Captured,
        }

        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let captured = self.captured.clone();
                canvas(
                    move |_, window, cx| {
                        let environment = environment();
                        let rows = [
                            ListRow::new("row", "Second Sunday of Lent", environment)
                                .detail("6:00 AM \u{b7} Cebuano")
                                .badges([
                                    Badge::new("Lent", environment)
                                        .tone(tokens::color::LITURGICAL_PURPLE),
                                    Badge::new("Incomplete", environment),
                                ])
                                .selected(true)
                                .on_activate(|_, _| {}),
                            ListRow::new("bare", "Templates", environment),
                        ]
                        .map(|row| {
                            let element = row.control().render(window, cx).into_element();
                            let mut node = accesskit::Node::new(
                                element.a11y_role().expect("the row is exposed"),
                            );
                            element.write_a11y_info(&mut node);
                            node
                        });
                        *captured.lock().unwrap() = rows.to_vec();
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
        let [full, bare] = &nodes[..] else {
            panic!("two nodes are captured");
        };
        assert_eq!(full.role(), accesskit::Role::Button);
        assert_eq!(
            full.label(),
            Some("Second Sunday of Lent. 6:00 AM \u{b7} Cebuano. Lent. Incomplete")
        );
        assert_eq!(full.is_selected(), Some(true));
        assert!(full.supports_action(accesskit::Action::Click));
        // A row with nothing but a title is named by it, and an unselected row
        // says so rather than leaving selection unstated.
        assert_eq!(bare.label(), Some("Templates"));
        assert_eq!(bare.is_selected(), Some(false));
    }
}
