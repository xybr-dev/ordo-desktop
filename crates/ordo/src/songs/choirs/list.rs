//! The Choirs list pane: header with Add choir, the archived toggle, the
//! inline add block, and one ruled column of Choirs, or the one state that
//! replaces it (loading, unavailable, no choirs yet).

use domain::{
    identity::ChoirId,
    song_library::{Choir, SongLibrary, match_key},
};
use gpui::{
    AnyElement, Context, Div, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        button::{Button, Tone},
        list::{self, Badge, ListRow},
    },
    tokens,
};

use super::ChoirsWorkspace;
use crate::songs::store::LibraryState;

/// A short state block: a title and lines of explanation.
pub(super) fn notice(
    selector: &'static str,
    title: impl Into<SharedString>,
    lines: Vec<String>,
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
}

/// How many songs a Choir's active Arrangements cover, and how many
/// Arrangements.
pub(super) fn repertoire_counts(library: &SongLibrary, id: ChoirId) -> (usize, usize) {
    let arrangements = library.choir_arrangements(id).unwrap_or_default();
    let active: Vec<_> = arrangements
        .into_iter()
        .filter(|arrangement| {
            !arrangement.is_archived()
                && library
                    .song(arrangement.song())
                    .is_ok_and(|song| !song.is_archived())
        })
        .collect();
    let mut songs: Vec<_> = active
        .iter()
        .map(|arrangement| arrangement.song())
        .collect();
    songs.sort();
    songs.dedup();
    (songs.len(), active.len())
}

/// The Choirs to list, A to Z by name.
fn listed(library: &SongLibrary, show_archived: bool) -> Vec<&Choir> {
    let mut choirs: Vec<&Choir> = library
        .choirs()
        .filter(|choir| show_archived || !choir.is_archived())
        .collect();
    choirs.sort_by_key(|choir| (match_key(choir.name()), choir.id()));
    choirs
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The pane the shell hosts in its list region.
pub(super) fn render(
    workspace: &mut ChoirsWorkspace,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ChoirsWorkspace>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let store = workspace.store.clone();
    let state = store.read(cx).state().clone();
    let ready = state == LibraryState::Ready;

    let mut add = Button::new("choirs-add-button", "Add choir", environment)
        .icon(icon::PLUS)
        .tone(Tone::Primary)
        .on_surface(palette.surface)
        .on_activate(ChoirsWorkspace::act(cx, |workspace, window, cx| {
            workspace.begin_add(window, cx);
        }));
    if !ready {
        add = add.availability(ui::primitives::Availability::unavailable(
            "The parish library is not open",
        ));
    }
    let header = div()
        .debug_selector(|| "choirs-header".into())
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
                .child("Choirs"),
        )
        .child(div().debug_selector(|| "choirs-add".into()).child(add));

    let body: AnyElement = match state {
        LibraryState::Loading => notice(
            "choirs-loading",
            "Opening the parish library",
            vec!["Choirs appear here as soon as the library is read.".to_owned()],
            environment,
        )
        .into_any_element(),
        LibraryState::Unavailable(message) => notice(
            "choirs-unavailable",
            "The parish library couldn't be opened.",
            vec![message.to_string()],
            environment,
        )
        .into_any_element(),
        LibraryState::Ready => ready_body(workspace, environment, window, cx),
    };

    div()
        .debug_selector(|| "choirs-list-pane".into())
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(body)
        .into_any_element()
}

/// The list pane once the library is readable.
fn ready_body(
    workspace: &mut ChoirsWorkspace,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ChoirsWorkspace>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let store = workspace.store.clone();
    let library = store.read(cx).library();
    let any_choirs = library.choirs().next().is_some();
    let show_archived = workspace.show_archived;
    let rows: Vec<(ChoirId, String, String, usize, bool)> = listed(library, show_archived)
        .into_iter()
        .map(|choir| {
            let mut facts: Vec<String> = Vec::new();
            if let Some(label) = choir.short_label() {
                facts.push(label.to_owned());
            }
            if !choir.languages().is_empty() {
                facts.push(choir.languages().join(", "));
            }
            let meta = if facts.is_empty() {
                "No details yet".to_owned()
            } else {
                facts.join(" \u{b7} ")
            };
            let (songs, _) = repertoire_counts(library, choir.id());
            (
                choir.id(),
                choir.name().to_owned(),
                meta,
                songs,
                choir.is_archived(),
            )
        })
        .collect();
    let shown = rows.len();
    let quick = workspace
        .quick
        .clone()
        .map(|quick| quick.update(cx, |quick, cx| quick.render(environment, cx)));

    let toggle = div()
        .debug_selector(|| "choirs-toolbar".into())
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .debug_selector(|| "choirs-count".into())
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child(format!("{shown} shown")),
        )
        .child(
            Button::new(
                "choirs-show-archived",
                if show_archived {
                    "Hide archived"
                } else {
                    "Show archived"
                },
                environment,
            )
            .quiet()
            .on_surface(palette.background)
            .on_activate(ChoirsWorkspace::act(cx, |workspace, _, cx| {
                let show = !workspace.show_archived;
                workspace.set_show_archived(show, cx);
            })),
        );

    let content: AnyElement = if rows.is_empty() {
        if any_choirs {
            notice(
                "choirs-none-shown",
                "All choirs are archived",
                vec!["Show archived to look at them.".to_owned()],
                environment,
            )
            .into_any_element()
        } else {
            notice(
                "choirs-empty",
                "No choirs yet",
                vec![
                    "Choirs help you find the Arrangements each one sings and fill in Mass plans. They never hold their own copy of the lyrics."
                        .to_owned(),
                ],
                environment,
            )
            .into_any_element()
        }
    } else {
        let selected = workspace.selected;
        let items = rows
            .into_iter()
            .enumerate()
            .map(|(at, (id, name, meta, songs, archived))| {
                let mut badges = vec![Badge::new(plural(songs, "song", "songs"), environment)];
                if archived {
                    badges.push(Badge::new("Archived", environment));
                }
                div()
                    .debug_selector(move || format!("choirs-row-{at}"))
                    .w_full()
                    .child(
                        ListRow::new(("choirs-row", at), name, environment)
                            .detail(meta)
                            .badges(badges)
                            .selected(selected == Some(id))
                            .on_surface(palette.background)
                            .on_activate(ChoirsWorkspace::act(cx, move |workspace, _, cx| {
                                workspace.select(id, cx);
                            })),
                    )
            })
            .collect::<Vec<_>>();
        let scroll = workspace.list_scroll.clone();
        div()
            .debug_selector(|| "choirs-results".into())
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                list::arrows(scroll.bounded(div().id("choirs-list")))
                    .size_full()
                    .flex()
                    .flex_col()
                    .children(items),
            )
            .children(scroll.indicator("choirs-list", 0.0, window, cx, environment))
            .into_any_element()
    };

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .children(quick)
        .when(any_choirs, |column| column.child(toggle))
        .child(content)
        .into_any_element()
}
