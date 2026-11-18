//! T2.10 WP2 acceptance tests for the Markdown song parser (AC2).
//!
//! Every test goes through the public Interface only. Test names start with
//! the acceptance criterion they cover.

use std::fs;
use std::path::PathBuf;

use domain::identity::{ArrangementId, EntityId, SongId};
use domain::song_library::import::markdown::{IssueKind, MarkdownDraft, parse};
use domain::song_library::import::{
    ImportDocument, SequenceEntry, rendition_attribution, resolve_language,
};
use serde_json::{Value, json};

fn ids() -> (SongId, ArrangementId) {
    let entity = |last: u8| {
        let mut bytes = [0_u8; 16];
        bytes[15] = last;
        EntityId::from_random_bytes(bytes)
    };
    (
        SongId::from_entity(entity(1)),
        ArrangementId::from_entity(entity(2)),
    )
}

fn draft(text: &str) -> MarkdownDraft {
    parse(text.as_bytes(), ids())
}

/// A draft with the required front matter, then `body`.
fn with_body(body: &str) -> MarkdownDraft {
    draft(&format!("---\ntitle: Song\nlanguage: English\n---\n{body}"))
}

fn sections(draft: &MarkdownDraft) -> Vec<(Option<&str>, &str)> {
    draft.document.arrangements[0]
        .sections
        .iter()
        .map(|section| (section.label.as_deref(), section.lyrics.as_str()))
        .collect()
}

fn kinds(draft: &MarkdownDraft) -> Vec<(usize, IssueKind, bool)> {
    draft
        .issues
        .iter()
        .map(|issue| (issue.line, issue.kind.clone(), issue.blocking))
        .collect()
}

#[test]
fn ac2_front_matter_maps_every_listed_key() {
    let draft = draft(
        "---\n\
         title: \"Salamat, Ginoo\"\n\
         aliases: [Thanksgiving Hymn, 'Salamat Ginoo']\n\
         composer: Sample Composer   # who wrote it\n\
         source: Parish binder 2019\n\
         rights: Used with permission\n\
         arrangement: Tagalog - Adult Choir\n\
         language: Bisaya\n\
         choirs:\n  - Adult Choir\n  - Youth Choir\n\
         translator: Maria Santos\n\
         adapter: Jose Cruz\n\
         liturgical_moments: [Offertory]\n\
         occasions: [Easter, Ordinary Time]\n\
         notes: From the binder\n\
         ---\n[Koro]\nSalamat\n",
    );
    assert!(draft.issues.is_empty(), "{:?}", draft.issues);
    let song = &draft.document.song;
    assert_eq!(song.title, "Salamat, Ginoo");
    assert_eq!(song.aliases, ["Thanksgiving Hymn", "Salamat Ginoo"]);
    assert_eq!(
        song.original_attribution.as_deref(),
        Some("Sample Composer")
    );
    assert_eq!(song.provenance.as_deref(), Some("Parish binder 2019"));
    assert_eq!(song.rights_notes.as_deref(), Some("Used with permission"));
    assert_eq!(song.liturgical_moments, ["Offertory"]);
    assert!(song.occasions.is_empty());
    let arrangement = &draft.document.arrangements[0];
    assert_eq!(arrangement.title.as_deref(), Some("Tagalog - Adult Choir"));
    assert_eq!(arrangement.languages, ["Cebuano"]);
    assert_eq!(arrangement.choirs, ["Adult Choir", "Youth Choir"]);
    assert_eq!(arrangement.occasions, ["Easter", "Ordinary Time"]);
    assert_eq!(arrangement.notes.as_deref(), Some("From the binder"));
    assert_eq!(arrangement.id, ids().1);
    assert_eq!(draft.document.id, ids().0);
    assert!(draft.can_save());
}

#[test]
fn ac2_title_is_required() {
    let missing = draft("---\nlanguage: English\n---\n[Koro]\nWords\n");
    assert_eq!(kinds(&missing), [(1, IssueKind::MissingTitle, true)]);
    assert!(!missing.can_save());

    let blank = draft("---\nlanguage: English\ntitle:   \n---\n");
    assert!(
        blank.issues.iter().any(|issue| issue.line == 3
            && issue.kind == IssueKind::MissingTitle
            && issue.blocking)
    );
    assert!(!blank.can_save());

    let none = draft("[Koro]\nWords\n");
    assert_eq!(kinds(&none), [(1, IssueKind::NoFrontMatter, true)]);
    assert!(!none.can_save());
    // The words still show in the preview.
    assert_eq!(sections(&none), [(Some("Koro"), "Words")]);
}

#[test]
fn ac2_unknown_keys_are_ignored_with_a_note() {
    let draft = draft("---\ntitle: Song\nlanguage: English\ncolour: blue\n---\n[Koro]\nWords\n");
    assert_eq!(
        kinds(&draft),
        [(4, IssueKind::UnknownKey("colour".to_owned()), false)]
    );
    assert!(draft.can_save());
}

#[test]
fn ac2_bracket_labels_start_sections_and_persist_across_slides() {
    let draft = with_body("[Verse]\nA\n\nB\n\n[Chorus]\nC\n");
    assert_eq!(
        sections(&draft),
        [(Some("Verse"), "A\n\nB"), (Some("Chorus"), "C")]
    );
    assert!(draft.issues.is_empty());
}

#[test]
fn ac2_a_blank_line_is_a_hard_slide_boundary_and_runs_are_one() {
    let draft = with_body("\n\n[Verse]\n\nA1\nA2\n\n   \n\t\nB\n\n\n");
    assert_eq!(sections(&draft), [(Some("Verse"), "A1\nA2\n\nB")]);
}

#[test]
fn ac2_unlabeled_starts_label_free_content() {
    let draft = with_body("[Koro]\nA\n[Unlabeled]\nB\n");
    assert_eq!(sections(&draft), [(Some("Koro"), "A"), (None, "B")]);
    assert_eq!(
        draft.document.arrangements[0].sequence,
        [SequenceEntry::Section(0), SequenceEntry::Section(1)]
    );
}

#[test]
fn ac2_text_before_any_label_is_unlabeled() {
    let draft = with_body("Opening words\n\nsecond slide\n[Koro]\nA\n");
    assert_eq!(
        sections(&draft),
        [(None, "Opening words\n\nsecond slide"), (Some("Koro"), "A")]
    );
}

#[test]
fn ac2_repeat_resolves_forward_and_backward_case_insensitively_after_trimming() {
    let draft = with_body("[Verse 1]\nA\n[Repeat: chorus]\n[Chorus]\nB\n[Repeat:  VERSE 1 ]\n");
    assert!(draft.issues.is_empty(), "{:?}", draft.issues);
    assert_eq!(
        draft.document.arrangements[0].sequence,
        [
            SequenceEntry::Section(0),
            SequenceEntry::Section(1),
            SequenceEntry::Section(1),
            SequenceEntry::Section(0),
        ]
    );
}

#[test]
fn ac2_sequence_is_document_order_with_repeats_in_place() {
    let draft = with_body("[A]\n1\n[B]\n2\n[Repeat: A]\n[C]\n3\n[Repeat: B]\n[Repeat: B]\n");
    let order: Vec<usize> = draft.document.arrangements[0]
        .sequence
        .iter()
        .map(|entry| match entry {
            SequenceEntry::Section(slot) => *slot,
            SequenceEntry::Unresolved { .. } => usize::MAX,
        })
        .collect();
    assert_eq!(order, [0, 1, 0, 2, 1, 1]);
    // Text after a repeat starts a new unlabeled section.
    let after = with_body("[A]\n1\n[Repeat: A]\nlate words\n");
    assert_eq!(sections(&after), [(Some("A"), "1"), (None, "late words")]);
    assert_eq!(
        after.document.arrangements[0].sequence,
        [
            SequenceEntry::Section(0),
            SequenceEntry::Section(0),
            SequenceEntry::Section(1)
        ]
    );
}

#[test]
fn ac2_unresolved_and_ambiguous_repeats_are_non_blocking_issues_on_their_line() {
    // Body starts on line 5.
    let draft = with_body("[Verse]\nOne\n[Repeat: Bridge]\n[Verse]\nTwo\n[Repeat: verse]\n");
    assert_eq!(
        kinds(&draft),
        [
            (7, IssueKind::UnresolvedRepeat("Bridge".to_owned()), false),
            (
                10,
                IssueKind::AmbiguousRepeat {
                    label: "verse".to_owned(),
                    sections: 2
                },
                false
            ),
        ]
    );
    assert!(draft.can_save(), "an unresolved repeat stays saveable");
    assert_eq!(
        draft.document.arrangements[0].sequence[1],
        SequenceEntry::Unresolved {
            label: "Bridge".to_owned(),
            line: 7,
            candidates: 0
        }
    );
    assert_eq!(
        draft.document.arrangements[0].sequence[3],
        SequenceEntry::Unresolved {
            label: "verse".to_owned(),
            line: 10,
            candidates: 2
        }
    );
}

#[test]
fn ac2_crlf_and_bom_parse_like_lf() {
    let lf = "---\ntitle: Song\nlanguage: English\n---\n[Verse]\nA\n\nB\n[Repeat: verse]\n";
    let crlf = format!("\u{feff}{}", lf.replace('\n', "\r\n"));
    assert_eq!(draft(lf), draft(&crlf));
    assert_eq!(sections(&draft(&crlf)), [(Some("Verse"), "A\n\nB")]);
}

#[test]
fn ac2_non_utf8_is_blocking() {
    let draft = parse(&[0x2d, 0xff, 0xfe, 0x0a], ids());
    assert_eq!(kinds(&draft), [(1, IssueKind::NotUtf8, true)]);
    assert!(!draft.can_save());
}

#[test]
fn ac2_translator_and_adapter_join() {
    assert_eq!(rendition_attribution(Some("T"), None).as_deref(), Some("T"));
    assert_eq!(rendition_attribution(None, Some("A")).as_deref(), Some("A"));
    assert_eq!(
        rendition_attribution(Some("T"), Some("A")).as_deref(),
        Some("T; A")
    );
    assert_eq!(rendition_attribution(Some(" "), None), None);
    let both = draft("---\ntitle: S\nlanguage: English\ntranslator: Maria\nadapter: Jose\n---\n");
    assert_eq!(
        both.document.arrangements[0]
            .rendition_attribution
            .as_deref(),
        Some("Maria; Jose")
    );
}

#[test]
fn ac2_language_matches_names_and_other_names_exactly() {
    assert_eq!(resolve_language("Bisaya"), Some("Cebuano"));
    assert_eq!(resolve_language("  cebuano "), Some("Cebuano"));
    assert_eq!(resolve_language("ILONGGO"), Some("Hiligaynon"));
    assert_eq!(resolve_language("Ceb"), None);
    assert_eq!(resolve_language(""), None);

    let draft = draft("---\ntitle: S\nlanguage: [Bisaya, Ceb, Cebuano]\n---\n");
    assert_eq!(draft.document.arrangements[0].languages, ["Cebuano"]);
    assert_eq!(
        kinds(&draft),
        [
            (3, IssueKind::UnlistedLanguage("Ceb".to_owned()), true),
            (5, IssueKind::NoWords(None), false),
        ]
    );
    assert!(!draft.can_save());

    let none = draft_without_language();
    assert_eq!(kinds(&none), [(1, IssueKind::NoLanguage, true)]);
}

fn draft_without_language() -> MarkdownDraft {
    draft("---\ntitle: S\n---\n[Koro]\nWords\n")
}

fn render(draft: &MarkdownDraft) -> Value {
    let document: &ImportDocument = &draft.document;
    let arrangement = &document.arrangements[0];
    json!({
        "title": document.song.title,
        "aliases": document.song.aliases,
        "composer": document.song.original_attribution,
        "source": document.song.provenance,
        "rights": document.song.rights_notes,
        "liturgical_moments": document.song.liturgical_moments,
        "arrangement": {
            "title": arrangement.title,
            "languages": arrangement.languages,
            "choirs": arrangement.choirs,
            "rendition_attribution": arrangement.rendition_attribution,
            "occasions": arrangement.occasions,
            "notes": arrangement.notes,
        },
        "sections": arrangement.sections.iter().map(|section| json!({
            "label": section.label,
            "slides": if section.lyrics.is_empty() {
                Vec::new()
            } else {
                section.lyrics.split("\n\n").map(|slide| slide.split('\n').collect::<Vec<_>>()).collect()
            },
        })).collect::<Vec<_>>(),
        "sequence": arrangement.sequence.iter().map(|entry| match entry {
            SequenceEntry::Section(slot) => json!(slot),
            SequenceEntry::Unresolved { label, line, candidates } =>
                json!(format!("unresolved {label:?} line {line} candidates {candidates}")),
        }).collect::<Vec<_>>(),
        "issues": draft.issues.iter().map(|issue| json!({
            "line": issue.line,
            "kind": format!("{:?}", issue.kind),
            "blocking": issue.blocking,
        })).collect::<Vec<_>>(),
        "can_save": draft.can_save(),
    })
}

#[test]
fn ac2_every_markdown_fixture_parses_to_its_expectation() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/song-import/markdown");
    let mut checked = 0;
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .expect("the Markdown fixture directory exists")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    entries.sort();
    for path in entries {
        let bytes = fs::read(&path).expect("a fixture is readable");
        let actual = render(&parse(&bytes, ids()));
        let expect = path.with_extension("expect.json");
        let expected: Value =
            serde_json::from_slice(&fs::read(&expect).unwrap_or_else(|_| {
                panic!("{} is missing; actual:\n{actual:#}", expect.display())
            }))
            .expect("an expectation is JSON");
        assert_eq!(actual, expected, "{}", path.display());
        checked += 1;
    }
    assert_eq!(checked, 8, "the corpus has eight Markdown fixtures");
}

/// Marker and front matter edge cases, one table: the body starts on line 5
/// of `with_body`, so a case's first body line is line 5.
#[test]
fn markdown_marker_edge_cases() {
    let bad = |text: &str| IssueKind::BadFrontMatter(text.to_owned());
    // (case, whole file, issues as (line, kind, blocking), sections, sequence)
    type Case = (
        &'static str,
        String,
        Vec<(usize, IssueKind, bool)>,
        Vec<(Option<&'static str>, &'static str)>,
        Vec<SequenceEntry>,
    );
    let body = |body: &str| format!("---\ntitle: Song\nlanguage: English\n---\n{body}");
    let cases: Vec<Case> = vec![
        (
            "an empty repeat label is a malformed marker kept as lyrics",
            body("[Verse]\nOne\n[Repeat:]\n"),
            vec![(7, IssueKind::NotALabel("[Repeat:]".to_owned()), false)],
            vec![(Some("Verse"), "One\n[Repeat:]")],
            vec![SequenceEntry::Section(0)],
        ),
        (
            "spacing and case inside a repeat marker do not matter",
            body("[Verse]\nOne\n[ repeat : verse ]\n"),
            vec![],
            vec![(Some("Verse"), "One")],
            vec![SequenceEntry::Section(0), SequenceEntry::Section(0)],
        ),
        (
            "a repeat before its section resolves to the later section",
            body("[Repeat: Verse]\n[Verse]\nOne\n"),
            vec![],
            vec![(Some("Verse"), "One")],
            vec![SequenceEntry::Section(0), SequenceEntry::Section(0)],
        ),
        (
            "text right after a repeat starts a new unlabeled section",
            body("[Verse]\nOne\n[Repeat: Verse]\nTwo\n"),
            vec![],
            vec![(Some("Verse"), "One"), (None, "Two")],
            vec![
                SequenceEntry::Section(0),
                SequenceEntry::Section(0),
                SequenceEntry::Section(1),
            ],
        ),
        (
            "a --- line inside the body is lyrics",
            body("[Verse]\nOne\n---\nTwo\n"),
            vec![],
            vec![(Some("Verse"), "One\n---\nTwo")],
            vec![SequenceEntry::Section(0)],
        ),
    ];
    for (name, text, issues, expected_sections, sequence) in cases {
        let found = draft(&text);
        assert_eq!(kinds(&found), issues, "{name}: issues");
        assert_eq!(sections(&found), expected_sections, "{name}: sections");
        assert_eq!(
            found.document.arrangements[0].sequence, sequence,
            "{name}: sequence"
        );
    }

    // Front matter faults are blocking, each with its own reason.
    let blocking = [
        (
            "a duplicate key",
            "---\ntitle: A\ntitle: B\nlanguage: English\n---\n[Verse]\nOne\n",
            (3, bad("title appears twice."), true),
        ),
        (
            "a block list under a scalar-only key",
            "---\ntitle:\n  - A\n  - B\nlanguage: English\n---\n[Verse]\nOne\n",
            (2, bad("title takes one value, not a list."), true),
        ),
        (
            "a line break escaped inside a quoted title",
            "---\ntitle: \"Ave\\nVerum\"\nlanguage: English\n---\n[Verse]\nOne\n",
            (2, bad("A value can't contain a line break or a tab."), true),
        ),
    ];
    for (name, text, expected) in blocking {
        let found = draft(text);
        assert!(!found.can_save(), "{name}: blocks saving");
        assert!(
            kinds(&found).contains(&expected),
            "{name}: expected {expected:?} in {:?}",
            kinds(&found)
        );
    }
}
