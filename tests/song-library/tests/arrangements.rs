//! T2.6 acceptance tests for Arrangement details, the derived label, the
//! built-in language list and duplication.
//!
//! Every test goes through the public Interface only. Test names start with
//! the acceptance criterion they cover.

use domain::identity::{
    ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp,
};
use domain::song_library::{
    ArchiveState, ArrangementDetails, ArrangementDraft, ChoirDraft, Field, OccurrenceDraft,
    PaginationState, RecordRef, SectionDraft, SongDraft, SongLibrary, SongLibraryError,
    find_language, is_listed, languages,
};

fn entity(n: u8) -> EntityId {
    EntityId::from_random_bytes([n; 16])
}

fn song_id(n: u8) -> SongId {
    SongId::from_entity(entity(n))
}

fn arrangement_id(n: u8) -> ArrangementId {
    ArrangementId::from_entity(entity(n))
}

fn choir_id(n: u8) -> ChoirId {
    ChoirId::from_entity(entity(n))
}

fn section_id(n: u8) -> SectionId {
    SectionId::from_entity(entity(n))
}

fn occurrence_id(n: u8) -> OccurrenceId {
    OccurrenceId::from_entity(entity(n))
}

fn at(offset: i64) -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000 + offset)
}

/// A library with one Song (1) and Choirs 20 ("Youth Choir"), 21 ("adult
/// Choir"), 22 ("Zion Choir").
fn library() -> SongLibrary {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie"))
        .expect("song");
    for (n, name) in [(20, "Youth Choir"), (21, "adult Choir"), (22, "Zion Choir")] {
        library
            .create_choir(choir_id(n), at(0), ChoirDraft::named(name))
            .expect("choir");
    }
    library
}

/// A one-language draft with two sections and a three-entry Sequence whose
/// identities start at `base`.
fn draft(language: &str, base: u8) -> ArrangementDraft {
    let first = section_id(base);
    let second = section_id(base + 1);
    ArrangementDraft {
        languages: vec![language.to_owned()],
        sections: vec![
            SectionDraft::labeled(first, "Verse", "Unang bersikulo"),
            SectionDraft::labeled(second, "Chorus", "Koro"),
        ],
        sequence: vec![
            OccurrenceDraft::new(occurrence_id(base), first),
            OccurrenceDraft::new(occurrence_id(base + 1), second),
            OccurrenceDraft::new(occurrence_id(base + 2), first),
        ],
        ..ArrangementDraft::default()
    }
}

fn create(library: &mut SongLibrary, n: u8, draft: ArrangementDraft, choirs: Vec<ChoirId>) {
    library
        .create_arrangement(arrangement_id(n), song_id(1), at(10), draft, choirs)
        .expect("arrangement");
}

fn details_of(library: &SongLibrary, n: u8) -> ArrangementDetails {
    library.arrangement(arrangement_id(n)).expect("a").details()
}

#[test]
fn ac2_creating_without_a_language_is_refused_and_stores_nothing() {
    let mut library = library();
    for languages in [vec![], vec!["  ".to_owned()]] {
        let draft = ArrangementDraft {
            languages,
            ..ArrangementDraft::titled("Mass")
        };
        assert_eq!(
            library
                .create_arrangement(arrangement_id(5), song_id(1), at(10), draft, vec![])
                .map(|_| ()),
            Err(SongLibraryError::BlankField(Field::Language))
        );
    }
    assert_eq!(library.arrangements().count(), 0);
}

#[test]
fn ac2_a_details_edit_without_a_language_is_refused_and_changes_nothing() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![choir_id(20)]);
    let before = library.arrangement(arrangement_id(5)).expect("a").clone();
    let mut details = before.details();
    details.languages.clear();
    details.title = Some("Changed".to_owned());
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), details)
            .map(|_| ()),
        Err(SongLibraryError::BlankField(Field::Language))
    );
    assert_eq!(library.arrangement(arrangement_id(5)), Ok(&before));
}

#[test]
fn ac2_a_content_edit_also_needs_a_language() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![]);
    assert_eq!(
        library
            .edit_arrangement(arrangement_id(5), ArrangementDraft::titled("No language"))
            .map(|_| ()),
        Err(SongLibraryError::BlankField(Field::Language))
    );
}

#[test]
fn ac2_a_language_outside_the_built_in_list_is_refused() {
    let mut library = library();
    assert_eq!(
        library
            .create_arrangement(
                arrangement_id(5),
                song_id(1),
                at(10),
                draft("Klingon", 100),
                vec![]
            )
            .map(|_| ()),
        Err(SongLibraryError::UnlistedLanguage("Klingon".to_owned()))
    );
    create(&mut library, 5, draft("Cebuano", 100), vec![]);
    let mut details = details_of(&library, 5);
    // An alias is a search term, not a stored value.
    details.languages = vec!["Ilonggo".to_owned()];
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), details)
            .map(|_| ()),
        Err(SongLibraryError::UnlistedLanguage("Ilonggo".to_owned()))
    );
    assert_eq!(
        library
            .arrangement(arrangement_id(5))
            .expect("a")
            .languages(),
        ["Cebuano"]
    );
}

#[test]
fn ac2_listed_languages_are_stored_under_their_display_name_and_stay_distinct() {
    let mut library = library();
    let mut lowercase = draft("cebuano", 100);
    lowercase.languages.push("ENGLISH".to_owned());
    create(&mut library, 5, lowercase, vec![]);
    assert_eq!(
        library
            .arrangement(arrangement_id(5))
            .expect("a")
            .languages(),
        ["Cebuano", "English"]
    );
    let mut details = details_of(&library, 5);
    details.languages = vec!["Latin".to_owned(), "latin".to_owned()];
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), details)
            .map(|_| ()),
        Err(SongLibraryError::DuplicateField(Field::Language))
    );
}

#[test]
fn ac2_the_language_list_finds_by_name_or_other_name_and_reports_the_alias() {
    let found = find_language("ilong");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].language.name(), "Hiligaynon");
    assert_eq!(found[0].matched_alias, Some("Ilonggo"));

    let by_name = find_language("CEBU");
    assert_eq!(by_name[0].language.name(), "Cebuano");
    assert_eq!(by_name[0].matched_alias, None);

    let folded = find_language("espanol");
    assert_eq!(folded[0].language.name(), "Spanish");
    assert_eq!(folded[0].matched_alias, Some("Español"));
    assert_eq!(find_language("zamboangue")[0].language.name(), "Chavacano");
    assert!(find_language("klingon").is_empty());
    assert_eq!(find_language("").len(), languages().len());

    assert_eq!(languages().len(), 15);
    assert_eq!(languages()[0].name(), "Cebuano");
    assert_eq!(
        languages()[0].other_names(),
        ["Bisaya", "Binisaya", "Sinugbuanon", "Visayan"]
    );
    assert!(is_listed("Chinese (Mandarin)"));
    assert!(is_listed("hiligaynon"));
    assert!(!is_listed("Ilonggo"));
}

#[test]
fn ac2_title_and_choirs_are_optional_but_a_blank_title_is_refused() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![]);
    let stored = library.arrangement(arrangement_id(5)).expect("a");
    assert_eq!(stored.title(), None);
    assert!(stored.choirs().is_empty());

    let whitespace = ArrangementDraft {
        title: Some("   ".to_owned()),
        ..draft("Cebuano", 110)
    };
    assert_eq!(
        library
            .create_arrangement(arrangement_id(6), song_id(1), at(10), whitespace, vec![])
            .map(|_| ()),
        Err(SongLibraryError::BlankField(Field::ArrangementTitle))
    );
    let mut details = details_of(&library, 5);
    details.title = Some("\t".to_owned());
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), details)
            .map(|_| ()),
        Err(SongLibraryError::BlankField(Field::ArrangementTitle))
    );
    let mut details = details_of(&library, 5);
    details.notes = Some(" ".to_owned());
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), details)
            .map(|_| ()),
        Err(SongLibraryError::BlankField(Field::ArrangementNotes))
    );
}

#[test]
fn ac2_a_blank_title_derives_the_label_from_languages_and_choirs() {
    let mut library = library();

    let mut titled = draft("Cebuano", 100);
    titled.title = Some("Ginoo, Kaloy-i Kami".to_owned());
    create(&mut library, 5, titled, vec![choir_id(20)]);
    assert_eq!(
        library.arrangement_label(arrangement_id(5)).as_deref(),
        Ok("Ginoo, Kaloy-i Kami")
    );

    create(&mut library, 6, draft("Cebuano", 110), vec![]);
    assert_eq!(
        library.arrangement_label(arrangement_id(6)).as_deref(),
        Ok("Cebuano")
    );

    let mut two = draft("Cebuano", 120);
    two.languages.push("English".to_owned());
    create(&mut library, 7, two, vec![]);
    assert_eq!(
        library.arrangement_label(arrangement_id(7)).as_deref(),
        Ok("Cebuano + English")
    );

    create(&mut library, 8, draft("English", 130), vec![choir_id(20)]);
    assert_eq!(
        library.arrangement_label(arrangement_id(8)).as_deref(),
        Ok("English \u{b7} Youth Choir")
    );

    // Ordered by Choir name key ("adult choir" < "youth choir" < "zion
    // choir"), not by identity or by the order given.
    create(
        &mut library,
        9,
        draft("English", 140),
        vec![choir_id(22), choir_id(20), choir_id(21)],
    );
    assert_eq!(
        library.arrangement_label(arrangement_id(9)).as_deref(),
        Ok("English \u{b7} adult Choir +2")
    );

    // Archived Choirs still count.
    library
        .archive_choir(choir_id(21), at(50))
        .expect("archive choir");
    assert_eq!(
        library.arrangement_label(arrangement_id(9)).as_deref(),
        Ok("English \u{b7} adult Choir +2")
    );

    assert_eq!(
        library.arrangement_label(arrangement_id(99)),
        Err(SongLibraryError::UnknownArrangement(arrangement_id(99)))
    );
}

#[test]
fn ac6_a_details_edit_keeps_sections_sequence_and_approved_pagination() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![choir_id(20)]);
    library
        .approve_pagination(arrangement_id(5))
        .expect("approve");
    let before = library.arrangement(arrangement_id(5)).expect("a").clone();

    let details = ArrangementDetails {
        title: Some("Lenten setting".to_owned()),
        languages: vec!["Cebuano".to_owned(), "Latin".to_owned()],
        choirs: vec![choir_id(21), choir_id(22)],
        rendition_attribution: Some("Parish adaptation".to_owned()),
        occasions: vec!["Lent".to_owned(), "Advent".to_owned()],
        notes: Some("Scanned from the 2019 booklet".to_owned()),
    };
    let after = library
        .edit_arrangement_details(arrangement_id(5), details)
        .expect("edit")
        .clone();

    assert_eq!(after.title(), Some("Lenten setting"));
    assert_eq!(after.languages(), ["Cebuano", "Latin"]);
    assert_eq!(
        after.choirs().iter().copied().collect::<Vec<_>>(),
        vec![choir_id(21), choir_id(22)]
    );
    assert_eq!(after.rendition_attribution(), Some("Parish adaptation"));
    assert_eq!(
        after
            .occasions()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Advent", "Lent"]
    );
    assert_eq!(after.notes(), Some("Scanned from the 2019 booklet"));
    assert_eq!(after.sections(), before.sections());
    assert_eq!(after.sequence(), before.sequence());
    assert_eq!(after.pagination_state(), PaginationState::Approved);
    assert_eq!(after.created_at(), before.created_at());
    assert_eq!(after.song(), before.song());
    assert_eq!(after.archive_state(), ArchiveState::Active);
    assert_eq!(
        after.details().occasions,
        vec!["Advent".to_owned(), "Lent".to_owned()]
    );
}

#[test]
fn ac6_a_details_edit_is_atomic() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![choir_id(20)]);
    library
        .archive_choir(choir_id(22), at(50))
        .expect("archive choir");
    let before = library.arrangement(arrangement_id(5)).expect("a").clone();

    let mut details = before.details();
    details.title = Some("Changed".to_owned());
    details.languages = vec!["English".to_owned()];
    details.notes = Some("Changed".to_owned());

    let mut unknown = details.clone();
    unknown.choirs = vec![choir_id(20), choir_id(77)];
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), unknown)
            .map(|_| ()),
        Err(SongLibraryError::UnknownChoir(choir_id(77)))
    );
    assert_eq!(library.arrangement(arrangement_id(5)), Ok(&before));

    let mut archived = details.clone();
    archived.choirs = vec![choir_id(20), choir_id(22)];
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), archived)
            .map(|_| ()),
        Err(SongLibraryError::Archived(RecordRef::Choir(choir_id(22))))
    );
    assert_eq!(library.arrangement(arrangement_id(5)), Ok(&before));

    let mut repeated = details.clone();
    repeated.choirs = vec![choir_id(20), choir_id(20)];
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), repeated)
            .map(|_| ()),
        Err(SongLibraryError::DuplicateField(Field::Choir))
    );
    assert_eq!(library.arrangement(arrangement_id(5)), Ok(&before));

    // A valid Choir list with an invalid occasion still changes nothing.
    let mut occasion = details.clone();
    occasion.choirs = vec![choir_id(21)];
    occasion.occasions = vec![" ".to_owned()];
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), occasion)
            .map(|_| ()),
        Err(SongLibraryError::BlankField(Field::Occasion))
    );
    assert_eq!(library.arrangement(arrangement_id(5)), Ok(&before));

    // An unknown Arrangement and an archived one are refused.
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(88), details.clone())
            .map(|_| ()),
        Err(SongLibraryError::UnknownArrangement(arrangement_id(88)))
    );
    library
        .archive_arrangement(arrangement_id(5), at(60))
        .expect("archive");
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(5), details)
            .map(|_| ()),
        Err(SongLibraryError::Archived(RecordRef::Arrangement(
            arrangement_id(5)
        )))
    );
}

#[test]
fn ac6_duplicate_branches_an_independent_copy_with_new_identities() {
    let mut library = library();
    let mut source = draft("Cebuano", 100);
    source.title = Some("Kyrie".to_owned());
    source.rendition_attribution = Some("R. Villanueva".to_owned());
    source.occasions = vec!["Lent".to_owned()];
    source.notes = Some("Rights: parish".to_owned());
    create(&mut library, 5, source, vec![choir_id(20), choir_id(21)]);
    library
        .approve_pagination(arrangement_id(5))
        .expect("approve");
    library
        .archive_choir(choir_id(21), at(50))
        .expect("archive choir");
    let original = library.arrangement(arrangement_id(5)).expect("a").clone();

    let copy = library
        .duplicate_arrangement(
            arrangement_id(5),
            arrangement_id(6),
            at(100),
            vec![section_id(151), section_id(152)],
            vec![occurrence_id(151), occurrence_id(152), occurrence_id(153)],
        )
        .expect("duplicate")
        .clone();

    assert_eq!(copy.id(), arrangement_id(6));
    assert_eq!(copy.song(), original.song());
    assert_eq!(copy.created_at(), at(100));
    assert_eq!(copy.title(), Some("Kyrie"));
    assert_eq!(copy.languages(), original.languages());
    assert_eq!(copy.rendition_attribution(), Some("R. Villanueva"));
    assert_eq!(copy.occasions(), original.occasions());
    assert_eq!(copy.notes(), Some("Rights: parish"));
    assert_eq!(copy.pagination_state(), PaginationState::Draft);
    assert_eq!(copy.archive_state(), ArchiveState::Active);
    // The archived Choir is dropped.
    assert_eq!(
        copy.choirs().iter().copied().collect::<Vec<_>>(),
        vec![choir_id(20)]
    );
    assert_eq!(
        copy.sections()
            .iter()
            .map(|s| (s.id(), s.label(), s.lyrics()))
            .collect::<Vec<_>>(),
        vec![
            (section_id(151), Some("Verse"), "Unang bersikulo"),
            (section_id(152), Some("Chorus"), "Koro"),
        ]
    );
    // The Sequence maps onto the new section identities, repeats included.
    assert_eq!(
        copy.sequence()
            .iter()
            .map(|o| (o.id(), o.section()))
            .collect::<Vec<_>>(),
        vec![
            (occurrence_id(151), section_id(151)),
            (occurrence_id(152), section_id(152)),
            (occurrence_id(153), section_id(151)),
        ]
    );
    // The source is untouched, including its Approved pagination.
    assert_eq!(library.arrangement(arrangement_id(5)), Ok(&original));
    assert_eq!(
        library
            .arrangements_for_song(song_id(1))
            .expect("song")
            .len(),
        2
    );

    // Later edits to either side do not reach the other.
    let mut details = details_of(&library, 6);
    details.title = Some("Kyrie (copy)".to_owned());
    library
        .edit_arrangement_details(arrangement_id(6), details)
        .expect("edit copy");
    let mut source_draft = draft("Cebuano", 100);
    source_draft.sections[0].lyrics = "Bag-ong bersikulo".to_owned();
    library
        .edit_arrangement(arrangement_id(5), source_draft)
        .expect("edit source");
    let copy = library.arrangement(arrangement_id(6)).expect("copy");
    assert_eq!(copy.title(), Some("Kyrie (copy)"));
    assert_eq!(copy.sections()[0].lyrics(), "Unang bersikulo");
    let source = library.arrangement(arrangement_id(5)).expect("source");
    assert_eq!(source.title(), None);
    assert_eq!(source.sections()[0].lyrics(), "Bag-ong bersikulo");
}

#[test]
fn ac6_duplicate_refuses_collisions_wrong_counts_and_an_archived_song() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![]);
    create(&mut library, 6, draft("English", 110), vec![]);
    let sections = || vec![section_id(201), section_id(202)];
    let occurrences = || vec![occurrence_id(201), occurrence_id(202), occurrence_id(203)];
    let attempt = |library: &mut SongLibrary,
                   new: u8,
                   sections: Vec<SectionId>,
                   occurrences: Vec<OccurrenceId>| {
        library
            .duplicate_arrangement(
                arrangement_id(5),
                arrangement_id(new),
                at(100),
                sections,
                occurrences,
            )
            .map(|_| ())
    };

    assert_eq!(
        attempt(&mut library, 6, sections(), occurrences()),
        Err(SongLibraryError::DuplicateArrangement(arrangement_id(6)))
    );
    // Identities that belong to the source or to another Arrangement.
    assert_eq!(
        attempt(
            &mut library,
            7,
            vec![section_id(100), section_id(202)],
            occurrences()
        ),
        Err(SongLibraryError::DuplicateSection(section_id(100)))
    );
    assert_eq!(
        attempt(
            &mut library,
            7,
            sections(),
            vec![occurrence_id(201), occurrence_id(112), occurrence_id(203)]
        ),
        Err(SongLibraryError::DuplicateOccurrence(occurrence_id(112)))
    );
    // Repeats within the new identities.
    assert_eq!(
        attempt(
            &mut library,
            7,
            vec![section_id(201), section_id(201)],
            occurrences()
        ),
        Err(SongLibraryError::DuplicateSection(section_id(201)))
    );
    assert_eq!(
        attempt(&mut library, 7, vec![section_id(201)], occurrences()),
        Err(SongLibraryError::IdentityCountMismatch {
            sections: 2,
            occurrences: 3
        })
    );
    assert_eq!(library.arrangements().count(), 2);

    // The failures reserved nothing: the same identities still work.
    assert_eq!(attempt(&mut library, 7, sections(), occurrences()), Ok(()));

    library.archive_song(song_id(1), at(200)).expect("archive");
    assert_eq!(
        library
            .duplicate_arrangement(
                arrangement_id(5),
                arrangement_id(8),
                at(300),
                vec![section_id(231), section_id(232)],
                vec![occurrence_id(231), occurrence_id(232), occurrence_id(233)],
            )
            .map(|_| ()),
        Err(SongLibraryError::Archived(RecordRef::Song(song_id(1))))
    );
}

#[test]
fn ac6_an_archived_source_may_be_duplicated_and_is_copied_as_stored() {
    let mut library = library();
    create(&mut library, 5, draft("Cebuano", 100), vec![choir_id(20)]);
    library
        .archive_arrangement(arrangement_id(5), at(60))
        .expect("archive");
    let copy = library
        .duplicate_arrangement(
            arrangement_id(5),
            arrangement_id(6),
            at(100),
            vec![section_id(201), section_id(202)],
            vec![occurrence_id(201), occurrence_id(202), occurrence_id(203)],
        )
        .expect("duplicate of an archived source");
    assert_eq!(copy.archive_state(), ArchiveState::Active);
    assert_eq!(copy.languages(), ["Cebuano"]);
}

/// A library holding Song 1, Choirs 20 and 21, and legacy Arrangements 5
/// (titled, no language), 6 (untitled, no language, no Choir) and 7 (untitled,
/// no language, Choir "Koro" only), plus 8 (unlisted language "ceb", "en").
fn legacy_library() -> SongLibrary {
    use domain::song_library::{HydrateArrangement, HydrateChoir, HydrateSong};
    let stored = |n: u8, title: Option<&str>, languages: &[&str], choirs: Vec<ChoirId>| {
        let mut draft = ArrangementDraft::titled("x");
        draft.title = title.map(str::to_owned);
        draft.languages = languages.iter().map(|l| (*l).to_owned()).collect();
        HydrateArrangement {
            id: arrangement_id(n),
            song: song_id(1),
            created_at: at(10),
            draft,
            choirs,
            pagination_state: PaginationState::Draft,
            archive_state: ArchiveState::Active,
        }
    };
    SongLibrary::hydrate(
        vec![HydrateSong {
            id: song_id(1),
            created_at: at(0),
            draft: SongDraft::titled("Kyrie"),
            archive_state: ArchiveState::Active,
        }],
        vec![
            stored(5, Some("Legacy"), &[], vec![]),
            stored(6, None, &[], vec![]),
            stored(7, None, &[], vec![choir_id(20)]),
            stored(8, None, &["ceb", "en"], vec![]),
        ],
        vec![HydrateChoir {
            id: choir_id(20),
            created_at: at(0),
            draft: ChoirDraft::named("Koro"),
            archive_state: ArchiveState::Active,
        }],
        vec![],
    )
    .expect("legacy records load")
}

#[test]
fn ac2_a_stored_arrangement_without_a_language_still_loads_and_labels_but_cannot_be_duplicated() {
    let mut library = legacy_library();
    assert_eq!(
        library.arrangement_label(arrangement_id(5)).as_deref(),
        Ok("Legacy")
    );
    assert_eq!(
        library.arrangement_label(arrangement_id(6)).as_deref(),
        Ok("Untitled")
    );
    assert_eq!(
        library.arrangement_label(arrangement_id(7)).as_deref(),
        Ok("Koro")
    );
    // Duplicating is a creation, so the language rule applies.
    for source in [5, 6, 7] {
        assert_eq!(
            library
                .duplicate_arrangement(
                    arrangement_id(source),
                    arrangement_id(50),
                    at(100),
                    vec![],
                    vec![],
                )
                .map(|_| ()),
            Err(SongLibraryError::BlankField(Field::Language))
        );
    }
    assert!(library.arrangement(arrangement_id(50)).is_err());
}

#[test]
fn ac2_an_unlisted_legacy_language_may_be_kept_but_not_added() {
    let mut library = legacy_library();
    let mut details = details_of(&library, 8);
    details.notes = Some("kept".to_owned());
    // Keeping "ceb" and "en" as stored is fine.
    library
        .edit_arrangement_details(arrangement_id(8), details.clone())
        .expect("legacy languages kept");
    assert_eq!(
        library
            .arrangement(arrangement_id(8))
            .expect("a")
            .languages(),
        ["ceb", "en"]
    );
    // Adding a built-in language next to them is fine too.
    details.languages.push("Latin".to_owned());
    library
        .edit_arrangement_details(arrangement_id(8), details.clone())
        .expect("a listed language added");
    assert_eq!(
        library
            .arrangement(arrangement_id(8))
            .expect("a")
            .languages(),
        ["ceb", "en", "Latin"]
    );
    // An unlisted language that was never stored is still refused.
    details.languages.push("Klingon".to_owned());
    assert_eq!(
        library
            .edit_arrangement_details(arrangement_id(8), details)
            .map(|_| ()),
        Err(SongLibraryError::UnlistedLanguage("Klingon".to_owned()))
    );
    // A lyrics edit keeps them as well.
    let mut draft = ArrangementDraft::titled("x");
    draft.languages = vec!["ceb".to_owned()];
    library
        .edit_arrangement(arrangement_id(8), draft.clone())
        .expect("content edit keeps a stored language");
    draft.languages = vec!["ceb".to_owned(), "xx".to_owned()];
    assert_eq!(
        library
            .edit_arrangement(arrangement_id(8), draft)
            .map(|_| ()),
        Err(SongLibraryError::UnlistedLanguage("xx".to_owned()))
    );
}
