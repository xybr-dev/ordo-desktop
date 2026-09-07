//! Bundled canonical fonts for the congregation-slide renderer.
//!
//! The canonical path loads exactly these two faces: Atkinson Hyperlegible
//! Next for primary text and Noto Sans as the missing-glyph fallback. There
//! is no system font enumeration here and no runtime path that consults one;
//! `tests/fonts.rs` proves both properties instead of this comment claiming
//! them.

pub const ATKINSON_HYPERLEGIBLE_NEXT: &[u8] = include_bytes!(
    "../../../../assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf"
);
pub const NOTO_SANS: &[u8] =
    include_bytes!("../../../../assets/fonts/noto-sans/NotoSans[wdth,wght].ttf");

/// The only faces the canonical path can load. The two variants close the
/// world: no font outside the bundle is expressible here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalFont {
    /// Atkinson Hyperlegible Next, the primary face.
    Primary,
    /// Noto Sans, the missing-glyph fallback.
    Fallback,
}

impl CanonicalFont {
    /// The bundled bytes for this face.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Primary => ATKINSON_HYPERLEGIBLE_NEXT,
            Self::Fallback => NOTO_SANS,
        }
    }
}

/// The bundled face that renders `character`: the primary face when it
/// carries the glyph, otherwise the Noto fallback — even when Noto also
/// lacks it, so shaping never consults an installed font.
pub fn resolve_font_for(character: char) -> CanonicalFont {
    if contains_glyph(ATKINSON_HYPERLEGIBLE_NEXT, character) {
        CanonicalFont::Primary
    } else {
        CanonicalFont::Fallback
    }
}

fn contains_glyph(font: &[u8], character: char) -> bool {
    ttf_parser::Face::parse(font, 0)
        .ok()
        .and_then(|face| face.glyph_index(character))
        .is_some()
}
