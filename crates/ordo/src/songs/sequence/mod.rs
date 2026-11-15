//! The Sequence task of the Arrangement editor: the performance order.
//!
//! State that belongs to one editor visit and is not content lives here: the
//! selected occurrence and slide, which occurrences are expanded, and the
//! sections the renderer could not compose. The words, names and order stay in
//! the one edit session, so every Sequence action is a structural edit of the
//! shared [`domain::song_library::ArrangementLyrics`]. [`readiness`] words and
//! draws what the editor header and the Sequence tab say about it.

pub mod readiness;
#[cfg(test)]
mod tests;

use domain::identity::OccurrenceId;

/// The occurrence and slide the Sequence task and the Inspector show.
///
/// Both tabs share it for a visit. It is keyed by identity, never by position,
/// so it survives a reorder, a collapse and a tab switch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceSelection {
    /// The selected occurrence.
    pub occurrence: OccurrenceId,
    /// The selected slide inside the occurrence's section (0-based).
    pub slide: usize,
}

impl SequenceSelection {
    /// The occurrence itself, on its first slide.
    pub fn occurrence(occurrence: OccurrenceId) -> Self {
        Self {
            occurrence,
            slide: 0,
        }
    }
}

/// A new occurrence identity, minted by the view as it mints section ones.
pub fn fresh_occurrence() -> OccurrenceId {
    OccurrenceId::from_entity(domain::identity::EntityId::from_random_bytes(
        *uuid::Uuid::new_v4().as_bytes(),
    ))
}
