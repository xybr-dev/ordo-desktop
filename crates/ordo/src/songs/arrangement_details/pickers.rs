//! The chips and the two search pickers of the details form.
//!
//! Languages and Choirs are chosen, never typed in: each shows chips for what
//! the operator is likely to want plus a **+ Language** / **+ Choir** control
//! that opens a search under the chips. The Language picker searches Ordo's
//! built-in list and has no create row. The Choir picker searches active
//! Choirs only; when the typed name is new it offers `Create “…”` as its last
//! row, and creating saves the Choir at once.

use domain::{
    identity::ChoirId,
    song_library::{find_language, languages, match_key},
};
use gpui::{Context, Div, KeyDownEvent, ParentElement, Styled, Window, div, prelude::*, px, rgb};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{button::Button, field::Field, list::ListRow},
    tokens,
};

use super::{
    ArrangementDetailsPanel, Picker,
    form::{OCCASIONS, chip, chip_row, same_language},
};

/// One row a picker offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Row {
    /// A built-in language, with the other name that matched when the display
    /// name did not.
    Language {
        /// The display name, which is the value stored.
        name: &'static str,
        /// "also called" text.
        also: Option<&'static str>,
    },
    /// An active Choir.
    Choir {
        /// Its identity.
        id: ChoirId,
        /// Its name as authored.
        name: String,
    },
    /// Create a name-only Choir with the typed name.
    CreateChoir(String),
}

impl ArrangementDetailsPanel {
    /// The rows the open picker offers for what is typed, in order.
    pub fn picker_rows(&self, cx: &gpui::App) -> Vec<Row> {
        match self.picker {
            None => Vec::new(),
            Some(Picker::Language) => find_language(&self.language_search.value(cx))
                .into_iter()
                .filter(|found| {
                    !self
                        .languages
                        .iter()
                        .any(|staged| same_language(staged, found.language.name()))
                })
                .map(|found| Row::Language {
                    name: found.language.name(),
                    also: found.matched_alias,
                })
                .collect(),
            Some(Picker::Choir) => {
                let typed = self.choir_search.value(cx);
                let key = match_key(&typed);
                let library = self.store.read(cx).library();
                let mut hits: Vec<(String, ChoirId, String)> = library
                    .choirs()
                    .filter(|choir| !choir.is_archived())
                    .filter(|choir| !self.choirs.contains(&choir.id()))
                    .map(|choir| (match_key(choir.name()), choir.id(), choir.name().to_owned()))
                    .filter(|(choir_key, _, _)| choir_key.contains(&key))
                    .collect();
                hits.sort();
                let mut rows: Vec<Row> = hits
                    .into_iter()
                    .map(|(_, id, name)| Row::Choir { id, name })
                    .collect();
                // Every Choir counts here, archived ones included: a name that
                // exists is never created a second time.
                let exists = library.choirs().any(|choir| match_key(choir.name()) == key);
                if !key.is_empty() && !exists {
                    rows.push(Row::CreateChoir(typed.to_string()));
                }
                rows
            }
        }
    }

    /// Opens a picker with its search focused, or closes it when it is the
    /// one already open.
    pub fn toggle_picker(&mut self, kind: Picker, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker == Some(kind) {
            self.close_picker(window, cx);
            return;
        }
        self.picker = Some(kind);
        self.active = 0;
        let field = match kind {
            Picker::Language => &self.language_search,
            Picker::Choir => &self.choir_search,
        };
        field.set_value("", window, cx);
        field.focus(window, cx);
        cx.notify();
    }

    /// Closes the open picker and gives focus back to its control.
    pub fn close_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.picker.take() else {
            return;
        };
        let back = match kind {
            Picker::Language => &self.add_language_focus,
            Picker::Choir => &self.add_choir_focus,
        };
        back.focus(window, cx);
        cx.notify();
    }

    /// Takes the row at `at` of the open picker.
    pub fn choose(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.picker_rows(cx).into_iter().nth(at) else {
            return;
        };
        match row {
            Row::Language { name, .. } => {
                if !self
                    .offered
                    .iter()
                    .any(|offered| same_language(offered, name))
                {
                    self.offered.push(name.to_owned());
                }
                self.languages.push(name.to_owned());
                self.error = None;
                self.close_picker(window, cx);
            }
            Row::Choir { id, .. } => {
                self.choirs.push(id);
                self.error = None;
                self.close_picker(window, cx);
            }
            Row::CreateChoir(name) => self.create_choir(name, window, cx),
        }
        cx.notify();
    }

    /// Keys for the open picker (Escape is taken first by the form root, so the
    /// panel stays); Up and Down move the active row; Enter takes it.
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = self.picker else {
            return;
        };
        if event.keystroke.modifiers.modified() {
            return;
        }
        let rows = self.picker_rows(cx).len();
        match event.keystroke.key.as_str() {
            "enter" => {
                let field = match kind {
                    Picker::Language => &self.language_search,
                    Picker::Choir => &self.choir_search,
                };
                if field.focus_handle(cx).is_focused(window) {
                    self.choose(self.active, window, cx);
                    cx.stop_propagation();
                }
            }
            "down" if rows > 0 => {
                self.active = (self.active + 1) % rows;
                cx.notify();
                cx.stop_propagation();
            }
            "up" if rows > 0 => {
                self.active = (self.active + rows - 1) % rows;
                cx.notify();
                cx.stop_propagation();
            }
            _ => {}
        }
    }
}

/// The language chips and the **+ Language** control. The chips are the
/// built-in languages already used in the Library, then the languages offered
/// during this edit (picked, or held by the Arrangement); a language is on
/// while it is staged.
pub(super) fn language_chips(
    panel: &mut ArrangementDetailsPanel,
    environment: UiEnvironment,
    cx: &mut Context<ArrangementDetailsPanel>,
) -> Div {
    let library = panel.store.read(cx).library();
    let mut names: Vec<String> = languages()
        .iter()
        .filter(|listed| {
            library.arrangements().any(|arrangement| {
                arrangement
                    .languages()
                    .iter()
                    .any(|used| same_language(used, listed.name()))
            })
        })
        .map(|listed| listed.name().to_owned())
        .collect();
    for extra in panel.offered.iter().chain(&panel.languages) {
        if !names.iter().any(|name| same_language(name, extra)) {
            names.push(extra.clone());
        }
    }
    let weak = cx.weak_entity();
    let chips = names.into_iter().enumerate().map(|(at, name)| {
        let on = panel
            .languages
            .iter()
            .any(|staged| same_language(staged, &name));
        let weak = weak.clone();
        let value = name.clone();
        chip(
            ("ad-language-chip", at),
            name,
            on,
            environment,
            move |_, cx| {
                let _ = weak.update(cx, |panel, cx| panel.toggle_language(&value, cx));
            },
        )
    });
    let weak = cx.weak_entity();
    chip_row(environment).children(chips).child(
        Button::new("ad-add-language", "Language", environment)
            .icon(icon::PLUS)
            .on_surface(tokens::palette(environment.appearance()).surface)
            .track_focus(&panel.add_language_focus)
            .on_activate(move |window, cx| {
                let _ = weak.update(cx, |panel, cx| {
                    panel.toggle_picker(Picker::Language, window, cx);
                });
            }),
    )
}

/// The Choir chips: every active Choir, plus any staged Choir that is not
/// active, and the **+ Choir** control. A Choir is on while it is staged.
pub(super) fn choir_chips(
    panel: &mut ArrangementDetailsPanel,
    environment: UiEnvironment,
    cx: &mut Context<ArrangementDetailsPanel>,
) -> Div {
    let library = panel.store.read(cx).library();
    let mut listed: Vec<(String, ChoirId, String)> = library
        .choirs()
        .filter(|choir| !choir.is_archived() || panel.choirs.contains(&choir.id()))
        .map(|choir| (match_key(choir.name()), choir.id(), choir.name().to_owned()))
        .collect();
    listed.sort();
    let weak = cx.weak_entity();
    let chips = listed.into_iter().enumerate().map(|(at, (_, id, name))| {
        let on = panel.choirs.contains(&id);
        let weak = weak.clone();
        chip(
            ("ad-choir-chip", at),
            name,
            on,
            environment,
            move |_, cx| {
                let _ = weak.update(cx, |panel, cx| panel.toggle_choir(id, cx));
            },
        )
    });
    let weak = cx.weak_entity();
    chip_row(environment).children(chips).child(
        Button::new("ad-add-choir", "Choir", environment)
            .icon(icon::PLUS)
            .on_surface(tokens::palette(environment.appearance()).surface)
            .track_focus(&panel.add_choir_focus)
            .on_activate(move |window, cx| {
                let _ = weak.update(cx, |panel, cx| {
                    panel.toggle_picker(Picker::Choir, window, cx);
                });
            }),
    )
}

/// The occasion chips: the fixed list, then any other occasion staged.
pub(super) fn occasion_chips(
    panel: &mut ArrangementDetailsPanel,
    environment: UiEnvironment,
    cx: &mut Context<ArrangementDetailsPanel>,
) -> Div {
    let mut names: Vec<String> = OCCASIONS.iter().map(|name| (*name).to_owned()).collect();
    for staged in &panel.occasions {
        if !names.contains(staged) {
            names.push(staged.clone());
        }
    }
    let weak = cx.weak_entity();
    let chips = names.into_iter().enumerate().map(|(at, name)| {
        let on = panel.occasions.contains(&name);
        let weak = weak.clone();
        let value = name.clone();
        chip(
            ("ad-occasion-chip", at),
            name,
            on,
            environment,
            move |_, cx| {
                let _ = weak.update(cx, |panel, cx| panel.toggle_occasion(&value, cx));
            },
        )
    });
    chip_row(environment).children(chips)
}

/// The open search under a chip row, when `kind` is the open picker.
pub(super) fn picker(
    panel: &mut ArrangementDetailsPanel,
    kind: Picker,
    environment: UiEnvironment,
    _window: &mut Window,
    cx: &mut Context<ArrangementDetailsPanel>,
) -> Option<Div> {
    if panel.picker != Some(kind) {
        return None;
    }
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let rows = panel.picker_rows(cx);
    let typed = match kind {
        Picker::Language => panel.language_search.value(cx),
        Picker::Choir => panel.choir_search.value(cx),
    };
    let weak = cx.weak_entity();
    let active = panel.active.min(rows.len().saturating_sub(1));
    let items = rows
        .iter()
        .enumerate()
        .map(|(at, row)| {
            let (title, detail) = match row {
                Row::Language { name, also } => (
                    (*name).to_owned(),
                    also.map(|alias| format!("also called {alias}")),
                ),
                Row::Choir { name, .. } => (name.clone(), None),
                Row::CreateChoir(name) => (
                    format!("Create \u{201c}{name}\u{201d}"),
                    Some("New choir \u{b7} name only".to_owned()),
                ),
            };
            let weak = weak.clone();
            let mut row = ListRow::new(("ad-pick-row", at), title, environment)
                .selected(at == active)
                .on_surface(palette.surface)
                .on_activate(move |window, cx| {
                    let _ = weak.update(cx, |panel, cx| panel.choose(at, window, cx));
                });
            if let Some(detail) = detail {
                row = row.detail(detail);
            }
            div()
                .debug_selector(move || format!("ad-pick-row-{at}"))
                .w_full()
                .child(row)
        })
        .collect::<Vec<_>>();
    let none = match kind {
        Picker::Choir => "Every choir is already chosen.".to_owned(),
        Picker::Language if typed.trim().is_empty() => {
            "Every language in Ordo\u{2019}s list is already chosen.".to_owned()
        }
        Picker::Language => {
            format!("No language in Ordo\u{2019}s list matches \u{201c}{typed}\u{201d}.")
        }
    };
    let (selector, id, name, placeholder, foot, state) = match kind {
        Picker::Language => (
            "ad-picker-language",
            "ad-language-search",
            "Language",
            "Find a language",
            "Languages come from Ordo\u{2019}s list, so every Arrangement and Mass names a language the same way.",
            &panel.language_search,
        ),
        Picker::Choir => (
            "ad-picker-choir",
            "ad-choir-search",
            "Choir",
            "Find or create a choir",
            "Archived choirs aren\u{2019}t suggested.",
            &panel.choir_search,
        ),
    };
    let meta = move |text: String| {
        div()
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.text_muted))
            .child(text)
    };
    let weak = cx.weak_entity();
    Some(
        div()
            .debug_selector(move || selector.into())
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .w_full()
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .child(
                div()
                    .debug_selector(move || format!("{selector}-search"))
                    .child(
                        Field::new(id, name, state, environment)
                            .placeholder(placeholder)
                            .label_hidden()
                            .on_surface(palette.surface),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "ad-pick-list".into())
                    .flex()
                    .flex_col()
                    .w_full()
                    .children(items)
                    .when(rows.is_empty(), |list| {
                        list.child(
                            div()
                                .debug_selector(|| "ad-pick-none".into())
                                .child(meta(none)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .child(div().min_w_0().child(meta(foot.to_owned())))
                    .child(
                        Button::new("ad-pick-done", "Done", environment)
                            .quiet()
                            .on_surface(palette.surface)
                            .on_activate(move |window, cx| {
                                let _ = weak.update(cx, |panel, cx| panel.close_picker(window, cx));
                            }),
                    ),
            ),
    )
}
