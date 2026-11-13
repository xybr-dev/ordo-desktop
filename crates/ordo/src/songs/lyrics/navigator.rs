//! The slide navigator and the Inspector of the Lyrics task.
//!
//! Every slide of the selected section is a thumbnail of the canonical frame
//! the congregation would see. Frames are composed off the GPUI thread and
//! cached by (header, body); a section change drops requests nobody waits for
//! any more. Selecting a thumbnail selects the slide's lines in the editor,
//! and the one Inspector describes the selected slide.

use std::{collections::HashSet, sync::Arc};

use domain::{
    pagination::{slide_body, slides},
    song_library::{ArrangementLyrics, PaginationState},
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WeakEntity, Window, div, px, rgb,
};
use rendering::{TemplateContent, TemplateKind};
use ui::{environment::UiEnvironment, overlay::Overlay, primitives::button::Button, tokens};

use super::{
    editor::ArrangementEditor,
    panes::{column, head, section_name},
};
use crate::canonical_views::live_viewport;

/// The width of every thumbnail in the navigator, before the Interface scale.
/// Its height is 9/16 of it. One size for a lone slide and a full row alike.
pub const THUMBNAIL_WIDTH: f32 = 160.0;

/// The width of the Inspector's preview, before the Interface scale.
pub const PREVIEW_WIDTH: f32 = 240.0;

/// The title of the Inspector slideover and its opening button.
pub const INSPECTOR: &str = "Inspector";

/// What a composed frame is cached under: the section name and the slide's
/// rendered body.
pub type FrameKey = (String, String);

/// What the navigator and the Inspector say about the selected slide.
struct Selected {
    section: String,
    slide: usize,
    count: usize,
    bodies: Vec<String>,
}

impl Selected {
    fn key(&self, slide: usize) -> FrameKey {
        (self.section.clone(), self.bodies[slide].clone())
    }
}

/// The selected section's slides, from the text now in the editor.
fn selected(editor: &ArrangementEditor, content: &ArrangementLyrics, cx: &App) -> Option<Selected> {
    let at = content
        .sections
        .iter()
        .position(|section| Some(section.id) == editor.lyrics.selected)?;
    let text = editor
        .lyrics
        .area
        .as_ref()
        .map(|area| area.value(cx).to_string())
        .unwrap_or_else(|| content.sections[at].lyrics.clone());
    let generated = slides(&text);
    let bodies: Vec<String> = generated
        .iter()
        .map(|source| slide_body(&text, source))
        .collect();
    Some(Selected {
        section: section_name(&content.sections, at),
        slide: editor.lyrics.slide.min(generated.len().saturating_sub(1)),
        count: generated.len(),
        bodies,
    })
}

/// Starts composing the selected section's slides that nothing holds yet and
/// drops the requests of other sections.
pub(super) fn request_frames(
    editor: &mut ArrangementEditor,
    content: &ArrangementLyrics,
    cx: &mut Context<ArrangementEditor>,
) {
    let Some(chosen) = selected(editor, content, cx) else {
        editor.lyrics.pending.clear();
        return;
    };
    let wanted: HashSet<FrameKey> = (0..chosen.count).map(|slide| chosen.key(slide)).collect();
    editor.lyrics.pending.retain(|key, _| wanted.contains(key));
    for key in wanted {
        if editor.lyrics.frames.peek(&key).is_some() || editor.lyrics.pending.contains_key(&key) {
            continue;
        }
        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some(key.0.clone()),
            body: key.1.clone(),
            footer: None,
            image: None,
        };
        let stored = key.clone();
        let task = cx.spawn(async move |editor, cx| {
            let composed = cx
                .background_executor()
                .spawn(async move { rendering::compose(&content) })
                .await;
            let _ = editor.update(cx, |editor, cx| {
                editor.lyrics.pending.remove(&stored);
                editor.lyrics.frames.insert(stored, composed.map(Arc::new));
                cx.notify();
            });
        });
        editor.lyrics.pending.insert(key, task);
    }
}

/// A frame box of `width`: the composed frame when it is ready, else an empty
/// box of the same size.
fn picture(
    editor: &ArrangementEditor,
    key: &FrameKey,
    width: f32,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    match editor.lyrics.frames.peek(key) {
        Some(rendering::FrameEntry::Ready(frame)) => {
            live_viewport(Some(frame.clone()), None, width, None, environment)
        }
        _ => div()
            .w(px(width))
            .h(px(width * 9.0 / 16.0))
            .rounded(px(tokens::radius::SLIDE_VIEWPORT))
            .bg(rgb(palette.surface_secondary))
            .into_any_element(),
    }
}

/// The wrapping row of thumbnails of the selected section.
pub(super) fn thumbnails(
    editor: &ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    environment: UiEnvironment,
    cx: &App,
) -> Option<AnyElement> {
    let chosen = selected(editor, content, cx).filter(|chosen| chosen.count > 0)?;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let width = scale.apply(THUMBNAIL_WIDTH);
    let tiles = (0..chosen.count).map(|slide| {
        let choose = weak.clone();
        let current = slide == chosen.slide;
        let key = chosen.key(slide);
        div()
            .id(("arrangement-thumbnail", slide))
            .debug_selector(move || format!("thumb-{slide}"))
            .flex_none()
            .w(px(width))
            .h(px(width * 9.0 / 16.0))
            .cursor_pointer()
            .rounded(px(tokens::radius::SLIDE_VIEWPORT))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(if current {
                tokens::color::PRIMARY
            } else {
                palette.border
            }))
            .on_click(move |_, window, cx| {
                let _ = choose.update(cx, |editor, cx| editor.select_slide(slide, window, cx));
            })
            .child(picture(editor, &key, width, environment))
    });
    Some(
        div()
            .debug_selector(|| "arrangement-thumbnails".into())
            .flex()
            .flex_wrap()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .children(tiles)
            .into_any_element(),
    )
}

/// The pagination state in the operator's words.
pub fn pagination_label(state: PaginationState) -> &'static str {
    match state {
        PaginationState::Draft => "Draft",
        PaginationState::Approved => "Approved",
        PaginationState::NeedsReview => "Needs review",
    }
}

/// The Inspector's content: the selected slide, larger, and what is known of
/// it.
pub(super) fn inspector_body(
    editor: &ArrangementEditor,
    content: &ArrangementLyrics,
    environment: UiEnvironment,
    cx: &App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let Some(chosen) = selected(editor, content, cx).filter(|chosen| chosen.count > 0) else {
        return div()
            .text_color(rgb(palette.text_muted))
            .child("Nothing to inspect yet.")
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
                .debug_selector(|| "inspector-slide".into())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child(format!("Slide {} of {}", chosen.slide + 1, chosen.count)),
        )
        .child(
            div()
                .debug_selector(|| "inspector-preview".into())
                .child(picture(
                    editor,
                    &chosen.key(chosen.slide),
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
                .child(row("Section", chosen.section.clone()))
                .child(row(
                    "Position",
                    format!("{} of {}", chosen.slide + 1, chosen.count),
                ))
                .child(row(
                    "Pagination",
                    pagination_label(content.pagination).to_owned(),
                )),
        )
        .into_any_element()
}

/// The Inspector pane of a wide window.
pub(super) fn inspector_pane(
    editor: &ArrangementEditor,
    content: &ArrangementLyrics,
    environment: UiEnvironment,
    cx: &App,
) -> AnyElement {
    column(environment)
        .debug_selector(|| "arrangement-inspector".into())
        .child(head(INSPECTOR, environment))
        .child(inspector_body(editor, content, environment, cx))
        .into_any_element()
}

/// The button of a narrow window that opens the Inspector as a slideover.
pub(super) fn inspector_button(
    weak: &WeakEntity<ArrangementEditor>,
    environment: UiEnvironment,
) -> Button {
    let weak = weak.clone();
    let palette = tokens::palette(environment.appearance());
    Button::new("arrangement-inspector-button", INSPECTOR, environment)
        .on_surface(palette.background)
        .on_activate(move |window, cx| open_inspector(&weak, window, cx))
}

/// Opens the Inspector as a labelled slideover whose body follows the
/// editor.
fn open_inspector(weak: &WeakEntity<ArrangementEditor>, window: &mut Window, cx: &mut App) {
    let Some(overlays) = weak
        .update(cx, |editor, _| editor.lyrics.overlays.clone())
        .ok()
        .flatten()
    else {
        return;
    };
    let editor = weak.clone();
    let overlay = Overlay::slideover(INSPECTOR)
        .description("The selected slide and what is known of it.")
        .body(move |environment, _, cx| {
            let Some(editor) = editor.upgrade() else {
                return div().into_any_element();
            };
            let editor = editor.read(cx);
            match editor.content(cx) {
                Some(content) => inspector_body(editor, &content, environment, cx),
                None => div().into_any_element(),
            }
        })
        .secondary("Close", |_, _| {});
    overlays.open(overlay, window, cx);
}
