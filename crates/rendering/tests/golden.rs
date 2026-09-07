//! T0.4 PR 6: the golden fixture corpus.
//!
//! Every fixture in `tests/fixtures/rendering/*.txt` composes through
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

fn template_kind(name: &str) -> Result<TemplateKind, String> {
    match name {
        "Lyrics" => Ok(TemplateKind::Lyrics),
        "Response" => Ok(TemplateKind::Response),
        "ReadingIdentification" => Ok(TemplateKind::ReadingIdentification),
        "ReadingPassage" => Ok(TemplateKind::ReadingPassage),
        "Psalm" => Ok(TemplateKind::Psalm),
        "SongIdentification" => Ok(TemplateKind::SongIdentification),
        "CelebrationTitle" => Ok(TemplateKind::CelebrationTitle),
        "GeneralText" => Ok(TemplateKind::GeneralText),
        "GeneralImage" => Ok(TemplateKind::GeneralImage),
        "Black" => Ok(TemplateKind::Black),
        _ => Err(format!("unknown template kind: {name}")),
    }
}

/// Fixtures use a deliberately simple line format — not JSON — so the harness
/// needs no parser crate: `RENDERING-T0.4.md` caps `rendering`'s direct
/// dependencies at the four qualified edges, and a fifth edge, even a
/// dev-dependency, would invalidate that qualification. Layout, in fixed
/// order, all lines LF:
///
/// ```text
/// ordo-fixture/1
/// kind:Lyrics
/// header:Bersikulo
/// body-lines:1
/// Maayong buntag, mga igsoon, pagdayeg sa Ginoo!
/// footer-absent
/// image-absent
/// ```
///
/// `header-absent` collapses the zone while `header:` alone reserves it with
/// an empty string; the two must never collapse into one another. `footer`
/// works the same way with `footer-lines:M` plus M raw lines, and an image is
/// `image:<id>` plus `image-size:WxH`. Counts are plain decimal with no
/// leading zeros. Header, body, footer, and image-id lines are raw: quotes,
/// commas, em-dashes, and non-Latin scripts pass through untouched, so
/// nothing needs escaping and nothing can misread silently. Anything out of
/// order, miscounted, or trailing is a loud error naming the line number.
fn strict_u32(text: &str, what: &str, line: usize) -> Result<u32, String> {
    let canonical = !text.is_empty()
        && (text.len() == 1 || !text.starts_with('0'))
        && text.bytes().all(|byte| byte.is_ascii_digit());
    if !canonical {
        return Err(format!(
            "fixture line {line}: {what} must be a plain decimal integer, got {text:?}"
        ));
    }
    text.parse()
        .map_err(|_| format!("fixture line {line}: {what} exceeds u32"))
}

fn parse_fixture(raw: &str) -> Result<TemplateContent, String> {
    let lines: Vec<&str> = raw.lines().collect();
    let mut pos = 0;
    // Yields the 1-based line number with the line, so errors name their
    // line without reading `pos` past the closure's borrow.
    let mut next = |what: &str| -> Result<(usize, &str), String> {
        if pos >= lines.len() {
            return Err(format!("fixture ends early: missing {what}"));
        }
        let at = pos + 1;
        let line = lines[pos];
        pos += 1;
        Ok((at, line))
    };

    if next("magic")?.1 != "ordo-fixture/1" {
        return Err("fixture line 1: expected `ordo-fixture/1`".to_owned());
    }
    let (kind_at, kind_line) = next("kind")?;
    let kind = kind_line.strip_prefix("kind:").ok_or_else(|| {
        format!("fixture line {kind_at}: expected `kind:<Kind>`, got {kind_line:?}")
    })?;
    let kind = template_kind(kind).map_err(|error| format!("fixture line {kind_at}: {error}"))?;

    let (header_at, header_line) = next("header")?;
    let header = if header_line == "header-absent" {
        None
    } else if let Some(text) = header_line.strip_prefix("header:") {
        Some(text.to_owned())
    } else {
        return Err(format!(
            "fixture line {header_at}: expected `header:<text>` or `header-absent`, got {header_line:?}"
        ));
    };

    let (count_at, count_line) = next("body count")?;
    let body_count = count_line
        .strip_prefix("body-lines:")
        .ok_or_else(|| format!("fixture line {count_at}: expected `body-lines:<n>`"))?;
    let body_count = strict_u32(body_count, "body-lines", count_at)? as usize;
    let mut body = Vec::with_capacity(body_count);
    for _ in 0..body_count {
        body.push(next("body line")?.1.to_owned());
    }
    let body = body.join("\n");

    let (footer_at, footer_line) = next("footer")?;
    let footer = match footer_line {
        "footer-absent" => None,
        line => {
            let count = line.strip_prefix("footer-lines:").ok_or_else(|| {
                format!(
                    "fixture line {footer_at}: expected `footer-lines:<n>` or `footer-absent`, got {line:?}"
                )
            })?;
            let count = strict_u32(count, "footer-lines", footer_at)? as usize;
            let mut footer = Vec::with_capacity(count);
            for _ in 0..count {
                footer.push(next("footer line")?.1.to_owned());
            }
            Some(footer.join("\n"))
        }
    };

    let (image_at, image_line) = next("image")?;
    let image = match image_line {
        "image-absent" => None,
        line => {
            let id = line.strip_prefix("image:").ok_or_else(|| {
                format!(
                    "fixture line {image_at}: expected `image:<id>` or `image-absent`, got {line:?}"
                )
            })?;
            if id.is_empty() {
                return Err(format!(
                    "fixture line {image_at}: image id must not be empty"
                ));
            }
            let (size_at, size_line) = next("image size")?;
            let size = size_line
                .strip_prefix("image-size:")
                .ok_or_else(|| format!("fixture line {size_at}: expected `image-size:<w>x<h>`"))?;
            let (width, height) = size
                .split_once('x')
                .ok_or_else(|| format!("fixture line {size_at}: expected `image-size:<w>x<h>`"))?;
            Some(ForegroundImage {
                id: id.to_owned(),
                natural_width: strict_u32(width, "image width", size_at)?,
                natural_height: strict_u32(height, "image height", size_at)?,
            })
        }
    };

    if pos != lines.len() {
        return Err(format!("fixture line {}: trailing content", pos + 1));
    }
    Ok(TemplateContent {
        kind,
        header,
        body,
        footer,
        image,
    })
}

fn load_fixture(path: &Path) -> TemplateContent {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("fixture {} must be readable: {error}", path.display()));
    parse_fixture(&raw)
        .unwrap_or_else(|error| panic!("fixture {} is invalid: {error}", path.display()))
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
        if path.extension().is_some_and(|extension| extension == "txt") {
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
    let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.txt")));
    let (frame, diagnostics) =
        rendering::render_template(&fixture, &Theme::default(), TargetDensity::SCALE_100)
            .unwrap_or_else(|error| panic!("fixture {name} must compose: {error}"));
    format_golden(&frame, &diagnostics)
}

/// The mismatch message for two serializations, or `None` when they match.
/// Every branch prints the first unmatched line — which names the element
/// and its coordinates — instead of reporting that two blobs differ.
fn golden_mismatch_message(name: &str, actual: &str, expected: &str) -> Option<String> {
    if actual == expected {
        return None;
    }
    let actual_lines: Vec<&str> = actual.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();
    let rebless = "review the composition change, then re-bless with \
        `cargo xtask bless-golden-frames` only if the new composition is correct.";
    if let Some(index) = actual_lines
        .iter()
        .zip(expected_lines.iter())
        .position(|(a, b)| a != b)
    {
        return Some(format!(
            "golden drift in {name} at line {}:\n  expected: {}\n  actual:   {}\n\
             That line names the element and coordinates that moved; {rebless}",
            index + 1,
            expected_lines[index],
            actual_lines[index],
        ));
    }
    // One serialization is a strict prefix of the other: the first trailing
    // line names the added or removed element and its coordinates.
    let shared = actual_lines.len().min(expected_lines.len());
    let (side, line) = if actual_lines.len() > expected_lines.len() {
        ("actual-only", actual_lines[shared])
    } else {
        ("expected-only", expected_lines[shared])
    };
    Some(format!(
        "golden drift in {name}: same first {shared} lines but different length \
         (expected {} lines, actual {} lines).\n  first {side} line {}: {line}\n\
         That line names the added or removed element and its coordinates; {rebless}",
        expected_lines.len(),
        actual_lines.len(),
        shared + 1,
    ))
}

/// Fail with the first differing line — which names the element and its
/// coordinates — instead of reporting that two blobs differ.
fn assert_golden_eq(name: &str, actual: &str, expected: &str) {
    if let Some(message) = golden_mismatch_message(name, actual, expected) {
        panic!("{message}");
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
        let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.txt")));
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
        let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.txt")));
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
    let absent = load_fixture(&workspace_fixtures_dir().join("response-header-absent.txt"));
    let empty = load_fixture(&workspace_fixtures_dir().join("response-header-empty.txt"));
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
        let fixture = load_fixture(&workspace_fixtures_dir().join(format!("{name}.txt")));
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
    let fixture = load_fixture(&workspace_fixtures_dir().join("lyrics-missing-glyph.txt"));
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
    let fixture = load_fixture(&workspace_fixtures_dir().join("lyrics-long-word.txt"));
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
    let labeled = load_fixture(&workspace_fixtures_dir().join("lyrics-cebuano-one-line.txt"));
    let unlabeled = load_fixture(&workspace_fixtures_dir().join("lyrics-label-off.txt"));
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
    let black = load_fixture(&workspace_fixtures_dir().join("black.txt"));
    let (black_frame, black_diagnostics) =
        rendering::render_template(&black, &theme, density).unwrap();
    assert_eq!(black_frame.background(), rendering::SlideBackground::Black);
    assert!(black_diagnostics.warnings.is_empty());

    let image = load_fixture(&workspace_fixtures_dir().join("general-image.txt"));
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

#[test]
fn length_only_drift_prints_the_trailing_element() {
    let base = "ordo-canonical-frame/2\nlogical 1920 1080\nwarnings 0\n";
    let appended = "ordo-canonical-frame/2\nlogical 1920 1080\nwarnings 0\nwarning lyric body has 5 rendered lines against a four-line ceiling; split before shrinking\n";
    assert!(golden_mismatch_message("demo", base, base).is_none());

    // An added trailing line: the message prints it with its line number.
    let message =
        golden_mismatch_message("demo", appended, base).expect("appended line must mismatch");
    assert!(
        message.contains("line 4: warning lyric body has 5 rendered lines"),
        "added element must print: {message}"
    );
    assert!(
        message.contains("actual-only"),
        "side must print: {message}"
    );

    // A removed trailing line: the message prints the expected-only line.
    let message =
        golden_mismatch_message("demo", base, appended).expect("removed line must mismatch");
    assert!(
        message.contains("line 4: warning lyric body has 5 rendered lines"),
        "removed element must print: {message}"
    );
    assert!(
        message.contains("expected-only"),
        "side must print: {message}"
    );
}

#[test]
fn fixture_format_keeps_absent_and_empty_headers_distinct() {
    let template = |header_line: &str| {
        format!(
            "ordo-fixture/1\nkind:Response\n{header_line}\nbody-lines:1\nresponse text\nfooter-absent\nimage-absent\n"
        )
    };
    let absent = parse_fixture(&template("header-absent")).expect("absence must parse");
    assert_eq!(absent.header, None);
    let empty = parse_fixture(&template("header:")).expect("empty header must parse");
    assert_eq!(empty.header, Some(String::new()));
    assert_ne!(absent.header, empty.header);
}

#[test]
fn fixture_format_carries_raw_text_and_images_verbatim() {
    let raw = "ordo-fixture/1\nkind:Psalm\nheader:SALMO — \"awit\"\nbody-lines:2\nDayeg α ₱ 中\nsecond, line!\nfooter-lines:1\nSalmo 95:1-3\nimage:asset-9\nimage-size:800x600\n";
    let content = parse_fixture(raw).expect("raw text must parse");
    assert_eq!(content.kind, TemplateKind::Psalm);
    assert_eq!(content.header.as_deref(), Some("SALMO — \"awit\""));
    assert_eq!(content.body, "Dayeg α ₱ 中\nsecond, line!");
    assert_eq!(content.footer.as_deref(), Some("Salmo 95:1-3"));
    let image = content.image.expect("image must parse");
    assert_eq!(image.id, "asset-9");
    assert_eq!((image.natural_width, image.natural_height), (800, 600));
}

#[test]
fn fixture_format_rejects_malformed_input() {
    // Each case must fail loudly rather than misread: bad magic, unknown
    // kinds, misordered sections, non-canonical counts, miscounted bodies,
    // bad image sizes, and trailing content all invalidate the fixture.
    let good =
        "ordo-fixture/1\nkind:Lyrics\nheader:Verse\nbody-lines:1\nx\nfooter-absent\nimage-absent\n";
    let bad = [
        good.replacen("ordo-fixture/1", "ordo-fixture/2", 1),
        good.replacen("kind:Lyrics", "kind:Nope", 1),
        good.replacen("header:Verse\nbody-lines:1", "body-lines:1\nheader:Verse", 1),
        good.replacen("body-lines:1", "body-lines:01", 1),
        good.replacen("body-lines:1", "body-lines:2", 1),
        format!("{good}extra\n"),
        "ordo-fixture/1\nkind:GeneralImage\nheader:H\nbody-lines:0\nfooter-lines:1\nC\nimage:a\nimage-size:800x\n".to_owned(),
    ];
    for raw in bad {
        assert!(
            parse_fixture(&raw).is_err(),
            "malformed fixture must fail: {raw:?}"
        );
    }
}
