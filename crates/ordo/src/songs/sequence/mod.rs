//! The Sequence task of the Arrangement editor: the performance order.
//!
//! State that belongs to one editor visit and is not content lives here: the
//! selected occurrence and slide, which occurrences are expanded, and the
//! sections the renderer could not compose. The words, names and order stay in
//! the one edit session, so every Sequence action is a structural edit of the
//! shared [`domain::song_library::ArrangementLyrics`]. [`readiness`] words and
//! draws what the editor header and the Sequence tab say about it.

pub mod actions;
pub mod available;
pub mod inspector;
pub mod occurrences;
pub mod readiness;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};

use domain::{
    identity::{OccurrenceId, SectionId},
    pagination::{slide_body, slides},
    song_library::{ArrangementLyrics, SectionDraft},
};
use gpui::{App, Context, FocusHandle, WeakEntity, Window};
use ui::environment::UiEnvironment;

use crate::songs::lyrics::{
    ArrangementEditor,
    navigator::{FrameKey, compose_frames},
    panes::{Panes, section_name},
};

/// The occurrence and slide the Sequence task and the Inspector show.
///
/// Both tabs share it for a visit. It is keyed by identity, never by position,
/// so it survives a reorder, a collapse and a tab switch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceSelection {
    /// The selected occurrence.
    pub occurrence: OccurrenceId,
    /// The selected slide inside the occurrence's section (0-based).
    pub slide: usize,
}

impl SequenceSelection {
    /// The occurrence itself, on its first slide.
    pub fn occurrence(occurrence: OccurrenceId) -> Self {
        Self {
            occurrence,
            slide: 0,
        }
    }
}

/// The three pane slots of the Sequence task: Available sections on the left,
/// the performance order in the middle and the Inspector on the right.
#[allow(clippy::too_many_arguments)]
pub fn panes(
    editor: &mut ArrangementEditor,
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    compact: bool,
    read_only: bool,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ArrangementEditor>,
) -> Panes {
    editor.sequence_selection = editor
        .sequence_selection
        .filter(|selection| content.occurrence_index(selection.occurrence).is_some());
    if !editor.sequence.shown {
        editor.sequence.shown = true;
        if content.sequence.is_empty() && !content.sections.is_empty() && !read_only {
            editor.sequence.focus_add_first = true;
        }
    }
    request_frames(editor, content, cx);
    let sections = available::pane(weak, content, read_only, environment);
    let main = occurrences::pane(
        editor,
        weak,
        content,
        compact,
        read_only,
        environment,
        window,
        cx,
    );
    Panes {
        sections: Some(sections),
        main,
        inspector: (!compact).then(|| inspector::pane(editor, content, environment)),
    }
}

/// What a composed slide of `section` is cached under, in slide order.
pub fn slide_keys(section: &SectionDraft) -> Vec<FrameKey> {
    slides(&section.lyrics)
        .iter()
        .map(|source| (section.label.clone(), slide_body(&section.lyrics, source)))
        .collect()
}

/// Starts composing what the Sequence shows: the selected slide and every
/// slide of an expanded occurrence. Repeated occurrences share one frame, and
/// nothing is composed on the GPUI thread.
fn request_frames(
    editor: &mut ArrangementEditor,
    content: &ArrangementLyrics,
    cx: &mut Context<ArrangementEditor>,
) {
    // The selected slide first, then every expanded slide in sung order: a
    // request cut at the cache's capacity keeps what matters most.
    let mut chosen: Option<FrameKey> = None;
    let mut wanted: Vec<FrameKey> = Vec::new();
    for occurrence in &content.sequence {
        let Some(section) = content
            .sections
            .iter()
            .find(|section| section.id == occurrence.section)
        else {
            continue;
        };
        let keys = slide_keys(section);
        if editor.expanded.contains(&occurrence.id) {
            wanted.extend(keys.iter().cloned());
        }
        if let Some(selection) = editor
            .sequence_selection
            .filter(|selection| selection.occurrence == occurrence.id)
            && let Some(key) = keys.get(selection.slide.min(keys.len().saturating_sub(1)))
        {
            chosen = Some(key.clone());
        }
    }
    let wanted: Vec<FrameKey> = chosen.into_iter().chain(wanted).collect();
    compose_frames(editor, wanted, cx);
}

/// The stops of the list in the order the operator walks them: each row, then
/// its slide tiles while it is expanded.
pub fn visible_items(content: &ArrangementLyrics, expanded: &BTreeSet<OccurrenceId>) -> Vec<Item> {
    let mut items = Vec::new();
    for occurrence in &content.sequence {
        items.push(Item::Row(occurrence.id));
        if expanded.contains(&occurrence.id)
            && let Some(section) = content
                .sections
                .iter()
                .find(|section| section.id == occurrence.section)
        {
            items.extend(
                (0..slides(&section.lyrics).len()).map(|slide| Item::Child(occurrence.id, slide)),
            );
        }
    }
    items
}

/// The name an occurrence shows: its section's label, or the placeholder.
pub fn occurrence_name(content: &ArrangementLyrics, section: SectionId) -> String {
    let at = content
        .sections
        .iter()
        .position(|candidate| candidate.id == section)
        .unwrap_or(usize::MAX);
    section_name(&content.sections, at)
}

/// One stop of the Sequence list that can own focus: an occurrence row, or
/// one of the slide tiles an expanded row shows.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Item {
    /// The occurrence row itself.
    Row(OccurrenceId),
    /// A slide tile of an expanded occurrence (0-based slide).
    Child(OccurrenceId, usize),
}

/// The Sequence task's own state inside one editor visit: focus handles and
/// the focus requests the next render applies. Nothing here is content.
pub struct SequenceView {
    /// One focus handle per occurrence row, so focus stays on a moved row.
    pub rows: HashMap<OccurrenceId, FocusHandle>,
    /// One focus handle per slide tile of an expanded row.
    pub kids: HashMap<(OccurrenceId, usize), FocusHandle>,
    /// Focus handle of **Add first occurrence**.
    pub add_first: FocusHandle,
    /// Focus **Add first occurrence** at the next render.
    pub focus_add_first: bool,
    /// The stop to focus at the next render.
    pub focus_item: Option<Item>,
    /// Whether the Sequence task was the one shown at the last render, so
    /// arriving on an empty Sequence can focus its action once.
    pub shown: bool,
    /// The stop that owned focus at the last render, if any. Selection
    /// follows focus when this changes, and a content replacement hands focus
    /// on only when a stop held it.
    pub last_focus: Option<Item>,
}

impl SequenceView {
    /// A fresh view for one visit.
    pub fn new(cx: &mut App) -> Self {
        Self {
            rows: HashMap::new(),
            kids: HashMap::new(),
            add_first: cx.focus_handle(),
            focus_add_first: false,
            focus_item: None,
            shown: false,
            last_focus: None,
        }
    }
}

/// A new occurrence identity, minted by the view as it mints section ones.
pub fn fresh_occurrence() -> OccurrenceId {
    OccurrenceId::from_entity(domain::identity::EntityId::from_random_bytes(
        *uuid::Uuid::new_v4().as_bytes(),
    ))
}
