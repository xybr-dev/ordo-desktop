//! T0.4 PR 4: the canonical shaping path and the serialized logical frame.
//!
//! Every check goes through [`rendering::render`] and [`rendering::Frame`],
//! never through shaping internals. Serialized comparisons are in-memory
//! strings, so no fixture file or line-ending pinning participates.

use rendering::{
    BODY_LINE_HEIGHT, Fit, RenderError, SemanticContent, TargetDensity, Theme, render,
};

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

    // A whitespace-only line is still blank: invisible spacing must not
    // become same-slide vertical spacing. Real spacing belongs in explicit
    // semantic input, not raw whitespace.
    let (spaced, _) = render(
        &body_only("unang linya\n \nikalawang linya"),
        &theme,
        density,
    )
    .unwrap();
    assert!(spaced.pagination().hard_split);
    assert_eq!(spaced.pagination().first_hard_split, Some(1));
    assert_eq!(spaced.pagination().body_lines, 2);
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

#[test]
fn combining_marks_shape_in_their_base_face() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // Decomposed alpha plus acute: the acute alone resolves to the primary
    // face, but split shaping would detach it from its base, so the whole
    // cluster must stay in the fallback face with its base. (The pinned
    // fallback face composes the pair into one glyph; the pinned property
    // is the face, not the count.)
    let (frame, _) = render(&body_only("A \u{3B1}\u{301}"), &theme, density).unwrap();
    assert_eq!(frame.body_lines().len(), 1);
    let line = &frame.body_lines()[0];
    let primary = line
        .glyphs
        .iter()
        .filter(|glyph| glyph.font == rendering::fonts::CanonicalFont::Primary)
        .count();
    let fallback = line
        .glyphs
        .iter()
        .filter(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback)
        .count();
    assert_eq!(primary, 1, "only the Latin base stays primary");
    assert!(
        fallback >= 1,
        "the fallback base and its mark must share one face"
    );
}

#[test]
fn cluster_falls_back_unless_primary_covers_every_member() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // U+0305 rides on a Latin base the primary face carries, but the face
    // lacks the mark itself while fallback carries both. Splitting the
    // cluster would emit a primary .notdef for the mark; the whole cluster
    // must shape in fallback instead.
    let (frame, _) = render(&body_only("A\u{305}"), &theme, density).unwrap();
    assert_eq!(frame.body_lines().len(), 1);
    let glyphs = &frame.body_lines()[0].glyphs;
    assert!(!glyphs.is_empty());
    assert!(
        glyphs
            .iter()
            .all(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback),
        "partial primary coverage must not win the cluster"
    );
    assert!(
        glyphs.iter().all(|glyph| glyph.id != 0),
        "no member may degrade to .notdef"
    );
}

#[test]
fn adjacent_mixed_script_token_splits_runs_without_whitespace() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // One token, two strong scripts, one face: the Greek alpha must still
    // shape under the Greek script and the Han character under Han, with
    // common characters riding along rather than forcing boundaries.
    let (frame, diagnostics) = render(&body_only("\u{3B1}\u{4E2D}"), &theme, density).unwrap();
    assert!(diagnostics.warnings.is_empty());
    assert_eq!(frame.body_lines().len(), 1);
    let line = &frame.body_lines()[0];
    assert!(
        line.glyphs.len() >= 2,
        "both strong-script members must survive"
    );
    assert!(
        line.glyphs
            .iter()
            .all(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback),
        "both members resolve fallback"
    );
    // Glyph order follows the token: Greek advances precede Han advances.
    let mut last_x = -1;
    for glyph in &line.glyphs {
        assert!(glyph.x >= last_x, "glyphs must follow token order");
        last_x = glyph.x;
    }
    // Clusters still map to both source characters (alpha is two bytes).
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 0));
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 2));
}

#[test]
fn adjacent_fallback_latin_and_greek_shape_under_their_own_scripts() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // One token, no whitespace: the Latin cluster `A`+mark must shape under
    // the Latin script even though it shares the fallback face with the
    // Greek alpha that follows it. Itemization is structural per the
    // qualification; this pins the routing faces, order, and clusters.
    let (frame, diagnostics) = render(&body_only("A\u{305}\u{3B1}"), &theme, density).unwrap();
    assert!(diagnostics.warnings.is_empty());
    assert_eq!(frame.body_lines().len(), 1);
    let line = &frame.body_lines()[0];
    assert!(
        line.glyphs.len() >= 3,
        "base, mark, and alpha must all survive"
    );
    assert!(
        line.glyphs
            .iter()
            .all(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback),
        "every member resolves fallback"
    );
    assert!(
        line.glyphs.iter().all(|glyph| glyph.id != 0),
        "no member may degrade to .notdef"
    );
    let mut last_x = -1;
    for glyph in &line.glyphs {
        assert!(glyph.x >= last_x, "glyphs must follow token order");
        last_x = glyph.x;
    }
    // Clusters map to both parts: Latin base at 0, Greek alpha at 3.
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 0));
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 3));
}

#[test]
fn leading_common_fallback_cannot_bridge_two_strong_scripts() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // One token: a common fallback mark (peso sign) leads fallback Greek
    // and Han members. The leader adopts the Greek run that follows it
    // instead of leaving a neutral run that would bridge Greek into Han,
    // so the Han member still starts its own run.
    let (frame, diagnostics) =
        render(&body_only("\u{20B1}\u{3B1}\u{4E2D}"), &theme, density).unwrap();
    assert!(diagnostics.warnings.is_empty());
    assert_eq!(frame.body_lines().len(), 1);
    let line = &frame.body_lines()[0];
    assert!(
        line.glyphs
            .iter()
            .all(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback),
        "every member resolves fallback"
    );
    let mut last_x = -1;
    for glyph in &line.glyphs {
        assert!(glyph.x >= last_x, "glyphs must follow token order");
        last_x = glyph.x;
    }
    // Clusters map to all three members: peso at 0, alpha at 3, Han at 5.
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 0));
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 3));
    assert!(line.glyphs.iter().any(|glyph| glyph.cluster == 5));
}

#[test]
fn oversized_header_and_footer_fail_instead_of_misbounding_zones() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let tall = (1..=20)
        .map(|n| format!("header {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let content = SemanticContent {
        response_header: Some(tall.clone()),
        footer: Some(tall),
        ..body_only("")
    };

    // Chrome taller than the safe area cannot be paginated away, so it is a
    // structured failure rather than a warning beside incoherent bounds.
    assert_eq!(
        render(&content, &theme, density),
        Err(RenderError::ZonesExceedSafeArea {
            header_height: 20 * BODY_LINE_HEIGHT,
            footer_height: 20 * BODY_LINE_HEIGHT,
            safe_height: 920,
        })
    );
    // Chrome that exactly fills the safe area still stacks coherently.
    let snug = (1..=11)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let (frame, diagnostics) = render(
        &SemanticContent {
            response_header: Some(snug),
            ..body_only("")
        },
        &theme,
        density,
    )
    .unwrap();
    assert_eq!(frame.pagination().fit, Fit::Fits);
    assert!(diagnostics.warnings.is_empty());
    assert_eq!(frame.body().height, 920 - 11 * BODY_LINE_HEIGHT);
}

#[test]
fn narrow_safe_area_reports_horizontal_overflow_without_dropping_glyphs() {
    let theme = Theme {
        safe_inset_x: 959,
        safe_inset_y: 80,
    };
    let density = TargetDensity::SCALE_100;

    let (frame, diagnostics) = render(&body_only("W"), &theme, density).unwrap();

    assert_eq!(frame.safe_area().width, 2);
    assert_eq!(frame.pagination().fit, Fit::Overflows);
    assert!(
        !diagnostics.warnings.is_empty(),
        "a glyph wider than the safe area must diagnose"
    );
    // Nothing is dropped: the overwide line stays present with its glyph.
    assert_eq!(frame.body_lines().len(), 1);
    assert!(!frame.body_lines()[0].glyphs.is_empty());
    assert!(frame.body_lines()[0].width > 2);
}

/// Fixed probe content for the cross-process determinism check below.
fn serializer_probe_content() -> SemanticContent {
    SemanticContent {
        response_header: Some("Authored header".to_owned()),
        footer: Some("Authored footer".to_owned()),
        ..body_only("Maayong buntag, mga igsoon! Awit \u{3B1} 123 \u{4E2D}")
    }
}

/// Child entry for the cross-process check: when this test binary re-runs
/// itself with the output variable set, it writes one serialized artifact to
/// the given path instead of asserting. Without the variable it passes
/// trivially so the ordinary suite is unaffected.
#[test]
fn cross_process_serializer_child() {
    let Some(out) = std::env::var_os("ORDO_CANONICAL_SERIALIZER_CHILD_OUT") else {
        return;
    };
    let (frame, _) = render(
        &serializer_probe_content(),
        &Theme::default(),
        TargetDensity::SCALE_100,
    )
    .expect("fixed serializer content must compose");
    std::fs::write(out, frame.serialize()).expect("child artifact must be writable");
}

#[test]
fn serialized_frames_are_identical_across_processes() {
    let scratch =
        std::env::temp_dir().join(format!("ordo-canonical-serializer-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir must be writable");
    let first_path = scratch.join("first.txt");
    let second_path = scratch.join("second.txt");
    for path in [&first_path, &second_path] {
        let status = std::process::Command::new(
            std::env::current_exe().expect("serializer test binary must have a path"),
        )
        .args(["cross_process_serializer_child", "--exact"])
        .env("ORDO_CANONICAL_SERIALIZER_CHILD_OUT", path)
        .status()
        .expect("serializer child must run");
        assert!(status.success(), "serializer child must succeed");
    }
    let first = std::fs::read_to_string(&first_path).expect("first artifact must exist");
    let second = std::fs::read_to_string(&second_path).expect("second artifact must exist");
    std::fs::remove_dir_all(&scratch).ok();

    assert_eq!(
        first, second,
        "two fresh processes must serialize byte-identically"
    );
    let (frame, _) = render(
        &serializer_probe_content(),
        &Theme::default(),
        TargetDensity::SCALE_100,
    )
    .unwrap();
    assert_eq!(
        frame.serialize(),
        first,
        "fresh processes must match this process"
    );
}
