//! The sections pane: the Lyrics list with its Add section action, and the
//! operator's wording for what the domain refuses.

use domain::{
    identity::SectionId,
    pagination::slides,
    song_library::{ArrangementLyrics, SongLibraryError, Step, match_key},
};
use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Role, Styled, WeakEntity, Window,
    canvas, div, prelude::*, px, rgb,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{Availability, button::Button},
    tokens,
};

use crate::commands::registry::{LYRICS_SECTIONS_CONTEXT, MoveDown, MoveUp};

use super::{
    add_section::{ADD_SECTION, open_add_section},
    editor::ArrangementEditor,
    panes::section_name,
    review::{row_status, status_line},
    view::Anchor,
};

/// What the operator reads when a save was refused for a reason the page has
/// no better words for. A raw domain or storage message is never shown.
pub const GENERIC_REFUSAL: &str = "Couldn't save.";

/// The name of the section that already holds `typed`, as it is spelled
/// there, so the message names what is on the page and not what was typed.
pub fn taken_name(content: &ArrangementLyrics, typed: &str) -> String {
    let key = match_key(typed);
    content
        .sections
        .iter()
        .filter_map(|section| section.label.as_deref())
        .find(|label| match_key(label) == key)
        .map_or_else(|| typed.trim().to_owned(), str::to_owned)
}

/// The operator's wording for a refusal of a section name.
pub fn refusal_copy(error: &SongLibraryError, content: &ArrangementLyrics, typed: &str) -> String {
    match error {
        SongLibraryError::BlankField(_) => "A section needs a name.".to_owned(),
        SongLibraryError::DuplicateField(_) => {
            format!(
                "Another section is already called {}.",
                taken_name(content, typed)
            )
        }
        _ => GENERIC_REFUSAL.to_owned(),
    }
}

/// The meta line of a section row: "{n} slide(s) · used {n}×" or
/// "not in Sequence".
pub fn row_meta(content: &ArrangementLyrics, id: SectionId, lyrics: &str) -> String {
    let count = slides(lyrics).len();
    let slides = if count == 1 {
        "1 slide".to_owned()
    } else {
        format!("{count} slides")
    };
    match content.occurrences_of(id) {
        0 => format!("{slides} \u{b7} not in Sequence"),
        used => format!("{slides} \u{b7} used {used}\u{d7}"),
    }
}

impl ArrangementEditor {
    /// The section the main pane edits, once the panes have been built.
    pub fn selected_section(&self) -> Option<SectionId> {
        self.lyrics.selected
    }

    /// Selects a section by identity.
    pub fn select_section(&mut self, id: SectionId, cx: &mut gpui::Context<Self>) {
        if self.lyrics.selected != Some(id) {
            self.lyrics.selected = Some(id);
            cx.notify();
        }
    }
}

/// A control whose painted bounds are remembered, so a popover can anchor to
/// it.
pub(super) fn anchored(control: impl IntoElement, anchor: &Anchor, selector: &'static str) -> Div {
    let cell = anchor.0.clone();
    div()
        .relative()
        .flex_none()
        .debug_selector(|| selector.into())
        .child(control)
        .child(
            canvas(move |bounds, _, _| cell.set(bounds), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
}

/// The sections pane.
pub(super) fn sections_pane(
    editor: &mut ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    read_only: bool,
    environment: UiEnvironment,
    cx: &mut App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let selected = editor.lyrics.selected;
    let add = (!read_only).then(|| {
        let (editor_weak, anchor) = (weak.clone(), editor.lyrics.head_anchor.clone());
        anchored(
            Button::new("arrangement-add-section-head", ADD_SECTION, environment)
                .on_surface(palette.background)
                .on_activate({
                    let anchor = anchor.clone();
                    move |window: &mut Window, cx: &mut App| {
                        open_add_section(&editor_weak, &anchor, window, cx);
                    }
                }),
            &anchor,
            "arrangement-add-section-head",
        )
    });
    let rows = content.sections.iter().enumerate().map(|(at, section)| {
        let id = section.id;
        let focus = editor
            .lyrics
            .rows
            .entry(id)
            .or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone();
        let choose = weak.clone();
        let (up, down) = (weak.clone(), weak.clone());
        let name = section_name(&content.sections, at);
        let last = content.sections.len() - 1;
        let current = selected == Some(id);
        div()
            .id(("arrangement-section-row", at))
            .debug_selector(move || format!("arrangement-section-{at}"))
            .track_focus(&focus)
            .key_context(LYRICS_SECTIONS_CONTEXT)
            .role(Role::ListItem)
            .aria_label(name.clone())
            .aria_selected(current)
            .on_action(move |_: &MoveUp, _, cx| {
                let _ = up.update(cx, |editor, cx| {
                    editor.select_section(id, cx);
                    let result = editor.move_section(id, Step::Up, cx);
                    editor.notify_refusal(result, cx);
                });
            })
            .on_action(move |_: &MoveDown, _, cx| {
                let _ = down.update(cx, |editor, cx| {
                    editor.select_section(id, cx);
                    let result = editor.move_section(id, Step::Down, cx);
                    editor.notify_refusal(result, cx);
                });
            })
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .when(current, |row| row.bg(rgb(palette.surface_secondary)))
            .on_click(move |_, _, cx| {
                let _ = choose.update(cx, |editor, cx| editor.select_section(id, cx));
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                    .child(
                        div()
                            .min_w_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.text))
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(scale.apply(tokens::typography::META.size)))
                            .text_color(rgb(palette.text_muted))
                            .child(row_meta(content, id, &section.lyrics)),
                    )
                    .children(
                        row_status(&section.lyrics, section.pagination)
                            .map(|status| status_line(status, at, environment)),
                    ),
            )
            .when(current && !read_only, |row| {
                row.child(move_buttons(weak, at, last, environment))
            })
    });
    div()
        .debug_selector(|| "arrangement-sections".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .size_full()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                        .text_color(rgb(palette.text))
                        .child("Sections"),
                )
                .children(add),
        )
        .children(rows)
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child("Lyrics order is for writing only; the Sequence sets what\u{2019}s sung."),
        )
        .into_any_element()
}

/// The Move up and Move down icon buttons of the selected row.
fn move_buttons(
    weak: &WeakEntity<ArrangementEditor>,
    at: usize,
    last: usize,
    environment: UiEnvironment,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let mover = |id: &'static str,
                 label: &'static str,
                 tip: &'static str,
                 glyph: &'static str,
                 step: Step,
                 applies: bool| {
        let weak = weak.clone();
        Button::new(id, label, environment)
            .icon(glyph)
            .icon_only()
            .quiet()
            .tooltip(tip)
            .on_surface(palette.surface_secondary)
            .availability(if applies {
                Availability::Available
            } else {
                Availability::Disabled
            })
            .on_activate(move |_, cx| {
                let _ = weak.update(cx, |editor, cx| {
                    if let Some(id) = editor.lyrics.selected {
                        let result = editor.move_section(id, step, cx);
                        editor.notify_refusal(result, cx);
                    }
                });
            })
    };
    div()
        .debug_selector(|| "arrangement-section-buttons".into())
        .flex()
        .flex_none()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .child(mover(
            "arrangement-move-up",
            "Move up",
            "Move up (Alt+Up)",
            icon::ARROW_UP,
            Step::Up,
            at > 0,
        ))
        .child(mover(
            "arrangement-move-down",
            "Move down",
            "Move down (Alt+Down)",
            icon::ARROW_DOWN,
            Step::Down,
            at < last,
        ))
}
