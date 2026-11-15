//! The Inspector of the Sequence task: the selected occurrence's selected
//! slide, from the frames the Lyrics task and the Sequence share.

use domain::{
    pagination::slides,
    song_library::{ArrangementLyrics, position_label},
};
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled, div, px, rgb,
};
use ui::{environment::UiEnvironment, tokens};

use super::slide_keys;
use crate::songs::lyrics::{
    ArrangementEditor,
    navigator::{INSPECTOR, PREVIEW_WIDTH, pagination_label, picture},
    panes::{column, head},
};

/// What the Inspector says while the Sequence has nothing to show.
pub const NOTHING: &str = "Nothing to inspect yet.";

/// What it says while occurrences exist but none is selected.
pub const SELECT_ONE: &str = "Select an occurrence to inspect its slides.";

/// What the Inspector describes: the selected slide of the selected
/// occurrence.
struct Shown {
    name: String,
    where_: String,
    pagination: &'static str,
    key: crate::songs::lyrics::navigator::FrameKey,
}

fn shown(editor: &ArrangementEditor, content: &ArrangementLyrics) -> Option<Shown> {
    let selection = editor.sequence_selection?;
    let at = content.occurrence_index(selection.occurrence)?;
    let section = content
        .sections
        .iter()
        .find(|section| section.id == content.sequence[at].section)?;
    let keys = slide_keys(section);
    let count = slides(&section.lyrics).len();
    let slide = selection.slide.min(count.checked_sub(1)?);
    let name = section
        .label
        .clone()
        .unwrap_or_else(|| "Unnamed section".to_owned());
    Some(Shown {
        where_: format!(
            "{} {name} \u{b7} slide {} of {count}",
            position_label(at),
            slide + 1
        ),
        name,
        pagination: pagination_label(section.pagination),
        key: keys[slide].clone(),
    })
}

/// The Inspector's content.
pub fn body(
    editor: &ArrangementEditor,
    content: &ArrangementLyrics,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let Some(shown) = shown(editor, content) else {
        let message = if content.sequence.is_empty() || editor.sequence_selection.is_some() {
            NOTHING
        } else {
            SELECT_ONE
        };
        return div()
            .debug_selector(|| "sequence-inspector-empty".into())
            .text_color(rgb(palette.text_muted))
            .child(message)
            .into_any_element();
    };
    let row = |label: &'static str, value: String| {
        div()
            .flex()
            .justify_between()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(div().text_color(rgb(palette.text_muted)).child(label))
            .child(div().text_color(rgb(palette.text)).child(value))
    };
    column(environment)
        .child(
            div()
                .debug_selector(|| "inspector-preview".into())
                .child(picture(
                    editor,
                    &shown.key,
                    scale.apply(PREVIEW_WIDTH),
                    environment,
                )),
        )
        .child(
            div()
                .debug_selector(|| "inspector-meta".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(row("Section", shown.name.clone()))
                .child(row("Position", shown.where_.clone()))
                .child(row("Pagination", shown.pagination.to_owned())),
        )
        .into_any_element()
}

/// The Inspector pane of a wide window.
pub fn pane(
    editor: &ArrangementEditor,
    content: &ArrangementLyrics,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    column(environment)
        .debug_selector(|| "arrangement-inspector".into())
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(head(INSPECTOR, environment))
                .children(shown(editor, content).map(|shown| {
                    div()
                        .debug_selector(|| "inspector-slide".into())
                        .font_weight(FontWeight::NORMAL)
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(shown.where_)
                })),
        )
        .child(body(editor, content, environment))
        .into_any_element()
}
