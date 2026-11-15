//! The main pane of the Lyrics task: the selected section's lyric editor, the
//! slide the caret is on, and the Split and Join actions.
//!
//! The text area edits the section's lyrics in the session as text (coalesced
//! into one History point per pause). Split and Join are structural edits and
//! reload the field from the session afterwards.

use crate::edit_session::SaveState;
use domain::{
    identity::SectionId,
    pagination::{slide_at, slides, split_point},
    revisions::EditPoint,
    song_library::ArrangementLyrics,
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled,
    WeakEntity, Window, div, prelude::FluentBuilder as _, px, rgb, svg,
};
use ui::{
    assets::icon,
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
    panes::{column, head, section_name},
    section_actions::{REMOVE_SECTION, rename_control},
    sections::GENERIC_REFUSAL,
};

/// Why the lyric editor cannot be typed in.
const READ_ONLY_REASON: &str = "Inspect only until restored.";

/// The lyric editor's accessible name.
const AREA_LABEL: &str = "Lyrics";

/// What a blank line does, beside Split and Join.
const BOUNDARY_HINT: &str = "A blank line starts a new slide.";

/// The hint while the section has no words yet.
const AREA_PLACEHOLDER: &str =
    "Type or paste the words. Leave a blank line where the next slide should start.";

/// Why Split is unavailable on a slide with one line.
const SPLIT_ONE_LINE: &str = "A slide needs two lines to split.";

/// Why Join is unavailable on the first slide.
const JOIN_FIRST: &str = "The first slide has no slide before it.";

/// Why a split or join has nothing to act on.
const NO_SLIDE: &str = "Type some words first.";

/// What a structural control says after a failed save: Retry save is in the
/// header.
pub const RETRY_TO_KEEP_EDITING: &str = "Couldn't save. Retry to keep editing.";

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

    /// Why the session refused a structural edit: after a failed save, to
    /// retry it; otherwise there was simply nothing to change.
    pub(crate) fn refusal(&self, cx: &App) -> &'static str {
        match self.save_state(cx) {
            Some(SaveState::CouldntSave) => RETRY_TO_KEEP_EDITING,
            _ => GENERIC_REFUSAL,
        }
    }

    /// Takes back the last toast this visit posted. A toast's Undo undoes the
    /// latest point, so it must not outlive the edit that follows it.
    pub(crate) fn dismiss_last_toast(&mut self, cx: &mut Context<Self>) {
        self.dismiss_last_toast_app(cx);
    }

    /// [`Self::dismiss_last_toast`] for the Undo and Redo entry points, which
    /// only hold an `App`.
    pub(super) fn dismiss_last_toast_app(&mut self, cx: &mut App) {
        if let (Some(message), Some(overlays)) =
            (self.lyrics.last_toast.take(), self.lyrics.overlays.clone())
        {
            overlays.dismiss_toast(message, cx);
        }
    }

    /// Posts a confirmation with an Undo button for the edit just made.
    pub(crate) fn notify_undoable(&mut self, message: String, cx: &mut Context<Self>) {
        let Some(overlays) = self.lyrics.overlays.clone() else {
            return;
        };
        self.lyrics.last_toast = Some(message.clone().into());
        let editor = cx.weak_entity();
        overlays.notify(
            Toast::confirmation(message).undo("Undo", move |_, cx| {
                let _ = editor.update(cx, |editor, cx| editor.undo(cx));
            }),
            cx,
        );
    }

    /// Shows a refused structural edit as a toast, so a keypress or click is
    /// never lost silently.
    pub(crate) fn notify_refusal<T>(&mut self, result: Result<T, String>, cx: &mut Context<Self>) {
        if let (Err(copy), Some(overlays)) = (result, self.lyrics.overlays.clone()) {
            self.lyrics.last_toast = Some(copy.clone().into());
            overlays.notify(Toast::failure(copy), cx);
        }
    }

    /// The typed text changed: write it into the shown section.
    fn area_changed(&mut self, cx: &mut Context<Self>) {
        self.dismiss_last_toast(cx);
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
            Some(slide + 1),
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
            Some(slide - 1),
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
        then_slide: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(session) = self.session().cloned() else {
            return Err(GENERIC_REFUSAL.to_owned());
        };
        self.dismiss_last_toast(cx);
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
            return Err(self.refusal(cx).to_owned());
        }
        self.lyrics.reload = true;
        self.lyrics.reload_slide = then_slide;
        self.notify_undoable(message, cx);
        cx.notify();
        Ok(())
    }

    /// Writes the editor's text into the session when they differ.
    pub(crate) fn sync_text(&mut self, cx: &mut Context<Self>) {
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
    let selected_id = content.sections.get(at).map(|section| section.id);
    let sung = match selected_id.map_or(0, |id| content.occurrences_of(id)) {
        0 => "Not in the Sequence yet".to_owned(),
        1 => "Sung 1 time in the Sequence".to_owned(),
        times => format!("Sung {times} times in the Sequence"),
    };
    let middle = generated.get(slide).and_then(split_point);
    let split = if generated.is_empty() {
        Availability::unavailable(NO_SLIDE)
    } else if middle.is_none() {
        Availability::unavailable(SPLIT_ONE_LINE)
    } else {
        Availability::Available
    };
    let split_text = middle.map_or_else(|| "Split slide".to_owned(), split_label);
    let join = if generated.is_empty() {
        Availability::unavailable(NO_SLIDE)
    } else if slide == 0 {
        Availability::unavailable(JOIN_FIRST)
    } else {
        Availability::Available
    };
    let join_text = if slide == 0 {
        "Join with previous slide".to_owned()
    } else {
        join_label(slide)
    };
    let slide_count = match generated.len() {
        1 => "1 slide".to_owned(),
        count => format!("{count} slides"),
    };
    let (split_editor, join_editor, remove_editor) = (weak.clone(), weak.clone(), weak.clone());
    let mut text_area = TextArea::new("arrangement-lyrics-area", AREA_LABEL, &area, environment)
        .placeholder(AREA_PLACEHOLDER)
        .rows(8, 24)
        .on_surface(palette.background);
    if read_only {
        text_area = text_area.read_only(READ_ONLY_REASON);
    }
    let name = if read_only {
        div()
            .debug_selector(|| "arrangement-lyrics-title".into())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(palette.text))
            .child(section_name(&content.sections, at))
            .into_any_element()
    } else {
        rename_control(editor, weak, environment).unwrap_or_else(|| div().into_any_element())
    };
    let meta = |size: f32| {
        div()
            .text_size(px(scale.apply(size)))
            .text_color(rgb(palette.text_muted))
    };
    column(environment)
        .debug_selector(|| "arrangement-lyrics".into())
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(div().flex_1().min_w(px(scale.apply(160.0))).child(name))
                .child(
                    meta(tokens::typography::META.size)
                        .debug_selector(|| "arrangement-section-sung".into())
                        .child(sung),
                )
                .when(!read_only, |row| {
                    row.child(
                        Button::new("arrangement-remove-section", REMOVE_SECTION, environment)
                            .quiet()
                            .on_surface(palette.background)
                            .on_activate(move |window: &mut Window, cx: &mut App| {
                                let _ = remove_editor
                                    .update(cx, |editor, cx| editor.request_remove(window, cx));
                            }),
                    )
                })
                .when(compact, |row| {
                    row.child(inspector_button(weak, environment))
                }),
        )
        .child(
            div()
                .debug_selector(move || format!("arrangement-lyrics-{at}"))
                .child(text_area),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(
                    meta(tokens::typography::META.size)
                        .debug_selector(|| "arrangement-boundary-hint".into())
                        .flex()
                        .items_center()
                        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                        .flex_1()
                        .child(
                            svg()
                                .flex_none()
                                .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                                .text_color(rgb(palette.text_muted))
                                .data(icon::CORNER_DOWN_LEFT.as_bytes()),
                        )
                        .child(BOUNDARY_HINT),
                )
                .when(!read_only, |row| {
                    row.child(
                        Button::new("arrangement-split", split_text, environment)
                            .on_surface(palette.background)
                            .availability(split)
                            .on_activate(move |_, cx| {
                                let _ = split_editor.update(cx, |editor, cx| {
                                    let result = editor.split_selected_slide(cx);
                                    editor.notify_refusal(result, cx);
                                });
                            }),
                    )
                    .child(
                        Button::new("arrangement-join", join_text, environment)
                            .on_surface(palette.background)
                            .availability(join)
                            .on_activate(move |_, cx| {
                                let _ = join_editor.update(cx, |editor, cx| {
                                    let result = editor.join_selected_slide(cx);
                                    editor.notify_refusal(result, cx);
                                });
                            }),
                    )
                }),
        )
        .child(
            div()
                .debug_selector(|| "arrangement-generated-head".into())
                .flex()
                .flex_wrap()
                .items_baseline()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(head("Generated slides", environment))
                .child(
                    meta(tokens::typography::META.size).child(if generated.is_empty() {
                        "No slides yet: this section has no lyrics.".to_owned()
                    } else {
                        format!("{slide_count} \u{b7} select one to inspect")
                    }),
                ),
        )
        .children(thumbnails(editor, weak, content, environment, cx))
        .into_any_element()
}
