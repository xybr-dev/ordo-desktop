//! The Arrangement editor's pane slots.
//!
//! The shell lays three slots out side by side and wraps each in its F6
//! region (Primary list, Main editor, Inspector). This module fills them. The
//! bodies here are the read-only frame's: the sections pane lists names, the
//! main pane shows the lyrics (or the empty state) and the Inspector says
//! there is nothing to inspect. The lyric editor replaces `main`, the
//! sections list replaces `sections` and the slide preview replaces
//! `inspector`, without changing the frame.

use domain::song_library::{ArrangementLyrics, SectionDraft};
use gpui::{
    AnyElement, Div, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{Availability, button::Button},
    tokens,
};

use super::editor::TaskTab;

/// Why sections cannot be added yet, in the operator's words.
const ADD_SECTION_REASON: &str = "Writing lyrics isn't available yet";

/// The Sequence tab's note.
pub const SEQUENCE_NOTE: &str = "Editing the Sequence isn't available yet";

/// The three slots of the editor body. A slot that is `None` is not shown
/// (the Sequence tab has a single pane, and a narrow window keeps only the
/// main one).
pub struct Panes {
    /// The sections list, in the Primary list region.
    pub sections: Option<AnyElement>,
    /// The task's main pane, in the Main editor region.
    pub main: AnyElement,
    /// The Inspector.
    pub inspector: Option<AnyElement>,
}

/// The slots for `tab`. `content` is `None` when the Arrangement is gone.
pub fn build(
    tab: TaskTab,
    content: Option<&ArrangementLyrics>,
    read_only: bool,
    compact: bool,
    environment: UiEnvironment,
) -> Panes {
    let Some(content) = content else {
        return Panes {
            sections: None,
            main: missing(environment),
            inspector: None,
        };
    };
    match tab {
        TaskTab::Lyrics => Panes {
            sections: (!compact).then(|| sections_pane(content, environment)),
            main: lyrics_pane(content, read_only, environment),
            inspector: (!compact).then(|| inspector_pane(environment)),
        },
        TaskTab::Sequence => Panes {
            sections: None,
            main: sequence_pane(content, environment),
            inspector: None,
        },
    }
}

/// A section's name: its label, or its place when it has none.
pub fn section_name(sections: &[SectionDraft], index: usize) -> String {
    sections
        .get(index)
        .and_then(|section| section.label.clone())
        .unwrap_or_else(|| format!("Section {}", index + 1))
}

fn missing(environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "arrangement-missing".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child("This Arrangement is no longer in the library"),
        )
        .child(
            div()
                .text_color(rgb(palette.text_muted))
                .child("Go back to the Songs list."),
        )
        .into_any_element()
}

fn head(text: &'static str, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
        .text_color(rgb(palette.text))
        .child(text)
}

fn column(environment: UiEnvironment) -> Div {
    let scale = environment.preferences().scale;
    div()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .size_full()
}

fn sections_pane(content: &ArrangementLyrics, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let rows = content.sections.iter().enumerate().map(|(at, section)| {
        div()
            .debug_selector(move || format!("arrangement-section-{at}"))
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(
                div()
                    .min_w_0()
                    .text_color(rgb(palette.text))
                    .child(section_name(&content.sections, at)),
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
    column(environment)
        .debug_selector(|| "arrangement-sections".into())
        .child(head("Sections", environment))
        .children(rows)
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child("Lyrics order is for writing only; the Sequence sets what\u{2019}s sung."),
        )
        .into_any_element()
}

fn lyrics_pane(
    content: &ArrangementLyrics,
    read_only: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    if content.sections.is_empty() {
        let add = Button::new(
            "arrangement-add-section-button",
            "Add first section",
            environment,
        )
        .on_surface(palette.background)
        .availability(Availability::unavailable(ADD_SECTION_REASON));
        return column(environment)
            .debug_selector(|| "arrangement-overview".into())
            .child(
                div()
                    .debug_selector(|| "arrangement-no-sections".into())
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.text))
                            .child("No sections yet"),
                    )
                    .child(div().text_color(rgb(palette.text_muted)).child(
                        "Add each part once, such as a verse, the refrain or an ending. \
                         A section used several times is still written once.",
                    ))
                    .when(!read_only, |empty| {
                        empty.child(
                            div()
                                .debug_selector(|| "arrangement-add-section".into())
                                .child(add),
                        )
                    }),
            )
            .into_any_element();
    }
    column(environment)
        .debug_selector(|| "arrangement-overview".into())
        .children(content.sections.iter().enumerate().map(|(at, section)| {
            div()
                .debug_selector(move || format!("arrangement-lyrics-{at}"))
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(section_name(&content.sections, at)),
                )
                .children(
                    section
                        .lyrics
                        .lines()
                        .map(|line| div().text_color(rgb(palette.text)).child(line.to_owned())),
                )
        }))
        .into_any_element()
}

fn inspector_pane(environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    column(environment)
        .debug_selector(|| "arrangement-inspector".into())
        .child(head("Inspector", environment))
        .child(
            div()
                .text_color(rgb(palette.text_muted))
                .child("Nothing to inspect yet."),
        )
        .into_any_element()
}

fn sequence_pane(content: &ArrangementLyrics, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let rows = content.sequence.iter().enumerate().map(|(at, occurrence)| {
        let index = content
            .sections
            .iter()
            .position(|section| section.id == occurrence.section)
            .unwrap_or(0);
        div()
            .debug_selector(move || format!("arrangement-sequence-{at}"))
            .flex()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .text_color(rgb(palette.text_muted))
                    .child(format!("{:02}", at + 1)),
            )
            .child(section_name(&content.sections, index))
    });
    column(environment)
        .child(head("Sequence", environment))
        .child(
            div()
                .debug_selector(|| "arrangement-sequence-note".into())
                .text_color(rgb(palette.text_muted))
                .child(SEQUENCE_NOTE),
        )
        .when(content.sequence.is_empty(), |pane| {
            pane.child(
                div()
                    .text_color(rgb(palette.text))
                    .child("No performance sequence yet"),
            )
        })
        .child(
            div()
                .debug_selector(|| "arrangement-sequence".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .children(rows),
        )
        .into_any_element()
}
