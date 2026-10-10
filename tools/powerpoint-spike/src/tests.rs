use crate::fixture::encode_png;
use crate::validate::outline;
use crate::{
    Cue, DeckPlan, ExportError, MassItem, build, export, fixture_plan, frame, sha256_hex, validate,
};
use std::fs;

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
