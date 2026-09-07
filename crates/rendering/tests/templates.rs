//! T0.4 PR 5: the canonical template corpus.
//!
//! Every check goes through [`rendering::render_template`] (and [`render`]
//! where the test pins shared behavior), never through corpus or shaping
//! internals. Zones, alignment, and reference placement are asserted from
//! measured geometry, and each ceiling is pinned at its boundary: the count
//! at the ceiling stays quiet, one past it warns.

use rendering::{
    BODY_FONT_SIZE, PROVISIONAL_DISPLAY_SIZE, PROVISIONAL_META_SIZE, PROVISIONAL_PASSAGE_SIZE,
    RenderError, TargetDensity, TemplateContent, TemplateKind, Theme, line_height_for,
    render_template,
};

fn content(
    kind: TemplateKind,
    header: Option<&str>,
    body: &str,
    footer: Option<&str>,
) -> TemplateContent {
    TemplateContent {
        kind,
        header: header.map(str::to_owned),
        body: body.to_owned(),
        footer: footer.map(str::to_owned),
    }
}

fn lyrics(body: &str) -> TemplateContent {
    content(TemplateKind::Lyrics, Some("Section label"), body, None)
}

#[test]
fn every_template_composes_through_the_same_interface() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let cases = [
        lyrics("first line\nsecond line"),
        content(
            TemplateKind::Response,
            Some("Authored header"),
            "response text",
            None,
        ),
        content(
            TemplateKind::ReadingIdentification,
            None,
            "Proclamation title",
            Some("Reference text"),
        ),
        content(
            TemplateKind::ReadingPassage,
            None,
            "Passage text",
            Some("Reference text"),
        ),
        content(
            TemplateKind::Psalm,
            Some("Authored header"),
            "refrain text",
            Some("Reference text"),
        ),
        content(
            TemplateKind::SongIdentification,
            None,
            "Song title",
            Some("Composer name\nArrangement credit"),
        ),
        content(
            TemplateKind::CelebrationTitle,
            None,
            "Celebration title",
            Some("Localized date"),
        ),
        content(
            TemplateKind::GeneralText,
            Some("General header"),
            "General body",
            Some("General footer"),
        ),
        content(
            TemplateKind::GeneralImage,
            Some("Image header"),
            "",
            Some("Image caption"),
        ),
        content(TemplateKind::Black, None, "", None),
    ];

    for case in &cases {
        let (frame, _) = render_template(case, &theme, density)
            .unwrap_or_else(|error| panic!("{kind:?} must compose: {error}", kind = case.kind));
        assert_eq!(frame.logical_size(), (1920, 1080));
        assert_eq!(frame.safe_area().x, 120);
        assert_eq!(frame.safe_area().y, 80);
        // The serialized artifact carries the whole composition for the
        // fixture corpus PR 6 builds on this.
        assert!(frame.serialize().starts_with("ordo-canonical-frame/1\n"));
    }
}

#[test]
fn lyrics_label_sits_top_left_and_absence_returns_space_to_the_body() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (labeled, _) = render_template(&lyrics("short line"), &theme, density).unwrap();
    let header = labeled.header().expect("label reserves the header zone");
    assert_eq!((header.x, header.y), (120, 80));
    assert_eq!(header.height, line_height_for(PROVISIONAL_META_SIZE));
    assert_eq!(labeled.footer(), None);
    // Body starts where the header ends and spans the safe width.
    assert_eq!(labeled.body().y, header.y + header.height);
    assert_eq!(labeled.body().width, 1680);

    // Label-off: no header, so the body is the whole safe area.
    let unlabeled_content = content(TemplateKind::Lyrics, None, "short line", None);
    let (unlabeled, _) = render_template(&unlabeled_content, &theme, density).unwrap();
    assert_eq!(unlabeled.header(), None);
    assert_eq!(unlabeled.body(), unlabeled.safe_area());
    assert!(unlabeled.body().height > labeled.body().height);
}

#[test]
fn lyrics_body_is_left_aligned_inside_a_centered_block() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    // Two short lines of different widths: one block, left-aligned lines.
    let (frame, _) =
        render_template(&lyrics("short\na much longer line"), &theme, density).unwrap();
    let lines = frame.body_lines();
    assert_eq!(lines.len(), 2);
    let safe = frame.safe_area();
    let safe_x = safe.x as i32;
    let safe_right = safe.x as i32 + safe.width as i32;
    // Every line shares one left edge: the block centers, lines do not.
    assert_eq!(lines[0].x, lines[1].x);
    assert!(
        lines[0].x > safe_x,
        "the block must center, not hug the edge"
    );
    // Outer margins match within one unit: truncating integer division
    // centers odd-width blocks asymmetrically by a single logical unit.
    let block_left = lines.iter().map(|line| line.x).min().unwrap();
    let block_right = lines.iter().map(|line| line.x + line.width).max().unwrap();
    let left_margin = block_left - safe_x;
    let right_margin = safe_right - block_right;
    assert!(
        (left_margin - right_margin).abs() <= 1,
        "block centers within one unit: {left_margin} vs {right_margin}"
    );
}

#[test]
fn response_header_passes_through_and_body_renders_uppercase_centered() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, _) = render_template(
        &content(
            TemplateKind::Response,
            Some("Authored header"),
            "response text",
            None,
        ),
        &theme,
        density,
    )
    .unwrap();

    assert_eq!(frame.header_lines().len(), 1);
    assert_eq!(frame.header_lines()[0].text, "Authored header");
    assert_eq!(frame.body_lines().len(), 1);
    assert_eq!(frame.body_lines()[0].text, "RESPONSE TEXT");
    // Centered in the body zone from measured width.
    let body = frame.body();
    let line = &frame.body_lines()[0];
    assert_eq!(line.x, body.x as i32 + (body.width as i32 - line.width) / 2);
    assert_eq!(line.height, line_height_for(PROVISIONAL_DISPLAY_SIZE));
}

#[test]
fn reading_identification_centers_title_above_bottom_left_reference() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, _) = render_template(
        &content(
            TemplateKind::ReadingIdentification,
            None,
            "Proclamation title",
            Some("Reference text"),
        ),
        &theme,
        density,
    )
    .unwrap();

    assert_eq!(frame.header(), None);
    assert_eq!(frame.body_lines().len(), 1);
    assert_eq!(frame.body_lines()[0].text, "PROCLAMATION TITLE");
    let body = frame.body();
    let title = &frame.body_lines()[0];
    assert_eq!(
        title.x,
        body.x as i32 + (body.width as i32 - title.width) / 2
    );
    // The reference sits bottom-left: footer origin at the safe corner and
    // its line starting exactly there.
    let safe = frame.safe_area();
    let footer = frame.footer().expect("reference reserves the footer zone");
    assert_eq!(
        (footer.x, footer.y + footer.height),
        (safe.x, safe.y + safe.height)
    );
    assert_eq!(frame.footer_lines().len(), 1);
    assert_eq!(frame.footer_lines()[0].text, "Reference text");
    assert_eq!(frame.footer_lines()[0].x, safe.x as i32);
}

#[test]
fn reading_passage_is_smaller_than_identification_with_the_same_reference() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (passage, _) = render_template(
        &content(
            TemplateKind::ReadingPassage,
            None,
            "Passage text",
            Some("Reference text"),
        ),
        &theme,
        density,
    )
    .unwrap();
    let (identification, _) = render_template(
        &content(
            TemplateKind::ReadingIdentification,
            None,
            "Proclamation title",
            Some("Reference text"),
        ),
        &theme,
        density,
    )
    .unwrap();

    assert_eq!(
        passage.body_lines()[0].height,
        line_height_for(PROVISIONAL_PASSAGE_SIZE)
    );
    assert_eq!(
        identification.body_lines()[0].height,
        line_height_for(PROVISIONAL_DISPLAY_SIZE)
    );
    assert!(
        passage.body_lines()[0].height < identification.body_lines()[0].height,
        "passage stays smaller than identification"
    );
    // Passage body is left-origin; references match bottom-left in both.
    assert_eq!(passage.body_lines()[0].x, passage.safe_area().x as i32);
    for frame in [&passage, &identification] {
        let safe = frame.safe_area();
        let footer = frame.footer().expect("reference reserves a footer");
        assert_eq!(
            (footer.x, footer.y + footer.height),
            (safe.x, safe.y + safe.height)
        );
        assert_eq!(frame.footer_lines()[0].x, safe.x as i32);
    }
}

#[test]
fn psalm_places_header_refrain_and_reference() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, _) = render_template(
        &content(
            TemplateKind::Psalm,
            Some("Authored header"),
            "refrain text",
            Some("Reference text"),
        ),
        &theme,
        density,
    )
    .unwrap();

    assert_eq!(frame.header_lines()[0].text, "Authored header");
    assert_eq!(
        frame.header_lines()[0].height,
        line_height_for(PROVISIONAL_META_SIZE)
    );
    // Refrain keeps authored case and centers; reference sits bottom-left.
    assert_eq!(frame.body_lines()[0].text, "refrain text");
    let body = frame.body();
    let refrain = &frame.body_lines()[0];
    assert_eq!(
        refrain.x,
        body.x as i32 + (body.width as i32 - refrain.width) / 2
    );
    assert_eq!(frame.footer_lines()[0].text, "Reference text");
    assert_eq!(frame.footer_lines()[0].x, frame.safe_area().x as i32);
}

#[test]
fn song_identification_keeps_one_bottom_left_block_with_mixed_sizes() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, diagnostics) = render_template(
        &content(
            TemplateKind::SongIdentification,
            None,
            "Song title",
            Some("Composer name\nArrangement credit"),
        ),
        &theme,
        density,
    )
    .unwrap();

    assert!(diagnostics.warnings.is_empty());
    assert_eq!(frame.header(), None);
    assert!(
        frame.body_lines().is_empty(),
        "title lives in the footer block"
    );
    // One block at the safe corner: title leading large, credits smaller.
    let safe = frame.safe_area();
    let footer = frame.footer().expect("identity block reserves the footer");
    assert_eq!(
        (footer.x, footer.y + footer.height),
        (safe.x, safe.y + safe.height)
    );
    assert_eq!(frame.footer_lines().len(), 3);
    assert_eq!(frame.footer_lines()[0].text, "SONG TITLE");
    assert_eq!(
        frame.footer_lines()[0].height,
        line_height_for(PROVISIONAL_DISPLAY_SIZE)
    );
    assert_eq!(frame.footer_lines()[1].text, "Composer name");
    assert_eq!(frame.footer_lines()[2].text, "Arrangement credit");
    for line in frame.footer_lines() {
        assert_eq!(line.x, safe.x as i32, "block stays bottom-left");
        assert!(
            line.height == line_height_for(PROVISIONAL_DISPLAY_SIZE)
                || line.height == line_height_for(PROVISIONAL_META_SIZE)
        );
    }
    assert_eq!(
        frame.footer_lines()[1].height,
        line_height_for(PROVISIONAL_META_SIZE)
    );
}

#[test]
fn celebration_title_centers_above_bottom_left_date() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, _) = render_template(
        &content(
            TemplateKind::CelebrationTitle,
            None,
            "Celebration title",
            Some("Localized date"),
        ),
        &theme,
        density,
    )
    .unwrap();

    assert_eq!(frame.header(), None);
    assert_eq!(frame.body_lines()[0].text, "CELEBRATION TITLE");
    let safe = frame.safe_area();
    let title = &frame.body_lines()[0];
    assert!(title.x > safe.x as i32, "the title block centers");
    let footer = frame.footer().expect("date reserves the footer");
    assert_eq!(
        (footer.x, footer.y + footer.height),
        (safe.x, safe.y + safe.height)
    );
    assert_eq!(frame.footer_lines()[0].text, "Localized date");
}

#[test]
fn general_text_and_image_use_zones_as_given() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (text, _) = render_template(
        &content(
            TemplateKind::GeneralText,
            Some("General header"),
            "General body",
            Some("General footer"),
        ),
        &theme,
        density,
    )
    .unwrap();
    assert_eq!(text.header_lines()[0].text, "General header");
    assert_eq!(text.body_lines()[0].text, "General body");
    assert_eq!(text.body_lines()[0].x, text.safe_area().x as i32);
    assert_eq!(text.footer_lines()[0].text, "General footer");

    // The image slide carries no body text: the empty body rect reserves the
    // contained foreground bounds between header and caption.
    let (image, _) = render_template(
        &content(
            TemplateKind::GeneralImage,
            Some("Image header"),
            "",
            Some("Image caption"),
        ),
        &theme,
        density,
    )
    .unwrap();
    assert!(image.body_lines().is_empty());
    assert_eq!(image.header_lines()[0].text, "Image header");
    assert_eq!(image.footer_lines()[0].text, "Image caption");
    let safe = image.safe_area();
    assert_eq!(image.body().width, safe.width);
    assert_eq!(
        image.body().height,
        safe.height - image.header().unwrap().height - image.footer().unwrap().height
    );
}

#[test]
fn black_composes_empty_and_fits() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, diagnostics) = render_template(
        &content(TemplateKind::Black, None, "", None),
        &theme,
        density,
    )
    .unwrap();

    assert_eq!(frame.header(), None);
    assert_eq!(frame.footer(), None);
    assert_eq!(frame.body(), frame.safe_area());
    assert!(frame.body_lines().is_empty());
    assert!(diagnostics.warnings.is_empty());
    assert_eq!(frame.pagination().fit, rendering::Fit::Fits);
}

#[test]
fn lyric_ceilings_warn_at_the_boundary_not_one_line_late() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    // Two phrases on two lines: both ceilings quiet.
    let (_, quiet) = render_template(&lyrics("aaa\nbbb"), &theme, density).unwrap();
    assert!(quiet.warnings.is_empty());

    // Three phrases on three lines: the phrase target warns, the line
    // ceiling stays quiet.
    let (_, three) = render_template(&lyrics("aaa\nbbb\nccc"), &theme, density).unwrap();
    assert_eq!(three.warnings.len(), 1);
    assert!(three.warnings[0].contains("two-phrase"));

    // Four phrases on four lines: phrase warning only, still no line warning.
    let (_, four) = render_template(&lyrics("aaa\nbbb\nccc\nddd"), &theme, density).unwrap();
    assert_eq!(four.warnings.len(), 1);
    assert!(four.warnings[0].contains("two-phrase"));

    // Five lines: both warnings ride beside the complete frame.
    let (frame, five) =
        render_template(&lyrics("aaa\nbbb\nccc\nddd\neee"), &theme, density).unwrap();
    assert_eq!(five.warnings.len(), 2);
    assert!(
        five.warnings
            .iter()
            .any(|warning| warning.contains("two-phrase"))
    );
    assert!(
        five.warnings
            .iter()
            .any(|warning| warning.contains("four-line"))
    );
    assert_eq!(frame.pagination().body_lines, 5);
}

#[test]
fn song_title_ceiling_warns_past_three_rendered_lines() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let song = |title: &str| {
        render_template(
            &content(
                TemplateKind::SongIdentification,
                None,
                title,
                Some("Composer name"),
            ),
            &theme,
            density,
        )
        .unwrap()
    };

    let (_, three) = song("line one\nline two\nline three");
    assert!(three.warnings.is_empty());

    let (frame, four) = song("line one\nline two\nline three\nline four");
    assert_eq!(four.warnings.len(), 1);
    assert!(four.warnings[0].contains("three-line"));
    assert_eq!(frame.footer_lines().len(), 5);
}

#[test]
fn celebration_title_ceiling_warns_past_three_rendered_lines() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let celebration = |title: &str| {
        render_template(
            &content(
                TemplateKind::CelebrationTitle,
                None,
                title,
                Some("Localized date"),
            ),
            &theme,
            density,
        )
        .unwrap()
    };

    let (_, three) = celebration("line one\nline two\nline three");
    assert!(three.warnings.is_empty());

    let (frame, four) = celebration("line one\nline two\nline three\nline four");
    assert_eq!(four.warnings.len(), 1);
    assert!(four.warnings[0].contains("three-line"));
    assert_eq!(frame.pagination().body_lines, 4);
}

#[test]
fn headers_are_copied_verbatim_never_derived() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    // Distinctive strings no renderer could invent: what goes in is exactly
    // what composes.
    let (response, _) = render_template(
        &content(
            TemplateKind::Response,
            Some("Header: with punctuation! 123"),
            "body",
            None,
        ),
        &theme,
        density,
    )
    .unwrap();
    assert_eq!(
        response.header_lines()[0].text,
        "Header: with punctuation! 123"
    );
    let (psalm, _) = render_template(
        &content(
            TemplateKind::Psalm,
            Some("Another header (kept whole)"),
            "body",
            Some("ref"),
        ),
        &theme,
        density,
    )
    .unwrap();
    assert_eq!(psalm.header_lines()[0].text, "Another header (kept whole)");
}

#[test]
fn provisional_hierarchy_carries_the_response_baseline() {
    assert_eq!(PROVISIONAL_DISPLAY_SIZE, BODY_FONT_SIZE);
    assert_eq!(BODY_FONT_SIZE, 72);
    assert_eq!(line_height_for(PROVISIONAL_DISPLAY_SIZE), 83);
    assert_eq!(PROVISIONAL_PASSAGE_SIZE, 56);
    assert_eq!(line_height_for(PROVISIONAL_PASSAGE_SIZE), 65);
    assert_eq!(PROVISIONAL_META_SIZE, 40);
    assert_eq!(line_height_for(PROVISIONAL_META_SIZE), 46);

    // The emphasized response body still measures at the baseline token.
    let (frame, _) = render_template(
        &content(TemplateKind::Response, None, "response text", None),
        &Theme::default(),
        TargetDensity::SCALE_100,
    )
    .unwrap();
    assert_eq!(frame.body_lines()[0].height, 83);
}

#[test]
fn templates_are_deterministic_and_density_free() {
    let theme = Theme::default();
    let cases = [
        lyrics("first line\nsecond line"),
        content(
            TemplateKind::SongIdentification,
            None,
            "Song title",
            Some("Composer"),
        ),
    ];
    for case in &cases {
        let (first, first_diagnostics) =
            render_template(case, &theme, TargetDensity::SCALE_100).unwrap();
        let (second, second_diagnostics) =
            render_template(case, &theme, TargetDensity::SCALE_100).unwrap();
        assert_eq!(first.serialize(), second.serialize());
        assert_eq!(first_diagnostics, second_diagnostics);

        let dense = TargetDensity::from_percent(200).unwrap();
        let (at_dense, dense_diagnostics) = render_template(case, &theme, dense).unwrap();
        assert_eq!(first, at_dense);
        assert_eq!(first.serialize(), at_dense.serialize());
        assert_eq!(first_diagnostics, dense_diagnostics);
    }
}

#[test]
fn template_failures_travel_like_interface_failures() {
    let case = lyrics("body");
    let density = TargetDensity::SCALE_100;
    assert_eq!(
        render_template(
            &case,
            &Theme {
                safe_inset_x: 119,
                safe_inset_y: 80,
            },
            density,
        ),
        Err(RenderError::SafeAreaBelowMinimum {
            requested_x: 119,
            requested_y: 80,
            minimum_x: 120,
            minimum_y: 80,
        })
    );
}

#[test]
fn omissions_are_structural_not_conventional() {
    // Reading templates take a resolved title and reference only: no reading
    // number can be named, and no theme picture can arrive as foreground
    // content, because neither is expressible in the template input. The scan
    // below keeps it that way.
    const FORBIDDEN: &[&str] = &[
        "First Reading",
        "Second Reading",
        "Gospel",
        "ThemeImage",
        "theme_image",
        "background_image",
    ];
    let mut checked = 0;
    visit(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut checked,
    );
    assert!(checked > 0, "expected rendering sources to scan");

    fn visit(directory: &std::path::Path, checked: &mut usize) {
        for entry in std::fs::read_dir(directory).expect("rendering src must be readable") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                visit(&path, checked);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                *checked += 1;
                let source = std::fs::read_to_string(&path).expect("readable source");
                for token in FORBIDDEN {
                    assert!(
                        !source.contains(token),
                        "{} expresses {token:?}",
                        path.display()
                    );
                }
            }
        }
    }
}
