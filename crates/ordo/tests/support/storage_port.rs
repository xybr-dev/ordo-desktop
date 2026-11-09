//! A [`RevisionPort`] over a real `storage::Storage` for one Arrangement.
//!
//! Shared by the edit-session E2E test and the edit-session lab through a
//! `#[path]` module include; `ordo` takes no normal dependency on `storage`.
//! The harness content maps onto an Arrangement as follows: `title` is the
//! title, `notes` is the rendition attribution (empty text is `None`), and
//! every item is one section (label and lyrics are the item text) placed once
//! in the Sequence, in item order. Items are unique labels, so a
//! `label -> (SectionId, OccurrenceId)` map keeps identities stable across
//! reorders and across Undo.

#![allow(
    dead_code,
    reason = "each includer uses a different part of the adapter"
)]

use std::{cell::Cell, cell::RefCell, collections::HashMap, future::Future, pin::Pin, rc::Rc};

use domain::{
    identity::{ArrangementId, EntityId, OccurrenceId, RevisionId, SectionId},
    revisions::RevisionEntity,
};
use ordo::edit_session::{CommitPoint, RevisionPort, SaveFailure, harness::HarnessContent};
use storage::{
    ArrangementContent, ArrangementRecord, OccurrenceContent, SectionContent, Storage, StorageError,
};

/// The one `Storage` handle, shared by the port and its host. `Storage` has a
/// single owner, so a host that must end with an orderly
/// [`Storage::shutdown`] takes it back with [`SharedStorage::take`] instead of
/// relying on every entity that holds a clone being dropped first.
#[derive(Clone)]
pub struct SharedStorage(Rc<RefCell<Option<Storage>>>);

impl SharedStorage {
    /// Wraps a spawned `Storage`.
    pub fn new(storage: Storage) -> Self {
        Self(Rc::new(RefCell::new(Some(storage))))
    }

    /// Runs `f` on the handle (to start a storage future, which owns its
    /// state and does not borrow the handle). Panics after [`Self::take`].
    pub fn with<R>(&self, f: impl FnOnce(&Storage) -> R) -> R {
        f(self.0.borrow().as_ref().expect("storage is still running"))
    }

    /// Takes the handle for shutdown; later commits fail with `SaveFailure`.
    pub fn take(&self) -> Option<Storage> {
        self.0.borrow_mut().take()
    }
}

type Ids = HashMap<String, (SectionId, OccurrenceId)>;

fn random_entity() -> EntityId {
    EntityId::from_random_bytes(uuid::Uuid::new_v4().into_bytes())
}

/// The commit side of the edit session over a real Storage.
pub struct StoragePort {
    storage: SharedStorage,
    arrangement: ArrangementId,
    base: ArrangementRecord,
    ids: Rc<RefCell<Ids>>,
    expected_parent: Rc<Cell<RevisionId>>,
    /// When set, the next commit fails with `SaveFailure` without calling
    /// storage (the lab's "Fail next save" toggle).
    pub fail_next: Rc<Cell<bool>>,
}

impl StoragePort {
    /// A port over `arrangement`, with its current content. The expected
    /// parent starts at the Arrangement's latest revision.
    pub async fn open(
        storage: SharedStorage,
        arrangement: ArrangementId,
    ) -> Result<(Self, HarnessContent), StorageError> {
        let latest = storage.with(|s| s.latest_revision(RevisionEntity::Arrangement(arrangement)));
        let record = storage.with(|s| s.arrangement(arrangement));
        let latest = latest.await?;
        let base = record.await?;
        let ids = base
            .sequence
            .iter()
            .filter_map(|occurrence| {
                let section = base.sections.iter().find(|s| s.id == occurrence.section)?;
                let label = section.label.clone()?;
                Some((label, (section.id, occurrence.id)))
            })
            .collect();
        let content = content_of(&base);
        Ok((
            Self {
                storage,
                arrangement,
                base,
                ids: Rc::new(RefCell::new(ids)),
                expected_parent: Rc::new(Cell::new(latest)),
                fail_next: Rc::new(Cell::new(false)),
            },
            content,
        ))
    }

    /// The Arrangement's current stored content as harness content (sections
    /// in Sequence order), for a restart or after a restore.
    pub fn load(
        storage: &Storage,
        arrangement: ArrangementId,
    ) -> impl Future<Output = Result<HarnessContent, StorageError>> + use<> {
        let record = storage.arrangement(arrangement);
        async move { Ok(content_of(&record.await?)) }
    }

    fn to_content(&self, content: &HarnessContent) -> ArrangementContent {
        let mut ids = self.ids.borrow_mut();
        let mut sections = Vec::new();
        let mut sequence = Vec::new();
        for item in &content.items {
            let (section, occurrence) = *ids.entry(item.clone()).or_insert_with(|| {
                (
                    SectionId::from_entity(random_entity()),
                    OccurrenceId::from_entity(random_entity()),
                )
            });
            sections.push(SectionContent {
                id: section,
                label: Some(item.clone()),
                lyrics: item.clone(),
            });
            sequence.push(OccurrenceContent {
                id: occurrence,
                section,
            });
        }
        ArrangementContent {
            title: content.title.clone(),
            languages: self.base.languages.clone(),
            choirs: self.base.choirs.clone(),
            rendition_attribution: (!content.notes.is_empty()).then(|| content.notes.clone()),
            occasions: self.base.occasions.clone(),
            sections,
            sequence,
        }
    }
}

fn content_of(record: &ArrangementRecord) -> HarnessContent {
    let items = record
        .sequence
        .iter()
        .filter_map(|occurrence| {
            let section = record
                .sections
                .iter()
                .find(|s| s.id == occurrence.section)?;
            Some(
                section
                    .label
                    .clone()
                    .unwrap_or_else(|| section.lyrics.clone()),
            )
        })
        .collect();
    HarnessContent {
        title: record.title.clone(),
        notes: record.rendition_attribution.clone().unwrap_or_default(),
        items,
    }
}

impl RevisionPort<HarnessContent> for StoragePort {
    fn commit(
        &self,
        content: HarnessContent,
        point: CommitPoint,
    ) -> Pin<Box<dyn Future<Output = Result<(), SaveFailure>>>> {
        if self.fail_next.replace(false) {
            return Box::pin(async {
                Err(SaveFailure {
                    message: "simulated save failure".into(),
                })
            });
        }
        let content = self.to_content(&content);
        let Some(future) = self.storage.0.borrow().as_ref().map(|storage| {
            storage.commit_arrangement_edit(
                self.arrangement,
                content,
                point.kind,
                point.session,
                self.expected_parent.get(),
            )
        }) else {
            return Box::pin(async {
                Err(SaveFailure {
                    message: "storage has shut down".into(),
                })
            });
        };
        let expected_parent = self.expected_parent.clone();
        Box::pin(async move {
            match future.await {
                Ok((_, revision)) => {
                    expected_parent.set(revision.id);
                    Ok(())
                }
                Err(error) => Err(SaveFailure {
                    message: error.to_string().into(),
                }),
            }
        })
    }
}
