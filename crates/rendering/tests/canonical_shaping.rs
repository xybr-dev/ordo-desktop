//! T0.4 PR 4: the canonical shaping path and the serialized logical frame.
//!
//! Every check goes through [`rendering::render`] and [`rendering::Frame`],
//! never through shaping internals. Serialized comparisons are in-memory
//! strings, so no fixture file or line-ending pinning participates.

use rendering::{BODY_LINE_HEIGHT, Fit, SemanticContent, TargetDensity, Theme, render};

fn body_only(body: &str) -> SemanticContent {
    SemanticContent {
        body: body.to_owned(),
        response_header: None,
        psalm_header: None,
        footer: None,
    }
}

#[test]
fn same_inputs_yield_byte_identical_serialized_frames() {
    let content = body_only("Maayong buntag, mga igsoon! Awit 123.");
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (first, first_diagnostics) = render(&content, &theme, density).unwrap();
    let (second, second_diagnostics) = render(&content, &theme, density).unwrap();

    assert_eq!(first.serialize(), second.serialize());
    assert_eq!(first_diagnostics, second_diagnostics);
    // The artifact carries glyphs, wrapping, pagination inputs, safe area,
    // and element bounds together.
    let serialized = first.serialize();
    assert!(serialized.starts_with("ordo-canonical-frame/1\n"));
    assert!(serialized.contains("safe-area 120 80 1680 920\n"));
    assert!(serialized.contains("body-rect "));
    assert!(serialized.contains("pagination "));
    assert!(serialized.contains("\nglyph "));
}

#[test]
fn wrapping_follows_measured_geometry_not_character_counts() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (narrow, _) = render(&body_only(&"i".repeat(40)), &theme, density).unwrap();
    let (wide, _) = render(&body_only(&"W".repeat(40)), &theme, density).unwrap();

    // Same character count, different measured widths, different wraps.
    assert_eq!(narrow.body_lines().len(), 1);
    assert!(wide.body_lines().len() > 1);
    for line in narrow.body_lines().iter().chain(wide.body_lines()) {
        assert!(
            line.width <= 1680,
            "wrapped line exceeds the body width: {}",
            line.width
        );
    }
}

#[test]
fn authored_line_breaks_are_preserved_and_blank_lines_mark_hard_split() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (kept, _) = render(&body_only("unang linya\nikalawang linya"), &theme, density).unwrap();
    assert_eq!(kept.pagination().authored_body_lines, 2);
    assert_eq!(kept.pagination().body_lines, 2);
    assert!(!kept.pagination().hard_split);

    let (split, _) = render(
        &body_only("unang linya\n\nikalawang linya"),
        &theme,
        density,
    )
    .unwrap();
    assert_eq!(split.pagination().authored_body_lines, 3);
    assert!(split.pagination().hard_split);
    assert_eq!(split.pagination().first_hard_split, Some(1));
    // The boundary never drops content: both sides stay in the complete frame.
    assert_eq!(split.pagination().body_lines, 2);
    assert_eq!(split.body_lines()[0].authored_index, 0);
    assert_eq!(split.body_lines()[1].authored_index, 2);
}

#[test]
fn overflow_warns_beside_a_complete_frame_instead_of_clipping() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let body = (1..=20)
        .map(|n| format!("linya {n} sa awit"))
        .collect::<Vec<_>>()
        .join("\n");

    let (frame, diagnostics) = render(&body_only(&body), &theme, density).unwrap();

    assert_eq!(frame.pagination().fit, Fit::Overflows);
    assert_eq!(frame.pagination().body_lines, 20);
    assert_eq!(
        frame.pagination().body_content_height,
        20 * BODY_LINE_HEIGHT
    );
    // No line is clipped: every authored line is present with its glyphs.
    assert_eq!(frame.body_lines().len(), 20);
    assert!(
        frame
            .body_lines()
            .iter()
            .all(|line| !line.glyphs.is_empty()),
        "overflow must keep every line"
    );
    assert_eq!(diagnostics.warnings.len(), 1);
    // A short slide still fits without warnings.
    let (fits, fits_diagnostics) = render(&body_only("usa ka linya"), &theme, density).unwrap();
    assert_eq!(fits.pagination().fit, Fit::Fits);
    assert!(fits_diagnostics.warnings.is_empty());
}

#[test]
fn missing_glyphs_shape_through_the_bundled_fallback() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // Greek alpha and the Han character are absent from the primary face;
    // Latin A stays primary. See tests/fonts.rs for the byte-level pinning.
    let (frame, _) = render(&body_only("A \u{3B1} \u{4E2D}"), &theme, density).unwrap();
    let glyphs: Vec<_> = frame
        .body_lines()
        .iter()
        .flat_map(|line| line.glyphs.iter())
        .collect();
    assert!(!glyphs.is_empty());
    assert!(
        glyphs
            .iter()
            .any(|glyph| glyph.font == rendering::fonts::CanonicalFont::Primary),
        "primary glyphs must stay primary"
    );
    assert!(
        glyphs
            .iter()
            .any(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback),
        "absent glyphs must shape through the bundled fallback"
    );
}

#[test]
fn target_density_never_alters_the_serialized_composition() {
    let content = SemanticContent {
        response_header: Some("Authored header".to_owned()),
        footer: Some("Salmo 95".to_owned()),
        ..body_only("Ang ginoo maanaa kaninyo\nuban sa imong espiritu")
    };
    let theme = Theme::default();

    let (light, _) = render(&content, &theme, TargetDensity::from_percent(100).unwrap()).unwrap();
    let (dense, _) = render(&content, &theme, TargetDensity::from_percent(200).unwrap()).unwrap();
    assert_eq!(light.serialize(), dense.serialize());
}

#[test]
fn overlong_words_break_between_characters_without_clipping() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (frame, _) = render(&body_only(&"W".repeat(60)), &theme, density).unwrap();

    assert!(frame.body_lines().len() > 1);
    for line in frame.body_lines() {
        assert!(
            line.width <= 1680,
            "broken fragment exceeds the body width: {}",
            line.width
        );
    }
    let glyphs: usize = frame
        .body_lines()
        .iter()
        .map(|line| line.glyphs.len())
        .sum();
    assert_eq!(glyphs, 60, "every character must survive the break");
}
