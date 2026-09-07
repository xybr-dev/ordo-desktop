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

/// Fixture files stay plain JSON, but the harness reads them with this
/// std-only subset parser rather than a JSON crate: `RENDERING-T0.4.md` caps
/// `rendering`'s direct dependencies at the four qualified edges, and a fifth
/// edge — even a dev-dependency — would invalidate that qualification. The
/// parser covers exactly what the schema needs: objects, strings with
/// escapes, `null`, and unsigned integers. Anything else is a loud error
/// naming the byte offset, never a silent misread.
struct FixtureReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl FixtureReader<'_> {
    fn error(&self, expected: &str) -> String {
        format!("byte {}: expected {expected}", self.pos)
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn expect_byte(&mut self, byte: u8, what: &str) -> Result<(), String> {
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(what))
        }
    }

    fn peek_byte(&mut self) -> Result<u8, String> {
        self.skip_ws();
        self.bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| self.error("more input"))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect_byte(b'"', "`\"`")?;
        let mut out = String::new();
        loop {
            let byte = *self
                .bytes
                .get(self.pos)
                .ok_or_else(|| self.error("closing `\"`"))?;
            match byte {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let escaped = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| self.error("an escape"))?;
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.parse_unicode_escape()?),
                        _ => return Err(self.error("a valid escape")),
                    }
                }
                0x00..=0x1F => return Err(self.error("an unescaped character")),
                // Raw UTF-8 (Cebuano text, em-dash, CJK, ...) passes through;
                // `raw` was already validated as UTF-8 on read.
                _ => {
                    let rest = std::str::from_utf8(&self.bytes[self.pos..])
                        .map_err(|_| self.error("valid UTF-8"))?;
                    let c = rest
                        .chars()
                        .next()
                        .ok_or_else(|| self.error("a character"))?;
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.error("four hex digits"));
        }
        let mut value = 0u32;
        for i in 0..4 {
            let digit = (self.bytes[self.pos + i] as char)
                .to_digit(16)
                .ok_or_else(|| self.error("four hex digits"))?;
            value = value * 16 + digit;
        }
        self.pos += 4;
        Ok(value)
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let unit = self.parse_hex4()?;
        if (0xD800..0xDC00).contains(&unit) {
            // High surrogate: a low surrogate must follow as a second escape.
            if self.bytes.get(self.pos) == Some(&b'\\')
                && self.bytes.get(self.pos + 1) == Some(&b'u')
            {
                self.pos += 2;
                let low = self.parse_hex4()?;
                if (0xDC00..0xE000).contains(&low) {
                    let scalar = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                    return char::from_u32(scalar).ok_or_else(|| self.error("a valid scalar"));
                }
            }
            return Err(self.error("a low surrogate"));
        }
        if (0xDC00..0xE000).contains(&unit) {
            return Err(self.error("a high surrogate first"));
        }
        char::from_u32(unit).ok_or_else(|| self.error("a valid scalar value"))
    }

    fn parse_null(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.bytes.get(self.pos..self.pos + 4) == Some(b"null") {
            self.pos += 4;
            Ok(())
        } else {
            Err(self.error("`null`"))
        }
    }

    fn parse_u64(&mut self) -> Result<u64, String> {
        self.skip_ws();
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(self.error("a number"));
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| self.error("a number"))
    }

    /// A string or `null`: `None` is absent chrome, `Some("")` reserves the
    /// zone, and the two must never collapse into one another.
    fn parse_opt_string(&mut self) -> Result<Option<String>, String> {
        match self.peek_byte()? {
            b'n' => {
                self.parse_null()?;
                Ok(None)
            }
            b'"' => Ok(Some(self.parse_string()?)),
            _ => Err(self.error("a string or `null`")),
        }
    }

    fn parse_image(&mut self) -> Result<Option<ForegroundImage>, String> {
        if self.peek_byte()? == b'n' {
            self.parse_null()?;
            return Ok(None);
        }
        self.expect_byte(b'{', "`{`")?;
        let mut id: Option<String> = None;
        let mut natural_width: Option<u64> = None;
        let mut natural_height: Option<u64> = None;
        let mut first = true;
        loop {
            self.skip_ws();
            if self.bytes.get(self.pos) == Some(&b'}') {
                self.pos += 1;
                break;
            }
            if !first {
                self.expect_byte(b',', "`,` or `}`")?;
                self.skip_ws();
                if self.bytes.get(self.pos) == Some(&b'}') {
                    return Err(self.error("a key (no trailing comma)"));
                }
            }
            first = false;
            let key = self.parse_string()?;
            self.expect_byte(b':', "`:`")?;
            match key.as_str() {
                "id" => {
                    if id.is_some() {
                        return Err(format!("byte {}: duplicate image key \"id\"", self.pos));
                    }
                    if self.peek_byte()? != b'"' {
                        return Err(self.error("a string id"));
                    }
                    id = Some(self.parse_string()?);
                }
                "natural_width" => {
                    if natural_width.is_some() {
                        return Err(format!(
                            "byte {}: duplicate image key \"natural_width\"",
                            self.pos
                        ));
                    }
                    natural_width = Some(self.parse_u64()?);
                }
                "natural_height" => {
                    if natural_height.is_some() {
                        return Err(format!(
                            "byte {}: duplicate image key \"natural_height\"",
                            self.pos
                        ));
                    }
                    natural_height = Some(self.parse_u64()?);
                }
                _ => return Err(format!("byte {}: unknown image key {key:?}", self.pos)),
            }
        }
        let (Some(id), Some(natural_width), Some(natural_height)) =
            (id, natural_width, natural_height)
        else {
            return Err(format!(
                "byte {}: image needs id, natural_width, and natural_height",
                self.pos
            ));
        };
        Ok(Some(ForegroundImage {
            id,
            natural_width: u32::try_from(natural_width)
                .map_err(|_| format!("byte {}: natural_width exceeds u32", self.pos))?,
            natural_height: u32::try_from(natural_height)
                .map_err(|_| format!("byte {}: natural_height exceeds u32", self.pos))?,
        }))
    }

    fn parse_top(&mut self) -> Result<TemplateContent, String> {
        self.expect_byte(b'{', "`{`")?;
        let mut kind: Option<TemplateKind> = None;
        let mut header: Option<Option<String>> = None;
        let mut body: Option<String> = None;
        let mut footer: Option<Option<String>> = None;
        let mut image: Option<Option<ForegroundImage>> = None;
        let mut first = true;
        loop {
            self.skip_ws();
            if self.bytes.get(self.pos) == Some(&b'}') {
                self.pos += 1;
                break;
            }
            if !first {
                self.expect_byte(b',', "`,` or `}`")?;
                self.skip_ws();
                if self.bytes.get(self.pos) == Some(&b'}') {
                    return Err(self.error("a key (no trailing comma)"));
                }
            }
            first = false;
            let key = self.parse_string()?;
            self.expect_byte(b':', "`:`")?;
            match key.as_str() {
                "kind" => {
                    if kind.is_some() {
                        return Err(format!("byte {}: duplicate key \"kind\"", self.pos));
                    }
                    if self.peek_byte()? != b'"' {
                        return Err(self.error("a string kind"));
                    }
                    kind = Some(template_kind(&self.parse_string()?)?);
                }
                "header" => {
                    if header.is_some() {
                        return Err(format!("byte {}: duplicate key \"header\"", self.pos));
                    }
                    header = Some(self.parse_opt_string()?);
                }
                "body" => {
                    if body.is_some() {
                        return Err(format!("byte {}: duplicate key \"body\"", self.pos));
                    }
                    if self.peek_byte()? != b'"' {
                        return Err(self.error("a string body"));
                    }
                    body = Some(self.parse_string()?);
                }
                "footer" => {
                    if footer.is_some() {
                        return Err(format!("byte {}: duplicate key \"footer\"", self.pos));
                    }
                    footer = Some(self.parse_opt_string()?);
                }
                "image" => {
                    if image.is_some() {
                        return Err(format!("byte {}: duplicate key \"image\"", self.pos));
                    }
                    image = Some(self.parse_image()?);
                }
                _ => return Err(format!("byte {}: unknown fixture key {key:?}", self.pos)),
            }
        }
        let (Some(kind), Some(header), Some(body), Some(footer), Some(image)) =
            (kind, header, body, footer, image)
        else {
            return Err(format!(
                "byte {}: fixture needs kind, header, body, footer, and image",
                self.pos
            ));
        };
        Ok(TemplateContent {
            kind,
            header,
            body,
            footer,
            image,
        })
    }
}

fn parse_fixture_json(raw: &str) -> Result<TemplateContent, String> {
    let mut reader = FixtureReader {
        bytes: raw.as_bytes(),
        pos: 0,
    };
    let content = reader.parse_top()?;
    reader.skip_ws();
    if reader.pos != reader.bytes.len() {
        return Err(reader.error("end of input"));
    }
    Ok(content)
}

fn load_fixture(path: &Path) -> TemplateContent {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("fixture {} must be readable: {error}", path.display()));
    parse_fixture_json(&raw)
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
fn fixture_parser_round_trips_escapes_and_surrogates() {
    let content = parse_fixture_json(
        "{\"kind\": \"Lyrics\", \"header\": \"A\\nB\\\"C\\\\D \\u00e9 \\ud83d\\ude00\", \"body\": \"x\", \"footer\": null, \"image\": null}",
    )
    .expect("escapes must parse");
    assert_eq!(content.header.as_deref(), Some("A\nB\"C\\D é 😀"));
    assert_eq!(content.footer, None);
}

#[test]
fn fixture_parser_keeps_absent_and_empty_headers_distinct() {
    let absent = parse_fixture_json(
        "{\"kind\": \"Response\", \"header\": null, \"body\": \"x\", \"footer\": null, \"image\": null}",
    )
    .expect("null header must parse");
    assert_eq!(absent.header, None);
    let empty = parse_fixture_json(
        "{\"kind\": \"Response\", \"header\": \"\", \"body\": \"x\", \"footer\": null, \"image\": null}",
    )
    .expect("empty header must parse");
    assert_eq!(empty.header, Some(String::new()));
}

#[test]
fn fixture_parser_rejects_malformed_input() {
    // Each case must fail loudly rather than misread: trailing garbage,
    // unknown and missing keys, wrong types, bad escapes, and trailing
    // commas all invalidate the fixture.
    let bad = [
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": \"x\", \"footer\": null, \"image\": null} trailing",
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": \"x\", \"footer\": null, \"image\": null, \"extra\": 1}",
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": \"x\", \"footer\": null}",
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": 7, \"footer\": null, \"image\": null}",
        "{\"kind\": \"Nope\", \"header\": null, \"body\": \"x\", \"footer\": null, \"image\": null}",
        "{\"kind\": \"Lyrics\", \"header\": \"\\q\", \"body\": \"x\", \"footer\": null, \"image\": null}",
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": \"x\", \"footer\": null, \"image\": null,}",
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": \"unterminated, \"footer\": null, \"image\": null}",
        "{\"kind\": \"Lyrics\", \"header\": null, \"body\": \"x\", \"footer\": null, \"image\": {\"id\": \"a\"}}",
    ];
    for raw in bad {
        assert!(
            parse_fixture_json(raw).is_err(),
            "malformed fixture must fail: {raw}"
        );
    }
}
