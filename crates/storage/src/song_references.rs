//! Durable references to Song Library records.
//!
//! A Song or Arrangement may be named by records outside the Library: Mass
//! plans, immutable Presentation snapshots and Celebration packages. No such
//! table exists in Phase 2, so [`durable_references`] returns only the test
//! planted references held in the worker's memory. Phase 3 adds the Mass-plan,
//! snapshot and package queries here; the deletion review and the delete
//! transaction already call this one lookup, so neither changes then
//! (Decision 2).

use domain::song_library::{DurableReference, RecordRef};
use rusqlite::Transaction;

use crate::StorageError;
use crate::worker::Core;

/// Test-planted references; empty in a normal build (only the
/// fault-injection hooks write it).
#[derive(Clone, Debug, Default)]
pub(crate) struct PlantedReferences(Vec<(RecordRef, DurableReference)>);

impl PlantedReferences {
    /// Adds one reference; an identical pair is kept once.
    #[cfg(feature = "fault-injection")]
    pub(crate) fn insert(&mut self, target: RecordRef, reference: DurableReference) {
        if !self.0.contains(&(target, reference.clone())) {
            self.0.push((target, reference));
        }
    }

    /// Removes one reference; a missing pair changes nothing.
    #[cfg(feature = "fault-injection")]
    pub(crate) fn remove(&mut self, target: RecordRef, reference: &DurableReference) {
        self.0
            .retain(|(stored_target, stored)| stored_target != &target || stored != reference);
    }

    /// The planted references that name `target`, sorted and deduplicated.
    fn references_for(&self, target: RecordRef) -> Vec<DurableReference> {
        let mut references: Vec<DurableReference> = self
            .0
            .iter()
            .filter(|(stored_target, _)| *stored_target == target)
            .map(|(_, reference)| reference.clone())
            .collect();
        references.sort();
        references.dedup();
        references
    }
}

/// Every durable record that names the target.
///
/// Phase 2 has no table that can, so only planted ones are returned; Phase 3
/// adds the Mass-plan, snapshot and package queries here.
pub(crate) fn durable_references(
    tx: &Transaction<'_>,
    target: RecordRef,
    planted: &PlantedReferences,
) -> Result<Vec<DurableReference>, StorageError> {
    // The transaction stays in the signature because the Phase 3 queries read
    // through it; Phase 2 has nothing to read.
    let _ = tx;
    Ok(planted.references_for(target))
}

impl Core {
    /// Plants one durable reference for a test.
    #[cfg(feature = "fault-injection")]
    pub(crate) fn plant_reference(
        &mut self,
        target: RecordRef,
        reference: DurableReference,
    ) -> Result<(), StorageError> {
        require_reference_target(target)?;
        self.planted_mut().insert(target, reference);
        Ok(())
    }

    /// Removes one planted durable reference.
    #[cfg(feature = "fault-injection")]
    pub(crate) fn remove_reference(
        &mut self,
        target: RecordRef,
        reference: DurableReference,
    ) -> Result<(), StorageError> {
        require_reference_target(target)?;
        self.planted_mut().remove(target, &reference);
        Ok(())
    }
}

/// Only a Song or Arrangement can be named by a durable reference.
#[cfg(feature = "fault-injection")]
fn require_reference_target(target: RecordRef) -> Result<(), StorageError> {
    if matches!(target, RecordRef::Choir(_)) {
        return Err(StorageError::InvalidInput {
            field: "target",
            reason: "a Choir cannot be referenced",
        });
    }
    Ok(())
}
