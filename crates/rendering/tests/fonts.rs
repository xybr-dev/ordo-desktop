//! T0.4 PR 2: the canonical path loads exactly the two bundled faces, with
//! Noto Sans as the missing-glyph fallback and no installed-font consultation.
//!
//! Greek alpha and the peso sign are absent from Atkinson but carried by
//! Noto, so they pin the fallback direction. A glyph neither face carries
//! (the shaping probe's Han character) still resolves to bundled Noto rather
//! than anything installed. The source scan is the check that no system font
//! enumeration exists in `rendering`, not a comment claiming so.

use std::path::Path;

use rendering::fonts::{self, ATKINSON_HYPERLEGIBLE_NEXT, CanonicalFont, NOTO_SANS};

/// Greek alpha and the peso sign: absent from Atkinson, carried by Noto.
const FALLBACK_CHARACTERS: [char; 2] = ['\u{3B1}', '\u{20B1}'];

/// The shaping probe's Han character: `.notdef` in both bundled faces.
const NOWHERE_CHARACTER: char = '\u{4E2D}';

fn glyph_id(font: &[u8], character: char) -> Option<u16> {
    ttf_parser::Face::parse(font, 0)
        .ok()
        .and_then(|face| face.glyph_index(character))
        .map(|id| id.0)
}

#[test]
fn glyphs_absent_from_primary_resolve_through_bundled_fallback() {
    for character in FALLBACK_CHARACTERS {
        assert_eq!(
            glyph_id(ATKINSON_HYPERLEGIBLE_NEXT, character),
            None,
            "{character:?} must be absent from the primary face"
        );
        assert!(
            glyph_id(NOTO_SANS, character).is_some_and(|id| id != 0),
            "{character:?} must be carried by the bundled fallback"
        );
        assert_eq!(
            fonts::resolve_font_for(character),
            CanonicalFont::Fallback,
            "{character:?} must resolve to the bundled fallback"
        );
    }
}

#[test]
fn primary_glyphs_stay_on_the_primary_face() {
    for character in ['A', 'a', '\u{2014}'] {
        assert_eq!(
            fonts::resolve_font_for(character),
            CanonicalFont::Primary,
            "{character:?} must stay on the primary face"
        );
    }
}

#[test]
fn glyphs_neither_face_carries_still_resolve_to_bundled_bytes() {
    assert_eq!(
        glyph_id(ATKINSON_HYPERLEGIBLE_NEXT, NOWHERE_CHARACTER),
        None
    );
    assert_eq!(glyph_id(NOTO_SANS, NOWHERE_CHARACTER), None);
    assert_eq!(
        fonts::resolve_font_for(NOWHERE_CHARACTER),
        CanonicalFont::Fallback,
        "a missing-everywhere glyph must fall back to bundled Noto, never to an installed font"
    );
}

#[test]
fn rendering_declares_no_system_font_enumeration() {
    const FORBIDDEN: &[&str] = &[
        "fontdb",
        "FontDatabase",
        "/usr/share/fonts",
        "Windows/Fonts",
        "DirectWrite",
        "fontconfig",
        "CoreText",
    ];
    let mut checked = 0;
    visit(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut checked,
    );
    assert!(checked > 0, "expected rendering sources to scan");

    fn visit(directory: &Path, checked: &mut usize) {
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
                        "{} consults system fonts via {token:?}",
                        path.display()
                    );
                }
            }
        }
    }
}
