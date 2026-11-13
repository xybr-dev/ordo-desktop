//! T2.7 acceptance tests for slide boundaries, section operations and the
//! Lyrics edit.
//!
//! Every test goes through the public Interface only. Test names start with
//! the acceptance criterion they cover.

use domain::identity::{
    ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp,
};
use domain::pagination::{
    join_with_previous, slide_at, slide_body, slides, split_after, split_point,
};
use domain::song_library::{
    ArchiveState, ArrangementDraft, ArrangementLyrics, ChoirDraft, Field, HydrateArrangement,
    HydrateSong, OccurrenceDraft, PaginationState, RecordRef, SectionDraft, SongDraft, SongLibrary,
    SongLibraryError, Step, section_name_suggestions,
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

/// A library with one Song (1).
fn library() -> SongLibrary {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie"))
        .expect("song");
    library
}

/// A one-language draft with the given sections and Sequence.
fn draft(
    language: &str,
    sections: Vec<SectionDraft>,
    sequence: Vec<OccurrenceDraft>,
) -> ArrangementDraft {
    ArrangementDraft {
        languages: vec![language.to_owned()],
        sections,
        sequence,
        ..ArrangementDraft::default()
    }
}

fn create(
    library: &mut SongLibrary,
    id: ArrangementId,
    draft: ArrangementDraft,
    choirs: Vec<ChoirId>,
) {
    library
        .create_arrangement(id, song_id(1), at(10), draft, choirs)
        .expect("arrangement");
}

fn lyrics_of(library: &SongLibrary, id: ArrangementId) -> ArrangementLyrics {
    library.arrangement(id).expect("arrangement").lyrics()
}

fn assert_refused_lyrics(
    library: &mut SongLibrary,
    id: ArrangementId,
    next: ArrangementLyrics,
    expected: SongLibraryError,
) {
    let before = lyrics_of(library, id);
    assert_eq!(
        library.edit_arrangement_lyrics(id, next).map(|_| ()),
        Err(expected)
    );
    assert_eq!(
        lyrics_of(library, id),
        before,
        "a refused lyrics edit changes nothing"
    );
}

#[test]
fn ac4_newline_continues_and_blank_line_starts_a_slide() {
    let lyrics = "Unang linya\nIkaduhang linya\n\nKoro\nKatapusan";
    let generated = slides(lyrics);
    assert_eq!(generated.len(), 2);
    assert_eq!(
        slide_body(lyrics, &generated[0]),
        "Unang linya\nIkaduhang linya"
    );
    assert_eq!(slide_body(lyrics, &generated[1]), "Koro\nKatapusan");
    assert_eq!(generated[0].range, 0.."Unang linya\nIkaduhang linya".len());
    assert_eq!(generated[0].lines.len(), 2);
    assert_eq!(generated[1].lines.len(), 2);
}

#[test]
fn ac4_removing_the_blank_line_joins() {
    let before = "Niño\n\nGinoó";
    assert_eq!(slides(before).len(), 2);

    let joined = join_with_previous(before, 1).expect("join");
    assert_eq!(joined, "Niño\nGinoó");
    let generated = slides(&joined);
    assert_eq!(generated.len(), 1);
    assert_eq!(slide_body(&joined, &generated[0]), "Niño\nGinoó");
}

#[test]
fn ac4_whitespace_runs_and_crlf_are_one_boundary() {
    let lyrics = "\r\nNiño\r\nGinoó\r\n \t \r\n\r\nAleluya\r\n";
    let generated = slides(lyrics);
    assert_eq!(generated.len(), 2);
    assert_eq!(slide_body(lyrics, &generated[0]), "Niño\nGinoó");
    assert_eq!(slide_body(lyrics, &generated[1]), "Aleluya");

    assert!(slides("").is_empty(), "empty lyrics make no slide");
    assert!(
        slides(" \n\t\n\r\n").is_empty(),
        "all-blank lyrics make no slide"
    );
    assert!(
        slides("\n\nKay ang Ginoó\n").len() == 1,
        "leading and trailing blank runs make no extra slide"
    );
}

#[test]
fn ac5_split_inserts_one_blank_line_and_keeps_other_bytes() {
    let lyrics = "Niño \nGinoó\nAleluya";
    let generated = slides(lyrics);
    assert_eq!(generated.len(), 1);
    assert_eq!(generated[0].lines.len(), 3);
    assert_eq!(
        generated[0].lines[0].end, 6,
        "ranges are UTF-8 byte offsets: five characters, six bytes"
    );
    assert_eq!(split_point(&generated[0]), Some(2));

    let split = split_after(lyrics, 0, 2).expect("split");
    assert_eq!(split, "Niño \nGinoó\n\nAleluya");
    assert_eq!(
        split.len(),
        lyrics.len() + 1,
        "Split inserts exactly one byte"
    );
    assert_eq!(slides(&split).len(), 2);
    assert_eq!(
        slide_body(&split, &slides(&split)[0]),
        "Niño\nGinoó",
        "the slide body trims trailing whitespace for rendering only"
    );

    assert_eq!(split_after(lyrics, 0, 0), None);
    assert_eq!(split_after(lyrics, 0, 3), None);
    assert_eq!(split_after(lyrics, 1, 1), None);

    let four = "a\nb\nc\nd";
    assert_eq!(split_point(&slides(four)[0]), Some(2));
    assert_eq!(split_after(four, 0, 2).as_deref(), Some("a\nb\n\nc\nd"));
    let single = slides("solo");
    assert_eq!(
        split_point(&single[0]),
        None,
        "a one-line slide cannot split"
    );
}

#[test]
fn ac5_join_is_refused_for_the_first_slide() {
    let lyrics = "a\n\nb";
    assert_eq!(join_with_previous(lyrics, 0), None);
    assert_eq!(join_with_previous(lyrics, 2), None);
    assert_eq!(join_with_previous(lyrics, 1).as_deref(), Some("a\nb"));

    assert_eq!(
        join_with_previous("a\n \t \r\nb", 1).as_deref(),
        Some("a\nb"),
        "the whole blank run becomes one newline"
    );
}

#[test]
fn slide_at_maps_offsets_to_slides() {
    let lyrics = "Niño\nGinoó\n\nAleluya";
    let generated = slides(lyrics);
    assert_eq!(generated.len(), 2);
    let first = &generated[0];
    let second = &generated[1];

    assert_eq!(slide_at(lyrics, 0), Some(0));
    assert_eq!(slide_at(lyrics, first.lines[0].end), Some(0));
    assert_eq!(slide_at(lyrics, first.range.end), Some(0));
    assert_eq!(slide_at(lyrics, second.range.start - 1), Some(0));
    assert_eq!(slide_at(lyrics, second.range.start), Some(1));
    assert_eq!(slide_at(lyrics, lyrics.len()), Some(1));
    assert_eq!(slide_at(lyrics, lyrics.len() + 100), Some(1));

    let leading = "\n\nNiño";
    assert_eq!(slide_at(leading, 0), Some(0));
    assert_eq!(slide_at(leading, 1), Some(0));

    assert_eq!(slide_at("", 0), None);
    assert_eq!(slide_at(" \n\t\n", 3), None);
}

#[test]
fn ac2_an_arrangement_with_no_sections_is_valid_and_gets_nothing_added() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft("English", Vec::new(), Vec::new()),
        Vec::new(),
    );

    let lyrics = lyrics_of(&library, id);
    assert!(lyrics.sections.is_empty());
    assert!(lyrics.sequence.is_empty());
    assert_eq!(lyrics.pagination, PaginationState::Draft);

    library
        .edit_arrangement_lyrics(id, lyrics.clone())
        .expect("an empty lyrics edit is valid");
    assert_eq!(
        lyrics_of(&library, id),
        lyrics,
        "Ordo adds no section on its own"
    );
}

#[test]
fn ac3_names_are_required_nonblank_and_unique_ignoring_case() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Verse 1", "a"),
                SectionDraft::labeled(section_id(101), "Chorus", "b"),
            ],
            vec![OccurrenceDraft::new(occurrence_id(200), section_id(100))],
        ),
        Vec::new(),
    );
    let mut lyrics = lyrics_of(&library, id);

    assert_eq!(
        lyrics.add_section(section_id(102), "  "),
        Err(SongLibraryError::BlankField(Field::SectionLabel))
    );
    assert_eq!(
        lyrics.add_section(section_id(102), "verse  1"),
        Err(SongLibraryError::DuplicateField(Field::SectionLabel))
    );
    assert_eq!(
        lyrics.add_section(section_id(100), "Bridge"),
        Err(SongLibraryError::DuplicateSection(section_id(100)))
    );
    assert_eq!(
        lyrics.rename_section(section_id(99), "Anything"),
        Err(SongLibraryError::UnknownSection(section_id(99)))
    );
    assert_eq!(
        lyrics.rename_section(section_id(101), "verse 1"),
        Err(SongLibraryError::DuplicateField(Field::SectionLabel))
    );
    assert_eq!(
        lyrics.rename_section(section_id(101), " "),
        Err(SongLibraryError::BlankField(Field::SectionLabel))
    );

    let index = lyrics
        .add_section(section_id(102), "Bridge")
        .expect("a new, free name");
    assert_eq!(index, 2, "new sections are appended");
    lyrics
        .rename_section(section_id(100), "verse  1")
        .expect("a section may keep its own match key");
    library
        .edit_arrangement_lyrics(id, lyrics.clone())
        .expect("valid names");

    let stored = lyrics_of(&library, id);
    assert_eq!(stored.sections.len(), 3);
    assert_eq!(stored.sections[2].label.as_deref(), Some("Bridge"));
    assert_eq!(stored.sections[0].label.as_deref(), Some("verse  1"));

    // Uniqueness is enforced where a name is introduced, not on the whole edit.
    let mut more = stored;
    assert_eq!(
        more.add_section(section_id(103), "CHORUS"),
        Err(SongLibraryError::DuplicateField(Field::SectionLabel))
    );
    assert_eq!(more.sections.len(), 3, "a refused add changes nothing");
}

#[test]
fn ac3_suggestions_number_the_next_verse_and_skip_taken_names() {
    let empty = ArrangementLyrics {
        sections: Vec::new(),
        sequence: Vec::new(),
        pagination: PaginationState::Draft,
    };
    let (prefill, suggestions) = section_name_suggestions(&["English".to_owned()], &empty);
    assert_eq!(prefill, "Verse 1");
    assert_eq!(
        suggestions,
        vec!["Verse 1", "Chorus", "Refrain", "Bridge", "Ending"]
    );

    let mut english = empty.clone();
    english
        .add_section(section_id(1), "Verse 1")
        .expect("section");
    english
        .add_section(section_id(2), "Verse 3")
        .expect("section");
    english
        .add_section(section_id(3), "chorus")
        .expect("section");
    let (prefill, suggestions) = section_name_suggestions(&["English".to_owned()], &english);
    assert_eq!(prefill, "Verse 4");
    assert_eq!(suggestions, vec!["Verse 4", "Refrain", "Bridge", "Ending"]);

    let mut cebuano = empty.clone();
    cebuano
        .add_section(section_id(10), "Bersikulo 2")
        .expect("section");
    cebuano
        .add_section(section_id(11), "Koro")
        .expect("section");
    let (prefill, suggestions) =
        section_name_suggestions(&["Cebuano".to_owned(), "English".to_owned()], &cebuano);
    assert_eq!(prefill, "Bersikulo 3");
    assert_eq!(suggestions, vec!["Bersikulo 3", "Katapusan"]);

    let (prefill, _) = section_name_suggestions(&["Spanish".to_owned()], &empty);
    assert_eq!(prefill, "Verse 1", "other languages use the English set");
}

#[test]
fn ac6_one_section_generates_several_slides_without_new_sections_or_occurrences() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "Cebuano",
            vec![SectionDraft::labeled(section_id(100), "Verse 1", "")],
            vec![
                OccurrenceDraft::new(occurrence_id(200), section_id(100)),
                OccurrenceDraft::new(occurrence_id(201), section_id(100)),
            ],
        ),
        Vec::new(),
    );

    let mut lyrics = lyrics_of(&library, id);
    let text = "Niño\nGinoó\n\nAleluya";
    lyrics
        .set_section_lyrics(section_id(100), text.to_owned())
        .expect("lyrics");
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("lyrics edit");

    let stored = lyrics_of(&library, id);
    assert_eq!(stored.sections.len(), 1);
    assert_eq!(stored.sequence.len(), 2);
    assert_eq!(stored.occurrences_of(section_id(100)), 2);
    assert_eq!(
        stored.sections[0].lyrics, text,
        "stored text is not normalized"
    );
    assert_eq!(slides(&stored.sections[0].lyrics).len(), 2);
}

#[test]
fn ac8_moving_a_section_keeps_the_sequence_and_the_review_state() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Verse 1", "a"),
                SectionDraft::labeled(section_id(101), "Chorus", "b"),
            ],
            vec![OccurrenceDraft::new(occurrence_id(200), section_id(101))],
        ),
        Vec::new(),
    );
    library.approve_pagination(id).expect("approved");
    let baseline = lyrics_of(&library, id);

    let mut moved = baseline.clone();
    assert_eq!(moved.move_section(section_id(101), Step::Up), Ok(true));
    assert_eq!(
        moved.sequence, baseline.sequence,
        "moving never edits the Sequence"
    );
    assert_eq!(moved.pagination, PaginationState::Approved);
    let moved = moved.reviewed_against(&baseline);
    assert_eq!(
        moved.pagination,
        PaginationState::Approved,
        "reordering the Lyrics list alone keeps approval"
    );
    library.edit_arrangement_lyrics(id, moved).expect("reorder");

    let stored = lyrics_of(&library, id);
    assert_eq!(
        stored.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![section_id(101), section_id(100)]
    );
    assert_eq!(stored.sequence, baseline.sequence);
    assert_eq!(stored.pagination, PaginationState::Approved);

    assert_eq!(
        stored.clone().move_section(section_id(101), Step::Up),
        Ok(false),
        "the first section cannot move up"
    );
    assert_eq!(
        stored.clone().move_section(section_id(100), Step::Down),
        Ok(false),
        "the last section cannot move down"
    );
}

#[test]
fn ac9_removing_a_used_section_removes_its_occurrences_atomically() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Verse 1", "a"),
                SectionDraft::labeled(section_id(101), "Chorus", "b"),
                SectionDraft::labeled(section_id(102), "Bridge", "c"),
            ],
            vec![
                OccurrenceDraft::new(occurrence_id(200), section_id(100)),
                OccurrenceDraft::new(occurrence_id(201), section_id(101)),
                OccurrenceDraft::new(occurrence_id(202), section_id(100)),
                OccurrenceDraft::new(occurrence_id(203), section_id(102)),
                OccurrenceDraft::new(occurrence_id(204), section_id(101)),
            ],
        ),
        Vec::new(),
    );
    library.approve_pagination(id).expect("approved");
    let baseline = lyrics_of(&library, id);

    let mut removed = baseline.clone();
    assert_eq!(removed.remove_section(section_id(101)), Ok(2));
    assert_eq!(
        removed.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![section_id(100), section_id(102)]
    );
    assert_eq!(
        removed.sequence.iter().map(|o| o.id).collect::<Vec<_>>(),
        vec![occurrence_id(200), occurrence_id(202), occurrence_id(203)],
        "remaining occurrences keep their relative order"
    );
    assert_eq!(removed.occurrences_of(section_id(101)), 0);
    let removed = removed.reviewed_against(&baseline);
    assert_eq!(removed.pagination, PaginationState::NeedsReview);
    library
        .edit_arrangement_lyrics(id, removed)
        .expect("remove");

    assert_eq!(lyrics_of(&library, id).occurrences_of(section_id(101)), 0);
    assert_eq!(
        baseline.clone().remove_section(section_id(99)),
        Err(SongLibraryError::UnknownSection(section_id(99)))
    );

    let mut refused = baseline.clone();
    refused.remove_section(section_id(101)).expect("remove");
    refused
        .sections
        .push(SectionDraft::labeled(section_id(103), "  ", "new words"));
    assert_refused_lyrics(
        &mut library,
        id,
        refused,
        SongLibraryError::BlankField(Field::SectionLabel),
    );
}

#[test]
fn ac9_restoring_the_previous_lyrics_restores_approval_and_positions() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Verse 1", "first words"),
                SectionDraft::labeled(section_id(101), "Chorus", "chorus words"),
            ],
            vec![
                OccurrenceDraft::new(occurrence_id(200), section_id(100)),
                OccurrenceDraft::new(occurrence_id(201), section_id(101)),
                OccurrenceDraft::new(occurrence_id(202), section_id(100)),
            ],
        ),
        Vec::new(),
    );
    library.approve_pagination(id).expect("approved");
    let before = lyrics_of(&library, id);

    let mut forward = before.clone();
    forward
        .set_section_lyrics(section_id(100), "changed words".to_owned())
        .expect("lyrics");
    assert_eq!(forward.remove_section(section_id(101)), Ok(1));
    let forward = forward.reviewed_against(&before);
    assert_eq!(forward.pagination, PaginationState::NeedsReview);
    library
        .edit_arrangement_lyrics(id, forward)
        .expect("forward edit");

    let changed = lyrics_of(&library, id);
    assert_eq!(changed.pagination, PaginationState::NeedsReview);
    assert_eq!(changed.sections.len(), 1);
    assert_eq!(changed.sequence.len(), 2);

    library
        .edit_arrangement_lyrics(id, before.clone())
        .expect("undo");
    assert_eq!(
        lyrics_of(&library, id),
        before,
        "Undo restores lyrics, sections, occurrence identities and approval"
    );
    assert_eq!(before.clone().reviewed_against(&before), before);
}

#[test]
fn rename_keeps_approval() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Verse 1", "a"),
                SectionDraft::labeled(section_id(101), "Chorus", "b"),
            ],
            vec![OccurrenceDraft::new(occurrence_id(200), section_id(100))],
        ),
        Vec::new(),
    );
    library.approve_pagination(id).expect("approved");
    let baseline = lyrics_of(&library, id);

    let mut renamed = baseline.clone();
    renamed
        .rename_section(section_id(100), "Opening Verse")
        .expect("rename");
    let renamed = renamed.reviewed_against(&baseline);
    assert_eq!(
        renamed.pagination,
        PaginationState::Approved,
        "rename alone does not trigger review"
    );
    library
        .edit_arrangement_lyrics(id, renamed)
        .expect("rename");
    assert_eq!(
        library
            .arrangement(id)
            .expect("arrangement")
            .pagination_state(),
        PaginationState::Approved
    );

    let arrangement = library.arrangement(id).expect("arrangement");
    let mut whole = ArrangementDraft {
        title: arrangement.title().map(str::to_owned),
        languages: arrangement.languages().to_vec(),
        rendition_attribution: arrangement.rendition_attribution().map(str::to_owned),
        occasions: arrangement.occasions().iter().cloned().collect(),
        notes: arrangement.notes().map(str::to_owned),
        sections: arrangement
            .sections()
            .iter()
            .map(|section| SectionDraft {
                id: section.id(),
                label: section.label().map(str::to_owned),
                lyrics: section.lyrics().to_owned(),
            })
            .collect(),
        sequence: arrangement
            .sequence()
            .iter()
            .map(|occurrence| OccurrenceDraft::new(occurrence.id(), occurrence.section()))
            .collect(),
    };
    whole.sections[1].label = Some("Refrain".to_owned());
    library
        .edit_arrangement(id, whole.clone())
        .expect("whole-content rename");
    assert_eq!(
        library
            .arrangement(id)
            .expect("arrangement")
            .pagination_state(),
        PaginationState::Approved,
        "the whole-content edit path follows the same label rule"
    );

    whole.sections[0].lyrics = "changed".to_owned();
    library
        .edit_arrangement(id, whole)
        .expect("whole-content lyrics change");
    assert_eq!(
        library
            .arrangement(id)
            .expect("arrangement")
            .pagination_state(),
        PaginationState::NeedsReview,
        "a lyric change still triggers review"
    );
}

#[test]
fn stored_duplicate_names_still_load_and_edit() {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Verse", "a"),
                SectionDraft::labeled(section_id(101), "Verse", "b"),
                SectionDraft::new(section_id(102), "c"),
            ],
            Vec::new(),
        ),
        Vec::new(),
    );

    let mut lyrics = lyrics_of(&library, id);
    lyrics
        .set_section_lyrics(section_id(100), "changed words".to_owned())
        .expect("lyrics");
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("stored duplicate names keep editing");

    let stored = lyrics_of(&library, id);
    assert_eq!(stored.sections[0].label.as_deref(), Some("Verse"));
    assert_eq!(stored.sections[1].label.as_deref(), Some("Verse"));
    assert_eq!(stored.sections[2].label, None, "unlabeled stays valid");

    let mut renamed = stored.clone();
    renamed
        .rename_section(section_id(101), "Chorus")
        .expect("rename");
    library
        .edit_arrangement_lyrics(id, renamed)
        .expect("rename one of the duplicates");

    let mut added = lyrics_of(&library, id);
    assert_eq!(
        added.add_section(section_id(103), "verse"),
        Err(SongLibraryError::DuplicateField(Field::SectionLabel)),
        "a new name still has to be unique"
    );
}

#[test]
fn edit_arrangement_lyrics_keeps_metadata_and_refuses_foreign_or_archived_ids() {
    let mut library = library();
    let choir = choir_id(20);
    library
        .create_choir(choir, at(0), ChoirDraft::named("Youth Choir"))
        .expect("choir");
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        ArrangementDraft {
            title: Some("Cebuano (Lent)".to_owned()),
            languages: vec!["Cebuano".to_owned(), "English".to_owned()],
            rendition_attribution: Some("Sr. Maria".to_owned()),
            occasions: vec!["Lent".to_owned()],
            notes: Some("From the parish book".to_owned()),
            ..draft(
                "Cebuano",
                vec![SectionDraft::labeled(section_id(100), "Verse 1", "a")],
                vec![OccurrenceDraft::new(occurrence_id(200), section_id(100))],
            )
        },
        vec![choir],
    );
    let before = library.arrangement(id).expect("arrangement").clone();
    let details = before.details();
    let created_at = before.created_at();

    let mut lyrics = lyrics_of(&library, id);
    lyrics
        .set_section_lyrics(section_id(100), "new words".to_owned())
        .expect("lyrics");
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("lyrics edit");

    let after = library.arrangement(id).expect("arrangement");
    assert_eq!(after.details(), details, "metadata is untouched");
    assert_eq!(after.choirs(), before.choirs(), "Choirs are untouched");
    assert_eq!(after.song(), before.song());
    assert_eq!(after.created_at(), created_at);
    assert!(!after.is_archived());

    let second = arrangement_id(2);
    create(
        &mut library,
        second,
        draft(
            "English",
            vec![SectionDraft::labeled(section_id(150), "Foreign", "")],
            Vec::new(),
        ),
        Vec::new(),
    );
    let mut foreign = lyrics_of(&library, id);
    foreign
        .sections
        .push(SectionDraft::labeled(section_id(150), "Foreign copy", ""));
    assert_refused_lyrics(
        &mut library,
        id,
        foreign,
        SongLibraryError::DuplicateSection(section_id(150)),
    );

    let mut broken = lyrics_of(&library, id);
    broken
        .sequence
        .push(OccurrenceDraft::new(occurrence_id(201), section_id(199)));
    assert_refused_lyrics(
        &mut library,
        id,
        broken,
        SongLibraryError::SectionNotInArrangement {
            arrangement: id,
            section: section_id(199),
        },
    );

    library.archive_arrangement(id, at(20)).expect("archive");
    let archived = lyrics_of(&library, id);
    assert_refused_lyrics(
        &mut library,
        id,
        archived,
        SongLibraryError::Archived(RecordRef::Arrangement(id)),
    );
}

/// Two stored sections named "Chorus" (a legacy or imported Arrangement), an
/// unlabeled third, and one Sequence occurrence each for the first two.
fn duplicate_names_library() -> (SongLibrary, ArrangementId) {
    let mut library = library();
    let id = arrangement_id(1);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(100), "Chorus", "a"),
                SectionDraft::labeled(section_id(101), "Chorus", "b"),
                SectionDraft::new(section_id(102), "c"),
            ],
            vec![
                OccurrenceDraft::new(occurrence_id(200), section_id(100)),
                OccurrenceDraft::new(occurrence_id(201), section_id(101)),
            ],
        ),
        Vec::new(),
    );
    library.approve_pagination(id).expect("approved");
    (library, id)
}

#[test]
fn b3_undo_restores_a_section_whose_name_is_a_stored_duplicate() {
    let (mut library, id) = duplicate_names_library();
    let before = lyrics_of(&library, id);

    let mut removed = before.clone();
    assert_eq!(removed.remove_section(section_id(100)), Ok(1));
    let removed = removed.reviewed_against(&before);
    library
        .edit_arrangement_lyrics(id, removed)
        .expect("remove one of the duplicates");
    library
        .edit_arrangement_lyrics(id, before.clone())
        .expect("Undo of the removal is committable again");
    assert_eq!(lyrics_of(&library, id), before);

    let mut renamed = before.clone();
    renamed
        .rename_section(section_id(100), "Koro")
        .expect("rename");
    library
        .edit_arrangement_lyrics(id, renamed)
        .expect("rename one of the duplicates");
    library
        .edit_arrangement_lyrics(id, before.clone())
        .expect("Undo of the rename is committable again");
    assert_eq!(lyrics_of(&library, id), before);
}

#[test]
fn n1_renaming_to_the_sections_own_key_skips_the_uniqueness_check() {
    let (library, id) = duplicate_names_library();
    let mut lyrics = lyrics_of(&library, id);
    lyrics
        .rename_section(section_id(100), "Chorus")
        .expect("the same name is allowed beside a stored duplicate");
    lyrics
        .rename_section(section_id(100), "  CHORUS ")
        .expect("a respelling with the same key is allowed");
    assert_eq!(lyrics.sections[0].label.as_deref(), Some("CHORUS"));
    assert_eq!(
        lyrics.rename_section(section_id(102), "chorus"),
        Err(SongLibraryError::DuplicateField(Field::SectionLabel)),
        "another section still may not take the name"
    );
}

#[test]
fn n3_names_are_trimmed_before_they_are_checked_and_stored() {
    let (library, id) = duplicate_names_library();
    let mut lyrics = lyrics_of(&library, id);
    assert_eq!(lyrics.add_section(section_id(103), "  Bridge \t"), Ok(3));
    assert_eq!(lyrics.sections[3].label.as_deref(), Some("Bridge"));
    lyrics
        .rename_section(section_id(102), " Ending ")
        .expect("rename");
    assert_eq!(lyrics.sections[2].label.as_deref(), Some("Ending"));
    assert_eq!(
        lyrics.add_section(section_id(104), "  bridge"),
        Err(SongLibraryError::DuplicateField(Field::SectionLabel)),
        "the trimmed name is what is compared"
    );
    assert_eq!(
        lyrics.rename_section(section_id(102), "   "),
        Err(SongLibraryError::BlankField(Field::SectionLabel))
    );
}

#[test]
fn n2_a_huge_verse_number_neither_panics_nor_wraps() {
    let mut lyrics = ArrangementLyrics {
        sections: vec![SectionDraft::labeled(
            section_id(1),
            "Verse 18446744073709551615",
            "",
        )],
        sequence: Vec::new(),
        pagination: PaginationState::Draft,
    };
    let languages = vec!["English".to_owned()];
    assert_eq!(
        section_name_suggestions(&languages, &lyrics).0,
        "Verse 1",
        "a number that does not fit u32 is ignored"
    );
    lyrics.sections[0].label = Some("Verse 4294967295".to_owned());
    assert_eq!(
        section_name_suggestions(&languages, &lyrics).0,
        "Verse 4294967295",
        "the highest u32 saturates instead of wrapping"
    );
}

#[test]
fn b2_the_whole_library_check_sees_ids_other_arrangements_hold_or_held() {
    let mut library = library();
    let id = arrangement_id(1);
    let other = arrangement_id(2);
    create(
        &mut library,
        id,
        draft(
            "English",
            vec![SectionDraft::labeled(section_id(100), "Verse 1", "a")],
            vec![OccurrenceDraft::new(occurrence_id(200), section_id(100))],
        ),
        Vec::new(),
    );
    create(
        &mut library,
        other,
        draft(
            "English",
            vec![
                SectionDraft::labeled(section_id(150), "Held", "x"),
                SectionDraft::labeled(section_id(151), "Gone", "y"),
            ],
            vec![OccurrenceDraft::new(occurrence_id(250), section_id(150))],
        ),
        Vec::new(),
    );
    // Section 151 and occurrence 250 leave the other Arrangement for good but
    // stay reserved to it; section 152 is current there.
    let mut shrunk = lyrics_of(&library, other);
    shrunk.remove_section(section_id(151)).expect("section");
    shrunk.remove_section(section_id(150)).expect("section");
    shrunk
        .sections
        .push(SectionDraft::labeled(section_id(152), "Kept", ""));
    library
        .edit_arrangement_lyrics(other, shrunk)
        .expect("other edit");

    let before = lyrics_of(&library, id);
    let cases: Vec<(ArrangementLyrics, SongLibraryError)> = vec![
        {
            let mut next = before.clone();
            next.sections
                .push(SectionDraft::labeled(section_id(152), "Current", ""));
            (next, SongLibraryError::DuplicateSection(section_id(152)))
        },
        {
            let mut next = before.clone();
            next.sections
                .push(SectionDraft::labeled(section_id(151), "History", ""));
            (next, SongLibraryError::DuplicateSection(section_id(151)))
        },
        {
            let mut next = before.clone();
            next.sequence
                .push(OccurrenceDraft::new(occurrence_id(250), section_id(100)));
            (
                next,
                SongLibraryError::DuplicateOccurrence(occurrence_id(250)),
            )
        },
    ];
    for (next, expected) in cases {
        assert_eq!(
            library.check_arrangement_lyrics(id, &next),
            Err(expected.clone()),
            "the check refuses"
        );
        assert_refused_lyrics(&mut library, id, next, expected);
    }

    let mut fine = before.clone();
    fine.set_section_lyrics(section_id(100), "ok".to_owned())
        .expect("lyrics");
    assert_eq!(library.check_arrangement_lyrics(id, &fine), Ok(()));
    assert_eq!(lyrics_of(&library, id), before, "a check never mutates");
    assert_eq!(
        library.check_arrangement_lyrics(arrangement_id(9), &fine),
        Err(SongLibraryError::UnknownArrangement(arrangement_id(9)))
    );
}

#[test]
fn b1_lyrics_of_an_arrangement_with_no_language_can_be_edited() {
    // A language is required on creation, so a no-language Arrangement is one
    // loaded from storage (stored before T2.6).
    let id = arrangement_id(1);
    let created = at(10);
    let mut library = SongLibrary::hydrate(
        vec![HydrateSong {
            id: song_id(1),
            created_at: created,
            draft: SongDraft::titled("Kyrie"),
            archive_state: ArchiveState::Active,
        }],
        vec![HydrateArrangement {
            id,
            song: song_id(1),
            created_at: created,
            draft: ArrangementDraft {
                sections: vec![SectionDraft::labeled(section_id(100), "Verse 1", "a")],
                ..ArrangementDraft::default()
            },
            choirs: Vec::new(),
            pagination_state: PaginationState::Draft,
            archive_state: ArchiveState::Active,
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("hydrate");
    let mut lyrics = lyrics_of(&library, id);
    lyrics
        .set_section_lyrics(section_id(100), "new words".to_owned())
        .expect("lyrics");
    assert_eq!(library.check_arrangement_lyrics(id, &lyrics), Ok(()));
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("the domain accepts it, so storage must too");
    let arrangement = library.arrangement(id).expect("arrangement");
    assert_eq!(arrangement.sections()[0].lyrics(), "new words");
    assert!(arrangement.languages().is_empty());
}

#[test]
fn n8_split_and_join_keep_crlf_line_ends() {
    let three = "a\r\nb\r\nc";
    assert_eq!(
        split_after(three, 0, 1).as_deref(),
        Some("a\r\n\nb\r\nc"),
        "Split lands after the CRLF"
    );
    assert_eq!(slides("a\r\n\nb\r\nc").len(), 2);
    assert_eq!(
        join_with_previous("a\r\n\r\nb", 1).as_deref(),
        Some("a\r\nb"),
        "Join replaces exactly the blank run"
    );
}

#[test]
fn n8_reviewed_against_passes_other_states_and_catches_a_sequence_only_change() {
    let (library, id) = duplicate_names_library();
    let baseline = lyrics_of(&library, id);
    assert_eq!(baseline.pagination, PaginationState::Approved);

    for state in [PaginationState::Draft, PaginationState::NeedsReview] {
        let mut changed = baseline.clone();
        changed.pagination = state;
        changed
            .set_section_lyrics(section_id(100), "other".to_owned())
            .expect("lyrics");
        assert_eq!(
            changed.reviewed_against(&baseline).pagination,
            state,
            "only Approved moves"
        );
    }

    let mut sequence_only = baseline.clone();
    sequence_only
        .sequence
        .push(OccurrenceDraft::new(occurrence_id(210), section_id(100)));
    assert_eq!(
        sequence_only.reviewed_against(&baseline).pagination,
        PaginationState::NeedsReview,
        "a Sequence-only change needs review"
    );
}

#[test]
fn n8_an_archived_song_refuses_a_lyrics_edit() {
    let (mut library, id) = duplicate_names_library();
    let lyrics = lyrics_of(&library, id);
    library.archive_song(song_id(1), at(30)).expect("archive");
    let expected = SongLibraryError::Archived(RecordRef::Song(song_id(1)));
    assert_eq!(
        library.check_arrangement_lyrics(id, &lyrics),
        Err(expected.clone())
    );
    assert_refused_lyrics(&mut library, id, lyrics, expected);
}

#[test]
fn n8_undo_of_a_reorder_restores_the_order() {
    let (mut library, id) = duplicate_names_library();
    let before = lyrics_of(&library, id);
    let mut moved = before.clone();
    assert_eq!(moved.move_section(section_id(102), Step::Up), Ok(true));
    let moved = moved.reviewed_against(&before);
    assert_eq!(moved.pagination, PaginationState::Approved);
    library.edit_arrangement_lyrics(id, moved).expect("reorder");
    assert_eq!(
        lyrics_of(&library, id)
            .sections
            .iter()
            .map(|section| section.id)
            .collect::<Vec<_>>(),
        vec![section_id(100), section_id(102), section_id(101)]
    );
    library
        .edit_arrangement_lyrics(id, before.clone())
        .expect("Undo");
    assert_eq!(lyrics_of(&library, id), before);
}

#[test]
fn n9_deleting_the_blank_line_by_hand_joins_the_slides() {
    let before = "Niño\nGinoó\n\nAleluya";
    assert_eq!(slides(before).len(), 2);
    // The operator puts the caret on the blank line and deletes it.
    let blank = before.find("\n\n").expect("blank line") + 1;
    let mut edited = before.to_owned();
    edited.remove(blank);
    assert_eq!(edited, "Niño\nGinoó\nAleluya");
    let generated = slides(&edited);
    assert_eq!(generated.len(), 1, "no blank line, one slide");
    assert_eq!(slide_body(&edited, &generated[0]), "Niño\nGinoó\nAleluya");
    assert_eq!(generated[0].lines.len(), 3);
}
