//! The Add section popover: one name, prefilled with a suggestion, with the
//! other suggestions as chips that fill the field for editing.
//!
//! Enter adds, Esc cancels (the overlay restores focus to the invoker). The
//! primary stays unavailable while the name is blank or already taken, and a
//! taken name is explained inline. Adding is one structural edit with a fresh
//! section identity.

use domain::{
    identity::{EntityId, SectionId},
    revisions::EditPoint,
    song_library::{ArrangementLyrics, section_name_suggestions},
};
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Styled,
    WeakEntity, Window, div, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::{Overlay, Toast},
    primitives::{
        Availability,
        button::Button,
        field::{Field, FieldState},
    },
    tokens,
};

use super::{
    editor::ArrangementEditor,
    sections::{GENERIC_REFUSAL, refusal_copy},
    view::{AddSection, Anchor},
};

/// The popover's title and its primary action.
pub const ADD_SECTION: &str = "Add section";

/// The popover's field label.
const NAME_LABEL: &str = "Section name";

/// The field's hint while empty.
const NAME_PLACEHOLDER: &str = "Name this part";

/// What is wrong with a typed section name.
enum Problem {
    Blank,
    Taken(String),
}

/// A new section identity, minted by the view as `fresh_session` does.
pub(super) fn fresh_section() -> SectionId {
    SectionId::from_entity(EntityId::from_random_bytes(
        *uuid::Uuid::new_v4().as_bytes(),
    ))
}

/// What is wrong with adding `name` to `content`, if anything.
fn problem(content: &ArrangementLyrics, name: &str) -> Option<Problem> {
    let mut probe = content.clone();
    match probe.add_section(fresh_section(), name) {
        Ok(_) => None,
        Err(_) if name.trim().is_empty() => Some(Problem::Blank),
        Err(error) => Some(Problem::Taken(refusal_copy(&error, name.trim()))),
    }
}

impl ArrangementEditor {
    /// Whether the Add section popover is open.
    pub fn add_section_open(&self) -> bool {
        self.lyrics.add.is_some()
    }

    /// The popover's name field while it is open.
    pub fn add_section_field(&self) -> Option<FieldState> {
        self.lyrics.add.as_ref().map(|add| add.name.clone())
    }

    /// Adds a section named `name` at the end of the Lyrics list, selects it
    /// and asks for the lyric editor to take focus. Refused with the
    /// operator's wording for a blank or taken name, and while an earlier
    /// edit is still saving, so a double Enter adds once.
    pub fn add_section(&mut self, name: &str, cx: &mut gpui::Context<Self>) -> Result<(), String> {
        let Some(session) = self.session().cloned() else {
            return Err(GENERIC_REFUSAL.to_owned());
        };
        let name = name.trim().to_owned();
        let id = fresh_section();
        let probe = session.read(cx).content().clone();
        match problem(&probe, &name) {
            Some(Problem::Blank) => return Err("A section needs a name.".to_owned()),
            Some(Problem::Taken(copy)) => return Err(copy),
            None => {}
        }
        let applied = session.update(cx, |session, cx| {
            session.structural(
                EditPoint::Structural,
                |content| {
                    let before = content.clone();
                    let _ = content.add_section(id, &name);
                    *content = content.clone().reviewed_against(&before);
                },
                cx,
            )
        });
        if !applied {
            return Err(GENERIC_REFUSAL.to_owned());
        }
        self.lyrics.add = None;
        self.lyrics.selected = Some(id);
        self.lyrics.focus_area = true;
        if let Some(overlays) = self.lyrics.overlays.clone() {
            let editor = cx.weak_entity();
            overlays.notify(
                Toast::confirmation(format!("Added section {name}.")).undo("Undo", move |_, cx| {
                    let _ = editor.update(cx, |editor, cx| editor.undo(cx));
                }),
                cx,
            );
        }
        cx.notify();
        Ok(())
    }
}

/// Opens the popover anchored at `anchor`. Does nothing while it is open or
/// when the Arrangement cannot be edited.
pub(super) fn open_add_section(
    editor: &WeakEntity<ArrangementEditor>,
    anchor: &Anchor,
    window: &mut Window,
    cx: &mut App,
) {
    let prepared = editor
        .update(cx, |editor, cx| {
            if editor.lyrics.add.is_some() || editor.is_read_only() {
                return None;
            }
            let overlays = editor.lyrics.overlays.clone()?;
            let content = editor.content(cx)?;
            let languages = editor
                .store()
                .read(cx)
                .library()
                .arrangement(editor.arrangement())
                .map(|found| found.languages().to_vec())
                .unwrap_or_default();
            let (prefill, chips) = section_name_suggestions(&languages, &content);
            let field = FieldState::new(window, cx);
            field.set_value(prefill, window, cx);
            editor.lyrics.add = Some(AddSection {
                name: field.clone(),
            });
            Some((overlays, field, chips))
        })
        .ok()
        .flatten();
    let Some((overlays, field, chips)) = prepared else {
        return;
    };
    let overlay = popover(editor.clone(), anchor.0.get(), field.clone(), chips);
    overlays.open(overlay, window, cx);
    field.focus(window, cx);
}

/// Submits the typed name: adds the section and closes the popover when it
/// stood.
fn submit(
    editor: &WeakEntity<ArrangementEditor>,
    field: &FieldState,
    window: &mut Window,
    cx: &mut App,
) {
    let name = field.value(cx).to_string();
    let added = editor
        .update(cx, |editor, cx| editor.add_section(&name, cx))
        .is_ok_and(|result| result.is_ok());
    if added {
        let overlays = editor
            .update(cx, |editor, _| editor.lyrics.overlays.clone())
            .ok()
            .flatten();
        if let Some(overlays) = overlays {
            overlays.close_top(window, cx);
        }
    }
}

/// The primary's availability for the name typed now.
fn availability(
    editor: &WeakEntity<ArrangementEditor>,
    field: &FieldState,
    cx: &App,
) -> Availability {
    let name = field.value(cx);
    let content = editor
        .upgrade()
        .and_then(|editor| editor.read(cx).content(cx));
    match content.and_then(|content| problem(&content, &name)) {
        None => Availability::Available,
        Some(Problem::Blank) => Availability::Disabled,
        Some(Problem::Taken(copy)) => Availability::unavailable(copy),
    }
}

fn popover(
    editor: WeakEntity<ArrangementEditor>,
    anchor: gpui::Bounds<gpui::Pixels>,
    field: FieldState,
    chips: Vec<String>,
) -> Overlay {
    let gate = {
        let (editor, field) = (editor.clone(), field.clone());
        move |cx: &App| availability(&editor, &field, cx)
    };
    let activate = {
        let (editor, field) = (editor.clone(), field.clone());
        move |window: &mut Window, cx: &mut App| submit(&editor, &field, window, cx)
    };
    let closed = editor.clone();
    Overlay::popover(ADD_SECTION, anchor)
        .body(move |environment, _, cx| body(&editor, &field, &chips, environment, cx))
        .primary_gated(ADD_SECTION, gate, activate)
        .secondary("Cancel", |_, _| {})
        .on_close(move |_, cx| {
            let _ = closed.update(cx, |editor, _| editor.lyrics.add = None);
        })
}

fn body(
    editor: &WeakEntity<ArrangementEditor>,
    field: &FieldState,
    chips: &[String],
    environment: UiEnvironment,
    cx: &mut App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let typed = field.value(cx).to_string();
    let content = editor
        .upgrade()
        .and_then(|editor| editor.read(cx).content(cx));
    let error = match content.and_then(|content| problem(&content, &typed)) {
        Some(Problem::Taken(copy)) => Some(copy),
        _ => None,
    };
    let chip_row = div()
        .debug_selector(|| "add-section-chips".into())
        .flex()
        .flex_wrap()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .children(chips.iter().map(|chip| {
            let (field, label) = (field.clone(), chip.clone());
            Button::new(
                format!("add-section-chip-{chip}"),
                chip.clone(),
                environment,
            )
            .current(typed.trim() == chip)
            .on_surface(palette.surface)
            .on_activate(move |window, cx| {
                field.set_value(label.clone(), window, cx);
                field.focus(window, cx);
            })
        }));
    let (submit_editor, submit_field) = (editor.clone(), field.clone());
    div()
        .debug_selector(|| "add-section-body".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "enter" && !event.keystroke.modifiers.modified() {
                cx.stop_propagation();
                submit(&submit_editor, &submit_field, window, cx);
            }
        })
        .child(
            Field::new("add-section-name", NAME_LABEL, field, environment)
                .placeholder(NAME_PLACEHOLDER)
                .on_surface(palette.surface),
        )
        .children(error.map(|copy| {
            div()
                .debug_selector(|| "add-section-error".into())
                .text_color(rgb(palette.warning))
                .child(copy)
        }))
        .child(chip_row)
        .into_any_element()
}
