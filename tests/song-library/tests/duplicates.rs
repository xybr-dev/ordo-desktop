//! T2.5 WP1 acceptance tests for ranked possible existing Songs.
//!
//! Every test goes through the public `SongLibrary` Interface only. Test names
//! start with the acceptance criterion they cover.

use domain::identity::{EntityId, SongId, Timestamp};
use domain::song_library::{
    IdentityEvidence, MatchStrength, MatchedOn, PossibleSong, SongDraft, SongLibrary, identity_key,
    match_key,
};

fn entity(n: u8) -> EntityId {
    EntityId::from_random_bytes([n; 16])
}

fn song_id(n: u8) -> SongId {
    SongId::from_entity(entity(n))
}

fn at(offset: i64) -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000 + offset)
}

/// The Song identities of `possible`, in rank order.
fn ids(possible: &[PossibleSong]) -> Vec<SongId> {
    possible.iter().map(|entry| entry.song).collect()
}

/// The strengths of `possible`, in rank order.
fn strengths(possible: &[PossibleSong]) -> Vec<MatchStrength> {
    possible.iter().map(|entry| entry.strength).collect()
}

#[test]
fn ac3_exact_title_then_exact_alias_then_near_title_then_near_alias() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie"))
        .unwrap();
    let mut aliased = SongDraft::titled("Kyrie VIII");
    aliased.aliases = vec!["Kyrie".to_owned()];
    library.create_song(song_id(2), at(1), aliased).unwrap();
    library
        .create_song(song_id(3), at(2), SongDraft::titled("Kyrie!"))
        .unwrap();
    let mut near_alias = SongDraft::titled("Lord, have mercy");
    near_alias.aliases = vec!["Kyrie!!".to_owned()];
    library.create_song(song_id(4), at(3), near_alias).unwrap();

    let possible = library.possible_existing_songs(&SongDraft::titled("Kyrie"));
    assert_eq!(
        ids(&possible),
        [song_id(1), song_id(2), song_id(3), song_id(4)]
    );
    assert_eq!(
        strengths(&possible),
        [
            MatchStrength::ExactTitle,
            MatchStrength::ExactAlias,
            MatchStrength::NearTitle,
            MatchStrength::NearAlias,
        ]
    );
}

#[test]
fn ac3_near_match_folds_punctuation_spacing_and_diacritics_without_rewriting() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("ginoo kaloy i kami"))
        .unwrap();
    library
        .create_song(song_id(2), at(1), SongDraft::titled("Ginoó Kaloyi Kami"))
        .unwrap();
    let before = library.clone();

    let possible = library.possible_existing_songs(&SongDraft::titled("Ginoo, Kaloy-i Kami"));
    assert_eq!(ids(&possible), [song_id(1), song_id(2)]);
    assert_eq!(
        strengths(&possible),
        [MatchStrength::NearTitle, MatchStrength::NearTitle]
    );
    for entry in &possible {
        assert_eq!(
            entry.evidence,
            [IdentityEvidence::Name {
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Title,
                exact: false,
            }]
        );
    }

    // Authored titles stay verbatim, and the pair really is a near match: the
    // exact key does not fold punctuation.
    assert_eq!(
        library.song(song_id(1)).unwrap().title(),
        "ginoo kaloy i kami"
    );
    assert_eq!(
        library.song(song_id(2)).unwrap().title(),
        "Ginoó Kaloyi Kami"
    );
    assert_eq!(library, before, "ranking is read-only");
    assert_ne!(
        match_key("Ginoo, Kaloy-i Kami"),
        match_key("ginoo kaloy i kami")
    );
}

#[test]
fn ac3_numbered_settings_are_not_candidates_of_each_other() {
    let mut library = SongLibrary::new();
    for (n, title) in [
        (1, "Kyrie I"),
        (2, "Kyrie II"),
        (3, "Kyrie 1"),
        (4, "Kyrie 8th Mass"),
        (5, "Kyrie 13th Mass"),
    ] {
        library
            .create_song(song_id(n), at(i64::from(n)), SongDraft::titled(title))
            .unwrap();
    }

    let candidates = |title: &str| ids(&library.possible_existing_songs(&SongDraft::titled(title)));
    assert_eq!(candidates("Kyrie I"), [song_id(1)]);
    assert_eq!(candidates("Kyrie II"), [song_id(2)]);
    assert_eq!(candidates("Kyrie 1"), [song_id(3)]);
    assert_eq!(candidates("Kyrie 8th Mass"), [song_id(4)]);
    assert_eq!(candidates("Kyrie 13th Mass"), [song_id(5)]);
}

#[test]
fn ac3_same_composer_outranks_an_equal_strength_candidate_and_never_creates_one() {
    let mut library = SongLibrary::new();
    let mut attributed = SongDraft::titled("Amazing Grace");
    attributed.original_attribution = Some("John Newton".to_owned());
    library.create_song(song_id(2), at(0), attributed).unwrap();
    library
        .create_song(song_id(1), at(1), SongDraft::titled("Amazing Grace"))
        .unwrap();
    let mut composer_only = SongDraft::titled("How Great Thou Art");
    composer_only.original_attribution = Some("John Newton".to_owned());
    library
        .create_song(song_id(3), at(2), composer_only)
        .unwrap();

    let mut candidate = SongDraft::titled("Amazing Grace");
    candidate.original_attribution = Some("john  newton,".to_owned());
    let possible = library.possible_existing_songs(&candidate);

    // Song 2 shares the attribution, so it precedes Song 1 despite the larger
    // identity. Song 3 matches by attribution only, so it is no candidate.
    assert_eq!(ids(&possible), [song_id(2), song_id(1)]);
    assert_eq!(
        strengths(&possible),
        [MatchStrength::ExactTitle, MatchStrength::ExactTitle]
    );
    assert_eq!(
        possible[0].evidence,
        [
            IdentityEvidence::Name {
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Title,
                exact: true,
            },
            IdentityEvidence::SameAttribution,
        ]
    );
    assert_eq!(
        possible[1].evidence,
        [IdentityEvidence::Name {
            candidate_on: MatchedOn::Title,
            existing_on: MatchedOn::Title,
            exact: true,
        }]
    );
}

#[test]
fn ac3_archived_song_is_included_and_every_matching_pair_is_listed() {
    let mut library = SongLibrary::new();
    let mut stored = SongDraft::titled("Praise");
    stored.aliases = vec!["Pagdayeg".to_owned()];
    library.create_song(song_id(1), at(0), stored).unwrap();
    let mut other = SongDraft::titled("Pagdayeg");
    other.aliases = vec!["Praise".to_owned()];
    library.create_song(song_id(2), at(1), other).unwrap();
    library.archive_song(song_id(1), at(2)).unwrap();

    let mut candidate = SongDraft::titled("Praise");
    candidate.aliases = vec!["Pagdayeg".to_owned()];
    let possible = library.possible_existing_songs(&candidate);

    assert_eq!(ids(&possible), [song_id(1), song_id(2)]);
    assert_eq!(
        strengths(&possible),
        [MatchStrength::ExactTitle, MatchStrength::ExactAlias]
    );
    assert!(
        library.song(song_id(1)).unwrap().is_archived(),
        "archived Songs are still ranked"
    );
    // Each Song appears once, listing every matching pair in candidate-name
    // then existing-name order.
    assert_eq!(
        possible[0].evidence,
        [
            IdentityEvidence::Name {
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Title,
                exact: true,
            },
            IdentityEvidence::Name {
                candidate_on: MatchedOn::Alias(0),
                existing_on: MatchedOn::Alias(0),
                exact: true,
            },
        ]
    );
    assert_eq!(
        possible[1].evidence,
        [
            IdentityEvidence::Name {
                candidate_on: MatchedOn::Title,
                existing_on: MatchedOn::Alias(0),
                exact: true,
            },
            IdentityEvidence::Name {
                candidate_on: MatchedOn::Alias(0),
                existing_on: MatchedOn::Title,
                exact: true,
            },
        ]
    );
}

#[test]
fn ac3_blank_names_match_nothing() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie"))
        .unwrap();
    library
        .create_song(song_id(2), at(1), SongDraft::titled("?! — ..."))
        .unwrap();

    assert!(
        library
            .possible_existing_songs(&SongDraft::default())
            .is_empty()
    );
    assert!(
        library
            .possible_existing_songs(&SongDraft::titled("   "))
            .is_empty()
    );
    let mut blank_alias = SongDraft::titled("   ");
    blank_alias.aliases = vec![" ".to_owned()];
    assert!(library.possible_existing_songs(&blank_alias).is_empty());
}

#[test]
fn ac3_symbol_only_names_match_only_an_exact_duplicate() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Kyrie"))
        .unwrap();
    library
        .create_song(song_id(2), at(1), SongDraft::titled("?! — ..."))
        .unwrap();

    // The same symbols, however cased or spaced, are an exact duplicate even
    // though the identity key is empty.
    let possible = library.possible_existing_songs(&SongDraft::titled("  ?!  —  ... "));
    assert_eq!(ids(&possible), [song_id(2)]);
    assert_eq!(strengths(&possible), [MatchStrength::ExactTitle]);
    assert_eq!(
        possible[0].evidence,
        [IdentityEvidence::Name {
            candidate_on: MatchedOn::Title,
            existing_on: MatchedOn::Title,
            exact: true,
        }]
    );
    // Different symbols are not a near match: there is no identity to compare.
    assert!(
        library
            .possible_existing_songs(&SongDraft::titled("!!!"))
            .is_empty()
    );
    // A symbol-only alias matches an equal alias exactly.
    let mut aliased = SongDraft::titled("Lord, have mercy");
    aliased.aliases = vec!["♫".to_owned()];
    library.create_song(song_id(3), at(2), aliased).unwrap();
    let mut candidate = SongDraft::titled("Other");
    candidate.aliases = vec!["♫".to_owned()];
    let possible = library.possible_existing_songs(&candidate);
    assert_eq!(ids(&possible), [song_id(3)]);
    assert_eq!(strengths(&possible), [MatchStrength::ExactAlias]);
}

#[test]
fn ac3_ties_order_by_title_key_then_identity_and_calls_are_repeatable() {
    let mut library = SongLibrary::new();
    library
        .create_song(song_id(3), at(0), SongDraft::titled("Ave Maria"))
        .unwrap();
    library
        .create_song(song_id(1), at(1), SongDraft::titled("ave  maria"))
        .unwrap();
    library
        .create_song(song_id(2), at(2), SongDraft::titled("Ave Maria!"))
        .unwrap();
    let before = library.clone();

    let possible = library.possible_existing_songs(&SongDraft::titled("Ave Maria"));
    // Songs 1 and 3 tie on strength and title key; identity breaks the tie.
    assert_eq!(ids(&possible), [song_id(1), song_id(3), song_id(2)]);
    assert_eq!(
        strengths(&possible),
        [
            MatchStrength::ExactTitle,
            MatchStrength::ExactTitle,
            MatchStrength::NearTitle,
        ]
    );
    assert_eq!(
        library.possible_existing_songs(&SongDraft::titled("Ave Maria")),
        possible,
        "repeating a call gives equal results"
    );
    assert_eq!(library, before, "ranking is read-only");
}

#[test]
fn ac3_identity_key_folds_case_spacing_diacritics_and_punctuation_only() {
    for (text, expected) in [
        ("Ginoo, Kaloy-i Kami", "ginookaloyikami"),
        ("ginoo kaloy i kami", "ginookaloyikami"),
        ("Ginoó Kaloyi Kami", "ginookaloyikami"),
        ("  Amazing \t Grace\n", "amazinggrace"),
        ("Kyrie I", "kyriei"),
        ("Kyrie II", "kyrieii"),
        ("Kyrie 1", "kyrie1"),
        ("Kyrie — 8th Mass", "kyrie8thmass"),
        ("Kyrie — 13th Mass", "kyrie13thmass"),
        ("Psalm 1-2", "psalm1 2"),
        ("Psalm 1 2", "psalm1 2"),
        ("Psalm 12", "psalm12"),
        ("Hymn 1, 3", "hymn1 3"),
        ("Hymn 13", "hymn13"),
        ("", ""),
        (" ?! — ... ", ""),
    ] {
        assert_eq!(identity_key(text), expected, "{text:?}");
    }
    assert_ne!(identity_key("Kyrie I"), identity_key("Kyrie II"));
    assert_ne!(
        identity_key("Kyrie I"),
        identity_key("Kyrie 1"),
        "roman and arabic numerals stay distinct"
    );
    assert_eq!(
        identity_key("Ginoo, Kaloy-i Kami"),
        identity_key("Ginoó Kaloyi Kami")
    );
}

#[test]
fn ac3_numbered_settings_never_join_across_separators() {
    assert_ne!(identity_key("Psalm 1-2"), identity_key("Psalm 12"));
    assert_ne!(identity_key("Hymn 1, 3"), identity_key("Hymn 13"));
    assert_eq!(identity_key("Psalm 1 2"), identity_key("Psalm 1-2"));
    assert_eq!(identity_key("kaloy-i"), identity_key("kaloy i"));
    assert_eq!(identity_key("kaloy i"), identity_key("kaloyi"));

    let mut library = SongLibrary::new();
    library
        .create_song(song_id(1), at(0), SongDraft::titled("Psalm 12"))
        .unwrap();
    library
        .create_song(song_id(2), at(1), SongDraft::titled("Psalm 1 2"))
        .unwrap();
    let possible = library.possible_existing_songs(&SongDraft::titled("Psalm 1-2"));
    assert_eq!(ids(&possible), [song_id(2)]);
    assert_eq!(strengths(&possible), [MatchStrength::NearTitle]);
}
