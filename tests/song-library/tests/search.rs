//! T2.4 WP2 acceptance tests for Song Library search, filters and evidence.
//!
//! Every test goes through the public Interface only. Test names start with
//! the acceptance criterion they cover.

use std::collections::BTreeSet;
use std::time::Instant;

use domain::identity::{ArrangementId, ChoirId, EntityId, SectionId, SongId, Timestamp};
use domain::song_library::{
    ArrangementDraft, AttributionField, ChoirDraft, Evidence, FilterValue, LibraryQuery,
    SectionDraft, SongDraft, SongHit, SongLibrary, search, search_key,
};

fn entity(n: u32) -> EntityId {
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&n.to_be_bytes());
    bytes[15] = 1;
    EntityId::from_random_bytes(bytes)
}

fn song_id(n: u32) -> SongId {
    SongId::from_entity(entity(n))
}

fn arrangement_id(n: u32) -> ArrangementId {
    ArrangementId::from_entity(entity(n))
}

fn choir_id(n: u32) -> ChoirId {
    ChoirId::from_entity(entity(n))
}

fn section_id(n: u32) -> SectionId {
    SectionId::from_entity(entity(n))
}

fn at(offset: i64) -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000 + offset)
}

fn query(text: &str) -> LibraryQuery {
    LibraryQuery {
        text: text.to_owned(),
        ..LibraryQuery::default()
    }
}

fn songs(hits: &[SongHit]) -> Vec<SongId> {
    hits.iter().map(|hit| hit.song).collect()
}

/// An Arrangement draft with one section per lyric string; section ids start
/// at `base`.
fn draft(title: &str, language: Option<&str>, lyrics: &[&str], base: u32) -> ArrangementDraft {
    let mut draft = ArrangementDraft::titled(title);
    draft.languages = language.map(str::to_owned).into_iter().collect();
    draft.sections = lyrics
        .iter()
        .zip(base..)
        .map(|(text, id)| SectionDraft::new(section_id(id), *text))
        .collect();
    draft
}

fn add_song(library: &mut SongLibrary, n: u32, title: &str) {
    library
        .create_song(song_id(n), at(0), SongDraft::titled(title))
        .unwrap();
}

fn add_arrangement(
    library: &mut SongLibrary,
    n: u32,
    song: u32,
    draft: ArrangementDraft,
    choirs: Vec<ChoirId>,
) {
    library
        .create_arrangement(
            arrangement_id(n),
            song_id(song),
            at(i64::from(n)),
            draft,
            choirs,
        )
        .unwrap();
}

#[test]
fn ac4_each_field_kind_is_found_case_and_diacritic_insensitively() {
    let mut library = SongLibrary::new();
    library
        .create_choir(choir_id(900), at(0), ChoirDraft::named("Koro Señora"))
        .unwrap();
    let mut song = SongDraft::titled("Pagdayeg Niño");
    song.aliases = vec!["Alabado Sí".to_owned()];
    song.original_attribution = Some("Fr. José Cruz".to_owned());
    song.provenance = Some("Hymnal Cebú 1950".to_owned());
    song.rights_notes = Some("Licensed by Zanzibar Press".to_owned());
    library.create_song(song_id(1), at(0), song).unwrap();
    let mut arrangement = draft(
        "Himno Español",
        Some("Cebuano"),
        &["Unang linya\nAng Ginoo mao ang akong Magbalantay"],
        100,
    );
    arrangement.rendition_attribution = Some("Arr. Zoë Bañez".to_owned());
    add_arrangement(&mut library, 10, 1, arrangement, vec![choir_id(900)]);

    let cases: [(&str, Evidence); 7] = [
        ("PAGDAYEG NINO", Evidence::Title),
        (
            "alabado si",
            Evidence::Alias {
                text: "Alabado Sí".to_owned(),
            },
        ),
        (
            "jose cruz",
            Evidence::Attribution {
                field: AttributionField::Original,
                text: "Fr. José Cruz".to_owned(),
            },
        ),
        (
            "CEBU 1950",
            Evidence::Attribution {
                field: AttributionField::Provenance,
                text: "Hymnal Cebú 1950".to_owned(),
            },
        ),
        (
            "zoe banez",
            Evidence::Attribution {
                field: AttributionField::Rendition {
                    arrangement: arrangement_id(10),
                },
                text: "Arr. Zoë Bañez".to_owned(),
            },
        ),
        (
            "espanol",
            Evidence::ArrangementTitle {
                arrangement: arrangement_id(10),
                text: "Himno Español".to_owned(),
            },
        ),
        (
            "SENORA",
            Evidence::Choir {
                arrangement: arrangement_id(10),
                choir: choir_id(900),
                name: "Koro Señora".to_owned(),
            },
        ),
    ];
    for (text, expected) in cases {
        let hits = search(&library, &query(text));
        assert_eq!(songs(&hits), vec![song_id(1)], "query {text:?}");
        assert!(
            hits[0].evidence.contains(&expected),
            "query {text:?} gave {:?}",
            hits[0].evidence
        );
    }

    let hits = search(&library, &query("cebuano"));
    assert_eq!(
        hits[0].evidence,
        vec![Evidence::Language {
            arrangement: arrangement_id(10),
            text: "Cebuano".to_owned(),
        }]
    );

    // The whole matching lyric line, verbatim, with its section.
    let hits = search(&library, &query("magbalantay"));
    assert_eq!(
        hits[0].evidence,
        vec![Evidence::Lyric {
            arrangement: arrangement_id(10),
            section: section_id(100),
            line: "Ang Ginoo mao ang akong Magbalantay".to_owned(),
        }]
    );

    // Private rights notes are not discovery metadata.
    assert!(search(&library, &query("zanzibar")).is_empty());
}

#[test]
fn ac4_search_key_folds_case_whitespace_and_latin_diacritics_only() {
    assert_eq!(search_key("  ÁÉÍ   Ñandú\tŁódź "), "aei nandu lodz");
    assert_eq!(search_key("Straße Œuvre Þor"), "strasse oeuvre thor");
    assert_eq!(search_key("Kyrie 8th"), "kyrie 8th");
    assert_ne!(search_key("Kyrie 8th"), search_key("Kyrie 13th"));
    // NFD input folds too: combining marks are dropped.
    assert_eq!(search_key("Nin\u{303}o"), "nino");
}

#[test]
fn ac4_empty_text_matches_every_song_without_evidence() {
    let mut library = SongLibrary::new();
    add_song(&mut library, 1, "Beta");
    add_song(&mut library, 2, "alpha");
    for text in ["", "   "] {
        let hits = search(&library, &query(text));
        assert_eq!(songs(&hits), vec![song_id(2), song_id(1)]);
        assert!(hits.iter().all(|hit| hit.evidence.is_empty()));
    }
}

#[test]
fn ac4_lyric_evidence_is_per_line_capped_at_three_per_song() {
    let mut library = SongLibrary::new();
    add_song(&mut library, 1, "Hymn");
    add_arrangement(
        &mut library,
        10,
        1,
        draft(
            "A",
            None,
            &["amen one\nplain\namen two", "amen three\namen four"],
            100,
        ),
        Vec::new(),
    );
    let hits = search(&library, &query("AMEN"));
    let lines: Vec<&str> = hits[0]
        .evidence
        .iter()
        .map(|evidence| match evidence {
            Evidence::Lyric { line, .. } => line.as_str(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(lines, ["amen one", "amen two", "amen three"]);
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(10)]);
}

/// Song 1 has a Cebuano/Youth Arrangement (10) and an English/Adult
/// Arrangement (11); Song 2 has one Cebuano/Adult Arrangement (20); Song 3
/// has no Arrangement; Song 4 has one Arrangement with no language, no Choir.
fn filter_library() -> SongLibrary {
    let mut library = SongLibrary::new();
    library
        .create_choir(choir_id(900), at(0), ChoirDraft::named("Youth"))
        .unwrap();
    library
        .create_choir(choir_id(901), at(0), ChoirDraft::named("Adult"))
        .unwrap();
    for (n, title) in [(1, "One"), (2, "Two"), (3, "Three"), (4, "Four")] {
        add_song(&mut library, n, title);
    }
    add_arrangement(
        &mut library,
        10,
        1,
        draft("Cebuano", Some("ceb"), &["x"], 100),
        vec![choir_id(900)],
    );
    let mut english = draft("English", Some("en"), &["y"], 110);
    english.occasions = vec!["Advent".to_owned()];
    add_arrangement(&mut library, 11, 1, english, vec![choir_id(901)]);
    add_arrangement(
        &mut library,
        20,
        2,
        draft("Cebuano", Some("ceb"), &["z"], 120),
        vec![choir_id(901)],
    );
    add_arrangement(
        &mut library,
        40,
        4,
        draft("Bare", None, &["w"], 130),
        Vec::new(),
    );
    library
}

fn filtered(
    choirs: &[FilterValue<ChoirId>],
    languages: &[FilterValue<&str>],
    occasions: &[FilterValue<&str>],
) -> LibraryQuery {
    let own = |values: &[FilterValue<&str>]| -> BTreeSet<FilterValue<String>> {
        values
            .iter()
            .map(|value| match value {
                FilterValue::Is(text) => FilterValue::Is((*text).to_owned()),
                FilterValue::Unspecified => FilterValue::Unspecified,
            })
            .collect()
    };
    LibraryQuery {
        choirs: choirs.iter().cloned().collect(),
        languages: own(languages),
        occasions: own(occasions),
        ..LibraryQuery::default()
    }
}

#[test]
fn ac5_attributes_of_separate_arrangements_are_never_combined() {
    let library = filter_library();
    // Song 1 has Cebuano (Youth) and English (Adult) but nothing both Cebuano
    // and Adult, so only Song 2 is retained.
    let hits = search(
        &library,
        &filtered(
            &[FilterValue::Is(choir_id(901))],
            &[FilterValue::Is("ceb")],
            &[],
        ),
    );
    assert_eq!(songs(&hits), vec![song_id(2)]);
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(20)]);

    // Cebuano and Youth together are met by one Arrangement of Song 1.
    let hits = search(
        &library,
        &filtered(
            &[FilterValue::Is(choir_id(900))],
            &[FilterValue::Is("CEB")],
            &[],
        ),
    );
    assert_eq!(songs(&hits), vec![song_id(1)]);
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(10)]);
    assert_eq!(hits[0].total_arrangements, 2);
}

#[test]
fn ac5_values_within_one_kind_are_ored() {
    let library = filter_library();
    let hits = search(
        &library,
        &filtered(
            &[
                FilterValue::Is(choir_id(900)),
                FilterValue::Is(choir_id(901)),
            ],
            &[],
            &[],
        ),
    );
    assert_eq!(songs(&hits), vec![song_id(1), song_id(2)]);
    assert_eq!(
        hits[0].matching_arrangements,
        vec![arrangement_id(10), arrangement_id(11)]
    );
}

#[test]
fn ac5_unspecified_matches_only_an_arrangement_with_none_of_that_kind() {
    let library = filter_library();
    // No Choir: only Song 4's bare Arrangement.
    let hits = search(&library, &filtered(&[FilterValue::Unspecified], &[], &[]));
    assert_eq!(songs(&hits), vec![song_id(4)]);

    // No language: Song 4 again; Unspecified is not a wildcard.
    let hits = search(&library, &filtered(&[], &[FilterValue::Unspecified], &[]));
    assert_eq!(songs(&hits), vec![song_id(4)]);

    // No occasion: Song 1's Cebuano Arrangement, Song 2 and Song 4, but not
    // Song 1's English Advent Arrangement.
    let hits = search(&library, &filtered(&[], &[], &[FilterValue::Unspecified]));
    assert_eq!(songs(&hits), vec![song_id(4), song_id(1), song_id(2)]);
    let one = hits.iter().find(|hit| hit.song == song_id(1)).unwrap();
    assert_eq!(one.matching_arrangements, vec![arrangement_id(10)]);

    // Unspecified OR a value inside one kind.
    let hits = search(
        &library,
        &filtered(&[], &[FilterValue::Unspecified, FilterValue::Is("en")], &[]),
    );
    assert_eq!(songs(&hits), vec![song_id(4), song_id(1)]);

    // The occasion filter reads the Arrangement's own tags.
    let hits = search(&library, &filtered(&[], &[], &[FilterValue::Is("advent")]));
    assert_eq!(songs(&hits), vec![song_id(1)]);
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(11)]);
}

#[test]
fn ac5_a_song_with_no_arrangement_needs_no_filter_to_be_retained() {
    let library = filter_library();
    let all = search(&library, &LibraryQuery::default());
    assert!(songs(&all).contains(&song_id(3)));
    let three = all.iter().find(|hit| hit.song == song_id(3)).unwrap();
    assert!(three.matching_arrangements.is_empty());
    assert_eq!(three.total_arrangements, 0);

    for filter in [
        filtered(&[FilterValue::Unspecified], &[], &[]),
        filtered(&[], &[FilterValue::Unspecified], &[]),
        filtered(&[], &[], &[FilterValue::Unspecified]),
    ] {
        assert!(!songs(&search(&library, &filter)).contains(&song_id(3)));
    }
}

#[test]
fn ac5_text_and_filters_apply_to_the_same_arrangement() {
    let mut library = filter_library();
    // Text matches Arrangement 11's title only; the Choir filter selects
    // Arrangement 10, so Song 1 is not retained.
    let mut q = filtered(&[FilterValue::Is(choir_id(900))], &[], &[]);
    q.text = "english".to_owned();
    assert!(search(&library, &q).is_empty());

    // With the Adult filter the text and filter meet in Arrangement 11.
    let mut q = filtered(&[FilterValue::Is(choir_id(901))], &[], &[]);
    q.text = "english".to_owned();
    let hits = search(&library, &q);
    assert_eq!(songs(&hits), vec![song_id(1)]);
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(11)]);

    // A Song-level text match keeps every filter-passing Arrangement.
    let mut q = filtered(&[FilterValue::Is(choir_id(901))], &[], &[]);
    q.text = "one".to_owned();
    let hits = search(&library, &q);
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(11)]);
    assert_eq!(hits[0].evidence, vec![Evidence::Title]);

    // Choir evidence reports the current Choir name.
    library
        .edit_choir(choir_id(901), ChoirDraft::named("Grown-ups"))
        .unwrap();
    let hits = search(&library, &query("grown"));
    assert!(hits[0].evidence.contains(&Evidence::Choir {
        arrangement: arrangement_id(11),
        choir: choir_id(901),
        name: "Grown-ups".to_owned(),
    }));
}

#[test]
fn ac6_alias_and_lyric_evidence_with_matching_and_total_counts() {
    let mut library = SongLibrary::new();
    let mut song = SongDraft::titled("Kyrie");
    song.aliases = vec!["Lord have mercy".to_owned()];
    library.create_song(song_id(1), at(0), song).unwrap();
    add_arrangement(
        &mut library,
        10,
        1,
        draft("Latin", Some("la"), &["Kyrie eleison"], 100),
        Vec::new(),
    );
    add_arrangement(
        &mut library,
        11,
        1,
        draft(
            "English",
            Some("en"),
            &["Lord, have mercy\nChrist, have mercy"],
            110,
        ),
        Vec::new(),
    );
    let before = library.clone();

    let hits = search(&library, &query("have mercy"));
    assert_eq!(songs(&hits), vec![song_id(1)]);
    assert_eq!(
        hits[0].evidence,
        vec![
            Evidence::Alias {
                text: "Lord have mercy".to_owned()
            },
            Evidence::Lyric {
                arrangement: arrangement_id(11),
                section: section_id(110),
                line: "Lord, have mercy".to_owned(),
            },
            Evidence::Lyric {
                arrangement: arrangement_id(11),
                section: section_id(110),
                line: "Christ, have mercy".to_owned(),
            },
        ]
    );
    // The alias is a Song-level match, so both Arrangements match.
    assert_eq!(
        hits[0].matching_arrangements,
        vec![arrangement_id(10), arrangement_id(11)]
    );
    assert_eq!(hits[0].total_arrangements, 2);

    // A lyric-only match narrows matching to the Arrangement it came from.
    let hits = search(&library, &query("eleison"));
    assert_eq!(hits[0].matching_arrangements, vec![arrangement_id(10)]);
    assert_eq!(hits[0].total_arrangements, 2);

    assert_eq!(library, before, "search never changes authored text");
}

#[test]
fn ac9_archived_songs_and_arrangements_are_excluded_unless_included() {
    let mut library = SongLibrary::new();
    for (n, title) in [(1, "Live"), (2, "Shelved"), (3, "Partial")] {
        add_song(&mut library, n, title);
    }
    add_arrangement(
        &mut library,
        10,
        1,
        draft("x", None, &["v"], 100),
        Vec::new(),
    );
    add_arrangement(
        &mut library,
        20,
        2,
        draft("x", None, &["v"], 110),
        Vec::new(),
    );
    add_arrangement(
        &mut library,
        30,
        3,
        draft("x", None, &["v"], 120),
        Vec::new(),
    );
    add_arrangement(
        &mut library,
        31,
        3,
        draft("x", None, &["v"], 130),
        Vec::new(),
    );
    library.archive_song(song_id(2), at(100)).unwrap();
    library
        .archive_arrangement(arrangement_id(31), at(100))
        .unwrap();
    let before = library.clone();

    // Default: the archived Song is gone; the archived Arrangement is neither
    // counted nor matched; the suppressed child is not reachable.
    let hits = search(&library, &LibraryQuery::default());
    assert_eq!(songs(&hits), vec![song_id(1), song_id(3)]);
    let partial = &hits[1];
    assert_eq!(partial.total_arrangements, 1);
    assert_eq!(partial.matching_arrangements, vec![arrangement_id(30)]);
    assert!(search(&library, &query("shelved")).is_empty());

    // An Arrangement-level filter cannot reach the archived Arrangement.
    let mut only_archived = query("");
    only_archived.languages.insert(FilterValue::Unspecified);
    assert_eq!(search(&library, &only_archived).len(), 2);

    // Included: everything is counted and matched.
    let all = LibraryQuery {
        include_archived: true,
        ..LibraryQuery::default()
    };
    let hits = search(&library, &all);
    assert_eq!(songs(&hits), vec![song_id(1), song_id(3), song_id(2)]);
    let by_song = |song: SongId| hits.iter().find(|hit| hit.song == song).unwrap();
    assert_eq!(by_song(song_id(3)).total_arrangements, 2);
    assert_eq!(
        by_song(song_id(3)).matching_arrangements,
        vec![arrangement_id(30), arrangement_id(31)]
    );
    assert_eq!(by_song(song_id(2)).total_arrangements, 1);
    assert_eq!(
        by_song(song_id(2)).matching_arrangements,
        vec![arrangement_id(20)]
    );

    assert_eq!(library, before, "search never changes archive state");
}

#[test]
fn ranking_puts_title_then_alias_then_other_evidence_each_by_title_then_id() {
    let mut library = SongLibrary::new();
    // Other evidence only (lyric), title "Aaa".
    add_song(&mut library, 1, "Aaa");
    add_arrangement(
        &mut library,
        10,
        1,
        draft("x", None, &["glory be"], 100),
        Vec::new(),
    );
    // Alias only, title "Bbb".
    let mut alias = SongDraft::titled("Bbb");
    alias.aliases = vec!["Glory".to_owned()];
    library.create_song(song_id(2), at(0), alias).unwrap();
    // Title hits: "Zeta Glory" and "Éclat glory" (key "eclat glory"), plus a
    // twin with the same key to exercise the id tie-break.
    add_song(&mut library, 3, "Zeta Glory");
    add_song(&mut library, 4, "Éclat glory");
    add_song(&mut library, 5, "eclat Glory");

    let hits = search(&library, &query("glory"));
    assert_eq!(
        songs(&hits),
        vec![song_id(4), song_id(5), song_id(3), song_id(2), song_id(1)]
    );
}

/// Performance smoke: 2,000 Songs, 2 Arrangements each, 4 sections each.
/// Runs well inside the 50 ms budget in debug builds on the development
/// machine, so it is not ignored.
#[test]
fn performance_smoke_two_thousand_songs_lyric_query_under_50ms() {
    let mut library = SongLibrary::new();
    for s in 0..2_000_u32 {
        let song = 10 * s + 1_000_000;
        library
            .create_song(song_id(song), at(0), SongDraft::titled(format!("Song {s}")))
            .unwrap();
        for a in 0..2_u32 {
            let base = song + 1 + a * 100_000_000;
            let lyrics: Vec<String> = (0..4)
                .map(|k| format!("line one of {s} {a} {k}\nthe Lord is my shepherd {s}"))
                .collect();
            let refs: Vec<&str> = lyrics.iter().map(String::as_str).collect();
            library
                .create_arrangement(
                    arrangement_id(base),
                    song_id(song),
                    at(i64::from(a)),
                    draft(&format!("Arr {a}"), Some("en"), &refs, base * 10),
                    Vec::new(),
                )
                .unwrap();
        }
    }
    let started = Instant::now();
    let hits = search(&library, &query("SHEPHERD 1999"));
    let elapsed = started.elapsed();
    println!("search over 2000 songs took {elapsed:?}");
    assert_eq!(hits.len(), 1);
    assert!(elapsed.as_millis() < 50, "search took {elapsed:?}");
}
