//! The main pane of the Lyrics task: the selected section's lyric editor, the
//! slide the caret is on, and the Split and Join actions.
//!
//! The text area edits the section's lyrics in the session as text (coalesced
//! into one History point per pause). Split and Join are structural edits and
//! reload the field from the session afterwards.

use domain::{
    identity::SectionId,
    pagination::{slide_at, slides, split_point},
    revisions::EditPoint,
    song_library::ArrangementLyrics,
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled,
    WeakEntity, Window, div, prelude::FluentBuilder as _, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::Toast,
    primitives::{
        Availability,
        button::Button,
        text_area::{TextArea, TextAreaEvent, TextAreaState},
    },
    tokens,
};

use super::{
    editor::ArrangementEditor,
    navigator::{inspector_button, thumbnails},
    section_actions::rename_control,
    panes::{column, section_name},
    sections::GENERIC_REFUSAL,
};

/// Why the lyric editor cannot be typed in.
const READ_ONLY_REASON: &str = "Archived Arrangements can\u{2019}t be edited.";

/// The lyric editor's accessible name.
const AREA_LABEL: &str = "Lyrics";

/// The hint while the section has no words yet.
const AREA_PLACEHOLDER: &str = "Type the words. Leave a blank line between slides.";

/// Why Split is unavailable on a slide with one line.
const SPLIT_ONE_LINE: &str = "A slide needs two lines to split.";

/// Why Join is unavailable on the first slide.
const JOIN_FIRST: &str = "The first slide has no slide before it.";

/// Why a split or join has nothing to act on.
const NO_SLIDE: &str = "Type some words first.";

/// The label of Split for a slide whose middle is after line `line`.
pub fn split_label(line: usize) -> String {
    format!("Split after line {line}")
}

/// The label of Join for the slide at 0-based `slide`.
pub fn join_label(slide: usize) -> String {
    format!("Join slide {} with slide {}", slide + 1, slide)
}

impl ArrangementEditor {
    /// The lyric editor's state once a section is shown.
    pub fn lyric_area(&self) -> Option<TextAreaState> {
        self.lyrics.area.clone()
    }

    /// The slide the caret is on (0-based).
    pub fn selected_slide(&self) -> usize {
        self.lyrics.slide
    }

    /// The typed text changed: write it into the shown section.
    fn area_changed(&mut self, cx: &mut Context<Self>) {
        let (Some(area), Some(id), Some(session)) = (
            self.lyrics.area.clone(),
            self.lyrics.shown,
            self.session().cloned(),
        ) else {
            return;
        };
        let value = area.value(cx).to_string();
        session.update(cx, |session, cx| {
            session.text_edited(
                |content| {
                    let before = content.clone();
                    let _ = content.set_section_lyrics(id, value);
                    *content = content.clone().reviewed_against(&before);
                },
                cx,
            );
        });
        self.refresh_slide(cx);
        cx.notify();
    }

    /// Focus left the lyric editor: commit pending text now.
    fn area_focus_left(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.session().cloned() {
            session.update(cx, |session, cx| session.focus_left(cx));
        }
    }

    /// Follows the caret to its slide, redrawing only when that changes.
    fn refresh_slide(&mut self, cx: &mut Context<Self>) {
        let Some(area) = self.lyrics.area.clone() else {
            return;
        };
        let (value, caret) = (area.value(cx), area.cursor(cx));
        let slide = slide_at(&value, caret).unwrap_or(0);
        if slide != self.lyrics.slide {
            self.lyrics.slide = slide;
            cx.notify();
        }
    }

    /// Selects slide `slide` (0-based) in the editor and gives it focus.
    pub fn select_slide(&mut self, slide: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(area) = self.lyrics.area.clone() else {
            return;
        };
        let value = area.value(cx);
        if let Some(source) = slides(&value).get(slide) {
            area.set_selected_range(source.range.clone(), cx);
            self.lyrics.slide = slide;
            area.focus(window, cx);
            cx.notify();
        }
    }

    /// Splits the selected slide at its middle line.
    pub fn split_selected_slide(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some((id, slide, text)) = self.slide_target(cx) else {
            return Err(NO_SLIDE.to_owned());
        };
        let source = slides(&text)
            .get(slide)
            .cloned()
            .ok_or_else(|| NO_SLIDE.to_owned())?;
        let line = split_point(&source).ok_or_else(|| SPLIT_ONE_LINE.to_owned())?;
        self.structural_lyrics(
            move |content| content.split_slide(id, slide).unwrap_or(false),
            format!("Split slide {} after line {line}.", slide + 1),
            slide,
            cx,
        )
    }

    /// Joins the selected slide with the one before it.
    pub fn join_selected_slide(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some((id, slide, _)) = self.slide_target(cx) else {
            return Err(NO_SLIDE.to_owned());
        };
        if slide == 0 {
            return Err(JOIN_FIRST.to_owned());
        }
        self.structural_lyrics(
            move |content| content.join_slide(id, slide).unwrap_or(false),
            format!("Joined slide {} with slide {}.", slide + 1, slide),
            slide - 1,
            cx,
        )
    }

    /// The shown section, the selected slide and the text now in the editor.
    fn slide_target(&self, cx: &App) -> Option<(SectionId, usize, String)> {
        let id = self.lyrics.shown?;
        let text = self.lyrics.area.as_ref()?.value(cx).to_string();
        let count = slides(&text).len();
        (count > 0).then(|| (id, self.lyrics.slide.min(count - 1), text))
    }

    /// One structural lyrics edit with its reload, toast and Undo.
    pub(super) fn structural_lyrics(
        &mut self,
        change: impl FnOnce(&mut ArrangementLyrics) -> bool,
        message: String,
        then_slide: usize,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(session) = self.session().cloned() else {
            return Err(GENERIC_REFUSAL.to_owned());
        };
        // Typed text joins the session before the structural edit starts
        // from it.
        self.sync_text(cx);
        let applied = session.update(cx, |session, cx| {
            session.structural(
                EditPoint::Structural,
                |content| {
                    let before = content.clone();
                    change(content);
                    *content = content.clone().reviewed_against(&before);
                },
                cx,
            )
        });
        if !applied {
            return Err(GENERIC_REFUSAL.to_owned());
        }
        self.lyrics.reload = true;
        self.lyrics.reload_slide = Some(then_slide);
        if let Some(overlays) = self.lyrics.overlays.clone() {
            let editor = cx.weak_entity();
            overlays.notify(
                Toast::confirmation(message).undo("Undo", move |_, cx| {
                    let _ = editor.update(cx, |editor, cx| editor.undo(cx));
                }),
                cx,
            );
        }
        cx.notify();
        Ok(())
    }

    /// Writes the editor's text into the session when they differ.
    fn sync_text(&mut self, cx: &mut Context<Self>) {
        let (Some(area), Some(id), Some(session)) = (
            self.lyrics.area.clone(),
            self.lyrics.shown,
            self.session().cloned(),
        ) else {
            return;
        };
        let value = area.value(cx).to_string();
        let current = session
            .read(cx)
            .content()
            .sections
            .iter()
            .find(|section| section.id == id)
            .map(|section| section.lyrics.clone());
        if current.as_deref().is_some_and(|stored| stored != value) {
            self.area_changed(cx);
        }
    }
}

/// Makes the lyric editor on first use and loads the selected section into
/// it when the section changed or the content was replaced.
pub(super) fn ensure_area(
    editor: &mut ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    window: &mut Window,
    cx: &mut Context<ArrangementEditor>,
) {
    if editor.lyrics.area.is_none() {
        let area = TextAreaState::new(window, cx);
        let (changes, caret) = (weak.clone(), weak.clone());
        let events = area.subscribe(cx, move |event, cx| {
            let _ = changes.update(cx, |editor, cx| match event {
                TextAreaEvent::Changed => editor.area_changed(cx),
                TextAreaEvent::FocusLeft => editor.area_focus_left(cx),
            });
        });
        let moves = area.observe(cx, move |cx| {
            let _ = caret.update(cx, |editor, cx| editor.refresh_slide(cx));
        });
        editor.lyrics.subscriptions.extend([events, moves]);
        editor.lyrics.area = Some(area);
    }
    let Some(area) = editor.lyrics.area.clone() else {
        return;
    };
    let selected = editor.lyrics.selected;
    if editor.lyrics.shown != selected || editor.lyrics.reload {
        let words = content
            .sections
            .iter()
            .find(|section| Some(section.id) == selected)
            .map(|section| section.lyrics.clone())
            .unwrap_or_default();
        if area.value(cx).as_ref() != words {
            area.set_value(words, window, cx);
        }
        if editor.lyrics.shown != selected {
            editor.lyrics.slide = 0;
        }
        if let Some(slide) = editor.lyrics.reload_slide.take()
            && let Some(source) = slides(&area.value(cx)).get(slide)
        {
            let start = source.range.start;
            area.set_selected_range(start..start, cx);
            editor.lyrics.slide = slide;
        }
        editor.lyrics.shown = selected;
        editor.lyrics.reload = false;
    }
    if editor.lyrics.focus_area {
        editor.lyrics.focus_area = false;
        let handle = area.focus_handle(cx);
        window.defer(cx, move |window, cx| window.focus(&handle, cx));
    }
}

/// The main pane for a section that exists.
pub(super) fn main_pane(
    editor: &ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    read_only: bool,
    compact: bool,
    environment: UiEnvironment,
    cx: &App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let Some(area) = editor.lyrics.area.clone() else {
        return column(environment).into_any_element();
    };
    let at = content
        .sections
        .iter()
        .position(|section| Some(section.id) == editor.lyrics.selected)
        .unwrap_or(0);
    let text = area.value(cx);
    let generated = slides(&text);
    let slide = editor.lyrics.slide.min(generated.len().saturating_sub(1));
    let caption = if generated.is_empty() {
        "No slides yet".to_owned()
    } else {
        format!("Slide {} of {}", slide + 1, generated.len())
    };
    let middle = generated.get(slide).and_then(split_point);
    let split = if read_only {
        Availability::unavailable(READ_ONLY_REASON)
    } else if generated.is_empty() {
        Availability::unavailable(NO_SLIDE)
    } else if middle.is_none() {
        Availability::unavailable(SPLIT_ONE_LINE)
    } else {
        Availability::Available
    };
    let split_text = middle.map_or_else(|| "Split slide".to_owned(), split_label);
    let join = if read_only {
        Availability::unavailable(READ_ONLY_REASON)
    } else if generated.is_empty() {
        Availability::unavailable(NO_SLIDE)
    } else if slide == 0 {
        Availability::unavailable(JOIN_FIRST)
    } else {
        Availability::Available
    };
    let join_text = if slide == 0 {
        "Join with previous".to_owned()
    } else {
        join_label(slide)
    };
    let (split_editor, join_editor) = (weak.clone(), weak.clone());
    let mut text_area = TextArea::new("arrangement-lyrics-area", AREA_LABEL, &area, environment)
        .placeholder(AREA_PLACEHOLDER)
        .rows(8, 24)
        .on_surface(palette.background);
    if read_only {
        text_area = text_area.read_only(READ_ONLY_REASON);
    }
    column(environment)
        .debug_selector(|| "arrangement-overview".into())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(
                    div()
                        .debug_selector(|| "arrangement-lyrics-title".into())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                        .text_color(rgb(palette.text))
                        .child(section_name(&content.sections, at)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .child(
                            div()
                                .debug_selector(|| "arrangement-slide-caption".into())
                                .text_color(rgb(palette.text_muted))
                                .child(caption),
                        )
                        .when(compact, |group| {
                            group.child(inspector_button(weak, environment))
                        }),
                ),
        )
        .children((!read_only).then(|| rename_control(editor, weak, environment)).flatten())
        .children(thumbnails(editor, weak, content, environment, cx))
        .child(
            div()
                .debug_selector(move || format!("arrangement-lyrics-{at}"))
                .child(text_area),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(
                    Button::new("arrangement-split", split_text, environment)
                        .on_surface(palette.background)
                        .availability(split)
                        .on_activate(move |_, cx| {
                            let _ = split_editor.update(cx, |editor, cx| {
                                let _ = editor.split_selected_slide(cx);
                            });
                        }),
                )
                .child(
                    Button::new("arrangement-join", join_text, environment)
                        .on_surface(palette.background)
                        .availability(join)
                        .on_activate(move |_, cx| {
                            let _ = join_editor.update(cx, |editor, cx| {
                                let _ = editor.join_selected_slide(cx);
                            });
                        }),
                ),
        )
        .into_any_element()
}
