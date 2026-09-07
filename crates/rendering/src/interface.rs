//! Canonical Slide Renderer Interface (T0.4 PRs 3-4).
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
//! safe-area height remains. Overflow keeps every line and reports a warning
//! beside the complete frame; nothing shrinks and nothing clips.

use crate::fonts::CanonicalFont;
use crate::shaping::{self, Fit, PaginationInputs, ShapedLine};

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
/// backgrounds belong to the template corpus (PR 5).
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
        }
    }
}

impl std::error::Error for RenderError {}

/// Compose one complete logical frame from resolved content.
///
/// Density is accepted and ignored for composition: the same content and
/// Theme always yield the same frame regardless of density.
pub fn render(
    content: &SemanticContent,
    theme: &Theme,
    density: TargetDensity,
) -> Result<(Frame, Diagnostics), RenderError> {
    let _ = density;
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
        .map(|text| shaping::layout_zone(text, max_width, safe_x, safe_y, &faces))
        .unwrap_or_default();
    // A present header reserves at least one line even when its string is
    // empty; an absent header collapses to zero so the body keeps the space.
    let header_height = header_text
        .map(|_| (header_lines.len().max(1) as u32).saturating_mul(shaping::BODY_LINE_HEIGHT))
        .unwrap_or(0);

    let footer_lines = content
        .footer
        .as_deref()
        .map(|text| shaping::layout_zone(text, max_width, 0, 0, &faces))
        .unwrap_or_default();
    let footer_height = content
        .footer
        .as_deref()
        .map(|_| (footer_lines.len().max(1) as u32).saturating_mul(shaping::BODY_LINE_HEIGHT))
        .unwrap_or(0);
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
    let laid_out = shaping::layout_body(&content.body, max_width, safe_x, body_y, &faces);
    let body_content_height =
        (laid_out.lines.len() as u32).saturating_mul(shaping::BODY_LINE_HEIGHT);
    let fit = if body_content_height <= body_available {
        Fit::Fits
    } else {
        Fit::Overflows
    };

    let header = header_text.map(|_| Rect {
        x: safe_area.x,
        y: safe_area.y,
        width: safe_area.width,
        height: header_height,
    });
    let footer = content.footer.as_deref().map(|_| Rect {
        x: safe_area.x,
        y: (footer_y as u32).max(safe_area.y),
        width: safe_area.width,
        height: footer_height,
    });
    let body = Rect {
        x: safe_area.x,
        y: (body_y as u32).max(safe_area.y),
        width: safe_area.width,
        height: body_available,
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
    let mut diagnostics = Diagnostics::default();
    if fit == Fit::Overflows {
        diagnostics.warnings.push(format!(
            "body content needs {body_content_height}px but the body zone offers {body_available}px at the approved minimum; split before shrinking"
        ));
    }
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
