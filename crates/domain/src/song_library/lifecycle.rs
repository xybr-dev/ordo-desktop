//! Lifecycle rules: which actions an entity offers, whether History may restore
//! a revision, and what selection follows an archive.
//!
//! These are pure content and view-state rules. [`SongLibrary`] keeps the
//! mutators; this module names the only lifecycle actions an overflow may
//! offer (Decision 145) and the selection rule the view
//! applies after an archive or a permanent deletion (Decision 16).
//!
//! [`SongLibrary`]: super::SongLibrary

use super::interface::ArrangementVisibility;
use super::model::{Arrangement, SongWork};

/// The only lifecycle actions an overflow may offer (Decision 145). There is no
/// merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleAction {
    /// Archive the entity, reversible.
    Archive,
    /// Restore an archived entity.
    Restore,
    /// Delete the entity permanently, after a reference-safety review.
    DeletePermanently,
}

/// The lifecycle actions one Song offers: **Archive** while active,
/// **Restore** and **Delete permanently…** while archived.
pub fn song_lifecycle_actions(song: &SongWork) -> &'static [LifecycleAction] {
    if song.is_archived() {
        &[LifecycleAction::Restore, LifecycleAction::DeletePermanently]
    } else {
        &[LifecycleAction::Archive]
    }
}

/// The lifecycle actions one Arrangement offers, decided by its own archive
/// state whatever its Song's.
pub fn arrangement_lifecycle_actions(arrangement: &Arrangement) -> &'static [LifecycleAction] {
    if arrangement.is_archived() {
        &[LifecycleAction::Restore, LifecycleAction::DeletePermanently]
    } else {
        &[LifecycleAction::Archive]
    }
}

/// Whether a comparison may offer **Restore this revision** (Decision 148).
///
/// Only an Arrangement that ordinary selection would offer may be restored: it
/// is archived itself, or only suppressed by its archived Song, is inspect-only.
pub fn can_restore_arrangement_revision(visibility: ArrangementVisibility) -> bool {
    visibility == ArrangementVisibility::Active
}

/// Whether a Song's History may offer **Restore this revision**. An archived
/// Song is inspect-only (Decision 148).
pub fn can_restore_song_revision(song: &SongWork) -> bool {
    !song.is_archived()
}

/// What selection should follow an archive or permanent deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AfterArchive<T> {
    /// Select this remaining entity.
    Select(T),
    /// Keep the current selection: Include archived still shows the removed
    /// Song.
    Keep,
    /// Nothing remains; show the empty state.
    EmptyState,
}

/// The selection to keep after `removed` leaves the visible list.
///
/// `visible` is the list as shown before the change, each entry with whether it
/// is active. In order: the next active result, then the previous active
/// result, then [`AfterArchive::Keep`] when `removed_stays_visible` (Include
/// archived keeps the archived Song visible), otherwise
/// [`AfterArchive::EmptyState`]. Archived neighbours are skipped.
pub fn selection_after_archive<T: Copy + Eq>(
    visible: &[(T, bool)],
    removed: T,
    removed_stays_visible: bool,
) -> AfterArchive<T> {
    let active = |entry: &&(T, bool)| entry.1 && entry.0 != removed;
    let chosen = match visible.iter().position(|(entity, _)| *entity == removed) {
        Some(index) => visible[index + 1..]
            .iter()
            .find(active)
            .or_else(|| visible[..index].iter().rev().find(active)),
        None => visible.iter().find(active),
    };
    match chosen {
        Some((entity, _)) => AfterArchive::Select(*entity),
        None if removed_stays_visible => AfterArchive::Keep,
        None => AfterArchive::EmptyState,
    }
}
