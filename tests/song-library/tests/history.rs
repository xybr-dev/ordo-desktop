//! T2.9a History classification and comparison acceptance tests.
//!
//! The tests call the pure domain rules directly: `arrangement_change`,
//! `session_topic`, `song_changed_fields`, `compare_arrangements` and
//! `carry_review_states`.

use domain::identity::{ChoirId, EntityId, OccurrenceId, SectionId};
use domain::song_library::{
    ArrangementChange, ArrangementDetails, ArrangementLyrics, ArrangementState, LineMark,
    OccurrenceDraft, PaginationState, SectionDraft, SessionTopic, SongDraft, SongField,
    arrangement_change, carry_review_states, compare_arrangements, session_topic,
    song_changed_fields,
};

fn entity(n: u8) -> EntityId {
    EntityId::from_random_bytes([n; 16])
}

fn section_id(n: u8) -> SectionId {
    SectionId::from_entity(entity(n))
}

fn occurrence_id(n: u8) -> OccurrenceId {
    OccurrenceId::from_entity(entity(n))
}

fn choir_id(n: u8) -> ChoirId {
    ChoirId::from_entity(entity(n))
}

fn details() -> ArrangementDetails {
    ArrangementDetails {
        title: Some("Cebuano".to_owned()),
        languages: vec!["Cebuano".to_owned()],
        choirs: vec![choir_id(20)],
        rendition_attribution: Some("Youth".to_owned()),
        occasions: vec!["Ordinary Time".to_owned()],
        notes: Some("Hymnal".to_owned()),
    }
}

fn section(id: u8, text: &str, pagination: PaginationState) -> SectionDraft {
    SectionDraft {
        id: section_id(id),
        label: Some(format!("Section {id}")),
        lyrics: text.to_owned(),
        pagination,
    }
}

/// Sections from `(id, text, state)` and occurrences from `(id, section)`.
fn lyrics(sections: Vec<SectionDraft>, sequence: Vec<(u8, u8)>) -> ArrangementLyrics {
    ArrangementLyrics {
        sections,
        sequence: sequence
            .into_iter()
            .map(|(occurrence, section)| {
                OccurrenceDraft::new(occurrence_id(occurrence), section_id(section))
            })
            .collect(),
    }
}

fn state<'a>(
    details: &'a ArrangementDetails,
    lyrics: &'a ArrangementLyrics,
    states_recorded: bool,
) -> ArrangementState<'a> {
    ArrangementState {
        details,
        lyrics,
        states_recorded,
    }
}

#[test]
fn ac1_an_approval_only_revision_is_pagination_approved() {
    let details = details();
    let before_lyrics = lyrics(
        vec![
            section(1, "Una\n\nIkaduha", PaginationState::Draft),
            section(2, "Koro", PaginationState::Approved),
        ],
        vec![(1, 1), (2, 2)],
    );
    let after_lyrics = lyrics(
        vec![
            section(1, "Una\n\nIkaduha", PaginationState::Approved),
            section(2, "Koro", PaginationState::Approved),
        ],
        vec![(1, 1), (2, 2)],
    );

    assert_eq!(
        arrangement_change(
            &state(&details, &before_lyrics, true),
            &state(&details, &after_lyrics, true),
        ),
        ArrangementChange::PaginationApproved {
            section: section_id(1)
        }
    );
}

#[test]
fn ac1_undoing_an_approval_is_pagination_returned() {
    let details = details();
    let approved = lyrics(
        vec![section(1, "Una\n\nIkaduha", PaginationState::Approved)],
        vec![(1, 1)],
    );
    let returned = lyrics(
        vec![section(1, "Una\n\nIkaduha", PaginationState::Draft)],
        vec![(1, 1)],
    );

    assert_eq!(
        arrangement_change(
            &state(&details, &approved, true),
            &state(&details, &returned, true),
        ),
        ArrangementChange::PaginationReturned {
            section: section_id(1)
        }
    );
}

#[test]
fn ac1_a_blank_line_only_change_is_a_slide_boundary() {
    let details = details();
    let before_lyrics = lyrics(
        vec![section(1, "Una\nIkaduha", PaginationState::Draft)],
        vec![(1, 1)],
    );
    let after_lyrics = lyrics(
        vec![section(1, "Una\n\nIkaduha", PaginationState::Draft)],
        vec![(1, 1)],
    );

    assert_eq!(
        arrangement_change(
            &state(&details, &before_lyrics, true),
            &state(&details, &after_lyrics, true),
        ),
        ArrangementChange::SlideBoundary {
            section: section_id(1)
        }
    );
}

#[test]
fn ac1_one_sections_words_is_wording() {
    let details = details();
    let before_lyrics = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    let after_lyrics = lyrics(
        vec![
            section(1, "Una kaayo", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );

    assert_eq!(
        arrangement_change(
            &state(&details, &before_lyrics, true),
            &state(&details, &after_lyrics, true),
        ),
        ArrangementChange::Wording {
            section: section_id(1)
        }
    );
}

#[test]
fn ac1_an_appended_repeat_is_repeated_at_the_end() {
    let details = details();
    let before_lyrics = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    let after_lyrics = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2), (3, 2)],
    );

    assert_eq!(
        arrangement_change(
            &state(&details, &before_lyrics, true),
            &state(&details, &after_lyrics, true),
        ),
        ArrangementChange::Repeated {
            section: section_id(2),
            at_end: true,
        }
    );
}

#[test]
fn ac1_a_session_of_details_only_is_a_details_session() {
    assert_eq!(
        session_topic(&[ArrangementChange::Details, ArrangementChange::Details]),
        SessionTopic::Details
    );
    assert_eq!(
        session_topic(&[
            ArrangementChange::Details,
            ArrangementChange::Wording {
                section: section_id(1)
            },
        ]),
        SessionTopic::Lyrics
    );
    assert_eq!(session_topic(&[]), SessionTopic::Lyrics);

    // A real details-only revision is a Details change.
    let before_details = details();
    let mut after_details = before_details.clone();
    after_details.title = Some("English".to_owned());
    let content = lyrics(
        vec![section(1, "Una", PaginationState::Draft)],
        vec![(1, 1)],
    );
    let change = arrangement_change(
        &state(&before_details, &content, true),
        &state(&after_details, &content, true),
    );
    assert_eq!(change, ArrangementChange::Details);
    assert_eq!(session_topic(&[change]), SessionTopic::Details);
}

#[test]
fn ac1_a_legacy_parent_never_reports_a_state_change() {
    let details = details();
    let legacy = lyrics(
        vec![section(1, "Una", PaginationState::Draft)],
        vec![(1, 1)],
    );
    let recorded = lyrics(
        vec![section(1, "Una", PaginationState::Approved)],
        vec![(1, 1)],
    );

    // A format-2 parent beside a format-3 child: equal words, unknown parent
    // state, so no claim either way.
    assert_eq!(
        arrangement_change(
            &state(&details, &legacy, false),
            &state(&details, &recorded, true),
        ),
        ArrangementChange::Unchanged
    );
    // The reverse boundary is equally silent.
    assert_eq!(
        arrangement_change(
            &state(&details, &recorded, true),
            &state(&details, &legacy, false),
        ),
        ArrangementChange::Unchanged
    );
}

#[test]
fn ac2_compare_marks_changed_lines_by_position_within_a_section() {
    let details = details();
    let this_lyrics = lyrics(
        vec![section(1, "one\ntwo\n\nthree", PaginationState::Approved)],
        vec![(1, 1)],
    );
    let other_lyrics = lyrics(
        vec![section(1, "one\nTWO\n\nthree", PaginationState::Draft)],
        vec![(1, 1)],
    );

    let comparison = compare_arrangements(
        &state(&details, &this_lyrics, true),
        &state(&details, &other_lyrics, true),
    );
    let marks = &comparison.sections[0];
    assert_eq!(marks.section, section_id(1));
    assert_eq!(
        marks.lines,
        vec![
            LineMark::Same,
            LineMark::Changed,
            LineMark::Same,
            LineMark::Same
        ]
    );
    assert_eq!(marks.state, Some(PaginationState::Approved));

    // A slide break the other side does not have at that position is marked.
    let no_break = lyrics(
        vec![section(1, "one\ntwo\nthree", PaginationState::Draft)],
        vec![(1, 1)],
    );
    let comparison = compare_arrangements(
        &state(&details, &this_lyrics, true),
        &state(&details, &no_break, true),
    );
    assert_eq!(
        comparison.sections[0].lines,
        vec![
            LineMark::Same,
            LineMark::Same,
            LineMark::SlideBreak,
            LineMark::Same
        ]
    );
    assert!(marks.state_differs);

    // A section the other side no longer has is wholly Changed.
    let gone = lyrics(Vec::new(), Vec::new());
    let comparison = compare_arrangements(
        &state(&details, &this_lyrics, true),
        &state(&details, &gone, true),
    );
    assert_eq!(
        comparison.sections[0].lines,
        vec![
            LineMark::Changed,
            LineMark::Changed,
            LineMark::Changed,
            LineMark::Changed
        ]
    );
}

#[test]
fn ac2_compare_marks_sequence_positions_whose_section_differs() {
    let details = details();
    let this_lyrics = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
            section(3, "Ending", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    let other_lyrics = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
            section(3, "Ending", PaginationState::Draft),
        ],
        vec![(1, 1), (3, 3), (4, 1)],
    );

    assert_eq!(
        compare_arrangements(
            &state(&details, &this_lyrics, true),
            &state(&details, &other_lyrics, true),
        )
        .sequence,
        vec![false, true]
    );
    // Each side is marked against the other: the other side's third position
    // has no counterpart here, so it differs too.
    assert_eq!(
        compare_arrangements(
            &state(&details, &other_lyrics, true),
            &state(&details, &this_lyrics, true),
        )
        .sequence,
        vec![false, true, true]
    );
}

#[test]
fn ac2_unknown_states_are_never_a_difference() {
    let details = details();
    let known = lyrics(
        vec![section(1, "Una", PaginationState::Approved)],
        vec![(1, 1)],
    );
    let unknown = lyrics(
        vec![section(1, "Una", PaginationState::Draft)],
        vec![(1, 1)],
    );

    let comparison = compare_arrangements(
        &state(&details, &known, true),
        &state(&details, &unknown, false),
    );
    assert_eq!(
        comparison.sections[0].state,
        Some(PaginationState::Approved)
    );
    assert!(!comparison.sections[0].state_differs);

    let comparison = compare_arrangements(
        &state(&details, &unknown, false),
        &state(&details, &known, true),
    );
    assert_eq!(comparison.sections[0].state, None);
    assert!(!comparison.sections[0].state_differs);
}

#[test]
fn ac2_overview_marks_each_differing_field() {
    let before_details = details();
    let after_details = ArrangementDetails {
        title: Some("English".to_owned()),
        languages: vec!["English".to_owned()],
        choirs: vec![choir_id(21)],
        rendition_attribution: Some("Adult".to_owned()),
        occasions: vec!["Advent".to_owned()],
        notes: None,
    };
    let content = lyrics(
        vec![section(1, "Una\n\nIkaduha", PaginationState::Draft)],
        vec![(1, 1)],
    );

    let comparison = compare_arrangements(
        &state(&before_details, &content, true),
        &state(&after_details, &content, true),
    );
    assert_eq!(
        comparison.overview,
        domain::song_library::OverviewMarks {
            languages: true,
            choirs: true,
            sections: false,
            sequence: false,
            title: true,
            attribution: true,
            occasions: true,
            notes: true,
        }
    );

    // A differing section count and a longer Sequence mark those rows too.
    let longer = lyrics(
        vec![
            section(1, "Una\n\nIkaduha", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    let comparison = compare_arrangements(
        &state(&before_details, &content, true),
        &state(&before_details, &longer, true),
    );
    assert!(comparison.overview.sections);
    assert!(comparison.overview.sequence);
    assert!(!comparison.overview.title);
}

#[test]
fn ac2_song_comparison_lists_the_changed_fields() {
    let before = SongDraft::titled("Pagdayeg");
    let after = SongDraft {
        title: "Bag-ong Pagdayeg".to_owned(),
        aliases: vec!["Praise".to_owned()],
        original_attribution: Some("Traditional".to_owned()),
        provenance: Some("Hymnal".to_owned()),
        rights_notes: Some("Public domain".to_owned()),
        liturgical_moments: vec!["Communion".to_owned()],
        occasions: vec!["Ordinary Time".to_owned()],
    };

    assert_eq!(
        song_changed_fields(&before, &after),
        vec![
            SongField::Title,
            SongField::Aliases,
            SongField::Attribution,
            SongField::Provenance,
            SongField::RightsNotes,
            SongField::Moments,
            SongField::Occasions,
        ]
    );
    assert_eq!(song_changed_fields(&before, &before), Vec::new());

    // Moments are a set: reordering them changes nothing.
    let mut reordered = after.clone();
    reordered.liturgical_moments = vec!["Entrance".to_owned(), "Communion".to_owned()];
    let mut other = reordered.clone();
    other.liturgical_moments.reverse();
    assert_eq!(song_changed_fields(&reordered, &other), Vec::new());
}

#[test]
fn ac3_carry_review_states_keeps_unchanged_and_returns_changed_approved_sections() {
    let current = lyrics(
        vec![
            section(1, "Una", PaginationState::Approved),
            section(2, "Koro", PaginationState::Approved),
            section(3, "Ending", PaginationState::NeedsReview),
        ],
        vec![(1, 1), (2, 2)],
    );
    let restored = lyrics(
        vec![
            // The restored format-1 document recorded no states: each one is
            // Draft here and would be falsely approved if simply kept.
            section(1, "Una", PaginationState::Approved),
            section(2, "Koro karon", PaginationState::Approved),
            section(3, "Katapusan", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );

    let carried = carry_review_states(&current, restored);
    let states: Vec<PaginationState> = carried
        .sections
        .iter()
        .map(|section| section.pagination)
        .collect();
    assert_eq!(
        states,
        vec![
            PaginationState::Approved,
            PaginationState::NeedsReview,
            PaginationState::NeedsReview
        ],
        "unchanged stays approved, changed approval returns to review, and a \
         NeedsReview section keeps its state"
    );
}

#[test]
fn ac3_carry_review_states_makes_vanished_sections_draft() {
    let current = lyrics(
        vec![section(1, "Una", PaginationState::Approved)],
        vec![(1, 1)],
    );
    let restored = lyrics(
        vec![
            section(1, "Una", PaginationState::Approved),
            section(9, "Bag-o", PaginationState::Approved),
        ],
        vec![(1, 1), (9, 9)],
    );

    let carried = carry_review_states(&current, restored);
    assert_eq!(carried.sections[0].pagination, PaginationState::Approved);
    assert_eq!(
        carried.sections[1].pagination,
        PaginationState::Draft,
        "an identity the current Arrangement no longer has cannot be approved"
    );
}

#[test]
fn two_approvals_or_a_mixed_state_change_are_several() {
    let details = details();
    let draft = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    let both = lyrics(
        vec![
            section(1, "Una", PaginationState::Approved),
            section(2, "Koro", PaginationState::Approved),
        ],
        vec![(1, 1), (2, 2)],
    );
    let review = lyrics(
        vec![
            section(1, "Una", PaginationState::NeedsReview),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    assert_eq!(
        arrangement_change(
            &state(&details, &draft, true),
            &state(&details, &both, true)
        ),
        ArrangementChange::Several
    );
    assert_eq!(
        arrangement_change(
            &state(&details, &draft, true),
            &state(&details, &review, true)
        ),
        ArrangementChange::Several
    );
}

#[test]
fn an_edit_with_its_automatic_review_return_is_one_wording_change() {
    let details = details();
    let before = lyrics(
        vec![section(1, "Una", PaginationState::Approved)],
        vec![(1, 1)],
    );
    let after = lyrics(
        vec![section(1, "Una!", PaginationState::NeedsReview)],
        vec![(1, 1)],
    );
    assert_eq!(
        arrangement_change(
            &state(&details, &before, true),
            &state(&details, &after, true)
        ),
        ArrangementChange::Wording {
            section: section_id(1)
        }
    );
    // An unrelated approval beside the edit is a second change.
    let before = lyrics(
        vec![
            section(1, "Una", PaginationState::Approved),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1)],
    );
    let after = lyrics(
        vec![
            section(1, "Una!", PaginationState::NeedsReview),
            section(2, "Koro", PaginationState::Approved),
        ],
        vec![(1, 1)],
    );
    assert_eq!(
        arrangement_change(
            &state(&details, &before, true),
            &state(&details, &after, true)
        ),
        ArrangementChange::Several
    );
}

#[test]
fn structure_and_sequence_changes_each_get_one_kind() {
    let details = details();
    let base = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2)],
    );
    let classify = |after: &ArrangementLyrics| {
        arrangement_change(&state(&details, &base, true), &state(&details, after, true))
    };

    let added = lyrics(
        vec![
            section(1, "Una", PaginationState::Draft),
            section(2, "Koro", PaginationState::Draft),
            section(3, "Wakas", PaginationState::Draft),
        ],
        vec![(1, 1), (2, 2), (3, 3)],
    );
    assert_eq!(
        classify(&added),
        ArrangementChange::SectionAdded {
            section: section_id(3)
        }
    );
    let removed = lyrics(
        vec![section(1, "Una", PaginationState::Draft)],
        vec![(1, 1)],
    );
    assert_eq!(
        classify(&removed),
        ArrangementChange::SectionRemoved {
            section: section_id(2)
        }
    );
    let mut renamed = base.clone();
    renamed.sections[1].label = Some("Refrain".to_owned());
    assert_eq!(
        classify(&renamed),
        ArrangementChange::SectionRenamed {
            section: section_id(2)
        }
    );
    let mut reordered = base.clone();
    reordered.sections.reverse();
    assert_eq!(classify(&reordered), ArrangementChange::SectionsReordered);

    let mut new_section = base.clone();
    new_section
        .sections
        .push(section(3, "Wakas", PaginationState::Draft));
    let mut placed = new_section.clone();
    placed
        .sequence
        .insert(1, OccurrenceDraft::new(occurrence_id(9), section_id(3)));
    assert_eq!(
        arrangement_change(
            &state(&details, &new_section, true),
            &state(&details, &placed, true),
        ),
        ArrangementChange::OccurrenceAdded {
            section: section_id(3),
            position: 1
        }
    );
    let mut middle = base.clone();
    middle
        .sequence
        .insert(1, OccurrenceDraft::new(occurrence_id(9), section_id(1)));
    assert_eq!(
        classify(&middle),
        ArrangementChange::Repeated {
            section: section_id(1),
            at_end: false
        }
    );
    let mut moved = base.clone();
    moved.sequence.reverse();
    assert_eq!(
        classify(&moved),
        ArrangementChange::OccurrenceMoved {
            section: section_id(1),
            to: 1
        }
    );
    let mut shorter = base.clone();
    shorter.sequence.remove(1);
    assert_eq!(
        classify(&shorter),
        ArrangementChange::OccurrenceRemoved {
            section: section_id(2),
            position: 1
        }
    );
    // Details beside lyrics is several.
    let mut other_details = details.clone();
    other_details.notes = Some("Other".to_owned());
    assert_eq!(
        arrangement_change(
            &state(&details, &base, true),
            &state(&other_details, &added, true),
        ),
        ArrangementChange::Several
    );
    assert_eq!(
        arrangement_change(
            &state(&details, &base, true),
            &state(&other_details, &base, true),
        ),
        ArrangementChange::Details
    );
}

#[test]
fn session_topic_of_no_changes_is_lyrics() {
    assert_eq!(session_topic(&[]), SessionTopic::Lyrics);
    assert_eq!(
        session_topic(&[ArrangementChange::Details, ArrangementChange::Unchanged]),
        SessionTopic::Details
    );
}
