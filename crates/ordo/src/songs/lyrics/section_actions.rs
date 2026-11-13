//! Rename, remove and reorder of the sections of the Lyrics list.
//!
//! Each is one structural edit with its own revision, a confirmation toast
//! and Undo. Reordering never touches the Sequence, a rename keeps the
//! approval, and removing a section the Sequence sings also removes those
//! occurrences after the operator confirms.

use domain::{identity::SectionId, song_library::Step};
use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Styled, Subscription, WeakEntity, Window, div, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::Overlay,
    primitives::{
        Availability,
        button::Button,
        field::{Field, FieldState},
    },
    tokens,
};

use super::{
    editor::ArrangementEditor,
    panes::section_name,
    sections::{GENERIC_REFUSAL, refusal_copy},
};

/// The rename field's label.
const NAME_LABEL: &str = "Section name";

/// The label of the remove action and of the dialog's primary.
pub const REMOVE_SECTION: &str = "Remove section";

/// The rename field of the selected section.
pub(super) struct Rename {
    pub id: SectionId,
    pub field: FieldState,
    pub error: Option<String>,
    _blur: Subscription,
}

impl ArrangementEditor {
    /// The selected section's rename field while the panes show one.
    pub fn rename_field(&self) -> Option<FieldState> {
        self.lyrics
            .rename
            .as_ref()
            .map(|rename| rename.field.clone())
    }

    /// Moves a section one place in the Lyrics list. A move past an edge does
    /// nothing and writes no revision. Returns whether it moved.
    pub fn move_section(
        &mut self,
        id: SectionId,
        step: Step,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        let Some(content) = self.content(cx) else {
            return Err(GENERIC_REFUSAL.to_owned());
        };
        let name = named(&content, id);
        let mut probe = content.clone();
        match probe.move_section(id, step) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(error) => return Err(refusal_copy(&error, &name)),
        }
        self.structural_lyrics(
            |content| content.move_section(id, step).is_ok(),
            format!("Moved {name} in Lyrics. The Sequence is unchanged."),
            0,
            cx,
        )?;
        self.lyrics.focus_row = Some(id);
        Ok(true)
    }

    /// Renames a section. Refused in the operator's words for a blank or
    /// taken name.
    pub fn rename_section(
        &mut self,
        id: SectionId,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(content) = self.content(cx) else {
            return Err(GENERIC_REFUSAL.to_owned());
        };
        let name = name.trim().to_owned();
        let old = named(&content, id);
        let mut probe = content.clone();
        if let Err(error) = probe.rename_section(id, &name) {
            return Err(refusal_copy(&error, &name));
        }
        if name == old {
            return Ok(());
        }
        self.structural_lyrics(
            |content| content.rename_section(id, &name).is_ok(),
            format!("Renamed {old} to {name}."),
            self.lyrics.slide,
            cx,
        )
    }

    /// Removes a section with the occurrences that place it.
    pub fn remove_section(&mut self, id: SectionId, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(content) = self.content(cx) else {
            return Err(GENERIC_REFUSAL.to_owned());
        };
        let name = named(&content, id);
        self.structural_lyrics(
            |content| content.remove_section(id).is_ok(),
            format!("Removed {name}."),
            0,
            cx,
        )
    }

    /// Removes the selected section: at once when the Sequence does not sing
    /// it, else after a confirmation that says what else goes.
    pub fn request_remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(id), Some(content)) = (self.lyrics.selected, self.content(cx)) else {
            return;
        };
        let used = content.occurrences_of(id);
        if used == 0 {
            let _ = self.remove_section(id, cx);
            return;
        }
        let Some(overlays) = self.lyrics.overlays.clone() else {
            return;
        };
        let name = named(&content, id);
        let weak = cx.weak_entity();
        let times = if used == 1 {
            "once".to_owned()
        } else {
            format!("{used} times")
        };
        let text = format!(
            "{name} is sung {times} in the Sequence. Removing it also removes {}.",
            if used == 1 {
                "that occurrence".to_owned()
            } else {
                format!("those {used} occurrences")
            }
        );
        let overlay = Overlay::dialog(format!("Remove {name}?"))
            .body(move |environment, _, _| {
                let palette = tokens::palette(environment.appearance());
                div()
                    .text_color(rgb(palette.text))
                    .child(text.clone())
                    .into_any_element()
            })
            .primary(REMOVE_SECTION, move |_, cx| {
                let _ = weak.update(cx, |editor, cx| editor.remove_section(id, cx));
            })
            .secondary("Cancel", |_, _| {});
        overlays.open(overlay, window, cx);
    }

    /// Commits the typed name of the rename field. A refusal on Enter keeps
    /// the text and says why; on blur the old name comes back.
    pub fn commit_rename(&mut self, blur: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, field)) = self
            .lyrics
            .rename
            .as_ref()
            .map(|rename| (rename.id, rename.field.clone()))
        else {
            return;
        };
        let typed = field.value(cx).to_string();
        let result = self.rename_section(id, &typed, cx);
        let current = self
            .content(cx)
            .map(|content| named(&content, id))
            .unwrap_or_default();
        match result {
            Ok(()) => {
                if let Some(rename) = self.lyrics.rename.as_mut() {
                    rename.error = None;
                }
            }
            Err(copy) if blur => {
                field.set_value(current, window, cx);
                if let Some(rename) = self.lyrics.rename.as_mut() {
                    rename.error = None;
                }
                let _ = copy;
            }
            Err(copy) => {
                if let Some(rename) = self.lyrics.rename.as_mut() {
                    rename.error = Some(copy);
                }
            }
        }
        cx.notify();
    }

    /// Esc in the rename field: the stored name comes back.
    pub fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, field)) = self
            .lyrics
            .rename
            .as_ref()
            .map(|rename| (rename.id, rename.field.clone()))
        else {
            return;
        };
        let current = self
            .content(cx)
            .map(|content| named(&content, id))
            .unwrap_or_default();
        field.set_value(current, window, cx);
        if let Some(rename) = self.lyrics.rename.as_mut() {
            rename.error = None;
        }
        cx.notify();
    }
}

fn named(content: &domain::song_library::ArrangementLyrics, id: SectionId) -> String {
    content
        .sections
        .iter()
        .position(|section| section.id == id)
        .map(|at| section_name(&content.sections, at))
        .unwrap_or_default()
}

/// Keeps the rename field on the selected section and showing its stored
/// name. Called from the pane builder, which owns the window.
pub(super) fn ensure_rename(
    editor: &mut ArrangementEditor,
    content: &domain::song_library::ArrangementLyrics,
    window: &mut Window,
    cx: &mut Context<ArrangementEditor>,
) {
    let Some(id) = editor.lyrics.selected else {
        editor.lyrics.rename = None;
        return;
    };
    let stored = named(content, id);
    match editor.lyrics.rename.as_ref() {
        Some(rename) if rename.id == id => {
            // Show the stored name again after a reload, unless the operator
            // is typing in the field.
            if editor.lyrics.reload && !rename.field.focus_handle(cx).is_focused(window) {
                rename.field.set_value(stored, window, cx);
            }
        }
        _ => {
            let field = FieldState::new(window, cx);
            field.set_value(stored, window, cx);
            let handle: FocusHandle = field.focus_handle(cx);
            let blur = cx.on_focus_out(&handle, window, |editor, _, window, cx| {
                editor.commit_rename(true, window, cx);
            });
            editor.lyrics.rename = Some(Rename {
                id,
                field,
                error: None,
                _blur: blur,
            });
        }
    }
}

/// The rename field with its inline refusal.
pub(super) fn rename_control(
    editor: &ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    environment: UiEnvironment,
) -> Option<AnyElement> {
    let rename = editor.lyrics.rename.as_ref()?;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let weak = weak.clone();
    Some(
        div()
            .debug_selector(|| "arrangement-rename".into())
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let plain = !event.keystroke.modifiers.modified();
                match event.keystroke.key.as_str() {
                    "enter" if plain => {
                        cx.stop_propagation();
                        let _ =
                            weak.update(cx, |editor, cx| editor.commit_rename(false, window, cx));
                    }
                    "escape" if plain => {
                        cx.stop_propagation();
                        let _ = weak.update(cx, |editor, cx| editor.cancel_rename(window, cx));
                    }
                    _ => {}
                }
            })
            .child(
                Field::new(
                    "arrangement-rename-field",
                    NAME_LABEL,
                    &rename.field,
                    environment,
                )
                .on_surface(palette.background),
            )
            .children(rename.error.clone().map(|copy| {
                div()
                    .debug_selector(|| "arrangement-rename-error".into())
                    .text_color(rgb(palette.warning))
                    .child(copy)
            }))
            .into_any_element(),
    )
}

/// The Move up, Move down and Remove section buttons under the list.
pub(super) fn section_buttons(
    weak: &WeakEntity<ArrangementEditor>,
    content: &domain::song_library::ArrangementLyrics,
    selected: Option<SectionId>,
    read_only: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let at = selected.and_then(|id| content.sections.iter().position(|s| s.id == id));
    let last = content.sections.len().saturating_sub(1);
    let gate = |applicable: bool| {
        if read_only || at.is_none() || !applicable {
            Availability::Disabled
        } else {
            Availability::Available
        }
    };
    let mover = |id: &'static str, label: &'static str, tip: &'static str, step: Step, on: bool| {
        let weak = weak.clone();
        Button::new(id, label, environment)
            .tooltip(tip)
            .on_surface(palette.background)
            .availability(gate(on))
            .on_activate(move |_, cx| {
                let _ = weak.update(cx, |editor, cx| {
                    if let Some(id) = editor.lyrics.selected {
                        let _ = editor.move_section(id, step, cx);
                    }
                });
            })
    };
    let remove = {
        let weak = weak.clone();
        Button::new("arrangement-remove-section", REMOVE_SECTION, environment)
            .on_surface(palette.background)
            .availability(gate(true))
            .on_activate(move |window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |editor, cx| editor.request_remove(window, cx));
            })
    };
    div()
        .debug_selector(|| "arrangement-section-buttons".into())
        .flex()
        .flex_wrap()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(mover(
            "arrangement-move-up",
            "Move up",
            "Move up (Alt+Up)",
            Step::Up,
            at.is_some_and(|at| at > 0),
        ))
        .child(mover(
            "arrangement-move-down",
            "Move down",
            "Move down (Alt+Down)",
            Step::Down,
            at.is_some_and(|at| at < last),
        ))
        .child(remove)
        .into_any_element()
}
