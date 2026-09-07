//! T0.4 PR 6: the golden fixture corpus.
//!
//! Every fixture in `tests/fixtures/rendering/*.json` composes through
//! [`rendering::render_template`] at the default Theme and density, and the
//! completed logical frame plus its diagnostics must match the committed
//! golden in `tests/golden-frames/<name>.txt` byte-for-byte. A failure prints
//! the first differing line with its element and coordinates instead of
//! reporting that two blobs differ.
//!
//! Goldens never update themselves: the committed files change only when a
//! reviewer runs `cargo xtask bless-golden-frames` (which re-runs this
//! harness with `ORDO_UPDATE_FIXTURES=1`) and reviews the resulting diff.
//! Ordinary `cargo test` and `cargo xtask verify` fail on drift.

use std::path::{Path, PathBuf};

use rendering::{ForegroundImage, TargetDensity, TemplateContent, TemplateKind, Theme};

fn workspace_fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/rendering")
}

fn workspace_goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden-frames")
}

fn template_kind(name: &str) -> TemplateKind {
    match name {
        "Lyrics" => TemplateKind::Lyrics,
        "Response" => TemplateKind::Response,
        "ReadingIdentification" => TemplateKind::ReadingIdentification,
        "ReadingPassage" => TemplateKind::ReadingPassage,
        "Psalm" => TemplateKind::Psalm,
        "SongIdentification" => TemplateKind::SongIdentification,
        "CelebrationTitle" => TemplateKind::CelebrationTitle,
        "GeneralText" => TemplateKind::GeneralText,
        "GeneralImage" => TemplateKind::GeneralImage,
        "Black" => TemplateKind::Black,
        _ => panic!("unknown template kind: {name}"),
    }
}

fn opt_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        _ => panic!("expected a string or null, got {value}"),
    }
}

fn load_fixture(path: &Path) -> TemplateContent {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("fixture {} must be readable: {error}", path.display()));
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("fixture must be valid JSON");
    let kind = template_kind(
        parsed
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .expect("fixture needs a kind"),
    );
    let header = opt_string(parsed.get("header").expect("fixture needs a header"));
    let body = parsed
        .get("body")
        .and_then(serde_json::Value::as_str)
        .expect("fixture needs a body")
        .to_owned();
    let footer = opt_string(parsed.get("footer").expect("fixture needs a footer"));
    let image = match parsed.get("image").expect("fixture needs an image") {
        serde_json::Value::Null => None,
        object => Some(ForegroundImage {
            id: object
                .get("id")
                .and_then(serde_json::Value::as_str)
                .expect("image needs an id")
                .to_owned(),
            natural_width: object
                .get("natural_width")
                .and_then(serde_json::Value::as_u64)
                .expect("image needs a natural_width") as u32,
            natural_height: object
                .get("natural_height")
                .and_then(serde_json::Value::as_u64)
                .expect("image needs a natural_height") as u32,
        }),
    };
    TemplateContent {
        kind,
        header,
        body,
        footer,
        image,
    }
}

/// The pinned artifact for one fixture: the serialized logical frame plus
/// the diagnostics riding beside it. Ceiling warnings leave the geometry
/// untouched, so pinning the frame alone would let a lost warning pass.
fn format_golden(frame: &rendering::Frame, diagnostics: &rendering::Diagnostics) -> String {
    let mut out = frame.serialize();
    out.push_str(&format!("warnings {}\n", diagnostics.warnings.len()));
    for warning in &diagnostics.warnings {
        out.push_str(&format!("warning {warning}\n"));
    }
    out
}

fn fixture_names() -> Vec<String> {
    let mut names = Vec::new();
    let entries = std::fs::read_dir(workspace_fixtures_dir()).expect("fixtures dir must exist");
    for entry in entries {
        let path = entry.expect("readable entry").path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            names.push(
                path.file_stem()
                    .expect("fixture needs a stem")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    names.sort();
    assert!(
        !names.is_empty(),
        "expected golden fixtures in tests/fixtures/rendering/"
    );
    names
}

fn render_fixture(name: &str) -> String {
    let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.json")));
    let (frame, diagnostics) =
        rendering::render_template(&fixture, &Theme::default(), TargetDensity::SCALE_100)
            .unwrap_or_else(|error| panic!("fixture {name} must compose: {error}"));
    format_golden(&frame, &diagnostics)
}

/// Fail with the first differing line — which names the element and its
/// coordinates — instead of reporting that two blobs differ.
fn assert_golden_eq(name: &str, actual: &str, expected: &str) {
    if actual == expected {
        return;
    }
    let actual_lines: Vec<&str> = actual.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();
    let common = actual_lines
        .iter()
        .zip(expected_lines.iter())
        .position(|(a, b)| a != b);
    match common {
        Some(index) => panic!(
            "golden drift in {name} at line {}:\n  expected: {}\n  actual:   {}\n\
             That line names the element and coordinates that moved; \
             review the composition change, then re-bless with \
             `cargo xtask bless-golden-frames` only if the new composition is correct.",
            index + 1,
            expected_lines[index],
            actual_lines[index],
        ),
        None => panic!(
            "golden drift in {name}: same prefix but different length \
             (expected {} lines, actual {} lines). \
             The trailing lines name the added or removed element; \
             review, then re-bless with `cargo xtask bless-golden-frames` only if correct.",
            expected_lines.len(),
            actual_lines.len(),
        ),
    }
}

#[test]
fn golden_frames_match_the_committed_corpus() {
    let bless = std::env::var_os("ORDO_UPDATE_FIXTURES").is_some();
    for name in fixture_names() {
        let rendered = render_fixture(&name);
        let golden_path = workspace_goldens_dir().join(format!("{name}.txt"));
        if bless {
            std::fs::create_dir_all(workspace_goldens_dir()).expect("goldens dir must be writable");
            std::fs::write(&golden_path, &rendered).unwrap_or_else(|error| {
                panic!("golden {} must be writable: {error}", golden_path.display())
            });
            continue;
        }
        let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!(
                "missing golden for {name}: run `cargo xtask bless-golden-frames` to create it, then review the diff"
            )
        });
        assert_golden_eq(&name, &rendered, &expected);
    }
}

#[test]
fn corpus_covers_every_template_kind() {
    let mut kinds = std::collections::BTreeSet::new();
    for name in fixture_names() {
        let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.json")));
        kinds.insert(format!("{:?}", fixture.kind));
    }
    for expected in [
        "Lyrics",
        "Response",
        "ReadingIdentification",
        "ReadingPassage",
        "Psalm",
        "SongIdentification",
        "CelebrationTitle",
        "GeneralText",
        "GeneralImage",
        "Black",
    ] {
        assert!(
            kinds.contains(expected),
            "corpus is missing template kind {expected}: {kinds:?}"
        );
    }
}

#[test]
fn lyrics_cover_one_to_four_lines() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    for (name, expected_lines) in [
        ("lyrics-cebuano-one-line", 1),
        ("lyrics-english-two-line", 2),
        ("lyrics-tagalog-three-line", 3),
        ("lyrics-mixed-four-line", 4),
    ] {
        let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.json")));
        let (frame, _) = rendering::render_template(&fixture, &theme, density)
            .unwrap_or_else(|error| panic!("{name} must compose: {error}"));
        assert_eq!(
            frame.body_lines().len(),
            expected_lines,
            "{name} must stay on {expected_lines} rendered lines without wrapping"
        );
    }
}

#[test]
fn absent_and_empty_headers_compose_differently() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let absent = load_fixture(&workspace_fixtures_dir().join("response-header-absent.json"));
    let empty = load_fixture(&workspace_fixtures_dir().join("response-header-empty.json"));
    let (absent_frame, _) = rendering::render_template(&absent, &theme, density).unwrap();
    let (empty_frame, _) = rendering::render_template(&empty, &theme, density).unwrap();
    assert_eq!(absent_frame.header(), None);
    assert!(empty_frame.header().is_some());
    assert_ne!(
        absent_frame.serialize(),
        empty_frame.serialize(),
        "absent and empty headers must stay distinct fixtures"
    );
}

#[test]
fn overflow_fixtures_warn_beside_complete_frames() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    for name in ["lyrics-overflow-warning", "celebration-title-overflow"] {
        let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.json")));
        let (frame, diagnostics) = rendering::render_template(&fixture, &theme, density)
            .unwrap_or_else(|error| panic!("{name} must compose: {error}"));
        assert!(
            !diagnostics.warnings.is_empty(),
            "{name} must carry its overflow warning beside the complete frame"
        );
        assert!(
            !frame.body_lines().is_empty() || !frame.footer_lines().is_empty(),
            "{name} must keep every line beside its warning"
        );
    }
}

#[test]
fn missing_glyph_fixture_shapes_through_the_bundled_fallback() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let fixture = load_fixture(&workspace_fixtures_dir().join("lyrics-missing-glyph.json"));
    let (frame, _) = rendering::render_template(&fixture, &theme, density).unwrap();
    let glyphs: Vec<_> = frame
        .body_lines()
        .iter()
        .flat_map(|line| line.glyphs.iter())
        .collect();
    assert!(!glyphs.is_empty());
    assert!(
        glyphs
            .iter()
            .any(|glyph| glyph.font == rendering::fonts::CanonicalFont::Fallback),
        "absent glyphs must shape through the bundled fallback, never an installed font"
    );
}

#[test]
fn long_word_fixture_survives_between_character_breaks() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let fixture = load_fixture(&workspace_fixtures_dir().join("lyrics-long-word.json"));
    let (frame, _) = rendering::render_template(&fixture, &theme, density).unwrap();
    assert!(
        frame.body_lines().len() > 1,
        "the overlong word must break rather than clip"
    );
    let glyphs: usize = frame
        .body_lines()
        .iter()
        .map(|line| line.glyphs.len())
        .sum();
    assert_eq!(glyphs, 60, "every character must survive the break");
}

#[test]
fn label_off_returns_header_space_to_the_body() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let labeled = load_fixture(&workspace_fixtures_dir().join("lyrics-cebuano-one-line.json"));
    let unlabeled = load_fixture(&workspace_fixtures_dir().join("lyrics-label-off.json"));
    let (labeled_frame, _) = rendering::render_template(&labeled, &theme, density).unwrap();
    let (unlabeled_frame, _) = rendering::render_template(&unlabeled, &theme, density).unwrap();
    assert!(labeled_frame.header().is_some());
    assert_eq!(unlabeled_frame.header(), None);
    assert_eq!(unlabeled_frame.body(), unlabeled_frame.safe_area());
    assert!(unlabeled_frame.body().height > labeled_frame.body().height);
}

#[test]
fn black_stays_distinguishable_and_general_image_carries_geometry() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let black = load_fixture(&workspace_fixtures_dir().join("black.json"));
    let (black_frame, black_diagnostics) =
        rendering::render_template(&black, &theme, density).unwrap();
    assert_eq!(black_frame.background(), rendering::SlideBackground::Black);
    assert!(black_diagnostics.warnings.is_empty());

    let image = load_fixture(&workspace_fixtures_dir().join("general-image.json"));
    let (image_frame, _) = rendering::render_template(&image, &theme, density).unwrap();
    let placed = image_frame
        .image()
        .expect("image slide carries its element");
    assert_eq!(placed.id, "fixture-image-800x600");
    assert!(image_frame.serialize().contains(&format!(
        "\nimage id=fixture-image-800x600 {} {} {} {}\n",
        placed.bounds.x, placed.bounds.y, placed.bounds.width, placed.bounds.height
    )));
}
