//! The Lyrics task's own state inside one editor visit: which section is
//! selected, whether the Add section popover is open, and the focus requests
//! the next render applies. Nothing here is content: the words, names and
//! order live in the edit session.

use std::{cell::Cell, collections::HashMap, rc::Rc};

use domain::{identity::SectionId, song_library::ArrangementLyrics};
use gpui::{App, Bounds, FocusHandle, Pixels, Subscription, Task};
use rendering::FrameCache;
use ui::{
    overlay::OverlayHost,
    primitives::{field::FieldState, text_area::TextAreaState},
};

/// The popover that names a new section.
pub(super) struct AddSection {
    /// The name being typed, prefilled with the suggestion.
    pub name: FieldState,
}

/// Where an Add section button sat when it was last painted, so the popover
/// can anchor to it.
#[derive(Clone, Default)]
pub(super) struct Anchor(pub Rc<Cell<Bounds<Pixels>>>);

/// State of the Lyrics task for one visit.
pub(super) struct LyricsView {
    /// The shell's overlay layer.
    pub overlays: Option<OverlayHost>,
    /// The section the main pane edits.
    pub selected: Option<SectionId>,
    /// The Add section popover, while open.
    pub add: Option<AddSection>,
    /// Focus handle of **Add first section**.
    pub add_first: FocusHandle,
    /// Focus **Add first section** at the next render.
    pub focus_add_first: bool,
    /// One focus handle per section row, so focus stays on a moved row.
    pub rows: HashMap<SectionId, FocusHandle>,
    /// Where the head's **Add section** button sits.
    pub head_anchor: Anchor,
    /// Where **Add first section** sits.
    pub empty_anchor: Anchor,
    /// Set when the session replaced the content (Undo, Redo, leave): views
    /// reload their fields at the next render.
    pub reload: bool,
    /// Set when the lyric editor should take focus at the next render.
    pub focus_area: bool,
    /// The lyric editor, made once the first section is shown.
    pub area: Option<TextAreaState>,
    /// The section whose lyrics the editor holds now.
    pub shown: Option<SectionId>,
    /// The slide the caret is on (0-based).
    pub slide: usize,
    /// The slide to put the caret on when the editor next reloads.
    pub reload_slide: Option<usize>,
    /// Keeps the editor's event and caret subscriptions alive.
    pub subscriptions: Vec<Subscription>,
    /// Composed slide frames, by (section name, slide body).
    pub frames: FrameCache<super::navigator::FrameKey>,
    /// Compositions in flight; dropping one cancels it.
    pub pending: HashMap<super::navigator::FrameKey, Task<()>>,
    /// The rename field of the selected section.
    pub rename: Option<super::section_actions::Rename>,
    /// A section whose row takes focus after a move.
    pub focus_row: Option<SectionId>,
}

impl LyricsView {
    /// A fresh view for one visit.
    pub fn new(cx: &mut App) -> Self {
        Self {
            overlays: None,
            selected: None,
            add: None,
            add_first: cx.focus_handle(),
            focus_add_first: false,
            rows: HashMap::new(),
            head_anchor: Anchor::default(),
            empty_anchor: Anchor::default(),
            reload: false,
            focus_area: false,
            area: None,
            shown: None,
            slide: 0,
            reload_slide: None,
            subscriptions: Vec::new(),
            frames: FrameCache::new(rendering::FRAME_CAPACITY),
            pending: HashMap::new(),
            rename: None,
            focus_row: None,
        }
    }

    /// The session replaced the working content.
    pub fn content_replaced(&mut self) {
        self.reload = true;
    }

    /// Keeps the selection on an existing section: the selected one, else the
    /// first. Returns the section's position.
    pub fn resolve_selection(&mut self, content: &ArrangementLyrics) -> Option<usize> {
        let found = self
            .selected
            .and_then(|id| content.sections.iter().position(|section| section.id == id));
        let at = found.or((!content.sections.is_empty()).then_some(0));
        self.selected = at.map(|at| content.sections[at].id);
        at
    }
}
