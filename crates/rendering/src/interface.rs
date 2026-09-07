//! Canonical Slide Renderer Interface (T0.4 PRs 3-5).
//!
//! The single seam every later T0.4 PR builds on: resolved semantic content, a
//! Theme, and a target density go in, and either one complete frame plus
//! diagnostics or one structured failure comes out. A partial frame is
//! unrepresentable: [`Frame`] fields are private, so only [`render`] can build
//! one, and the success value always carries the whole composition.
//!
//! The content inputs are already resolved. Response and psalm headers arrive
//! as authored strings or not at all; the renderer never supplies header text
//! and performs no per-language branching to find it. There is deliberately no
//! language type anywhere in this module.
//!
//! Composition happens in explicit 1920x1080 logical coordinates with the
//! minimum 120x80 safe-area inset. The signature takes no container size,
//! operator text scale, or OS scale, so there is no parameter through which
//! one could arrive. Zones are renderer-owned: header, body, and footer stack
//! top to bottom inside the safe area, an absent zone collapses so the body
//! receives its space, and there is no free-placement parameter, now or
//! behind a flag.
//!
//! Zone heights are measured, not proportional: each present zone occupies its
//! wrapped lines at the provisional body size, and the body receives whatever
//! safe-area height remains. Body and line overflow keeps every line and
//! reports a warning beside the complete frame; chrome taller than the safe
//! area is a structured failure instead, because no pagination can split
//! header and footer content and anything else would misbound the frame.

use crate::fonts::CanonicalFont;
use crate::shaping::{self, Alignment, Fit, PaginationInputs, ShapedLine, VAlign};

/// Logical canvas width every slide composes in.
pub const LOGICAL_WIDTH: u32 = 1920;
/// Logical canvas height every slide composes in.
pub const LOGICAL_HEIGHT: u32 = 1080;
/// Minimum horizontal safe-area inset (left and right).
pub const MIN_SAFE_INSET_X: u32 = 120;
/// Minimum vertical safe-area inset (top and bottom).
pub const MIN_SAFE_INSET_Y: u32 = 80;

/// A rectangle in logical coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Resolved congregation content for one slide.
///
/// `body` carries the resolved text with authored line breaks preserved: a
/// plain newline continues the slide, while a blank source line marks a hard
/// slide boundary that overrides any automatic proposal. `response_header`
/// and `psalm_header` each carry an already-resolved authored header string
/// or nothing; `Some` reserves the header zone even when the string is empty,
/// while `None` collapses it so the body receives the space. At most one of
/// the two headers may be present. `footer` works the same way for the footer
/// zone (for example a reference or caption). No field here names a language,
/// and no constructor derives text from one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticContent {
    pub body: String,
    pub response_header: Option<String>,
    pub psalm_header: Option<String>,
    pub footer: Option<String>,
}

/// Theme input for the canonical path.
///
/// PR 3 carries only the safe-area insets: a Theme may increase them and is
/// rejected for reducing them below the minimum. Typography, colors, and
/// backgrounds belong to the template corpus (see `templates`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub safe_inset_x: u32,
    pub safe_inset_y: u32,
}

impl Theme {
    /// The minimum safe area: 120 horizontal, 80 vertical.
    pub const fn minimum() -> Self {
        Self {
            safe_inset_x: MIN_SAFE_INSET_X,
            safe_inset_y: MIN_SAFE_INSET_Y,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::minimum()
    }
}

/// Target output density for rasterization (PR 7).
///
/// Density changes raster dimensions and sampling sharpness only. It is
/// accepted here so the seam is stable, but [`render`] must not let it alter
/// the logical composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetDensity {
    scale_percent: u16,
}

impl TargetDensity {
    /// 100%: one raster pixel per logical unit.
    pub const SCALE_100: Self = Self { scale_percent: 100 };

    /// A density from a percentage (100, 150, 200, ...). Zero is rejected.
    pub fn from_percent(scale_percent: u16) -> Option<Self> {
        (scale_percent > 0).then_some(Self { scale_percent })
    }

    pub fn scale_percent(self) -> u16 {
        self.scale_percent
    }
}

/// Per-zone composition style: the provisional size the zone measures at and
/// the alignment its lines take inside the zone width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ZoneStyle {
    pub(crate) size: u32,
    pub(crate) align: Alignment,
}

impl ZoneStyle {
    /// The PR 4 neutral style: the 72px baseline, left-origin lines.
    pub(crate) const fn neutral() -> Self {
        Self {
            size: shaping::BODY_FONT_SIZE,
            align: Alignment::Left,
        }
    }
}

/// Per-composition zone plan: per-zone styles, the song identity-block lead,
/// and the body's vertical placement inside its rect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompositionPlan {
    pub(crate) header: ZoneStyle,
    pub(crate) body: ZoneStyle,
    pub(crate) footer: ZoneStyle,
    pub(crate) footer_lead: Option<(ZoneStyle, u32)>,
    pub(crate) body_valign: VAlign,
}

impl CompositionPlan {
    /// The PR 4 neutral plan: the baseline size, left-origin lines starting
    /// at the top of each zone.
    pub(crate) const fn neutral() -> Self {
        Self {
            header: ZoneStyle::neutral(),
            body: ZoneStyle::neutral(),
            footer: ZoneStyle::neutral(),
            footer_lead: None,
            body_valign: VAlign::Top,
        }
    }
}

/// Diagnostics riding beside a complete frame.
///
/// Diagnostics never stand in for missing content: they accompany [`Frame`],
/// which is always whole. PR 4 adds the overflow warning emitted when body
/// content cannot fit the zone at the approved minimum.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    pub warnings: Vec<String>,
}

/// One complete logical composition.
///
/// Fields are private so a partially composed frame cannot be named: the
/// only way to obtain one is [`render`], which returns every zone at once.
/// `header` and `footer` are `None` exactly when the corresponding input was
/// absent; the body then covers their share. The shaped lines carry every
/// glyph position, and [`Frame::serialize`] pins the whole composition in a
/// form stable enough to compare byte-for-byte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    safe_area: Rect,
    header: Option<Rect>,
    body: Rect,
    footer: Option<Rect>,
    header_lines: Vec<ShapedLine>,
    body_lines: Vec<ShapedLine>,
    footer_lines: Vec<ShapedLine>,
    pagination: PaginationInputs,
}

impl Frame {
    pub fn safe_area(&self) -> Rect {
        self.safe_area
    }

    pub fn header(&self) -> Option<Rect> {
        self.header
    }

    pub fn body(&self) -> Rect {
        self.body
    }

    pub fn footer(&self) -> Option<Rect> {
        self.footer
    }

    pub fn header_lines(&self) -> &[ShapedLine] {
        &self.header_lines
    }

    pub fn body_lines(&self) -> &[ShapedLine] {
        &self.body_lines
    }

    pub fn footer_lines(&self) -> &[ShapedLine] {
        &self.footer_lines
    }

    pub fn pagination(&self) -> &PaginationInputs {
        &self.pagination
    }

    /// The logical canvas size this frame was composed for.
    pub fn logical_size(&self) -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
    }

    /// Serialize the completed logical frame: safe area, element bounds,
    /// glyph positions, wrapping, and pagination inputs. The output uses only
    /// integers and `'\n'` separators with glyphs in visual order, so the
    /// same content, Theme, and density produce byte-identical output in
    /// every process on both packaged targets. Density never appears here:
    /// it must not alter the logical composition.
    pub fn serialize(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "ordo-canonical-frame/1");
        let _ = writeln!(out, "logical {LOGICAL_WIDTH} {LOGICAL_HEIGHT}");
        let safe = self.safe_area;
        let _ = writeln!(
            out,
            "safe-area {} {} {} {}",
            safe.x, safe.y, safe.width, safe.height
        );
        match self.header {
            Some(rect) => {
                let _ = writeln!(
                    out,
                    "header-rect {} {} {} {}",
                    rect.x, rect.y, rect.width, rect.height
                );
            }
            None => {
                let _ = writeln!(out, "header-absent");
            }
        }
        let body = self.body;
        let _ = writeln!(
            out,
            "body-rect {} {} {} {}",
            body.x, body.y, body.width, body.height
        );
        match self.footer {
            Some(rect) => {
                let _ = writeln!(
                    out,
                    "footer-rect {} {} {} {}",
                    rect.x, rect.y, rect.width, rect.height
                );
            }
            None => {
                let _ = writeln!(out, "footer-absent");
            }
        }
        let pagination = &self.pagination;
        let fit = match pagination.fit {
            Fit::Fits => "fits",
            Fit::Overflows => "overflows",
        };
        let first_split = match pagination.first_hard_split {
            Some(index) => index.to_string(),
            None => "none".to_owned(),
        };
        let _ = writeln!(
            out,
            "pagination authored-body={} header-lines={} body-lines={} footer-lines={} content-height={} available-height={} fit={fit} hard-split={} first-hard-split={first_split}",
            pagination.authored_body_lines,
            pagination.header_lines,
            pagination.body_lines,
            pagination.footer_lines,
            pagination.body_content_height,
            pagination.body_available_height,
            pagination.hard_split,
        );
        for (zone, lines) in [
            ("header", self.header_lines.as_slice()),
            ("body", self.body_lines.as_slice()),
            ("footer", self.footer_lines.as_slice()),
        ] {
            for (index, line) in lines.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "line zone={zone} index={index} authored={} x={} y={} width={} height={} text={}",
                    line.authored_index, line.x, line.y, line.width, line.height, line.text
                );
                for glyph in &line.glyphs {
                    let font = match glyph.font {
                        CanonicalFont::Primary => "primary",
                        CanonicalFont::Fallback => "fallback",
                    };
                    let _ = writeln!(
                        out,
                        "glyph zone={zone} line={index} font={font} id={} x={} y={} x-advance={} y-advance={} x-offset={} y-offset={} cluster={}",
                        glyph.id,
                        glyph.x,
                        glyph.y,
                        glyph.x_advance,
                        glyph.y_advance,
                        glyph.x_offset,
                        glyph.y_offset,
                        glyph.cluster
                    );
                }
            }
        }
        out
    }
}

/// The structured ways rendering can fail.
///
/// A Theme that reduces the safe area is rejected here rather than clamped
/// silently, so the caller sees the requested and minimum values together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderError {
    SafeAreaBelowMinimum {
        requested_x: u32,
        requested_y: u32,
        minimum_x: u32,
        minimum_y: u32,
    },
    SafeAreaExceedsCanvas {
        requested_x: u32,
        requested_y: u32,
    },
    ConflictingHeaders,
    FontUnavailable,
    ZonesExceedSafeArea {
        header_height: u32,
        footer_height: u32,
        safe_height: u32,
    },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SafeAreaBelowMinimum {
                requested_x,
                requested_y,
                minimum_x,
                minimum_y,
            } => write!(
                f,
                "safe-area inset {requested_x}x{requested_y} is below the minimum {minimum_x}x{minimum_y}"
            ),
            Self::SafeAreaExceedsCanvas {
                requested_x,
                requested_y,
            } => write!(
                f,
                "safe-area inset {requested_x}x{requested_y} leaves no canvas"
            ),
            Self::ConflictingHeaders => {
                write!(f, "response and psalm headers must not both be present")
            }
            Self::FontUnavailable => write!(f, "bundled canonical font is unavailable"),
            Self::ZonesExceedSafeArea {
                header_height,
                footer_height,
                safe_height,
            } => write!(
                f,
                "header ({header_height}px) and footer ({footer_height}px) exceed the {safe_height}px safe area"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// Compose one complete logical frame from resolved content.
///
/// `plan` carries the per-zone size, alignment, and body vertical placement
/// the template corpus assigns; [`render`] passes the neutral plan.
/// `footer_lead` handles the one mixed-size zone in the corpus, the song
/// identity block: when present, the footer's first `u32` authored lines lay
/// out at the lead style and the rest at the footer style, keeping one
/// bottom-left block. Density is accepted and ignored for composition: the
/// same content and Theme always yield the same frame regardless of density.
pub(crate) fn compose_zoned(
    content: &SemanticContent,
    plan: &CompositionPlan,
    theme: &Theme,
    density: TargetDensity,
) -> Result<(Frame, Diagnostics), RenderError> {
    let _ = density;
    let header = plan.header;
    let body = plan.body;
    let footer = plan.footer;
    if theme.safe_inset_x < MIN_SAFE_INSET_X || theme.safe_inset_y < MIN_SAFE_INSET_Y {
        return Err(RenderError::SafeAreaBelowMinimum {
            requested_x: theme.safe_inset_x,
            requested_y: theme.safe_inset_y,
            minimum_x: MIN_SAFE_INSET_X,
            minimum_y: MIN_SAFE_INSET_Y,
        });
    }
    let inset_x = u64::from(theme.safe_inset_x);
    let inset_y = u64::from(theme.safe_inset_y);
    let safe_width = u64::from(LOGICAL_WIDTH)
        .checked_sub(inset_x.saturating_mul(2))
        .and_then(|width| u32::try_from(width).ok());
    let safe_height = u64::from(LOGICAL_HEIGHT)
        .checked_sub(inset_y.saturating_mul(2))
        .and_then(|height| u32::try_from(height).ok());
    let (Some(safe_width), Some(safe_height)) = (safe_width, safe_height) else {
        return Err(RenderError::SafeAreaExceedsCanvas {
            requested_x: theme.safe_inset_x,
            requested_y: theme.safe_inset_y,
        });
    };
    if safe_width == 0 || safe_height == 0 {
        return Err(RenderError::SafeAreaExceedsCanvas {
            requested_x: theme.safe_inset_x,
            requested_y: theme.safe_inset_y,
        });
    }
    if content.response_header.is_some() && content.psalm_header.is_some() {
        return Err(RenderError::ConflictingHeaders);
    }

    let faces = shaping::load_faces().ok_or(RenderError::FontUnavailable)?;
    let max_width = i32::try_from(safe_width).map_err(|_| RenderError::SafeAreaExceedsCanvas {
        requested_x: theme.safe_inset_x,
        requested_y: theme.safe_inset_y,
    })?;
    let safe_x =
        i32::try_from(theme.safe_inset_x).map_err(|_| RenderError::SafeAreaExceedsCanvas {
            requested_x: theme.safe_inset_x,
            requested_y: theme.safe_inset_y,
        })?;
    let safe_y =
        i32::try_from(theme.safe_inset_y).map_err(|_| RenderError::SafeAreaExceedsCanvas {
            requested_x: theme.safe_inset_x,
            requested_y: theme.safe_inset_y,
        })?;

    let safe_area = Rect {
        x: theme.safe_inset_x,
        y: theme.safe_inset_y,
        width: safe_width,
        height: safe_height,
    };
    let header_text = content
        .response_header
        .as_deref()
        .or(content.psalm_header.as_deref());
    let header_lines = header_text
        .map(|text| {
            shaping::layout_zone(
                text,
                header.size,
                max_width,
                safe_x,
                safe_y,
                &faces,
                header.align,
            )
        })
        .unwrap_or_default();
    // A present header reserves at least one line even when its string is
    // empty; an absent header collapses to zero so the body keeps the space.
    let header_height = header_text
        .map(|_| {
            (header_lines.len().max(1) as u32).saturating_mul(shaping::line_height_for(header.size))
        })
        .unwrap_or(0);

    let (footer_lines, footer_height) = match content.footer.as_deref() {
        None => (Vec::new(), 0),
        Some(text) => layout_footer(text, footer, plan.footer_lead, max_width, &faces),
    };
    // Chrome taller than the safe area cannot be paginated away, so it fails
    // structurally instead of warning beside misbounded zones.
    if header_height.saturating_add(footer_height) > safe_height {
        return Err(RenderError::ZonesExceedSafeArea {
            header_height,
            footer_height,
            safe_height,
        });
    }
    let footer_y = safe_y + safe_height as i32 - footer_height as i32;
    let mut footer_lines = footer_lines;
    for line in &mut footer_lines {
        line.x += safe_x;
        line.y += footer_y;
        for glyph in &mut line.glyphs {
            glyph.x += safe_x;
            glyph.y += footer_y;
        }
    }

    let body_y = safe_y + header_height as i32;
    let body_available = safe_height
        .saturating_sub(header_height)
        .saturating_sub(footer_height);
    let mut laid_out = shaping::layout_body(
        &content.body,
        body.size,
        max_width,
        safe_x,
        body_y,
        &faces,
        body.align,
    );
    let body_content_height =
        (laid_out.lines.len() as u32).saturating_mul(shaping::line_height_for(body.size));
    // Centered templates offset the shaped block from its measured height so
    // short content sits in the middle of the remaining region instead of
    // hugging its top edge. Truncating division keeps it deterministic, and
    // an overflowing block stays top-anchored beside its warning.
    if plan.body_valign == VAlign::Center {
        let shift = body_available.saturating_sub(body_content_height) / 2;
        if shift > 0 {
            for line in &mut laid_out.lines {
                line.y += shift as i32;
                for glyph in &mut line.glyphs {
                    glyph.y += shift as i32;
                }
            }
        }
    }
    // Horizontal overflow is possible only when a single glyph exceeds the
    // safe width; like every other overflow it is reported, never dropped.
    let widest = header_lines
        .iter()
        .chain(laid_out.lines.iter())
        .chain(footer_lines.iter())
        .map(|line| line.width)
        .max()
        .unwrap_or(0);

    // Rect origins saturate inside the safe area so large-but-possible
    // totals never wrap; heights stay measured and every line stays
    // present while the warnings below name the overflow. Impossible
    // chrome already failed above.
    let footer_rect_y = safe_area.y + safe_height.saturating_sub(footer_height);
    let body_rect_y = safe_area.y + header_height.min(safe_height);

    let header = header_text.map(|_| Rect {
        x: safe_area.x,
        y: safe_area.y,
        width: safe_area.width,
        height: header_height,
    });
    let footer = content.footer.as_deref().map(|_| Rect {
        x: safe_area.x,
        y: footer_rect_y,
        width: safe_area.width,
        height: footer_height,
    });
    let body = Rect {
        x: safe_area.x,
        y: body_rect_y,
        width: safe_area.width,
        height: body_available,
    };

    let mut diagnostics = Diagnostics::default();
    if widest > max_width {
        diagnostics.warnings.push(format!(
            "a line needs {widest}px but the safe area offers {max_width}px at the approved minimum; split before shrinking"
        ));
    }
    if body_content_height > body_available {
        diagnostics.warnings.push(format!(
            "body content needs {body_content_height}px but the body zone offers {body_available}px at the approved minimum; split before shrinking"
        ));
    }
    let fit = if diagnostics.warnings.is_empty() {
        Fit::Fits
    } else {
        Fit::Overflows
    };

    let pagination = PaginationInputs {
        authored_body_lines: laid_out.authored_line_count,
        header_lines: header_lines.len() as u32,
        body_lines: laid_out.lines.len() as u32,
        footer_lines: footer_lines.len() as u32,
        body_content_height,
        body_available_height: body_available,
        fit,
        hard_split: laid_out.hard_split,
        first_hard_split: laid_out.first_hard_split,
    };
    Ok((
        Frame {
            safe_area,
            header,
            body,
            footer,
            header_lines,
            body_lines: laid_out.lines,
            footer_lines,
            pagination,
        },
        diagnostics,
    ))
}

/// Lay out a present footer and measure its height. Without a lead the whole
/// zone measures at one style and reserves at least one line, exactly as PR 4
/// did. With a lead, the first `lead_authored` authored lines measure at the
/// lead style and the rest stack beneath at the footer style; the lead
/// reserves the line, so the rest adds only its rendered lines. Rest authored
/// indices shift past the lead so serialized lines stay attributable.
fn layout_footer(
    text: &str,
    style: ZoneStyle,
    lead: Option<(ZoneStyle, u32)>,
    max_width: i32,
    faces: &shaping::Faces,
) -> (Vec<ShapedLine>, u32) {
    let Some((lead_style, lead_authored)) = lead else {
        let lines = shaping::layout_zone(text, style.size, max_width, 0, 0, faces, style.align);
        let height =
            (lines.len().max(1) as u32).saturating_mul(shaping::line_height_for(style.size));
        return (lines, height);
    };
    let authored: Vec<&str> = text.split('\n').collect();
    let split_at = (lead_authored as usize).min(authored.len());
    let (lead_part, rest_part) = authored.split_at(split_at);
    let mut lead_lines = shaping::layout_zone(
        &lead_part.join("\n"),
        lead_style.size,
        max_width,
        0,
        0,
        faces,
        lead_style.align,
    );
    let lead_height =
        (lead_lines.len().max(1) as u32).saturating_mul(shaping::line_height_for(lead_style.size));
    let mut rest_lines = shaping::layout_zone(
        &rest_part.join("\n"),
        style.size,
        max_width,
        0,
        lead_height as i32,
        faces,
        style.align,
    );
    for line in &mut rest_lines {
        line.authored_index += split_at as u32;
    }
    let rest_height =
        (rest_lines.len() as u32).saturating_mul(shaping::line_height_for(style.size));
    lead_lines.extend(rest_lines);
    (lead_lines, lead_height.saturating_add(rest_height))
}

/// Compose one complete logical frame from resolved content.
///
/// Every zone measures at the 72px baseline with left-origin lines starting
/// at the top of each zone; the template corpus (see `templates`) varies
/// size, alignment, and vertical placement per zone through the same
/// composition core.
pub fn render(
    content: &SemanticContent,
    theme: &Theme,
    density: TargetDensity,
) -> Result<(Frame, Diagnostics), RenderError> {
    compose_zoned(content, &CompositionPlan::neutral(), theme, density)
}
