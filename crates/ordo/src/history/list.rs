//! The History list: sessions as disclosure rows, their revisions inside.
//!
//! The whole list is one Tab stop (DESIGN 619). Up and Down walk the visible
//! rows, Right opens a session and Left closes it (or steps to the session a
//! revision belongs to), and Enter or Space acts on the row. Rows are
//! `Role::ListItem`s with accessible names; the head revision reads as
//! **Current** and opens nothing.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::*, px, rgb, svg,
};
use ui::{assets::icon, environment::UiEnvironment, tokens};

use super::{HistoryView, Load};

/// The line shown while History is being read.
pub const LOADING: &str = "Loading History\u{2026}";
/// The footnote of an Arrangement's History (mockup 797).
pub const ARRANGEMENT_FOOTNOTE: &str =
    "Every saved change is kept. Select a revision to compare it with the current Arrangement.";
/// The footnote of a Song's History (mockup 797).
pub const SONG_FOOTNOTE: &str = "Song History covers its title, attribution and metadata. Each Arrangement keeps its own History.";
/// The chip on the head revision.
pub const CURRENT: &str = "Current";

/// One visible row of the list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Item {
    /// A session row, by its position (newest first).
    Session(usize),
    /// A revision row: its session and its position inside it.
    Revision(usize, usize),
}

impl HistoryView {
    /// The visible rows in order: every session, and the revisions of each
    /// expanded one.
    pub fn items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        for (si, session) in self.sessions.iter().enumerate() {
            items.push(Item::Session(si));
            if self.expanded.contains(&si) {
                items.extend((0..session.revisions.len()).map(|ri| Item::Revision(si, ri)));
            }
        }
        items
    }

    /// Where the active row is in `items`: the row itself, else the session it
    /// belonged to (it vanished when that session closed), else the first.
    pub(super) fn active_index(&self, items: &[Item]) -> usize {
        let session = match self.active {
            Item::Session(si) | Item::Revision(si, _) => si,
        };
        items
            .iter()
            .position(|item| *item == self.active)
            .or_else(|| {
                items
                    .iter()
                    .position(|item| *item == Item::Session(session))
            })
            .unwrap_or(0)
    }

    /// Acts on a row: a session opens or closes, a revision opens its
    /// comparison, and the head (Current) opens nothing.
    pub(super) fn activate(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        self.active = item;
        self.focus.focus(window, cx);
        match item {
            Item::Session(si) => {
                if !self.expanded.remove(&si) {
                    self.expanded.insert(si);
                }
                cx.notify();
            }
            Item::Revision(si, ri) => {
                let current = self
                    .sessions
                    .get(si)
                    .and_then(|session| session.revisions.get(ri))
                    .is_none_or(|row| row.current);
                if current {
                    cx.notify();
                    return;
                }
                self.open_compare(si, ri, window, cx);
            }
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let items = self.items();
        if items.is_empty() {
            return;
        }
        let at = self.active_index(&items);
        let item = items[at];
        let handled = match event.keystroke.key.as_str() {
            "down" => {
                self.active = items[(at + 1).min(items.len() - 1)];
                true
            }
            "up" => {
                self.active = items[at.saturating_sub(1)];
                true
            }
            "home" => {
                self.active = items[0];
                true
            }
            "end" => {
                self.active = items[items.len() - 1];
                true
            }
            "right" => {
                if let Item::Session(si) = item {
                    self.expanded.insert(si);
                }
                true
            }
            "left" => {
                match item {
                    Item::Session(si) => {
                        self.expanded.remove(&si);
                    }
                    Item::Revision(si, _) => {
                        self.active = Item::Session(si);
                    }
                }
                true
            }
            "enter" | "space" => {
                self.activate(item, window, cx);
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            window.prevent_default();
            cx.notify();
        }
    }

    /// Draws the list, or its loading line or refusal.
    pub(super) fn render_list(
        &mut self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let meta = px(scale.apply(tokens::typography::META.size));
        let gap = px(scale.apply(tokens::spacing::RELATED_TIGHT));
        let focused = self.focus.is_focused(window);
        let footnote = if self.song() {
            SONG_FOOTNOTE
        } else {
            ARRANGEMENT_FOOTNOTE
        };
        let body: AnyElement = match &self.load {
            Load::Loading => div()
                .debug_selector(|| "history-loading".into())
                .text_size(meta)
                .text_color(rgb(palette.text_muted))
                .child(LOADING)
                .into_any_element(),
            Load::Failed(message) => div()
                .id("history-failed")
                .debug_selector(|| "history-failed".into())
                .role(Role::Alert)
                .aria_label(message.clone())
                .child(message.clone())
                .into_any_element(),
            Load::Ready => {
                let items = self.items();
                let active = self.active_index(&items);
                let rows: Vec<AnyElement> = items
                    .iter()
                    .enumerate()
                    .map(|(at, item)| {
                        self.render_row(*item, at == active && focused, environment, cx)
                    })
                    .collect();
                div()
                    .id("history-list")
                    .debug_selector(|| "history-list".into())
                    .track_focus(&self.focus)
                    .role(Role::List)
                    .aria_label("History")
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        this.on_key(event, window, cx);
                    }))
                    .flex()
                    .flex_col()
                    .gap(gap)
                    .children(rows)
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(body)
            .child(
                div()
                    .text_size(meta)
                    .text_color(rgb(palette.text_muted))
                    .child(footnote),
            )
            .into_any_element()
    }

    fn render_row(
        &self,
        item: Item,
        ring: bool,
        environment: UiEnvironment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let meta = px(scale.apply(tokens::typography::META.size));
        let gap = px(scale.apply(tokens::spacing::RELATED_TIGHT));
        let glyph = px(scale.apply(tokens::icon::CONTROL_SIZE));
        let selected = |si: usize, ri: usize| self.selected == Some((si, ri));
        let base = |id: SharedString, selector: String, name: String| {
            div()
                .id(id)
                .debug_selector(move || selector.clone())
                .role(Role::ListItem)
                .aria_label(name)
                .flex()
                .items_center()
                .gap(gap)
                .p(gap)
                .rounded(px(scale.apply(tokens::radius::CONTROL)))
                .cursor_pointer()
                .border_1()
                .border_color(rgb(if ring { palette.focus } else { palette.border }))
        };
        match item {
            Item::Session(si) => {
                let session = &self.sessions[si];
                let open = self.expanded.contains(&si);
                let count = match session.revisions.len() {
                    1 => "1 revision".to_owned(),
                    n => format!("{n} revisions"),
                };
                base(
                    format!("history-session-{si}").into(),
                    format!("history-session-{si}"),
                    format!(
                        "{}, {} {}, {count}",
                        session.what, session.when, session.how
                    ),
                )
                .aria_expanded(open)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate(Item::Session(si), window, cx);
                }))
                .child(
                    svg()
                        .flex_none()
                        .size(glyph)
                        .text_color(rgb(palette.text_muted))
                        .data(
                            if open {
                                icon::CHEVRON_DOWN
                            } else {
                                icon::CHEVRON_RIGHT
                            }
                            .as_bytes(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(session.what.clone()),
                        )
                        .child(
                            div()
                                .text_size(meta)
                                .text_color(rgb(palette.text_muted))
                                .child(format!("{} \u{b7} {}", session.when, session.how)),
                        ),
                )
                .child(
                    div()
                        .text_size(meta)
                        .text_color(rgb(palette.text_muted))
                        .child(count),
                )
                .into_any_element()
            }
            Item::Revision(si, ri) => {
                let row = &self.sessions[si].revisions[ri];
                let name = if row.current {
                    format!("{} {}, current revision", row.at, row.what)
                } else {
                    format!("{} {}", row.at, row.what)
                };
                base(
                    format!("history-revision-{si}-{ri}").into(),
                    format!("history-revision-{si}-{ri}"),
                    name,
                )
                .ml(px(scale.apply(tokens::spacing::RELATED) * 2.0))
                .aria_selected(selected(si, ri))
                .when(selected(si, ri), |row| row.bg(rgb(palette.selection)))
                .when(row.current, |row| {
                    row.cursor_default().aria_description(format!(
                        "{}. The current revision opens no comparison.",
                        ui::primitives::Availability::UNAVAILABLE
                    ))
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate(Item::Revision(si, ri), window, cx);
                }))
                .child(
                    div()
                        .text_size(meta)
                        .text_color(rgb(palette.text_muted))
                        .child(row.at.clone()),
                )
                .child(div().flex_1().min_w_0().child(row.what.clone()))
                .child(if row.current {
                    div()
                        .debug_selector(|| "history-current".into())
                        .px(gap)
                        .rounded(px(scale.apply(tokens::radius::CONTROL)))
                        .border_1()
                        .border_color(rgb(palette.border))
                        .text_size(meta)
                        .child(CURRENT)
                        .into_any_element()
                } else {
                    svg()
                        .flex_none()
                        .size(glyph)
                        .text_color(rgb(palette.text_muted))
                        .data(icon::CHEVRON_RIGHT.as_bytes())
                        .into_any_element()
                })
                .into_any_element()
            }
        }
    }
}
