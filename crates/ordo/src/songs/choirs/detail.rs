//! The Choirs detail pane: the selected Choir's name, details and derived
//! repertoire, or the one state that stands in for it.
//!
//! An archived Choir is inspect-only: its inputs are read-only and the header
//! offers Restore. Repertoire is derived, never stored: one row per active
//! Arrangement that names the Choir. In a narrow pane each Song becomes a card
//! holding its Arrangements.

use domain::{
    identity::{ArrangementId, ChoirId, SongId},
    song_library::{Arrangement, Choir, SongLibrary},
};
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, KeyDownEvent, ParentElement, Styled, Window,
    div, prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{
        button::{Button, Tone},
        field::Field,
        list::Badge,
        record::Chip,
    },
    tokens,
};

use super::{ChoirsWorkspace, list};
use crate::{
    edit_session::SaveState,
    songs::{library::OpenArrangement, store::LibraryState},
};

/// Below this width of the repertoire block, Songs become cards (mockup G's
/// container query).
const NARROW_BELOW: f32 = 720.0;

/// One row of a Choir's repertoire.
struct Entry {
    arrangement: ArrangementId,
    song: SongId,
    song_title: String,
    title: String,
    languages: String,
    shared_with: String,
}

fn entries(library: &SongLibrary, choir: &Choir) -> Vec<Entry> {
    let arrangements = library.choir_arrangements(choir.id()).unwrap_or_default();
    let mut found: Vec<Entry> = arrangements
        .into_iter()
        .filter_map(|arrangement| {
            let song = library.song(arrangement.song()).ok()?;
            (!arrangement.is_archived() && !song.is_archived())
                .then(|| entry(library, song.title(), arrangement, choir.id()))
        })
        .collect();
    // Songs in title order, Arrangements keeping creation order inside.
    found.sort_by_key(|entry| (entry.song_title.to_lowercase(), entry.song));
    found
}

fn entry(
    library: &SongLibrary,
    song_title: &str,
    arrangement: &Arrangement,
    own: ChoirId,
) -> Entry {
    let mut others: Vec<&str> = arrangement
        .choirs()
        .iter()
        .filter(|id| **id != own)
        .filter_map(|id| library.choir(*id).ok().map(Choir::name))
        .collect();
    others.sort_by_key(|name| name.to_lowercase());
    Entry {
        arrangement: arrangement.id(),
        song: arrangement.song(),
        song_title: song_title.to_owned(),
        title: arrangement.title().to_owned(),
        languages: if arrangement.languages().is_empty() {
            "No language".to_owned()
        } else {
            arrangement.languages().join(", ")
        },
        shared_with: if others.is_empty() {
            "Not shared".to_owned()
        } else {
            others.join(", ")
        },
    }
}

/// The pane the shell hosts in its viewer region.
pub(super) fn render(
    workspace: &mut ChoirsWorkspace,
    environment: UiEnvironment,
    open: OpenArrangement,
    window: &mut Window,
    cx: &mut Context<ChoirsWorkspace>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let pane = |content: AnyElement| {
        div()
            .debug_selector(|| "choirs-detail-pane".into())
            .size_full()
            .bg(rgb(palette.background))
            .child(content)
            .into_any_element()
    };
    let store = workspace.store.clone();
    let state = store.read(cx).state().clone();
    match state {
        LibraryState::Ready => {}
        LibraryState::Loading => {
            return pane(
                list::notice(
                    "choirs-detail-waiting",
                    "No choir selected",
                    vec!["The selected choir appears here once the library is open.".to_owned()],
                    environment,
                )
                .into_any_element(),
            );
        }
        LibraryState::Unavailable(message) => {
            return pane(
                list::notice(
                    "choirs-detail-unavailable",
                    "No choir to show",
                    vec![message.to_string()],
                    environment,
                )
                .into_any_element(),
            );
        }
    }
    let library = store.read(cx).library();
    if library.choirs().next().is_none() {
        return pane(
            list::notice(
                "choirs-detail-intro",
                "Choirs are optional",
                vec![
                    "Songs and Masses work without them. An Arrangement or Mass can stay Not assigned."
                        .to_owned(),
                    "Add a choir by name. A label, preferred languages and notes can wait."
                        .to_owned(),
                    "Name it on its Arrangements. Its repertoire then fills in here.".to_owned(),
                ],
                environment,
            )
            .into_any_element(),
        );
    }
    let selected = workspace
        .selected
        .and_then(|id| library.choir(id).ok().map(|choir| (id, choir)));
    let Some((_, choir)) = selected else {
        return pane(
            list::notice(
                "choirs-detail-none",
                "No choir selected",
                vec!["Choose a choir in the list to see its details and repertoire.".to_owned()],
                environment,
            )
            .into_any_element(),
        );
    };
    let archived = choir.is_archived();
    let repertoire = entries(library, choir);
    let current_name = choir.name().to_owned();
    let songs = {
        let mut songs: Vec<SongId> = repertoire.iter().map(|entry| entry.song).collect();
        songs.dedup();
        songs.len()
    };
    let arrangement_count = repertoire.len();
    let scale = environment.preferences().scale;

    // The repertoire block's width: the viewport less the rail and the list
    // pane, the pane's own padding on both sides.
    let pane_width = window.viewport_size().width.to_f64() as f32
        - scale.apply(crate::shell::rail::EXPANDED_WIDTH)
        - scale.apply(crate::shell::LIST_WIDTH)
        - 2.0 * scale.apply(tokens::spacing::GROUP);
    let narrow = pane_width < scale.apply(NARROW_BELOW);

    let label = match workspace.save_state() {
        _ if archived => None,
        state => Some(state.label()),
    };
    let save_label = label.map(|text| {
        div()
            .debug_selector(|| "choirs-save-state".into())
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(if workspace.save_state() == SaveState::CouldntSave {
                palette.text
            } else {
                palette.text_muted
            }))
            .child(text)
    });

    let name_control: AnyElement = if workspace.renaming && !archived {
        div()
            .debug_selector(|| "choir-name-field".into())
            .flex_1()
            .min_w_0()
            .child(
                Field::new(
                    "choir-name-input",
                    "Choir name",
                    &workspace.form.name,
                    environment,
                )
                .label_hidden()
                .placeholder("Choir name"),
            )
            .into_any_element()
    } else {
        let weak = cx.weak_entity();
        div()
            .id("choir-name-title")
            .debug_selector(|| "choir-name".into())
            .min_w_0()
            .truncate()
            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(palette.text))
            .when(!archived, |title| {
                title.cursor_pointer().on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |workspace, cx| {
                        workspace.renaming = true;
                        workspace.form.name.focus(window, cx);
                        cx.notify();
                    });
                })
            })
            .child(current_name)
            .into_any_element()
    };

    let archive_button = if archived {
        Button::new("choir-restore", "Restore", environment)
            .tone(Tone::Primary)
            .on_surface(palette.background)
            .on_activate(ChoirsWorkspace::act(cx, |workspace, _, cx| {
                workspace.restore_selected(cx);
            }))
    } else {
        Button::new("choir-archive", "Archive", environment)
            .on_surface(palette.background)
            .on_activate(ChoirsWorkspace::act(cx, |workspace, _, cx| {
                workspace.archive_selected(cx);
            }))
    };

    let header = div()
        .debug_selector(|| "choirs-detail-header".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .on_key_down(cx.listener(|workspace, event: &KeyDownEvent, window, cx| {
            if !workspace.renaming || event.keystroke.modifiers.modified() {
                return;
            }
            match event.keystroke.key.as_str() {
                "enter" => {
                    workspace.renaming = false;
                    workspace.flush(cx);
                    cx.stop_propagation();
                }
                "escape" => {
                    let stored = workspace
                        .selected
                        .and_then(|id| workspace.store.read(cx).library().choir(id).ok())
                        .map(|choir| choir.name().to_owned())
                        .unwrap_or_default();
                    workspace.form.name.set_value(stored, window, cx);
                    workspace.renaming = false;
                    cx.stop_propagation();
                }
                _ => return,
            }
            cx.notify();
        }))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child("Choir"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(name_control)
                .children(archived.then(|| Badge::new("Archived", environment)))
                .child(div().flex_1())
                .children(save_label)
                .child(
                    div()
                        .debug_selector(|| "choir-archive-action".into())
                        .child(archive_button),
                ),
        )
        .children(workspace.message.clone().map(|message| {
            div()
                .debug_selector(|| "choirs-message".into())
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text))
                .child(format!("Couldn't save. {message}"))
        }));

    let archived_notice = archived.then(|| {
        div()
            .debug_selector(|| "choirs-archived-notice".into())
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
            .child(
                "Archived. Choir selectors don\u{2019}t suggest it. Arrangements and Mass plans that name it keep it.",
            )
    });

    let form = form(workspace, archived, environment, cx);
    let repertoire = repertoire_block(
        &repertoire,
        songs,
        arrangement_count,
        narrow,
        archived,
        environment,
        open,
        cx,
    );

    let scroll = workspace.detail_scroll.clone();
    let body = div()
        .debug_selector(|| "choirs-detail".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .w_full()
        .p(px(scale.apply(tokens::spacing::GROUP)))
        .child(header)
        .children(archived_notice)
        .child(form)
        .child(repertoire);
    pane(
        div()
            .relative()
            .size_full()
            .child(
                scroll
                    .bounded(div().id("choirs-detail-scroll"))
                    .size_full()
                    .child(body),
            )
            .children(scroll.indicator("choirs-detail-scroll", 0.0, window, cx, environment))
            .into_any_element(),
    )
}

/// A form row: the field, then one line of help.
fn help(text: &'static str, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .text_color(rgb(palette.text_muted))
        .child(text)
}

/// Short label, preferred languages and private notes.
fn form(
    workspace: &ChoirsWorkspace,
    archived: bool,
    environment: UiEnvironment,
    cx: &mut Context<ChoirsWorkspace>,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let read_only = |field: Field| {
        if archived {
            field.read_only("Restore to edit")
        } else {
            field
        }
    };
    let tags = workspace
        .form
        .languages
        .iter()
        .enumerate()
        .map(|(at, language)| {
            let mut remove = Button::new(
                ("choir-language-remove", at),
                format!("Remove {language}"),
                environment,
            )
            .icon(ui::assets::icon::X)
            .icon_only()
            .quiet()
            .on_surface(palette.background);
            if !archived {
                remove = remove.on_activate(ChoirsWorkspace::act(cx, move |workspace, _, cx| {
                    workspace.remove_language(at, cx);
                }));
            } else {
                remove = remove
                    .availability(ui::primitives::Availability::unavailable("Restore to edit"));
            }
            div()
                .flex()
                .items_center()
                .child(Chip::new(language.clone(), environment))
                .child(remove)
        })
        .collect::<Vec<_>>();
    div()
        .debug_selector(|| "choirs-form".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .child(
            div()
                .debug_selector(|| "choir-short-label".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(read_only(
                    Field::new(
                        "choir-short-label-input",
                        "Short label",
                        &workspace.form.short_label,
                        environment,
                    )
                    .placeholder("Optional, such as KSR"),
                ))
                .child(help(
                    "Used where space is tight, such as the Masses list.",
                    environment,
                )),
        )
        .child(
            div()
                .debug_selector(|| "choir-languages".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(
                    div()
                        .debug_selector(|| "choir-language-tags".into())
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                        .children(tags),
                )
                .child(
                    div()
                        .on_key_down(cx.listener(|workspace, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "enter"
                                && !event.keystroke.modifiers.modified()
                            {
                                workspace.add_language(window, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .child(read_only(
                            Field::new(
                                "choir-language-input",
                                "Preferred languages",
                                &workspace.form.language,
                                environment,
                            )
                            .placeholder("Type a language, then press Enter"),
                        )),
                )
                .child(help(
                    "The languages it usually sings in. Its Arrangements in those languages are suggested first.",
                    environment,
                )),
        )
        .child(
            div()
                .debug_selector(|| "choir-notes".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(read_only(
                    Field::new(
                        "choir-notes-input",
                        "Private notes",
                        &workspace.form.notes,
                        environment,
                    )
                    .placeholder("Optional"),
                ))
                .child(help(
                    "For the people preparing Masses. Never projected.",
                    environment,
                )),
        )
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The derived repertoire: a table of rows, or a card per Song when narrow.
#[allow(clippy::too_many_arguments)]
fn repertoire_block(
    entries: &[Entry],
    songs: usize,
    arrangements: usize,
    narrow: bool,
    archived: bool,
    environment: UiEnvironment,
    open: OpenArrangement,
    cx: &mut Context<ChoirsWorkspace>,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let open_button = |at: usize, entry: &Entry| {
        let open = open.clone();
        let id = entry.arrangement;
        let weak = cx.weak_entity();
        div()
            .flex_none()
            .debug_selector(move || format!("arrangement-open-{at}"))
            .child(
                Button::new(("arrangement-open-button", at), "Open", environment)
                    .name(format!("Open {}", entry.title))
                    .on_surface(palette.background)
                    .on_activate(move |window, cx| {
                        let _ = weak.update(cx, |workspace, _| workspace.remember_scroll());
                        open(id, window, cx);
                    }),
            )
    };
    let muted = |text: String| {
        div()
            .min_w_0()
            .truncate()
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.text_muted))
            .child(text)
    };
    let rule = |row: Div| {
        row.w_full()
            .py(px(scale.apply(tokens::spacing::RELATED)))
            .border_b(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
    };

    let body: Div = if entries.is_empty() {
        div()
            .debug_selector(|| "choir-repertoire-empty".into())
            .child(list::notice(
                "choir-repertoire-empty-text",
                if archived {
                    "No active Arrangements name this choir"
                } else {
                    "No Arrangements name this choir yet"
                },
                vec!["Add the choir in an Arrangement\u{2019}s details, under Choirs.".to_owned()],
                environment,
            ))
    } else if narrow {
        type Card<'a> = (SongId, &'a str, Vec<(usize, &'a Entry)>);
        let mut cards: Vec<Card<'_>> = Vec::new();
        for (at, entry) in entries.iter().enumerate() {
            match cards.last_mut() {
                Some((song, _, rows)) if *song == entry.song => rows.push((at, entry)),
                _ => cards.push((entry.song, &entry.song_title, vec![(at, entry)])),
            }
        }
        div()
            .debug_selector(|| "choir-repertoire-cards".into())
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .children(
                cards
                    .into_iter()
                    .enumerate()
                    .map(|(card, (_, title, rows))| {
                        div()
                            .debug_selector(move || format!("choir-repertoire-card-{card}"))
                            .flex()
                            .flex_col()
                            .w_full()
                            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                            .rounded(px(scale.apply(tokens::radius::CONTROL)))
                            .border(px(tokens::border::SEPARATOR))
                            .border_color(rgb(palette.border))
                            .bg(rgb(palette.surface))
                            .child(
                                div()
                                    .pt(px(scale.apply(tokens::spacing::RELATED)))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(palette.text))
                                    .child(title.to_owned()),
                            )
                            .children(rows.into_iter().map(|(at, entry)| {
                                rule(
                                    div()
                                        .debug_selector(move || {
                                            format!("choir-repertoire-row-{at}")
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(scale.apply(tokens::spacing::RELATED))),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .text_color(rgb(palette.text))
                                                .child(entry.title.clone()),
                                        )
                                        .child(muted(entry.languages.clone()))
                                        .child(muted(format!("Shared with {}", entry.shared_with))),
                                )
                                .child(open_button(at, entry))
                            }))
                    }),
            )
    } else {
        div()
            .debug_selector(|| "choir-repertoire-table".into())
            .flex()
            .flex_col()
            .w_full()
            .children(entries.iter().enumerate().map(|(at, entry)| {
                rule(
                    div()
                        .debug_selector(move || format!("choir-repertoire-row-{at}"))
                        .flex()
                        .items_center()
                        .gap(px(scale.apply(tokens::spacing::GROUP))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(entry.song_title.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(rgb(palette.text))
                                .child(entry.title.clone()),
                        )
                        .child(muted(entry.languages.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(muted(format!("Shared with {}", entry.shared_with))),
                )
                .child(open_button(at, entry))
            }))
    };

    div()
        .debug_selector(|| "choir-repertoire".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .w_full()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child("Repertoire"),
                )
                .child(
                    div()
                        .debug_selector(|| "choir-repertoire-count".into())
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(format!(
                            "{} \u{b7} {}",
                            plural(songs, "song", "songs"),
                            plural(arrangements, "Arrangement", "Arrangements")
                        )),
                ),
        )
        .child(body)
        .child(muted(
            "Members, rehearsals and contacts aren\u{2019}t kept in Ordo.".to_owned(),
        ))
}
