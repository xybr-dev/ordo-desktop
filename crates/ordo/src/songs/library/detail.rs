//! The Songs detail pane: the selected Song's identity and its Arrangements,
//! or the one state that stands in for it. No editing controls exist in this
//! build; an archived Song is inspect-only and says so.

use domain::{
    identity::ArrangementId,
    song_library::{Arrangement, SongLibrary, SongWork},
};
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, Styled, Window, div,
    prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{Availability, button::Button, list::Badge},
    tokens,
};

use super::{OpenArrangement, SongsWorkspace, filters, list};
use crate::songs::store::LibraryState;

/// The pane the shell hosts in its viewer region.
pub(super) fn render(
    workspace: &mut SongsWorkspace,
    environment: UiEnvironment,
    open: OpenArrangement,
    window: &mut Window,
    cx: &mut Context<SongsWorkspace>,
) -> AnyElement {
    let store = workspace.store.clone();
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let pane = |content: AnyElement| {
        div()
            .debug_selector(|| "songs-detail-pane".into())
            .size_full()
            .bg(rgb(palette.background))
            .child(content)
            .into_any_element()
    };
    let store = store.read(cx);
    match store.state() {
        LibraryState::Ready => {}
        LibraryState::Loading => {
            return pane(
                list::notice(
                    "songs-detail-waiting",
                    "No song selected",
                    vec!["The selected song appears here once the library is open.".to_owned()],
                    Vec::new(),
                    environment,
                )
                .into_any_element(),
            );
        }
        LibraryState::Unavailable(message) => {
            return pane(
                list::notice(
                    "songs-detail-unavailable",
                    "No song to show",
                    vec![message.to_string()],
                    Vec::new(),
                    environment,
                )
                .into_any_element(),
            );
        }
    }
    let library = store.library();
    if library.songs().next().is_none() {
        return pane(
            list::notice(
                "songs-detail-steps",
                "Build your parish Song Library",
                vec![
                    "1. Create or import a Song. Its title, composer and aliases identify the composition."
                        .to_owned(),
                    "2. Add an Arrangement. Write its lyrics in sections, then set the Sequence they\u{2019}re sung in."
                        .to_owned(),
                    "3. Say who sings it. Choirs, languages and occasions help you find the right one when planning a Mass."
                        .to_owned(),
                ],
                Vec::new(),
                environment,
            )
            .into_any_element(),
        );
    }
    let selected = workspace
        .selected
        .and_then(|id| library.song(id).ok().map(|song| (id, song)));
    let Some((id, song)) = selected else {
        return pane(
            list::notice(
                "songs-detail-none",
                "No song selected",
                vec!["Choose a song in the list to see its Arrangements.".to_owned()],
                Vec::new(),
                environment,
            )
            .into_any_element(),
        );
    };
    let hit = workspace.hits.iter().find(|hit| hit.song == id);
    let matching: Vec<ArrangementId> = hit
        .map(|hit| hit.matching_arrangements.clone())
        .unwrap_or_default();
    let narrowed = filters::any_active(&workspace.query) || !workspace.query.text.trim().is_empty();
    let arrangements: Vec<&Arrangement> = library
        .arrangements_for_song(id)
        .unwrap_or_default()
        .into_iter()
        .filter(|arrangement| workspace.query.include_archived || !arrangement.is_archived())
        .collect();
    let total = arrangements.len();
    // Matching Arrangements first, in library order; the rest stay visible.
    let (first, rest): (Vec<_>, Vec<_>) = arrangements
        .into_iter()
        .partition(|arrangement| !narrowed || matching.contains(&arrangement.id()));
    let ordered = first
        .iter()
        .map(|arrangement| (*arrangement, true))
        .chain(rest.iter().map(|arrangement| (*arrangement, false)))
        .collect::<Vec<_>>();

    let weak = cx.weak_entity();
    let rows = ordered
        .into_iter()
        .enumerate()
        .map(|(at, (arrangement, matches))| {
            let open = open.clone();
            let weak = weak.clone();
            let arrangement_id = arrangement.id();
            arrangement_row(
                library,
                song,
                arrangement,
                at,
                matches,
                environment,
                move |window, cx| {
                    let _ = weak.update(cx, |workspace, _| workspace.remember_scroll());
                    open(arrangement_id, window, cx);
                },
            )
        })
        .collect::<Vec<_>>();

    let count = if narrowed {
        format!("{} matching \u{b7} {total} total", first.len())
    } else {
        format!("{total} total")
    };
    let archived_notice = song.is_archived().then(|| {
        div()
            .debug_selector(|| "songs-archived-notice".into())
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .w_full()
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .bg(rgb(palette.surface_secondary))
            .text_color(rgb(palette.text))
            .child("Archived \u{2014} restore it to edit")
    });
    let scroll = workspace.detail_scroll.clone();
    let body = div()
        .debug_selector(|| "songs-detail".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .w_full()
        .p(px(scale.apply(tokens::spacing::GROUP)))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("Song"),
                )
                .child(
                    div()
                        .debug_selector(|| "songs-detail-title".into())
                        .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(song.title().to_owned()),
                ),
        )
        .children(archived_notice)
        .child(metadata(song, environment))
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child("Arrangements"),
                )
                .child(
                    div()
                        .debug_selector(|| "songs-arrangement-count".into())
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(count),
                ),
        )
        .child(
            div()
                .debug_selector(|| "songs-arrangements".into())
                .flex()
                .flex_col()
                .w_full()
                .when(rows.is_empty(), |list| {
                    list.child(no_arrangements(song.is_archived(), environment))
                })
                .children(rows),
        );
    pane(
        div()
            .relative()
            .size_full()
            .child(
                scroll
                    .bounded(div().id("songs-detail-scroll"))
                    .size_full()
                    .child(body),
            )
            .children(scroll.indicator("songs-detail-scroll", 0.0, window, cx, environment))
            .into_any_element(),
    )
}

/// Why Arrangement creation is unavailable in this build.
const ADD_ARRANGEMENT_REASON: &str = "Arrangements arrive in a later build";

/// A Song with no Arrangements: what one is, and the action that makes the
/// first. An archived Song is inspect-only, so it offers no action.
fn no_arrangements(archived: bool, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "songs-no-arrangements".into())
        .flex()
        .flex_col()
        .items_start()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .py(px(scale.apply(tokens::spacing::GROUP)))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child("No Arrangements yet"),
        )
        .child(
            div().text_color(rgb(palette.text_muted)).child(
                "An Arrangement holds one rendition\u{2019}s lyrics, sections and Sequence for a language and choir.",
            ),
        )
        .when(!archived, |block| {
            block.child(
                div()
                    .debug_selector(|| "songs-add-arrangement".into())
                    .child(
                        Button::new(
                            "songs-add-arrangement-button",
                            "Add first Arrangement",
                            environment,
                        )
                        .availability(Availability::unavailable(ADD_ARRANGEMENT_REASON)),
                    ),
            )
        })
}

/// The Song's identity facts that exist: aliases, attribution, moments and
/// occasions. Empty ones are left out rather than shown blank.
fn metadata(song: &SongWork, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let mut facts: Vec<(&'static str, String)> = Vec::new();
    if !song.aliases().is_empty() {
        facts.push(("Also known as", song.aliases().join(", ")));
    }
    if let Some(text) = song.original_attribution() {
        facts.push(("Attribution", text.to_owned()));
    }
    if let Some(text) = song.provenance() {
        facts.push(("Provenance", text.to_owned()));
    }
    if !song.liturgical_moments().is_empty() {
        let moments: Vec<&str> = song
            .liturgical_moments()
            .iter()
            .map(String::as_str)
            .collect();
        facts.push(("Liturgical moments", moments.join(", ")));
    }
    if !song.occasions().is_empty() {
        let occasions: Vec<&str> = song.occasions().iter().map(String::as_str).collect();
        facts.push(("Occasions", occasions.join(", ")));
    }
    div()
        .debug_selector(|| "songs-detail-meta".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .children(facts.into_iter().map(|(label, value)| {
            div()
                .flex()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(
                    div()
                        .flex_none()
                        .w(px(scale.apply(140.0)))
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(label),
                )
                .child(div().min_w_0().text_color(rgb(palette.text)).child(value))
        }))
}

/// One Arrangement of the selected Song.
fn arrangement_row(
    library: &SongLibrary,
    song: &SongWork,
    arrangement: &Arrangement,
    at: usize,
    matches: bool,
    environment: UiEnvironment,
    open: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    // Arrangements that do not match stay readable but step back.
    let ink = if matches {
        palette.text
    } else {
        palette.text_muted
    };
    let languages = if arrangement.languages().is_empty() {
        "No language".to_owned()
    } else {
        arrangement.languages().join(", ")
    };
    let choirs: Vec<&str> = arrangement
        .choirs()
        .iter()
        .filter_map(|id| library.choir(*id).ok().map(|choir| choir.name()))
        .collect();
    let choirs = if choirs.is_empty() {
        "No choir".to_owned()
    } else {
        choirs.join(", ")
    };
    let title = library
        .arrangement_label(arrangement.id())
        .unwrap_or_default();
    // Archived Songs and Arrangements are inspect-only.
    let open_label = if arrangement.is_archived() || song.is_archived() {
        "Inspect"
    } else {
        "Open"
    };
    let mut badges = Vec::new();
    if arrangement.is_archived() {
        badges.push(Badge::new("Archived", environment));
    } else if song.is_archived() {
        badges.push(Badge::new("Available when Song is restored", environment));
    }
    div()
        .debug_selector(move || format!("songs-arrangement-{at}"))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .w_full()
        .py(px(scale.apply(tokens::spacing::RELATED)))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(ink))
                                .child(title.clone()),
                        )
                        .children(badges),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(format!("{languages} \u{b7} {choirs}")),
                ),
        )
        .child(
            div()
                .flex_none()
                .debug_selector(move || format!("arrangement-open-{at}"))
                .child(
                    Button::new(("arrangement-open-button", at), open_label, environment)
                        .name(format!("{open_label} {title}"))
                        .on_surface(palette.background)
                        .on_activate(open),
                ),
        )
}
