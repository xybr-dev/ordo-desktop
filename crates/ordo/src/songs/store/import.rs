//! Song import staging, commit and Markdown save (T2.10).
//!
//! Staging reads files and the in-memory library only; nothing is written
//! until the one commit transaction. The commit and the Markdown save join the
//! store's single write queue, so an import never interleaves with an
//! autosave: the plan is built when the write reaches the front of the queue,
//! against the library as it is then, checked with the domain's whole-library
//! rules, and sent to storage as one batch.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use domain::{
    identity::{ArrangementId, EntityId, RevisionId, SongId},
    revisions::{RevisionEntity, RevisionKind},
    song_library::{
        ArchiveState, ArrangementDraft, HydrateArrangement, HydrateChoir, HydrateSong, SongLibrary,
        import::{
            Choices, ImportCommitted, ImportPlan, InvalidReason, MarkdownDraft, Outcome, PlanError,
            ReplacedArrangement, Review, SongChoice, SongIdentity, Staged, StagedDocument,
            markdown, plan, plan_markdown, review_with_choices,
        },
    },
};
use gpui::{AppContext as _, Context, SharedString, Task};
use storage::{
    ArrangementContent, ImportBatch, ImportCommittedRecords, ImportedArrangement, NewChoir,
    NewSong, OccurrenceContent, SectionContent, StorageError,
    exchange::{Finding, FindingCode, InspectLimits, SongImportRead, read_song_import},
};

use super::{
    Done, LibraryStore, LibraryWrite, Prepared, Reply, Verdict, arrangement_draft_of, draft_of,
    refusal, song_draft_of,
};

/// The refusal for an import the Library moved away from while it was
/// reviewed (Decision 14).
pub(crate) const STALE_IMPORT: &str =
    "Something in the Library changed while you reviewed. Review the import again.";
/// The most bytes a Markdown or text song may have: 1 MiB.
pub const MARKDOWN_LIMIT: u64 = 1024 * 1024;

/// A Markdown or text file as read once: its name, its bytes and the draft
/// they parse to.
#[derive(Debug)]
pub struct MarkdownFile {
    /// The file's name, as the draft shows it.
    pub name: String,
    /// The bytes as read, for the editable text.
    pub bytes: Vec<u8>,
    /// What the bytes parse to.
    pub draft: MarkdownDraft,
}

/// The refusal for any other failure of the commit.
pub(super) const COULDNT_IMPORT: &str = "Couldn't import. Nothing was changed.";
/// The refusal for a commit while a decision is still open. No copy exists
/// for it in the plan; the review keeps **Import approved** disabled, so this
/// only guards a caller that skipped that.
const OPEN_DECISION: &str = "Some items still need a decision.";
/// The refusal for a Markdown save that has nothing to write.
const NOTHING_TO_SAVE: &str = "There is nothing to save.";

/// The operator copy for a storage failure of an import commit: a moved
/// identity, head, target or Choir asks for a new review; anything else is
/// the generic failure.
pub(super) fn import_copy(error: &StorageError) -> &'static str {
    match error {
        StorageError::IdentityCollision { .. }
        | StorageError::StaleRevision { .. }
        | StorageError::MissingReference { .. }
        | StorageError::NotFound { .. }
        | StorageError::InvalidInput {
            reason: "archived", ..
        } => STALE_IMPORT,
        _ => COULDNT_IMPORT,
    }
}

/// The files picked for an import, read and classified against the Library
/// as it was at staging time. Nothing about it has been written.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImportStaging {
    /// One document per picked file, in file-name order.
    pub documents: Vec<StagedDocument>,
    /// One review per document, from classifying the batch.
    pub reviews: Vec<Review>,
    /// The head revision each Arrangement that an Update could replace had
    /// when it was reviewed; `None` for a T2.1-era row with no history
    /// (storage backfills it inside the commit transaction, never here).
    pub heads: BTreeMap<ArrangementId, Option<RevisionId>>,
}

/// What an approved import or Markdown save wrote.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImportOutcome {
    /// The Songs created, in plan order.
    pub songs: Vec<SongId>,
    /// The Arrangements created, in plan order.
    pub arrangements: Vec<ArrangementId>,
    /// The Arrangements replaced, in plan order.
    pub replaced: Vec<ArrangementId>,
}

/// What a queued import write carries.
pub(super) struct ImportRequest {
    source: Source,
    /// Filled when the write is prepared: what the batch will write.
    outcome: ImportOutcome,
}

impl ImportRequest {
    /// Whether this is a reviewed batch, whose verdict runs in the background.
    pub(super) fn is_batch(&self) -> bool {
        matches!(self.source, Source::Batch { .. })
    }
}

/// Where the plan comes from.
// One request per commit; boxing the variants would only add a deref.
#[allow(clippy::large_enum_variant)]
enum Source {
    /// Reviewed `.ordo` documents.
    Batch {
        documents: Vec<StagedDocument>,
        reviews: Vec<Review>,
        choices: Choices,
        heads: BTreeMap<ArrangementId, Option<RevisionId>>,
    },
    /// One Markdown draft saved to a new or an existing Song.
    Markdown {
        draft: MarkdownDraft,
        target: SongChoice,
    },
}

fn fresh_entity() -> EntityId {
    EntityId::from_random_bytes(*uuid::Uuid::new_v4().as_bytes())
}

fn reason_of(finding: &Finding) -> InvalidReason {
    let code = match finding.code {
        FindingCode::Container => "container",
        FindingCode::Schema => "schema",
        FindingCode::UnsupportedVersion => "unsupported-version",
        FindingCode::MissingReference => "missing-reference",
        FindingCode::HashMismatch => "hash-mismatch",
        FindingCode::WrongKind => "wrong-kind",
        FindingCode::Assets => "assets",
        FindingCode::Unreadable => "unreadable",
    };
    let message = if finding.pointer.is_empty() {
        finding.message.clone()
    } else {
        format!("{} · {}", finding.pointer, finding.message)
    };
    InvalidReason {
        code: code.to_owned(),
        message,
    }
}

/// Reads one picked file. An unreadable file is an Invalid document, never an
/// error: the review shows why.
fn read_document(path: &Path) -> StagedDocument {
    let source = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let content = match read_song_import(path, &InspectLimits::default()) {
        SongImportRead::Valid(document) => Staged::Valid(document),
        SongImportRead::Invalid(findings) => {
            Staged::Invalid(findings.iter().map(reason_of).collect())
        }
        SongImportRead::Unsupported { message } => Staged::Invalid(vec![InvalidReason {
            code: "unsupported-version".to_owned(),
            message,
        }]),
    };
    StagedDocument { source, content }
}

/// The Arrangements whose head a commit may need: every active Arrangement of
/// a Song a review names, and every Arrangement a review would update.
fn head_targets(library: &SongLibrary, reviews: &[Review]) -> BTreeSet<ArrangementId> {
    let mut songs: BTreeSet<SongId> = BTreeSet::new();
    let mut targets: BTreeSet<ArrangementId> = BTreeSet::new();
    for review in reviews {
        let Review::Valid(review) = review else {
            continue;
        };
        match &review.identity {
            SongIdentity::Existing { song, .. } => {
                songs.insert(*song);
            }
            SongIdentity::NeedsDecision { candidates, .. } => {
                songs.extend(candidates.iter().map(|(song, _)| *song));
            }
            SongIdentity::New | SongIdentity::StagedBy { .. } => {}
        }
        for arrangement in &review.arrangements {
            if arrangement.offered.contains(&Outcome::UpdateExisting)
                && let Some(nearest) = arrangement.nearest
            {
                targets.insert(nearest);
            }
        }
    }
    for song in songs {
        if let Ok(arrangements) = library.arrangements_for_song(song) {
            targets.extend(
                arrangements
                    .into_iter()
                    .filter(|arrangement| !arrangement.is_archived())
                    .map(|arrangement| arrangement.id()),
            );
        }
    }
    targets
}

/// The stored content of a planned Arrangement.
fn content_of(
    draft: ArrangementDraft,
    choirs: Vec<domain::identity::ChoirId>,
) -> ArrangementContent {
    ArrangementContent {
        title: draft.title,
        languages: draft.languages,
        choirs,
        rendition_attribution: draft.rendition_attribution,
        occasions: draft.occasions,
        notes: draft.notes,
        sections: draft
            .sections
            .into_iter()
            .map(|section| SectionContent {
                id: section.id,
                label: section.label,
                lyrics: section.lyrics,
                pagination: section.pagination,
            })
            .collect(),
        sequence: draft
            .sequence
            .into_iter()
            .map(|occurrence| OccurrenceContent {
                id: occurrence.id,
                section: occurrence.section,
            })
            .collect(),
    }
}

/// Maps a plan refusal to operator copy.
fn plan_refusal(error: &PlanError) -> SharedString {
    match error {
        PlanError::OpenDecision { .. } => OPEN_DECISION.into(),
        PlanError::NotSavable => "This song can't be saved yet.".into(),
        _ => STALE_IMPORT.into(),
    }
}

impl LibraryStore {
    /// Reads and classifies the picked files off the GPUI thread; nothing is
    /// written. An unreadable file becomes an Invalid document. Documents are
    /// classified in file-name order (Decision 6).
    pub fn stage_import(&self, paths: Vec<PathBuf>, cx: &mut Context<Self>) -> Task<ImportStaging> {
        cx.spawn(async move |this, cx| {
            let documents = cx
                .background_spawn(async move {
                    let mut documents: Vec<StagedDocument> =
                        paths.iter().map(|path| read_document(path)).collect();
                    documents.sort_by(|a, b| a.source.cmp(&b.source));
                    documents
                })
                .await;
            let task = this.update(cx, |this, cx| {
                this.classify_task(documents, BTreeMap::new(), cx)
            });
            match task {
                Ok(task) => task.await,
                Err(_) => ImportStaging::default(),
            }
        })
    }

    /// Re-classifies after **Remove from review**, **Choose another file** or
    /// a Song choice, against the current library. `songs` are the Song
    /// choices made so far (by document index): the reviews come from
    /// [`review_with_choices`], the same function the commit's stale check
    /// uses, so a Song choice never makes the commit stale.
    pub fn reclassify(
        &self,
        documents: Vec<StagedDocument>,
        songs: BTreeMap<usize, SongChoice>,
        cx: &mut Context<Self>,
    ) -> Task<ImportStaging> {
        self.classify_task(documents, songs, cx)
    }

    /// Classifies on a snapshot of the library in the background, with the
    /// heads the review may replace read from storage (a read: nothing is
    /// backfilled or written).
    ///
    /// The snapshot is taken only while no write is queued or in flight, and
    /// the head reads are queued in the same turn, so no write can land
    /// between the library the review saw and the heads it records: a head
    /// can only be older than what the review saw, which storage refuses as
    /// stale, never newer. If the library moved while the snapshot was being
    /// classified, it is taken again.
    fn classify_task(
        &self,
        documents: Vec<StagedDocument>,
        songs: BTreeMap<usize, SongChoice>,
        cx: &mut Context<Self>,
    ) -> Task<ImportStaging> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(snapshot)) = this.update(cx, |this, _| {
                    let idle = !this.writing && this.queue.is_empty();
                    idle.then(|| this.library.clone())
                }) else {
                    // The store is gone, or a write is in flight: wait a moment.
                    if this.upgrade().is_none() {
                        return ImportStaging::default();
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(10))
                        .await;
                    continue;
                };
                let (documents_back, reviews, targets) = {
                    let documents = documents.clone();
                    let songs = songs.clone();
                    let snapshot = snapshot.clone();
                    cx.background_spawn(async move {
                        let reviews = review_with_choices(&snapshot, &documents, &songs);
                        let targets = head_targets(&snapshot, &reviews);
                        (documents, reviews, targets)
                    })
                    .await
                };
                let reads = this
                    .update(cx, |this, _| {
                        let unchanged =
                            !this.writing && this.queue.is_empty() && this.library == snapshot;
                        unchanged.then(|| {
                            this.storage
                                .with(|storage| {
                                    targets
                                        .iter()
                                        .map(|id| {
                                            (
                                                *id,
                                                storage.revisions(RevisionEntity::Arrangement(*id)),
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        })
                    })
                    .ok()
                    .flatten();
                let Some(reads) = reads else {
                    // The library moved while it was classified: start over.
                    continue;
                };
                let heads = cx
                    .background_spawn(async move {
                        let mut heads = BTreeMap::new();
                        for (id, read) in reads {
                            // A failed read leaves no head: a commit that needs
                            // it is then refused as changed, never guessed.
                            if let Ok(revisions) = read.await {
                                heads.insert(id, revisions.last().map(|revision| revision.id));
                            }
                        }
                        heads
                    })
                    .await;
                return ImportStaging {
                    documents: documents_back,
                    reviews,
                    heads,
                };
            }
        })
    }

    /// Plans, checks against the whole library, then commits through the
    /// write queue as one storage transaction. An open decision, or a Library
    /// that moved since staging, is refused without I/O; a plan with nothing
    /// to write resolves to an empty outcome without I/O.
    ///
    /// `staging` is borrowed because the caller keeps it: a refused commit
    /// leaves the operator in the review, which still needs the documents,
    /// reviews and heads. The request clones them once into the queue.
    pub fn commit_import(
        &mut self,
        staging: &ImportStaging,
        choices: Choices,
        cx: &mut Context<Self>,
    ) -> Task<Result<ImportOutcome, SharedString>> {
        let request = ImportRequest {
            source: Source::Batch {
                documents: staging.documents.clone(),
                reviews: staging.reviews.clone(),
                choices,
                heads: staging.heads.clone(),
            },
            outcome: ImportOutcome::default(),
        };
        self.submit_done(
            LibraryWrite::CommitImport(Box::new(request)),
            |done| match done {
                Done::Imported(outcome) => Ok(outcome),
                _ => Err(SharedString::from(super::WRONG_KIND)),
            },
            cx,
        )
    }

    /// Reads one Markdown or text file off the GPUI thread, once, and parses
    /// it into a draft with fresh Song and Arrangement identities. A file over
    /// [`MARKDOWN_LIMIT`] bytes is refused as unreadable: no song is that
    /// long, and the text is shown whole. Nothing is saved.
    pub fn read_markdown(
        &self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<Result<MarkdownFile, SharedString>> {
        cx.background_spawn(async move {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            let unreadable = || SharedString::from(format!("Couldn't read {name}."));
            let file = std::fs::File::open(&path).map_err(|_| unreadable())?;
            let mut bytes = Vec::new();
            let mut taken = std::io::Read::take(file, MARKDOWN_LIMIT + 1);
            std::io::Read::read_to_end(&mut taken, &mut bytes).map_err(|_| unreadable())?;
            if bytes.len() as u64 > MARKDOWN_LIMIT {
                return Err(unreadable());
            }
            let ids = (
                SongId::from_entity(fresh_entity()),
                ArrangementId::from_entity(fresh_entity()),
            );
            let draft = markdown::parse(&bytes, ids);
            Ok(MarkdownFile { name, bytes, draft })
        })
    }

    /// Saves a draft as a new Song and Arrangement, or as one Arrangement of
    /// an existing Song, in one transaction through the write queue. Sections
    /// begin Draft; an unresolved repeat is saved as an empty placeholder
    /// section (Decision 23). Resolves to the new Arrangement.
    pub fn save_markdown(
        &mut self,
        draft: MarkdownDraft,
        target: SongChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<ArrangementId, SharedString>> {
        let request = ImportRequest {
            source: Source::Markdown { draft, target },
            outcome: ImportOutcome::default(),
        };
        self.submit_done(
            LibraryWrite::CommitImport(Box::new(request)),
            |done| match done {
                Done::Imported(outcome) => outcome
                    .arrangements
                    .first()
                    .copied()
                    .ok_or_else(|| SharedString::from(NOTHING_TO_SAVE)),
                _ => Err(SharedString::from(super::WRONG_KIND)),
            },
            cx,
        )
    }

    /// The domain's verdict on an import write, before any I/O, with the
    /// storage batch it needs.
    pub(super) fn import_verdict(
        library: &SongLibrary,
        request: &mut ImportRequest,
    ) -> Result<Verdict, SharedString> {
        let mut mint = fresh_entity;
        let (plan, first_kind) = match &request.source {
            Source::Batch {
                documents,
                reviews,
                choices,
                ..
            } => {
                // The review is only as good as the Library it saw: a
                // different classification now, with the same Song choices
                // applied, means it must be redone.
                //
                // Timed in release at 50 documents against 1,000 Songs, this
                // verdict costs about 30 ms (T2.10c), past the 16 ms frame
                // budget, so a batch runs it in `background_spawn` on a
                // snapshot (`verdict_in_background`); only Markdown saves,
                // which touch one Song, keep it on the GPUI thread.
                if review_with_choices(library, documents, &choices.songs) != *reviews {
                    return Err(STALE_IMPORT.into());
                }
                let plan = plan(library, documents, reviews, choices, &mut mint)
                    .map_err(|error| plan_refusal(&error))?;
                library
                    .check_import(&plan)
                    .map_err(|_| SharedString::from(STALE_IMPORT))?;
                (plan, RevisionKind::Import)
            }
            Source::Markdown { draft, target } => {
                let plan = plan_markdown(library, draft, *target, &mut mint)
                    .map_err(|error| plan_refusal(&error))?;
                library.check_import(&plan).map_err(refusal)?;
                (plan, RevisionKind::Creation)
            }
        };
        if plan.is_empty() {
            return Ok(Verdict::Resolved(Done::Imported(ImportOutcome::default())));
        }
        let heads = match &request.source {
            Source::Batch { heads, .. } => Some(heads),
            Source::Markdown { .. } => None,
        };
        let (batch, outcome) = batch_of(plan, heads, first_kind)?;
        request.outcome = outcome;
        Ok(Verdict::Write(Prepared::CommitImport(Box::new(batch))))
    }

    /// Runs a batch import's verdict on a snapshot of the library in the
    /// background, then starts the storage write. The queue is held
    /// (`writing`) meanwhile, so nothing else writes; the generation check
    /// still refuses the commit if the library was replaced under it.
    pub(super) fn verdict_in_background(
        &mut self,
        write: LibraryWrite,
        reply: Reply,
        cx: &mut Context<Self>,
    ) {
        let LibraryWrite::CommitImport(mut request) = write else {
            let _ = reply.try_send(Err(super::WRONG_KIND.into()));
            return;
        };
        self.writing = true;
        let library = self.library.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let (request, verdict) = cx
                .background_spawn(async move {
                    let verdict = Self::import_verdict(&library, &mut request);
                    (request, verdict)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.writing = false;
                let verdict = if this.generation == generation {
                    verdict
                } else {
                    Err(SharedString::from(STALE_IMPORT))
                };
                match verdict {
                    Ok(Verdict::Write(prepared)) => {
                        this.dispatch(LibraryWrite::CommitImport(request), reply, prepared, cx);
                    }
                    Ok(Verdict::Unchanged(id)) => {
                        let _ = reply.try_send(Ok(Done::Id(id)));
                    }
                    Ok(Verdict::Resolved(done)) => {
                        let _ = reply.try_send(Ok(done));
                    }
                    Err(message) => {
                        let _ = reply.try_send(Err(message));
                    }
                }
                this.pump(cx);
            });
        })
        .detach();
    }

    /// Takes what a commit wrote into memory, from the returned records and
    /// never a second read.
    pub(super) fn apply_imported(
        &mut self,
        request: &ImportRequest,
        records: &ImportCommittedRecords,
    ) -> Result<Done, SharedString> {
        let replaced: BTreeSet<ArrangementId> = request.outcome.replaced.iter().copied().collect();
        let mut committed = ImportCommitted {
            choirs: Vec::new(),
            songs: Vec::new(),
            arrangements: Vec::new(),
            replaced: Vec::new(),
        };
        for record in &records.choirs {
            committed.choirs.push(HydrateChoir {
                id: record.id,
                created_at: record.created_at,
                draft: draft_of(record),
                archive_state: ArchiveState::Active,
            });
        }
        for record in &records.songs {
            committed.songs.push(HydrateSong {
                id: record.id,
                created_at: record.created_at,
                draft: song_draft_of(record),
                archive_state: ArchiveState::Active,
            });
        }
        for record in &records.arrangements {
            if replaced.contains(&record.id) {
                committed.replaced.push(ReplacedArrangement {
                    id: record.id,
                    draft: arrangement_draft_of(record),
                    choirs: record.choirs.clone(),
                });
            } else {
                committed.arrangements.push(HydrateArrangement {
                    id: record.id,
                    song: record.song,
                    created_at: record.created_at,
                    draft: arrangement_draft_of(record),
                    choirs: record.choirs.clone(),
                    archive_state: ArchiveState::Active,
                });
            }
        }
        self.library.apply_import(committed).map_err(refusal)?;
        Ok(Done::Imported(request.outcome.clone()))
    }
}

/// The storage batch for a plan, with what it will write. A replacement
/// whose reviewed head is unknown is refused as changed.
fn batch_of(
    plan: ImportPlan,
    heads: Option<&BTreeMap<ArrangementId, Option<RevisionId>>>,
    first_kind: RevisionKind,
) -> Result<(ImportBatch, ImportOutcome), SharedString> {
    let outcome = ImportOutcome {
        songs: plan.songs.iter().map(|(id, _)| *id).collect(),
        arrangements: plan.arrangements.iter().map(|planned| planned.id).collect(),
        replaced: plan.replacements.iter().map(|r| r.id).collect(),
    };
    let mut replacements = Vec::with_capacity(plan.replacements.len());
    for replacement in plan.replacements {
        let Some(expected_parent) = heads.and_then(|heads| heads.get(&replacement.id)).copied()
        else {
            return Err(STALE_IMPORT.into());
        };
        replacements.push(storage::ArrangementReplacement {
            id: replacement.id,
            content: content_of(replacement.draft, replacement.choirs),
            expected_parent,
        });
    }
    let batch = ImportBatch {
        choirs: plan
            .choirs
            .into_iter()
            .map(|(id, draft)| {
                (
                    id,
                    NewChoir {
                        name: draft.name,
                        short_label: draft.short_label,
                        languages: draft.languages,
                        notes: draft.notes,
                    },
                )
            })
            .collect(),
        songs: plan
            .songs
            .into_iter()
            .map(|(id, draft)| {
                (
                    id,
                    NewSong {
                        title: draft.title,
                        aliases: draft.aliases,
                        original_attribution: draft.original_attribution,
                        provenance: draft.provenance,
                        rights_notes: draft.rights_notes,
                        liturgical_moments: draft.liturgical_moments,
                        occasions: draft.occasions,
                    },
                )
            })
            .collect(),
        arrangements: plan
            .arrangements
            .into_iter()
            .map(|planned| ImportedArrangement {
                id: planned.id,
                song: planned.song,
                content: content_of(planned.draft, planned.choirs),
            })
            .collect(),
        replacements,
        first_kind,
    };
    Ok((batch, outcome))
}
