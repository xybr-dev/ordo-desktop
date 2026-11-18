//! The operator's choices become one plan the storage commit carries out
//! (Decisions 3, 5, 7, 10, 12, 13 and 23).
//!
//! Planning is pure. Identities the plan mints come from the caller's `mint`
//! closure; the plan never changes the Library, and the commit re-checks
//! everything it carries.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::identity::{ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId};
use crate::song_library::{
    ArrangementDraft, ChoirDraft, HydrateArrangement, HydrateChoir, HydrateSong, OccurrenceDraft,
    PaginationState, Section, SectionDraft, SongDraft, SongLibrary, match_key,
};

use super::classify::{
    ChoirLookup, Classification, DocumentReview, ResolvedSong, Review, SongIdentity, Staged,
    StagedDocument, classify_arrangements, classify_batch, lookup_choir,
};
use super::evidence::pair_sections;
use super::{
    ArrangementEvidence, ImportArrangement, ImportDocument, ImportSection, MarkdownDraft,
    SequenceEntry,
};

/// What the operator chose for one imported Arrangement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Create the Song and this Arrangement under it.
    AddNewSong,
    /// Add the Arrangement to the existing Song.
    Attach,
    /// Add it as a new Arrangement although a resembling one exists.
    AddAsNew,
    /// Replace the matched Arrangement's whole content (Decision 136).
    UpdateExisting,
    /// Add the same content as another Arrangement of the Song (offered on
    /// Skip unchanged).
    AttachAsAnother,
    /// Write nothing for it.
    Skip,
}

impl Outcome {
    /// Whether the outcome writes a new Arrangement.
    const fn adds_arrangement(self) -> bool {
        matches!(
            self,
            Self::AddNewSong | Self::Attach | Self::AddAsNew | Self::AttachAsAnother
        )
    }
}

/// What the operator chose for one document's Song.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SongChoice {
    /// Attach to this Library Song.
    Existing(SongId),
    /// Add the document's Song as a new Song.
    New,
    /// Skip the whole document.
    Skip,
}

/// Every choice made so far. A document or Arrangement with no entry takes
/// its classification's default, or stays an open decision when the
/// classification needs one.
///
/// After changing a document's Song choice, take the reviews from
/// [`review_with_choices`] (with `songs`) so [`tally`] and [`can_commit`] see
/// the new Song; the commit compares its reviews with the same function, and
/// [`plan`] re-classifies on its own.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Choices {
    /// Song choices by document index.
    pub songs: BTreeMap<usize, SongChoice>,
    /// Outcomes by (document index, Arrangement index).
    pub arrangements: BTreeMap<(usize, usize), Outcome>,
}

/// The footer summary of a review.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Tally {
    /// Songs the commit creates.
    pub new_songs: usize,
    /// Arrangements the commit adds.
    pub new_arrangements: usize,
    /// Arrangements the commit replaces.
    pub updates: usize,
    /// Arrangements not imported: skipped outcomes and skipped documents.
    pub skipped: usize,
    /// Documents excluded as invalid.
    pub invalid: usize,
    /// Choirs the commit creates.
    pub new_choirs: usize,
    /// Decisions still open.
    pub open: usize,
}

/// Where one document's Song ends up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Resolution {
    Skipped,
    Existing(SongId),
    /// Created by this document of the batch.
    New(usize),
    Open(usize),
}

/// Where each valid document's Song ends up.
///
/// A document staged by an earlier one follows that document's resolution. When
/// the earlier document is skipped, the first later document with that Song
/// becomes its creator (so its own Song details win), and the rest follow it.
fn resolutions(reviews: &[Review], choices: &Choices) -> Vec<Option<Resolution>> {
    let mut resolved: Vec<Option<Resolution>> = Vec::with_capacity(reviews.len());
    let mut heirs: BTreeMap<usize, Resolution> = BTreeMap::new();
    for (index, review) in reviews.iter().enumerate() {
        let Review::Valid(review) = review else {
            resolved.push(None);
            continue;
        };
        let resolution = match choices.songs.get(&index) {
            Some(SongChoice::Skip) => Resolution::Skipped,
            Some(SongChoice::Existing(song)) => Resolution::Existing(*song),
            Some(SongChoice::New) => Resolution::New(index),
            None => match &review.identity {
                SongIdentity::Existing { song, .. } => Resolution::Existing(*song),
                SongIdentity::New => Resolution::New(index),
                SongIdentity::NeedsDecision { .. } => Resolution::Open(index),
                SongIdentity::StagedBy { document } => {
                    match resolved.get(*document).copied().flatten() {
                        Some(Resolution::Skipped) => {
                            *heirs.entry(*document).or_insert(Resolution::New(index))
                        }
                        Some(other) => other,
                        None => Resolution::Open(*document),
                    }
                }
            },
        };
        resolved.push(Some(resolution));
    }
    resolved
}

/// The reviews of a batch with the operator's Song choices applied.
///
/// This is [`classify_batch`] followed by [`classify_arrangements`] for every
/// document whose Song a choice settles, dependent documents included. A
/// document staged by an earlier one that was skipped becomes the creator of
/// the Song (`SongIdentity::New`), and later ones follow it. Both the review
/// screen and the commit's stale check use this one function, so they agree on
/// what the operator was shown.
pub fn review_with_choices(
    library: &SongLibrary,
    documents: &[StagedDocument],
    songs: &BTreeMap<usize, SongChoice>,
) -> Vec<Review> {
    let mut reviews = classify_batch(library, documents);
    let choices = Choices {
        songs: songs.clone(),
        arrangements: BTreeMap::new(),
    };
    let resolved = resolutions(&reviews, &choices);
    for (index, resolution) in resolved.iter().enumerate() {
        let (Some(resolution), Review::Valid(review), Staged::Valid(document)) =
            (resolution, &mut reviews[index], &documents[index].content)
        else {
            continue;
        };
        let song = match *resolution {
            Resolution::Skipped | Resolution::Open(_) => continue,
            Resolution::Existing(song) => ResolvedSong::Existing(song),
            Resolution::New(creator) if creator == index => ResolvedSong::New,
            Resolution::New(_) => ResolvedSong::Staged,
        };
        let mut settled = choices.songs.contains_key(&index)
            || matches!(review.identity, SongIdentity::NeedsDecision { .. });
        if let SongIdentity::StagedBy { document: creator } = review.identity {
            settled = true;
            match *resolution {
                // The creator was skipped: this document creates the Song.
                Resolution::New(heir) if heir == index => review.identity = SongIdentity::New,
                // A later document of the same Song follows the new creator.
                Resolution::New(heir) if heir != creator => {
                    review.identity = SongIdentity::StagedBy { document: heir };
                }
                _ => {}
            }
        }
        if settled {
            review.arrangements = classify_arrangements(library, document, song);
        }
    }
    reviews
}

/// The outcome of one Arrangement: the operator's, else the classification's
/// default; `None` when the classification needs a decision.
fn outcome_of(
    choices: &Choices,
    key: (usize, usize),
    classification: Classification,
) -> Option<Outcome> {
    choices
        .arrangements
        .get(&key)
        .copied()
        .or(match classification {
            Classification::AddNewSong => Some(Outcome::AddNewSong),
            Classification::AttachArrangement => Some(Outcome::Attach),
            Classification::SkipUnchanged => Some(Outcome::Skip),
            Classification::NeedsDecision => None,
        })
}

/// Counts each outcome of the choices over the reviews.
pub fn tally(reviews: &[Review], choices: &Choices) -> Tally {
    let mut tally = Tally::default();
    let resolved = resolutions(reviews, choices);
    let mut creators = BTreeSet::new();
    let mut new_choirs = BTreeSet::new();
    for (index, review) in reviews.iter().enumerate() {
        let Review::Valid(review) = review else {
            tally.invalid += 1;
            continue;
        };
        match resolved[index] {
            Some(Resolution::Skipped) => tally.skipped += review.arrangements.len(),
            Some(Resolution::Open(creator)) => {
                // A Song decision of the document itself counts once; a
                // dependent document waits on the earlier one.
                if creator == index {
                    tally.open += 1;
                }
            }
            Some(Resolution::New(creator)) => {
                count_arrangements(
                    &mut tally,
                    review,
                    choices,
                    index,
                    |writes| {
                        if writes {
                            creators.insert(creator);
                        }
                    },
                    &mut new_choirs,
                );
            }
            Some(Resolution::Existing(_)) => {
                count_arrangements(&mut tally, review, choices, index, |_| {}, &mut new_choirs);
            }
            None => {}
        }
    }
    tally.new_songs = creators.len();
    tally.new_choirs = new_choirs.len();
    tally
}

fn count_arrangements(
    tally: &mut Tally,
    review: &DocumentReview,
    choices: &Choices,
    document: usize,
    mut wrote: impl FnMut(bool),
    new_choirs: &mut BTreeSet<String>,
) {
    for (index, arrangement) in review.arrangements.iter().enumerate() {
        match outcome_of(choices, (document, index), arrangement.classification) {
            None => tally.open += 1,
            Some(Outcome::Skip) => tally.skipped += 1,
            Some(outcome) => {
                if outcome == Outcome::UpdateExisting {
                    tally.updates += 1;
                } else {
                    tally.new_arrangements += 1;
                }
                wrote(outcome.adds_arrangement());
                for item in &arrangement.evidence {
                    if let ArrangementEvidence::AddsChoir(name) = item {
                        new_choirs.insert(match_key(name));
                    }
                }
            }
        }
    }
}

/// Whether the choices leave no open decision and something to write.
pub fn can_commit(reviews: &[Review], choices: &Choices) -> bool {
    let tally = tally(reviews, choices);
    tally.open == 0 && tally.new_arrangements + tally.updates > 0
}

/// Why a plan could not be made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanError {
    /// The reviews do not belong to these documents.
    ReviewsOutOfStep,
    /// A decision is still open (document index, Arrangement index).
    OpenDecision {
        /// The document.
        document: usize,
        /// The Arrangement, or `None` for the document's Song.
        arrangement: Option<usize>,
    },
    /// The chosen outcome is not one the review offers.
    NotOffered {
        /// The document.
        document: usize,
        /// The Arrangement.
        arrangement: usize,
    },
    /// The chosen Song is unknown or archived.
    SongUnavailable(SongId),
    /// Two outcomes replace the same Arrangement.
    TargetTaken(ArrangementId),
    /// The Markdown draft has a blocking issue.
    NotSavable,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReviewsOutOfStep => f.write_str("the reviews do not match the documents"),
            Self::OpenDecision {
                document,
                arrangement,
            } => {
                write!(
                    f,
                    "document {document} has an open decision ({arrangement:?})"
                )
            }
            Self::NotOffered {
                document,
                arrangement,
            } => write!(
                f,
                "the outcome chosen for document {document}, Arrangement {arrangement} is not offered"
            ),
            Self::SongUnavailable(song) => write!(f, "Song {song} is unknown or archived"),
            Self::TargetTaken(id) => write!(f, "Arrangement {id} is replaced twice"),
            Self::NotSavable => f.write_str("the draft has a blocking issue"),
        }
    }
}

impl std::error::Error for PlanError {}

/// The mutations an import makes, with their identities.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImportPlan {
    /// Choirs to create (Decision 10).
    pub choirs: Vec<(ChoirId, ChoirDraft)>,
    /// Songs to create.
    pub songs: Vec<(SongId, SongDraft)>,
    /// Arrangements to create.
    pub arrangements: Vec<PlannedArrangement>,
    /// Arrangements to replace.
    pub replacements: Vec<PlannedReplacement>,
}

impl ImportPlan {
    /// Whether the plan writes nothing.
    pub fn is_empty(&self) -> bool {
        self.choirs.is_empty()
            && self.songs.is_empty()
            && self.arrangements.is_empty()
            && self.replacements.is_empty()
    }
}

/// One Arrangement to create. Sections are `NeedsReview` for a `.ordo`
/// import and `Draft` for Markdown (Decision 7).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedArrangement {
    /// Its identity.
    pub id: ArrangementId,
    /// The Song it belongs to: a Library Song or one in the same plan.
    pub song: SongId,
    /// Its content, with section and occurrence identities minted.
    pub draft: ArrangementDraft,
    /// Its Choirs: Library Choirs or ones in the same plan.
    pub choirs: Vec<ChoirId>,
}

/// One Arrangement to replace whole (Decisions 13 and 136). Section
/// identities are aligned to the existing sections where they are comparable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedReplacement {
    /// The Arrangement replaced.
    pub id: ArrangementId,
    /// The whole new content.
    pub draft: ArrangementDraft,
    /// The Arrangement's Choirs after the replacement.
    pub choirs: Vec<ChoirId>,
}

/// The values a commit stored, as storage returned them (hydration).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportCommitted {
    /// Choirs created.
    pub choirs: Vec<HydrateChoir>,
    /// Songs created.
    pub songs: Vec<HydrateSong>,
    /// Arrangements created.
    pub arrangements: Vec<HydrateArrangement>,
    /// Arrangements replaced.
    pub replaced: Vec<ReplacedArrangement>,
}

/// One replaced Arrangement as stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacedArrangement {
    /// Its identity.
    pub id: ArrangementId,
    /// The stored content, section states included.
    pub draft: ArrangementDraft,
    /// The stored Choirs.
    pub choirs: Vec<ChoirId>,
}

/// Maps each imported section onto the existing section with the same label
/// (`match_key`), then by position among the unlabeled, reusing that
/// section's identity; an unmatched section gets a minted one (Decision 13).
pub fn align_sections(
    imported: &[ImportSection],
    existing: &[Section],
    mint: &mut dyn FnMut() -> EntityId,
) -> Vec<SectionId> {
    pair_sections(imported, existing)
        .into_iter()
        .map(|pair| match pair {
            Some(index) => existing[index].id(),
            None => SectionId::from_entity(mint()),
        })
        .collect()
}

/// The identities a plan is built from, with everything minted free of the
/// Library and of the plan so far.
struct Builder<'a> {
    library: &'a SongLibrary,
    mint: &'a mut dyn FnMut() -> EntityId,
    plan: ImportPlan,
    songs: BTreeSet<SongId>,
    arrangements: BTreeSet<ArrangementId>,
    new_choirs: BTreeMap<String, ChoirId>,
}

impl<'a> Builder<'a> {
    fn new(library: &'a SongLibrary, mint: &'a mut dyn FnMut() -> EntityId) -> Self {
        Self {
            library,
            mint,
            plan: ImportPlan::default(),
            songs: BTreeSet::new(),
            arrangements: BTreeSet::new(),
            new_choirs: BTreeMap::new(),
        }
    }

    /// The wanted Song identity when free, else a minted one.
    fn song_id(&mut self, wanted: SongId) -> SongId {
        let mut id = wanted;
        while self.library.song(id).is_ok() || !self.songs.insert(id) {
            id = SongId::from_entity((self.mint)());
        }
        id
    }

    /// The wanted Arrangement identity when free, else a minted one
    /// (Decision 3).
    fn arrangement_id(&mut self, wanted: ArrangementId) -> ArrangementId {
        let mut id = wanted;
        while self.library.arrangement(id).is_ok() || !self.arrangements.insert(id) {
            id = ArrangementId::from_entity((self.mint)());
        }
        id
    }

    /// The Choirs of an Arrangement: matched active Choirs, Choirs this plan
    /// creates for unmatched names, and nothing for archived matches (so an
    /// Update unlinks an archived Choir; the Choirs difference discloses it).
    fn choirs(&mut self, names: &[String]) -> Vec<ChoirId> {
        let mut linked: Vec<ChoirId> = Vec::new();
        for name in names {
            let id = match lookup_choir(self.library, name) {
                ChoirLookup::Active(id) => id,
                ChoirLookup::Archived(_) => continue,
                ChoirLookup::Missing => {
                    let key = match_key(name);
                    if let Some(id) = self.new_choirs.get(&key) {
                        *id
                    } else {
                        let mut id = ChoirId::from_entity((self.mint)());
                        while self.library.choir(id).is_ok() {
                            id = ChoirId::from_entity((self.mint)());
                        }
                        self.new_choirs.insert(key, id);
                        self.plan.choirs.push((id, ChoirDraft::named(name.clone())));
                        id
                    }
                }
            };
            if !linked.contains(&id) {
                linked.push(id);
            }
        }
        linked
    }

    /// The content of an imported Arrangement. `sections` supplies section
    /// identities when they are aligned; otherwise each is minted. Unresolved
    /// Markdown repeats become empty placeholder sections (Decision 23).
    fn draft(
        &mut self,
        imported: &ImportArrangement,
        pagination: PaginationState,
        sections: Option<Vec<SectionId>>,
    ) -> ArrangementDraft {
        let ids = sections.unwrap_or_else(|| {
            imported
                .sections
                .iter()
                .map(|_| SectionId::from_entity((self.mint)()))
                .collect()
        });
        let mut drafts: Vec<SectionDraft> = imported
            .sections
            .iter()
            .zip(&ids)
            .map(|(section, id)| SectionDraft {
                id: *id,
                label: section.label.clone(),
                lyrics: section.lyrics.clone(),
                pagination,
            })
            .collect();
        let mut placeholders: BTreeMap<String, SectionId> = BTreeMap::new();
        let mut sequence = Vec::with_capacity(imported.sequence.len());
        for entry in &imported.sequence {
            let section = match entry {
                SequenceEntry::Section(index) => ids.get(*index).copied(),
                SequenceEntry::Unresolved { label, .. } => {
                    let key = match_key(label);
                    let id = *placeholders.entry(key).or_insert_with(|| {
                        let id = SectionId::from_entity((self.mint)());
                        drafts.push(SectionDraft::labeled(id, label.clone(), ""));
                        id
                    });
                    Some(id)
                }
            };
            if let Some(section) = section {
                sequence.push(OccurrenceDraft::new(
                    OccurrenceId::from_entity((self.mint)()),
                    section,
                ));
            }
        }
        ArrangementDraft {
            title: imported.title.clone(),
            languages: imported.languages.clone(),
            rendition_attribution: imported.rendition_attribution.clone(),
            occasions: imported.occasions.clone(),
            notes: imported.notes.clone(),
            sections: drafts,
            sequence,
        }
    }

    /// Plans a new Arrangement of `song`.
    fn add(&mut self, song: SongId, imported: &ImportArrangement, pagination: PaginationState) {
        let id = self.arrangement_id(imported.id);
        let draft = self.draft(imported, pagination, None);
        let choirs = self.choirs(&imported.choirs);
        self.plan.arrangements.push(PlannedArrangement {
            id,
            song,
            draft,
            choirs,
        });
    }
}

/// The mutations with their identities; the mint closure supplies section,
/// occurrence and Choir ids (and a fresh Arrangement or Song id when the
/// imported one is taken, Decision 3).
///
/// An Arrangement of a new Song is planned only when something is written for
/// it; a Song is created from the first document that introduces it, so its
/// details win (Decision 6). Existing Songs' details are never planned
/// (Decision 12). Invalid documents never enter the plan.
///
/// # Errors
///
/// `OpenDecision` while a Song or Arrangement decision is open, `NotOffered`
/// for an outcome the Arrangement's review does not offer, `SongUnavailable`
/// for a chosen Song that is unknown or archived, `TargetTaken` when two
/// outcomes replace one Arrangement, and `ReviewsOutOfStep` when `reviews` is
/// not the result of classifying `documents`.
pub fn plan(
    library: &SongLibrary,
    documents: &[StagedDocument],
    reviews: &[Review],
    choices: &Choices,
    mint: &mut dyn FnMut() -> EntityId,
) -> Result<ImportPlan, PlanError> {
    if documents.len() != reviews.len() {
        return Err(PlanError::ReviewsOutOfStep);
    }
    let resolved = resolutions(reviews, choices);
    let mut builder = Builder::new(library, mint);
    let mut created: BTreeMap<usize, SongId> = BTreeMap::new();
    let mut targets: BTreeSet<ArrangementId> = BTreeSet::new();
    for (index, resolution) in resolved.iter().enumerate() {
        let (Some(resolution), Staged::Valid(document)) = (resolution, &documents[index].content)
        else {
            continue;
        };
        let song = match *resolution {
            Resolution::Skipped => continue,
            Resolution::Open(creator) => {
                return Err(PlanError::OpenDecision {
                    document: creator,
                    arrangement: None,
                });
            }
            Resolution::Existing(song) => {
                if library.song(song).is_ok_and(|stored| !stored.is_archived()) {
                    ResolvedSong::Existing(song)
                } else {
                    return Err(PlanError::SongUnavailable(song));
                }
            }
            Resolution::New(creator) if creator == index => ResolvedSong::New,
            Resolution::New(_) => ResolvedSong::Staged,
        };
        let arrangements = classify_arrangements(library, document, song);
        for (position, review) in arrangements.iter().enumerate() {
            let key = (index, position);
            let Some(outcome) = outcome_of(choices, key, review.classification) else {
                return Err(PlanError::OpenDecision {
                    document: index,
                    arrangement: Some(position),
                });
            };
            if outcome != Outcome::Skip && !review.offered.contains(&outcome) {
                return Err(PlanError::NotOffered {
                    document: index,
                    arrangement: position,
                });
            }
            let imported = &document.arrangements[position];
            match outcome {
                Outcome::Skip => {}
                Outcome::UpdateExisting => {
                    let Some(target) = review.nearest else {
                        return Err(PlanError::NotOffered {
                            document: index,
                            arrangement: position,
                        });
                    };
                    if !targets.insert(target) {
                        return Err(PlanError::TargetTaken(target));
                    }
                    let Ok(existing) = library.arrangement(target) else {
                        return Err(PlanError::NotOffered {
                            document: index,
                            arrangement: position,
                        });
                    };
                    let ids =
                        align_sections(&imported.sections, existing.sections(), &mut *builder.mint);
                    let draft = builder.draft(imported, PaginationState::NeedsReview, Some(ids));
                    let choirs = builder.choirs(&imported.choirs);
                    builder.plan.replacements.push(PlannedReplacement {
                        id: target,
                        draft,
                        choirs,
                    });
                }
                _ => {
                    let song_id = match song {
                        ResolvedSong::Existing(song) => song,
                        ResolvedSong::New | ResolvedSong::Staged => {
                            let creator = match resolved[index] {
                                Some(Resolution::New(creator)) => creator,
                                _ => index,
                            };
                            ensure_song(&mut builder, documents, &mut created, creator)
                        }
                    };
                    builder.add(song_id, imported, PaginationState::NeedsReview);
                }
            }
        }
    }
    Ok(builder.plan)
}

/// Creates the Song of the document `creator` in the plan once.
fn ensure_song(
    builder: &mut Builder<'_>,
    documents: &[StagedDocument],
    created: &mut BTreeMap<usize, SongId>,
    creator: usize,
) -> SongId {
    if let Some(id) = created.get(&creator) {
        return *id;
    }
    let (wanted, draft) = match &documents[creator].content {
        Staged::Valid(document) => (document.id, document.song.clone()),
        Staged::Invalid(_) => (SongId::from_entity((builder.mint)()), SongDraft::default()),
    };
    let id = builder.song_id(wanted);
    builder.plan.songs.push((id, draft));
    created.insert(creator, id);
    id
}

/// Plans a Markdown save: one new or existing Song and one Arrangement;
/// unresolved repeats become placeholder sections (Decision 23). Sections
/// begin `Draft` (Decision 7). `SongChoice::Skip` plans nothing.
///
/// # Errors
///
/// `NotSavable` for a draft with a blocking issue and `SongUnavailable` for
/// an unknown or archived target Song.
pub fn plan_markdown(
    library: &SongLibrary,
    draft: &MarkdownDraft,
    target: SongChoice,
    mint: &mut dyn FnMut() -> EntityId,
) -> Result<ImportPlan, PlanError> {
    if !draft.can_save() {
        return Err(PlanError::NotSavable);
    }
    let mut builder = Builder::new(library, mint);
    let document: &ImportDocument = &draft.document;
    let song = match target {
        SongChoice::Skip => return Ok(builder.plan),
        SongChoice::Existing(song) => {
            if !library.song(song).is_ok_and(|stored| !stored.is_archived()) {
                return Err(PlanError::SongUnavailable(song));
            }
            song
        }
        SongChoice::New => {
            let id = builder.song_id(document.id);
            builder.plan.songs.push((id, document.song.clone()));
            id
        }
    };
    for arrangement in &document.arrangements {
        builder.add(song, arrangement, PaginationState::Draft);
    }
    Ok(builder.plan)
}
