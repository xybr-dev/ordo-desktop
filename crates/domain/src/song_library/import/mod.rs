//! Song import: the typed import document, its whole-document validation and
//! the Markdown song parser (T2.10).
//!
//! Both import routes produce one [`ImportDocument`]: a machine `.ordo`
//! document decoded by the storage exchange, and a human Markdown song parsed
//! by [`markdown::parse`]. The domain never sees JSON, ZIP or files; it takes
//! typed values and plain text only. Nothing here reads a clock, draws
//! randomness or performs I/O: callers pass identities in.

mod document;
mod front_matter;
pub mod markdown;
mod validate;

pub use document::{
    ImportArrangement, ImportDocument, ImportSection, SequenceEntry, rendition_attribution,
    resolve_language,
};
pub use markdown::{IssueKind, MarkdownDraft, MarkdownIssue};
pub use validate::{DocumentProblem, validate_document};
