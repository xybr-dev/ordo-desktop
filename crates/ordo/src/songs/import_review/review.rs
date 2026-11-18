//! The review's state and what the operator's decisions do to it.

use std::{collections::BTreeMap, path::PathBuf, rc::Rc};

use domain::{
    identity::SongId,
    song_library::import::{
        Choices, Classification, DocumentReview, Outcome, Review, SongChoice, SongIdentity, Staged,
        StagedDocument, can_commit, tally,
    },
};
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString, Task, Window};
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
use crate::songs::store::{ImportOutcome, ImportStaging, LibraryStore};

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
    /// The shell's overlay layer, which hosts Compare contents.
    overlays: Option<OverlayHost>,
    /// The open comparison and the id its host gave it, while it is open.
    compare: Option<(usize, Entity<ImportCompare>)>,
    pub(super) refusal: Option<SharedString>,
    /// Which restage or pick is current; an older answer is dropped.
    generation: u64,
    /// The in-flight restage, pick or commit, owned here so closing the
    /// review drops the interest.
    pending: Option<Task<()>>,
    focus: FocusHandle,
}

impl EventEmitter<ImportReviewEvent> for ImportReview {}

impl Focusable for ImportReview {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

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
    /// Opens the review over a staged batch. Focus starts in the document
    /// list.
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
            overlays: None,
            compare: None,
            refusal: None,
            generation: 0,
            pending: None,
            focus: cx.focus_handle(),
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
        let (Some(host), None) = (self.overlays.clone(), &self.compare) else {
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
        !self.busy && can_commit(&self.staging.reviews, &self.choices)
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
        if index < self.staging.documents.len() && index != self.selected {
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
    pub(super) fn song_choice_at(
        &self,
        document: usize,
        candidates: &[SongId],
        index: usize,
    ) -> Option<SongChoice> {
        let _ = document;
        match index.checked_sub(candidates.len()) {
            None => candidates.get(index).copied().map(SongChoice::Existing),
            Some(0) => Some(SongChoice::New),
            Some(1) => Some(SongChoice::Skip),
            Some(_) => None,
        }
    }

    /// **Remove from review**: drops the document and classifies the rest
    /// again.
    pub(super) fn remove(&mut self, document: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || document >= self.staging.documents.len() {
            return;
        }
        let mut documents = self.staging.documents.clone();
        documents.remove(document);
        let shift = |index: usize| if index > document { index - 1 } else { index };
        self.choices.songs = std::mem::take(&mut self.choices.songs)
            .into_iter()
            .filter(|(index, _)| *index != document)
            .map(|(index, choice)| (shift(index), choice))
            .collect();
        self.choices.arrangements = std::mem::take(&mut self.choices.arrangements)
            .into_iter()
            .filter(|((index, _), _)| *index != document)
            .map(|((index, at), outcome)| ((shift(index), at), outcome))
            .collect();
        self.controls.clear();
        self.selected = self.selected.min(documents.len().saturating_sub(1));
        if self.selected > document {
            self.selected -= 1;
        }
        self.refusal = None;
        self.restage(documents, window, cx);
    }

    /// **Choose another file**: asks for one file and, when one is picked,
    /// replaces document `document` with it.
    pub(super) fn choose_another(
        &mut self,
        document: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.generation += 1;
        let generation = self.generation;
        let pick = self.picker.pick(false, cx);
        let weak = cx.weak_entity();
        self.pending = Some(window.spawn(cx, async move |cx| {
            let paths = pick.await;
            cx.update(|window, cx| {
                let _ = weak.update(cx, |review, cx| {
                    review.picked(generation, document, paths, window, cx);
                });
            })
            .ok();
        }));
    }

    fn picked(
        &mut self,
        generation: u64,
        document: usize,
        paths: Option<Vec<PathBuf>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = paths.and_then(|paths| paths.into_iter().next()) else {
            return;
        };
        if generation != self.generation {
            return;
        }
        let staged = self
            .store
            .update(cx, |store, cx| store.stage_import(vec![path], cx));
        let weak = cx.weak_entity();
        self.pending = Some(window.spawn(cx, async move |cx| {
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
        let Some(new) = staging.documents.into_iter().next() else {
            return;
        };
        if generation != self.generation || document >= self.staging.documents.len() {
            return;
        }
        let mut documents = self.staging.documents.clone();
        documents[document] = new;
        self.choices.songs.remove(&document);
        self.choices
            .arrangements
            .retain(|(owner, _), _| *owner != document);
        self.controls.retain(|(owner, _), _| *owner != document);
        self.refusal = None;
        self.restage(documents, window, cx);
    }

    /// Classifies `documents` again off the GPUI thread. Only the newest
    /// answer is applied.
    fn restage(
        &mut self,
        documents: Vec<StagedDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.generation += 1;
        let generation = self.generation;
        let songs = self.choices.songs.clone();
        let task = self
            .store
            .update(cx, |store, cx| store.reclassify(documents, songs, cx));
        let weak = cx.weak_entity();
        self.pending = Some(window.spawn(cx, async move |cx| {
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
        self.pending = None;
        cx.notify();
    }

    /// **Import approved**: writes every approved change in one update.
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
        self.pending = Some(window.spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|_, cx| {
                let _ = weak.update(cx, |review, cx| {
                    review.busy = false;
                    match result {
                        Ok(outcome) => cx.emit(ImportReviewEvent::Committed(outcome)),
                        Err(message) => review.refusal = Some(message),
                    }
                    cx.notify();
                });
            })
            .ok();
        }));
        cx.notify();
    }

    /// **Cancel import**: discards the staging. Nothing has been written, so
    /// nothing is asked.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        cx.emit(ImportReviewEvent::Cancelled);
    }

    /// Whether document `index` is one the operator has not yet decided.
    pub(super) fn is_staged_valid(&self, index: usize) -> bool {
        matches!(
            self.staging.documents.get(index).map(|d| &d.content),
            Some(Staged::Valid(_))
        )
    }
}
