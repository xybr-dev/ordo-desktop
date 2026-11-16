//! T2.9a lifecycle and reference-safety acceptance tests.
//!
//! Every test goes through the public Song Library Interface and the pure
//! lifecycle helpers declared in `song_library::lifecycle` and
//! `song_library::reference_safety`.

use domain::identity::{
    ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp,
};
use domain::song_library::{
    AfterArchive, ArrangementDraft, ArrangementVisibility, BlockerGroup, ChoirDraft,
    DeletionBlocker, DurableReference, LifecycleAction, OccurrenceDraft, SectionDraft, SongDraft,
    SongLibrary, SongLibraryError, arrangement_lifecycle_actions, blocker_groups,
    can_restore_arrangement_revision, can_restore_song_revision, selection_after_archive,
    song_lifecycle_actions,
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

/// An Arrangement draft in `language` whose section and occurrence identities
/// start at `base`, so two drafts never collide.
fn draft(title: &str, language: &str, base: u8) -> ArrangementDraft {
    let first = section_id(base);
    let second = section_id(base + 1);
    ArrangementDraft {
        title: Some(title.to_owned()),
        languages: vec![language.to_owned()],
        rendition_attribution: None,
        notes: None,
        occasions: Vec::new(),
        sections: vec![
            SectionDraft::labeled(first, "Verse 1", "Unang bersikulo"),
            SectionDraft::labeled(second, "Chorus", "Koro"),
        ],
        sequence: vec![
            OccurrenceDraft::new(occurrence_id(base), first),
            OccurrenceDraft::new(occurrence_id(base + 1), second),
        ],
    }
}

/// One Song (1) with Arrangements (10) and (11), and one Choir (20), all
/// active.
fn small_library() -> SongLibrary {
    let mut library = SongLibrary::new();
    library
        .create_choir(choir_id(20), at(0), ChoirDraft::named("Youth"))
        .unwrap();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Pagdayeg"))
        .unwrap();
    for (id, base) in [(10, 100), (11, 110)] {
        library
            .create_arrangement(
                arrangement_id(id),
                song_id(1),
                at(1),
                draft("Cebuano", "Cebuano", base),
                vec![choir_id(20)],
            )
            .unwrap();
    }
    library
}

fn selectable(library: &SongLibrary, song: u8) -> Vec<ArrangementId> {
    library
        .selectable_arrangements(song_id(song))
        .unwrap()
        .iter()
        .map(|arrangement| {
            assert_eq!(
                arrangement.song(),
                song_id(song),
                "a selectable Arrangement belongs to the Song"
            );
            arrangement.id()
        })
        .collect()
}

#[test]
fn ac4_archiving_a_song_keeps_every_child_archive_state() {
    let mut library = small_library();
    library
        .archive_arrangement(arrangement_id(11), at(4))
        .unwrap();

    library.archive_song(song_id(1), at(5)).unwrap();

    assert!(library.song(song_id(1)).unwrap().is_archived());
    assert!(
        !library
            .arrangement(arrangement_id(10))
            .unwrap()
            .is_archived(),
        "archiving a Song never archives an active child"
    );
    assert!(
        library
            .arrangement(arrangement_id(11))
            .unwrap()
            .is_archived(),
        "an independently archived child stays archived"
    );

    library.restore_song(song_id(1)).unwrap();

    assert!(!library.song(song_id(1)).unwrap().is_archived());
    assert!(
        !library
            .arrangement(arrangement_id(10))
            .unwrap()
            .is_archived(),
        "restoring a Song never restores a child"
    );
    assert!(
        library
            .arrangement(arrangement_id(11))
            .unwrap()
            .is_archived(),
        "restoring a Song keeps an independently archived child archived"
    );
}

#[test]
fn ac4_an_archived_song_suppresses_child_selection() {
    let mut library = small_library();
    library.archive_song(song_id(1), at(5)).unwrap();

    assert_eq!(
        library.arrangement_visibility(arrangement_id(10)),
        Ok(ArrangementVisibility::SuppressedBySong)
    );
    assert!(!library.is_selectable(arrangement_id(10)).unwrap());
    assert!(selectable(&library, 1).is_empty());
    assert!(
        library
            .selectable_choir_arrangements(choir_id(20))
            .unwrap()
            .is_empty(),
        "a Choir's selectable repertoire excludes an archived Song's Arrangements"
    );
}

#[test]
fn ac4_restoring_the_song_exposes_only_independently_active_children() {
    let mut library = small_library();
    library
        .archive_arrangement(arrangement_id(11), at(4))
        .unwrap();
    library.archive_song(song_id(1), at(5)).unwrap();

    library.restore_song(song_id(1)).unwrap();

    assert_eq!(
        library.selectable_arrangements(song_id(1)).unwrap().len(),
        1
    );
    assert_eq!(
        library.selectable_arrangements(song_id(1)).unwrap()[0].id(),
        arrangement_id(10)
    );
    assert_eq!(
        library.arrangement_visibility(arrangement_id(11)),
        Ok(ArrangementVisibility::Archived)
    );
}

#[test]
fn ac4_arrangement_archive_is_independent_of_its_song() {
    let mut library = small_library();
    library.archive_song(song_id(1), at(5)).unwrap();

    // Archive an active child while its Song is archived.
    library
        .archive_arrangement(arrangement_id(10), at(6))
        .unwrap();
    assert_eq!(
        library.arrangement_visibility(arrangement_id(10)),
        Ok(ArrangementVisibility::Archived),
        "its own archive state wins over suppression"
    );

    // Restore it while the Song is still archived: it becomes suppressed.
    library.restore_arrangement(arrangement_id(10)).unwrap();
    assert!(
        !library
            .arrangement(arrangement_id(10))
            .unwrap()
            .is_archived()
    );
    assert_eq!(
        library.arrangement_visibility(arrangement_id(10)),
        Ok(ArrangementVisibility::SuppressedBySong)
    );

    // The Song is untouched by both calls.
    assert!(library.song(song_id(1)).unwrap().is_archived());
}

#[test]
fn ac6_blocker_groups_name_each_reference_kind_in_order() {
    let other = DurableReference::Other {
        kind: "export",
        id: entity(7),
    };
    let blockers = vec![
        DeletionBlocker::HasArrangements(vec![arrangement_id(11), arrangement_id(10)]),
        DeletionBlocker::Referenced(vec![
            DurableReference::Other {
                kind: "export",
                id: entity(9),
            },
            DurableReference::MassPlan(entity(2)),
            DurableReference::PresentationSnapshot(entity(5)),
            DurableReference::CelebrationPackage(entity(3)),
            DurableReference::MassPlan(entity(1)),
            DurableReference::MassPlan(entity(2)),
        ]),
        DeletionBlocker::Referenced(vec![DurableReference::MassPlan(entity(4))]),
        DeletionBlocker::NotArchived,
    ];

    assert_eq!(
        blocker_groups(&blockers),
        vec![
            BlockerGroup::NotArchived,
            BlockerGroup::Arrangements(vec![arrangement_id(11), arrangement_id(10)]),
            BlockerGroup::MassPlans(vec![entity(2), entity(1), entity(4)]),
            BlockerGroup::PresentationSnapshots(vec![entity(5)]),
            BlockerGroup::CelebrationPackages(vec![entity(3)]),
            BlockerGroup::Other(vec![("export", entity(9))]),
        ]
    );
    // Input order is kept, duplicates once, and several Referenced blockers
    // merge. Empty groups are omitted, and the other-record kind is carried through.
    assert_eq!(
        blocker_groups(&[DeletionBlocker::Referenced(vec![other])]),
        vec![BlockerGroup::Other(vec![("export", entity(7))])]
    );
    assert_eq!(blocker_groups(&[]), Vec::new());
}

#[test]
fn ac6_a_song_with_only_archived_arrangements_is_still_blocked() {
    let mut library = small_library();
    library
        .archive_arrangement(arrangement_id(10), at(4))
        .unwrap();
    library
        .archive_arrangement(arrangement_id(11), at(4))
        .unwrap();
    library.archive_song(song_id(1), at(5)).unwrap();

    let verdict = library.song_deletion_verdict(song_id(1), &[]).unwrap();
    assert_eq!(
        verdict,
        domain::song_library::DeletionVerdict::Blocked(vec![DeletionBlocker::HasArrangements(
            vec![arrangement_id(10), arrangement_id(11)]
        )])
    );
    let before = library.clone();
    assert_eq!(
        library.delete_song(song_id(1), &[]),
        Err(SongLibraryError::DeletionBlocked(vec![
            DeletionBlocker::HasArrangements(vec![arrangement_id(10), arrangement_id(11)])
        ]))
    );
    assert_eq!(library, before, "a refused deletion changes nothing");
}

#[test]
fn ac8_selection_after_archive_prefers_the_next_active_result() {
    let visible = [(song_id(1), true), (song_id(2), true), (song_id(3), true)];
    assert_eq!(
        selection_after_archive(&visible, song_id(2), false),
        AfterArchive::Select(song_id(3))
    );
    assert_eq!(
        selection_after_archive(&visible, song_id(1), false),
        AfterArchive::Select(song_id(2))
    );
}

#[test]
fn ac8_selection_after_archive_falls_back_to_the_previous_active_result() {
    let visible = [(song_id(1), true), (song_id(2), true), (song_id(3), true)];
    assert_eq!(
        selection_after_archive(&visible, song_id(3), false),
        AfterArchive::Select(song_id(2))
    );
}

#[test]
fn ac8_archived_neighbours_are_skipped() {
    let visible = [
        (song_id(1), true),
        (song_id(2), false),
        (song_id(3), true),
        (song_id(4), false),
    ];
    assert_eq!(
        selection_after_archive(&visible, song_id(1), false),
        AfterArchive::Select(song_id(3))
    );
    // Backwards from the removed last entry, archived entries are skipped too.
    assert_eq!(
        selection_after_archive(&visible, song_id(4), false),
        AfterArchive::Select(song_id(3))
    );
}

#[test]
fn ac8_no_active_neighbour_keeps_a_still_visible_song_or_yields_the_empty_state() {
    let visible = [(song_id(1), false), (song_id(2), true), (song_id(3), false)];
    assert_eq!(
        selection_after_archive(&visible, song_id(2), false),
        AfterArchive::EmptyState
    );
    assert_eq!(
        selection_after_archive(&visible, song_id(2), true),
        AfterArchive::Keep
    );
}

/// Every lifecycle action, exhaustively: adding a fourth variant must be a
/// deliberate edit here, and no merge action exists.
fn action_name(action: LifecycleAction) -> &'static str {
    match action {
        LifecycleAction::Archive => "archive",
        LifecycleAction::Restore => "restore",
        LifecycleAction::DeletePermanently => "delete permanently",
    }
}

#[test]
fn ac9_lifecycle_actions_are_archive_restore_and_delete_only() {
    let mut library = small_library();

    let song = library.song(song_id(1)).unwrap();
    assert_eq!(
        song_lifecycle_actions(song),
        [LifecycleAction::Archive].as_slice(),
        "an active Song offers only Archive"
    );
    let arrangement = library.arrangement(arrangement_id(10)).unwrap();
    assert_eq!(
        arrangement_lifecycle_actions(arrangement),
        [LifecycleAction::Archive].as_slice(),
        "an active Arrangement offers only Archive"
    );

    library.archive_song(song_id(1), at(5)).unwrap();
    library
        .archive_arrangement(arrangement_id(10), at(5))
        .unwrap();
    assert_eq!(
        song_lifecycle_actions(library.song(song_id(1)).unwrap()),
        [LifecycleAction::Restore, LifecycleAction::DeletePermanently].as_slice()
    );
    assert_eq!(
        arrangement_lifecycle_actions(library.arrangement(arrangement_id(10)).unwrap()),
        [LifecycleAction::Restore, LifecycleAction::DeletePermanently].as_slice()
    );

    let mut names: Vec<&str> = [
        LifecycleAction::Archive,
        LifecycleAction::Restore,
        LifecycleAction::DeletePermanently,
    ]
    .into_iter()
    .map(action_name)
    .collect();
    names.sort_unstable();
    assert_eq!(names, ["archive", "delete permanently", "restore"]);

    let source = include_str!("../../../crates/domain/src/song_library/lifecycle.rs");
    for pattern in ["fn merge", "fn redirect", "Merge", "Redirect"] {
        assert!(
            !source.contains(pattern),
            "lifecycle offers no merge or redirect action (Decision 167)"
        );
    }
}

#[test]
fn archived_entities_cannot_restore_a_revision() {
    let mut library = small_library();
    assert!(can_restore_song_revision(library.song(song_id(1)).unwrap()));
    assert!(can_restore_arrangement_revision(
        ArrangementVisibility::Active
    ));

    library.archive_song(song_id(1), at(5)).unwrap();
    assert!(!can_restore_song_revision(
        library.song(song_id(1)).unwrap()
    ));
    assert!(!can_restore_arrangement_revision(
        ArrangementVisibility::SuppressedBySong
    ));

    library
        .archive_arrangement(arrangement_id(10), at(6))
        .unwrap();
    assert!(!can_restore_arrangement_revision(
        ArrangementVisibility::Archived
    ));
}
