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
    AnyElement, App, Context, Div, FontWeight, IntoElement, ParentElement, Styled, Window, div,
    prelude::*, px, rgb,
};
use ui::{environment::UiEnvironment, primitives::button::Button, tokens};

use super::{
    add_section::open_add_section,
    editor::{ArrangementEditor, TaskTab},
    editor_pane::{ensure_area, main_pane},
    navigator::{inspector_pane, request_frames},
    section_actions::ensure_rename,
    sections::{anchored, sections_pane},
};

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

/// The slots for the editor's selected tab. A compact window keeps the
/// sections and the main pane; the Inspector then opens as a slideover.
pub fn build(
    editor: &mut ArrangementEditor,
    compact: bool,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ArrangementEditor>,
) -> Panes {
    let tab = editor.tab(cx);
    let read_only = editor.is_read_only();
    let Some(content) = editor.content(cx) else {
        return Panes {
            sections: None,
            main: missing(environment),
            inspector: None,
        };
    };
    let weak = cx.weak_entity();
    editor.refresh_unrenderable(&content);
    match tab {
        TaskTab::Lyrics => {
            editor.sequence.shown = false;
            editor.lyrics.resolve_selection(&content);
            ensure_area(editor, &weak, &content, window, cx);
            request_frames(editor, &content, cx);
            ensure_rename(editor, &content, window, cx);
            let sections = sections_pane(editor, &weak, &content, read_only, environment, cx);
            if let Some(handle) = editor
                .lyrics
                .focus_row
                .take()
                .and_then(|id| editor.lyrics.rows.get(&id).cloned())
            {
                window.defer(cx, move |window, cx| window.focus(&handle, cx));
            }
            Panes {
                sections: Some(sections),
                main: lyrics_pane(
                    editor,
                    &weak,
                    &content,
                    read_only,
                    compact,
                    environment,
                    window,
                    cx,
                ),
                inspector: (!compact).then(|| inspector_pane(editor, &content, environment, cx)),
            }
        }
        TaskTab::Sequence => crate::songs::sequence::panes(
            editor,
            &weak,
            &content,
            compact,
            read_only,
            environment,
            window,
            cx,
        ),
    }
}

/// A section's name: its label, or its place when it has none.
pub fn section_name(sections: &[SectionDraft], index: usize) -> String {
    sections
        .get(index)
        .and_then(|section| section.label.clone())
        .unwrap_or_else(|| "Unnamed section".to_owned())
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
                .child("This Arrangement is no longer in the library."),
        )
        .child(
            div()
                .text_color(rgb(palette.text_muted))
                .child("Go back to the Songs list."),
        )
        .into_any_element()
}

pub(crate) fn head(text: &'static str, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
        .text_color(rgb(palette.text))
        .child(text)
}

pub(crate) fn column(environment: UiEnvironment) -> Div {
    let scale = environment.preferences().scale;
    div()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .size_full()
}

#[allow(clippy::too_many_arguments)]
fn lyrics_pane(
    editor: &mut ArrangementEditor,
    weak: &gpui::WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    read_only: bool,
    compact: bool,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    if content.sections.is_empty() {
        let (editor_weak, anchor) = (weak.clone(), editor.lyrics.empty_anchor.clone());
        let add = Button::new(
            "arrangement-add-section-button",
            "Add first section",
            environment,
        )
        .on_surface(palette.background)
        .track_focus(&editor.lyrics.add_first)
        .on_activate({
            let anchor = anchor.clone();
            move |window, cx| open_add_section(&editor_weak, &anchor, window, cx)
        });
        if editor.lyrics.focus_add_first && !read_only {
            editor.lyrics.focus_add_first = false;
            let handle = editor.lyrics.add_first.clone();
            window.defer(cx, move |window, cx| handle.focus(window, cx));
        }
        return column(environment)
            .debug_selector(|| "arrangement-lyrics-empty".into())
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
                        "Add each part once, such as a verse, the refrain or an ending. A section used several times is still written once.",
                    ))
                    .when(!read_only, |empty| {
                        empty.child(anchored(add, &anchor, "arrangement-add-section"))
                    }),
            )
            .into_any_element();
    }
    main_pane(editor, weak, content, read_only, compact, environment, cx)
}
