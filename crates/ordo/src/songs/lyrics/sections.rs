//! The sections pane: the Lyrics list with its Add section action, and the
//! operator's wording for what the domain refuses.

use domain::{
    identity::SectionId,
    pagination::slides,
    song_library::{ArrangementLyrics, SongLibraryError},
};
use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Styled, WeakEntity, Window,
    canvas, div, prelude::*, px, rgb,
};
use ui::{environment::UiEnvironment, primitives::button::Button, tokens};

use crate::commands::registry::{LYRICS_SECTIONS_CONTEXT, MoveDown, MoveUp};

use super::{
    add_section::{ADD_SECTION, open_add_section},
    editor::ArrangementEditor,
    panes::section_name,
    section_actions::section_buttons,
    view::Anchor,
};

/// What the operator reads when a save was refused for a reason the page has
/// no better words for. A raw domain or storage message is never shown.
pub const GENERIC_REFUSAL: &str = "Couldn't save.";

/// The operator's wording for a refusal of a section name.
pub fn refusal_copy(error: &SongLibraryError, name: &str) -> String {
    match error {
        SongLibraryError::BlankField(_) => "A section needs a name.".to_owned(),
        SongLibraryError::DuplicateField(_) => {
            format!("Another section is already called {name}.")
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
        div()
            .id(("arrangement-section-row", at))
            .debug_selector(move || format!("arrangement-section-{at}"))
            .track_focus(&focus)
            .key_context(LYRICS_SECTIONS_CONTEXT)
            .on_action(move |_: &MoveUp, _, cx| {
                let _ = up.update(cx, |editor, cx| {
                    editor.select_section(id, cx);
                    let _ = editor.move_section(id, domain::song_library::Step::Up, cx);
                });
            })
            .on_action(move |_: &MoveDown, _, cx| {
                let _ = down.update(cx, |editor, cx| {
                    editor.select_section(id, cx);
                    let _ = editor.move_section(id, domain::song_library::Step::Down, cx);
                });
            })
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .when(selected == Some(id), |row| {
                row.bg(rgb(palette.surface_secondary))
            })
            .on_click(move |_, _, cx| {
                let _ = choose.update(cx, |editor, cx| editor.select_section(id, cx));
            })
            .child(
                div()
                    .min_w_0()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.text))
                    .child(section_name(&content.sections, at)),
            )
            .child(
                div()
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(row_meta(content, id, &section.lyrics)),
            )
            .when(section.lyrics.trim().is_empty(), |row| {
                row.child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("Empty"),
                )
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
        .child(section_buttons(
            weak,
            content,
            selected,
            read_only,
            environment,
        ))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child("Lyrics order is for writing only; the Sequence sets what\u{2019}s sung."),
        )
        .into_any_element()
}
