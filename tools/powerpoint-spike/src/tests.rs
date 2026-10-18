use crate::fixture::encode_png;
use crate::validate::outline;
use crate::{
    Cue, DeckPlan, ExportError, MassItem, build, export, export_with, fixture_plan, frame,
    sha256_hex, validate,
};
use std::{
    fs,
    io::{Cursor, Read, Write},
};

/// A one-item deck whose only Cue is operator-only notes.
fn notes_plan(title: &str, notes: &str) -> DeckPlan {
    DeckPlan {
        items: vec![MassItem {
            title: title.to_owned(),
            cues: vec![Cue::OperatorOnly {
                notes: notes.to_owned(),
            }],
        }],
    }
}

/// Copies every entry of `bytes`, letting `edit` change or drop each one.
fn rewrite_zip(bytes: &[u8], mut edit: impl FnMut(&str, Vec<u8>) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("entry");
        let name = file.name().to_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).expect("read entry");
        drop(file);
        if let Some(data) = edit(&name, data) {
            writer.start_file(name.as_str(), options).expect("start");
            writer.write_all(&data).expect("write");
        }
    }
    writer.finish().expect("finish").into_inner()
}

#[test]
fn escaped_text_round_trips_exactly() {
    let plan = notes_plan("Q&A", "Bread & wine <\"x'>");
    let bytes = build(&plan).expect("builds");
    validate(&bytes, &plan).expect("validates");
    let found = outline(&bytes).expect("outline");
    assert_eq!(found.sections[1].0, "Q&A");
    assert_eq!(
        found.slides[1].notes.as_deref(),
        Some("Bread & wine <\"x'>")
    );
}

#[test]
fn numeric_references_are_resolved() {
    let xml = b"<a:p><a:r><a:t>one&#10;two&#x41;&#66;</a:t></a:r></a:p>";
    let evs = crate::xmlscan::events(xml).expect("events");
    assert_eq!(crate::xmlscan::text_in(&evs, "a:t"), "one\ntwoAB");
}

#[test]
fn two_line_notes_round_trip_with_a_paragraph_per_line() {
    let notes = "First line & more\nSecond line <b>";
    let plan = notes_plan("Reading", notes);
    let bytes = build(&plan).expect("builds");
    validate(&bytes, &plan).expect("validates");
    let found = outline(&bytes).expect("outline");
    assert_eq!(found.slides[1].notes.as_deref(), Some(notes));
}

#[test]
fn empty_and_blank_lines_in_notes_round_trip() {
    let notes = "a\n\nb\n";
    let plan = notes_plan("Reading", notes);
    let bytes = build(&plan).expect("builds");
    let found = outline(&bytes).expect("outline");
    assert_eq!(found.slides[1].notes.as_deref(), Some(notes));
}

#[test]
fn bad_titles_are_rejected_as_bad_text() {
    for title in [
        "",
        "   ",
        "tab\there",
        "line\nbreak",
        "cr\rhere",
        "bell\u{7}",
    ] {
        match build(&notes_plan(title, "ok")) {
            Err(ExportError::BadText {
                item: 0, cue: None, ..
            }) => {}
            other => panic!("title {title:?}: expected BadText, got {other:?}"),
        }
    }
}

#[test]
fn forbidden_characters_are_rejected_in_notes() {
    for ch in [
        '\u{0}', '\u{8}', '\u{B}', '\u{C}', '\u{E}', '\u{1F}', '\u{FFFE}', '\u{FFFF}',
    ] {
        let notes = format!("bad {ch} char");
        match build(&notes_plan("Reading", &notes)) {
            Err(ExportError::BadText {
                item: 0,
                cue: Some(0),
                ..
            }) => {}
            other => panic!("{ch:?}: expected BadText, got {other:?}"),
        }
    }
}

#[test]
fn forbidden_characters_are_rejected_in_titles() {
    assert!(matches!(
        build(&notes_plan("Bad\u{1}title", "ok")),
        Err(ExportError::BadText { cue: None, .. })
    ));
}

#[test]
fn carriage_return_in_notes_is_rejected() {
    assert!(matches!(
        build(&notes_plan("Reading", "a\r\nb")),
        Err(ExportError::BadText { cue: Some(0), .. })
    ));
}

#[test]
fn export_with_forced_failure_publishes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("deck.pptx");
    let sentinel = b"sentinel bytes".to_vec();
    fs::write(&dest, &sentinel).expect("write sentinel");

    match export_with(&dest, &fixture_plan(), |_, _| {
        Err(vec!["forced".to_owned()])
    }) {
        Err(ExportError::Validation(failures)) => assert_eq!(failures, vec!["forced"]),
        other => panic!("expected Validation, got {other:?}"),
    }
    assert_eq!(fs::read(&dest).expect("read dest"), sentinel);
    assert_no_temp_siblings(dir.path());
}

#[test]
fn export_with_forced_failure_creates_no_destination() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("deck.pptx");

    assert!(matches!(
        export_with(&dest, &fixture_plan(), |_, _| Err(vec![
            "forced".to_owned()
        ])),
        Err(ExportError::Validation(_))
    ));
    assert!(!dest.exists());
    assert_no_temp_siblings(dir.path());
}

#[test]
fn export_with_real_validation_against_other_plan_leaves_dest_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("deck.pptx");
    let sentinel = b"sentinel bytes".to_vec();
    fs::write(&dest, &sentinel).expect("write sentinel");

    let plan = fixture_plan();
    let mut other = fixture_plan();
    if let Cue::Image { png } = &mut other.items[0].cues[0] {
        *png = frame(99);
    }
    assert!(matches!(
        export_with(&dest, &plan, |bytes, _| validate(bytes, &other)),
        Err(ExportError::Validation(_))
    ));
    assert_eq!(fs::read(&dest).expect("read dest"), sentinel);
    assert_no_temp_siblings(dir.path());
}

#[test]
fn dropping_an_override_fails_validation() {
    let bytes = build(&fixture_plan()).expect("builds");
    let broken = rewrite_zip(&bytes, |name, data| {
        if name == "[Content_Types].xml" {
            let text = String::from_utf8(data).expect("utf8");
            let start = text
                .find("<Override PartName=\"/ppt/slides/slide1.xml\"")
                .expect("override present");
            let end = start + text[start..].find("/>").expect("end") + 2;
            Some(format!("{}{}", &text[..start], &text[end..]).into_bytes())
        } else {
            Some(data)
        }
    });
    let failures = validate(&broken, &fixture_plan()).expect_err("missing override must fail");
    assert!(
        failures.iter().any(|f| f.contains("no content type")),
        "{failures:?}"
    );
}

fn edit_content_types(edit: impl Fn(String) -> String) -> Vec<u8> {
    let bytes = build(&fixture_plan()).expect("builds");
    rewrite_zip(&bytes, |name, data| {
        if name == "[Content_Types].xml" {
            Some(edit(String::from_utf8(data).expect("utf8")).into_bytes())
        } else {
            Some(data)
        }
    })
}

const SLIDE_TYPE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const NOTES_SLIDE_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";

#[test]
fn override_on_a_relationship_part_fails_validation() {
    let broken = edit_content_types(|text| {
        text.replace(
            "</Types>",
            &format!(
                "<Override PartName=\"/ppt/slides/_rels/slide1.xml.rels\" ContentType=\"{SLIDE_TYPE}\"/></Types>"
            ),
        )
    });
    let failures = validate(&broken, &fixture_plan()).expect_err("rels override must fail");
    assert!(
        failures
            .iter()
            .any(|f| f.contains("slide1.xml.rels") && f.contains("relationships+xml")),
        "{failures:?}"
    );
}

#[test]
fn wrong_override_type_for_a_slide_fails_validation() {
    let broken = edit_content_types(|text| {
        let needle =
            format!("<Override PartName=\"/ppt/slides/slide1.xml\" ContentType=\"{SLIDE_TYPE}\"/>");
        assert!(text.contains(&needle), "override present");
        text.replace(&needle, &needle.replace(SLIDE_TYPE, NOTES_SLIDE_TYPE))
    });
    let failures = validate(&broken, &fixture_plan()).expect_err("wrong type must fail");
    assert!(
        failures
            .iter()
            .any(|f| f.contains("ppt/slides/slide1.xml") && f.contains("requires")),
        "{failures:?}"
    );
}

#[test]
fn override_for_a_missing_part_fails_validation() {
    let broken = edit_content_types(|text| {
        text.replace(
            "</Types>",
            &format!(
                "<Override PartName=\"/ppt/slides/slide99.xml\" ContentType=\"{SLIDE_TYPE}\"/></Types>"
            ),
        )
    });
    let failures = validate(&broken, &fixture_plan()).expect_err("ghost override must fail");
    assert!(
        failures
            .iter()
            .any(|f| f.contains("slide99.xml") && f.contains("not in the package")),
        "{failures:?}"
    );
}

#[test]
fn built_package_has_no_override_on_relationship_parts() {
    let bytes = build(&fixture_plan()).expect("builds");
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut text = String::new();
    archive
        .by_name("[Content_Types].xml")
        .expect("content types")
        .read_to_string(&mut text)
        .expect("read");
    for chunk in text.split("<Override ").skip(1) {
        let part = chunk.split('"').nth(1).expect("PartName value");
        assert!(!part.ends_with(".rels"), "Override on {part}");
    }
}

#[test]
fn dropping_a_related_part_fails_validation() {
    let bytes = build(&fixture_plan()).expect("builds");
    let broken = rewrite_zip(&bytes, |name, data| {
        (name != "ppt/slideLayouts/slideLayout1.xml").then_some(data)
    });
    let failures = validate(&broken, &fixture_plan()).expect_err("missing target must fail");
    assert!(
        failures.iter().any(|f| f.contains("is missing")),
        "{failures:?}"
    );
}

const SAFETY: &str = "Safety";

fn wrong_size_png() -> Vec<u8> {
    encode_png(10, 10, &vec![0u8; 10 * 10 * 3]).expect("encode small png")
}

#[test]
fn fixture_builds_and_validates() {
    let plan = fixture_plan();
    let bytes = build(&plan).expect("fixture builds");
    validate(&bytes, &plan).expect("fixture validates");
}

#[test]
fn slide_order_and_sections_match_the_plan() {
    let bytes = build(&fixture_plan()).expect("fixture builds");
    let found = outline(&bytes).expect("outline");
    let names: Vec<(&str, Vec<usize>)> = found
        .sections
        .iter()
        .map(|(name, indices)| (name.as_str(), indices.clone()))
        .collect();
    assert_eq!(
        names,
        vec![
            (SAFETY, vec![0]),
            ("Identification", vec![1]),
            ("Entrance Song", vec![2, 3, 4]),
            ("Penitential Act response", vec![5]),
            ("First Reading", vec![6, 7]),
            (SAFETY, vec![8]),
        ]
    );
    assert_eq!(found.slides.len(), 9);
    assert_eq!(
        found.slides[0].notes.as_deref(),
        Some("Safety lead-in: black slide before the first Mass item.")
    );
    assert_eq!(
        found.slides[8].notes.as_deref(),
        Some("Safety closing: black slide after the final Mass item.")
    );
}

#[test]
fn image_hashes_match_the_frames() {
    let plan = fixture_plan();
    let bytes = build(&plan).expect("fixture builds");
    let found = outline(&bytes).expect("outline");
    let images = [1, 2, 3, 4, 5];
    let image_slides = [1, 2, 3, 5, 6];
    for (seed, slide) in images.iter().zip(image_slides) {
        assert_eq!(
            found.slides[slide].media_sha256.as_deref(),
            Some(sha256_hex(&frame(*seed)).as_str()),
            "slide {slide} hash"
        );
        assert_eq!(found.slides[slide].media_size, Some((1920, 1080)));
    }
}

#[test]
fn notes_only_on_black_slides() {
    let bytes = build(&fixture_plan()).expect("fixture builds");
    let found = outline(&bytes).expect("outline");
    for (index, slide) in found.slides.iter().enumerate() {
        let black = slide.background.as_deref() == Some("000000");
        assert_eq!(slide.notes.is_some(), black, "slide {index}");
    }
    assert_eq!(
        found.slides[4].notes.as_deref(),
        Some("Hold: wait for the procession")
    );
    assert_eq!(
        found.slides[7].notes.as_deref(),
        Some("Blackout: reader at ambo")
    );
}

#[test]
fn image_slides_have_no_shapes_or_text() {
    let bytes = build(&fixture_plan()).expect("fixture builds");
    let found = outline(&bytes).expect("outline");
    for slide in [1, 2, 3, 5, 6] {
        let image = &found.slides[slide];
        assert_eq!(image.shapes, 0, "slide {slide} shapes");
        assert!(image.text.is_empty(), "slide {slide} text");
        assert_eq!(image.pictures, 1, "slide {slide} pictures");
        assert!(image.no_change_aspect);
    }
}

#[test]
fn wrong_size_frame_is_rejected_and_dest_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("deck.pptx");
    let sentinel = b"sentinel bytes".to_vec();
    fs::write(&dest, &sentinel).expect("write sentinel");

    let plan = DeckPlan {
        items: vec![MassItem {
            title: "Bad".to_owned(),
            cues: vec![
                Cue::Image { png: frame(1) },
                Cue::Image {
                    png: wrong_size_png(),
                },
            ],
        }],
    };
    match export(&dest, &plan) {
        Err(ExportError::BadFrame { item, cue, .. }) => {
            assert_eq!((item, cue), (0, 1));
        }
        other => panic!("expected BadFrame, got {other:?}"),
    }
    assert_eq!(fs::read(&dest).expect("read dest"), sentinel);
    assert_no_temp_siblings(dir.path());
}

#[test]
fn non_png_frame_is_rejected() {
    let plan = DeckPlan {
        items: vec![MassItem {
            title: "Bad".to_owned(),
            cues: vec![Cue::Image {
                png: b"not a png".to_vec(),
            }],
        }],
    };
    assert!(matches!(
        build(&plan),
        Err(ExportError::BadFrame {
            item: 0,
            cue: 0,
            ..
        })
    ));
}

#[test]
fn validation_fails_against_a_different_plan() {
    let bytes = build(&fixture_plan()).expect("fixture builds");
    let mut other = fixture_plan();
    if let Cue::Image { png } = &mut other.items[0].cues[0] {
        *png = frame(99);
    }
    let failures = validate(&bytes, &other).expect_err("different frame must fail");
    assert!(
        failures
            .iter()
            .any(|failure| failure.contains("image bytes differ")),
        "{failures:?}"
    );
}

#[test]
fn export_replaces_an_existing_file_on_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("deck.pptx");
    fs::write(&dest, b"old contents").expect("write old file");

    let plan = fixture_plan();
    export(&dest, &plan).expect("export succeeds");

    let written = fs::read(&dest).expect("read dest");
    assert_ne!(written, b"old contents");
    validate(&written, &plan).expect("published deck validates");
    assert_no_temp_siblings(dir.path());
}

fn assert_no_temp_siblings(dir: &std::path::Path) {
    let leftovers: Vec<_> = fs::read_dir(dir)
        .expect("read dir")
        .map(|entry| entry.expect("entry").file_name())
        .filter(|name| name.to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}
