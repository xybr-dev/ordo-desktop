//! T2.10 WP2 acceptance tests for whole-document validation (AC9).
//!
//! Every test goes through the public Interface only. Test names start with
//! the acceptance criterion they cover.

use domain::identity::{ArrangementId, EntityId, SongId};
use domain::song_library::import::{
    DocumentProblem, ImportArrangement, ImportDocument, ImportSection, SequenceEntry,
    validate_document,
};
use domain::song_library::{Field, SongDraft};

fn entity(n: u8) -> EntityId {
    let mut bytes = [0_u8; 16];
    bytes[15] = n;
    EntityId::from_random_bytes(bytes)
}

fn arrangement(n: u8) -> ImportArrangement {
    ImportArrangement {
        id: ArrangementId::from_entity(entity(n)),
        title: None,
        languages: vec!["Cebuano".to_owned()],
        choirs: vec!["Koro".to_owned()],
        rendition_attribution: None,
        occasions: Vec::new(),
        notes: None,
        sections: vec![ImportSection {
            label: Some("Koro".to_owned()),
            lyrics: "Salamat".to_owned(),
        }],
        sequence: vec![SequenceEntry::Section(0)],
        warnings: Vec::new(),
    }
}

fn document(arrangements: Vec<ImportArrangement>) -> ImportDocument {
    ImportDocument {
        id: SongId::from_entity(entity(1)),
        song: SongDraft::titled("Salamat"),
        arrangements,
        warnings: Vec::new(),
    }
}

#[test]
fn ac9_any_invalid_arrangement_rejects_the_whole_document() {
    assert!(validate_document(&document(vec![arrangement(10)])).is_empty());

    let mut bad = arrangement(11);
    bad.title = Some("   ".to_owned());
    let problems = validate_document(&document(vec![arrangement(10), bad]));
    assert_eq!(
        problems,
        [DocumentProblem::BlankField {
            arrangement: Some(1),
            field: Field::ArrangementTitle
        }]
    );

    let mut song = document(vec![arrangement(10)]);
    song.song.title = " ".to_owned();
    assert_eq!(validate_document(&song), [DocumentProblem::BlankTitle]);

    let twice = document(vec![arrangement(10), arrangement(10)]);
    assert_eq!(
        validate_document(&twice),
        [DocumentProblem::DuplicateArrangement(
            ArrangementId::from_entity(entity(10))
        )]
    );

    let mut aliased = document(vec![arrangement(10)]);
    aliased.song.aliases = vec!["Salamat".to_owned()];
    assert_eq!(
        validate_document(&aliased),
        [DocumentProblem::Duplicate {
            arrangement: None,
            field: Field::Aliases
        }]
    );
}

#[test]
fn ac9_an_unlisted_language_is_a_document_problem() {
    let mut klingon = arrangement(10);
    klingon.languages = vec!["Klingon".to_owned()];
    assert_eq!(
        validate_document(&document(vec![klingon])),
        [DocumentProblem::UnlistedLanguage {
            arrangement: 0,
            value: "Klingon".to_owned()
        }]
    );
    // The document holds resolved display names; another name is not one.
    let mut other_name = arrangement(10);
    other_name.languages = vec!["Bisaya".to_owned()];
    assert_eq!(
        validate_document(&document(vec![other_name])),
        [DocumentProblem::UnlistedLanguage {
            arrangement: 0,
            value: "Bisaya".to_owned()
        }]
    );
    let mut none = arrangement(10);
    none.languages.clear();
    assert_eq!(
        validate_document(&document(vec![none])),
        [DocumentProblem::NoLanguage { arrangement: 0 }]
    );
}

#[test]
fn ac9_a_sequence_entry_outside_its_arrangement_is_a_document_problem() {
    let mut stray = arrangement(10);
    stray.sequence = vec![SequenceEntry::Section(0), SequenceEntry::Section(1)];
    assert_eq!(
        validate_document(&document(vec![stray])),
        [DocumentProblem::UnknownSection {
            arrangement: 0,
            position: 1
        }]
    );
}
