//! The Sequence pane: the performance order as rows, each one an
//! occurrence of a section, with its slides when expanded.
//!
//! Every row is one focus stop (`Role::ListItem`) in the
//! `SequenceOccurrences` key context, so Alt+Up, Alt+Down and Delete act on
//! it. The slide tiles of an expanded row are stops of their own outside that
//! context: Delete never removes an occurrence from a slide.

use domain::{
    identity::OccurrenceId,
    pagination::{slide_body, slides},
    song_library::{ArrangementLyrics, SectionDraft, Step, flatten, position_label},
};
use gpui::{
    AnyElement, App, Context, ElementId, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div,
    prelude::*, px, rgb, svg,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        Availability,
        button::{Button, Tone},
        focus_visible,
        list::Badge,
    },
    tokens,
};

use super::{Item, occurrence_name};
use crate::{
    commands::registry::{MoveDown, MoveUp, RemoveOccurrence, SEQUENCE_OCCURRENCES_CONTEXT},
    songs::lyrics::{
        ArrangementEditor,
        navigator::{THUMBNAIL_WIDTH, inspector_button, picture},
        panes::head,
    },
};

/// The pane's heading.
pub const HEAD: &str = "Sequence";

/// What the pane says while it is empty.
pub const EMPTY_TITLE: &str = "No performance sequence yet";

/// Why the pane is empty, and what that means.
pub const EMPTY_BODY: &str = "Add sections in the order they\u{2019}re sung. Until then this Arrangement is incomplete and gives a Mass no slides.";

/// The chip of a section the Sequence uses more than once.
pub const SAME_SECTION: &str = "Same section";

/// The head's meta line: the occurrence and slide counts, or "Incomplete".
pub fn summary(content: &ArrangementLyrics) -> String {
    if content.sequence.is_empty() {
        "Incomplete".to_owned()
    } else {
        format!(
            "{} occurrences \u{b7} {} slides in sung order",
            content.sequence.len(),
            flatten(content).len()
        )
    }
}

/// A row's slide count: "{n} slide(s)".
fn slide_count(count: usize) -> String {
    if count == 1 {
        "1 slide".to_owned()
    } else {
        format!("{count} slides")
    }
}

/// Up, Down, Left and Right on the stop that owns focus.
fn walker(
    weak: &WeakEntity<ArrangementEditor>,
    item: Item,
    focus: gpui::FocusHandle,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    let weak = weak.clone();
    move |event, window, cx| {
        if !focus.is_focused(window) || event.keystroke.modifiers.modified() {
            return;
        }
        let key = event.keystroke.key.clone();
        let handled = weak
            .update(cx, |editor, cx| editor.walk(item, &key, cx))
            .unwrap_or(false);
        if handled {
            cx.stop_propagation();
            window.prevent_default();
        }
    }
}

/// The Sequence pane.
#[allow(clippy::too_many_arguments)]
pub(super) fn pane(
    editor: &mut ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    compact: bool,
    read_only: bool,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ArrangementEditor>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    editor
        .sequence
        .rows
        .retain(|id, _| content.occurrence_index(*id).is_some());
    editor.sequence.kids.retain(|(id, _), _| {
        content.occurrence_index(*id).is_some() && editor.expanded.contains(id)
    });
    let selected = editor.sequence_selection;
    let last = content.sequence.len().saturating_sub(1);
    let mut rows = Vec::with_capacity(content.sequence.len());
    for (at, occurrence) in content.sequence.iter().enumerate() {
        let Some(section) = content
            .sections
            .iter()
            .find(|section| section.id == occurrence.section)
        else {
            continue;
        };
        let id = occurrence.id;
        let focus = editor
            .sequence
            .rows
            .entry(id)
            .or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone();
        let open = editor.expanded.contains(&id);
        let kid_focus: Vec<_> = if open {
            (0..slides(&section.lyrics).len())
                .map(|slide| {
                    editor
                        .sequence
                        .kids
                        .entry((id, slide))
                        .or_insert_with(|| cx.focus_handle().tab_stop(true))
                        .clone()
                })
                .collect()
        } else {
            Vec::new()
        };
        rows.push(occurrence_row(
            editor,
            weak,
            content,
            section,
            at,
            last,
            (focus, kid_focus),
            selected,
            read_only,
            environment,
        ));
    }
    if let Some(item) = editor.sequence.focus_item.take() {
        let handle = match item {
            Item::Row(id) => editor.sequence.rows.get(&id),
            Item::Child(id, slide) => editor.sequence.kids.get(&(id, slide)),
        }
        .cloned();
        if let Some(handle) = handle {
            window.defer(cx, move |window, cx| window.focus(&handle, cx));
        }
    }
    let empty = content.sequence.is_empty();
    if empty && editor.sequence.focus_add_first && !read_only {
        editor.sequence.focus_add_first = false;
        let handle = editor.sequence.add_first.clone();
        window.defer(cx, move |window, cx| handle.focus(window, cx));
    }
    let toggle = (!empty).then(|| {
        let weak = weak.clone();
        let label = if editor.all_expanded(content) {
            "Collapse all"
        } else {
            "Expand all"
        };
        Button::new("sequence-expand-all", label, environment)
            .quiet()
            .on_surface(palette.background)
            .on_activate(move |_, cx| {
                let _ = weak.update(cx, |editor, cx| editor.toggle_all_expanded(cx));
            })
    });
    let body = if empty {
        empty_state(editor, weak, content, read_only, environment)
    } else {
        div()
            .debug_selector(|| "arrangement-sequence".into())
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .children(rows)
            .into_any_element()
    };
    div()
        .debug_selector(|| "sequence-pane".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .size_full()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .child(head(HEAD, environment))
                        .child(
                            div()
                                .debug_selector(|| "sequence-summary".into())
                                .text_size(px(scale.apply(tokens::typography::META.size)))
                                .text_color(rgb(palette.text_muted))
                                .child(summary(content)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .children(toggle)
                        .when(compact, |actions| {
                            actions.child(inspector_button(weak, environment))
                        }),
                ),
        )
        .child(body)
        .into_any_element()
}

/// The empty state, with its focused first action.
fn empty_state(
    editor: &ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    read_only: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let first = content
        .sections
        .first()
        .filter(|_| !read_only)
        .map(|first| {
            let (weak, id) = (weak.clone(), first.id);
            let name = occurrence_name(content, id);
            div().debug_selector(|| "sequence-add-first".into()).child(
                Button::new(
                    "sequence-add-first-button",
                    format!("Add first occurrence \u{b7} {name}"),
                    environment,
                )
                .icon(icon::PLUS)
                .tone(Tone::Primary)
                .on_surface(palette.background)
                .track_focus(&editor.sequence.add_first)
                .on_activate(move |_, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.add_occurrence(id, true, cx));
                }),
            )
        });
    div()
        .debug_selector(|| "sequence-empty".into())
        .flex()
        .flex_col()
        .items_start()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child(EMPTY_TITLE),
        )
        .child(div().text_color(rgb(palette.text_muted)).child(EMPTY_BODY))
        .children(first)
        .into_any_element()
}

/// One occurrence: its row and, when expanded, its slide tiles.
#[allow(clippy::too_many_arguments)]
fn occurrence_row(
    editor: &ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    section: &SectionDraft,
    at: usize,
    last: usize,
    (focus, kid_focus): (gpui::FocusHandle, Vec<gpui::FocusHandle>),
    selected: Option<super::SequenceSelection>,
    read_only: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let id = content.sequence[at].id;
    let name = occurrence_name(content, section.id);
    let pos = position_label(at);
    let count = slides(&section.lyrics).len();
    let open = editor.expanded.contains(&id);
    let current = selected.is_some_and(|selection| selection.occurrence == id);
    let same = content.occurrences_of(section.id) > 1;
    let (up, down, gone, chosen, toggled, repeat) = (
        weak.clone(),
        weak.clone(),
        weak.clone(),
        weak.clone(),
        weak.clone(),
        weak.clone(),
    );
    let removal_focus = focus.clone();
    let glyph = if open {
        icon::CHEVRON_DOWN
    } else {
        icon::CHEVRON_RIGHT
    };
    let expander = div()
        .id(("sequence-expander", at))
        .debug_selector(move || format!("sequence-expander-{at}"))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(scale.apply(tokens::icon::CONTROL_SIZE) + 8.0))
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(if open { "Collapse" } else { "Expand" })
        .aria_expanded(open)
        .on_click(move |_, _, cx| {
            let _ = toggled.update(cx, |editor, cx| editor.toggle_expanded(id, cx));
        })
        .child(
            svg()
                .flex_none()
                .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                .text_color(rgb(palette.text))
                .data(glyph.as_bytes()),
        );
    let label = div()
        .id(("sequence-name", at))
        .debug_selector(move || format!("sequence-name-{at}"))
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            let _ = chosen.update(cx, |editor, cx| editor.click_occurrence(id, cx));
        })
        .child(div().text_color(rgb(palette.text_muted)).child(pos.clone()))
        .child(
            div()
                .min_w_0()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child(name.clone()),
        )
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child(slide_count(count)),
        )
        .when(same, |label| {
            label.child(Badge::new(SAME_SECTION, environment))
        });
    let actions = (current && !read_only).then(|| {
        let icon_button = |button_id: &'static str,
                           label: &'static str,
                           tip: &'static str,
                           glyph: &'static str,
                           availability: Availability| {
            Button::new(button_id, label, environment)
                .icon(glyph)
                .icon_only()
                .quiet()
                .tooltip(tip)
                .on_surface(palette.surface_secondary)
                .availability(availability)
        };
        let applies = |applies: bool| {
            if applies {
                Availability::Available
            } else {
                Availability::Disabled
            }
        };
        let (step_up, step_down, remove) = (weak.clone(), weak.clone(), weak.clone());
        div()
            .debug_selector(|| "sequence-row-buttons".into())
            .flex()
            .flex_none()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .child(
                Button::new("sequence-repeat", "Repeat", environment)
                    .quiet()
                    .on_surface(palette.surface_secondary)
                    .on_activate(move |_, cx| {
                        let _ = repeat.update(cx, |editor, cx| editor.repeat_occurrence(id, cx));
                    }),
            )
            .child(
                icon_button(
                    "sequence-move-up",
                    "Move up",
                    "Move up (Alt+Up)",
                    icon::ARROW_UP,
                    applies(at > 0),
                )
                .on_activate(move |_, cx| {
                    let _ = step_up.update(cx, |editor, cx| {
                        editor.move_occurrence_by(id, Step::Up, cx);
                    });
                }),
            )
            .child(
                icon_button(
                    "sequence-move-down",
                    "Move down",
                    "Move down (Alt+Down)",
                    icon::ARROW_DOWN,
                    applies(at < last),
                )
                .on_activate(move |_, cx| {
                    let _ = step_down.update(cx, |editor, cx| {
                        editor.move_occurrence_by(id, Step::Down, cx);
                    });
                }),
            )
            .child(
                icon_button(
                    "sequence-remove",
                    "Remove from Sequence",
                    "Remove (Delete)",
                    icon::X,
                    Availability::Available,
                )
                .on_activate(move |_, cx| {
                    let _ = remove.update(cx, |editor, cx| editor.remove_occurrence(id, cx));
                }),
            )
    });
    let row = div()
        .id(("sequence-row", at))
        .debug_selector(move || format!("arrangement-sequence-{at}"))
        .track_focus(&focus)
        .key_context(SEQUENCE_OCCURRENCES_CONTEXT)
        .role(Role::ListItem)
        .aria_label(format!("{pos} {name}"))
        .aria_selected(current)
        .aria_expanded(open)
        .on_action(move |_: &MoveUp, _, cx| {
            let _ = up.update(cx, |editor, cx| editor.move_occurrence_by(id, Step::Up, cx));
        })
        .on_action(move |_: &MoveDown, _, cx| {
            let _ = down.update(cx, |editor, cx| {
                editor.move_occurrence_by(id, Step::Down, cx);
            });
        })
        .on_action(move |_: &RemoveOccurrence, window, cx| {
            // A button inside the row shares the key context; Delete is the
            // row's alone.
            if !removal_focus.is_focused(window) {
                cx.propagate();
                return;
            }
            let _ = gone.update(cx, |editor, cx| editor.remove_occurrence(id, cx));
        })
        .on_key_down(walker(weak, Item::Row(id), focus.clone()))
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .p(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .when(current, |row| row.bg(rgb(palette.surface_secondary)))
        .child(expander)
        .child(label)
        .children(actions);
    let row = focus_visible(row, environment, palette.background);
    div()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .child(row)
        .when(open && count > 0, |column| {
            column.child(tiles(
                editor,
                weak,
                section,
                (id, at, &pos, &name),
                kid_focus,
                selected,
                environment,
            ))
        })
        .into_any_element()
}

/// The slide tiles of an expanded occurrence, in order.
fn tiles(
    editor: &ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    section: &SectionDraft,
    (id, at, pos, name): (OccurrenceId, usize, &str, &str),
    focus: Vec<gpui::FocusHandle>,
    selected: Option<super::SequenceSelection>,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let width = scale.apply(THUMBNAIL_WIDTH);
    let sources = slides(&section.lyrics);
    let tiles = sources
        .iter()
        .zip(focus)
        .enumerate()
        .map(|(slide, (source, handle))| {
            let body = slide_body(&section.lyrics, source);
            let key = (section.label.clone(), body.clone());
            let current = selected.is_some_and(|s| s.occurrence == id && s.slide == slide);
            let choose = weak.clone();
            let caption = format!(
                "{} {}",
                slide + 1,
                body.lines().next().unwrap_or_default().trim()
            );
            let tile = div()
                .id(ElementId::NamedInteger(
                    SharedString::from(format!("sequence-child-{at}")),
                    slide as u64,
                ))
                .track_focus(&handle)
                .debug_selector(move || format!("sequence-child-{at}-{slide}"))
                .flex()
                .flex_col()
                .flex_none()
                .w(px(width))
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .cursor_pointer()
                .role(Role::ListItem)
                .aria_label(format!("{pos} {name}, slide {}", slide + 1))
                .aria_selected(current)
                .on_key_down(walker(weak, Item::Child(id, slide), handle.clone()))
                .on_click(move |_, _, cx| {
                    let _ =
                        choose.update(cx, |editor, cx| editor.go_to(Item::Child(id, slide), cx));
                })
                .child(
                    div()
                        .flex_none()
                        .w(px(width))
                        .h(px(width * 9.0 / 16.0))
                        .rounded(px(tokens::radius::SLIDE_VIEWPORT))
                        .border(px(tokens::border::SEPARATOR))
                        .border_color(rgb(if current {
                            tokens::color::PRIMARY
                        } else {
                            palette.border
                        }))
                        .child(picture(
                            editor,
                            &key,
                            width - 2.0 * tokens::border::SEPARATOR,
                            environment,
                        )),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(caption),
                );
            focus_visible(tile, environment, palette.background)
        });
    div()
        .debug_selector(move || format!("sequence-children-{at}"))
        .flex()
        .flex_wrap()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .pl(px(scale.apply(tokens::spacing::SECTION)))
        .children(tiles)
        .into_any_element()
}
