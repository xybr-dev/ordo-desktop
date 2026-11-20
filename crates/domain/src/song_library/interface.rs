//! The Song Library Module Interface.
//!
//! [`SongLibrary`] is the only way to add, change, or read the Song works,
//! Arrangements, and Choirs of one Parish. It owns their identities and
//! authored content in memory and refuses a change that would break one of its
//! invariants, so a caller never has to repeat validation.
//!
//! The Interface is pure and deterministic. Callers draw identities from
//! [`crate::identity`] and pass creation times in; the library reads no clock,
//! generates no identity, and performs no I/O.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::identity::{
    ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp,
};

use super::model::text;
use super::model::{Arrangement, ArrangementDraft, Choir, ChoirDraft, SongDraft, SongWork};

mod hydrate;

pub use hydrate::{HydrateArrangement, HydrateChoir, HydrateReserved, HydrateSong, HydrationError};

/// One authored field that failed a Song Library validation rule.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Field {
    /// A Song work's primary title.
    SongTitle,
    /// The alternate titles of a Song work.
    Aliases,
    /// A Song work's original attribution.
    OriginalAttribution,
    /// A Song work's provenance.
    Provenance,
    /// A Song work's rights notes.
    RightsNotes,
    /// One liturgical moment of a Song work.
    LiturgicalMoment,
    /// One occasion applicability tag.
    Occasion,
    /// A Choir's name.
    ChoirName,
    /// A Choir's short label.
    ChoirShortLabel,
    /// One preferred language of a Choir.
    ChoirLanguage,
    /// A Choir's private notes.
    ChoirNotes,
    /// An Arrangement's title.
    ArrangementTitle,
    /// One language of an Arrangement.
    Language,
    /// An Arrangement's rendition attribution.
    RenditionAttribution,
    /// One section label.
    SectionLabel,
    /// One Choir association of an Arrangement.
    Choir,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SongTitle => "Song title",
            Self::Aliases => "alias",
            Self::OriginalAttribution => "original attribution",
            Self::Provenance => "provenance",
            Self::RightsNotes => "rights notes",
            Self::LiturgicalMoment => "liturgical moment",
            Self::Occasion => "occasion",
            Self::ChoirName => "Choir name",
            Self::ChoirShortLabel => "Choir short label",
            Self::ChoirLanguage => "Choir language",
            Self::ChoirNotes => "Choir notes",
            Self::ArrangementTitle => "Arrangement title",
            Self::Language => "language",
            Self::RenditionAttribution => "rendition attribution",
            Self::SectionLabel => "section label",
            Self::Choir => "Choir association",
        })
    }
}

/// Why a Song Library operation was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SongLibraryError {
    /// No Song work has this identity.
    UnknownSong(SongId),
    /// No Arrangement has this identity.
    UnknownArrangement(ArrangementId),
    /// No Choir has this identity.
    UnknownChoir(ChoirId),
    /// A Song work already has this identity.
    DuplicateSong(SongId),
    /// An Arrangement already has this identity.
    DuplicateArrangement(ArrangementId),
    /// A Choir already has this identity.
    DuplicateChoir(ChoirId),
    /// A section already has this identity.
    DuplicateSection(SectionId),
    /// An occurrence already has this identity.
    DuplicateOccurrence(OccurrenceId),
    /// An authored value that is present but empty or only whitespace.
    BlankField(Field),
    /// A field contains a value that duplicates another value it may not
    /// repeat.
    DuplicateField(Field),
    /// An occurrence names a section that is not one of its Arrangement's own
    /// sections.
    SectionNotInArrangement {
        /// The Arrangement whose Sequence holds the occurrence.
        arrangement: ArrangementId,
        /// The section the occurrence names.
        section: SectionId,
    },
    /// The entity is already archived.
    AlreadyArchived(RecordRef),
    /// The entity is active, so there is nothing to restore.
    NotArchived(RecordRef),
    /// The entity (or, for an Arrangement, its Song) is archived and therefore
    /// inspect-only until restored.
    Archived(RecordRef),
    /// Permanent deletion is refused; nothing changed. Lists every blocker.
    DeletionBlocked(Vec<DeletionBlocker>),
}

impl fmt::Display for SongLibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSong(id) => write!(f, "no Song work has identity {id}"),
            Self::UnknownArrangement(id) => write!(f, "no Arrangement has identity {id}"),
            Self::UnknownChoir(id) => write!(f, "no Choir has identity {id}"),
            Self::DuplicateSong(id) => write!(f, "a Song work with identity {id} already exists"),
            Self::DuplicateArrangement(id) => {
                write!(f, "an Arrangement with identity {id} already exists")
            }
            Self::DuplicateChoir(id) => write!(f, "a Choir with identity {id} already exists"),
            Self::DuplicateSection(id) => write!(f, "a section with identity {id} already exists"),
            Self::DuplicateOccurrence(id) => {
                write!(f, "an occurrence with identity {id} already exists")
            }
            Self::BlankField(field) => write!(f, "{field} is blank"),
            Self::DuplicateField(field) => write!(f, "{field} repeats a value"),
            Self::SectionNotInArrangement {
                arrangement,
                section,
            } => write!(
                f,
                "occurrence section {section} does not belong to Arrangement {arrangement}"
            ),
            Self::AlreadyArchived(record) => write!(f, "{record} is already archived"),
            Self::NotArchived(record) => write!(f, "{record} is not archived"),
            Self::Archived(record) => {
                write!(f, "{record} is archived and inspect-only until restored")
            }
            Self::DeletionBlocked(blockers) => {
                write!(f, "permanent deletion is blocked: {blockers:?}")
            }
        }
    }
}

impl std::error::Error for SongLibraryError {}

/// Names one archivable Song Library record in an error or verdict.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecordRef {
    /// A Song work.
    Song(SongId),
    /// An Arrangement.
    Arrangement(ArrangementId),
    /// A Choir.
    Choir(ChoirId),
}

impl fmt::Display for RecordRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Song(id) => write!(f, "Song work {id}"),
            Self::Arrangement(id) => write!(f, "Arrangement {id}"),
            Self::Choir(id) => write!(f, "Choir {id}"),
        }
    }
}

/// How one Arrangement appears to ordinary planning and selection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ArrangementVisibility {
    /// The Arrangement and its Song are both active.
    Active,
    /// The Arrangement is archived in its own right, whatever its Song's state.
    Archived,
    /// The Arrangement is itself active but its Song is archived.
    SuppressedBySong,
}

/// Which archive states a Song listing includes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StatusFilter {
    /// Only active entities.
    Active,
    /// Only archived entities.
    Archived,
    /// Both.
    All,
}

impl StatusFilter {
    /// Whether an entity whose archived flag is `archived` passes the filter.
    const fn includes(self, archived: bool) -> bool {
        match self {
            Self::Active => !archived,
            Self::Archived => archived,
            Self::All => true,
        }
    }
}

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

/// Case-insensitive match key for duplicate evidence.
///
/// Unicode lowercase (`str::to_lowercase`), whitespace runs collapsed to one
/// space, trimmed. Nothing else is folded: digits, ordinals, punctuation and
/// diacritics stay significant, so "Kyrie — 8th Mass" never matches
/// "Kyrie — 13th Mass".
pub fn match_key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The Song works, Arrangements, and Choirs of one Parish.
///
/// The library is an in-memory aggregate: a Parish holds hundreds of Songs,
/// not millions, so `BTreeMap`s keyed by typed identity give deterministic
/// ordering without an index. Every collection the Interface returns is in
/// identity order unless its own documentation says otherwise.
///
/// The Interface deliberately has no merge and no redirect operation: similar
/// titles never unite two Song works automatically, and numbered settings stay
/// separate works.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SongLibrary {
    songs: BTreeMap<SongId, SongWork>,
    arrangements: BTreeMap<ArrangementId, Arrangement>,
    choirs: BTreeMap<ChoirId, Choir>,
    /// Reserved section identities and the Arrangement that first used each.
    /// An entry is never removed, not by an edit that drops the section and
    /// not by deleting the Arrangement, because earlier revisions and snapshots
    /// may still name it.
    section_ids: BTreeMap<SectionId, ArrangementId>,
    /// Reserved occurrence identities, with the same rules as `section_ids`.
    occurrence_ids: BTreeMap<OccurrenceId, ArrangementId>,
}

impl SongLibrary {
    /// An empty library.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every Song work, in identity order.
    pub fn songs(&self) -> impl Iterator<Item = &SongWork> {
        self.songs.values()
    }

    /// The Song works passing `filter`, in creation-time then identity order.
    pub fn songs_with_status(&self, filter: StatusFilter) -> Vec<&SongWork> {
        let mut songs: Vec<&SongWork> = self
            .songs
            .values()
            .filter(|song| filter.includes(song.is_archived()))
            .collect();
        songs.sort_by_key(|song| (song.created_at(), song.id()));
        songs
    }

    /// One Song work.
    pub fn song(&self, id: SongId) -> Result<&SongWork, SongLibraryError> {
        self.songs.get(&id).ok_or(SongLibraryError::UnknownSong(id))
    }

    /// Creates one Song work and returns it.
    ///
    /// Only the primary title is required. The identity must be unused; the
    /// library never regenerates an identity or unites a title with another
    /// work.
    pub fn create_song(
        &mut self,
        id: SongId,
        created_at: Timestamp,
        draft: SongDraft,
    ) -> Result<&SongWork, SongLibraryError> {
        if self.songs.contains_key(&id) {
            return Err(SongLibraryError::DuplicateSong(id));
        }
        self.songs.insert(id, SongWork::new(id, created_at, draft)?);
        self.song(id)
    }

    /// Replaces the authored content of one Song work.
    ///
    /// Identity, creation time, and archive state are preserved. An archived
    /// Song is inspect-only: editing it is refused with
    /// [`SongLibraryError::Archived`] until it is restored.
    ///
    /// Editing never consults [`Self::duplicate_evidence`] and never changes
    /// authored capitalization.
    pub fn edit_song(
        &mut self,
        id: SongId,
        draft: SongDraft,
    ) -> Result<&SongWork, SongLibraryError> {
        let song = self
            .songs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownSong(id))?;
        if song.is_archived() {
            return Err(SongLibraryError::Archived(RecordRef::Song(id)));
        }
        song.replace_content(draft)?;
        Ok(song)
    }

    /// Every Arrangement, in identity order.
    pub fn arrangements(&self) -> impl Iterator<Item = &Arrangement> {
        self.arrangements.values()
    }

    /// One Arrangement.
    pub fn arrangement(&self, id: ArrangementId) -> Result<&Arrangement, SongLibraryError> {
        self.arrangements
            .get(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))
    }

    /// Every Arrangement of one Song work, in creation-time then identity
    /// order.
    pub fn arrangements_for_song(
        &self,
        song: SongId,
    ) -> Result<Vec<&Arrangement>, SongLibraryError> {
        if !self.songs.contains_key(&song) {
            return Err(SongLibraryError::UnknownSong(song));
        }
        Ok(self
            .arrangements_in_creation_order()
            .into_iter()
            .filter(|arrangement| arrangement.song() == song)
            .collect())
    }

    /// Every exact Arrangement one Choir sings, in creation-time then identity
    /// order.
    pub fn choir_arrangements(
        &self,
        choir: ChoirId,
    ) -> Result<Vec<&Arrangement>, SongLibraryError> {
        if !self.choirs.contains_key(&choir) {
            return Err(SongLibraryError::UnknownChoir(choir));
        }
        Ok(self
            .arrangements_in_creation_order()
            .into_iter()
            .filter(|arrangement| arrangement.choirs().contains(&choir))
            .collect())
    }

    /// Every Arrangement in creation-time then identity order.
    fn arrangements_in_creation_order(&self) -> Vec<&Arrangement> {
        let mut arrangements: Vec<&Arrangement> = self.arrangements.values().collect();
        arrangements.sort_by_key(|arrangement| (arrangement.created_at(), arrangement.id()));
        arrangements
    }

    /// Creates one Arrangement of `song` and returns it.
    ///
    /// The identity must be unused, the Song must exist, every Choir in
    /// `choirs` must exist and be distinct, and every section and occurrence
    /// identity must be free. A new Arrangement starts with
    /// [`super::PaginationState::Draft`] pagination and an active archive state.
    /// An archived Song accepts no new Arrangement ([`SongLibraryError::Archived`]
    /// naming the Song), and an archived Choir cannot be associated.
    ///
    /// `draft` carries authored content only; Choir associations change only
    /// through [`Self::set_arrangement_choirs`],
    /// [`Self::add_arrangement_choir`], and
    /// [`Self::remove_arrangement_choir`].
    pub fn create_arrangement(
        &mut self,
        id: ArrangementId,
        song: SongId,
        created_at: Timestamp,
        draft: ArrangementDraft,
        choirs: Vec<ChoirId>,
    ) -> Result<&Arrangement, SongLibraryError> {
        if self.arrangements.contains_key(&id) {
            return Err(SongLibraryError::DuplicateArrangement(id));
        }
        let owner = self
            .songs
            .get(&song)
            .ok_or(SongLibraryError::UnknownSong(song))?;
        if owner.is_archived() {
            return Err(SongLibraryError::Archived(RecordRef::Song(song)));
        }
        text::require_distinct(Field::Choir, &choirs)?;
        self.require_choirs_exist(&choirs)?;
        for choir in &choirs {
            self.require_choir_active(*choir)?;
        }
        Self::require_free_section_and_occurrence_ids(
            &self.section_ids,
            &self.occurrence_ids,
            &draft,
            id,
        )?;
        let arrangement = Arrangement::new(id, song, created_at, draft, choirs)?;
        self.section_ids
            .extend(arrangement.sections().iter().map(|s| (s.id(), id)));
        self.occurrence_ids
            .extend(arrangement.sequence().iter().map(|o| (o.id(), id)));
        self.arrangements.insert(id, arrangement);
        self.arrangement(id)
    }

    /// Replaces the authored content of one Arrangement.
    ///
    /// Refused with [`SongLibraryError::Archived`] when the Arrangement is
    /// archived (naming it) or its Song is archived (naming the Song).
    ///
    /// The owning Song, identity, creation time, Choir associations, and
    /// archive state are preserved.
    ///
    /// Section and occurrence identities are reserved for the Arrangement that
    /// first used them, permanently: an identity this edit drops stays
    /// reserved to this Arrangement (earlier revisions still name it, and
    /// restoring such a revision must never collide with another
    /// Arrangement). A later edit of this same Arrangement may bring a dropped
    /// identity back; any other Arrangement is refused with
    /// `DuplicateSection` or `DuplicateOccurrence`.
    ///
    /// Adding or removing a section, changing a section's lyrics or label, or
    /// changing the Sequence moves an approved Arrangement to
    /// [`super::PaginationState::NeedsReview`]. Title, language, attribution,
    /// occasion, and section-order changes do not.
    pub fn edit_arrangement(
        &mut self,
        id: ArrangementId,
        draft: ArrangementDraft,
    ) -> Result<&Arrangement, SongLibraryError> {
        let arrangement = self
            .arrangements
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))?;
        Self::require_editable(&self.songs, arrangement)?;
        Self::require_free_section_and_occurrence_ids(
            &self.section_ids,
            &self.occurrence_ids,
            &draft,
            id,
        )?;
        arrangement.replace_content(draft)?;
        self.section_ids
            .extend(arrangement.sections().iter().map(|s| (s.id(), id)));
        self.occurrence_ids
            .extend(arrangement.sequence().iter().map(|o| (o.id(), id)));
        Ok(arrangement)
    }

    /// Every Choir, in identity order.
    pub fn choirs(&self) -> impl Iterator<Item = &Choir> {
        self.choirs.values()
    }

    /// One Choir.
    pub fn choir(&self, id: ChoirId) -> Result<&Choir, SongLibraryError> {
        self.choirs
            .get(&id)
            .ok_or(SongLibraryError::UnknownChoir(id))
    }

    /// Creates one Choir and returns it.
    ///
    /// A Choir needs only a nonblank name; the short label, preferred
    /// languages and notes are optional details (blank text and repeated
    /// languages are refused). The identity must be unused.
    pub fn create_choir(
        &mut self,
        id: ChoirId,
        created_at: Timestamp,
        draft: ChoirDraft,
    ) -> Result<&Choir, SongLibraryError> {
        if self.choirs.contains_key(&id) {
            return Err(SongLibraryError::DuplicateChoir(id));
        }
        self.choirs.insert(id, Choir::new(id, created_at, draft)?);
        self.choir(id)
    }

    /// Replaces one Choir's details.
    ///
    /// Identity, creation time, and archive state are preserved. An archived
    /// Choir is inspect-only ([`SongLibraryError::Archived`]).
    pub fn edit_choir(
        &mut self,
        id: ChoirId,
        draft: ChoirDraft,
    ) -> Result<&Choir, SongLibraryError> {
        let choir = self
            .choirs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownChoir(id))?;
        if choir.is_archived() {
            return Err(SongLibraryError::Archived(RecordRef::Choir(id)));
        }
        choir.replace_details(draft)?;
        Ok(choir)
    }

    /// Replaces the Choir associations of one Arrangement.
    ///
    /// The inspect-only rules of [`Self::edit_arrangement`] apply. An archived
    /// Choir cannot be newly associated ([`SongLibraryError::Archived`]), but
    /// an archived Choir that is already associated may stay in the set.
    ///
    /// Duplicate Choir identities and unknown Choirs are refused. The
    /// Arrangement may end with no Choir.
    pub fn set_arrangement_choirs(
        &mut self,
        id: ArrangementId,
        choirs: Vec<ChoirId>,
    ) -> Result<(), SongLibraryError> {
        let arrangement = self
            .arrangements
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))?;
        Self::require_editable(&self.songs, arrangement)?;
        text::require_distinct(Field::Choir, &choirs)?;
        for choir in &choirs {
            let stored = self
                .choirs
                .get(choir)
                .ok_or(SongLibraryError::UnknownChoir(*choir))?;
            if stored.is_archived() && !arrangement.choirs().contains(choir) {
                return Err(SongLibraryError::Archived(RecordRef::Choir(*choir)));
            }
        }
        arrangement.replace_choirs(choirs);
        Ok(())
    }

    /// Associates one more Choir with an Arrangement.
    ///
    /// Association is set-shaped: adding a Choir that is already associated
    /// succeeds without changing the Arrangement.
    pub fn add_arrangement_choir(
        &mut self,
        arrangement: ArrangementId,
        choir: ChoirId,
    ) -> Result<(), SongLibraryError> {
        let stored = self
            .arrangements
            .get_mut(&arrangement)
            .ok_or(SongLibraryError::UnknownArrangement(arrangement))?;
        Self::require_editable(&self.songs, stored)?;
        let known = self
            .choirs
            .get(&choir)
            .ok_or(SongLibraryError::UnknownChoir(choir))?;
        if known.is_archived() && !stored.choirs().contains(&choir) {
            return Err(SongLibraryError::Archived(RecordRef::Choir(choir)));
        }
        stored.add_choir(choir);
        Ok(())
    }

    /// Drops one Choir association from an Arrangement.
    ///
    /// A Choir that is not associated succeeds without changing the
    /// Arrangement, so the caller can remove membership without a preceding
    /// read.
    pub fn remove_arrangement_choir(
        &mut self,
        arrangement: ArrangementId,
        choir: ChoirId,
    ) -> Result<(), SongLibraryError> {
        let arrangement = self
            .arrangements
            .get_mut(&arrangement)
            .ok_or(SongLibraryError::UnknownArrangement(arrangement))?;
        Self::require_editable(&self.songs, arrangement)?;
        arrangement.remove_choir(choir);
        Ok(())
    }

    /// The Song works in one Choir's repertoire, in identity order.
    ///
    /// Repertoire is derived membership: a Song is in the Choir's repertoire
    /// when at least one of its Arrangements names that Choir. Every Song
    /// appears once, however many qualifying Arrangements it owns.
    pub fn choir_repertoire(&self, choir: ChoirId) -> Result<Vec<&SongWork>, SongLibraryError> {
        if !self.choirs.contains_key(&choir) {
            return Err(SongLibraryError::UnknownChoir(choir));
        }
        let songs: BTreeSet<SongId> = self
            .arrangements
            .values()
            .filter(|arrangement| arrangement.choirs().contains(&choir))
            .map(Arrangement::song)
            .collect();
        Ok(self
            .songs
            .values()
            .filter(|song| songs.contains(&song.id()))
            .collect())
    }

    /// Approves the current pagination of one Arrangement.
    ///
    /// Approving an already-approved Arrangement succeeds without a change.
    pub fn approve_pagination(&mut self, id: ArrangementId) -> Result<(), SongLibraryError> {
        let arrangement = self
            .arrangements
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))?;
        Self::require_editable(&self.songs, arrangement)?;
        arrangement.approve_pagination();
        Ok(())
    }

    /// Archives one Song work at `at`. Reversible; never touches its
    /// Arrangements, whose own archive states stay exactly as they were.
    pub fn archive_song(&mut self, id: SongId, at: Timestamp) -> Result<(), SongLibraryError> {
        let song = self
            .songs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownSong(id))?;
        if song.is_archived() {
            return Err(SongLibraryError::AlreadyArchived(RecordRef::Song(id)));
        }
        song.archive(at);
        Ok(())
    }

    /// Restores one archived Song work. Only Arrangements that were
    /// independently active become selectable again, because archiving the Song
    /// never changed them.
    pub fn restore_song(&mut self, id: SongId) -> Result<(), SongLibraryError> {
        let song = self
            .songs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownSong(id))?;
        if !song.is_archived() {
            return Err(SongLibraryError::NotArchived(RecordRef::Song(id)));
        }
        song.restore();
        Ok(())
    }

    /// Archives one Arrangement at `at`. Reversible; the Song is untouched.
    pub fn archive_arrangement(
        &mut self,
        id: ArrangementId,
        at: Timestamp,
    ) -> Result<(), SongLibraryError> {
        let arrangement = self
            .arrangements
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))?;
        if arrangement.is_archived() {
            return Err(SongLibraryError::AlreadyArchived(RecordRef::Arrangement(
                id,
            )));
        }
        arrangement.archive(at);
        Ok(())
    }

    /// Restores one archived Arrangement. If its Song is archived it stays
    /// suppressed until the Song is restored.
    pub fn restore_arrangement(&mut self, id: ArrangementId) -> Result<(), SongLibraryError> {
        let arrangement = self
            .arrangements
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))?;
        if !arrangement.is_archived() {
            return Err(SongLibraryError::NotArchived(RecordRef::Arrangement(id)));
        }
        arrangement.restore();
        Ok(())
    }

    /// Archives one Choir at `at`. Reversible; its associations stay and no
    /// Arrangement is invalidated.
    pub fn archive_choir(&mut self, id: ChoirId, at: Timestamp) -> Result<(), SongLibraryError> {
        let choir = self
            .choirs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownChoir(id))?;
        if choir.is_archived() {
            return Err(SongLibraryError::AlreadyArchived(RecordRef::Choir(id)));
        }
        choir.archive(at);
        Ok(())
    }

    /// Restores one archived Choir.
    pub fn restore_choir(&mut self, id: ChoirId) -> Result<(), SongLibraryError> {
        let choir = self
            .choirs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownChoir(id))?;
        if !choir.is_archived() {
            return Err(SongLibraryError::NotArchived(RecordRef::Choir(id)));
        }
        choir.restore();
        Ok(())
    }

    /// How one Arrangement appears to ordinary selection.
    pub fn arrangement_visibility(
        &self,
        id: ArrangementId,
    ) -> Result<ArrangementVisibility, SongLibraryError> {
        let arrangement = self.arrangement(id)?;
        if arrangement.is_archived() {
            return Ok(ArrangementVisibility::Archived);
        }
        if self.song(arrangement.song())?.is_archived() {
            return Ok(ArrangementVisibility::SuppressedBySong);
        }
        Ok(ArrangementVisibility::Active)
    }

    /// Whether ordinary planning and selection may offer the Arrangement: it is
    /// active and so is its Song.
    pub fn is_selectable(&self, id: ArrangementId) -> Result<bool, SongLibraryError> {
        Ok(self.arrangement_visibility(id)? == ArrangementVisibility::Active)
    }

    /// The selectable Arrangements of one Song, in creation-time then identity
    /// order. Empty when the Song is archived.
    pub fn selectable_arrangements(
        &self,
        song: SongId,
    ) -> Result<Vec<&Arrangement>, SongLibraryError> {
        if self.song(song)?.is_archived() {
            return Ok(Vec::new());
        }
        Ok(self
            .arrangements_in_creation_order()
            .into_iter()
            .filter(|arrangement| arrangement.song() == song && !arrangement.is_archived())
            .collect())
    }

    /// The selectable Arrangements one Choir sings, in creation-time then
    /// identity order.
    ///
    /// The Choir itself may be archived: repertoire membership is not
    /// availability, so an archived Choir still lists what it sings.
    pub fn selectable_choir_arrangements(
        &self,
        choir: ChoirId,
    ) -> Result<Vec<&Arrangement>, SongLibraryError> {
        self.choir(choir)?;
        Ok(self
            .arrangements_in_creation_order()
            .into_iter()
            .filter(|arrangement| {
                arrangement.choirs().contains(&choir)
                    && !arrangement.is_archived()
                    && self
                        .songs
                        .get(&arrangement.song())
                        .is_some_and(|song| !song.is_archived())
            })
            .collect())
    }

    /// Reference-safety verdict for permanently deleting a Song.
    ///
    /// Allowed only when the Song is archived, owns no Arrangement at all
    /// (active or archived; deletion never cascades) and `references` is empty.
    pub fn song_deletion_verdict(
        &self,
        id: SongId,
        references: &[DurableReference],
    ) -> Result<DeletionVerdict, SongLibraryError> {
        let song = self.song(id)?;
        let mut blockers = Vec::new();
        if !song.is_archived() {
            blockers.push(DeletionBlocker::NotArchived);
        }
        let owned: Vec<ArrangementId> = self
            .arrangements_in_creation_order()
            .into_iter()
            .filter(|arrangement| arrangement.song() == id)
            .map(Arrangement::id)
            .collect();
        if !owned.is_empty() {
            blockers.push(DeletionBlocker::HasArrangements(owned));
        }
        Ok(Self::verdict(blockers, references))
    }

    /// Reference-safety verdict for permanently deleting an Arrangement.
    ///
    /// Allowed only when the Arrangement is archived in its own right (its
    /// Song's state is irrelevant) and `references` is empty.
    pub fn arrangement_deletion_verdict(
        &self,
        id: ArrangementId,
        references: &[DurableReference],
    ) -> Result<DeletionVerdict, SongLibraryError> {
        let arrangement = self.arrangement(id)?;
        let mut blockers = Vec::new();
        if !arrangement.is_archived() {
            blockers.push(DeletionBlocker::NotArchived);
        }
        Ok(Self::verdict(blockers, references))
    }

    /// Permanently deletes one Song when [`Self::song_deletion_verdict`]
    /// allows it; otherwise refuses with [`SongLibraryError::DeletionBlocked`]
    /// and changes nothing.
    pub fn delete_song(
        &mut self,
        id: SongId,
        references: &[DurableReference],
    ) -> Result<SongWork, SongLibraryError> {
        if let DeletionVerdict::Blocked(blockers) = self.song_deletion_verdict(id, references)? {
            return Err(SongLibraryError::DeletionBlocked(blockers));
        }
        self.songs
            .remove(&id)
            .ok_or(SongLibraryError::UnknownSong(id))
    }

    /// Permanently deletes one Arrangement when
    /// [`Self::arrangement_deletion_verdict`] allows it; otherwise refuses with
    /// [`SongLibraryError::DeletionBlocked`] and changes nothing.
    ///
    /// The deleted Arrangement's section and occurrence identities stay
    /// reserved: revisions and snapshots may still name them, so no later
    /// Arrangement can reuse them.
    pub fn delete_arrangement(
        &mut self,
        id: ArrangementId,
        references: &[DurableReference],
    ) -> Result<Arrangement, SongLibraryError> {
        if let DeletionVerdict::Blocked(blockers) =
            self.arrangement_deletion_verdict(id, references)?
        {
            return Err(SongLibraryError::DeletionBlocked(blockers));
        }
        self.arrangements
            .remove(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))
    }

    /// Builds a verdict from the structural blockers plus the references.
    fn verdict(
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

    /// Refuses an Arrangement that is archived, or whose Song is archived.
    fn require_editable(
        songs: &BTreeMap<SongId, SongWork>,
        arrangement: &Arrangement,
    ) -> Result<(), SongLibraryError> {
        if arrangement.is_archived() {
            return Err(SongLibraryError::Archived(RecordRef::Arrangement(
                arrangement.id(),
            )));
        }
        if songs
            .get(&arrangement.song())
            .is_some_and(SongWork::is_archived)
        {
            return Err(SongLibraryError::Archived(RecordRef::Song(
                arrangement.song(),
            )));
        }
        Ok(())
    }

    /// Refuses a Choir that is archived (the Choir is already known to exist).
    fn require_choir_active(&self, choir: ChoirId) -> Result<(), SongLibraryError> {
        if self.choirs.get(&choir).is_some_and(Choir::is_archived) {
            return Err(SongLibraryError::Archived(RecordRef::Choir(choir)));
        }
        Ok(())
    }

    /// Refuses a set of Choir associations that names an unknown Choir.
    fn require_choirs_exist(&self, choirs: &[ChoirId]) -> Result<(), SongLibraryError> {
        for choir in choirs {
            if !self.choirs.contains_key(choir) {
                return Err(SongLibraryError::UnknownChoir(*choir));
            }
        }
        Ok(())
    }

    /// Refuses section or occurrence identities reserved by an Arrangement
    /// other than `owner`.
    ///
    /// Reservations are never released, so an identity `owner` dropped earlier
    /// may return to `owner` but never moves to another Arrangement. This is an
    /// associated function rather than a method so the caller can hold the
    /// Arrangement's mutable borrow while validating against the library's
    /// identity maps.
    fn require_free_section_and_occurrence_ids(
        section_ids: &BTreeMap<SectionId, ArrangementId>,
        occurrence_ids: &BTreeMap<OccurrenceId, ArrangementId>,
        draft: &ArrangementDraft,
        owner: ArrangementId,
    ) -> Result<(), SongLibraryError> {
        for section in &draft.sections {
            if section_ids
                .get(&section.id)
                .is_some_and(|held| *held != owner)
            {
                return Err(SongLibraryError::DuplicateSection(section.id));
            }
        }
        for occurrence in &draft.sequence {
            if occurrence_ids
                .get(&occurrence.id)
                .is_some_and(|held| *held != owner)
            {
                return Err(SongLibraryError::DuplicateOccurrence(occurrence.id));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::EntityId;
    use crate::song_library::model::{
        ArchiveState, OccurrenceDraft, PaginationState, SectionDraft,
    };

    fn entity(n: u8) -> EntityId {
        EntityId::from_random_bytes([n; 16])
    }

    fn song(n: u8) -> SongId {
        SongId::from_entity(entity(n))
    }

    fn arrangement(n: u8) -> ArrangementId {
        ArrangementId::from_entity(entity(n))
    }

    fn choir(n: u8) -> ChoirId {
        ChoirId::from_entity(entity(n))
    }

    fn section(n: u8) -> SectionId {
        SectionId::from_entity(entity(n))
    }

    fn occurrence(n: u8) -> OccurrenceId {
        OccurrenceId::from_entity(entity(n))
    }

    fn at() -> Timestamp {
        Timestamp::from_millis(1_760_000_000_000)
    }

    /// A translated-arrangement draft whose section and occurrence identities
    /// start at `base`, so two drafts never collide.
    fn arrangement_draft(title: &str, base: u8) -> ArrangementDraft {
        let first = section(base);
        let second = section(base + 1);
        ArrangementDraft {
            title: title.to_owned(),
            languages: vec!["Cebuano".to_owned()],
            rendition_attribution: None,
            occasions: Vec::new(),
            sections: vec![
                SectionDraft::labeled(first, "Verse 1", "Unang bersikulo"),
                SectionDraft::labeled(second, "Chorus", "Koro"),
            ],
            sequence: vec![
                OccurrenceDraft::new(occurrence(base), first),
                OccurrenceDraft::new(occurrence(base + 1), second),
                OccurrenceDraft::new(occurrence(base + 2), first),
            ],
        }
    }

    /// Asserts a refused call reports `expected` and leaves `library` unchanged.
    macro_rules! assert_refused {
        ($library:ident, $expected:expr, $call:expr) => {{
            let before = $library.clone();
            assert_eq!($call.unwrap_err(), $expected);
            assert_eq!($library, before, "a refused call changes nothing");
        }};
    }

    #[test]
    fn create_song_requires_only_the_primary_title() {
        let mut library = SongLibrary::new();
        let id = song(1);
        let created_at = at();
        let created = library
            .create_song(id, created_at, SongDraft::titled("  Kyrie  VIII  "))
            .expect("a title is the only requirement");

        assert_eq!(created.id(), id);
        assert_eq!(
            created.title(),
            "  Kyrie  VIII  ",
            "authored text is verbatim"
        );
        assert!(created.aliases().is_empty());
        assert_eq!(created.original_attribution(), None);
        assert_eq!(created.provenance(), None);
        assert_eq!(created.rights_notes(), None);
        assert!(created.liturgical_moments().is_empty());
        assert!(created.occasions().is_empty());
        assert_eq!(created.created_at(), created_at);
        assert_eq!(created.archive_state(), ArchiveState::Active);
        assert!(!created.is_archived());
        assert_eq!(library.song(id).expect("stored").id(), id);
    }

    #[test]
    fn song_edits_replace_content_and_keep_identity_and_creation_time() {
        let mut library = SongLibrary::new();
        let id = song(1);
        let created_at = at();
        library
            .create_song(id, created_at, SongDraft::titled("kyrie viii"))
            .expect("created");

        let mut draft = SongDraft::titled("Kyrie VIII");
        draft.aliases = vec!["Kyrie 8".to_owned(), "Lord, have mercy".to_owned()];
        draft.original_attribution = Some("Missa de Angelis".to_owned());
        draft.provenance = Some("Parish binder, 1998".to_owned());
        draft.rights_notes = Some("Public domain".to_owned());
        draft.liturgical_moments = vec!["Ordinary".to_owned(), "Entrance".to_owned()];
        draft.occasions = vec!["Lent".to_owned(), "Easter".to_owned()];
        let edited = library.edit_song(id, draft).expect("edited");

        assert_eq!(edited.id(), id);
        assert_eq!(edited.title(), "Kyrie VIII");
        assert_eq!(
            edited.aliases(),
            &["Kyrie 8".to_owned(), "Lord, have mercy".to_owned()][..]
        );
        assert_eq!(edited.original_attribution(), Some("Missa de Angelis"));
        assert_eq!(edited.provenance(), Some("Parish binder, 1998"));
        assert_eq!(edited.rights_notes(), Some("Public domain"));
        assert_eq!(
            edited
                .liturgical_moments()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["Entrance", "Ordinary"]
        );
        assert_eq!(
            edited
                .occasions()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["Easter", "Lent"]
        );
        assert_eq!(edited.created_at(), created_at);
        assert_eq!(edited.archive_state(), ArchiveState::Active);
    }

    #[test]
    fn song_validation_names_the_failing_field_and_changes_nothing() {
        let mut library = SongLibrary::new();

        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::SongTitle),
            library.create_song(song(1), at(), SongDraft::default())
        );

        let mut blank_original_attribution = SongDraft::titled("Kyrie");
        blank_original_attribution.original_attribution = Some(" \t ".to_owned());
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::OriginalAttribution),
            library.create_song(song(2), at(), blank_original_attribution)
        );

        let mut blank_provenance = SongDraft::titled("Kyrie");
        blank_provenance.provenance = Some(" ".to_owned());
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::Provenance),
            library.create_song(song(3), at(), blank_provenance)
        );

        let mut blank_rights_notes = SongDraft::titled("Kyrie");
        blank_rights_notes.rights_notes = Some(String::new());
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::RightsNotes),
            library.create_song(song(4), at(), blank_rights_notes)
        );

        let mut blank_alias = SongDraft::titled("Kyrie");
        blank_alias.aliases = vec![" \t ".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::Aliases),
            library.create_song(song(5), at(), blank_alias)
        );

        let mut duplicate_alias = SongDraft::titled("Kyrie");
        duplicate_alias.aliases = vec!["Kyrie 8".to_owned(), "Kyrie 8".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::Aliases),
            library.create_song(song(6), at(), duplicate_alias)
        );

        let mut title_alias = SongDraft::titled("Kyrie");
        title_alias.aliases = vec!["Kyrie".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::Aliases),
            library.create_song(song(7), at(), title_alias)
        );

        let mut case_distinct = SongDraft::titled("Kyrie VIII");
        case_distinct.aliases = vec!["kyrie 8".to_owned(), "Kyrie 8".to_owned()];
        library
            .create_song(song(8), at(), case_distinct)
            .expect("authored case is preserved and not normalized here");

        let mut blank_moment = SongDraft::titled("Kyrie");
        blank_moment.liturgical_moments = vec![" ".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::LiturgicalMoment),
            library.create_song(song(9), at(), blank_moment)
        );

        let mut duplicate_moment = SongDraft::titled("Kyrie");
        duplicate_moment.liturgical_moments = vec!["Entrance".to_owned(), "Entrance".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::LiturgicalMoment),
            library.create_song(song(10), at(), duplicate_moment)
        );

        let mut blank_occasion = SongDraft::titled("Kyrie");
        blank_occasion.occasions = vec!["".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::Occasion),
            library.create_song(song(11), at(), blank_occasion)
        );

        let mut duplicate_occasion = SongDraft::titled("Kyrie");
        duplicate_occasion.occasions = vec!["Lent".to_owned(), "Lent".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::Occasion),
            library.create_song(song(12), at(), duplicate_occasion)
        );
    }

    #[test]
    fn duplicate_identities_are_refused() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("created");
        assert_refused!(
            library,
            SongLibraryError::DuplicateSong(song_id),
            library.create_song(song_id, at(), SongDraft::titled("Another"))
        );

        let choir_id = choir(1);
        library
            .create_choir(choir_id, at(), ChoirDraft::named("Youth"))
            .expect("created");
        assert_refused!(
            library,
            SongLibraryError::DuplicateChoir(choir_id),
            library.create_choir(choir_id, at(), ChoirDraft::named("Adult"))
        );

        let arrangement_id = arrangement(1);
        library
            .create_arrangement(
                arrangement_id,
                song_id,
                at(),
                arrangement_draft("One", 10),
                Vec::new(),
            )
            .expect("created");
        assert_refused!(
            library,
            SongLibraryError::DuplicateArrangement(arrangement_id),
            library.create_arrangement(
                arrangement_id,
                song_id,
                at(),
                arrangement_draft("Two", 20),
                Vec::new(),
            )
        );
    }

    #[test]
    fn arrangement_belongs_to_one_song_and_owns_its_content() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        let youth = choir(1);
        let adult = choir(2);
        library
            .create_choir(youth, at(), ChoirDraft::named("Youth"))
            .expect("youth");
        library
            .create_choir(adult, at(), ChoirDraft::named("Adult"))
            .expect("adult");

        let id = arrangement(1);
        let created_at = at();
        let mut draft = arrangement_draft("Cebuano", 10);
        draft.languages = vec!["Cebuano".to_owned(), "English".to_owned()];
        draft.rendition_attribution = Some("Sr. Maria".to_owned());
        draft.occasions = vec!["Lent".to_owned()];
        let created = library
            .create_arrangement(id, song_id, created_at, draft, vec![youth, adult])
            .expect("arrangement");

        assert_eq!(created.id(), id);
        assert_eq!(created.song(), song_id);
        assert_eq!(created.title(), "Cebuano");
        assert_eq!(created.languages().len(), 2);
        assert_eq!(created.languages()[0], "Cebuano");
        assert_eq!(created.languages()[1], "English");
        assert_eq!(created.choirs().len(), 2);
        assert!(created.choirs().contains(&youth));
        assert!(created.choirs().contains(&adult));
        assert_eq!(created.rendition_attribution(), Some("Sr. Maria"));
        assert!(created.occasions().contains("Lent"));
        assert_eq!(created.sections().len(), 2);
        assert_eq!(created.sections()[0].label(), Some("Verse 1"));
        assert_eq!(created.sections()[0].lyrics(), "Unang bersikulo");
        assert_eq!(created.sections()[1].label(), Some("Chorus"));
        assert_eq!(created.sequence().len(), 3);
        assert_eq!(created.sequence()[0].section(), created.sections()[0].id());
        assert_eq!(created.sequence()[1].section(), created.sections()[1].id());
        assert_eq!(
            created.sequence()[2].section(),
            created.sections()[0].id(),
            "a section may repeat in the Sequence"
        );
        assert_eq!(created.pagination_state(), PaginationState::Draft);
        assert_eq!(created.created_at(), created_at);
        assert_eq!(created.archive_state(), ArchiveState::Active);

        let mut edited_draft = arrangement_draft("Cebuano (Lent)", 10);
        edited_draft.title = "Cebuano (Lent)".to_owned();
        edited_draft.sections[0].lyrics = "Bag-ong bersikulo".to_owned();
        let edited = library
            .edit_arrangement(id, edited_draft)
            .expect("edited arrangement");
        assert_eq!(edited.id(), id);
        assert_eq!(edited.song(), song_id);
        assert_eq!(edited.title(), "Cebuano (Lent)");
        assert_eq!(edited.created_at(), created_at);
        assert_eq!(edited.pagination_state(), PaginationState::Draft);
        assert_eq!(
            edited.choirs().len(),
            2,
            "a content edit never changes Choir associations"
        );
        assert!(edited.choirs().contains(&youth));
        assert!(edited.choirs().contains(&adult));
        assert_eq!(
            library.song(song_id).expect("song").title(),
            "Kyrie",
            "editing an Arrangement never changes the Song work"
        );
    }

    #[test]
    fn numbered_settings_are_separate_songs_and_translations_are_separate_arrangements() {
        let mut library = SongLibrary::new();
        let eighth = song(1);
        let thirteenth = song(2);
        library
            .create_song(eighth, at(), SongDraft::titled("Kyrie 8th"))
            .expect("8th");
        library
            .create_song(thirteenth, at(), SongDraft::titled("Kyrie 13th"))
            .expect("13th");
        assert_ne!(eighth, thirteenth);
        assert_eq!(library.song(eighth).expect("8th").title(), "Kyrie 8th");
        assert_eq!(
            library.song(thirteenth).expect("13th").title(),
            "Kyrie 13th"
        );

        library
            .edit_song(eighth, SongDraft::titled("Kyrie VIII (8th Mass)"))
            .expect("edited");
        assert_eq!(
            library.song(thirteenth).expect("13th").title(),
            "Kyrie 13th",
            "a numbered setting is never rewritten by editing another"
        );

        let cebuano = arrangement(1);
        let english = arrangement(2);
        let mut cebuano_draft = arrangement_draft("Cebuano", 10);
        cebuano_draft.languages = vec!["Cebuano".to_owned()];
        let mut english_draft = arrangement_draft("English", 20);
        english_draft.languages = vec!["English".to_owned()];
        library
            .create_arrangement(cebuano, eighth, at(), cebuano_draft, Vec::new())
            .expect("Cebuano");
        library
            .create_arrangement(english, eighth, at(), english_draft, Vec::new())
            .expect("English");

        assert_eq!(
            library.arrangements_for_song(eighth).expect("song").len(),
            2,
            "both translated renditions remain Arrangements of one work"
        );
        library
            .edit_arrangement(cebuano, arrangement_draft("Cebuano (Lent)", 10))
            .expect("edited");
        assert_eq!(
            library.arrangement(english).expect("English").title(),
            "English",
            "each translated Arrangement is independent"
        );
    }

    #[test]
    fn multiple_choirs_share_one_arrangement_and_choirs_are_optional() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");

        let no_choir = arrangement(1);
        library
            .create_arrangement(
                no_choir,
                song_id,
                at(),
                arrangement_draft("A cappella", 10),
                Vec::new(),
            )
            .expect("an Arrangement needs no Choir");
        assert!(
            library
                .arrangement(no_choir)
                .expect("arrangement")
                .choirs()
                .is_empty()
        );

        let youth = choir(1);
        let adult = choir(2);
        library
            .create_choir(youth, at(), ChoirDraft::named("Youth"))
            .expect("youth");
        library
            .create_choir(adult, at(), ChoirDraft::named("Adult"))
            .expect("adult");

        let shared = arrangement(2);
        library
            .create_arrangement(
                shared,
                song_id,
                at(),
                arrangement_draft("Shared", 20),
                Vec::new(),
            )
            .expect("shared");
        library
            .add_arrangement_choir(shared, youth)
            .expect("added youth");
        library
            .add_arrangement_choir(shared, youth)
            .expect("adding an existing Choir is a no-op");
        library
            .add_arrangement_choir(shared, adult)
            .expect("added adult");

        let arrangement_record = library.arrangement(shared).expect("shared arrangement");
        assert_eq!(arrangement_record.choirs().len(), 2);
        assert!(arrangement_record.choirs().contains(&youth));
        assert!(arrangement_record.choirs().contains(&adult));

        let repertoire = library.choir_repertoire(youth).expect("repertoire");
        assert_eq!(repertoire.len(), 1);
        assert_eq!(repertoire[0].id(), song_id);

        library
            .remove_arrangement_choir(shared, youth)
            .expect("removed youth");
        let arrangement_record = library.arrangement(shared).expect("shared arrangement");
        assert!(!arrangement_record.choirs().contains(&youth));
        assert!(arrangement_record.choirs().contains(&adult));
        assert!(
            library
                .choir_repertoire(youth)
                .expect("repertoire")
                .is_empty()
        );
        library
            .remove_arrangement_choir(shared, youth)
            .expect("removing a missing association is a no-op");

        library
            .set_arrangement_choirs(shared, vec![youth, adult])
            .expect("set associations");
        assert_eq!(
            library
                .arrangement(shared)
                .expect("shared arrangement")
                .choirs()
                .len(),
            2
        );
        library
            .set_arrangement_choirs(shared, Vec::new())
            .expect("an Arrangement stays valid with no Choir");
        assert!(
            library
                .arrangement(shared)
                .expect("shared arrangement")
                .choirs()
                .is_empty()
        );

        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::Choir),
            library.set_arrangement_choirs(shared, vec![youth, youth])
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(choir(9)),
            library.set_arrangement_choirs(shared, vec![choir(9)])
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(choir(9)),
            library.add_arrangement_choir(shared, choir(9))
        );

        let unknown_choir = arrangement_draft("Unknown Choir", 30);
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(choir(9)),
            library.create_arrangement(
                arrangement(3),
                song_id,
                at(),
                unknown_choir,
                vec![choir(9)]
            )
        );
    }

    #[test]
    fn choir_repertoire_lists_each_song_once() {
        let mut library = SongLibrary::new();
        let youth = choir(1);
        library
            .create_choir(youth, at(), ChoirDraft::named("Youth"))
            .expect("choir");

        let first_song = song(1);
        let second_song = song(2);
        library
            .create_song(first_song, at(), SongDraft::titled("Kyrie"))
            .expect("first");
        library
            .create_song(second_song, at(), SongDraft::titled("Gloria"))
            .expect("second");

        let first = arrangement(1);
        let second = arrangement(2);
        let unrelated = arrangement(3);
        let first_draft = arrangement_draft("Kyrie Cebuano", 10);
        let second_draft = arrangement_draft("Kyrie English", 20);
        let unrelated_draft = arrangement_draft("Gloria", 30);
        library
            .create_arrangement(first, first_song, at(), first_draft, vec![youth])
            .expect("first arrangement");
        library
            .create_arrangement(second, first_song, at(), second_draft, vec![youth])
            .expect("second arrangement");
        library
            .create_arrangement(unrelated, second_song, at(), unrelated_draft, Vec::new())
            .expect("unrelated");

        let repertoire = library.choir_repertoire(youth).expect("repertoire");
        assert_eq!(
            repertoire.len(),
            1,
            "two qualifying Arrangements of one Song add it once"
        );
        assert_eq!(repertoire[0].id(), first_song);

        assert_eq!(
            library
                .choir_arrangements(youth)
                .expect("renditions")
                .iter()
                .map(|arrangement| arrangement.id())
                .collect::<Vec<_>>(),
            vec![first, second],
            "choir_arrangements lists the exact renditions the Choir sings"
        );
    }

    /// A library with one song, one Choir, and one unapproved Arrangement.
    fn pagination_fixture() -> (
        SongLibrary,
        SongId,
        ArrangementId,
        ArrangementDraft,
        ChoirId,
    ) {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        let youth = choir(1);
        library
            .create_choir(youth, at(), ChoirDraft::named("Youth"))
            .expect("choir");
        let id = arrangement(1);
        let base = arrangement_draft("Cebuano", 10);
        library
            .create_arrangement(id, song_id, at(), base.clone(), vec![youth])
            .expect("arrangement");
        (library, song_id, id, base, youth)
    }

    /// Edits one Arrangement and returns its pagination state after the edit.
    fn edit_pagination(
        library: &mut SongLibrary,
        id: ArrangementId,
        draft: ArrangementDraft,
    ) -> PaginationState {
        library.edit_arrangement(id, draft).expect("edit");
        library
            .arrangement(id)
            .expect("arrangement")
            .pagination_state()
    }

    #[test]
    fn arrangements_for_song_and_choir_use_creation_time_then_identity() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        let youth = choir(1);
        library
            .create_choir(youth, at(), ChoirDraft::named("Youth"))
            .expect("choir");

        library
            .create_arrangement(
                arrangement(2),
                song_id,
                Timestamp::from_millis(200),
                arrangement_draft("Two", 20),
                vec![youth],
            )
            .expect("second created");
        library
            .create_arrangement(
                arrangement(3),
                song_id,
                Timestamp::from_millis(100),
                arrangement_draft("Three", 30),
                Vec::new(),
            )
            .expect("third created");
        library
            .create_arrangement(
                arrangement(1),
                song_id,
                Timestamp::from_millis(100),
                arrangement_draft("One", 10),
                vec![youth],
            )
            .expect("first created");

        assert_eq!(
            library
                .arrangements_for_song(song_id)
                .expect("song")
                .iter()
                .map(|arrangement| arrangement.id())
                .collect::<Vec<_>>(),
            vec![arrangement(1), arrangement(3), arrangement(2)],
            "arrangements_for_song sorts by creation time then identity"
        );
        assert_eq!(
            library
                .choir_arrangements(youth)
                .expect("choir")
                .iter()
                .map(|arrangement| arrangement.id())
                .collect::<Vec<_>>(),
            vec![arrangement(1), arrangement(2)],
            "choir_arrangements sorts by creation time then identity"
        );
    }

    #[test]
    fn approved_pagination_follows_the_review_rule() {
        let (mut library, song_id, id, base, youth) = pagination_fixture();
        assert_eq!(
            edit_pagination(&mut library, id, base.clone()),
            PaginationState::Draft
        );

        let mut content = base;
        content.sections[0].lyrics = "Unang bersikulo, giusab".to_owned();
        assert_eq!(
            edit_pagination(&mut library, id, content.clone()),
            PaginationState::Draft,
            "an unapproved Arrangement stays Draft when its content changes"
        );

        library.approve_pagination(id).expect("approved");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::Approved
        );

        let mut metadata = content.clone();
        metadata.title = "Cebuano (Lent)".to_owned();
        metadata.languages = vec!["Cebuano".to_owned(), "English".to_owned()];
        metadata.rendition_attribution = Some("Sr. Maria".to_owned());
        metadata.occasions = vec!["Lent".to_owned()];
        assert_eq!(
            edit_pagination(&mut library, id, metadata),
            PaginationState::Approved,
            "title, language, attribution, and occasion changes do not affect pagination"
        );
        assert!(
            library
                .arrangement(id)
                .expect("arrangement")
                .choirs()
                .contains(&youth),
            "a content edit never changes Choir associations"
        );

        let mut lyrics = content.clone();
        lyrics.sections[0].lyrics = "Bag-ong bersikulo".to_owned();
        assert_eq!(
            edit_pagination(&mut library, id, lyrics),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut label = content.clone();
        label.sections[1].label = Some("Refrain".to_owned());
        assert_eq!(
            edit_pagination(&mut library, id, label),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut reordered_sequence = content.clone();
        reordered_sequence.sequence.swap(0, 2);
        assert_eq!(
            edit_pagination(&mut library, id, reordered_sequence.clone()),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut reordered_sections = reordered_sequence;
        reordered_sections.sections.swap(0, 1);
        assert_eq!(
            edit_pagination(&mut library, id, reordered_sections.clone()),
            PaginationState::Approved,
            "reordering the section list alone keeps approval"
        );

        library.approve_pagination(id).expect("approved again");
        let mut added = reordered_sections.clone();
        added
            .sections
            .push(SectionDraft::new(section(19), "Bridge"));
        assert_eq!(
            edit_pagination(&mut library, id, added),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut removed = reordered_sections.clone();
        removed
            .sections
            .retain(|candidate| candidate.id != section(11));
        removed
            .sequence
            .retain(|occurrence| occurrence.section != section(11));
        let unchanged = removed.clone();
        assert_eq!(
            edit_pagination(&mut library, id, removed),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        assert_eq!(
            edit_pagination(&mut library, id, unchanged),
            PaginationState::Approved,
            "an edit that changes no pagination content keeps approval"
        );

        let second = arrangement(2);
        library
            .create_arrangement(
                second,
                song_id,
                at(),
                arrangement_draft("English", 20),
                Vec::new(),
            )
            .expect("second arrangement");
        assert_eq!(
            library
                .arrangement(second)
                .expect("second arrangement")
                .pagination_state(),
            PaginationState::Draft,
            "a new Arrangement always starts in Draft"
        );
    }

    #[test]
    fn arrangement_validation_names_the_failing_field_and_changes_nothing() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        let youth = choir(1);
        library
            .create_choir(youth, at(), ChoirDraft::named("Youth"))
            .expect("choir");
        let id = arrangement(1);

        let mut blank_title = arrangement_draft("Title", 10);
        blank_title.title = "  ".to_owned();
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::ArrangementTitle),
            library.create_arrangement(id, song_id, at(), blank_title, Vec::new())
        );

        let mut blank_language = arrangement_draft("Title", 10);
        blank_language.languages = vec![" ".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::Language),
            library.create_arrangement(id, song_id, at(), blank_language, Vec::new())
        );

        let mut duplicate_language = arrangement_draft("Title", 10);
        duplicate_language.languages = vec!["Cebuano".to_owned(), "Cebuano".to_owned()];
        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::Language),
            library.create_arrangement(id, song_id, at(), duplicate_language, Vec::new())
        );

        let mut blank_rendition_attribution = arrangement_draft("Title", 10);
        blank_rendition_attribution.rendition_attribution = Some(" \t ".to_owned());
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::RenditionAttribution),
            library.create_arrangement(id, song_id, at(), blank_rendition_attribution, Vec::new())
        );

        let duplicate_choir = arrangement_draft("Title", 10);
        assert_refused!(
            library,
            SongLibraryError::DuplicateField(Field::Choir),
            library.create_arrangement(id, song_id, at(), duplicate_choir, vec![youth, youth])
        );

        let mut blank_label = arrangement_draft("Title", 10);
        blank_label.sections[0].label = Some("  ".to_owned());
        assert_refused!(
            library,
            SongLibraryError::BlankField(Field::SectionLabel),
            library.create_arrangement(id, song_id, at(), blank_label, Vec::new())
        );

        let mut foreign_section = arrangement_draft("Title", 10);
        foreign_section.sequence[0].section = section(99);
        assert_refused!(
            library,
            SongLibraryError::SectionNotInArrangement {
                arrangement: id,
                section: section(99),
            },
            library.create_arrangement(id, song_id, at(), foreign_section, Vec::new())
        );

        let mut duplicate_section = arrangement_draft("Title", 10);
        duplicate_section.sections[1].id = duplicate_section.sections[0].id;
        duplicate_section.sequence[1].section = duplicate_section.sections[0].id;
        assert_refused!(
            library,
            SongLibraryError::DuplicateSection(section(10)),
            library.create_arrangement(id, song_id, at(), duplicate_section, Vec::new())
        );

        let mut duplicate_occurrence = arrangement_draft("Title", 10);
        duplicate_occurrence.sequence[1].id = duplicate_occurrence.sequence[0].id;
        assert_refused!(
            library,
            SongLibraryError::DuplicateOccurrence(occurrence(10)),
            library.create_arrangement(id, song_id, at(), duplicate_occurrence, Vec::new())
        );
    }

    #[test]
    fn section_and_occurrence_identities_are_unique_across_the_library() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        library
            .create_arrangement(
                arrangement(1),
                song_id,
                at(),
                arrangement_draft("One", 10),
                Vec::new(),
            )
            .expect("first arrangement");

        let stolen_section = section(10);
        let mut stolen = arrangement_draft("Two", 20);
        stolen.sections[0].id = stolen_section;
        stolen.sequence[0].section = stolen_section;
        stolen.sequence[2].section = stolen_section;
        assert_refused!(
            library,
            SongLibraryError::DuplicateSection(stolen_section),
            library.create_arrangement(arrangement(2), song_id, at(), stolen, Vec::new())
        );

        let stolen_occurrence = occurrence(10);
        let mut stolen = arrangement_draft("Two", 20);
        stolen.sequence[0].id = stolen_occurrence;
        assert_refused!(
            library,
            SongLibraryError::DuplicateOccurrence(stolen_occurrence),
            library.create_arrangement(arrangement(2), song_id, at(), stolen, Vec::new())
        );

        library
            .create_arrangement(
                arrangement(2),
                song_id,
                at(),
                arrangement_draft("Two", 20),
                Vec::new(),
            )
            .expect("second arrangement");
        let mut stolen = arrangement_draft("Two edited", 20);
        stolen.sections[0].id = stolen_section;
        stolen.sequence[0].section = stolen_section;
        stolen.sequence[2].section = stolen_section;
        assert_refused!(
            library,
            SongLibraryError::DuplicateSection(stolen_section),
            library.edit_arrangement(arrangement(2), stolen)
        );

        let mut stolen = arrangement_draft("Two edited", 20);
        stolen.sequence[0].id = stolen_occurrence;
        assert_refused!(
            library,
            SongLibraryError::DuplicateOccurrence(stolen_occurrence),
            library.edit_arrangement(arrangement(2), stolen)
        );

        library
            .edit_arrangement(arrangement(2), arrangement_draft("Two edited", 20))
            .expect("an edit that keeps its own identities succeeds");
    }

    #[test]
    fn unknown_entities_are_named_and_change_nothing() {
        let mut library = SongLibrary::new();
        let unknown_song = song(1);
        let unknown_arrangement = arrangement(1);
        let unknown_choir = choir(1);

        assert_refused!(
            library,
            SongLibraryError::UnknownSong(unknown_song),
            library.song(unknown_song)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownArrangement(unknown_arrangement),
            library.arrangement(unknown_arrangement)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(unknown_choir),
            library.choir(unknown_choir)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownSong(unknown_song),
            library.arrangements_for_song(unknown_song)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(unknown_choir),
            library.choir_arrangements(unknown_choir)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(unknown_choir),
            library.choir_repertoire(unknown_choir)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownSong(unknown_song),
            library.edit_song(unknown_song, SongDraft::titled("Kyrie"))
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownArrangement(unknown_arrangement),
            library.edit_arrangement(unknown_arrangement, arrangement_draft("Kyrie", 10))
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownChoir(unknown_choir),
            library.edit_choir(unknown_choir, ChoirDraft::named("Youth"))
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownArrangement(unknown_arrangement),
            library.approve_pagination(unknown_arrangement)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownArrangement(unknown_arrangement),
            library.set_arrangement_choirs(unknown_arrangement, Vec::new())
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownArrangement(unknown_arrangement),
            library.add_arrangement_choir(unknown_arrangement, unknown_choir)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownArrangement(unknown_arrangement),
            library.remove_arrangement_choir(unknown_arrangement, unknown_choir)
        );
        assert_refused!(
            library,
            SongLibraryError::UnknownSong(unknown_song),
            library.create_arrangement(
                unknown_arrangement,
                unknown_song,
                at(),
                ArrangementDraft::titled("Kyrie"),
                Vec::new(),
            )
        );
    }

    #[test]
    fn reads_list_every_entity_in_identity_order() {
        let mut library = SongLibrary::new();
        assert_eq!(library.songs().count(), 0);
        assert_eq!(library.arrangements().count(), 0);
        assert_eq!(library.choirs().count(), 0);

        library
            .create_song(song(2), at(), SongDraft::titled("Second"))
            .expect("second song");
        library
            .create_song(song(1), at(), SongDraft::titled("First"))
            .expect("first song");
        assert_eq!(
            library.songs().map(SongWork::id).collect::<Vec<_>>(),
            vec![song(1), song(2)]
        );

        library
            .create_choir(choir(2), at(), ChoirDraft::named("Second"))
            .expect("second choir");
        library
            .create_choir(choir(1), at(), ChoirDraft::named("First"))
            .expect("first choir");
        assert_eq!(
            library.choirs().map(Choir::id).collect::<Vec<_>>(),
            vec![choir(1), choir(2)]
        );

        library
            .create_arrangement(
                arrangement(2),
                song(1),
                at(),
                arrangement_draft("Two", 20),
                Vec::new(),
            )
            .expect("second arrangement");
        library
            .create_arrangement(
                arrangement(1),
                song(1),
                at(),
                arrangement_draft("One", 10),
                Vec::new(),
            )
            .expect("first arrangement");
        assert_eq!(
            library
                .arrangements()
                .map(Arrangement::id)
                .collect::<Vec<_>>(),
            vec![arrangement(1), arrangement(2)]
        );
        assert_eq!(
            library
                .arrangements_for_song(song(1))
                .expect("song")
                .iter()
                .map(|arrangement| arrangement.id())
                .collect::<Vec<_>>(),
            vec![arrangement(1), arrangement(2)]
        );
    }
}
