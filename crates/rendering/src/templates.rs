//! Canonical template corpus (T0.4 PR 5).
//!
//! The fixed template relationships `DESIGN.md` adopts, composed through the
//! same Renderer Interface as every other slide: each template maps resolved
//! content onto header, body, and footer zones with a per-zone size and
//! alignment, and [`render_template`] returns one complete frame plus
//! diagnostics or one structured failure, exactly like [`crate::render`].
//!
//! Zone mapping per kind:
//!
//! - Lyrics: section label at top-left, authored-case body in the remainder,
//!   no footer. A missing label collapses the header so the body keeps the
//!   space; that absent-label composition is the label-off variant.
//! - Response: resolved header, stored text rendered uppercase as the
//!   emphasized body, no footer.
//! - Reading identification: no header, large uppercase proclamation title
//!   centered in the body, reference at bottom-left.
//! - Reading passage: no header, passage in the body, reference at
//!   bottom-left on every slide. A projected cantor verse uses this kind.
//! - Psalm: resolved header at top-left, refrain centered in the remainder,
//!   reference at bottom-left.
//! - Song identification: one bottom-left identity block in the footer, with
//!   the large uppercase title leading at display size and the
//!   composer/source plus optional credit following at meta size. The body
//!   stays empty.
//! - Celebration title: uppercase title in the body, localized date at
//!   bottom-left.
//! - General text: optional header, body, and optional footer as given.
//! - General image: optional header and caption footer; the empty body rect
//!   reserves the contained foreground bounds the raster pass fills in PR 7.
//! - Black: everything empty. Black paint is raster-side; the logical frame
//!   is an empty composition that fits without warnings.
//!
//! Contract points, not oversights:
//!
//! - Headers arrive resolved and are copied verbatim: the corpus never
//!   supplies header text and performs no lookup of any kind. The two header
//!   slots name zone provenance for the response/psalm pair; every other kind
//!   with chrome reuses the response slot, and no template populates both.
//! - Reading templates take the resolved proclamation title and reference
//!   only. There is no field that could name which reading a slide is, so no
//!   proclamation-number label can reach congregation output.
//! - There is no foreground slot for a theme picture: it stays background
//!   paint outside this module, so a theme picture can never arrive as
//!   foreground content.
//!
//! Relative hierarchy with provisional numbers: display (72px, the
//! projected-response token and first gallery baseline) carries emphasized
//! bodies and large titles; passage (56px) carries running scripture and
//! general text, smaller than identification titles; meta (40px) carries
//! labels, headers, references, credits, dates, and captions. Every number
//! except the 72px baseline is provisional: the PR 10 gallery qualifies them
//! and only then do they freeze.
//!
//! Line ceilings warn beside a complete frame without changing its fit:
//! lyrics target two logical phrases and no more than four rendered lines,
//! and song and celebration titles warn past three rendered lines. Each
//! warning fires at its boundary: the count at the ceiling is quiet, one
//! past it warns.

use crate::interface::{
    self, Diagnostics, Frame, RenderError, SemanticContent, TargetDensity, Theme, ZoneStyle,
};
use crate::shaping::{self, Alignment};

/// Provisional display size: the 72px projected-response token. Emphasized
/// bodies and large titles measure here until the PR 10 gallery freezes the
/// levels.
pub const PROVISIONAL_DISPLAY_SIZE: u32 = shaping::BODY_FONT_SIZE;
/// Provisional passage size: running scripture and general text, smaller than
/// identification titles.
pub const PROVISIONAL_PASSAGE_SIZE: u32 = 56;
/// Provisional meta size: section labels, resolved headers, references,
/// credits, dates, and captions.
pub const PROVISIONAL_META_SIZE: u32 = 40;

/// The adopted congregation templates, plus the black slide.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateKind {
    Lyrics,
    Response,
    ReadingIdentification,
    ReadingPassage,
    Psalm,
    SongIdentification,
    CelebrationTitle,
    GeneralText,
    GeneralImage,
    Black,
}

/// Resolved content for one template of [`TemplateKind`].
///
/// `header` is the section label or resolved header where the kind has
/// chrome; `body` is the main resolved text (the song title for
/// SongIdentification, empty for GeneralImage and Black); `footer` is the
/// reference, credit block, localized date, caption, or general footer.
/// Absent chrome is `None`, never an invented default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateContent {
    pub kind: TemplateKind,
    pub header: Option<String>,
    pub body: String,
    pub footer: Option<String>,
}

/// One template's zone plan: per-zone styles plus the footer lead for the
/// song identity block.
struct Composition {
    semantic: SemanticContent,
    header: ZoneStyle,
    body: ZoneStyle,
    footer: ZoneStyle,
    footer_lead: Option<(ZoneStyle, u32)>,
}

fn meta_left() -> ZoneStyle {
    ZoneStyle {
        size: PROVISIONAL_META_SIZE,
        align: Alignment::Left,
    }
}

fn display(align: Alignment) -> ZoneStyle {
    ZoneStyle {
        size: PROVISIONAL_DISPLAY_SIZE,
        align,
    }
}

/// Map one template onto resolved zones. Uppercase kinds transform only
/// their title/body text with the pinned Unicode mapping, which is
/// deterministic for a pinned toolchain; headers and references always pass
/// through verbatim.
fn compose_content(content: &TemplateContent) -> Composition {
    let neutral = ZoneStyle::neutral();
    let passage_left = ZoneStyle {
        size: PROVISIONAL_PASSAGE_SIZE,
        align: Alignment::Left,
    };
    match content.kind {
        TemplateKind::Lyrics => Composition {
            semantic: SemanticContent {
                body: content.body.clone(),
                response_header: content.header.clone(),
                psalm_header: None,
                footer: None,
            },
            header: meta_left(),
            body: display(Alignment::CenterBlock),
            footer: neutral,
            footer_lead: None,
        },
        TemplateKind::Response => Composition {
            semantic: SemanticContent {
                body: content.body.to_uppercase(),
                response_header: content.header.clone(),
                psalm_header: None,
                footer: None,
            },
            header: meta_left(),
            body: display(Alignment::Center),
            footer: neutral,
            footer_lead: None,
        },
        TemplateKind::ReadingIdentification => Composition {
            semantic: SemanticContent {
                body: content.body.to_uppercase(),
                response_header: None,
                psalm_header: None,
                footer: content.footer.clone(),
            },
            header: neutral,
            body: display(Alignment::Center),
            footer: meta_left(),
            footer_lead: None,
        },
        TemplateKind::ReadingPassage => Composition {
            semantic: SemanticContent {
                body: content.body.clone(),
                response_header: None,
                psalm_header: None,
                footer: content.footer.clone(),
            },
            header: neutral,
            body: passage_left,
            footer: meta_left(),
            footer_lead: None,
        },
        TemplateKind::Psalm => Composition {
            semantic: SemanticContent {
                body: content.body.clone(),
                response_header: None,
                psalm_header: content.header.clone(),
                footer: content.footer.clone(),
            },
            header: meta_left(),
            body: display(Alignment::Center),
            footer: meta_left(),
            footer_lead: None,
        },
        TemplateKind::SongIdentification => {
            let title = content.body.to_uppercase();
            let lead_authored = title.split('\n').count() as u32;
            let footer_text = match content.footer.as_deref() {
                Some(credits) if !credits.is_empty() => format!("{title}\n{credits}"),
                _ => title,
            };
            Composition {
                semantic: SemanticContent {
                    body: String::new(),
                    response_header: None,
                    psalm_header: None,
                    footer: Some(footer_text),
                },
                header: neutral,
                body: neutral,
                footer: meta_left(),
                footer_lead: Some((display(Alignment::Left), lead_authored)),
            }
        }
        TemplateKind::CelebrationTitle => Composition {
            semantic: SemanticContent {
                body: content.body.to_uppercase(),
                response_header: None,
                psalm_header: None,
                footer: content.footer.clone(),
            },
            header: neutral,
            body: display(Alignment::CenterBlock),
            footer: meta_left(),
            footer_lead: None,
        },
        TemplateKind::GeneralText => Composition {
            semantic: SemanticContent {
                body: content.body.clone(),
                response_header: content.header.clone(),
                psalm_header: None,
                footer: content.footer.clone(),
            },
            header: meta_left(),
            body: passage_left,
            footer: meta_left(),
            footer_lead: None,
        },
        TemplateKind::GeneralImage => Composition {
            semantic: SemanticContent {
                body: String::new(),
                response_header: content.header.clone(),
                psalm_header: None,
                footer: content.footer.clone(),
            },
            header: meta_left(),
            body: neutral,
            footer: meta_left(),
            footer_lead: None,
        },
        TemplateKind::Black => Composition {
            semantic: SemanticContent {
                body: String::new(),
                response_header: None,
                psalm_header: None,
                footer: None,
            },
            header: neutral,
            body: neutral,
            footer: neutral,
            footer_lead: None,
        },
    }
}

/// Compose one complete template frame through the same core as [`crate::render`].
///
/// Density is accepted and ignored for composition, exactly like [`crate::render`]:
/// it may change raster dimensions later without altering this frame.
/// Ceiling warnings ride beside the complete frame; geometric fit is
/// untouched by them.
pub fn render_template(
    content: &TemplateContent,
    theme: &Theme,
    density: TargetDensity,
) -> Result<(Frame, Diagnostics), RenderError> {
    let composition = compose_content(content);
    let (frame, mut diagnostics) = interface::compose_zoned(
        &composition.semantic,
        composition.header,
        composition.body,
        composition.footer,
        composition.footer_lead,
        theme,
        density,
    )?;
    match content.kind {
        TemplateKind::Lyrics => {
            let phrases = composition
                .semantic
                .body
                .split('\n')
                .filter(|line| !line.trim().is_empty())
                .count();
            if phrases > 2 {
                diagnostics.warnings.push(format!(
                    "lyric body carries {phrases} phrases against a two-phrase target; split before shrinking"
                ));
            }
            let lines = frame.pagination().body_lines;
            if lines > 4 {
                diagnostics.warnings.push(format!(
                    "lyric body has {lines} rendered lines against a four-line ceiling; split before shrinking"
                ));
            }
        }
        TemplateKind::SongIdentification => {
            let lead_authored = composition.footer_lead.map_or(0, |(_, count)| count);
            let title_lines = frame
                .footer_lines()
                .iter()
                .filter(|line| line.authored_index < lead_authored)
                .count();
            if title_lines > 3 {
                diagnostics.warnings.push(format!(
                    "song title has {title_lines} rendered lines against a three-line ceiling; split before shrinking"
                ));
            }
        }
        TemplateKind::CelebrationTitle => {
            let title_lines = frame.pagination().body_lines;
            if title_lines > 3 {
                diagnostics.warnings.push(format!(
                    "celebration title has {title_lines} rendered lines against a three-line ceiling; split before shrinking"
                ));
            }
        }
        TemplateKind::Response
        | TemplateKind::ReadingIdentification
        | TemplateKind::ReadingPassage
        | TemplateKind::Psalm
        | TemplateKind::GeneralText
        | TemplateKind::GeneralImage
        | TemplateKind::Black => {}
    }
    Ok((frame, diagnostics))
}
