//! T2.8 acceptance tests for Sequence operations, flattening, per-section
//! pagination approval, and readiness.
//!
//! Every test goes through the public Interface only. Test names start with the
//! acceptance criterion they cover.

use std::collections::BTreeSet;

use domain::identity::{ArrangementId, EntityId, OccurrenceId, SectionId, SongId, Timestamp};
use domain::song_library::{
    ArrangementDraft, ArrangementLyrics, Finding, FindingKind, OccurrenceDraft, PaginationState,
    ReadinessState, SectionDraft, SongDraft, SongLibrary, SongLibraryError, Step, flatten,
    position_label, readiness, selection_after_replace,
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

fn section_id(n: u8) -> SectionId {
    SectionId::from_entity(entity(n))
}

fn occurrence_id(n: u8) -> OccurrenceId {
    OccurrenceId::from_entity(entity(n))
}

fn at(offset: i64) -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000 + offset)
}

/// A library with one Arrangement (1) of one Song (1), built from `sections`
/// and `sequence`.
fn library_with(
    sections: Vec<SectionDraft>,
    sequence: Vec<OccurrenceDraft>,
) -> (SongLibrary, ArrangementId) {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie"))
        .expect("song");
    let draft = ArrangementDraft {
        languages: vec!["English".to_owned()],
        sections,
        sequence,
        ..ArrangementDraft::default()
    };
    library
        .create_arrangement(arrangement_id(1), song_id(1), at(10), draft, Vec::new())
        .expect("arrangement");
    (library, arrangement_id(1))
}

/// The Arrangement's current content as the editor would hold it.
fn lyrics_of(library: &SongLibrary, id: ArrangementId) -> ArrangementLyrics {
    library.arrangement(id).expect("arrangement").lyrics()
}

/// One section with words.
fn section(n: u8, label: &str, lyrics: &str) -> SectionDraft {
    SectionDraft::labeled(section_id(n), label, lyrics)
}

/// One occurrence of section `n`.
fn use_of(n: u8, section: u8) -> OccurrenceDraft {
    OccurrenceDraft::new(occurrence_id(n), section_id(section))
}

/// Approves one section through the Lyrics editor, as the operator does.
fn approve(library: &mut SongLibrary, id: ArrangementId, section: u8) {
    let mut lyrics = lyrics_of(library, id);
    lyrics
        .approve_section(section_id(section))
        .expect("approved");
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("save the approval");
}

/// Each section's pagination state, in authored order.
fn section_states(lyrics: &ArrangementLyrics) -> Vec<PaginationState> {
    lyrics
        .sections
        .iter()
        .map(|section| section.pagination)
        .collect()
}

#[test]
fn ac2_adding_a_section_never_adds_an_occurrence() {
    let (mut library, id) = library_with(
        vec![section(100, "Verse 1", "first words")],
        vec![use_of(200, 100)],
    );
    let before = lyrics_of(&library, id);

    let mut lyrics = before.clone();
    assert_eq!(lyrics.add_section(section_id(101), "Chorus"), Ok(1));
    assert_eq!(
        lyrics.sequence, before.sequence,
        "adding a section to Lyrics never places it"
    );
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("save the new section");

    let stored = lyrics_of(&library, id);
    assert_eq!(stored.sections.len(), 2);
    assert_eq!(stored.sequence, before.sequence);
    assert_eq!(stored.occurrences_of(section_id(101)), 0);
}

#[test]
fn ac2_insert_goes_after_the_selected_occurrence() {
    let (library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101), use_of(202, 100)],
    );
    let mut lyrics = lyrics_of(&library, id);

    let index = lyrics
        .insert_occurrence(
            occurrence_id(203),
            section_id(100),
            Some(occurrence_id(201)),
        )
        .expect("insert after the selected occurrence");

    assert_eq!(index, 2);
    assert_eq!(
        lyrics.sequence.iter().map(|o| o.id).collect::<Vec<_>>(),
        vec![
            occurrence_id(200),
            occurrence_id(201),
            occurrence_id(203),
            occurrence_id(202)
        ]
    );
    assert_eq!(lyrics.sequence[2].section, section_id(100));
    assert_eq!(
        lyrics.sections.len(),
        2,
        "inserting never copies or adds a section"
    );
}

#[test]
fn ac2_insert_appends_when_nothing_is_selected_and_is_first_in_an_empty_sequence() {
    let (library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    let mut lyrics = lyrics_of(&library, id);

    assert_eq!(
        lyrics.insert_occurrence(occurrence_id(202), section_id(101), None),
        Ok(2),
        "with nothing selected the occurrence is appended"
    );
    assert_eq!(
        lyrics.sequence.last().map(|occurrence| occurrence.id),
        Some(occurrence_id(202))
    );

    let (empty_library, empty_id) =
        library_with(vec![section(100, "Verse 1", "first words")], Vec::new());
    let mut empty = lyrics_of(&empty_library, empty_id);
    assert_eq!(
        empty.insert_occurrence(occurrence_id(200), section_id(100), None),
        Ok(0),
        "the first occurrence of an empty Sequence is at position 0"
    );
    assert_eq!(empty.sequence.len(), 1);

    let refused = empty.clone();
    assert_eq!(
        empty.insert_occurrence(occurrence_id(201), section_id(99), None),
        Err(SongLibraryError::UnknownSection(section_id(99)))
    );
    assert_eq!(
        empty.insert_occurrence(occurrence_id(201), section_id(100), Some(occurrence_id(99))),
        Err(SongLibraryError::UnknownOccurrence(occurrence_id(99)))
    );
    assert_eq!(
        empty.insert_occurrence(occurrence_id(200), section_id(100), None),
        Err(SongLibraryError::DuplicateOccurrence(occurrence_id(200)))
    );
    assert_eq!(empty, refused, "a refused insert changes nothing");
}

#[test]
fn ac3_repeat_references_the_same_section_right_after_and_copies_no_lyrics() {
    let (library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    let mut lyrics = lyrics_of(&library, id);

    let index = lyrics
        .repeat_occurrence(occurrence_id(202), occurrence_id(200))
        .expect("repeat the selected occurrence");

    assert_eq!(index, 1);
    assert_eq!(
        lyrics.sequence.iter().map(|o| o.id).collect::<Vec<_>>(),
        vec![occurrence_id(200), occurrence_id(202), occurrence_id(201)]
    );
    assert_eq!(
        lyrics.sequence[1].section,
        section_id(100),
        "the repeat references the source's section"
    );
    assert_eq!(lyrics.sections.len(), 2, "no section was copied");
    assert_eq!(lyrics.occurrences_of(section_id(100)), 2);

    // Editing the one section changes what both occurrences sing.
    lyrics
        .set_section_lyrics(section_id(100), "edited words".to_owned())
        .expect("edit the shared section");
    let shared: Vec<_> = flatten(&lyrics)
        .into_iter()
        .filter(|slide| slide.section == section_id(100))
        .map(|slide| slide.occurrence)
        .collect();
    assert_eq!(
        shared,
        vec![occurrence_id(200), occurrence_id(202)],
        "both occurrences carry the section's slides"
    );

    let refused = lyrics.clone();
    assert_eq!(
        lyrics.repeat_occurrence(occurrence_id(203), occurrence_id(99)),
        Err(SongLibraryError::UnknownOccurrence(occurrence_id(99)))
    );
    assert_eq!(
        lyrics.repeat_occurrence(occurrence_id(200), occurrence_id(201)),
        Err(SongLibraryError::DuplicateOccurrence(occurrence_id(200)))
    );
    assert_eq!(lyrics, refused);
}

#[test]
fn ac4_move_by_step_and_to_an_index_moves_every_child_slide_together() {
    let (library, id) = library_with(
        vec![
            section(100, "Verse 1", "one\n\ntwo"),
            section(101, "Chorus", "chorus"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    let mut lyrics = lyrics_of(&library, id);
    let before = flatten(&lyrics);
    assert_eq!(
        before
            .iter()
            .map(|slide| (slide.occurrence, slide.slide))
            .collect::<Vec<_>>(),
        vec![
            (occurrence_id(200), 0),
            (occurrence_id(200), 1),
            (occurrence_id(201), 0)
        ]
    );

    assert_eq!(
        lyrics.move_occurrence(occurrence_id(200), Step::Down),
        Ok(true)
    );
    let after = flatten(&lyrics);
    assert_eq!(
        after
            .iter()
            .map(|slide| (slide.occurrence, slide.slide, slide.position))
            .collect::<Vec<_>>(),
        vec![
            (occurrence_id(201), 0, 0),
            (occurrence_id(200), 0, 1),
            (occurrence_id(200), 1, 1)
        ],
        "both of the moved occurrence's slides moved with it"
    );

    assert_eq!(
        lyrics.move_occurrence_to(occurrence_id(200), 0),
        Ok(true),
        "a drag to the top restores the order"
    );
    assert_eq!(flatten(&lyrics), before);
    assert_eq!(
        lyrics.move_occurrence_to(occurrence_id(200), 99),
        Ok(true),
        "an index past the end clamps to the last position"
    );
    assert_eq!(
        lyrics.sequence.iter().map(|o| o.id).collect::<Vec<_>>(),
        vec![occurrence_id(201), occurrence_id(200)]
    );
}

#[test]
fn ac4_moves_past_an_edge_change_nothing() {
    let (library, id) = library_with(
        vec![section(100, "Verse 1", "first words")],
        vec![use_of(200, 100), use_of(201, 100)],
    );
    let mut lyrics = lyrics_of(&library, id);
    let before = lyrics.clone();

    assert_eq!(
        lyrics.move_occurrence(occurrence_id(200), Step::Up),
        Ok(false),
        "the first occurrence cannot move up"
    );
    assert_eq!(
        lyrics.move_occurrence(occurrence_id(201), Step::Down),
        Ok(false),
        "the last occurrence cannot move down"
    );
    assert_eq!(
        lyrics.move_occurrence_to(occurrence_id(200), 0),
        Ok(false),
        "moving an occurrence onto its own position changes nothing"
    );
    assert_eq!(lyrics, before);
    assert_eq!(
        lyrics.move_occurrence(occurrence_id(99), Step::Up),
        Err(SongLibraryError::UnknownOccurrence(occurrence_id(99)))
    );
    assert_eq!(
        lyrics.move_occurrence_to(occurrence_id(99), 0),
        Err(SongLibraryError::UnknownOccurrence(occurrence_id(99)))
    );
}

#[test]
fn ac5_remove_keeps_the_section_and_picks_next_then_previous() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101), use_of(202, 100)],
    );
    let mut lyrics = lyrics_of(&library, id);

    assert_eq!(
        lyrics.remove_occurrence(occurrence_id(201)),
        Ok(Some(occurrence_id(202))),
        "the occurrence now at the removed position is next"
    );
    assert_eq!(
        lyrics.sequence.iter().map(|o| o.id).collect::<Vec<_>>(),
        vec![occurrence_id(200), occurrence_id(202)]
    );
    assert_eq!(
        lyrics.sections.len(),
        2,
        "removing an occurrence never removes its section"
    );

    assert_eq!(
        lyrics.remove_occurrence(occurrence_id(202)),
        Ok(Some(occurrence_id(200))),
        "with nothing after it the previous occurrence is next"
    );
    assert_eq!(
        lyrics.remove_occurrence(occurrence_id(200)),
        Ok(None),
        "the last occurrence leaves nothing selected"
    );
    assert!(lyrics.sequence.is_empty());
    assert_eq!(lyrics.sections.len(), 2, "every section stays in Lyrics");

    assert_eq!(
        lyrics.remove_occurrence(occurrence_id(200)),
        Err(SongLibraryError::UnknownOccurrence(occurrence_id(200)))
    );

    // The removal saves like any other Sequence edit and keeps the sections.
    library
        .edit_arrangement_lyrics(id, lyrics)
        .expect("save the empty Sequence");
    let stored = lyrics_of(&library, id);
    assert!(stored.sequence.is_empty());
    assert_eq!(stored.sections.len(), 2);
}

#[test]
fn ac5_restoring_the_previous_content_returns_the_occurrence_to_its_position() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101), use_of(202, 100)],
    );
    let before = lyrics_of(&library, id);

    let mut removed = before.clone();
    assert_eq!(
        removed.remove_occurrence(occurrence_id(201)),
        Ok(Some(occurrence_id(202)))
    );
    library
        .edit_arrangement_lyrics(id, removed.clone())
        .expect("remove");
    assert_eq!(
        lyrics_of(&library, id).sequence,
        removed.sequence,
        "the removal is stored"
    );

    library
        .edit_arrangement_lyrics(id, before.clone())
        .expect("Undo");
    let restored = lyrics_of(&library, id);
    assert_eq!(
        restored.sequence, before.sequence,
        "Undo puts the occurrence back at its old position"
    );
    assert_eq!(restored, before);
    assert_eq!(
        selection_after_replace(&removed, &restored, Some(occurrence_id(201))),
        Some(occurrence_id(201)),
        "Undo selects the occurrence it restored"
    );
}

#[test]
fn ac8_flatten_lists_each_occurrence_slides_in_order_and_repeats_share_the_section() {
    let (library, id) = library_with(
        vec![
            section(100, "Verse 1", "one\n\ntwo"),
            section(101, "Chorus", "chorus"),
            section(102, "Bridge", ""),
        ],
        vec![
            use_of(200, 100),
            use_of(203, 102),
            use_of(201, 101),
            use_of(202, 100),
        ],
    );
    let lyrics = lyrics_of(&library, id);
    let flat = flatten(&lyrics);

    assert_eq!(
        flat.iter()
            .map(|slide| (
                slide.occurrence,
                slide.position,
                slide.section,
                slide.slide,
                slide.slides_in_section
            ))
            .collect::<Vec<_>>(),
        vec![
            (occurrence_id(200), 0, section_id(100), 0, 2),
            (occurrence_id(200), 0, section_id(100), 1, 2),
            // The Bridge has no lyrics, so it contributes no slide, but the
            // Chorus after it still reports position 2.
            (occurrence_id(201), 2, section_id(101), 0, 1),
            (occurrence_id(202), 3, section_id(100), 0, 2),
            (occurrence_id(202), 3, section_id(100), 1, 2),
        ],
        "each occurrence contributes its section's slides in order"
    );
    assert_eq!(
        flat.iter()
            .filter(|slide| slide.section == section_id(100))
            .count(),
        4,
        "the repeated section's slides are listed once per occurrence"
    );
}

#[test]
fn position_labels_are_two_digit_and_recompute_after_edits() {
    assert_eq!(position_label(0), "01");
    assert_eq!(position_label(8), "09");
    assert_eq!(position_label(9), "10");
    assert_eq!(position_label(99), "100");
    assert_eq!(position_label(999), "1000");

    let (library, id) = library_with(
        vec![section(100, "Verse 1", "words")],
        vec![use_of(200, 100), use_of(201, 100)],
    );
    let mut lyrics = lyrics_of(&library, id);
    assert_eq!(
        lyrics.insert_occurrence(
            occurrence_id(202),
            section_id(100),
            Some(occurrence_id(200))
        ),
        Ok(1)
    );
    let labels: Vec<String> = lyrics
        .sequence
        .iter()
        .enumerate()
        .map(|(index, _)| position_label(index))
        .collect();
    assert_eq!(labels, vec!["01", "02", "03"]);
    assert_eq!(
        lyrics.occurrence_index(occurrence_id(201)),
        Some(2),
        "the label follows the new position"
    );
}

#[test]
fn selection_after_undo_selects_the_restored_occurrence() {
    let (library, id) = library_with(
        vec![section(100, "Verse 1", "words")],
        vec![use_of(200, 100), use_of(201, 100), use_of(202, 100)],
    );
    let before = lyrics_of(&library, id);
    let mut after = before.clone();
    after.remove_occurrence(occurrence_id(201)).expect("remove");

    assert_eq!(
        selection_after_replace(&before, &after, Some(occurrence_id(201))),
        Some(occurrence_id(202)),
        "without the selected occurrence the one now at its position is chosen"
    );
    assert_eq!(
        selection_after_replace(&before, &before, Some(occurrence_id(201))),
        Some(occurrence_id(201)),
        "a selection that still exists is kept"
    );
    assert_eq!(
        selection_after_replace(&before, &after, None),
        None,
        "nothing selected stays nothing selected"
    );

    let mut tail_removed = before.clone();
    tail_removed
        .remove_occurrence(occurrence_id(202))
        .expect("remove");
    assert_eq!(
        selection_after_replace(&before, &tail_removed, Some(occurrence_id(202))),
        Some(occurrence_id(201)),
        "after the last occurrence the previous one is chosen"
    );

    let mut all_removed = before.clone();
    for occurrence in [200, 201, 202] {
        all_removed
            .remove_occurrence(occurrence_id(occurrence))
            .expect("remove");
    }
    assert_eq!(
        selection_after_replace(&before, &all_removed, Some(occurrence_id(201))),
        None
    );
}

#[test]
fn ac6_an_empty_sequence_with_sections_is_incomplete() {
    let (library, id) = library_with(vec![section(100, "Verse 1", "words")], Vec::new());
    let judged = readiness(&lyrics_of(&library, id), &BTreeSet::new());

    assert_eq!(judged.state, ReadinessState::Incomplete);
    assert_eq!(
        judged.findings,
        vec![Finding {
            kind: FindingKind::EmptySequence,
            section: None,
            occurrences: 0,
        }],
        "an empty Sequence reports itself rather than every waiting section"
    );
}

#[test]
fn ac6_unused_sections_raise_no_finding() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Bridge", ""),
        ],
        vec![use_of(200, 100)],
    );
    approve(&mut library, id, 100);
    let judged = readiness(&lyrics_of(&library, id), &BTreeSet::new());

    assert_eq!(judged.state, ReadinessState::Ready);
    assert!(
        judged.findings.is_empty(),
        "a blank section the Sequence never places raises no finding"
    );
}

#[test]
fn ac7_only_referenced_sections_are_examined() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
            section(102, "Bridge", ""),
        ],
        vec![use_of(200, 100)],
    );
    approve(&mut library, id, 100);
    let unrenderable = BTreeSet::from([section_id(101), section_id(102)]);
    let judged = readiness(&lyrics_of(&library, id), &unrenderable);

    assert_eq!(judged.state, ReadinessState::Ready);
    assert!(
        judged.findings.is_empty(),
        "unreferenced sections are never examined, even when unrenderable or blank"
    );
}

#[test]
fn ac7_a_repeated_blank_section_is_one_finding_with_its_occurrence_count() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", ""),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101), use_of(202, 100)],
    );
    approve(&mut library, id, 101);
    let judged = readiness(&lyrics_of(&library, id), &BTreeSet::new());

    assert_eq!(judged.state, ReadinessState::Incomplete);
    assert_eq!(
        judged.findings,
        vec![Finding {
            kind: FindingKind::NoLyrics,
            section: Some(section_id(100)),
            occurrences: 2,
        }],
        "a repeated section is reported once with every occurrence counted"
    );
}

#[test]
fn ac7_an_unapproved_referenced_section_is_needs_review_not_incomplete() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    approve(&mut library, id, 100);
    let judged = readiness(&lyrics_of(&library, id), &BTreeSet::new());

    assert_eq!(judged.state, ReadinessState::NeedsReview);
    assert_eq!(
        judged.findings,
        vec![Finding {
            kind: FindingKind::NotApproved(PaginationState::Draft),
            section: Some(section_id(101)),
            occurrences: 1,
        }],
        "a section that has slides but is not approved is a review item"
    );
}

#[test]
fn ac7_unrenderable_input_makes_a_referenced_section_incomplete() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    approve(&mut library, id, 100);
    approve(&mut library, id, 101);
    let lyrics = lyrics_of(&library, id);
    assert_eq!(
        readiness(&lyrics, &BTreeSet::new()).state,
        ReadinessState::Ready
    );

    let judged = readiness(&lyrics, &BTreeSet::from([section_id(101)]));
    assert_eq!(judged.state, ReadinessState::Incomplete);
    assert_eq!(
        judged.findings,
        vec![Finding {
            kind: FindingKind::Unrenderable,
            section: Some(section_id(101)),
            occurrences: 1,
        }],
        "approved slides that cannot be composed still make the Arrangement incomplete"
    );
}

#[test]
fn no_sections_is_its_own_state() {
    let (library, id) = library_with(Vec::new(), Vec::new());
    let judged = readiness(&lyrics_of(&library, id), &BTreeSet::new());

    assert_eq!(judged.state, ReadinessState::NoSections);
    assert!(
        judged.findings.is_empty(),
        "without sections there is no reason to report beyond the state"
    );
}

#[test]
fn editing_one_section_marks_only_that_section_for_review() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    approve(&mut library, id, 100);
    approve(&mut library, id, 101);
    let committed = lyrics_of(&library, id);
    assert_eq!(
        section_states(&committed),
        vec![PaginationState::Approved, PaginationState::Approved]
    );

    let mut edited = committed.clone();
    edited
        .set_section_lyrics(section_id(101), "new chorus words".to_owned())
        .expect("edit the chorus");
    let reviewed = edited.reviewed_against(&committed);
    assert_eq!(
        section_states(&reviewed),
        vec![PaginationState::Approved, PaginationState::NeedsReview],
        "only the section whose lyrics changed needs review"
    );
    library
        .edit_arrangement_lyrics(id, reviewed.clone())
        .expect("save the edit");
    assert_eq!(lyrics_of(&library, id), reviewed);

    let judged = readiness(&lyrics_of(&library, id), &BTreeSet::new());
    assert_eq!(judged.state, ReadinessState::NeedsReview);
    assert_eq!(
        judged.findings,
        vec![Finding {
            kind: FindingKind::NotApproved(PaginationState::NeedsReview),
            section: Some(section_id(101)),
            occurrences: 1,
        }]
    );
}

#[test]
fn sequence_edits_and_renames_keep_every_approval() {
    let (mut library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Chorus", "chorus words"),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    approve(&mut library, id, 100);
    approve(&mut library, id, 101);
    let approved = vec![PaginationState::Approved, PaginationState::Approved];

    let mut lyrics = lyrics_of(&library, id);
    lyrics
        .rename_section(section_id(101), "Refrain")
        .expect("rename");
    lyrics
        .insert_occurrence(occurrence_id(202), section_id(100), None)
        .expect("insert");
    lyrics
        .repeat_occurrence(occurrence_id(203), occurrence_id(201))
        .expect("repeat");
    lyrics
        .move_occurrence(occurrence_id(200), Step::Down)
        .expect("move");
    lyrics
        .remove_occurrence(occurrence_id(201))
        .expect("remove");
    let reviewed = lyrics.reviewed_against(&lyrics_of(&library, id));

    assert_eq!(
        section_states(&reviewed),
        approved,
        "rename and every Sequence edit leave each section's approval alone"
    );
    library.edit_arrangement_lyrics(id, reviewed).expect("save");
    let stored = lyrics_of(&library, id);
    assert_eq!(section_states(&stored), approved);
    assert_eq!(
        readiness(&stored, &BTreeSet::new()).state,
        ReadinessState::Ready
    );
}

#[test]
fn approve_refuses_a_section_without_slides() {
    let (library, id) = library_with(
        vec![
            section(100, "Verse 1", "first words"),
            section(101, "Bridge", "  \n\n "),
        ],
        vec![use_of(200, 100), use_of(201, 101)],
    );
    let mut lyrics = lyrics_of(&library, id);
    let refused = lyrics.clone();

    assert_eq!(
        lyrics.approve_section(section_id(101)),
        Err(SongLibraryError::SectionWithoutSlides(section_id(101)))
    );
    assert_eq!(lyrics, refused, "a refused approval changes no section");
    assert_eq!(
        lyrics.approve_section(section_id(99)),
        Err(SongLibraryError::UnknownSection(section_id(99)))
    );
    assert_eq!(
        lyrics.approve_section(section_id(100)),
        Ok(true),
        "a section with slides is approved"
    );
    assert_eq!(
        lyrics.approve_section(section_id(100)),
        Ok(false),
        "an already approved section is left as it is"
    );
}

#[test]
fn undo_restores_section_approval() {
    let (mut library, id) = library_with(
        vec![section(100, "Verse 1", "first words")],
        vec![use_of(200, 100)],
    );
    let draft = lyrics_of(&library, id);
    assert_eq!(
        section_states(&draft),
        vec![PaginationState::Draft],
        "a new section starts Draft"
    );

    let mut approved = draft.clone();
    assert_eq!(approved.approve_section(section_id(100)), Ok(true));
    library
        .edit_arrangement_lyrics(id, approved.clone())
        .expect("approve");
    assert_eq!(lyrics_of(&library, id), approved);

    let mut edited = approved.clone();
    edited
        .set_section_lyrics(section_id(100), "second words".to_owned())
        .expect("edit the lyrics");
    let reviewed = edited.reviewed_against(&approved);
    assert_eq!(
        section_states(&reviewed),
        vec![PaginationState::NeedsReview]
    );
    library
        .edit_arrangement_lyrics(id, reviewed.clone())
        .expect("edit");

    library
        .edit_arrangement_lyrics(id, approved.clone())
        .expect("undo");
    assert_eq!(
        lyrics_of(&library, id),
        approved,
        "Undo restores the snapshot with its approval intact"
    );
    assert_eq!(
        readiness(&lyrics_of(&library, id), &BTreeSet::new()).state,
        ReadinessState::Ready
    );

    library
        .edit_arrangement_lyrics(id, reviewed.clone())
        .expect("redo");
    assert_eq!(
        lyrics_of(&library, id),
        reviewed,
        "Redo puts the review state back"
    );
}
