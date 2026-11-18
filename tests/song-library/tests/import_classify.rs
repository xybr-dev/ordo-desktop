//! T2.10 WP3 acceptance tests for import identity resolution, classification,
//! evidence and the plan (AC4, AC5, AC7, AC8 rules, AC10 rules).
//!
//! Every test goes through the public Interface only. Test names start with
//! the acceptance criterion they cover.

use std::collections::BTreeSet;

use domain::identity::{ArrangementId, ChoirId, EntityId, SectionId, SongId, Timestamp};
use domain::song_library::import::markdown::parse;
use domain::song_library::import::{
    ArrangementDifference, ArrangementEvidence, ArrangementReview, Choices, Classification,
    DocumentReview, ImportArrangement, ImportCommitted, ImportDocument, ImportPlan, ImportSection,
    InvalidReason, Outcome, PlanError, ReplacedArrangement, Review, SequenceEntry, SongChoice,
    SongEvidence, SongIdentity, Staged, StagedDocument, Tally, align_sections, can_commit,
    classify_batch, plan, plan_markdown, tally,
};
use domain::song_library::{
    ArchiveState, ArrangementDraft, ChoirDraft, FindingKind, HydrateArrangement, HydrateChoir,
    HydrateSong, OccurrenceDraft, PaginationState, ReadinessState, RecordRef, SectionDraft,
    SongDraft, SongLibrary, SongLibraryError, readiness,
};

fn eid(kind: u8, n: u8) -> EntityId {
    let mut bytes = [0_u8; 16];
    bytes[14] = kind;
    bytes[15] = n;
    EntityId::from_random_bytes(bytes)
}

fn song(n: u8) -> SongId {
    SongId::from_entity(eid(1, n))
}

fn arrangement_id(n: u8) -> ArrangementId {
    ArrangementId::from_entity(eid(2, n))
}

fn choir(n: u8) -> ChoirId {
    ChoirId::from_entity(eid(3, n))
}

fn section(n: u8) -> SectionId {
    SectionId::from_entity(eid(4, n))
}

fn at() -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000)
}

/// A mint of fresh identities that never collide with the fixtures.
fn minter() -> impl FnMut() -> EntityId {
    let mut next = 0_u16;
    move || {
        next += 1;
        let mut bytes = [0_u8; 16];
        bytes[0] = 0xee;
        bytes[12..14].copy_from_slice(&next.to_be_bytes());
        EntityId::from_random_bytes(bytes)
    }
}

const KORO: &str = "Salamat sa Dios\nAng among Ginoo";
const BERSIKULO: &str = "Ang Ginoo\nmaluloy-on";

fn local_draft(
    language: &str,
    sections: &[(u8, Option<&str>, &str)],
    sequence: &[u8],
) -> ArrangementDraft {
    let mut draft = ArrangementDraft::in_language(language);
    draft.sections = sections
        .iter()
        .map(|(n, label, lyrics)| match label {
            Some(label) => SectionDraft::labeled(section(*n), *label, *lyrics),
            None => SectionDraft::new(section(*n), *lyrics),
        })
        .collect();
    draft.sequence = sequence
        .iter()
        .enumerate()
        .map(|(index, n)| {
            OccurrenceDraft::new(
                domain::identity::OccurrenceId::from_entity(eid(
                    5,
                    n * 4 + u8::try_from(index).unwrap(),
                )),
                section(*n),
            )
        })
        .collect();
    draft
}

/// Choirs 1-4 (3 archived), Song 1 with Arrangements 10 (Cebuano, choir 1)
/// and 11 (English, choir 2), and Song 2 "Awit sa Pag-asa".
fn local() -> SongLibrary {
    let mut library = SongLibrary::new();
    let mut first = ChoirDraft::named("Koro San Roque");
    first.short_label = Some("KSR".to_owned());
    library.create_choir(choir(1), at(), first).unwrap();
    library
        .create_choir(choir(2), at(), ChoirDraft::named("Koro Sta. Cecilia"))
        .unwrap();
    library
        .create_choir(choir(3), at(), ChoirDraft::named("Koro Lumang"))
        .unwrap();
    library.archive_choir(choir(3), at()).unwrap();
    library
        .create_choir(choir(4), at(), ChoirDraft::named("Koro Bata"))
        .unwrap();

    let mut draft = SongDraft::titled("Salamat sa Dios");
    draft.aliases = vec!["Thanks to God".to_owned()];
    draft.original_attribution = Some("Fr. Reyes".to_owned());
    library.create_song(song(1), at(), draft).unwrap();
    library
        .create_arrangement(
            arrangement_id(10),
            song(1),
            at(),
            local_draft(
                "Cebuano",
                &[(1, Some("Koro"), KORO), (2, Some("Bersikulo 1"), BERSIKULO)],
                &[1, 2, 1],
            ),
            vec![choir(1)],
        )
        .unwrap();
    library
        .create_arrangement(
            arrangement_id(11),
            song(1),
            at(),
            local_draft("English", &[(3, Some("Chorus"), "Thanks be to God")], &[3]),
            vec![choir(2)],
        )
        .unwrap();
    library
        .create_song(song(2), at(), SongDraft::titled("Awit sa Pag-asa"))
        .unwrap();
    library
}

fn sections_of(items: &[(Option<&str>, &str)]) -> Vec<ImportSection> {
    items
        .iter()
        .map(|(label, lyrics)| ImportSection {
            label: label.map(str::to_owned),
            lyrics: (*lyrics).to_owned(),
        })
        .collect()
}

/// An imported copy of local Arrangement 10 under identity `n`.
fn copy_of_10(n: u8) -> ImportArrangement {
    ImportArrangement {
        id: arrangement_id(n),
        title: None,
        languages: vec!["Cebuano".to_owned()],
        choirs: vec!["Koro San Roque".to_owned()],
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: sections_of(&[(Some("Koro"), KORO), (Some("Bersikulo 1"), BERSIKULO)]),
        sequence: vec![
            SequenceEntry::Section(0),
            SequenceEntry::Section(1),
            SequenceEntry::Section(0),
        ],
        warnings: Vec::new(),
    }
}

/// An imported copy of local Arrangement 11 under identity `n`.
fn copy_of_11(n: u8) -> ImportArrangement {
    ImportArrangement {
        languages: vec!["English".to_owned()],
        choirs: vec!["Koro Sta. Cecilia".to_owned()],
        sections: sections_of(&[(Some("Chorus"), "Thanks be to God")]),
        sequence: vec![SequenceEntry::Section(0)],
        ..copy_of_10(n)
    }
}

fn document(song_n: u8, title: &str, arrangements: Vec<ImportArrangement>) -> ImportDocument {
    ImportDocument {
        id: song(song_n),
        song: SongDraft::titled(title),
        arrangements,
        warnings: Vec::new(),
    }
}

fn staged(name: &str, document: ImportDocument) -> StagedDocument {
    StagedDocument {
        source: name.to_owned(),
        content: Staged::Valid(document),
    }
}

fn valid(review: &Review) -> &DocumentReview {
    match review {
        Review::Valid(review) => review,
        Review::Invalid(reasons) => panic!("expected a valid review, got {reasons:?}"),
    }
}

fn classify_one(library: &SongLibrary, document: ImportDocument) -> DocumentReview {
    let reviews = classify_batch(library, &[staged("one.ordo", document)]);
    valid(&reviews[0]).clone()
}

fn choose(entries: &[((usize, usize), Outcome)]) -> Choices {
    Choices {
        arrangements: entries.iter().copied().collect(),
        ..Choices::default()
    }
}

fn make_plan(
    library: &SongLibrary,
    documents: &[StagedDocument],
    choices: &Choices,
) -> Result<ImportPlan, PlanError> {
    let reviews = classify_batch(library, documents);
    let mut mint = minter();
    plan(library, documents, &reviews, choices, &mut mint)
}

/// What storage would return for a plan, applied to the library.
fn commit(library: &mut SongLibrary, plan: &ImportPlan) {
    library.check_import(plan).unwrap();
    let committed = ImportCommitted {
        choirs: plan
            .choirs
            .iter()
            .map(|(id, draft)| HydrateChoir {
                id: *id,
                created_at: at(),
                draft: draft.clone(),
                archive_state: ArchiveState::Active,
            })
            .collect(),
        songs: plan
            .songs
            .iter()
            .map(|(id, draft)| HydrateSong {
                id: *id,
                created_at: at(),
                draft: draft.clone(),
                archive_state: ArchiveState::Active,
            })
            .collect(),
        arrangements: plan
            .arrangements
            .iter()
            .map(|planned| HydrateArrangement {
                id: planned.id,
                song: planned.song,
                created_at: at(),
                draft: planned.draft.clone(),
                choirs: planned.choirs.clone(),
                archive_state: ArchiveState::Active,
            })
            .collect(),
        replaced: plan
            .replacements
            .iter()
            .map(|replacement| ReplacedArrangement {
                id: replacement.id,
                draft: replacement.draft.clone(),
                choirs: replacement.choirs.clone(),
            })
            .collect(),
    };
    library.apply_import(committed).unwrap();
}

fn evidence_of(review: &ArrangementReview) -> &[ArrangementEvidence] {
    &review.evidence
}

// --- AC4 -------------------------------------------------------------------

#[test]
fn ac4_identity_comparison_folds_case_and_spacing() {
    let review = classify_one(
        &local(),
        document(50, "  SALAMAT   sa  dios ", vec![copy_of_10(60)]),
    );
    let SongIdentity::Existing {
        song: found,
        evidence,
    } = &review.identity
    else {
        panic!("expected an existing Song, got {:?}", review.identity);
    };
    assert_eq!(*found, song(1));
    assert!(
        evidence
            .iter()
            .any(|item| matches!(item, SongEvidence::Name { exact: true, .. }))
    );
}

#[test]
fn ac4_a_unique_exact_title_or_alias_attaches() {
    let library = local();
    for (title, aliases) in [
        ("Salamat sa Dios", vec![]),
        ("Another title", vec!["salamat sa dios"]),
        ("thanks to GOD", vec![]),
    ] {
        let mut doc = document(50, title, vec![copy_of_10(60)]);
        doc.song.aliases = aliases.into_iter().map(str::to_owned).collect();
        let review = classify_one(&library, doc);
        assert!(
            matches!(&review.identity, SongIdentity::Existing { song: s, .. } if *s == song(1)),
            "{title}: {:?}",
            review.identity
        );
    }
}

#[test]
fn ac4_near_only_matches_need_a_decision() {
    let review = classify_one(
        &local(),
        document(50, "Salamat, sa Dios!", vec![copy_of_10(60)]),
    );
    let SongIdentity::NeedsDecision {
        candidates,
        archived,
    } = &review.identity
    else {
        panic!("expected a decision, got {:?}", review.identity);
    };
    assert_eq!(archived, &None);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, song(1));
    assert!(
        candidates[0]
            .1
            .iter()
            .any(|item| matches!(item, SongEvidence::Name { exact: false, .. }))
    );
}

#[test]
fn ac4_no_candidate_adds_a_new_song() {
    let review = classify_one(&local(), document(50, "Bag-ong Awit", vec![copy_of_10(60)]));
    assert_eq!(review.identity, SongIdentity::New);
    assert_eq!(
        review.arrangements[0].classification,
        Classification::AddNewSong
    );
    assert_eq!(
        evidence_of(&review.arrangements[0])[0],
        ArrangementEvidence::CreatesSong { arrangements: 1 }
    );
}

#[test]
fn ac4_equal_content_is_skip_unchanged_whatever_the_pagination_states() {
    for state in [
        PaginationState::Draft,
        PaginationState::Approved,
        PaginationState::NeedsReview,
    ] {
        let mut library = local();
        let mut draft = local_draft(
            "Cebuano",
            &[(1, Some("Koro"), KORO), (2, Some("Bersikulo 1"), BERSIKULO)],
            &[1, 2, 1],
        );
        for item in &mut draft.sections {
            item.pagination = state;
        }
        library
            .refresh_arrangement(arrangement_id(10), draft, vec![choir(1)])
            .unwrap();
        let review = classify_one(
            &library,
            document(1, "Salamat sa Dios", vec![copy_of_10(60)]),
        );
        let first = &review.arrangements[0];
        assert_eq!(
            first.classification,
            Classification::SkipUnchanged,
            "{state:?}"
        );
        assert_eq!(first.nearest, Some(arrangement_id(10)));
        assert!(!first.same_id);
        assert!(first.evidence.contains(&ArrangementEvidence::Identical));
        assert_eq!(first.recommended, Some(Outcome::Skip));
    }
}

#[test]
fn ac4_a_resembling_changed_arrangement_needs_a_decision_with_differences() {
    let mut changed = copy_of_10(60);
    changed.sections = sections_of(&[
        (Some("Koro"), KORO),
        (Some("Bersikulo 1"), "Ang Ginoo\nmaluloy-on kanato"),
        (Some("Bersikulo 2"), "Dugang nga pulong"),
    ]);
    changed.sequence = vec![
        SequenceEntry::Section(0),
        SequenceEntry::Section(1),
        SequenceEntry::Section(0),
        SequenceEntry::Section(2),
    ];
    let review = classify_one(&local(), document(1, "Salamat sa Dios", vec![changed]));
    let first = &review.arrangements[0];
    assert_eq!(first.classification, Classification::NeedsDecision);
    assert_eq!(first.nearest, Some(arrangement_id(10)));
    assert_eq!(first.recommended, Some(Outcome::AddAsNew));
    assert_eq!(
        first.evidence,
        [ArrangementEvidence::Resembles(vec![
            ArrangementDifference::SectionReworded("Bersikulo 1".to_owned()),
            ArrangementDifference::SectionAdded("Bersikulo 2".to_owned()),
            ArrangementDifference::SequenceChanged,
        ])]
    );
}

#[test]
fn ac4_slide_break_and_detail_differences_are_named() {
    let mut changed = copy_of_10(60);
    changed.sections[1].lyrics = "Ang Ginoo\n\nmaluloy-on".to_owned();
    changed.notes = Some("From the binder".to_owned());
    let review = classify_one(&local(), document(1, "Salamat sa Dios", vec![changed]));
    assert_eq!(
        review.arrangements[0].evidence,
        [ArrangementEvidence::Resembles(vec![
            ArrangementDifference::Details(vec![domain::song_library::import::DetailField::Notes]),
            ArrangementDifference::SlideBreaks("Bersikulo 1".to_owned()),
        ])]
    );
}

#[test]
fn ac4_a_new_choir_or_language_attaches_with_its_reason() {
    let library = local();
    let mut tagalog = copy_of_10(60);
    tagalog.languages = vec!["Tagalog".to_owned()];
    tagalog.choirs = vec!["Koro Bag-o".to_owned()];
    let review = classify_one(&library, document(1, "Salamat sa Dios", vec![tagalog]));
    let first = &review.arrangements[0];
    assert_eq!(first.classification, Classification::AttachArrangement);
    for expected in [
        ArrangementEvidence::NewLanguage("Tagalog".to_owned()),
        ArrangementEvidence::NoArrangementForChoir("Koro Bag-o".to_owned()),
        ArrangementEvidence::AddsChoir("Koro Bag-o".to_owned()),
    ] {
        assert!(first.evidence.contains(&expected), "{expected:?}");
    }

    // Same language, an active Choir that has no Arrangement of the Song, and
    // nothing else shared: a new Arrangement for that Choir.
    let mut other_choir = copy_of_10(61);
    other_choir.choirs = vec!["Koro Bata".to_owned()];
    other_choir.sections = sections_of(&[(Some("Koro"), "Iba nga pulong")]);
    other_choir.sequence = vec![SequenceEntry::Section(0)];
    let review = classify_one(&library, document(1, "Salamat sa Dios", vec![other_choir]));
    let first = &review.arrangements[0];
    assert_eq!(first.classification, Classification::AttachArrangement);
    assert_eq!(
        first.evidence,
        [ArrangementEvidence::NoArrangementForChoir(
            "Koro Bata".to_owned()
        )]
    );
}

#[test]
fn ac4_every_classification_carries_evidence() {
    let library = local();
    let mut changed = copy_of_10(10);
    changed.sections[0].lyrics = "Bag-o".to_owned();
    let mut warned = copy_of_10(63);
    warned.languages = vec!["Tagalog".to_owned()];
    warned.warnings = vec!["Slide 2 was blurry".to_owned()];
    let batch = [
        staged(
            "a.ordo",
            document(1, "Salamat sa Dios", vec![copy_of_10(60), changed, warned]),
        ),
        staged("b.ordo", document(70, "Bag-ong Awit", vec![copy_of_10(71)])),
        staged(
            "c.ordo",
            document(1, "Salamat sa Dios", vec![copy_of_11(72)]),
        ),
    ];
    let reviews = classify_batch(&library, &batch);
    let mut seen = BTreeSet::new();
    for review in &reviews {
        let review = valid(review);
        if let SongIdentity::Existing { evidence, .. } = &review.identity {
            assert!(!evidence.is_empty());
        }
        for arrangement in &review.arrangements {
            assert!(!arrangement.evidence.is_empty(), "{arrangement:?}");
            seen.insert(format!("{:?}", arrangement.classification));
        }
    }
    assert_eq!(seen.len(), 4, "{seen:?}");
    assert!(valid(&reviews[0]).arrangements[2].evidence.contains(
        &ArrangementEvidence::FileWarning("Slide 2 was blurry".to_owned())
    ));
}

#[test]
fn ac4_numbered_settings_never_match_each_other() {
    let mut library = local();
    library
        .create_song(song(8), at(), SongDraft::titled("Kyrie \u{2014} 8th Mass"))
        .unwrap();
    let review = classify_one(
        &library,
        document(50, "Kyrie \u{2014} 13th Mass", vec![copy_of_10(60)]),
    );
    assert_eq!(review.identity, SongIdentity::New);
    let review = classify_one(
        &library,
        document(51, "kyrie \u{2014}  8TH mass", vec![copy_of_10(61)]),
    );
    assert!(matches!(&review.identity, SongIdentity::Existing { song: s, .. } if *s == song(8)));
}

// --- AC5 -------------------------------------------------------------------

#[test]
fn ac5_song_identity_resolves_once_per_document() {
    let mut second = copy_of_10(61);
    second.languages = vec!["Tagalog".to_owned()];
    let reviews = classify_batch(
        &local(),
        &[staged(
            "a.ordo",
            document(
                50,
                "Salamat sa Dios",
                vec![copy_of_10(60), second, copy_of_11(62)],
            ),
        )],
    );
    let review = valid(&reviews[0]);
    assert!(matches!(&review.identity, SongIdentity::Existing { song: s, .. } if *s == song(1)));
    assert_eq!(review.arrangements.len(), 3);
    assert!(
        review
            .arrangements
            .iter()
            .all(|a| a.classification != Classification::NeedsDecision)
    );
}

#[test]
fn ac5_a_document_id_wins_over_names() {
    let review = classify_one(
        &local(),
        document(1, "Awit sa Pag-asa", vec![copy_of_10(60)]),
    );
    let SongIdentity::Existing {
        song: found,
        evidence,
    } = &review.identity
    else {
        panic!("{:?}", review.identity);
    };
    assert_eq!(*found, song(1));
    assert_eq!(evidence[0], SongEvidence::SameId);
    assert!(evidence.contains(&SongEvidence::FileTitle("Awit sa Pag-asa".to_owned())));
}

#[test]
fn ac5_arrangement_ids_owned_by_two_songs_need_a_decision() {
    let mut library = local();
    library
        .create_arrangement(
            arrangement_id(20),
            song(2),
            at(),
            local_draft("Cebuano", &[(8, Some("Koro"), "Pag-asa")], &[8]),
            vec![],
        )
        .unwrap();
    let review = classify_one(
        &library,
        document(50, "Bag-ong Awit", vec![copy_of_10(10), copy_of_10(20)]),
    );
    let SongIdentity::NeedsDecision {
        candidates,
        archived,
    } = &review.identity
    else {
        panic!("{:?}", review.identity);
    };
    assert_eq!(archived, &None);
    assert_eq!(
        candidates.iter().map(|(song, _)| *song).collect::<Vec<_>>(),
        [song(1), song(2)]
    );
    assert!(
        candidates
            .iter()
            .all(|(_, evidence)| evidence.contains(&SongEvidence::SameArrangementId))
    );
}

#[test]
fn ac5_no_choice_distributes_a_documents_arrangements_across_songs() {
    let library = local();
    let mut arrangements = vec![copy_of_10(60), copy_of_10(61), copy_of_11(62)];
    arrangements[1].languages = vec!["Tagalog".to_owned()];
    arrangements[1].sections[0].lyrics = "Lahi".to_owned();
    let documents = [staged("a.ordo", document(50, "Bag-ong Awit", arrangements))];
    let reviews = classify_batch(&library, &documents);

    let songs = [
        None,
        Some(SongChoice::New),
        Some(SongChoice::Existing(song(1))),
        Some(SongChoice::Existing(song(2))),
        Some(SongChoice::Skip),
    ];
    let outcomes = [
        None,
        Some(Outcome::Skip),
        Some(Outcome::AddNewSong),
        Some(Outcome::Attach),
        Some(Outcome::AddAsNew),
    ];
    let mut planned = 0;
    for song_choice in songs {
        for first in outcomes {
            for second in outcomes {
                for third in outcomes {
                    let mut choices = Choices::default();
                    if let Some(choice) = song_choice {
                        choices.songs.insert(0, choice);
                    }
                    for (index, outcome) in [first, second, third].into_iter().enumerate() {
                        if let Some(outcome) = outcome {
                            choices.arrangements.insert((0, index), outcome);
                        }
                    }
                    let mut mint = minter();
                    let Ok(result) = plan(&library, &documents, &reviews, &choices, &mut mint)
                    else {
                        continue;
                    };
                    planned += 1;
                    let parents: BTreeSet<SongId> =
                        result.arrangements.iter().map(|a| a.song).collect();
                    assert!(parents.len() <= 1, "{choices:?} -> {parents:?}");
                    assert!(result.songs.len() <= 1);
                    library.check_import(&result).unwrap();
                }
            }
        }
    }
    assert!(planned > 20, "only {planned} combinations planned");
}

#[test]
fn ac5_an_archived_song_offers_only_new_or_skip() {
    let mut library = local();
    library.archive_song(song(2), at()).unwrap();
    let documents = [staged(
        "a.ordo",
        document(2, "Awit sa Pag-asa", vec![copy_of_10(60)]),
    )];
    let reviews = classify_batch(&library, &documents);
    assert_eq!(
        valid(&reviews[0]).identity,
        SongIdentity::NeedsDecision {
            candidates: Vec::new(),
            archived: Some(song(2)),
        }
    );
    assert!(!can_commit(&reviews, &Choices::default()));
    let mut choices = Choices::default();
    choices.songs.insert(0, SongChoice::Existing(song(2)));
    let mut mint = minter();
    assert_eq!(
        plan(&library, &documents, &reviews, &choices, &mut mint),
        Err(PlanError::SongUnavailable(song(2)))
    );
    // A new Song cannot reuse the archived Song's identity.
    choices.songs.insert(0, SongChoice::New);
    let result = plan(&library, &documents, &reviews, &choices, &mut mint).unwrap();
    assert_eq!(result.songs.len(), 1);
    assert_ne!(result.songs[0].0, song(2));
    assert_eq!(result.arrangements[0].song, result.songs[0].0);
    library.check_import(&result).unwrap();
    choices.songs.insert(0, SongChoice::Skip);
    assert!(
        plan(&library, &documents, &reviews, &choices, &mut mint)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ac5_a_later_document_attaches_to_a_song_an_earlier_one_adds() {
    let library = local();
    let mut later = copy_of_10(61);
    later.languages = vec!["Tagalog".to_owned()];
    let documents = [
        staged("a.ordo", document(50, "Bag-ong Awit", vec![copy_of_10(60)])),
        staged("b.ordo", document(50, "Ibang Ngalan", vec![later])),
    ];
    let reviews = classify_batch(&library, &documents);
    assert_eq!(valid(&reviews[0]).identity, SongIdentity::New);
    assert_eq!(
        valid(&reviews[1]).identity,
        SongIdentity::StagedBy { document: 0 }
    );
    assert_eq!(
        valid(&reviews[1]).arrangements[0].classification,
        Classification::AttachArrangement
    );
    let mut mint = minter();
    let result = plan(
        &library,
        &documents,
        &reviews,
        &Choices::default(),
        &mut mint,
    )
    .unwrap();
    assert_eq!(result.songs.len(), 1);
    assert_eq!(result.songs[0].1.title, "Bag-ong Awit");
    assert_eq!(result.arrangements.len(), 2);
    assert!(result.arrangements.iter().all(|a| a.song == song(50)));
    library.check_import(&result).unwrap();
}

// --- AC7 -------------------------------------------------------------------

fn changed_10() -> ImportArrangement {
    let mut changed = copy_of_10(10);
    changed.sections[1].lyrics = "Bag-ong pulong".to_owned();
    changed
}

#[test]
fn ac7_same_id_changed_recommends_update() {
    let review = classify_one(&local(), document(1, "Salamat sa Dios", vec![changed_10()]));
    let first = &review.arrangements[0];
    assert_eq!(first.classification, Classification::NeedsDecision);
    assert!(first.same_id);
    assert_eq!(first.nearest, Some(arrangement_id(10)));
    assert_eq!(first.recommended, Some(Outcome::UpdateExisting));
    assert_eq!(
        first.offered,
        [Outcome::UpdateExisting, Outcome::AddAsNew, Outcome::Skip]
    );
    assert_eq!(first.evidence[0], ArrangementEvidence::SameId);
}

#[test]
fn ac7_resembling_without_same_id_recommends_add_as_new() {
    let mut changed = changed_10();
    changed.id = arrangement_id(61);
    let review = classify_one(&local(), document(1, "Salamat sa Dios", vec![changed]));
    let first = &review.arrangements[0];
    assert!(!first.same_id);
    assert_eq!(first.recommended, Some(Outcome::AddAsNew));
    assert!(first.offered.contains(&Outcome::UpdateExisting));
}

#[test]
fn ac7_update_is_not_offered_over_an_archived_arrangement() {
    let mut library = local();
    library
        .archive_arrangement(arrangement_id(10), at())
        .unwrap();
    let documents = [staged(
        "a.ordo",
        document(1, "Salamat sa Dios", vec![changed_10()]),
    )];
    let reviews = classify_batch(&library, &documents);
    let first = &valid(&reviews[0]).arrangements[0];
    assert_eq!(first.offered, [Outcome::AddAsNew, Outcome::Skip]);
    assert!(
        first
            .evidence
            .contains(&ArrangementEvidence::ExistingArchived)
    );
    let mut mint = minter();
    assert_eq!(
        plan(
            &library,
            &documents,
            &reviews,
            &choose(&[((0, 0), Outcome::UpdateExisting)]),
            &mut mint
        ),
        Err(PlanError::NotOffered {
            document: 0,
            arrangement: 0
        })
    );
}

#[test]
fn ac7_update_plans_the_whole_imported_content() {
    let library = local();
    let mut changed = changed_10();
    changed.title = Some("Salamat (bag-o)".to_owned());
    changed.notes = Some("Revised".to_owned());
    changed.sections.push(ImportSection {
        label: Some("Koda".to_owned()),
        lyrics: "Amen".to_owned(),
    });
    changed.sequence = vec![SequenceEntry::Section(1), SequenceEntry::Section(2)];
    let documents = [staged(
        "a.ordo",
        document(1, "Salamat sa Dios", vec![changed.clone()]),
    )];
    let result = make_plan(
        &library,
        &documents,
        &choose(&[((0, 0), Outcome::UpdateExisting)]),
    )
    .unwrap();
    assert!(result.arrangements.is_empty() && result.songs.is_empty());
    let [replacement] = result.replacements.as_slice() else {
        panic!("{result:?}");
    };
    assert_eq!(replacement.id, arrangement_id(10));
    assert_eq!(replacement.choirs, [choir(1)]);
    let draft = &replacement.draft;
    assert_eq!(draft.title, changed.title);
    assert_eq!(draft.notes, changed.notes);
    assert_eq!(
        draft
            .sections
            .iter()
            .map(|s| (s.label.clone(), s.lyrics.clone(), s.pagination))
            .collect::<Vec<_>>(),
        changed
            .sections
            .iter()
            .map(|s| (
                s.label.clone(),
                s.lyrics.clone(),
                PaginationState::NeedsReview
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        draft.sequence.iter().map(|o| o.section).collect::<Vec<_>>(),
        [draft.sections[1].id, draft.sections[2].id]
    );
    library.check_import(&result).unwrap();
}

#[test]
fn ac7_update_aligns_section_ids_by_label_then_position() {
    let existing = local_draft(
        "Cebuano",
        &[
            (1, Some("Koro"), "a"),
            (2, Some("Bersikulo 1"), "b"),
            (3, None, "c"),
            (4, None, "d"),
        ],
        &[1],
    );
    let mut library = SongLibrary::new();
    library
        .create_song(song(1), at(), SongDraft::titled("T"))
        .unwrap();
    library
        .create_arrangement(arrangement_id(10), song(1), at(), existing, vec![])
        .unwrap();
    let stored = library
        .arrangement(arrangement_id(10))
        .unwrap()
        .sections()
        .to_vec();
    let imported = sections_of(&[
        (None, "x"),
        (Some("  BERSIKULO   1 "), "y"),
        (Some("Bersikulo 9"), "z"),
        (None, "w"),
        (Some("Koro"), "k"),
        (None, "extra"),
    ]);
    let mut mint = minter();
    let ids = align_sections(&imported, &stored, &mut mint);
    assert_eq!(ids[0], section(3));
    assert_eq!(ids[1], section(2));
    assert!(
        !stored.iter().any(|s| s.id() == ids[2]),
        "a new label gets a new id"
    );
    assert_eq!(ids[3], section(4));
    assert_eq!(ids[4], section(1));
    assert!(
        !stored.iter().any(|s| s.id() == ids[5]),
        "a third unlabeled gets a new id"
    );
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
}

// --- AC8 -------------------------------------------------------------------

#[test]
fn ac8_open_decisions_block_commit() {
    let library = local();
    let documents = [
        staged("a.ordo", document(1, "Salamat sa Dios", vec![changed_10()])),
        staged(
            "b.ordo",
            document(2, "Awit sa Pag-asa", vec![copy_of_10(61)]),
        ),
    ];
    let reviews = classify_batch(&library, &documents);
    let none = Choices::default();
    assert!(!can_commit(&reviews, &none));
    assert_eq!(tally(&reviews, &none).open, 1);
    let mut mint = minter();
    assert_eq!(
        plan(&library, &documents, &reviews, &none, &mut mint),
        Err(PlanError::OpenDecision {
            document: 0,
            arrangement: Some(0)
        })
    );
    let decided = choose(&[((0, 0), Outcome::UpdateExisting)]);
    assert!(can_commit(&reviews, &decided));
    assert!(plan(&library, &documents, &reviews, &decided, &mut mint).is_ok());

    // An open Song decision blocks too, and counts once.
    let near = [staged(
        "c.ordo",
        document(
            50,
            "Salamat, sa Dios!",
            vec![copy_of_10(60), copy_of_11(62)],
        ),
    )];
    let reviews = classify_batch(&library, &near);
    assert_eq!(tally(&reviews, &Choices::default()).open, 1);
    assert!(!can_commit(&reviews, &Choices::default()));
    let mut chosen = Choices::default();
    chosen.songs.insert(0, SongChoice::New);
    assert_eq!(
        plan(&library, &near, &reviews, &Choices::default(), &mut mint),
        Err(PlanError::OpenDecision {
            document: 0,
            arrangement: None
        })
    );
    assert!(plan(&library, &near, &reviews, &chosen, &mut mint).is_ok());
    // Nothing to write is not committable either.
    assert!(!can_commit(&[], &Choices::default()));
}

#[test]
fn ac8_invalid_documents_never_enter_the_plan() {
    let library = local();
    let mut blank = document(51, "  ", vec![copy_of_10(61)]);
    blank.song.title = "  ".to_owned();
    let documents = [
        StagedDocument {
            source: "broken.ordo".to_owned(),
            content: Staged::Invalid(vec![InvalidReason {
                code: "schema".to_owned(),
                message: "/id is missing".to_owned(),
            }]),
        },
        staged("blank.ordo", blank),
        staged(
            "good.ordo",
            document(52, "Bag-ong Awit", vec![copy_of_10(62)]),
        ),
    ];
    let reviews = classify_batch(&library, &documents);
    assert!(matches!(&reviews[0], Review::Invalid(r) if r[0].code == "schema"));
    assert!(matches!(&reviews[1], Review::Invalid(_)));
    let choices = Choices::default();
    assert_eq!(tally(&reviews, &choices).invalid, 2);
    let mut mint = minter();
    let result = plan(&library, &documents, &reviews, &choices, &mut mint).unwrap();
    assert_eq!(result.songs.len(), 1);
    assert_eq!(result.songs[0].0, song(52));
    assert_eq!(result.arrangements.len(), 1);
    assert!(can_commit(&reviews, &choices));
}

#[test]
fn ac8_any_suggested_action_can_be_changed() {
    let library = local();
    // An attach becomes a skip.
    let mut tagalog = copy_of_10(60);
    tagalog.languages = vec!["Tagalog".to_owned()];
    let attach = [staged(
        "a.ordo",
        document(1, "Salamat sa Dios", vec![tagalog]),
    )];
    assert_eq!(
        make_plan(&library, &attach, &Choices::default())
            .unwrap()
            .arrangements
            .len(),
        1
    );
    let skipped = make_plan(&library, &attach, &choose(&[((0, 0), Outcome::Skip)])).unwrap();
    assert!(skipped.is_empty());

    // An unchanged Arrangement can be added anyway; its taken identity is
    // replaced by a fresh one (Decision 3).
    let unchanged = [staged(
        "b.ordo",
        document(1, "Salamat sa Dios", vec![copy_of_10(10)]),
    )];
    assert!(
        make_plan(&library, &unchanged, &Choices::default())
            .unwrap()
            .is_empty()
    );
    let added = make_plan(
        &library,
        &unchanged,
        &choose(&[((0, 0), Outcome::AddAsNew)]),
    )
    .unwrap();
    assert_eq!(added.arrangements.len(), 1);
    assert_ne!(added.arrangements[0].id, arrangement_id(10));
    library.check_import(&added).unwrap();

    // A suggested Song becomes a new Song, or the document is skipped.
    let suggested = [staged(
        "c.ordo",
        document(50, "Salamat sa Dios", vec![copy_of_10(61)]),
    )];
    let mut choices = Choices::default();
    choices.songs.insert(0, SongChoice::New);
    let new_song = make_plan(&library, &suggested, &choices).unwrap();
    assert_eq!(new_song.songs[0].0, song(50));
    assert_eq!(new_song.arrangements[0].song, song(50));
    choices.songs.insert(0, SongChoice::Existing(song(2)));
    let elsewhere = make_plan(&library, &suggested, &choices).unwrap();
    assert!(elsewhere.songs.is_empty());
    assert_eq!(elsewhere.arrangements[0].song, song(2));
    choices.songs.insert(0, SongChoice::Skip);
    assert!(
        make_plan(&library, &suggested, &choices)
            .unwrap()
            .is_empty()
    );

    // An outcome the review does not offer is refused.
    assert_eq!(
        make_plan(
            &library,
            &attach,
            &choose(&[((0, 0), Outcome::UpdateExisting)])
        ),
        Err(PlanError::NotOffered {
            document: 0,
            arrangement: 0
        })
    );
}

#[test]
fn ac8_tally_counts_each_outcome() {
    let library = local();
    let mut first = copy_of_10(60);
    first.choirs = vec!["Koro Bag-o".to_owned()];
    let mut second = copy_of_10(61);
    second.choirs = vec!["koro  BAG-O".to_owned()];
    let mut tagalog = copy_of_10(62);
    tagalog.languages = vec!["Tagalog".to_owned()];
    let documents = [
        StagedDocument {
            source: "broken.ordo".to_owned(),
            content: Staged::Invalid(Vec::new()),
        },
        staged(
            "new.ordo",
            document(70, "Bag-ong Awit", vec![first, second]),
        ),
        staged(
            "old.ordo",
            document(
                1,
                "Salamat sa Dios",
                vec![changed_10(), tagalog, copy_of_11(63)],
            ),
        ),
    ];
    let reviews = classify_batch(&library, &documents);
    let choices = choose(&[((1, 1), Outcome::Skip), ((2, 0), Outcome::UpdateExisting)]);
    assert_eq!(
        tally(&reviews, &choices),
        Tally {
            new_songs: 1,
            new_arrangements: 2,
            updates: 1,
            skipped: 2,
            invalid: 1,
            new_choirs: 1,
            open: 0,
        }
    );
    // Skipping a whole document counts its Arrangements as skipped.
    let mut skip_new = choices.clone();
    skip_new.songs.insert(1, SongChoice::Skip);
    let counted = tally(&reviews, &skip_new);
    assert_eq!(
        (counted.new_songs, counted.new_choirs, counted.skipped),
        (0, 0, 3)
    );
}

// --- AC10 ------------------------------------------------------------------

#[test]
fn ac10_a_plan_after_committing_the_same_documents_is_empty() {
    let mut library = local();
    let mut added = copy_of_10(61);
    added.choirs = vec![
        "Koro Bag-o".to_owned(),
        "KSR".to_owned(),
        "Koro Lumang".to_owned(),
    ];
    let mut updated = changed_10();
    updated.notes = Some("Revised".to_owned());
    let documents = [
        staged(
            "a.ordo",
            document(1, "Salamat sa Dios", vec![updated, copy_of_11(62)]),
        ),
        staged(
            "b.ordo",
            document(70, "Bag-ong Awit", vec![added, copy_of_11(64)]),
        ),
    ];
    let choices = choose(&[((0, 0), Outcome::UpdateExisting)]);
    let first = make_plan(&library, &documents, &choices).unwrap();
    assert!(!first.is_empty());
    commit(&mut library, &first);

    let reviews = classify_batch(&library, &documents);
    for review in &reviews {
        let review = valid(review);
        assert!(
            matches!(review.identity, SongIdentity::Existing { .. }),
            "{:?}",
            review.identity
        );
        assert!(
            review
                .arrangements
                .iter()
                .all(|a| a.classification == Classification::SkipUnchanged),
            "{review:?}"
        );
    }
    let mut mint = minter();
    let again = plan(
        &library,
        &documents,
        &reviews,
        &Choices::default(),
        &mut mint,
    )
    .unwrap();
    assert!(again.is_empty(), "{again:?}");
    assert!(!can_commit(&reviews, &Choices::default()));
}

// --- Others ----------------------------------------------------------------

#[test]
fn imported_sections_plan_needs_review_and_markdown_sections_plan_draft() {
    let library = local();
    let documents = [staged(
        "a.ordo",
        document(50, "Bag-ong Awit", vec![copy_of_10(60)]),
    )];
    let result = make_plan(&library, &documents, &Choices::default()).unwrap();
    assert!(
        result.arrangements[0]
            .draft
            .sections
            .iter()
            .all(|s| s.pagination == PaginationState::NeedsReview)
    );
    let draft = parse(
        b"---\ntitle: Bag-ong Awit\nlanguage: Cebuano\n---\n[Koro]\nSalamat\n",
        (song(50), arrangement_id(60)),
    );
    let mut mint = minter();
    let result = plan_markdown(&library, &draft, SongChoice::New, &mut mint).unwrap();
    assert_eq!(result.songs[0].0, song(50));
    assert!(
        result.arrangements[0]
            .draft
            .sections
            .iter()
            .all(|s| s.pagination == PaginationState::Draft)
    );
    library.check_import(&result).unwrap();
}

#[test]
fn unmatched_choir_names_plan_new_choirs_and_archived_ones_are_dropped() {
    let library = local();
    let mut first = copy_of_10(60);
    first.choirs = vec![
        "ksr".to_owned(),
        "Koro Lumang".to_owned(),
        "Koro  Bag-o".to_owned(),
        "koro sta. cecilia".to_owned(),
    ];
    let mut second = copy_of_10(61);
    second.languages = vec!["Tagalog".to_owned()];
    second.choirs = vec!["koro bag-o".to_owned()];
    let documents = [staged(
        "a.ordo",
        document(1, "Salamat sa Dios", vec![first, second]),
    )];
    let reviews = classify_batch(&library, &documents);
    assert!(valid(&reviews[0]).arrangements[0].evidence.contains(
        &ArrangementEvidence::ArchivedChoir("Koro Lumang".to_owned())
    ));
    let result = make_plan(
        &library,
        &documents,
        &choose(&[((0, 0), Outcome::AddAsNew)]),
    )
    .unwrap();
    assert_eq!(result.choirs.len(), 1, "one Choir for both spellings");
    assert_eq!(result.choirs[0].1.name, "Koro  Bag-o");
    let created = result.choirs[0].0;
    assert_eq!(result.arrangements[0].choirs, [choir(1), created, choir(2)]);
    assert_eq!(result.arrangements[1].choirs, [created]);
    library.check_import(&result).unwrap();
}

#[test]
fn existing_song_details_are_never_planned() {
    let mut library = local();
    let mut doc = document(1, "Bread of Life", vec![copy_of_10(10)]);
    doc.song.aliases = vec!["Thanks to God".to_owned(), "Tinapay".to_owned()];
    doc.song.original_attribution = Some("Someone Else".to_owned());
    doc.song.provenance = Some("A binder".to_owned());
    let documents = [staged("a.ordo", doc)];
    let reviews = classify_batch(&library, &documents);
    let SongIdentity::Existing { evidence, .. } = &valid(&reviews[0]).identity else {
        panic!();
    };
    assert!(evidence.contains(&SongEvidence::FileTitle("Bread of Life".to_owned())));
    assert!(evidence.contains(&SongEvidence::AlsoCalled("Tinapay".to_owned())));
    assert!(!evidence.contains(&SongEvidence::AlsoCalled("Thanks to God".to_owned())));

    let mut tagalog = copy_of_10(60);
    tagalog.languages = vec!["Tagalog".to_owned()];
    let documents = [staged(
        "b.ordo",
        ImportDocument {
            arrangements: vec![tagalog],
            ..document(1, "Bread of Life", vec![])
        },
    )];
    let before = library.song(song(1)).unwrap().clone();
    let result = make_plan(&library, &documents, &Choices::default()).unwrap();
    assert!(result.songs.is_empty());
    commit(&mut library, &result);
    assert_eq!(library.song(song(1)).unwrap(), &before);
}

#[test]
fn a_repeated_arrangement_id_in_a_batch_is_invalid_for_the_later_document() {
    let reviews = classify_batch(
        &local(),
        &[
            staged(
                "first.ordo",
                document(50, "Bag-ong Awit", vec![copy_of_10(60)]),
            ),
            staged(
                "second.ordo",
                document(51, "Ibang Awit", vec![copy_of_10(60)]),
            ),
        ],
    );
    assert!(matches!(&reviews[0], Review::Valid(_)));
    let Review::Invalid(reasons) = &reviews[1] else {
        panic!("{:?}", reviews[1]);
    };
    assert_eq!(reasons[0].code, "duplicate-arrangement-id");
    assert_eq!(
        reasons[0].message,
        format!("Arrangement {} is also in first.ordo.", arrangement_id(60))
    );
}

#[test]
fn markdown_unresolved_repeats_plan_placeholder_sections_that_read_incomplete() {
    let mut library = local();
    let draft = parse(
        b"---\ntitle: Bag-ong Awit\nlanguage: Cebuano\n---\n[Koro]\nSalamat\n[Repeat: Koda]\n[Repeat: koda]\n",
        (song(50), arrangement_id(60)),
    );
    assert!(draft.can_save(), "{:?}", draft.issues);
    let mut mint = minter();
    let result = plan_markdown(&library, &draft, SongChoice::New, &mut mint).unwrap();
    let planned = &result.arrangements[0].draft;
    assert_eq!(
        planned.sections.len(),
        2,
        "one placeholder per distinct label"
    );
    assert_eq!(planned.sections[1].label.as_deref(), Some("Koda"));
    assert_eq!(planned.sections[1].lyrics, "");
    assert_eq!(planned.sequence.len(), 3);
    commit(&mut library, &result);
    let stored = library.arrangement(arrangement_id(60)).unwrap();
    let report = readiness(&stored.lyrics(), &BTreeSet::new());
    assert_eq!(report.state, ReadinessState::Incomplete);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::NoLyrics)
    );
}

#[test]
fn markdown_with_a_blocking_issue_is_not_planned() {
    let draft = parse(b"no front matter", (song(50), arrangement_id(60)));
    let mut mint = minter();
    assert_eq!(
        plan_markdown(&local(), &draft, SongChoice::New, &mut mint),
        Err(PlanError::NotSavable)
    );
}

#[test]
fn check_import_refuses_taken_ids_and_archived_targets() {
    let library = local();
    let documents = [staged(
        "a.ordo",
        document(50, "Bag-ong Awit", vec![copy_of_10(60)]),
    )];
    let good = make_plan(&library, &documents, &Choices::default()).unwrap();
    assert_eq!(library.check_import(&good), Ok(()));
    let unchanged = library.clone();

    let mut taken = good.clone();
    taken.arrangements[0].id = arrangement_id(10);
    assert_eq!(
        library.check_import(&taken),
        Err(SongLibraryError::DuplicateArrangement(arrangement_id(10)))
    );

    let mut song_taken = good.clone();
    song_taken.songs[0].0 = song(1);
    song_taken.arrangements[0].song = song(1);
    assert_eq!(
        library.check_import(&song_taken),
        Err(SongLibraryError::DuplicateSong(song(1)))
    );

    let mut section_taken = good.clone();
    section_taken.arrangements[0].draft.sections[0].id = section(1);
    assert!(matches!(
        library.check_import(&section_taken),
        Err(SongLibraryError::DuplicateSection(_))
    ));

    let mut unlisted = good.clone();
    unlisted.arrangements[0].draft.languages = vec!["Klingon".to_owned()];
    assert_eq!(
        library.check_import(&unlisted),
        Err(SongLibraryError::UnlistedLanguage("Klingon".to_owned()))
    );

    let mut archived = local();
    archived.archive_song(song(2), at()).unwrap();
    let mut onto_archived = good.clone();
    onto_archived.songs.clear();
    onto_archived.arrangements[0].song = song(2);
    assert_eq!(
        archived.check_import(&onto_archived),
        Err(SongLibraryError::Archived(RecordRef::Song(song(2))))
    );

    let mut archived_choir = good.clone();
    archived_choir.arrangements[0].choirs = vec![choir(3)];
    assert_eq!(
        library.check_import(&archived_choir),
        Err(SongLibraryError::Archived(RecordRef::Choir(choir(3))))
    );

    let mut gone = good;
    gone.replacements
        .push(domain::song_library::import::PlannedReplacement {
            id: arrangement_id(99),
            draft: ArrangementDraft::in_language("Cebuano"),
            choirs: Vec::new(),
        });
    assert_eq!(
        library.check_import(&gone),
        Err(SongLibraryError::UnknownArrangement(arrangement_id(99)))
    );
    let mut archived_target = local();
    archived_target
        .archive_arrangement(arrangement_id(10), at())
        .unwrap();
    let mut replace = ImportPlan::default();
    replace
        .replacements
        .push(domain::song_library::import::PlannedReplacement {
            id: arrangement_id(10),
            draft: ArrangementDraft::in_language("Cebuano"),
            choirs: Vec::new(),
        });
    assert_eq!(
        archived_target.check_import(&replace),
        Err(SongLibraryError::Archived(RecordRef::Arrangement(
            arrangement_id(10)
        )))
    );
    assert_eq!(library, unchanged, "checking never changes the library");
}

#[test]
fn apply_import_is_all_or_nothing() {
    let mut library = local();
    let before = library.clone();
    let committed = ImportCommitted {
        choirs: vec![HydrateChoir {
            id: choir(9),
            created_at: at(),
            draft: ChoirDraft::named("Koro Bag-o"),
            archive_state: ArchiveState::Active,
        }],
        songs: Vec::new(),
        arrangements: vec![HydrateArrangement {
            id: arrangement_id(10),
            song: song(1),
            created_at: at(),
            draft: ArrangementDraft::in_language("Cebuano"),
            choirs: vec![choir(9)],
            archive_state: ArchiveState::Active,
        }],
        replaced: Vec::new(),
    };
    assert_eq!(
        library.apply_import(committed),
        Err(SongLibraryError::DuplicateArrangement(arrangement_id(10)))
    );
    assert_eq!(library, before);
}
