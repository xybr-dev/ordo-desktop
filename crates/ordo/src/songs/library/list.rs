//! The Songs list pane: header, search and filters, and one ruled column of
//! result rows, or the one state that replaces the rows (loading, unavailable,
//! empty library, archived-only, no match).

use domain::song_library::{Evidence, SongHit, SongLibrary};
use gpui::{
    AnyElement, Context, Div, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        Availability,
        button::{Button, Tone},
        list::{self, Badge, ListRow},
    },
    tokens,
};

use super::{SongsWorkspace, filters};
use crate::songs::store::LibraryState;

/// Why Song creation is unavailable in this build.
pub(super) const CREATE_REASON: &str = "Song creation arrives in a later build";
/// Why `.ordo` import is unavailable in this build.
pub(super) const IMPORT_REASON: &str = "Import arrives in a later build";

/// A short state block: a title, a line or more of explanation, and actions.
pub(super) fn notice(
    selector: &'static str,
    title: impl Into<SharedString>,
    lines: Vec<String>,
    actions: Vec<AnyElement>,
    environment: UiEnvironment,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(move || selector.into())
        .flex()
        .flex_col()
        .items_start()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .w_full()
        .p(px(scale.apply(tokens::spacing::GROUP)))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child(title.into()),
        )
        .children(lines.into_iter().map(|line| {
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child(line)
        }))
        .children(actions)
}

/// The evidence line under a Song's title: shown only when the match was not
/// on the title itself.
pub(super) fn evidence_line(hit: &SongHit) -> Option<String> {
    let first = hit.evidence.first()?;
    Some(match first {
        Evidence::Title => return None,
        Evidence::Alias { text } => format!("Alias \u{b7} {text}"),
        Evidence::Attribution { text, .. } => format!("Attribution \u{b7} {text}"),
        Evidence::ArrangementTitle { text, .. } => format!("Arrangement \u{b7} {text}"),
        Evidence::Choir { name, .. } => format!("Choir \u{b7} {name}"),
        Evidence::Language { text, .. } => format!("Language \u{b7} {text}"),
        Evidence::Lyric { line, .. } => format!("Lyric \u{b7} {line}"),
    })
}

/// `m of n arrangements`, shown when a filter is active or not every
/// Arrangement matches.
pub(super) fn count_line(hit: &SongHit, filtered: bool) -> Option<String> {
    let matching = hit.matching_arrangements.len();
    (filtered || matching < hit.total_arrangements).then(|| {
        format!(
            "{matching} of {} arrangement{}",
            hit.total_arrangements,
            if hit.total_arrangements == 1 { "" } else { "s" }
        )
    })
}

/// The pane the shell hosts in its list region.
pub(super) fn render(
    workspace: &mut SongsWorkspace,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<SongsWorkspace>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let store = workspace.store.clone();
    let state = store.read(cx).state().clone();

    let header = div()
        .debug_selector(|| "songs-header".into())
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .py(px(scale.apply(tokens::spacing::RELATED)))
        .bg(rgb(palette.surface))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child("Songs"),
        )
        .child(
            div().debug_selector(|| "songs-new".into()).child(
                Button::new("songs-new-button", "New song", environment)
                    .icon(icon::PLUS)
                    .tone(Tone::Primary)
                    .availability(Availability::unavailable(CREATE_REASON))
                    .on_surface(palette.surface),
            ),
        );

    let body: AnyElement = match state {
        LibraryState::Loading => notice(
            "songs-loading",
            "Opening the parish library",
            vec!["Songs appear here as soon as the library is read.".to_owned()],
            Vec::new(),
            environment,
        )
        .into_any_element(),
        LibraryState::Unavailable(message) => notice(
            "songs-unavailable",
            "The parish library couldn't be opened.",
            vec![message.to_string()],
            Vec::new(),
            environment,
        )
        .into_any_element(),
        LibraryState::Ready => ready(workspace, &store, environment, window, cx),
    };

    div()
        .debug_selector(|| "songs-list-pane".into())
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(body)
        .into_any_element()
}

/// The list pane once the library is readable.
fn ready(
    workspace: &mut SongsWorkspace,
    store: &gpui::Entity<crate::songs::store::LibraryStore>,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<SongsWorkspace>,
) -> AnyElement {
    let library = store.read(cx).library();
    if library.songs().next().is_none() {
        return empty_library(environment).into_any_element();
    }
    let bar = filters::bar(workspace, environment, cx);
    let library = store.read(cx).library();
    let query = workspace.query.clone();
    let untouched = query.text.trim().is_empty() && !filters::any_active(&query);
    let everything_archived = library.songs().all(|song| song.is_archived());

    let content: AnyElement = if workspace.hits.is_empty() {
        if untouched && everything_archived && !query.include_archived {
            notice(
                "songs-archived-only",
                "All songs are archived",
                vec!["Include archived to look at them.".to_owned()],
                vec![
                    Button::new(
                        "songs-archived-only-action",
                        "Include archived",
                        environment,
                    )
                    .on_activate(SongsWorkspace::act(cx, |workspace, _, cx| {
                        workspace.set_include_archived(true, cx);
                    }))
                    .into_any_element(),
                ],
                environment,
            )
            .into_any_element()
        } else {
            no_match(library, workspace, environment, cx).into_any_element()
        }
    } else {
        // Titles are copied out so the library borrow ends before the rows
        // (which need the context mutably for the scroll indicator) are built.
        let titles = workspace
            .hits
            .iter()
            .map(|hit| {
                library
                    .song(hit.song)
                    .map(|song| (song.title().to_owned(), song.is_archived()))
                    .unwrap_or_default()
            })
            .collect();
        rows(workspace, titles, environment, window, cx)
    };

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(bar)
        .child(content)
        .into_any_element()
}

/// The empty library, in the same two-pane workspace.
fn empty_library(environment: UiEnvironment) -> Div {
    notice(
        "songs-empty",
        "No songs yet",
        vec!["Songs you create or import appear here, ready to use in any Mass.".to_owned()],
        vec![
            div()
                .debug_selector(|| "songs-create".into())
                .child(
                    Button::new("songs-create-button", "Create song", environment)
                        .icon(icon::PLUS)
                        .tone(Tone::Primary)
                        .availability(Availability::unavailable(CREATE_REASON)),
                )
                .into_any_element(),
            div()
                .debug_selector(|| "songs-import".into())
                .child(
                    Button::new("songs-import-button", "Import .ordo", environment)
                        .availability(Availability::unavailable(IMPORT_REASON)),
                )
                .into_any_element(),
        ],
        environment,
    )
}

/// No Song matches: names the search text and active filters and offers the
/// one correction. Never first-use guidance.
fn no_match(
    library: &SongLibrary,
    workspace: &SongsWorkspace,
    environment: UiEnvironment,
    cx: &Context<SongsWorkspace>,
) -> Div {
    let query = &workspace.query;
    let text = query.text.trim();
    let mut parts = Vec::new();
    if !text.is_empty() {
        parts.push(format!("\u{201c}{text}\u{201d}"));
    }
    if filters::any_active(query) {
        parts.push(format!(
            "these filters ({})",
            filters::describe(library, query)
        ));
    }
    let line = match parts.len() {
        0 => "No song is shown with the current settings.".to_owned(),
        1 => format!("Nothing matches {}.", parts[0]),
        _ => format!("Nothing matches {} with {}.", parts[0], parts[1]),
    };
    notice(
        "songs-no-match",
        "No songs match",
        vec![line],
        vec![
            Button::new(
                "songs-clear-button",
                "Clear search and filters",
                environment,
            )
            .on_activate(SongsWorkspace::act(cx, |workspace, window, cx| {
                workspace.clear_search_and_filters(window, cx);
            }))
            .into_any_element(),
        ],
        environment,
    )
}

/// The ruled column of result rows.
fn rows(
    workspace: &SongsWorkspace,
    titles: Vec<(String, bool)>,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<SongsWorkspace>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let filtered = filters::any_active(&workspace.query);
    let scroll = workspace.list_scroll.clone();
    let rows = workspace
        .hits
        .iter()
        .zip(titles)
        .enumerate()
        .map(|(at, (hit, (title, archived)))| {
            let id = hit.song;
            let mut badges = Vec::new();
            if let Some(count) = count_line(hit, filtered) {
                badges.push(Badge::new(count, environment));
            }
            if archived {
                badges.push(Badge::new("Archived", environment));
            }
            let row = ListRow::new(("songs-row", at), title, environment)
                .badges(badges)
                .selected(workspace.selected == Some(id))
                .on_surface(palette.background)
                .on_activate(SongsWorkspace::act(cx, move |workspace, _, cx| {
                    workspace.select(id, cx);
                }));
            let row = match evidence_line(hit) {
                Some(line) => row.detail(line),
                None => row,
            };
            div()
                .debug_selector(move || format!("songs-row-{at}"))
                .w_full()
                .child(row)
        })
        .collect::<Vec<_>>();
    div()
        .debug_selector(|| "songs-results".into())
        .relative()
        .flex_1()
        .min_h_0()
        .child(
            list::arrows(scroll.bounded(div().id("songs-list")))
                .size_full()
                .flex()
                .flex_col()
                .children(rows),
        )
        .children(scroll.indicator("songs-list", 0.0, window, cx, environment))
        .into_any_element()
}
