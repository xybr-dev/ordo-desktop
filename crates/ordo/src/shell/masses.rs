//! Masses → Upcoming: what Ordo opens on (the `masses-index` mockup).
//!
//! Each Celebration is a group with its date, rank, liturgical color, Mass
//! count, and a readiness roll-up, and its Mass plans sit under it with time,
//! language, choir, presider, and readiness (Decision 83). A row opens that
//! plan. The records are [`fixtures`] until T1 reads them from storage; the
//! layout, the filters, and the readiness wording are the product's.

use std::collections::HashSet;

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px, rgb, svg,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        button::Button,
        field::{Field, FieldState},
        record::{Chip, Marker, RecordRow, Status, StatusTone},
        selection::{Choice, Select, SelectionState},
    },
    scroll::Scroll,
    tokens,
};

use super::fixtures::{self, Celebration, Mass};

/// Activation of one control, built per control by the shell.
pub type Activate = Box<dyn Fn(&mut Window, &mut App)>;

const LANGUAGES: [&str; 4] = ["All", "Cebuano", "English", "Tagalog"];
const CHOIRS: [&str; 5] = [
    "All",
    "Koro San Roque",
    "Koro Sta. Cecilia",
    "Youth Choir",
    "No choir",
];
const READINESS: [&str; 3] = ["Any", "Ready", "Needs work"];
/// Mass rows start under the Celebration's title, past its chevron and swatch.
const ROW_INSET: f32 = 48.0;
/// The index reads as one column, not a full-bleed table, on a wide window.
const MAX_WIDTH: f32 = 1180.0;
/// The liturgical color swatch beside a Celebration's title.
const SWATCH: (f32, f32) = (12.0, 28.0);

/// The index's own state: the filter bar and which groups are folded.
pub struct MassesIndex {
    search: FieldState,
    language: SelectionState,
    choir: SelectionState,
    readiness: SelectionState,
    collapsed: HashSet<usize>,
    scroll: Scroll,
}

impl MassesIndex {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let select = |choices: &[&'static str], cx: &mut App| {
            SelectionState::new(
                choices.iter().map(|choice| Choice::new(*choice)).collect(),
                Some(0),
                cx,
            )
        };
        Self {
            search: FieldState::new(window, cx),
            language: select(&LANGUAGES, cx),
            choir: select(&CHOIRS, cx),
            readiness: select(&READINESS, cx),
            collapsed: HashSet::new(),
            scroll: Scroll::new(),
        }
    }

    /// Folds or unfolds one Celebration's Mass plans.
    pub fn toggle(&mut self, celebration: usize) {
        if !self.collapsed.remove(&celebration) {
            self.collapsed.insert(celebration);
        }
    }

    pub fn is_collapsed(&self, celebration: usize) -> bool {
        self.collapsed.contains(&celebration)
    }

    fn chosen(state: &SelectionState, choices: &[&'static str], cx: &App) -> Option<&'static str> {
        state
            .selected(cx)
            .filter(|&index| index > 0)
            .map(|index| choices[index])
    }

    fn matches(&self, mass: &Mass, cx: &App) -> bool {
        let language = Self::chosen(&self.language, &LANGUAGES, cx);
        let choir = Self::chosen(&self.choir, &CHOIRS, cx);
        let readiness = Self::chosen(&self.readiness, &READINESS, cx);
        language.is_none_or(|language| mass.language == language)
            && choir.is_none_or(|choir| mass.choir.unwrap_or("No choir") == choir)
            && readiness.is_none_or(|readiness| (readiness == "Ready") == ready(mass))
    }

    fn searched(&self, celebration: &Celebration, cx: &App) -> bool {
        let query = self.search.value(cx).trim().to_lowercase();
        query.is_empty() || {
            let mut haystack = format!("{} {}", celebration.title, celebration.date);
            for mass in &celebration.masses {
                haystack.push_str(&format!(
                    " {} {} {}",
                    mass.choir.unwrap_or_default(),
                    mass.presider,
                    mass.language
                ));
            }
            haystack.to_lowercase().contains(&query)
        }
    }
}

fn ready(mass: &Mass) -> bool {
    mass.incomplete == 0 && mass.warnings == 0
}

/// One Mass plan's readiness, worded and shaped as the index shows it.
fn readiness(mass: &Mass, environment: UiEnvironment) -> Status {
    if ready(mass) {
        return Status::new("Ready", StatusTone::Live, Marker::Dot, environment);
    }
    let mut parts = Vec::new();
    if mass.incomplete > 0 {
        parts.push(format!("{} incomplete", mass.incomplete));
    }
    if mass.warnings > 0 {
        parts.push(format!(
            "{} warning{}",
            mass.warnings,
            if mass.warnings == 1 { "" } else { "s" }
        ));
    }
    if mass.incomplete > 0 {
        Status::new(
            parts.join(" \u{b7} "),
            StatusTone::Danger,
            Marker::Diamond,
            environment,
        )
    } else {
        Status::new(
            parts.join(" \u{b7} "),
            StatusTone::Warning,
            Marker::Square,
            environment,
        )
    }
}

/// A Celebration's roll-up: how many of its plans are ready.
fn rollup(celebration: &Celebration, environment: UiEnvironment) -> Status {
    let total = celebration.masses.len();
    let ok = celebration.masses.iter().filter(|mass| ready(mass)).count();
    if total == 0 {
        Status::new(
            "No Mass plans",
            StatusTone::Danger,
            Marker::Diamond,
            environment,
        )
    } else if ok == total {
        Status::new(
            format!("All {total} ready"),
            StatusTone::Live,
            Marker::Dot,
            environment,
        )
    } else if celebration.masses.iter().any(|mass| mass.incomplete > 0) {
        Status::new(
            format!("{ok} of {total} ready"),
            StatusTone::Danger,
            Marker::Diamond,
            environment,
        )
    } else {
        Status::new(
            format!("{ok} of {total} ready"),
            StatusTone::Warning,
            Marker::Square,
            environment,
        )
    }
}

/// What the shell does when the index asks for something.
pub struct Actions {
    pub toggle: Box<dyn Fn(usize) -> Activate>,
    pub add_mass: Box<dyn Fn(&'static str) -> Activate>,
    pub open: Box<dyn Fn(SharedString) -> Activate>,
}

/// The filter bar under the header: search, then language, choir, and
/// readiness.
fn filters(index: &MassesIndex, gutter: f32, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "masses-filters".into())
        .flex_none()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(gutter))
        .py(px(
            scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
        ))
        .bg(rgb(palette.surface))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div().flex_1().max_w(px(scale.apply(420.0))).child(
                Field::new("masses-search", "Search", &index.search, environment)
                    .placeholder("Search Celebrations, choirs, presiders")
                    .label_hidden()
                    .on_surface(palette.surface),
            ),
        )
        .child(
            Select::new("masses-language", "Language", &index.language, environment)
                .on_surface(palette.surface),
        )
        .child(
            Select::new("masses-choir", "Choir", &index.choir, environment)
                .on_surface(palette.surface),
        )
        .child(
            Select::new(
                "masses-readiness",
                "Readiness",
                &index.readiness,
                environment,
            )
            .on_surface(palette.surface),
        )
}

fn mass_row(mass: &Mass, open: Activate, environment: UiEnvironment) -> RecordRow {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let choir = mass.choir.unwrap_or("No choir");
    let status = readiness(mass, environment);
    let name = format!(
        "{}{}, {}, {}, {}",
        mass.time,
        if mass.anticipated {
            ", Anticipated"
        } else {
            ""
        },
        mass.language,
        choir,
        mass.presider
    );
    RecordRow::new(
        SharedString::from(format!("mass-{}", mass.time)),
        name,
        environment,
    )
    .inset(ROW_INSET)
    .on_surface(palette.surface)
    .cell(
        1.3,
        div()
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(mass.time),
            )
            .when(mass.anticipated, |time| {
                time.child(Chip::new("Anticipated", environment))
            }),
    )
    .cell(0.7, mass.language)
    .cell(
        1.0,
        div()
            .when(mass.choir.is_none(), |cell| {
                cell.text_color(rgb(palette.text_muted))
            })
            .child(choir),
    )
    .cell(
        1.0,
        div()
            .text_color(rgb(palette.text_muted))
            .child(mass.presider),
    )
    .cell(1.1, status)
    .trailing(
        svg()
            .flex_none()
            .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
            .text_color(rgb(palette.text_muted))
            .data(icon::CHEVRON_RIGHT.as_bytes()),
    )
    .on_activate(open)
}

fn group(
    at: usize,
    celebration: &Celebration,
    index: &MassesIndex,
    actions: &Actions,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let collapsed = index.is_collapsed(at);
    // The group's plans grow from its header down while fading in, and its
    // chevron turns with them; folding reverses both (`DESIGN.md`).
    let fold_id = SharedString::from(format!("celebration-fold-{at}"));
    let open = ui::motion::fold(fold_id.clone(), !collapsed, window, cx);
    let title = celebration.title;
    let count = celebration.masses.len();
    let meta = format!(
        "{} \u{b7} {} \u{b7} {} \u{b7} {count} Mass{}",
        celebration.date,
        celebration.rank,
        celebration.color,
        if count == 1 { "" } else { "es" }
    );
    let head = div()
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(
            scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
        ))
        .py(px(scale.apply(tokens::spacing::RELATED)))
        .child(
            Button::new(
                SharedString::from(format!("fold-{at}")),
                format!("{} {title}", if collapsed { "Expand" } else { "Collapse" }),
                environment,
            )
            .icon(icon::CHEVRON_RIGHT)
            .icon_turn(open)
            .icon_only()
            .quiet()
            .on_surface(palette.surface)
            .on_activate((actions.toggle)(at)),
        )
        .child(
            div()
                .flex_none()
                .w(px(scale.apply(SWATCH.0)))
                .h(px(scale.apply(SWATCH.1)))
                .rounded(px(scale.apply(tokens::radius::CONTROL)))
                .border(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border_strong))
                .bg(rgb(celebration.swatch)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .min_w_0()
                        .child(
                            div()
                                .debug_selector(move || format!("celebration-{title}"))
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .when(celebration.installed, |row| {
                            row.child(
                                Chip::new("Installed package", environment).icon(icon::PACKAGE),
                            )
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(palette.text_muted))
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .child(meta),
                ),
        )
        .child(rollup(celebration, environment))
        .child(
            Button::new(
                SharedString::from(format!("add-mass-{at}")),
                "Add Mass",
                environment,
            )
            .name(format!("Add Mass to {title}"))
            .icon(icon::PLUS)
            .quiet()
            .on_surface(palette.surface)
            .on_activate((actions.add_mass)(title)),
        );

    let note = |text: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(px(
                scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
            ))
            .pl(px(scale.apply(ROW_INSET)))
            .pr(px(
                scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
            ))
            .py(px(scale.apply(tokens::spacing::RELATED)))
            .border_t(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .text_color(rgb(palette.text_muted))
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .child(text)
    };
    let shown: Vec<&Mass> = celebration
        .masses
        .iter()
        .filter(|mass| index.matches(mass, cx))
        .collect();
    let rows: Vec<AnyElement> = if open == 0.0 {
        Vec::new()
    } else if celebration.masses.is_empty() {
        vec![
            note("No Mass plans yet. A Celebration needs one before it can be presented or exported.")
                .into_any_element(),
        ]
    } else if shown.is_empty() {
        vec![note("No Mass plans match the filters.").into_any_element()]
    } else {
        shown
            .into_iter()
            .map(|mass| {
                let name = SharedString::from(format!("{title} \u{b7} {}", mass.time));
                mass_row(mass, (actions.open)(name), environment).into_any_element()
            })
            .collect()
    };

    div()
        .debug_selector(move || format!("celebration-group-{title}"))
        .flex_none()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .border(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .bg(rgb(palette.surface))
        .child(head)
        .children((!rows.is_empty()).then(|| {
            let (natural, record) = ui::motion::natural_height(fold_id, window, cx);
            let content = div().flex_none().flex().flex_col().w_full().children(rows);
            div()
                .flex()
                .flex_col()
                .w_full()
                .on_children_prepainted(record)
                // Clipped only while it moves: once open nothing is cut off,
                // and a group that reflows keeps all of its plans in view.
                .when(open < 1.0, |container| {
                    container
                        .overflow_hidden()
                        .opacity(open)
                        .h(px(natural.unwrap_or(0.0) * open))
                })
                .child(content)
        }))
}

/// The whole of Masses → Upcoming beneath the header: the filter bar and the
/// scrolling column of Celebrations.
pub fn upcoming(
    index: &MassesIndex,
    gutter: f32,
    actions: Actions,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let celebrations = fixtures::upcoming();
    let found: Vec<usize> = (0..celebrations.len())
        .filter(|&at| index.searched(&celebrations[at], cx))
        .collect();
    let groups: Vec<Div> = found
        .into_iter()
        .map(|at| {
            group(
                at,
                &celebrations[at],
                index,
                &actions,
                environment,
                window,
                cx,
            )
        })
        .collect();
    let empty = groups.is_empty().then(|| {
        div()
            .debug_selector(|| "masses-none".into())
            .text_color(rgb(palette.text_muted))
            .child("No Celebrations match the search and filters.")
    });
    let scroll = index.scroll.clone();
    div()
        .debug_selector(|| "masses-index".into())
        .size_full()
        .flex()
        .flex_col()
        .child(filters(index, gutter, environment))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    scroll.bounded(div().id("masses-scroll")).size_full().child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(
                                scale.apply(tokens::spacing::RELATED + tokens::spacing::UNIT)
                            ))
                            .w_full()
                            .max_w(px(scale.apply(MAX_WIDTH)))
                            .p(px(gutter))
                            .children(groups)
                            .children(empty),
                    ),
                )
                .children(scroll.indicator("masses-scroll", 0.0, window, cx, environment)),
        )
        .into_any_element()
}
