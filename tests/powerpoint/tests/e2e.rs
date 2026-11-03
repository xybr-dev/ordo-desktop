//! T0.9 WP3 acceptance: export the fixture deck, then reopen it with the
//! independent inspector and with raw archive reads.
//!
//! Every test writes only under a temporary directory.

use powerpoint_spike::{Cue, DeckPlan, ExportError, export, fixture_plan, frame};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
use xtask::inspect_pptx::{SlideKind, check_rules, outline, read_deck};

const SENTINEL: &[u8] = b"sentinel: the previous destination";

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/powerpoint")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Line endings do not matter for the outline comparison.
fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

/// Image Cues in plan order, as `(png bytes)`.
fn image_pngs(plan: &DeckPlan) -> Vec<&[u8]> {
    plan.items
        .iter()
        .flat_map(|item| item.cues.iter())
        .filter_map(|cue| match cue {
            Cue::Image { png } => Some(png.as_slice()),
            Cue::OperatorOnly { .. } => None,
        })
        .collect()
}

/// Every entry of the archive whose name matches `keep`, as UTF-8 text.
fn raw_parts(bytes: &[u8], keep: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("deck is a ZIP");
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    let mut parts = Vec::new();
    for name in names.into_iter().filter(|name| keep(name)) {
        let mut text = String::new();
        archive
            .by_name(&name)
            .expect("named entry exists")
            .read_to_string(&mut text)
            .expect("XML part is UTF-8");
        parts.push((name, text));
    }
    parts
}

/// Names of entries directly inside `dir`.
fn dir_entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("directory is readable")
        .map(|entry| entry.expect("entry is readable").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn fixture_deck_reopens_with_clean_rules_and_expected_outline() {
    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("fixture.pptx");
    let plan = fixture_plan();
    export(&dest, &plan).expect("fixture exports");

    let bytes = fs::read(&dest).expect("exported deck is readable");
    let slides = read_deck(&bytes).expect("inspector reads the deck");
    assert_eq!(check_rules(&slides, &bytes), Vec::<String>::new());

    let expected = fs::read_to_string(fixture_dir().join("expected-outline.txt"))
        .expect("expected outline fixture exists");
    assert_eq!(normalise(&outline(&slides)), normalise(&expected));
}

#[test]
fn image_slides_match_input_frames_in_plan_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("fixture.pptx");
    let plan = fixture_plan();
    export(&dest, &plan).expect("fixture exports");

    let bytes = fs::read(&dest).expect("exported deck is readable");
    let slides = read_deck(&bytes).expect("inspector reads the deck");

    let expected: Vec<String> = image_pngs(&plan).into_iter().map(sha256_hex).collect();
    let actual: Vec<String> = slides
        .iter()
        .filter_map(|slide| match &slide.kind {
            SlideKind::Image {
                width,
                height,
                sha256,
            } => {
                assert_eq!((*width, *height), (1920, 1080), "slide {}", slide.index);
                Some(sha256.clone())
            }
            SlideKind::Black => None,
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(expected.len(), 5, "fixture has five image Cues");
}

#[test]
fn raw_archive_has_no_text_on_image_slides_and_declares_sections_and_size() {
    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("fixture.pptx");
    let plan = fixture_plan();
    export(&dest, &plan).expect("fixture exports");
    let bytes = fs::read(&dest).expect("exported deck is readable");

    let slide_parts = raw_parts(&bytes, |name| {
        name.starts_with("ppt/slides/slide") && name.ends_with(".xml")
    });
    let image_slides: Vec<&(String, String)> = slide_parts
        .iter()
        .filter(|(_, xml)| xml.contains("<p:pic"))
        .collect();
    assert_eq!(image_slides.len(), image_pngs(&plan).len());
    for (name, xml) in &image_slides {
        assert!(!xml.contains("<a:t>"), "{name} has text runs");
        assert!(!xml.contains("<a:t "), "{name} has text runs");
    }

    let presentation = raw_parts(&bytes, |name| name == "ppt/presentation.xml");
    let (_, presentation) = presentation.first().expect("presentation.xml exists");
    assert!(presentation.contains("p14:sectionLst"), "no section list");
    assert!(
        presentation.contains(r#"sldSz cx="12192000" cy="6858000""#),
        "slide size is not 16:9 12192000x6858000"
    );
}

#[test]
fn failed_export_over_existing_destination_leaves_it_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("fixture.pptx");
    fs::write(&dest, SENTINEL).expect("seed destination");

    // A truncated PNG cannot decode, so the export is refused as a BadFrame.
    // The spike exposes no helper for a wrong-size PNG, so truncation is used.
    // This covers the pre-write BadFrame path only. The post-write
    // validation-failure path (export_with) is covered by
    // tools/powerpoint-spike/src/tests.rs.
    let mut plan = fixture_plan();
    let png = frame(1);
    plan.items[0].cues[0] = Cue::Image {
        png: png[..png.len() / 2].to_vec(),
    };

    match export(&dest, &plan) {
        Err(ExportError::BadFrame {
            item: 0, cue: 0, ..
        }) => {}
        other => panic!("expected BadFrame for item 0 cue 0, got {other:?}"),
    }
    assert_eq!(fs::read(&dest).expect("destination still exists"), SENTINEL);
    assert_eq!(dir_entries(dir.path()), vec!["fixture.pptx".to_owned()]);
}

#[test]
fn failed_export_to_missing_destination_creates_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("fixture.pptx");

    let mut plan = fixture_plan();
    let png = frame(2);
    plan.items[1].cues[0] = Cue::Image {
        png: png[..png.len() / 2].to_vec(),
    };

    assert!(matches!(
        export(&dest, &plan),
        Err(ExportError::BadFrame {
            item: 1,
            cue: 0,
            ..
        })
    ));
    assert!(
        !dest.exists(),
        "a failed export must not create the destination"
    );
    assert!(
        dir_entries(dir.path()).is_empty(),
        "no temporary left behind"
    );
}

#[test]
fn successful_export_replaces_existing_destination() {
    let dir = tempfile::tempdir().expect("temp dir");
    let dest = dir.path().join("fixture.pptx");
    fs::write(&dest, SENTINEL).expect("seed destination");

    export(&dest, &fixture_plan()).expect("fixture exports over a sentinel");

    let bytes = fs::read(&dest).expect("replaced destination is readable");
    assert_ne!(bytes, SENTINEL);
    let slides = read_deck(&bytes).expect("replacement reopens");
    assert_eq!(check_rules(&slides, &bytes), Vec::<String>::new());
    assert_eq!(dir_entries(dir.path()), vec!["fixture.pptx".to_owned()]);
}
