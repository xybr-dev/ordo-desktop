//! T2.3 acceptance tests for the Song Library Module Interface.
//!
//! Every test goes through the public `SongLibrary` Interface only. Test names
//! start with the acceptance criterion they cover.

use domain::identity::{
    ArrangementId, ChoirId, EntityId, OccurrenceId, SectionId, SongId, Timestamp,
};
use domain::song_library::{
    ArchiveState, ArrangementDraft, ArrangementVisibility, DeletionBlocker, DeletionVerdict,
    DuplicateEvidence, DurableReference, MatchedOn, OccurrenceDraft, PaginationState, RecordRef,
    SectionDraft, SongDraft, SongLibrary, SongLibraryError, StatusFilter, match_key,
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
        title: title.to_owned(),
        languages: vec![language.to_owned()],
        rendition_attribution: None,
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

/// The Song identities `library` lists for `filter`.
fn song_ids(library: &SongLibrary, filter: StatusFilter) -> Vec<SongId> {
    library
        .songs_with_status(filter)
        .iter()
        .map(|song| song.id())
        .collect()
}

/// Names of every public function declared in the Song Library Interface and
/// model sources (`pub fn`, `pub const fn`, `pub async fn`; not `pub(crate)`).
fn public_fn_names() -> Vec<String> {
    let sources = [
        include_str!("../../../crates/domain/src/song_library/interface.rs"),
        include_str!("../../../crates/domain/src/song_library/model/mod.rs"),
        include_str!("../../../crates/domain/src/song_library/model/song.rs"),
        include_str!("../../../crates/domain/src/song_library/model/arrangement.rs"),
        include_str!("../../../crates/domain/src/song_library/model/choir.rs"),
        include_str!("../../../crates/domain/src/song_library/model/text.rs"),
    ];
    let mut names = Vec::new();
    for source in sources {
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        for line in production.lines() {
            let Some(mut rest) = line.trim_start().strip_prefix("pub ") else {
                continue;
            };
            while let Some(next) = rest
                .strip_prefix("const ")
                .or_else(|| rest.strip_prefix("async "))
            {
                rest = next;
            }
            if let Some(rest) = rest.strip_prefix("fn ") {
                let name = rest.split(['(', '<']).next().unwrap_or(rest);
                names.push(name.trim().to_lowercase());
            }
        }
    }
    names
}

/// Asserts a refused call reports `expected` and leaves `library` unchanged.
macro_rules! assert_refused {
    ($library:ident, $expected:expr, $call:expr) => {{
        let before = $library.clone();
        assert_eq!($call.unwrap_err(), $expected);
        assert_eq!($library, before, "a refused call changes nothing");
    }};
}

/// One Song (1) with Arrangement (10) and Choir (20), all active.
fn small_library() -> SongLibrary {
    let mut library = SongLibrary::new();
    library
        .create_choir(choir_id(20), at(0), "Youth".to_owned())
        .unwrap();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Pagdayeg"))
        .unwrap();
    library
        .create_arrangement(
            arrangement_id(10),
            song_id(1),
            at(1),
            draft("Cebuano", "ceb", 100),
            vec![choir_id(20)],
        )
        .unwrap();
    library
}

#[test]
fn ac1_song_work_owns_its_content_and_needs_only_a_title() {
    let mut library = SongLibrary::new();
    let minimal = library
        .create_song(song_id(1), at(0), SongDraft::titled("Pagdayeg"))
        .unwrap();
    assert_eq!(minimal.title(), "Pagdayeg");
    assert!(minimal.aliases().is_empty());
    assert_eq!(minimal.original_attribution(), None);
    assert_eq!(minimal.archive_state(), ArchiveState::Active);

    let full = SongDraft {
        title: "Amazing Grace".to_owned(),
        aliases: vec!["Grace".to_owned()],
        original_attribution: Some("John Newton".to_owned()),
        provenance: Some("Hymnal".to_owned()),
        rights_notes: Some("Public domain".to_owned()),
        liturgical_moments: vec!["Communion".to_owned()],
        occasions: vec!["Ordinary Time".to_owned()],
    };
    let stored = library.create_song(song_id(2), at(1), full).unwrap();
    assert_eq!(stored.id(), song_id(2));
    assert_eq!(stored.aliases(), ["Grace".to_owned()]);
    assert_eq!(stored.original_attribution(), Some("John Newton"));
    assert_eq!(stored.provenance(), Some("Hymnal"));
    assert_eq!(stored.rights_notes(), Some("Public domain"));
    assert!(stored.liturgical_moments().contains("Communion"));
    assert!(stored.occasions().contains("Ordinary Time"));
    assert_eq!(stored.created_at(), at(1));

    assert!(matches!(
        library.create_song(song_id(3), at(2), SongDraft::titled("  ")),
        Err(SongLibraryError::BlankField(_))
    ));
}

#[test]
fn ac2_arrangement_belongs_to_one_song_and_independently_owns_its_content() {
    let mut library = small_library();
    let mut english = draft("English", "en", 110);
    english.rendition_attribution = Some("Fr. Cruz".to_owned());
    english.occasions = vec!["Christmas".to_owned()];
    library
        .create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(2),
            english,
            vec![choir_id(20)],
        )
        .unwrap();

    let cebuano = library.arrangement(arrangement_id(10)).unwrap();
    let english = library.arrangement(arrangement_id(11)).unwrap();
    assert_eq!(cebuano.song(), song_id(1));
    assert_eq!(english.song(), song_id(1));
    assert_eq!(cebuano.languages(), ["ceb".to_owned()]);
    assert_eq!(english.languages(), ["en".to_owned()]);
    assert_eq!(english.rendition_attribution(), Some("Fr. Cruz"));
    assert_eq!(cebuano.rendition_attribution(), None);
    assert!(english.occasions().contains("Christmas"));
    assert_eq!(english.sections().len(), 2);
    assert_eq!(english.sequence().len(), 2);
    assert_eq!(cebuano.pagination_state(), PaginationState::Draft);

    // Archiving one Arrangement leaves its sibling untouched.
    library
        .archive_arrangement(arrangement_id(11), at(3))
        .unwrap();
    assert!(
        !library
            .arrangement(arrangement_id(10))
            .unwrap()
            .is_archived()
    );
    assert!(
        library
            .arrangement(arrangement_id(11))
            .unwrap()
            .is_archived()
    );

    assert_eq!(
        library
            .create_arrangement(
                arrangement_id(12),
                song_id(99),
                at(4),
                draft("Orphan", "en", 120),
                Vec::new(),
            )
            .unwrap_err(),
        SongLibraryError::UnknownSong(song_id(99))
    );
}

#[test]
fn ac3_numbered_settings_are_separate_songs_and_translations_are_arrangements() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie — 8th Mass"))
        .unwrap();
    library
        .create_song(song_id(2), at(1), SongDraft::titled("Kyrie — 13th Mass"))
        .unwrap();
    assert_eq!(library.songs().count(), 2);
    assert!(
        library
            .duplicate_evidence(&SongDraft::titled("Kyrie — 13th Mass"), Some(song_id(2)))
            .is_empty()
    );

    library
        .create_arrangement(
            arrangement_id(10),
            song_id(1),
            at(2),
            draft("Cebuano", "ceb", 100),
            Vec::new(),
        )
        .unwrap();
    library
        .create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(3),
            draft("English", "en", 110),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(library.songs().count(), 2);
    let ids: Vec<_> = library
        .arrangements_for_song(song_id(1))
        .unwrap()
        .iter()
        .map(|arrangement| arrangement.id())
        .collect();
    assert_eq!(ids, [arrangement_id(10), arrangement_id(11)]);
    assert!(
        library
            .arrangements_for_song(song_id(2))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ac4_choirs_share_one_arrangement_and_songs_are_valid_without_a_choir() {
    let mut library = SongLibrary::new();
    for (n, name) in [(20, "Youth"), (21, "Adult")] {
        library
            .create_choir(choir_id(n), at(0), name.to_owned())
            .unwrap();
    }
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Pagdayeg"))
        .unwrap();
    library
        .create_arrangement(
            arrangement_id(10),
            song_id(1),
            at(1),
            draft("Cebuano", "ceb", 100),
            vec![choir_id(20), choir_id(21)],
        )
        .unwrap();
    library
        .create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(2),
            draft("English", "en", 110),
            Vec::new(),
        )
        .unwrap();

    for choir in [choir_id(20), choir_id(21)] {
        let sung: Vec<_> = library
            .choir_arrangements(choir)
            .unwrap()
            .iter()
            .map(|arrangement| arrangement.id())
            .collect();
        assert_eq!(sung, [arrangement_id(10)]);
    }
    assert!(
        library
            .arrangement(arrangement_id(11))
            .unwrap()
            .choirs()
            .is_empty()
    );
    assert_eq!(library.song(song_id(1)).unwrap().title(), "Pagdayeg");
}

#[test]
fn ac5_duplicate_evidence_reports_without_rewriting_or_uniting() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Amazing Grace"))
        .unwrap();
    library
        .create_song(song_id(2), at(1), SongDraft::titled("amazing  grace"))
        .unwrap();

    // Both exist, verbatim; nothing was united or recapitalized.
    assert_eq!(library.songs().count(), 2);
    assert_eq!(library.song(song_id(1)).unwrap().title(), "Amazing Grace");
    assert_eq!(library.song(song_id(2)).unwrap().title(), "amazing  grace");

    let for_second = library.duplicate_evidence(
        &SongDraft::titled(library.song(song_id(2)).unwrap().title()),
        Some(song_id(2)),
    );
    assert_eq!(
        for_second,
        [DuplicateEvidence {
            song: song_id(1),
            candidate_on: MatchedOn::Title,
            existing_on: MatchedOn::Title,
        }]
    );
    let for_first =
        library.duplicate_evidence(&SongDraft::titled("Amazing Grace"), Some(song_id(1)));
    assert_eq!(
        for_first,
        [DuplicateEvidence {
            song: song_id(2),
            candidate_on: MatchedOn::Title,
            existing_on: MatchedOn::Title,
        }]
    );

    // Editing a Song consults nothing and keeps authored capitalization.
    let edited = library
        .edit_song(song_id(2), SongDraft::titled("AMAZING GRACE"))
        .unwrap();
    assert_eq!(edited.title(), "AMAZING GRACE");
}

#[test]
fn ac5_match_key_folds_only_case_and_whitespace() {
    assert_eq!(match_key("  Amazing \t Grace\n"), "amazing grace");
    assert_eq!(match_key("ÁVE"), "áve");
    assert_ne!(
        match_key("Kyrie — 8th Mass"),
        match_key("Kyrie — 13th Mass")
    );
    assert_ne!(match_key("Ave"), match_key("Avé"));
    assert_ne!(match_key("Kyrie, 8"), match_key("Kyrie 8"));
}

#[test]
fn ac5_duplicate_evidence_matches_aliases_includes_archived_and_orders_stably() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(2), at(5), SongDraft::titled("Pagdayeg"))
        .unwrap();
    let mut aliased = SongDraft::titled("Praise");
    aliased.aliases = vec!["PAGDAYEG".to_owned()];
    library.create_song(song_id(1), at(5), aliased).unwrap();
    library
        .create_song(song_id(3), at(1), SongDraft::titled("Pagdayeg "))
        .unwrap();
    library.archive_song(song_id(3), at(9)).unwrap();

    let mut candidate = SongDraft::titled("pagdayeg");
    candidate.aliases = vec!["praise".to_owned()];
    let evidence = library.duplicate_evidence(&candidate, None);
    assert_eq!(
        evidence,
        [
            // Song 3: oldest; archived Songs are still reported.
            DuplicateEvidence {
                song: song_id(3),
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Title,
            },
            // Song 1 and 2 share created_at; identity breaks the tie.
            DuplicateEvidence {
                song: song_id(1),
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Alias(0),
            },
            DuplicateEvidence {
                song: song_id(1),
                candidate_on: MatchedOn::Alias(0),
                existing_on: MatchedOn::Title,
            },
            DuplicateEvidence {
                song: song_id(2),
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Title,
            },
        ]
    );
    assert!(library.song(song_id(3)).unwrap().is_archived());

    let except = library.duplicate_evidence(&candidate, Some(song_id(3)));
    assert!(except.iter().all(|entry| entry.song != song_id(3)));
}

#[test]
fn ac6_archival_is_non_cascading_and_reversible() {
    let mut library = small_library();
    library
        .create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(2),
            draft("English", "en", 110),
            vec![choir_id(20)],
        )
        .unwrap();
    library
        .archive_arrangement(arrangement_id(11), at(3))
        .unwrap();
    library.archive_song(song_id(1), at(4)).unwrap();

    // Archiving the Song did not touch either Arrangement.
    assert!(
        !library
            .arrangement(arrangement_id(10))
            .unwrap()
            .is_archived()
    );
    assert_eq!(
        library
            .arrangement(arrangement_id(11))
            .unwrap()
            .archive_state(),
        ArchiveState::Archived(at(3))
    );
    assert_eq!(
        library.arrangement_visibility(arrangement_id(10)),
        Ok(ArrangementVisibility::SuppressedBySong)
    );
    assert_eq!(
        library.arrangement_visibility(arrangement_id(11)),
        Ok(ArrangementVisibility::Archived)
    );
    assert!(
        library
            .selectable_arrangements(song_id(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(library.is_selectable(arrangement_id(10)), Ok(false));

    // Restoring exposes only the independently active Arrangement.
    library.restore_song(song_id(1)).unwrap();
    assert_eq!(library.is_selectable(arrangement_id(10)), Ok(true));
    assert_eq!(library.is_selectable(arrangement_id(11)), Ok(false));
    let selectable: Vec<_> = library
        .selectable_arrangements(song_id(1))
        .unwrap()
        .iter()
        .map(|arrangement| arrangement.id())
        .collect();
    assert_eq!(selectable, [arrangement_id(10)]);

    library.restore_arrangement(arrangement_id(11)).unwrap();
    assert_eq!(library.is_selectable(arrangement_id(11)), Ok(true));
}

#[test]
fn ac6_choir_archive_keeps_associations_and_arrangements_valid() {
    let mut library = small_library();
    library.archive_choir(choir_id(20), at(5)).unwrap();
    let arrangement = library.arrangement(arrangement_id(10)).unwrap();
    assert!(arrangement.choirs().contains(&choir_id(20)));
    assert_eq!(library.is_selectable(arrangement_id(10)), Ok(true));
    // Repertoire membership is not availability.
    let sung: Vec<_> = library
        .selectable_choir_arrangements(choir_id(20))
        .unwrap()
        .iter()
        .map(|arrangement| arrangement.id())
        .collect();
    assert_eq!(sung, [arrangement_id(10)]);
    library.restore_choir(choir_id(20)).unwrap();
    assert!(!library.choir(choir_id(20)).unwrap().is_archived());
}

#[test]
fn ac6_archive_and_restore_state_errors_name_the_record() {
    let mut library = small_library();
    assert_refused!(
        library,
        SongLibraryError::NotArchived(RecordRef::Song(song_id(1))),
        library.restore_song(song_id(1))
    );
    assert_refused!(
        library,
        SongLibraryError::NotArchived(RecordRef::Arrangement(arrangement_id(10))),
        library.restore_arrangement(arrangement_id(10))
    );
    assert_refused!(
        library,
        SongLibraryError::NotArchived(RecordRef::Choir(choir_id(20))),
        library.restore_choir(choir_id(20))
    );
    library.archive_song(song_id(1), at(5)).unwrap();
    library
        .archive_arrangement(arrangement_id(10), at(5))
        .unwrap();
    library.archive_choir(choir_id(20), at(5)).unwrap();
    assert_refused!(
        library,
        SongLibraryError::AlreadyArchived(RecordRef::Song(song_id(1))),
        library.archive_song(song_id(1), at(6))
    );
    assert_refused!(
        library,
        SongLibraryError::AlreadyArchived(RecordRef::Arrangement(arrangement_id(10))),
        library.archive_arrangement(arrangement_id(10), at(6))
    );
    assert_refused!(
        library,
        SongLibraryError::AlreadyArchived(RecordRef::Choir(choir_id(20))),
        library.archive_choir(choir_id(20), at(6))
    );
    assert_refused!(
        library,
        SongLibraryError::UnknownSong(song_id(99)),
        library.archive_song(song_id(99), at(6))
    );
}

#[test]
fn ac6_archived_entities_are_inspect_only_until_restored() {
    let mut library = small_library();
    library
        .create_choir(choir_id(21), at(0), "Adult".to_owned())
        .unwrap();
    library.archive_song(song_id(1), at(5)).unwrap();
    let song = RecordRef::Song(song_id(1));

    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.edit_song(song_id(1), SongDraft::titled("Changed"))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.edit_arrangement(arrangement_id(10), draft("Changed", "en", 100))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.set_arrangement_choirs(arrangement_id(10), Vec::new())
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.add_arrangement_choir(arrangement_id(10), choir_id(21))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.remove_arrangement_choir(arrangement_id(10), choir_id(20))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.approve_pagination(arrangement_id(10))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(song),
        library.create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(6),
            draft("English", "en", 110),
            Vec::new(),
        )
    );

    // Restoring the Song reopens it for editing.
    library.restore_song(song_id(1)).unwrap();
    library
        .edit_song(song_id(1), SongDraft::titled("Changed"))
        .unwrap();
    library.approve_pagination(arrangement_id(10)).unwrap();

    // An archived Arrangement is inspect-only on its own.
    library
        .archive_arrangement(arrangement_id(10), at(7))
        .unwrap();
    let arrangement = RecordRef::Arrangement(arrangement_id(10));
    assert_refused!(
        library,
        SongLibraryError::Archived(arrangement),
        library.edit_arrangement(arrangement_id(10), draft("Changed", "en", 100))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(arrangement),
        library.approve_pagination(arrangement_id(10))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(arrangement),
        library.add_arrangement_choir(arrangement_id(10), choir_id(21))
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(arrangement),
        library.set_arrangement_choirs(arrangement_id(10), Vec::new())
    );
    assert_refused!(
        library,
        SongLibraryError::Archived(arrangement),
        library.remove_arrangement_choir(arrangement_id(10), choir_id(20))
    );
}

#[test]
fn ac6_archived_choir_is_inspect_only_and_cannot_be_newly_associated() {
    let mut library = small_library();
    library
        .create_choir(choir_id(21), at(0), "Adult".to_owned())
        .unwrap();
    library.archive_choir(choir_id(21), at(5)).unwrap();
    library.archive_choir(choir_id(20), at(5)).unwrap();
    let archived = SongLibraryError::Archived(RecordRef::Choir(choir_id(21)));

    assert_refused!(
        library,
        SongLibraryError::Archived(RecordRef::Choir(choir_id(20))),
        library.edit_choir(choir_id(20), "Renamed".to_owned())
    );
    assert_refused!(
        library,
        archived,
        library.add_arrangement_choir(arrangement_id(10), choir_id(21))
    );
    assert_refused!(
        library,
        archived,
        library.set_arrangement_choirs(arrangement_id(10), vec![choir_id(20), choir_id(21)])
    );
    assert_refused!(
        library,
        archived,
        library.create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(6),
            draft("English", "en", 110),
            vec![choir_id(21)],
        )
    );
    // Existing associations stay editable as a set.
    library
        .set_arrangement_choirs(arrangement_id(10), vec![choir_id(20)])
        .unwrap();
    library
        .add_arrangement_choir(arrangement_id(10), choir_id(20))
        .unwrap();
}

#[test]
fn ac6_songs_with_status_filters_and_orders_by_creation_time() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(3), at(0), SongDraft::titled("First"))
        .unwrap();
    library
        .create_song(song_id(1), at(5), SongDraft::titled("Second"))
        .unwrap();
    library
        .create_song(song_id(2), at(5), SongDraft::titled("Third"))
        .unwrap();
    library.archive_song(song_id(1), at(9)).unwrap();
    let ids = |filter| -> Vec<SongId> {
        library
            .songs_with_status(filter)
            .iter()
            .map(|song| song.id())
            .collect()
    };
    assert_eq!(ids(StatusFilter::Active), [song_id(3), song_id(2)]);
    assert_eq!(ids(StatusFilter::Archived), [song_id(1)]);
    assert_eq!(ids(StatusFilter::All), [song_id(3), song_id(1), song_id(2)]);
}

#[test]
fn ac6_deletion_needs_archive_no_arrangements_and_no_references() {
    let mut library = small_library();
    let plan = DurableReference::MassPlan(entity(50));

    // Active Song with an Arrangement: both blockers, in order.
    assert_eq!(
        library.song_deletion_verdict(song_id(1), &[]),
        Ok(DeletionVerdict::Blocked(vec![
            DeletionBlocker::NotArchived,
            DeletionBlocker::HasArrangements(vec![arrangement_id(10)]),
        ]))
    );
    let blockers = vec![
        DeletionBlocker::NotArchived,
        DeletionBlocker::HasArrangements(vec![arrangement_id(10)]),
        DeletionBlocker::Referenced(vec![plan.clone()]),
    ];
    assert_refused!(
        library,
        SongLibraryError::DeletionBlocked(blockers),
        library.delete_song(song_id(1), std::slice::from_ref(&plan))
    );

    // An archived Song that still owns an archived Arrangement stays blocked.
    library
        .archive_arrangement(arrangement_id(10), at(5))
        .unwrap();
    library.archive_song(song_id(1), at(6)).unwrap();
    assert_refused!(
        library,
        SongLibraryError::DeletionBlocked(vec![DeletionBlocker::HasArrangements(vec![
            arrangement_id(10)
        ])]),
        library.delete_song(song_id(1), &[])
    );

    // The Arrangement's own state decides, even with its Song archived.
    assert_refused!(
        library,
        SongLibraryError::DeletionBlocked(vec![DeletionBlocker::Referenced(vec![plan.clone()])]),
        library.delete_arrangement(arrangement_id(10), std::slice::from_ref(&plan))
    );
    assert_eq!(
        library.arrangement_deletion_verdict(arrangement_id(10), &[]),
        Ok(DeletionVerdict::Allowed)
    );
    let removed = library.delete_arrangement(arrangement_id(10), &[]).unwrap();
    assert_eq!(removed.id(), arrangement_id(10));
    assert_eq!(
        library.arrangement(arrangement_id(10)).unwrap_err(),
        SongLibraryError::UnknownArrangement(arrangement_id(10))
    );

    // Now the Song may go; its references still veto it.
    assert_refused!(
        library,
        SongLibraryError::DeletionBlocked(vec![DeletionBlocker::Referenced(vec![
            DurableReference::CelebrationPackage(entity(51))
        ])]),
        library.delete_song(
            song_id(1),
            &[DurableReference::CelebrationPackage(entity(51))]
        )
    );
    let song = library.delete_song(song_id(1), &[]).unwrap();
    assert_eq!(song.id(), song_id(1));
    assert_eq!(library.songs().count(), 0);
}

#[test]
fn ac6_active_arrangement_deletion_is_blocked_and_choirs_cannot_be_deleted() {
    let mut library = small_library();
    assert_refused!(
        library,
        SongLibraryError::DeletionBlocked(vec![DeletionBlocker::NotArchived]),
        library.delete_arrangement(arrangement_id(10), &[])
    );
    // Reference lists are sorted and deduplicated so the verdict is stable.
    library
        .archive_arrangement(arrangement_id(10), at(5))
        .unwrap();
    let verdict = library
        .arrangement_deletion_verdict(
            arrangement_id(10),
            &[
                DurableReference::PresentationSnapshot(entity(9)),
                DurableReference::MassPlan(entity(8)),
                DurableReference::MassPlan(entity(8)),
                DurableReference::Other {
                    kind: "export",
                    id: entity(7),
                },
            ],
        )
        .unwrap();
    assert_eq!(
        verdict,
        DeletionVerdict::Blocked(vec![DeletionBlocker::Referenced(vec![
            DurableReference::MassPlan(entity(8)),
            DurableReference::PresentationSnapshot(entity(9)),
            DurableReference::Other {
                kind: "export",
                id: entity(7),
            },
        ])])
    );
    for name in public_fn_names() {
        assert!(
            !(name.contains("delete") && name.contains("choir")),
            "{name}: Choirs have no permanent deletion in T2.3"
        );
    }
}

#[test]
fn ac6_deleted_arrangement_section_and_occurrence_ids_stay_reserved() {
    let mut library = small_library();
    library
        .archive_arrangement(arrangement_id(10), at(5))
        .unwrap();
    library.delete_arrangement(arrangement_id(10), &[]).unwrap();

    // The identities (section 100/101, occurrence 100/101) may still be named
    // by revisions and snapshots, so a new Arrangement cannot reuse them.
    assert_eq!(
        library
            .create_arrangement(
                arrangement_id(11),
                song_id(1),
                at(6),
                draft("Reuse", "en", 100),
                Vec::new(),
            )
            .unwrap_err(),
        SongLibraryError::DuplicateSection(section_id(100))
    );
    library
        .create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(6),
            draft("Fresh", "en", 130),
            Vec::new(),
        )
        .unwrap();
}

#[test]
fn ac7_the_interface_has_no_merge_or_redirect_operation() {
    let names = public_fn_names();
    assert!(
        names.len() > 40,
        "the scan must see the Interface, saw {}",
        names.len()
    );
    for name in &names {
        assert!(
            !name.contains("merge") && !name.contains("redirect"),
            "{name} must not exist in the MVP Interface"
        );
    }
}

#[test]
fn ac6_dropped_section_and_occurrence_ids_stay_reserved_to_their_arrangement() {
    let mut library = small_library();
    // Arrangement 10 owns sections 100/101 and occurrences 100/101. Drop the
    // second of each by editing.
    let first = section_id(100);
    let mut shrunk = draft("Cebuano", "ceb", 100);
    shrunk.sections.truncate(1);
    shrunk.sequence = vec![OccurrenceDraft::new(occurrence_id(100), first)];
    library
        .edit_arrangement(arrangement_id(10), shrunk)
        .unwrap();

    // Another Arrangement can neither be created nor edited onto them.
    assert_refused!(
        library,
        SongLibraryError::DuplicateSection(section_id(101)),
        library.create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(6),
            draft("Other", "en", 100 + 1),
            Vec::new(),
        )
    );
    let mut stolen = draft("Other", "en", 110);
    stolen.sequence[0].id = occurrence_id(101);
    assert_refused!(
        library,
        SongLibraryError::DuplicateOccurrence(occurrence_id(101)),
        library.create_arrangement(arrangement_id(11), song_id(1), at(6), stolen, Vec::new(),)
    );
    library
        .create_arrangement(
            arrangement_id(11),
            song_id(1),
            at(6),
            draft("Other", "en", 110),
            Vec::new(),
        )
        .unwrap();
    let mut stolen = draft("Other", "en", 110);
    stolen.sections[0].id = section_id(101);
    stolen.sequence[0].section = section_id(101);
    assert_refused!(
        library,
        SongLibraryError::DuplicateSection(section_id(101)),
        library.edit_arrangement(arrangement_id(11), stolen)
    );

    // The same Arrangement may bring them back (restoration of a revision).
    library
        .edit_arrangement(arrangement_id(10), draft("Cebuano", "ceb", 100))
        .unwrap();
    assert_eq!(
        library
            .arrangement(arrangement_id(10))
            .unwrap()
            .sections()
            .len(),
        2
    );
}

#[test]
fn t2_3_e2e_numbered_kyries_translated_arrangements_archive_and_restore() {
    let mut library = SongLibrary::new();

    // 1. Choirs and two numbered Kyries.
    for (n, name) in [(20, "Youth"), (21, "Adult"), (22, "Children")] {
        library
            .create_choir(choir_id(n), at(0), name.to_owned())
            .unwrap();
    }
    let (youth, adult, children) = (choir_id(20), choir_id(21), choir_id(22));
    let eighth = SongDraft::titled("Kyrie — 8th Mass");
    let thirteenth = SongDraft::titled("Kyrie — 13th Mass");
    library
        .create_song(song_id(1), at(1), eighth.clone())
        .unwrap();
    library
        .create_song(song_id(2), at(2), thirteenth.clone())
        .unwrap();
    assert_eq!(library.songs().count(), 2);
    assert!(
        library
            .duplicate_evidence(&eighth, Some(song_id(1)))
            .is_empty()
    );
    assert!(
        library
            .duplicate_evidence(&thirteenth, Some(song_id(2)))
            .is_empty()
    );
    assert!(
        library
            .duplicate_evidence(&eighth, None)
            .iter()
            .all(|e| e.song == song_id(1))
    );

    // 2. Pagdayeg with a Cebuano and an English Arrangement.
    library
        .create_song(song_id(3), at(3), SongDraft::titled("Pagdayeg"))
        .unwrap();
    let (cebuano, english) = (arrangement_id(10), arrangement_id(11));
    library
        .create_arrangement(
            cebuano,
            song_id(3),
            at(4),
            draft("Cebuano", "ceb", 100),
            vec![youth, adult],
        )
        .unwrap();
    library
        .create_arrangement(
            english,
            song_id(3),
            at(5),
            draft("English", "en", 110),
            vec![adult, children],
        )
        .unwrap();
    assert_eq!(
        library.arrangement(cebuano).unwrap().languages(),
        ["ceb".to_owned()]
    );
    assert_eq!(
        library.arrangement(english).unwrap().languages(),
        ["en".to_owned()]
    );
    let ids = |arrangements: Vec<&domain::song_library::Arrangement>| -> Vec<ArrangementId> {
        arrangements.iter().map(|a| a.id()).collect()
    };
    assert_eq!(ids(library.choir_arrangements(youth).unwrap()), [cebuano]);
    assert_eq!(
        ids(library.choir_arrangements(adult).unwrap()),
        [cebuano, english]
    );
    assert_eq!(
        ids(library.choir_arrangements(children).unwrap()),
        [english]
    );
    let repertoire = |choir| -> Vec<SongId> {
        library
            .choir_repertoire(choir)
            .unwrap()
            .iter()
            .map(|song| song.id())
            .collect()
    };
    assert_eq!(repertoire(youth), [song_id(3)]);
    assert_eq!(repertoire(adult), [song_id(3)]);
    assert_eq!(repertoire(children), [song_id(3)]);

    // 3. Archive the English Arrangement, then the parent Song.
    library.archive_arrangement(english, at(6)).unwrap();
    library.archive_song(song_id(3), at(7)).unwrap();
    assert_eq!(
        library.arrangement_visibility(cebuano),
        Ok(ArrangementVisibility::SuppressedBySong)
    );
    assert_eq!(
        library.arrangement_visibility(english),
        Ok(ArrangementVisibility::Archived)
    );
    assert!(
        library
            .selectable_arrangements(song_id(3))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        song_ids(&library, StatusFilter::Active),
        [song_id(1), song_id(2)]
    );
    assert_eq!(song_ids(&library, StatusFilter::Archived), [song_id(3)]);

    // 4. Restore the Song: only the independently active child returns.
    library.restore_song(song_id(3)).unwrap();
    assert_eq!(
        library.arrangement_visibility(cebuano),
        Ok(ArrangementVisibility::Active)
    );
    assert_eq!(library.is_selectable(cebuano), Ok(true));
    assert_eq!(
        library.arrangement_visibility(english),
        Ok(ArrangementVisibility::Archived)
    );
    assert_eq!(library.is_selectable(english), Ok(false));
    assert_eq!(
        ids(library.selectable_arrangements(song_id(3)).unwrap()),
        [cebuano]
    );
    assert_eq!(
        ids(library.selectable_choir_arrangements(adult).unwrap()),
        [cebuano]
    );
    assert!(
        library
            .selectable_choir_arrangements(children)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        song_ids(&library, StatusFilter::Active),
        [song_id(1), song_id(2), song_id(3)]
    );
    assert!(!library.song(song_id(1)).unwrap().is_archived());
    assert!(!library.song(song_id(2)).unwrap().is_archived());

    // 5. Deletion safety. The Song is active again, so its verdict is blocked
    // on NotArchived and on both Arrangements.
    assert_eq!(
        library.song_deletion_verdict(song_id(3), &[]),
        Ok(DeletionVerdict::Blocked(vec![
            DeletionBlocker::NotArchived,
            DeletionBlocker::HasArrangements(vec![cebuano, english]),
        ]))
    );
    let plan = DurableReference::MassPlan(entity(60));
    assert_eq!(
        library.arrangement_deletion_verdict(english, std::slice::from_ref(&plan)),
        Ok(DeletionVerdict::Blocked(vec![DeletionBlocker::Referenced(
            vec![plan]
        )]))
    );
    assert_eq!(
        library.arrangement_deletion_verdict(english, &[]),
        Ok(DeletionVerdict::Allowed)
    );
    library.delete_arrangement(english, &[]).unwrap();
    assert_eq!(
        ids(library.arrangements_for_song(song_id(3)).unwrap()),
        [cebuano]
    );
    assert!(library.song(song_id(3)).is_ok());
}
