//! Permanent deletion and its reference-safety facts.
//!
//! Deletion is one immediate transaction that re-checks every precondition the
//! domain verdict checked earlier: a reference can appear between the review
//! and the commit. Nothing cascades and nothing is tombstoned. Every statement
//! runs through [`WriteSteps`], so the fault-injection hook can stop the
//! deletion at any step and prove the whole transaction rolls back
//! (Decision 3).
//!
//! Choirs have no permanent deletion in T2.3; a Choir target is refused as
//! invalid input.

use std::future::Future;

use domain::identity::{ArrangementId, EntityId, SongId};
use domain::song_library::{DurableReference, RecordRef};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::parish_store::write::{WriteSteps, write};
use crate::song_references::durable_references;
use crate::worker::{self, Core};
use crate::{RecordKind, Storage, StorageError, map_error};

/// What a reference-safety review needs, read in one transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionFacts {
    /// Durable records that still name the target.
    pub references: Vec<DurableReference>,
    /// The Arrangements a Song still owns (active or archived); empty for an
    /// Arrangement.
    pub children: Vec<ArrangementId>,
    /// The target Arrangement's sections; 0 for a Song.
    pub sections: usize,
    /// The target Arrangement's Sequence occurrences; 0 for a Song.
    pub occurrences: usize,
    /// The stored revisions the deletion would erase.
    pub revisions: usize,
}

/// What one permanent deletion erased.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Deleted {
    /// How many revisions of its own History the deletion erased.
    pub revisions: usize,
}

impl Storage {
    /// Reads everything a reference-safety review shows, in one transaction:
    /// the durable references, a Song's remaining Arrangements, and the
    /// section, occurrence and revision counts.
    pub fn deletion_facts(
        &self,
        target: RecordRef,
    ) -> impl Future<Output = Result<DeletionFacts, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.deletion_facts(target)
        })
    }

    /// Permanently deletes one archived, unreferenced Arrangement and its
    /// sections, Sequence rows, links and revisions in one transaction.
    ///
    /// `NotArchived` when the Arrangement is active; `DeletionBlocked` when a
    /// durable reference exists (re-checked inside the deleting transaction);
    /// `NotFound` when it does not exist. Nothing is deleted on any refusal.
    pub fn delete_arrangement(
        &self,
        id: ArrangementId,
    ) -> impl Future<Output = Result<Deleted, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.delete_arrangement(id)
        })
    }

    /// Permanently deletes one archived, unreferenced Song that owns no
    /// Arrangement, with its aliases, moments, occasions and revisions in one
    /// transaction.
    ///
    /// `NotArchived` when the Song is active; `DeletionBlocked` when it still
    /// owns an Arrangement or a durable reference exists (both re-checked
    /// inside the deleting transaction); `NotFound` when it does not exist.
    /// Nothing is deleted on any refusal.
    pub fn delete_song(
        &self,
        id: SongId,
    ) -> impl Future<Output = Result<Deleted, StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.delete_song(id)
        })
    }

    /// Plants one durable reference to a Song or Arrangement for a test.
    #[cfg(feature = "fault-injection")]
    pub fn plant_reference(
        &self,
        target: RecordRef,
        reference: DurableReference,
    ) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.plant_reference(target, reference)
        })
    }

    /// Removes one planted durable reference.
    #[cfg(feature = "fault-injection")]
    pub fn remove_reference(
        &self,
        target: RecordRef,
        reference: DurableReference,
    ) -> impl Future<Output = Result<(), StorageError>> + use<> {
        worker::submit(self.tx.as_ref(), move |core: &mut Core| {
            core.remove_reference(target, reference)
        })
    }
}

impl Core {
    /// Reads the reference-safety facts in one read transaction.
    pub(crate) fn deletion_facts(
        &mut self,
        target: RecordRef,
    ) -> Result<DeletionFacts, StorageError> {
        let (conn, _fault, planted) = self.deletion_parts()?;
        let tx = conn.transaction().map_err(map_error)?;
        require_target(&tx, target)?;
        let references = durable_references(&tx, target, planted)?;
        let (children, sections, occurrences, revisions) = match target {
            RecordRef::Song(id) => (
                child_arrangements(&tx, id)?,
                0,
                0,
                song_revision_count(&tx, id)?,
            ),
            RecordRef::Arrangement(id) => (
                Vec::new(),
                section_count(&tx, id)?,
                occurrence_count(&tx, id)?,
                arrangement_revision_count(&tx, id)?,
            ),
            RecordRef::Choir(_) => unreachable_target()?,
        };
        Ok(DeletionFacts {
            references,
            children,
            sections,
            occurrences,
            revisions,
        })
    }

    /// Deletes one archived, unreferenced Arrangement and every row it owns.
    pub(crate) fn delete_arrangement(
        &mut self,
        id: ArrangementId,
    ) -> Result<Deleted, StorageError> {
        let target = RecordRef::Arrangement(id);
        let key = id.to_string();
        let (conn, fault, planted) = self.deletion_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        if !archived(&tx, target)? {
            return Err(StorageError::NotArchived {
                kind: RecordKind::Arrangement,
            });
        }
        let references = durable_references(&tx, target, planted)?;
        if !references.is_empty() {
            return Err(StorageError::DeletionBlocked {
                references,
                children: Vec::new(),
            });
        }
        let revisions = arrangement_revision_count(&tx, id)?;
        let mut steps = WriteSteps::new(fault);
        // Children first, then the revisions (their self-references resolve at
        // the end of the one statement), then the row itself.
        for sql in [
            "DELETE FROM section_occurrence WHERE arrangement_id = ?1",
            "DELETE FROM arrangement_section WHERE arrangement_id = ?1",
            "DELETE FROM arrangement_language WHERE arrangement_id = ?1",
            "DELETE FROM arrangement_choir WHERE arrangement_id = ?1",
            "DELETE FROM arrangement_occasion WHERE arrangement_id = ?1",
            "DELETE FROM edit_revision WHERE arrangement_id = ?1",
            "DELETE FROM arrangement WHERE id = ?1",
        ] {
            write(&tx, &mut steps, sql, params![key])?;
        }
        tx.commit().map_err(map_error)?;
        Ok(Deleted { revisions })
    }

    /// Deletes one archived, unreferenced Song that owns no Arrangement.
    pub(crate) fn delete_song(&mut self, id: SongId) -> Result<Deleted, StorageError> {
        let target = RecordRef::Song(id);
        let key = id.to_string();
        let (conn, fault, planted) = self.deletion_parts()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_error)?;
        if !archived(&tx, target)? {
            return Err(StorageError::NotArchived {
                kind: RecordKind::Song,
            });
        }
        let children = child_arrangements(&tx, id)?;
        let references = durable_references(&tx, target, planted)?;
        if !children.is_empty() || !references.is_empty() {
            return Err(StorageError::DeletionBlocked {
                references,
                children,
            });
        }
        let revisions = song_revision_count(&tx, id)?;
        let mut steps = WriteSteps::new(fault);
        for sql in [
            "DELETE FROM song_alias WHERE song_id = ?1",
            "DELETE FROM song_liturgical_moment WHERE song_id = ?1",
            "DELETE FROM song_occasion WHERE song_id = ?1",
            "DELETE FROM edit_revision WHERE song_id = ?1",
            "DELETE FROM song WHERE id = ?1",
        ] {
            write(&tx, &mut steps, sql, params![key])?;
        }
        tx.commit().map_err(map_error)?;
        Ok(Deleted { revisions })
    }
}

/// Refuses a Choir target; T2.3 gives Choirs no permanent deletion.
fn unreachable_target<T>() -> Result<T, StorageError> {
    Err(StorageError::InvalidInput {
        field: "target",
        reason: "a Choir cannot be deleted",
    })
}

/// Refuses a target row that does not exist.
fn require_target(tx: &Transaction<'_>, target: RecordRef) -> Result<(), StorageError> {
    archived(tx, target).map(|_| ())
}

/// Whether the target row exists and is archived; `NotFound` when missing.
fn archived(tx: &Transaction<'_>, target: RecordRef) -> Result<bool, StorageError> {
    let (sql, kind, id) = match target {
        RecordRef::Song(id) => (
            "SELECT archived_at FROM song WHERE id = ?1",
            RecordKind::Song,
            id.entity(),
        ),
        RecordRef::Arrangement(id) => (
            "SELECT archived_at FROM arrangement WHERE id = ?1",
            RecordKind::Arrangement,
            id.entity(),
        ),
        RecordRef::Choir(_) => return unreachable_target(),
    };
    let state = tx
        .query_row(sql, params![id.to_string()], |row| {
            row.get::<_, Option<String>>(0)
        })
        .optional()
        .map_err(map_error)?;
    match state {
        None => Err(StorageError::NotFound { kind, id }),
        Some(None) => Ok(false),
        Some(Some(_)) => Ok(true),
    }
}

/// The Arrangements a Song still owns, active or archived, in creation order.
fn child_arrangements(
    tx: &Transaction<'_>,
    song: SongId,
) -> Result<Vec<ArrangementId>, StorageError> {
    let mut statement = tx
        .prepare("SELECT id FROM arrangement WHERE song_id = ?1 ORDER BY created_at, id")
        .map_err(map_error)?;
    let rows = statement
        .query_map(params![song.to_string()], |row| row.get::<_, String>(0))
        .map_err(map_error)?;
    let mut children = Vec::new();
    for row in rows {
        let text = row.map_err(map_error)?;
        children.push(
            text.parse::<ArrangementId>()
                .map_err(|_| StorageError::Sqlite("corrupt arrangement id".to_owned()))?,
        );
    }
    Ok(children)
}

/// How many sections the Arrangement owns.
fn section_count(tx: &Transaction<'_>, id: ArrangementId) -> Result<usize, StorageError> {
    count_of(
        tx,
        "SELECT COUNT(*) FROM arrangement_section WHERE arrangement_id = ?1",
        id.entity(),
    )
}

/// How many Sequence occurrences the Arrangement owns.
fn occurrence_count(tx: &Transaction<'_>, id: ArrangementId) -> Result<usize, StorageError> {
    count_of(
        tx,
        "SELECT COUNT(*) FROM section_occurrence WHERE arrangement_id = ?1",
        id.entity(),
    )
}

/// How many revisions belong to the Song.
fn song_revision_count(tx: &Transaction<'_>, id: SongId) -> Result<usize, StorageError> {
    count_of(
        tx,
        "SELECT COUNT(*) FROM edit_revision WHERE song_id = ?1",
        id.entity(),
    )
}

/// How many revisions belong to the Arrangement.
fn arrangement_revision_count(
    tx: &Transaction<'_>,
    id: ArrangementId,
) -> Result<usize, StorageError> {
    count_of(
        tx,
        "SELECT COUNT(*) FROM edit_revision WHERE arrangement_id = ?1",
        id.entity(),
    )
}

/// One literal `COUNT(*)` query for one identity.
fn count_of(tx: &Transaction<'_>, sql: &'static str, id: EntityId) -> Result<usize, StorageError> {
    let value: i64 = tx
        .query_row(sql, params![id.to_string()], |row| row.get(0))
        .map_err(map_error)?;
    Ok(usize::try_from(value).unwrap_or(0))
}
