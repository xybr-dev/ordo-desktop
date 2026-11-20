//! The filter bar over the Songs list: search, the Choir / Language /
//! Occasion filters, active-filter chips and the Include archived toggle.
//!
//! The values of each filter come from the library: every Choir by name, every
//! language and occasion in use, and Unspecified. There is no multi-select
//! primitive, so each filter is a toggle that opens an inline panel of option
//! buttons, and each active value is shown as a removable chip. Filters are
//! OR-ed within one filter and AND-ed across filters (the domain's rule).

use std::collections::BTreeSet;

use domain::{
    identity::ChoirId,
    song_library::{FilterValue, LibraryQuery, SongLibrary},
};
use gpui::{Context, Div, ParentElement, Styled, div, prelude::*, px, rgb};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{button::Button, field::Field, record::Chip},
    tokens,
};

use super::SongsWorkspace;

/// Which filter an option panel belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterKind {
    /// Arrangements performed by a Choir.
    Choir,
    /// Arrangements in a language.
    Language,
    /// Arrangements for an occasion.
    Occasion,
}

impl FilterKind {
    const ALL: [Self; 3] = [Self::Choir, Self::Language, Self::Occasion];

    fn label(self) -> &'static str {
        match self {
            Self::Choir => "Choir",
            Self::Language => "Language",
            Self::Occasion => "Occasion",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Choir => "choir",
            Self::Language => "language",
            Self::Occasion => "occasion",
        }
    }
}

/// One value a filter can take.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Choice {
    Choir(FilterValue<ChoirId>),
    Language(FilterValue<String>),
    Occasion(FilterValue<String>),
}

impl Choice {
    fn is_on(&self, query: &LibraryQuery) -> bool {
        match self {
            Self::Choir(value) => query.choirs.contains(value),
            Self::Language(value) => query.languages.contains(value),
            Self::Occasion(value) => query.occasions.contains(value),
        }
    }

    fn toggle(&self, query: &mut LibraryQuery) {
        fn flip<T: Ord + Clone>(set: &mut BTreeSet<T>, value: &T) {
            if !set.remove(value) {
                set.insert(value.clone());
            }
        }
        match self {
            Self::Choir(value) => flip(&mut query.choirs, value),
            Self::Language(value) => flip(&mut query.languages, value),
            Self::Occasion(value) => flip(&mut query.occasions, value),
        }
    }
}

/// The values the library offers for one filter, labeled, then Unspecified.
///
/// Options follow the same archive rule as the results. Language and Occasion
/// options come only from Songs and Arrangements the list can show, so archived
/// ones contribute nothing unless Include archived is on (an Arrangement of an
/// archived Song is hidden with it). Choir options leave out archived Choirs
/// unless Include archived is on.
fn options(
    library: &SongLibrary,
    kind: FilterKind,
    include_archived: bool,
) -> Vec<(String, Choice)> {
    let mut found: Vec<(String, Choice)> = match kind {
        FilterKind::Choir => {
            let mut choirs: Vec<_> = library
                .choirs()
                .filter(|choir| include_archived || !choir.is_archived())
                .map(|choir| (choir.name().to_owned(), choir.id()))
                .collect();
            choirs.sort();
            choirs
                .into_iter()
                .map(|(name, id)| (name, Choice::Choir(FilterValue::Is(id))))
                .collect()
        }
        FilterKind::Language => visible_arrangements(library, include_archived)
            .into_iter()
            .flat_map(|arrangement| arrangement.languages().iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|language| {
                (
                    language.clone(),
                    Choice::Language(FilterValue::Is(language)),
                )
            })
            .collect(),
        FilterKind::Occasion => library
            .songs()
            .filter(|song| include_archived || !song.is_archived())
            .flat_map(|song| song.occasions().iter().cloned())
            .chain(
                visible_arrangements(library, include_archived)
                    .into_iter()
                    .flat_map(|arrangement| arrangement.occasions().iter().cloned()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|occasion| {
                (
                    occasion.clone(),
                    Choice::Occasion(FilterValue::Is(occasion)),
                )
            })
            .collect(),
    };
    found.push((
        "Unspecified".to_owned(),
        match kind {
            FilterKind::Choir => Choice::Choir(FilterValue::Unspecified),
            FilterKind::Language => Choice::Language(FilterValue::Unspecified),
            FilterKind::Occasion => Choice::Occasion(FilterValue::Unspecified),
        },
    ));
    found
}

/// The Arrangements the list can show: not archived, and not under an archived
/// Song, unless `include_archived`.
fn visible_arrangements(
    library: &SongLibrary,
    include_archived: bool,
) -> Vec<&domain::song_library::Arrangement> {
    library
        .arrangements()
        .filter(|arrangement| {
            include_archived
                || (!arrangement.is_archived()
                    && library
                        .song(arrangement.song())
                        .is_ok_and(|song| !song.is_archived()))
        })
        .collect()
}

/// The text a filter value shows.
fn text_of(value: &FilterValue<String>) -> String {
    match value {
        FilterValue::Is(text) => text.clone(),
        FilterValue::Unspecified => "Unspecified".to_owned(),
    }
}

/// Every active filter value, labeled for a chip, in a stable order.
fn active(library: &SongLibrary, query: &LibraryQuery) -> Vec<(String, Choice)> {
    let mut chips = Vec::new();
    for choice in &query.choirs {
        let name = match choice {
            FilterValue::Is(id) => library.choir(*id).map_or_else(
                |_| "Unknown choir".to_owned(),
                |choir| choir.name().to_owned(),
            ),
            FilterValue::Unspecified => "Unspecified".to_owned(),
        };
        chips.push((format!("Choir: {name}"), Choice::Choir(choice.clone())));
    }
    for choice in &query.languages {
        chips.push((
            format!("Language: {}", text_of(choice)),
            Choice::Language(choice.clone()),
        ));
    }
    for choice in &query.occasions {
        chips.push((
            format!("Occasion: {}", text_of(choice)),
            Choice::Occasion(choice.clone()),
        ));
    }
    chips
}

/// Whether any Arrangement filter is on.
pub(super) fn any_active(query: &LibraryQuery) -> bool {
    !(query.choirs.is_empty() && query.languages.is_empty() && query.occasions.is_empty())
}

/// What a no-match message calls the active filters, for example
/// `Choir: Koro San Roque, Language: Cebuano`.
pub(super) fn describe(library: &SongLibrary, query: &LibraryQuery) -> String {
    active(library, query)
        .into_iter()
        .map(|(label, _)| label)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The search field, the filter row, the active chips and the open panel.
pub(super) fn bar(
    workspace: &mut SongsWorkspace,
    environment: UiEnvironment,
    cx: &mut Context<SongsWorkspace>,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let gap = px(scale.apply(tokens::spacing::RELATED));
    let store = workspace.store.clone();
    let library = store.read(cx).library();
    let query = workspace.query.clone();

    let toggles = FilterKind::ALL.map(|kind| {
        let count = match kind {
            FilterKind::Choir => query.choirs.len(),
            FilterKind::Language => query.languages.len(),
            FilterKind::Occasion => query.occasions.len(),
        };
        let open = workspace.open_filter == Some(kind);
        let label = if count > 0 {
            format!("{} \u{b7} {count}", kind.label())
        } else {
            kind.label().to_owned()
        };
        div()
            .debug_selector(move || format!("songs-filter-{}", kind.key()))
            .child(
                Button::new(
                    format!("songs-filter-{}-button", kind.key()),
                    label,
                    environment,
                )
                .name(format!("{} filter", kind.label()))
                .icon(icon::CHEVRON_DOWN)
                .icon_trailing()
                .current(open || count > 0)
                .on_surface(palette.surface)
                .on_activate(SongsWorkspace::act(cx, move |workspace, _, cx| {
                    workspace.open_filter = (workspace.open_filter != Some(kind)).then_some(kind);
                    cx.notify();
                })),
            )
    });

    let archived = query.include_archived;
    let include = div()
        .debug_selector(|| "songs-include-archived".into())
        .child(
            Button::new(
                "songs-include-archived-button",
                "Include archived",
                environment,
            )
            .current(archived)
            .when(archived, |button| button.icon(icon::CHECK))
            .on_surface(palette.surface)
            .on_activate(SongsWorkspace::act(cx, move |workspace, _, cx| {
                workspace.set_include_archived(!archived, cx);
            })),
        );

    let chips = active(library, &query)
        .into_iter()
        .enumerate()
        .map(|(at, (label, choice))| {
            div()
                .debug_selector(move || format!("songs-chip-{at}"))
                .flex()
                .flex_none()
                .items_center()
                .child(Chip::new(label.clone(), environment))
                .child(
                    div()
                        .debug_selector(move || format!("songs-chip-remove-{at}"))
                        .child(
                            Button::new(
                                format!("songs-chip-remove-{at}-button"),
                                format!("Remove filter {label}"),
                                environment,
                            )
                            .icon(icon::X)
                            .icon_only()
                            .quiet()
                            .control_height(tokens::density::COMPACT_CONTROL)
                            .on_surface(palette.surface)
                            .on_activate(SongsWorkspace::act(cx, move |workspace, _, cx| {
                                choice.toggle(&mut workspace.query);
                                workspace.refresh(cx);
                            })),
                        ),
                )
        })
        .collect::<Vec<_>>();

    let panel = workspace.open_filter.map(|kind| {
        let choices = options(library, kind, query.include_archived)
            .into_iter()
            .enumerate()
            .map(|(at, (label, choice))| {
                let on = choice.is_on(&query);
                div()
                    .debug_selector(move || format!("songs-option-{at}"))
                    .child(
                        Button::new(format!("songs-option-{at}-button"), label, environment)
                            .stretch()
                            .truncate()
                            .quiet()
                            .current(on)
                            .when(on, |button| button.icon(icon::CHECK))
                            .on_surface(palette.surface_secondary)
                            .on_activate(SongsWorkspace::act(cx, move |workspace, _, cx| {
                                choice.toggle(&mut workspace.query);
                                workspace.refresh(cx);
                            })),
                    )
            })
            .collect::<Vec<_>>();
        div()
            .debug_selector(|| "songs-filter-panel".into())
            .flex()
            .flex_col()
            .w_full()
            .p(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .bg(rgb(palette.surface_secondary))
            .children(choices)
    });

    div()
        .debug_selector(|| "songs-filters".into())
        .flex_none()
        .flex()
        .flex_col()
        .gap(gap)
        .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .bg(rgb(palette.surface))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div().debug_selector(|| "songs-search".into()).child(
                Field::new(
                    "songs-search-field",
                    "Search songs",
                    &workspace.search,
                    environment,
                )
                .placeholder("Search titles, lyrics, choirs\u{2026}")
                .label_hidden()
                .on_surface(palette.surface),
            ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(gap)
                .children(toggles)
                .child(include),
        )
        .children(panel)
        .when(!chips.is_empty(), |bar| {
            bar.child(
                div()
                    .debug_selector(|| "songs-chips".into())
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(gap)
                    .children(chips),
            )
        })
}
