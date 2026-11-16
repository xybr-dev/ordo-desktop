//! Reference-safety rules for permanent deletion.
//!
//! The domain cannot see Mass plans, Presentation snapshots, or Celebration
//! packages; callers report the durable records that still name a Song or
//! Arrangement. This module turns the structural blockers and those reported
//! references into one verdict, and groups the blockers the way the deletion
//! dialog shows them.

use crate::identity::{ArrangementId, EntityId};

/// A durable record outside the Song Library that names a Song or Arrangement.
///
/// The domain cannot see Mass plans or snapshots; callers report them.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DurableReference {
    /// A Mass plan.
    MassPlan(EntityId),
    /// An immutable Presentation snapshot.
    PresentationSnapshot(EntityId),
    /// A Celebration package.
    CelebrationPackage(EntityId),
    /// Any other durable record kind.
    Other {
        /// The record kind, for display.
        kind: &'static str,
        /// The record's identity.
        id: EntityId,
    },
}

/// One reason permanent deletion is refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeletionBlocker {
    /// The entity is not archived; archive it first.
    NotArchived,
    /// The Song still owns Arrangements (active or archived); each needs its
    /// own review. Deletion never cascades.
    HasArrangements(Vec<ArrangementId>),
    /// Durable records still name the entity.
    Referenced(Vec<DurableReference>),
}

/// Whether permanent deletion may proceed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeletionVerdict {
    /// No blocker applies.
    Allowed,
    /// Every blocker that applies, in the order `NotArchived`,
    /// `HasArrangements`, `Referenced`.
    Blocked(Vec<DeletionBlocker>),
}

/// Blockers grouped for the reference-safety dialog, in display order; empty
/// groups omitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockerGroup {
    /// The entity is still active.
    NotArchived,
    /// The Song still owns these Arrangements.
    Arrangements(Vec<ArrangementId>),
    /// These Mass plans still name the entity.
    MassPlans(Vec<EntityId>),
    /// These Presentation snapshots still name the entity.
    PresentationSnapshots(Vec<EntityId>),
    /// These Celebration packages still name the entity.
    CelebrationPackages(Vec<EntityId>),
    /// Other durable records that still name the entity.
    Other(Vec<(&'static str, EntityId)>),
}

/// Builds a verdict from the structural blockers plus the references.
pub(crate) fn verdict(
    mut blockers: Vec<DeletionBlocker>,
    references: &[DurableReference],
) -> DeletionVerdict {
    if !references.is_empty() {
        let mut sorted = references.to_vec();
        sorted.sort();
        sorted.dedup();
        blockers.push(DeletionBlocker::Referenced(sorted));
    }
    if blockers.is_empty() {
        DeletionVerdict::Allowed
    } else {
        DeletionVerdict::Blocked(blockers)
    }
}

/// Groups blockers for display: NotArchived, Arrangements, Mass plans,
/// Presentation snapshots, Celebration packages, then other records.
///
/// Identities keep their input order and appear once; several `Referenced`
/// blockers merge into the same groups, and an empty group is omitted.
pub fn blocker_groups(blockers: &[DeletionBlocker]) -> Vec<BlockerGroup> {
    fn push_once<T: PartialEq>(list: &mut Vec<T>, item: T) {
        if !list.contains(&item) {
            list.push(item);
        }
    }
    let mut not_archived = false;
    let mut arrangements = Vec::new();
    let mut mass_plans = Vec::new();
    let mut snapshots = Vec::new();
    let mut packages = Vec::new();
    let mut other = Vec::new();
    for blocker in blockers {
        match blocker {
            DeletionBlocker::NotArchived => not_archived = true,
            DeletionBlocker::HasArrangements(ids) => {
                for id in ids {
                    push_once(&mut arrangements, *id);
                }
            }
            DeletionBlocker::Referenced(references) => {
                for reference in references {
                    match reference {
                        DurableReference::MassPlan(id) => push_once(&mut mass_plans, *id),
                        DurableReference::PresentationSnapshot(id) => {
                            push_once(&mut snapshots, *id);
                        }
                        DurableReference::CelebrationPackage(id) => {
                            push_once(&mut packages, *id);
                        }
                        DurableReference::Other { kind, id } => {
                            push_once(&mut other, (*kind, *id));
                        }
                    }
                }
            }
        }
    }
    let mut groups = Vec::new();
    if not_archived {
        groups.push(BlockerGroup::NotArchived);
    }
    if !arrangements.is_empty() {
        groups.push(BlockerGroup::Arrangements(arrangements));
    }
    if !mass_plans.is_empty() {
        groups.push(BlockerGroup::MassPlans(mass_plans));
    }
    if !snapshots.is_empty() {
        groups.push(BlockerGroup::PresentationSnapshots(snapshots));
    }
    if !packages.is_empty() {
        groups.push(BlockerGroup::CelebrationPackages(packages));
    }
    if !other.is_empty() {
        groups.push(BlockerGroup::Other(other));
    }
    groups
}
