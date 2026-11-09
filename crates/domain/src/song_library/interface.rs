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

use crate::identity::{ArrangementId, ChoirId, OccurrenceId, SectionId, SongId, Timestamp};

use super::model::text;
use super::model::{
    Arrangement, ArrangementDraft, Choir, Occurrence, Section, SongDraft, SongWork,
};

/// One authored field that failed a Song Library validation rule.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Field {
    /// A Song work's primary title.
    SongTitle,
    /// One alternate title of a Song work.
    Alias,
    /// One liturgical moment of a Song work.
    LiturgicalMoment,
    /// One occasion applicability tag.
    Occasion,
    /// A Choir's name.
    ChoirName,
    /// An Arrangement's title.
    ArrangementTitle,
    /// One language of an Arrangement.
    Language,
    /// One section label.
    SectionLabel,
    /// One Choir association of an Arrangement.
    Choir,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SongTitle => "Song title",
            Self::Alias => "alias",
            Self::LiturgicalMoment => "liturgical moment",
            Self::Occasion => "occasion",
            Self::ChoirName => "Choir name",
            Self::ArrangementTitle => "Arrangement title",
            Self::Language => "language",
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
    /// A required authored value is empty or only whitespace.
    BlankField(Field),
    /// A field that holds a set of values repeats one value.
    DuplicateField(Field),
    /// An occurrence names a section that is not one of its Arrangement's own
    /// sections.
    SectionNotInArrangement {
        /// The Arrangement whose Sequence holds the occurrence.
        arrangement: ArrangementId,
        /// The section the occurrence names.
        section: SectionId,
    },
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
        }
    }
}

impl std::error::Error for SongLibraryError {}

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
    section_ids: BTreeSet<SectionId>,
    occurrence_ids: BTreeSet<OccurrenceId>,
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
    /// Identity, creation time, and archive state are preserved.
    pub fn edit_song(
        &mut self,
        id: SongId,
        draft: SongDraft,
    ) -> Result<&SongWork, SongLibraryError> {
        let song = self
            .songs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownSong(id))?;
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

    /// Every Arrangement of one Song work, in identity order.
    pub fn arrangements_for_song(
        &self,
        song: SongId,
    ) -> Result<impl Iterator<Item = &Arrangement> + '_, SongLibraryError> {
        if !self.songs.contains_key(&song) {
            return Err(SongLibraryError::UnknownSong(song));
        }
        Ok(self
            .arrangements
            .values()
            .filter(move |arrangement| arrangement.song() == song))
    }

    /// Creates one Arrangement of `song` and returns it.
    ///
    /// The identity must be unused, the Song must exist, every Choir must
    /// exist, and every section and occurrence identity must be free. A new
    /// Arrangement starts with [`super::PaginationState::Draft`] pagination and
    /// an active archive state.
    pub fn create_arrangement(
        &mut self,
        id: ArrangementId,
        song: SongId,
        created_at: Timestamp,
        draft: ArrangementDraft,
    ) -> Result<&Arrangement, SongLibraryError> {
        if self.arrangements.contains_key(&id) {
            return Err(SongLibraryError::DuplicateArrangement(id));
        }
        if !self.songs.contains_key(&song) {
            return Err(SongLibraryError::UnknownSong(song));
        }
        self.require_choirs_exist(&draft.choirs)?;
        self.require_free_section_and_occurrence_ids(&draft, None)?;
        let arrangement = Arrangement::new(id, song, created_at, draft)?;
        self.section_ids
            .extend(arrangement.sections().iter().map(Section::id));
        self.occurrence_ids
            .extend(arrangement.sequence().iter().map(Occurrence::id));
        self.arrangements.insert(id, arrangement);
        self.arrangement(id)
    }

    /// Replaces the authored content of one Arrangement.
    ///
    /// The owning Song, identity, creation time, and archive state are
    /// preserved. Every Choir must exist. Section and occurrence identities
    /// that already belong to this Arrangement may be reused; an identity owned
    /// by another Arrangement is refused.
    ///
    /// A change to a section's lyrics or label, to the section list, or to the
    /// Sequence moves an approved Arrangement to
    /// [`super::PaginationState::NeedsReview`].
    pub fn edit_arrangement(
        &mut self,
        id: ArrangementId,
        draft: ArrangementDraft,
    ) -> Result<&Arrangement, SongLibraryError> {
        let (previous_sections, previous_occurrences) = {
            let previous = self
                .arrangements
                .get(&id)
                .ok_or(SongLibraryError::UnknownArrangement(id))?;
            (
                previous
                    .sections()
                    .iter()
                    .map(Section::id)
                    .collect::<BTreeSet<_>>(),
                previous
                    .sequence()
                    .iter()
                    .map(Occurrence::id)
                    .collect::<BTreeSet<_>>(),
            )
        };
        self.require_choirs_exist(&draft.choirs)?;
        self.require_free_section_and_occurrence_ids(
            &draft,
            Some((&previous_sections, &previous_occurrences)),
        )?;
        let (next_sections, next_occurrences) = {
            let arrangement = self
                .arrangements
                .get_mut(&id)
                .ok_or(SongLibraryError::UnknownArrangement(id))?;
            arrangement.replace_content(draft)?;
            (
                arrangement
                    .sections()
                    .iter()
                    .map(Section::id)
                    .collect::<Vec<_>>(),
                arrangement
                    .sequence()
                    .iter()
                    .map(Occurrence::id)
                    .collect::<Vec<_>>(),
            )
        };
        for section in previous_sections {
            self.section_ids.remove(&section);
        }
        self.section_ids.extend(next_sections);
        for occurrence in previous_occurrences {
            self.occurrence_ids.remove(&occurrence);
        }
        self.occurrence_ids.extend(next_occurrences);
        self.arrangement(id)
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
    /// A Choir needs only a nonblank name. The identity must be unused.
    pub fn create_choir(
        &mut self,
        id: ChoirId,
        created_at: Timestamp,
        name: String,
    ) -> Result<&Choir, SongLibraryError> {
        if self.choirs.contains_key(&id) {
            return Err(SongLibraryError::DuplicateChoir(id));
        }
        self.choirs.insert(id, Choir::new(id, created_at, name)?);
        self.choir(id)
    }

    /// Replaces one Choir's name.
    ///
    /// Identity, creation time, and archive state are preserved.
    pub fn edit_choir(&mut self, id: ChoirId, name: String) -> Result<&Choir, SongLibraryError> {
        let choir = self
            .choirs
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownChoir(id))?;
        choir.rename(name)?;
        Ok(choir)
    }

    /// Replaces the Choir associations of one Arrangement.
    ///
    /// Duplicate Choir identities and unknown Choirs are refused. The
    /// Arrangement may end with no Choir.
    pub fn set_arrangement_choirs(
        &mut self,
        id: ArrangementId,
        choirs: Vec<ChoirId>,
    ) -> Result<(), SongLibraryError> {
        if !self.arrangements.contains_key(&id) {
            return Err(SongLibraryError::UnknownArrangement(id));
        }
        text::require_distinct(Field::Choir, &choirs)?;
        self.require_choirs_exist(&choirs)?;
        let arrangement = self
            .arrangements
            .get_mut(&id)
            .ok_or(SongLibraryError::UnknownArrangement(id))?;
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
        if !self.arrangements.contains_key(&arrangement) {
            return Err(SongLibraryError::UnknownArrangement(arrangement));
        }
        if !self.choirs.contains_key(&choir) {
            return Err(SongLibraryError::UnknownChoir(choir));
        }
        let arrangement = self
            .arrangements
            .get_mut(&arrangement)
            .ok_or(SongLibraryError::UnknownArrangement(arrangement))?;
        arrangement.add_choir(choir);
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
        arrangement.approve_pagination();
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

    /// Refuses section or occurrence identities that another Arrangement
    /// already owns.
    ///
    /// `previous` names the identities the edited Arrangement is giving up, so
    /// an ordinary edit that keeps its own identities succeeds.
    fn require_free_section_and_occurrence_ids(
        &self,
        draft: &ArrangementDraft,
        previous: Option<(&BTreeSet<SectionId>, &BTreeSet<OccurrenceId>)>,
    ) -> Result<(), SongLibraryError> {
        let empty_sections = BTreeSet::new();
        let empty_occurrences = BTreeSet::new();
        let (previous_sections, previous_occurrences) = match previous {
            Some((sections, occurrences)) => (sections, occurrences),
            None => (&empty_sections, &empty_occurrences),
        };
        for section in &draft.sections {
            if self.section_ids.contains(&section.id) && !previous_sections.contains(&section.id) {
                return Err(SongLibraryError::DuplicateSection(section.id));
            }
        }
        for occurrence in &draft.sequence {
            if self.occurrence_ids.contains(&occurrence.id)
                && !previous_occurrences.contains(&occurrence.id)
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
            choirs: Vec::new(),
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
    fn song_validation_names_the_failing_field() {
        let mut library = SongLibrary::new();
        assert_eq!(
            library
                .create_song(song(1), at(), SongDraft::default())
                .unwrap_err(),
            SongLibraryError::BlankField(Field::SongTitle)
        );

        let mut blank_alias = SongDraft::titled("Kyrie");
        blank_alias.aliases = vec![" \t ".to_owned()];
        assert_eq!(
            library.create_song(song(2), at(), blank_alias).unwrap_err(),
            SongLibraryError::BlankField(Field::Alias)
        );

        let mut duplicate_alias = SongDraft::titled("Kyrie");
        duplicate_alias.aliases = vec!["Kyrie 8".to_owned(), "Kyrie 8".to_owned()];
        assert_eq!(
            library
                .create_song(song(3), at(), duplicate_alias)
                .unwrap_err(),
            SongLibraryError::DuplicateField(Field::Alias)
        );

        let mut case_distinct = SongDraft::titled("Kyrie");
        case_distinct.aliases = vec!["Kyrie".to_owned(), "kyrie".to_owned()];
        library
            .create_song(song(4), at(), case_distinct)
            .expect("authored case is preserved and not normalized here");

        let mut blank_moment = SongDraft::titled("Kyrie");
        blank_moment.liturgical_moments = vec![" ".to_owned()];
        assert_eq!(
            library
                .create_song(song(5), at(), blank_moment)
                .unwrap_err(),
            SongLibraryError::BlankField(Field::LiturgicalMoment)
        );

        let mut duplicate_moment = SongDraft::titled("Kyrie");
        duplicate_moment.liturgical_moments = vec!["Entrance".to_owned(), "Entrance".to_owned()];
        assert_eq!(
            library
                .create_song(song(6), at(), duplicate_moment)
                .unwrap_err(),
            SongLibraryError::DuplicateField(Field::LiturgicalMoment)
        );

        let mut blank_occasion = SongDraft::titled("Kyrie");
        blank_occasion.occasions = vec!["".to_owned()];
        assert_eq!(
            library
                .create_song(song(7), at(), blank_occasion)
                .unwrap_err(),
            SongLibraryError::BlankField(Field::Occasion)
        );

        let mut duplicate_occasion = SongDraft::titled("Kyrie");
        duplicate_occasion.occasions = vec!["Lent".to_owned(), "Lent".to_owned()];
        assert_eq!(
            library
                .create_song(song(8), at(), duplicate_occasion)
                .unwrap_err(),
            SongLibraryError::DuplicateField(Field::Occasion)
        );
    }

    #[test]
    fn duplicate_identities_are_refused() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("created");
        assert_eq!(
            library
                .create_song(song_id, at(), SongDraft::titled("Another"))
                .unwrap_err(),
            SongLibraryError::DuplicateSong(song_id)
        );

        let choir_id = choir(1);
        library
            .create_choir(choir_id, at(), "Youth".to_owned())
            .expect("created");
        assert_eq!(
            library
                .create_choir(choir_id, at(), "Adult".to_owned())
                .unwrap_err(),
            SongLibraryError::DuplicateChoir(choir_id)
        );

        let arrangement_id = arrangement(1);
        library
            .create_arrangement(arrangement_id, song_id, at(), arrangement_draft("One", 10))
            .expect("created");
        assert_eq!(
            library
                .create_arrangement(arrangement_id, song_id, at(), arrangement_draft("Two", 20))
                .unwrap_err(),
            SongLibraryError::DuplicateArrangement(arrangement_id)
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
            .create_choir(youth, at(), "Youth".to_owned())
            .expect("youth");
        library
            .create_choir(adult, at(), "Adult".to_owned())
            .expect("adult");

        let id = arrangement(1);
        let created_at = at();
        let mut draft = arrangement_draft("Cebuano", 10);
        draft.languages = vec!["Cebuano".to_owned(), "English".to_owned()];
        draft.choirs = vec![youth, adult];
        draft.rendition_attribution = Some("Sr. Maria".to_owned());
        draft.occasions = vec!["Lent".to_owned()];
        let created = library
            .create_arrangement(id, song_id, created_at, draft)
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
            .create_arrangement(cebuano, eighth, at(), cebuano_draft)
            .expect("Cebuano");
        library
            .create_arrangement(english, eighth, at(), english_draft)
            .expect("English");

        assert_eq!(
            library.arrangements_for_song(eighth).expect("song").count(),
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
            .create_arrangement(no_choir, song_id, at(), arrangement_draft("A cappella", 10))
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
            .create_choir(youth, at(), "Youth".to_owned())
            .expect("youth");
        library
            .create_choir(adult, at(), "Adult".to_owned())
            .expect("adult");

        let shared = arrangement(2);
        library
            .create_arrangement(shared, song_id, at(), arrangement_draft("Shared", 20))
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

        assert_eq!(
            library
                .set_arrangement_choirs(shared, vec![youth, youth])
                .unwrap_err(),
            SongLibraryError::DuplicateField(Field::Choir)
        );
        assert_eq!(
            library
                .set_arrangement_choirs(shared, vec![choir(9)])
                .unwrap_err(),
            SongLibraryError::UnknownChoir(choir(9))
        );
        assert_eq!(
            library.add_arrangement_choir(shared, choir(9)).unwrap_err(),
            SongLibraryError::UnknownChoir(choir(9))
        );

        let mut unknown_choir = arrangement_draft("Unknown Choir", 30);
        unknown_choir.choirs = vec![choir(9)];
        assert_eq!(
            library
                .create_arrangement(arrangement(3), song_id, at(), unknown_choir)
                .unwrap_err(),
            SongLibraryError::UnknownChoir(choir(9))
        );
    }

    #[test]
    fn choir_repertoire_lists_each_song_once() {
        let mut library = SongLibrary::new();
        let youth = choir(1);
        library
            .create_choir(youth, at(), "Youth".to_owned())
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
        let mut first_draft = arrangement_draft("Kyrie Cebuano", 10);
        first_draft.choirs = vec![youth];
        let mut second_draft = arrangement_draft("Kyrie English", 20);
        second_draft.choirs = vec![youth];
        let mut unrelated_draft = arrangement_draft("Gloria", 30);
        unrelated_draft.choirs = Vec::new();
        library
            .create_arrangement(first, first_song, at(), first_draft)
            .expect("first arrangement");
        library
            .create_arrangement(second, first_song, at(), second_draft)
            .expect("second arrangement");
        library
            .create_arrangement(unrelated, second_song, at(), unrelated_draft)
            .expect("unrelated");

        let repertoire = library.choir_repertoire(youth).expect("repertoire");
        assert_eq!(
            repertoire.len(),
            1,
            "two qualifying Arrangements of one Song add it once"
        );
        assert_eq!(repertoire[0].id(), first_song);
    }

    #[test]
    fn approved_pagination_follows_the_review_rule() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        let youth = choir(1);
        library
            .create_choir(youth, at(), "Youth".to_owned())
            .expect("choir");

        let id = arrangement(1);
        let mut base = arrangement_draft("Cebuano", 10);
        library
            .create_arrangement(id, song_id, at(), base.clone())
            .expect("arrangement");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::Draft
        );

        let mut draft_edit = base.clone();
        draft_edit.sections[0].lyrics = "Unang bersikulo, giusab".to_owned();
        let current_lyrics = draft_edit.sections[0].lyrics.clone();
        library
            .edit_arrangement(id, draft_edit)
            .expect("draft edit");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::Draft,
            "an unapproved Arrangement stays Draft when its content changes"
        );
        base.sections[0].lyrics = current_lyrics;

        library.approve_pagination(id).expect("approved");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::Approved
        );

        let mut metadata = base.clone();
        metadata.title = "Cebuano (Lent)".to_owned();
        metadata.languages = vec!["Cebuano".to_owned(), "English".to_owned()];
        metadata.choirs = vec![youth];
        metadata.rendition_attribution = Some("Sr. Maria".to_owned());
        metadata.occasions = vec!["Lent".to_owned()];
        library
            .edit_arrangement(id, metadata)
            .expect("metadata edit");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::Approved,
            "title, language, Choir, attribution, and occasion changes do not affect pagination"
        );

        let mut lyrics = base.clone();
        lyrics.sections[0].lyrics = "Bag-ong bersikulo".to_owned();
        library.edit_arrangement(id, lyrics).expect("lyrics edit");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut label = base.clone();
        label.sections[1].label = Some("Refrain".to_owned());
        library.edit_arrangement(id, label).expect("label edit");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut reordered = base.clone();
        reordered.sequence.swap(0, 2);
        library
            .edit_arrangement(id, reordered)
            .expect("sequence edit");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut added = base.clone();
        added
            .sections
            .push(SectionDraft::new(section(19), "Bridge"));
        library.edit_arrangement(id, added).expect("section added");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        let mut removed = base.clone();
        removed.sections.pop();
        removed
            .sequence
            .retain(|occurrence| occurrence.section != section(11));
        let unchanged = removed.clone();
        library
            .edit_arrangement(id, removed)
            .expect("section removed");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::NeedsReview
        );

        library.approve_pagination(id).expect("approved again");
        library
            .edit_arrangement(id, unchanged)
            .expect("identical edit");
        assert_eq!(
            library
                .arrangement(id)
                .expect("arrangement")
                .pagination_state(),
            PaginationState::Approved,
            "an edit that changes no pagination content keeps approval"
        );

        let second = arrangement(2);
        library
            .create_arrangement(second, song_id, at(), arrangement_draft("English", 20))
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
    fn arrangement_validation_names_the_failing_field() {
        let mut library = SongLibrary::new();
        let song_id = song(1);
        library
            .create_song(song_id, at(), SongDraft::titled("Kyrie"))
            .expect("song");
        let youth = choir(1);
        library
            .create_choir(youth, at(), "Youth".to_owned())
            .expect("choir");
        let id = arrangement(1);

        let mut blank_title = arrangement_draft("Title", 10);
        blank_title.title = "  ".to_owned();
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), blank_title)
                .unwrap_err(),
            SongLibraryError::BlankField(Field::ArrangementTitle)
        );

        let mut blank_language = arrangement_draft("Title", 10);
        blank_language.languages = vec![" ".to_owned()];
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), blank_language)
                .unwrap_err(),
            SongLibraryError::BlankField(Field::Language)
        );

        let mut duplicate_language = arrangement_draft("Title", 10);
        duplicate_language.languages = vec!["Cebuano".to_owned(), "Cebuano".to_owned()];
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), duplicate_language)
                .unwrap_err(),
            SongLibraryError::DuplicateField(Field::Language)
        );

        let mut duplicate_choir = arrangement_draft("Title", 10);
        duplicate_choir.choirs = vec![youth, youth];
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), duplicate_choir)
                .unwrap_err(),
            SongLibraryError::DuplicateField(Field::Choir)
        );

        let mut blank_label = arrangement_draft("Title", 10);
        blank_label.sections[0].label = Some("  ".to_owned());
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), blank_label)
                .unwrap_err(),
            SongLibraryError::BlankField(Field::SectionLabel)
        );

        let mut foreign_section = arrangement_draft("Title", 10);
        foreign_section.sequence[0].section = section(99);
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), foreign_section)
                .unwrap_err(),
            SongLibraryError::SectionNotInArrangement {
                arrangement: id,
                section: section(99)
            }
        );

        let mut duplicate_section = arrangement_draft("Title", 10);
        duplicate_section.sections[1].id = duplicate_section.sections[0].id;
        duplicate_section.sequence[1].section = duplicate_section.sections[0].id;
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), duplicate_section)
                .unwrap_err(),
            SongLibraryError::DuplicateSection(section(10))
        );

        let mut duplicate_occurrence = arrangement_draft("Title", 10);
        duplicate_occurrence.sequence[1].id = duplicate_occurrence.sequence[0].id;
        assert_eq!(
            library
                .create_arrangement(id, song_id, at(), duplicate_occurrence)
                .unwrap_err(),
            SongLibraryError::DuplicateOccurrence(occurrence(10))
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
            .create_arrangement(arrangement(1), song_id, at(), arrangement_draft("One", 10))
            .expect("first arrangement");

        let stolen_section = section(10);
        let mut stolen = arrangement_draft("Two", 20);
        stolen.sections[0].id = stolen_section;
        stolen.sequence[0].section = stolen_section;
        stolen.sequence[2].section = stolen_section;
        assert_eq!(
            library
                .create_arrangement(arrangement(2), song_id, at(), stolen)
                .unwrap_err(),
            SongLibraryError::DuplicateSection(stolen_section)
        );

        let stolen_occurrence = occurrence(10);
        let mut stolen = arrangement_draft("Two", 20);
        stolen.sequence[0].id = stolen_occurrence;
        assert_eq!(
            library
                .create_arrangement(arrangement(2), song_id, at(), stolen)
                .unwrap_err(),
            SongLibraryError::DuplicateOccurrence(stolen_occurrence)
        );

        library
            .create_arrangement(arrangement(2), song_id, at(), arrangement_draft("Two", 20))
            .expect("second arrangement");
        let mut stolen = arrangement_draft("Two edited", 20);
        stolen.sections[0].id = stolen_section;
        stolen.sequence[0].section = stolen_section;
        stolen.sequence[2].section = stolen_section;
        assert_eq!(
            library
                .edit_arrangement(arrangement(2), stolen)
                .unwrap_err(),
            SongLibraryError::DuplicateSection(stolen_section)
        );

        library
            .edit_arrangement(arrangement(2), arrangement_draft("Two edited", 20))
            .expect("an edit that keeps its own identities succeeds");
    }

    #[test]
    fn unknown_entities_are_named() {
        let mut library = SongLibrary::new();
        let unknown_song = song(1);
        let unknown_arrangement = arrangement(1);
        let unknown_choir = choir(1);

        assert_eq!(
            library.song(unknown_song).unwrap_err(),
            SongLibraryError::UnknownSong(unknown_song)
        );
        assert_eq!(
            library.arrangement(unknown_arrangement).unwrap_err(),
            SongLibraryError::UnknownArrangement(unknown_arrangement)
        );
        assert_eq!(
            library.choir(unknown_choir).unwrap_err(),
            SongLibraryError::UnknownChoir(unknown_choir)
        );
        assert_eq!(
            library.arrangements_for_song(unknown_song).err(),
            Some(SongLibraryError::UnknownSong(unknown_song))
        );
        assert_eq!(
            library.choir_repertoire(unknown_choir).unwrap_err(),
            SongLibraryError::UnknownChoir(unknown_choir)
        );
        assert_eq!(
            library
                .edit_song(unknown_song, SongDraft::titled("Kyrie"))
                .unwrap_err(),
            SongLibraryError::UnknownSong(unknown_song)
        );
        assert_eq!(
            library
                .edit_arrangement(unknown_arrangement, arrangement_draft("Kyrie", 10))
                .unwrap_err(),
            SongLibraryError::UnknownArrangement(unknown_arrangement)
        );
        assert_eq!(
            library
                .edit_choir(unknown_choir, "Youth".to_owned())
                .unwrap_err(),
            SongLibraryError::UnknownChoir(unknown_choir)
        );
        assert_eq!(
            library.approve_pagination(unknown_arrangement).unwrap_err(),
            SongLibraryError::UnknownArrangement(unknown_arrangement)
        );
        assert_eq!(
            library
                .set_arrangement_choirs(unknown_arrangement, Vec::new())
                .unwrap_err(),
            SongLibraryError::UnknownArrangement(unknown_arrangement)
        );
        assert_eq!(
            library
                .add_arrangement_choir(unknown_arrangement, unknown_choir)
                .unwrap_err(),
            SongLibraryError::UnknownArrangement(unknown_arrangement)
        );
        assert_eq!(
            library
                .remove_arrangement_choir(unknown_arrangement, unknown_choir)
                .unwrap_err(),
            SongLibraryError::UnknownArrangement(unknown_arrangement)
        );
        assert_eq!(
            library
                .create_arrangement(
                    unknown_arrangement,
                    unknown_song,
                    at(),
                    ArrangementDraft::titled("Kyrie")
                )
                .unwrap_err(),
            SongLibraryError::UnknownSong(unknown_song)
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
            .create_choir(choir(2), at(), "Second".to_owned())
            .expect("second choir");
        library
            .create_choir(choir(1), at(), "First".to_owned())
            .expect("first choir");
        assert_eq!(
            library.choirs().map(Choir::id).collect::<Vec<_>>(),
            vec![choir(1), choir(2)]
        );

        library
            .create_arrangement(arrangement(2), song(1), at(), arrangement_draft("Two", 20))
            .expect("second arrangement");
        library
            .create_arrangement(arrangement(1), song(1), at(), arrangement_draft("One", 10))
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
                .map(Arrangement::id)
                .collect::<Vec<_>>(),
            vec![arrangement(1), arrangement(2)]
        );
    }

    #[test]
    fn every_field_and_error_displays_a_message() {
        for field in [
            Field::SongTitle,
            Field::Alias,
            Field::LiturgicalMoment,
            Field::Occasion,
            Field::ChoirName,
            Field::ArrangementTitle,
            Field::Language,
            Field::SectionLabel,
            Field::Choir,
        ] {
            assert!(!field.to_string().is_empty(), "{field:?}");
        }
        for error in [
            SongLibraryError::UnknownSong(song(1)),
            SongLibraryError::UnknownArrangement(arrangement(1)),
            SongLibraryError::UnknownChoir(choir(1)),
            SongLibraryError::DuplicateSong(song(1)),
            SongLibraryError::DuplicateArrangement(arrangement(1)),
            SongLibraryError::DuplicateChoir(choir(1)),
            SongLibraryError::DuplicateSection(section(1)),
            SongLibraryError::DuplicateOccurrence(occurrence(1)),
            SongLibraryError::BlankField(Field::SongTitle),
            SongLibraryError::DuplicateField(Field::Alias),
            SongLibraryError::SectionNotInArrangement {
                arrangement: arrangement(1),
                section: section(1),
            },
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
        }
    }
}
