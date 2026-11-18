//! The review's state and what the operator's decisions do to it.

use std::{collections::BTreeMap, path::PathBuf, rc::Rc};

use domain::{
    identity::SongId,
    song_library::import::{
        Choices, Classification, DocumentReview, Outcome, Review, SongChoice, SongIdentity, Staged,
        StagedDocument, can_commit, tally,
    },
};
use gpui::{Context, Entity, EventEmitter, FocusHandle, SharedString, Task, Window};
use ui::{
    accessibility::RegionRegistry,
    overlay::OverlayHost,
    primitives::{disclosure::DisclosureState, selection::SelectionState},
    scroll::Scroll,
};

use super::{
    compare::{ImportCompare, open_compare},
    picker::FilePicker,
};
use crate::songs::store::{ImportOutcome, ImportStaging, LibraryStore, STALE_IMPORT};

/// Why decisions, removal and Cancel import wait while a commit is in flight.
pub(super) const IMPORTING: &str = "Importing\u{2026}";
/// Why they wait while the documents are classified again.
pub(super) const UPDATING: &str = "Updating the review\u{2026}";

/// Where focus goes after the next draw: the selected document's row (a
/// document was removed or replaced, or the review opened), or Cancel import
/// (no document is left).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refocus {
    Row,
    Cancel,
}

/// What the review tells its owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportReviewEvent {
    /// The operator left without importing; nothing was written.
    Cancelled,
    /// The approved changes were written in one update.
    Committed(ImportOutcome),
}

/// Which control of which document a state belongs to: the Song identity
/// block (`None`) or one Arrangement.
pub(super) type ControlKey = (usize, Option<usize>);

/// The full-workspace review. Owned by the shell while open; dropping it
/// discards the staging.
pub struct ImportReview {
    pub(super) staging: ImportStaging,
    pub(super) choices: Choices,
    pub(super) selected: usize,
    pub(super) store: Entity<LibraryStore>,
    pub(super) picker: Rc<dyn FilePicker>,
    pub(super) regions: RegionRegistry,
    pub(super) list_scroll: Scroll,
    pub(super) detail_scroll: Scroll,
    /// The radio groups and selects, kept so a choice survives a redraw.
    /// Cleared whenever the staging changes under them.
    pub(super) controls: BTreeMap<ControlKey, SelectionState>,
    pub(super) technical: DisclosureState,
    pub(super) busy: bool,
    /// Whether the documents are being classified again (or a replacement
    /// file is being picked and read). Decisions wait until it is done.
    pub(super) restaging: bool,
    /// The selected document's row, so focus can follow a removal.
    pub(super) row_focus: FocusHandle,
    /// **Cancel import**, where focus goes when no document is left.
    pub(super) cancel_focus: FocusHandle,
    refocus: Option<Refocus>,
    /// The shell's overlay layer, which hosts Compare contents.
    overlays: Option<OverlayHost>,
    /// The open comparison and the id its host gave it, while it is open.
    compare: Option<(usize, Entity<ImportCompare>)>,
    pub(super) refusal: Option<SharedString>,
    /// Which restage or pick is current; an older answer is dropped.
    generation: u64,
    /// The in-flight commit, owned here so closing the review drops the
    /// interest. It never overlaps a restage: each waits for the other.
    commit_task: Option<Task<()>>,
    /// The in-flight restage or pick, owned here for the same reason.
    restage_task: Option<Task<()>>,
}

impl EventEmitter<ImportReviewEvent> for ImportReview {}

/// The first document that needs a decision, else the first.
fn initial_selection(reviews: &[Review]) -> usize {
    reviews
        .iter()
        .position(|review| match review {
            Review::Valid(review) => {
                matches!(review.identity, SongIdentity::NeedsDecision { .. })
                    || review
                        .arrangements
                        .iter()
                        .any(|a| a.classification == Classification::NeedsDecision)
            }
            Review::Invalid(_) => false,
        })
        .unwrap_or(0)
}

/// What the Arrangement does when nothing was chosen, by classification.
pub(super) fn default_outcome(classification: Classification) -> Option<Outcome> {
    match classification {
        Classification::AddNewSong => Some(Outcome::AddNewSong),
        Classification::AttachArrangement => Some(Outcome::Attach),
        Classification::SkipUnchanged => Some(Outcome::Skip),
        Classification::NeedsDecision => None,
    }
}

impl ImportReview {
    /// Opens the review over a staged batch. Focus starts on the selected
    /// document's row.
    pub fn new(
        staging: ImportStaging,
        store: Entity<LibraryStore>,
        picker: Rc<dyn FilePicker>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut regions = RegionRegistry::default();
        regions.focus_region("Primary list");
        Self {
            selected: initial_selection(&staging.reviews),
            staging,
            choices: Choices::default(),
            store,
            picker,
            regions,
            list_scroll: Scroll::new(),
            detail_scroll: Scroll::new(),
            controls: BTreeMap::new(),
            technical: DisclosureState::new(cx),
            busy: false,
            restaging: false,
            row_focus: cx.focus_handle().tab_stop(true),
            cancel_focus: cx.focus_handle().tab_stop(true),
            refocus: Some(Refocus::Row),
            overlays: None,
            compare: None,
            refusal: None,
            generation: 0,
            commit_task: None,
            restage_task: None,
        }
    }

    /// Hands the review the shell's overlay layer, which hosts **Compare
    /// contents**. Without it the comparison cannot open.
    pub fn set_overlays(&mut self, overlays: OverlayHost) {
        self.overlays = Some(overlays);
    }

    /// The open comparison, while it is open.
    pub fn compare(&self) -> Option<&Entity<ImportCompare>> {
        self.compare.as_ref().map(|(_, compare)| compare)
    }

    /// The id the host gave the open comparison, while it is open.
    pub fn compare_id(&self) -> Option<usize> {
        self.compare.as_ref().map(|(id, _)| *id)
    }

    /// **Compare contents** / **Preview document**: opens the read-only
    /// comparison of Arrangement `arrangement` of document `document`. One at
    /// a time; the review's choices, selection and scroll are not touched.
    pub(super) fn open_compare(
        &mut self,
        document: usize,
        arrangement: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(host), None, None) = (self.overlays.clone(), &self.compare, self.held()) else {
            return;
        };
        let weak = cx.weak_entity();
        let closed = move |_: &mut Window, cx: &mut gpui::App| {
            let _ = weak.update(cx, |review, cx| review.compare_closed(cx));
        };
        let opened = open_compare(self, document, arrangement, host, closed, window, cx);
        let Some((id, compare)) = opened else {
            return;
        };
        self.compare = Some((id, compare));
        cx.notify();
    }

    /// Lets go of the comparison once its surface has closed, however it was
    /// dismissed.
    pub(super) fn compare_closed(&mut self, cx: &mut Context<Self>) {
        self.compare = None;
        cx.notify();
    }

    /// The index of the document on the right.
    pub fn selected_document(&self) -> usize {
        self.selected
    }

    /// Whether **Import approved** can be taken: every decision is made,
    /// something would change, and no commit is in flight.
    pub fn can_commit(&self) -> bool {
        self.held().is_none() && can_commit(&self.staging.reviews, &self.choices)
    }

    /// The staged documents, in file order.
    pub fn documents(&self) -> &[StagedDocument] {
        &self.staging.documents
    }

    /// The decisions made so far.
    pub fn choices(&self) -> &Choices {
        &self.choices
    }

    /// The reason the last commit was refused, while the footer shows it.
    pub fn refusal(&self) -> Option<&SharedString> {
        self.refusal.as_ref()
    }

    /// Whether a commit is in flight.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Whether the documents are being classified again.
    pub fn restaging(&self) -> bool {
        self.restaging
    }

    /// Why decisions, removal, Compare and Cancel import wait right now, or
    /// `None` when the review is at rest.
    pub(super) fn held(&self) -> Option<&'static str> {
        if self.busy {
            Some(IMPORTING)
        } else if self.restaging {
            Some(UPDATING)
        } else {
            None
        }
    }

    /// Takes the focus the last change asked for.
    pub(super) fn take_refocus(&mut self) -> Option<FocusHandle> {
        self.refocus.take().map(|target| match target {
            Refocus::Row => self.row_focus.clone(),
            Refocus::Cancel => self.cancel_focus.clone(),
        })
    }

    pub(super) fn review_of(&self, index: usize) -> Option<&DocumentReview> {
        match self.staging.reviews.get(index) {
            Some(Review::Valid(review)) => Some(review),
            _ => None,
        }
    }

    pub(super) fn tally(&self) -> domain::song_library::import::Tally {
        tally(&self.staging.reviews, &self.choices)
    }

    /// A handler for a control, run against this entity.
    pub(super) fn act(
        cx: &Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl Fn(&mut Window, &mut gpui::App) + 'static {
        let weak = cx.weak_entity();
        move |window, cx| {
            let _ = weak.update(cx, |review, cx| run(review, window, cx));
        }
    }

    /// Shows document `index` on the right.
    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.restaging && index < self.staging.documents.len() && index != self.selected {
            self.selected = index;
            self.technical.set_expanded(false, cx);
            cx.notify();
        }
    }

    /// The control state for `key`, made on first use from `choices`.
    pub(super) fn control(
        &mut self,
        key: ControlKey,
        choices: impl FnOnce() -> (Vec<ui::primitives::selection::Choice>, Option<usize>),
        cx: &mut Context<Self>,
    ) -> SelectionState {
        if let Some(state) = self.controls.get(&key) {
            return state.clone();
        }
        let (choices, selected) = choices();
        let state = SelectionState::new(choices, selected, cx);
        self.controls.insert(key, state.clone());
        state
    }

    /// Chooses outcome `index` of those the Arrangement offers.
    pub(super) fn choose_outcome(
        &mut self,
        document: usize,
        arrangement: usize,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.held().is_some() {
            return;
        }
        let outcome = self
            .review_of(document)
            .and_then(|review| review.arrangements.get(arrangement))
            .and_then(|review| review.offered.get(index).copied());
        if let Some(outcome) = outcome {
            self.choices
                .arrangements
                .insert((document, arrangement), outcome);
            self.refusal = None;
            cx.notify();
        }
    }

    /// Chooses the Song for a document that needs a decision; its
    /// Arrangements are classified again against that Song.
    pub(super) fn choose_song(
        &mut self,
        document: usize,
        choice: SongChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.held().is_some() {
            return;
        }
        self.choices.songs.insert(document, choice);
        self.choices
            .arrangements
            .retain(|(owner, _), _| *owner != document);
        self.controls
            .retain(|(owner, arrangement), _| *owner != document || arrangement.is_none());
        self.refusal = None;
        let documents = self.staging.documents.clone();
        self.restage(documents, window, cx);
    }

    /// The Song a candidate index of the identity block stands for.
    pub(super) fn song_choice_at(candidates: &[SongId], index: usize) -> Option<SongChoice> {
        match index.checked_sub(candidates.len()) {
            None => candidates.get(index).copied().map(SongChoice::Existing),
            Some(0) => Some(SongChoice::New),
            Some(1) => Some(SongChoice::Skip),
            Some(_) => None,
        }
    }

    /// **Remove from review**: drops the document and classifies the rest
    /// again. Refused while a commit or a restage is in flight, so it never
    /// works from a document list that is about to be replaced.
    pub(super) fn remove(&mut self, document: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.held().is_some() || document >= self.staging.documents.len() {
            return;
        }
        let mut documents = self.staging.documents.clone();
        documents.remove(document);
        self.remap(|index| match index.cmp(&document) {
            std::cmp::Ordering::Less => Some(index),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(index - 1),
        });
        self.selected = self.selected.min(documents.len().saturating_sub(1));
        if self.selected > document {
            self.selected -= 1;
        }
        self.refusal = None;
        self.refocus = Some(Refocus::Row);
        self.restage(documents, window, cx);
    }

    /// Moves every decision to where its document now stands; `map` gives the
    /// new index of an old one, or `None` when the document is gone.
    fn remap(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.choices.songs = std::mem::take(&mut self.choices.songs)
            .into_iter()
            .filter_map(|(index, choice)| map(index).map(|index| (index, choice)))
            .collect();
        self.choices.arrangements = std::mem::take(&mut self.choices.arrangements)
            .into_iter()
            .filter_map(|((index, at), outcome)| map(index).map(|index| ((index, at), outcome)))
            .collect();
        self.controls.clear();
    }

    /// **Choose another file**: asks for one file and, when one is picked,
    /// replaces document `document` with it.
    pub(super) fn choose_another(
        &mut self,
        document: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.held().is_some() {
            return;
        }
        self.generation += 1;
        let generation = self.generation;
        // The pick is part of the restage: decisions wait for it too.
        self.restaging = true;
        let pick = self.picker.pick(false, cx);
        let weak = cx.weak_entity();
        self.restage_task = Some(window.spawn(cx, async move |cx| {
            let paths = pick.await;
            cx.update(|window, cx| {
                let _ = weak.update(cx, |review, cx| {
                    review.picked(generation, document, paths, window, cx);
                });
            })
            .ok();
        }));
        cx.notify();
    }

    fn picked(
        &mut self,
        generation: u64,
        document: usize,
        paths: Option<Vec<PathBuf>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation {
            return;
        }
        let Some(path) = paths.and_then(|paths| paths.into_iter().next()) else {
            self.restaging = false;
            self.restage_task = None;
            cx.notify();
            return;
        };
        let staged = self
            .store
            .update(cx, |store, cx| store.stage_import(vec![path], cx));
        let weak = cx.weak_entity();
        self.restage_task = Some(window.spawn(cx, async move |cx| {
            let staging = staged.await;
            cx.update(|window, cx| {
                let _ = weak.update(cx, |review, cx| {
                    review.replace(generation, document, staging, window, cx);
                });
            })
            .ok();
        }));
    }

    fn replace(
        &mut self,
        generation: u64,
        document: usize,
        staging: ImportStaging,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation {
            return;
        }
        let new = staging.documents.into_iter().next();
        let (Some(new), true) = (new, document < self.staging.documents.len()) else {
            self.restaging = false;
            self.restage_task = None;
            cx.notify();
            return;
        };
        // The list stays in file order, as a fresh staging would have it, so
        // the replacement lands where its name sorts; every other decision
        // follows its document.
        let mut tagged: Vec<(Option<usize>, StagedDocument)> = self
            .staging
            .documents
            .iter()
            .cloned()
            .enumerate()
            .filter(|(index, _)| *index != document)
            .map(|(index, document)| (Some(index), document))
            .collect();
        tagged.push((None, new));
        tagged.sort_by(|a, b| a.1.source.cmp(&b.1.source));
        let mut forward = vec![None; self.staging.documents.len()];
        let mut landed = 0;
        for (at, (old, _)) in tagged.iter().enumerate() {
            match old {
                Some(old) => forward[*old] = Some(at),
                None => landed = at,
            }
        }
        self.remap(|index| forward.get(index).copied().flatten());
        self.selected = landed;
        let documents = tagged.into_iter().map(|(_, document)| document).collect();
        self.refusal = None;
        self.refocus = Some(Refocus::Row);
        self.restage(documents, window, cx);
    }

    /// Classifies `documents` again off the GPUI thread. Only the newest
    /// answer is applied; decisions wait until it is.
    fn restage(
        &mut self,
        documents: Vec<StagedDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.generation += 1;
        let generation = self.generation;
        self.restaging = true;
        let songs = self.choices.songs.clone();
        let task = self
            .store
            .update(cx, |store, cx| store.reclassify(documents, songs, cx));
        let weak = cx.weak_entity();
        self.restage_task = Some(window.spawn(cx, async move |cx| {
            let staging = task.await;
            cx.update(|_, cx| {
                let _ = weak.update(cx, |review, cx| review.apply(generation, staging, cx));
            })
            .ok();
        }));
        cx.notify();
    }

    fn apply(&mut self, generation: u64, staging: ImportStaging, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.staging = staging;
        let reviews = &self.staging.reviews;
        self.choices.songs.retain(|index, _| *index < reviews.len());
        // A choice the new review no longer offers is no choice.
        self.choices
            .arrangements
            .retain(|(document, at), outcome| match reviews.get(*document) {
                Some(Review::Valid(review)) => review
                    .arrangements
                    .get(*at)
                    .is_some_and(|review| review.offered.contains(outcome)),
                _ => false,
            });
        self.controls
            .retain(|(_, arrangement), _| arrangement.is_none());
        if self.selected >= self.staging.documents.len() {
            self.selected = self.staging.documents.len().saturating_sub(1);
        }
        if self.staging.documents.is_empty() {
            self.refocus = Some(Refocus::Cancel);
        }
        self.restaging = false;
        self.restage_task = None;
        cx.notify();
    }

    /// Closes the surface this review opened (the comparison), if any. Run
    /// from outside the review's own update: closing hands focus back and
    /// calls `compare_closed`, which updates the review.
    pub(super) fn close_surfaces(
        weak: &gpui::WeakEntity<Self>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) {
        let open = weak.read_with(cx, |review, _| (review.overlays.clone(), review.compare_id()));
        if let Ok((Some(host), Some(id))) = open {
            host.close(id, window, cx);
        }
    }

    /// **Import approved**: writes every approved change in one update.
    ///
    /// A refusal stays in the footer. When it says the Library moved on, the
    /// documents are classified again at once, so the operator re-decides
    /// against what is there now; any other refusal leaves the review as it
    /// was.
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_commit() {
            return;
        }
        self.busy = true;
        self.refusal = None;
        let task = self.store.update(cx, |store, cx| {
            store.commit_import(&self.staging, self.choices.clone(), cx)
        });
        let weak = cx.weak_entity();
        self.commit_task = Some(window.spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|window, cx| {
                Self::close_surfaces(&weak, window, cx);
                let _ = weak.update(cx, |review, cx| {
                    review.busy = false;
                    review.commit_task = None;
                    match result {
                        Ok(outcome) => cx.emit(ImportReviewEvent::Committed(outcome)),
                        Err(message) => {
                            let stale = message.as_ref() == STALE_IMPORT;
                            review.refusal = Some(message);
                            if stale {
                                let documents = review.staging.documents.clone();
                                review.restage(documents, window, cx);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .ok();
        }));
        cx.notify();
    }

    /// **Cancel import**: discards the staging. Nothing has been written, so
    /// nothing is asked. Refused while a commit or a restage is in flight.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.held().is_some() {
            return;
        }
        cx.emit(ImportReviewEvent::Cancelled);
    }

    /// Whether document `index` is a valid Ordo document (an invalid file has
    /// nothing to decide).
    pub(super) fn is_staged_valid(&self, index: usize) -> bool {
        matches!(
            self.staging.documents.get(index).map(|d| &d.content),
            Some(Staged::Valid(_))
        )
    }
}
