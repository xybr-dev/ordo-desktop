//! Durable Edit revisions (T2.2).
//!
//! Every committed edit point updates the Song or Arrangement and appends a
//! content-hashed revision linked to its parent in one immediate transaction.
//! Restoring an earlier revision commits its content as a new, later revision.
//! One linear chain exists per entity; history is read oldest first.

mod content;
mod read;
mod write;

use domain::identity::{HistorySessionId, RevisionId, Timestamp};
use domain::presentation::ContentHash;
use domain::revisions::{RevisionEntity, RevisionKind};

pub use content::{
    ArrangementContent, OccurrenceContent, RevisionContent, SectionContent, SongContent,
};
pub(crate) use write::append_creation;

/// The metadata of one stored revision; its content is read separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionRecord {
    /// Canonical UUIDv4 identity.
    pub id: RevisionId,
    /// The Song or Arrangement this revision belongs to.
    pub entity: RevisionEntity,
    /// The entity's previous latest revision; `None` for the creation revision.
    pub parent: Option<RevisionId>,
    /// Why the revision exists.
    pub kind: RevisionKind,
    /// The History session an `Edit` revision belongs to.
    pub session: Option<HistorySessionId>,
    /// The revision a `Restoration` brought back.
    pub restored_from: Option<RevisionId>,
    /// SHA-256 of the stored canonical content.
    pub content_sha256: ContentHash,
    /// When the revision was committed.
    pub created_at: Timestamp,
}
