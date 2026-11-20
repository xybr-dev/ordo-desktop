//! Checking and taking an import plan (T2.10).
//!
//! Both operations are all or nothing: they work on a copy and replace the
//! library only when every step passed.

use crate::identity::Timestamp;

use super::super::import::{ImportCommitted, ImportPlan};
use super::{SongLibrary, SongLibraryError};

impl SongLibrary {
    /// Checks a plan against the whole library without changing it: ids free,
    /// Songs active, Choirs active, languages listed, every text rule.
    ///
    /// The steps are the commit's: Choirs, Songs, new Arrangements, then
    /// replacements. The first refusal is returned.
    ///
    /// # Errors
    ///
    /// The `SongLibraryError` the first refused step would give, for example
    /// `DuplicateArrangement`, `Archived`, `UnknownSong` or
    /// `UnlistedLanguage`.
    pub fn check_import(&self, plan: &ImportPlan) -> Result<(), SongLibraryError> {
        let mut trial = self.clone();
        let at = Timestamp::from_millis(0);
        for (id, draft) in &plan.choirs {
            trial.create_choir(*id, at, draft.clone())?;
        }
        for (id, draft) in &plan.songs {
            trial.create_song(*id, at, draft.clone())?;
        }
        for planned in &plan.arrangements {
            trial.create_arrangement(
                planned.id,
                planned.song,
                at,
                planned.draft.clone(),
                planned.choirs.clone(),
            )?;
        }
        for replacement in &plan.replacements {
            trial.edit_arrangement(replacement.id, replacement.draft.clone())?;
            trial.set_arrangement_choirs(replacement.id, replacement.choirs.clone())?;
        }
        Ok(())
    }

    /// Takes a committed plan as storage returned it, all or nothing
    /// (hydration, like [`Self::refresh_arrangement`]): content, Choirs and
    /// each section's pagination state are taken as stored.
    ///
    /// # Errors
    ///
    /// The `SongLibraryError` of the first stored value that breaks an
    /// invariant; the library is then unchanged.
    pub fn apply_import(&mut self, committed: ImportCommitted) -> Result<(), SongLibraryError> {
        let mut next = self.clone();
        for stored in committed.choirs {
            if next.choirs.contains_key(&stored.id) {
                return Err(SongLibraryError::DuplicateChoir(stored.id));
            }
            let choir =
                super::super::model::Choir::new(stored.id, stored.created_at, stored.draft)?
                    .with_archive_state(stored.archive_state);
            next.choirs.insert(stored.id, choir);
        }
        for stored in committed.songs {
            if next.songs.contains_key(&stored.id) {
                return Err(SongLibraryError::DuplicateSong(stored.id));
            }
            let song =
                super::super::model::SongWork::new(stored.id, stored.created_at, stored.draft)?
                    .with_archive_state(stored.archive_state);
            next.songs.insert(stored.id, song);
        }
        for stored in committed.arrangements {
            next.hydrate_arrangement(stored)
                .map_err(|error| error.cause)?;
        }
        for replaced in committed.replaced {
            next.refresh_arrangement(replaced.id, replaced.draft, replaced.choirs)?;
        }
        *self = next;
        Ok(())
    }
}
