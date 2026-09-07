//! T0.4 PR 1 invariance probe: shape one mixed-script string with the
//! candidate canonical stack and pin its serialized logical output.
//!
//! This is qualification evidence, not the renderer: PR 4 owns script
//! itemization, wrapping, and Noto fallback, and PR 8 re-runs this exact test
//! in packaged Windows and Linux builds to close the cross-platform comparison.
//! Positions are font-unit integers, so the artifact must be byte-for-byte
//! stable for identical font bytes, shaper revision, and shaping inputs.
//!
//! The committed `fixtures/shaping-probe.txt` is regenerated only by running
//! with `ORDO_UPDATE_FIXTURES=1`; a font or shaper revision change invalidates
//! `docs/qualification/RENDERING-T0.4.md` first.

use std::fmt::Write as _;

const PROBE_TEXT: &str = "Maayong buntag, mga igsoon! The choir sings \u{2014} awit 123: \"Pagdayeg\" (Salmo 95) \u{4E2D}";

fn serialize_shaping() -> String {
    let font_data = include_bytes!(
        "../../ui/assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf"
    );
    let face = rustybuzz::Face::from_slice(font_data, 0).expect("bundled Atkinson must parse");
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(PROBE_TEXT);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);
    buffer.set_script(rustybuzz::script::LATIN);
    let shaped = rustybuzz::shape(&face, &[], buffer);
    let mut out = String::new();
    let _ = writeln!(out, "font=AtkinsonHyperlegibleNext[wght].ttf");
    let _ = writeln!(out, "direction=ltr script=Latn");
    let _ = writeln!(out, "text={PROBE_TEXT}");
    for (info, position) in shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions().iter())
    {
        let _ = writeln!(
            out,
            "glyph_id={} x_advance={} y_advance={} x_offset={} y_offset={} cluster={}",
            info.glyph_id,
            position.x_advance,
            position.y_advance,
            position.x_offset,
            position.y_offset,
            info.cluster,
        );
    }
    out
}

#[test]
fn shaping_probe_is_stable() {
    let serialized = serialize_shaping();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shaping-probe.txt");
    if std::env::var_os("ORDO_UPDATE_FIXTURES").is_some() {
        std::fs::write(&fixture, &serialized).expect("probe fixture must be writable");
        return;
    }
    let expected = std::fs::read_to_string(&fixture).expect("probe fixture must exist");
    assert_eq!(
        serialized, expected,
        "shaping output drifted; see test docs"
    );
}
