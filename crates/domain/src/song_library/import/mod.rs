//! Song import: the typed import document, its whole-document validation and
//! the Markdown song parser (T2.10).
//!
//! Both import routes produce one [`ImportDocument`]: a machine `.ordo`
//! document decoded by the storage exchange, and a human Markdown song parsed
//! by [`markdown::parse`]. The domain never sees JSON, ZIP or files; it takes
//! typed values and plain text only. Nothing here reads a clock, draws
//! randomness or performs I/O: callers pass identities in.

mod classify;
mod document;
mod evidence;
mod front_matter;
pub mod markdown;
mod plan;
mod validate;

pub use classify::{
    ArrangementReview, Classification, DocumentReview, InvalidReason, ResolvedSong, Review,
    SongIdentity, Staged, StagedDocument, classify_arrangements, classify_batch, same_content,
};
pub use evidence::{ArrangementDifference, ArrangementEvidence, DetailField, SongEvidence};
pub use plan::{
    Choices, ImportCommitted, ImportPlan, Outcome, PlanError, PlannedArrangement,
    PlannedReplacement, ReplacedArrangement, SongChoice, Tally, align_sections, can_commit, plan,
    plan_markdown, review_with_choices, tally,
};

pub use document::{
    ImportArrangement, ImportDocument, ImportSection, SequenceEntry, rendition_attribution,
    resolve_language,
};
pub use markdown::{IssueKind, MarkdownDraft, MarkdownIssue};
pub use validate::{DocumentProblem, validate_document};
