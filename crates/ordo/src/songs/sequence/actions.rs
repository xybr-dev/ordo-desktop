//! What the Sequence task does to the content: add, repeat, move and remove
//! an occurrence, each one structural edit of the shared session, and the
//! selection and disclosure the list keeps beside it.
//!
//! The domain decides every position and refusal. A refused edit is shown as
//! a toast; a domain error is a programming error (the view only offers
//! occurrences that exist), so it is logged and never shown.

use domain::{
    identity::{OccurrenceId, SectionId},
    pagination::slides,
    revisions::EditPoint,
    song_library::{ArrangementLyrics, SongLibraryError, Step, position_label},
};
use gpui::Context;

use super::{Item, SequenceSelection, fresh_occurrence, occurrence_name, visible_items};
use crate::songs::lyrics::ArrangementEditor;

impl ArrangementEditor {
    /// The selected occurrence, when it still exists.
    pub fn selected_occurrence(&self, content: &ArrangementLyrics) -> Option<OccurrenceId> {
        self.sequence_selection
            .map(|selection| selection.occurrence)
            .filter(|id| content.occurrence_index(*id).is_some())
    }

    /// One structural edit of the Sequence. Returns what `change` returned
    /// when the edit was accepted. A refused edit (a failed save, or an Undo
    /// still queued) posts the refusal; a domain error is logged.
    fn structural_sequence<T>(
        &mut self,
        change: impl FnOnce(&mut ArrangementLyrics) -> Result<T, SongLibraryError>,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        let session = self.session().cloned()?;
        self.dismiss_last_toast(cx);
        // Typed text joins the session before the structural edit starts
        // from it.
        self.sync_text(cx);
        let mut ran = false;
        let mut outcome = None;
        let applied = session.update(cx, |session, cx| {
            session.structural(
                EditPoint::Structural,
                |content| {
                    ran = true;
                    match change(content) {
                        Ok(value) => outcome = Some(value),
                        Err(error) => eprintln!("sequence edit refused by the domain: {error:?}"),
                    }
                },
                cx,
            )
        });
        if applied {
            return outcome;
        }
        if !ran {
            let copy = self.refusal(cx).to_owned();
            self.notify_refusal::<()>(Err(copy), cx);
        }
        None
    }

    /// Adds an occurrence of `section` after the selected occurrence, or at
    /// the end when none is selected, and selects it. `focus` also moves
    /// focus to the new row.
    pub fn add_occurrence(&mut self, section: SectionId, focus: bool, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        let after = self.selected_occurrence(&content);
        let id = fresh_occurrence();
        let Some(index) = self.structural_sequence(|c| c.insert_occurrence(id, section, after), cx)
        else {
            return;
        };
        self.placed(id, index, &content, section, focus, cx);
    }

    /// Adds another occurrence of the selected occurrence's section right
    /// after it, and selects the new one.
    pub fn repeat_occurrence(&mut self, source: OccurrenceId, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        let Some(section) = content
            .occurrence_index(source)
            .map(|at| content.sequence[at].section)
        else {
            return;
        };
        let id = fresh_occurrence();
        let Some(index) = self.structural_sequence(|c| c.repeat_occurrence(id, source), cx) else {
            return;
        };
        self.placed(id, index, &content, section, false, cx);
    }

    /// Selects and announces an occurrence that was just inserted.
    fn placed(
        &mut self,
        id: OccurrenceId,
        index: usize,
        content: &ArrangementLyrics,
        section: SectionId,
        focus: bool,
        cx: &mut Context<Self>,
    ) {
        self.sequence_selection = Some(SequenceSelection::occurrence(id));
        if focus {
            self.sequence.focus_item = Some(Item::Row(id));
        }
        let name = occurrence_name(content, section);
        self.notify_undoable(
            format!("Added {name} at {} in the Sequence.", position_label(index)),
            cx,
        );
        cx.notify();
    }

    /// Moves a whole occurrence one place. A move past an edge writes nothing
    /// and says nothing. Focus and selection stay on the moved row.
    pub fn move_occurrence_by(&mut self, id: OccurrenceId, step: Step, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        let mut probe = content.clone();
        match probe.move_occurrence(id, step) {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                eprintln!("sequence move refused by the domain: {error:?}");
                return;
            }
        }
        if self
            .structural_sequence(|c| c.move_occurrence(id, step), cx)
            .is_none()
        {
            return;
        }
        self.moved(id, &probe, cx);
    }

    /// Moves a whole occurrence to `index`, counted in the list without it
    /// (a drop). Dropping where it already is writes nothing.
    pub fn move_occurrence_to(&mut self, id: OccurrenceId, index: usize, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        let mut probe = content.clone();
        match probe.move_occurrence_to(id, index) {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                eprintln!("sequence drop refused by the domain: {error:?}");
                return;
            }
        }
        if self
            .structural_sequence(|c| c.move_occurrence_to(id, index), cx)
            .is_none()
        {
            return;
        }
        self.moved(id, &probe, cx);
    }

    /// Selects, focuses and announces a moved occurrence; `after` is the
    /// content as the move left it.
    fn moved(&mut self, id: OccurrenceId, after: &ArrangementLyrics, cx: &mut Context<Self>) {
        let Some(index) = after.occurrence_index(id) else {
            return;
        };
        let name = occurrence_name(after, after.sequence[index].section);
        if self.selected_occurrence(after) != Some(id) {
            self.sequence_selection = Some(SequenceSelection::occurrence(id));
        }
        self.sequence.focus_item = Some(Item::Row(id));
        self.notify_undoable(format!("Moved {name} to {}.", position_label(index)), cx);
        cx.notify();
    }

    /// Removes an occurrence (never its section) and selects and focuses the
    /// next one, else the previous, else **Add first occurrence**.
    pub fn remove_occurrence(&mut self, id: OccurrenceId, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        let Some(index) = content.occurrence_index(id) else {
            return;
        };
        let name = occurrence_name(&content, content.sequence[index].section);
        let Some(next) = self.structural_sequence(|c| c.remove_occurrence(id), cx) else {
            return;
        };
        self.expanded.remove(&id);
        match next {
            Some(next) => {
                self.sequence_selection = Some(SequenceSelection::occurrence(next));
                self.sequence.focus_item = Some(Item::Row(next));
            }
            None => {
                self.sequence_selection = None;
                self.sequence.focus_add_first = true;
            }
        }
        self.notify_undoable(
            format!(
                "Removed {} {name} from the Sequence. The section stays in Lyrics.",
                position_label(index)
            ),
            cx,
        );
        cx.notify();
    }

    /// Selects an occurrence and slide; the selection is never content.
    pub fn select_occurrence(&mut self, id: OccurrenceId, slide: usize, cx: &mut Context<Self>) {
        let next = SequenceSelection {
            occurrence: id,
            slide,
        };
        if self.sequence_selection != Some(next) {
            self.sequence_selection = Some(next);
            cx.notify();
        }
    }

    /// Clicking an occurrence's name selects its first slide, unless it is
    /// already the selected occurrence.
    pub fn click_occurrence(&mut self, id: OccurrenceId, cx: &mut Context<Self>) {
        if self
            .sequence_selection
            .map(|selection| selection.occurrence)
            != Some(id)
        {
            self.select_occurrence(id, 0, cx);
        }
    }

    /// Shows or hides one occurrence's slides. Selection is untouched.
    pub fn toggle_expanded(&mut self, id: OccurrenceId, cx: &mut Context<Self>) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        cx.notify();
    }

    /// Whether every occurrence of a non-empty Sequence is expanded.
    pub fn all_expanded(&self, content: &ArrangementLyrics) -> bool {
        !content.sequence.is_empty()
            && content
                .sequence
                .iter()
                .all(|occurrence| self.expanded.contains(&occurrence.id))
    }

    /// **Expand all** when any row is collapsed, else **Collapse all**.
    /// Disclosure only: selection and content are untouched.
    pub fn toggle_all_expanded(&mut self, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        if self.all_expanded(&content) {
            self.expanded.clear();
        } else {
            self.expanded
                .extend(content.sequence.iter().map(|occurrence| occurrence.id));
        }
        cx.notify();
    }

    /// Up and Down walk the visible stops in flattened order with the
    /// selection following focus; Right expands (or steps into the slides);
    /// Left collapses or steps to the parent row. Returns whether the key was
    /// one of these.
    pub fn walk(&mut self, from: Item, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(content) = self.content(cx) else {
            return false;
        };
        let items = visible_items(&content, &self.expanded);
        let Some(at) = items.iter().position(|item| *item == from) else {
            return false;
        };
        let target = match (key, from) {
            ("down", _) => items.get(at + 1).copied(),
            ("up", _) => at.checked_sub(1).and_then(|at| items.get(at)).copied(),
            ("right", Item::Row(id)) => {
                if self.expanded.insert(id) {
                    cx.notify();
                    None
                } else {
                    items
                        .get(at + 1)
                        .copied()
                        .filter(|next| matches!(next, Item::Child(owner, _) if *owner == id))
                }
            }
            ("left", Item::Row(id)) => {
                if self.expanded.remove(&id) {
                    cx.notify();
                }
                None
            }
            ("left", Item::Child(id, _)) => Some(Item::Row(id)),
            ("right", Item::Child(..)) => None,
            _ => return false,
        };
        if let Some(target) = target {
            self.go_to(target, cx);
        }
        true
    }

    /// Selects and focuses a stop.
    pub fn go_to(&mut self, item: Item, cx: &mut Context<Self>) {
        let (id, slide) = match item {
            Item::Row(id) => (id, 0),
            Item::Child(id, slide) => (id, slide),
        };
        self.sequence_selection = Some(SequenceSelection {
            occurrence: id,
            slide,
        });
        self.sequence.focus_item = Some(item);
        cx.notify();
    }

    /// How many slides an occurrence's section generates.
    pub fn slides_of(&self, content: &ArrangementLyrics, id: OccurrenceId) -> usize {
        content
            .occurrence_index(id)
            .and_then(|at| {
                content
                    .sections
                    .iter()
                    .find(|section| section.id == content.sequence[at].section)
            })
            .map_or(0, |section| slides(&section.lyrics).len())
    }
}
