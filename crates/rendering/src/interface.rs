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

/// Per-zone composition style: the provisional size the zone measures at,
/// the alignment its lines take inside the zone width, and the variable-font
/// `wght` its advances were shaped with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ZoneStyle {
    pub(crate) size: u32,
    pub(crate) align: Alignment,
    pub(crate) weight: u16,
}

impl ZoneStyle {
    /// The neutral style: the candidate 90px baseline, left-origin lines at
    /// the regular (default-instance) weight.
    pub(crate) const fn neutral() -> Self {
        Self {
            size: shaping::BODY_FONT_SIZE,
            align: Alignment::Left,
            weight: shaping::REGULAR_WEIGHT,
        }
    }
}

/// Per-composition zone plan: per-zone styles, the song identity-block lead,
/// the optional second body segment, the body's vertical placement inside
/// its rect, and the slide background the raster pass paints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompositionPlan {
    pub(crate) header: ZoneStyle,
    pub(crate) body: ZoneStyle,
    pub(crate) footer: ZoneStyle,
    pub(crate) footer_lead: Option<(ZoneStyle, u32)>,
    pub(crate) body_tail: Option<(ZoneStyle, u32)>,
    pub(crate) body_valign: VAlign,
    pub(crate) background: SlideBackground,
}

impl CompositionPlan {
    /// The PR 4 neutral plan: the baseline size, left-origin lines starting
    /// at the top of each zone, on the theme background.
    pub(crate) const fn neutral() -> Self {
        Self {
            header: ZoneStyle::neutral(),
            body: ZoneStyle::neutral(),
            footer: ZoneStyle::neutral(),
            footer_lead: None,
            body_tail: None,
            body_valign: VAlign::Top,
            background: SlideBackground::Theme,
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

/// Slide background intent, as the completed frame carries it: the theme
/// background or the required black slide. The raster pass paints from this;
/// an empty text slide and a black slide serialize differently because only
/// one carries the black intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlideBackground {
    Theme,
    Black,
}

/// One complete logical composition.
///
/// Fields are private so a partially composed frame cannot be named: the
/// only way to obtain one is [`render`], which returns every zone at once.
/// `header` and `footer` are `None` exactly when the corresponding input was
/// absent; the body then covers their share. The shaped lines carry every
/// glyph position, `image` carries the resolved foreground element with its
/// contained bounds, and [`Frame::serialize`] pins the whole composition in
/// a form stable enough to compare byte-for-byte.
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
    image: Option<PlacedImage>,
    background: SlideBackground,
}

/// A resolved foreground image, as the template corpus accepts it: the
/// single-line asset identity the raster pass resolves, with the natural
/// dimensions the logical bounds contain. No bytes travel here and nothing
/// is fetched. PR 7 rasterizes the completed frame with a separate
/// raster-time image source that maps this identity to encoded bytes and
/// paints them into the frame's contained bounds; the logical frame carries
/// geometry only, so no decode pipeline or pixel store enters this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForegroundImage {
    pub id: String,
    pub natural_width: u32,
    pub natural_height: u32,
}

/// One foreground image placed in the completed frame: the resolved identity
/// with aspect-ratio-contained logical bounds inside the body zone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedImage {
    pub id: String,
    pub bounds: Rect,
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

    pub fn image(&self) -> Option<&PlacedImage> {
        self.image.as_ref()
    }

    pub fn background(&self) -> SlideBackground {
        self.background
    }

    /// The logical canvas size this frame was composed for.
    pub fn logical_size(&self) -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
    }

    /// Serialize the completed logical frame: safe area, element bounds,
    /// glyph positions, wrapping, pagination inputs, and the foreground
    /// image. The output uses only integers and `'\n'` separators with glyphs
    /// in visual order, so the same content, Theme, and density produce
    /// byte-identical output in every process on both packaged targets.
    /// Density never appears here: it must not alter the logical composition.
    pub fn serialize(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "ordo-canonical-frame/2");
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
        match &self.image {
            Some(image) => {
                let bounds = image.bounds;
                let _ = writeln!(
                    out,
                    "image id={} {} {} {} {}",
                    image.id, bounds.x, bounds.y, bounds.width, bounds.height
                );
            }
            None => {
                let _ = writeln!(out, "image-absent");
            }
        }
        let background = match self.background {
            SlideBackground::Theme => "theme",
            SlideBackground::Black => "black",
        };
        let _ = writeln!(out, "background {background}");
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
    InvalidImageDimensions {
        natural_width: u32,
        natural_height: u32,
    },
    MissingForegroundImage,
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
            Self::InvalidImageDimensions {
                natural_width,
                natural_height,
            } => write!(
                f,
                "foreground image has no extent to contain: {natural_width}x{natural_height}"
            ),
            Self::MissingForegroundImage => {
                write!(f, "general image slide is missing its foreground image")
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// Compose one complete logical frame from resolved content.
///
/// `plan` carries the per-zone size, alignment, and body vertical placement
/// the template corpus assigns; [`render`] passes the neutral plan.
/// `footer_lead` handles the one mixed-size footer in the corpus, the song
/// identity block: when present, the footer's first `u32` authored lines lay
/// out at the lead style and the rest at the footer style, keeping one
/// bottom-left block. `body_tail` is the analogous mixed-style body segment:
/// when present, the body's first `u32` authored lines lay out at the body
/// style and the rest stack beneath at the tail style, centered together.
/// `image` carries the resolved foreground element, if any, whose contained
/// bounds join the frame. Density is accepted and ignored for composition:
/// the same content and Theme always yield the same frame regardless of
/// density.
pub(crate) fn compose_zoned(
    content: &SemanticContent,
    plan: &CompositionPlan,
    image: Option<&ForegroundImage>,
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

    // Regular faces cover every zone except the celebration body segments;
    // each distinct non-regular `wght` loads once so shaping advances match
    // the raster masks per line. Chrome stays regular in the current corpus.
    let regular_faces = shaping::load_faces().ok_or(RenderError::FontUnavailable)?;
    let body_faces_owned = if body.weight != shaping::REGULAR_WEIGHT {
        Some(shaping::load_faces_with_weight(body.weight).ok_or(RenderError::FontUnavailable)?)
    } else {
        None
    };
    let body_faces = body_faces_owned.as_ref().unwrap_or(&regular_faces);
    let tail_faces_owned = match plan.body_tail {
        Some((tail_style, _))
            if tail_style.weight != body.weight && tail_style.weight != shaping::REGULAR_WEIGHT =>
        {
            Some(
                shaping::load_faces_with_weight(tail_style.weight)
                    .ok_or(RenderError::FontUnavailable)?,
            )
        }
        _ => None,
    };
    let tail_faces: &shaping::Faces = match plan.body_tail {
        Some((tail_style, _)) if tail_style.weight == body.weight => body_faces,
        Some((tail_style, _)) if tail_style.weight == shaping::REGULAR_WEIGHT => &regular_faces,
        Some(_) => tail_faces_owned.as_ref().expect("tail faces must load"),
        None => &regular_faces,
    };
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
                (safe_x, safe_y),
                &regular_faces,
                header.align,
                header.weight,
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
        Some(text) => layout_footer(text, footer, plan.footer_lead, max_width, &regular_faces),
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
    let mut laid_out = match plan.body_tail {
        None => shaping::layout_body(
            &content.body,
            body.size,
            max_width,
            (safe_x, body_y),
            body_faces,
            body.align,
            body.weight,
        ),
        Some(tail) => layout_body_with_tail(
            &content.body,
            body,
            tail,
            max_width,
            (safe_x, body_y),
            body_faces,
            tail_faces,
        ),
    };
    let body_content_height = laid_out.lines.iter().map(|line| line.height).sum::<u32>();
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
    // The foreground element joins the completed frame with contained bounds,
    // so the raster pass never recovers semantics the logical frame dropped.
    // Zero natural extent cannot contain and fails structurally, like chrome
    // that cannot paginate.
    let image = match image {
        None => None,
        Some(requested) => {
            if requested.natural_width == 0 || requested.natural_height == 0 {
                return Err(RenderError::InvalidImageDimensions {
                    natural_width: requested.natural_width,
                    natural_height: requested.natural_height,
                });
            }
            Some(PlacedImage {
                id: requested.id.clone(),
                bounds: contain(requested.natural_width, requested.natural_height, body),
            })
        }
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
            image,
            background: plan.background,
        },
        diagnostics,
    ))
}

/// Lay out a mixed-style body: the first `tail.1` authored lines at the body
/// style plus the rest stacked beneath at the tail style, analogous to
/// [`layout_footer`] with a lead. The lead keeps body semantics (blank lines
/// mark a hard split); the tail ignores blank lines like any zone layout.
/// Tail authored indices shift past the lead so serialized lines stay
/// attributable. The caller centers the combined lines with the usual
/// `body_valign` handling.
fn layout_body_with_tail(
    text: &str,
    body: ZoneStyle,
    tail: (ZoneStyle, u32),
    max_width: i32,
    origin: (i32, i32),
    body_faces: &shaping::Faces,
    tail_faces: &shaping::Faces,
) -> shaping::LaidOutBody {
    let (origin_x, origin_y) = origin;
    let (tail_style, split_at) = tail;
    let authored: Vec<&str> = text.split('\n').collect();
    let split_at = (split_at as usize).min(authored.len());
    let (lead_part, tail_part) = authored.split_at(split_at);
    let mut lead = if lead_part.is_empty() {
        shaping::LaidOutBody {
            lines: Vec::new(),
            authored_line_count: 0,
            hard_split: false,
            first_hard_split: None,
        }
    } else {
        shaping::layout_body(
            &lead_part.join("\n"),
            body.size,
            max_width,
            (origin_x, origin_y),
            body_faces,
            body.align,
            body.weight,
        )
    };
    let lead_height = (lead.lines.len() as u32).saturating_mul(shaping::line_height_for(body.size));
    let mut tail_lines = shaping::layout_zone(
        &tail_part.join("\n"),
        tail_style.size,
        max_width,
        (origin_x, origin_y + lead_height as i32),
        tail_faces,
        tail_style.align,
        tail_style.weight,
    );
    for line in &mut tail_lines {
        line.authored_index += split_at as u32;
    }
    let authored_line_count = authored.len() as u32;
    lead.lines.extend(tail_lines);
    shaping::LaidOutBody {
        lines: lead.lines,
        authored_line_count,
        hard_split: lead.hard_split,
        first_hard_split: lead.first_hard_split,
    }
}

/// Fit natural dimensions inside `bounds` preserving aspect ratio, centered.
/// The narrower fit wins; u64 intermediates with truncating division keep the
/// result deterministic on both packaged targets.
fn contain(natural_width: u32, natural_height: u32, bounds: Rect) -> Rect {
    let (avail_w, avail_h) = (u64::from(bounds.width), u64::from(bounds.height));
    let (nat_w, nat_h) = (u64::from(natural_width), u64::from(natural_height));
    // Natural dimensions are nonzero here, so both divisors are nonzero.
    let (contained_w, contained_h) = if nat_w * avail_h <= avail_w * nat_h {
        (avail_h * nat_w / nat_h, avail_h)
    } else {
        (avail_w, avail_w * nat_h / nat_w)
    };
    let (contained_w, contained_h) = (contained_w as u32, contained_h as u32);
    Rect {
        x: bounds.x + bounds.width.saturating_sub(contained_w) / 2,
        y: bounds.y + bounds.height.saturating_sub(contained_h) / 2,
        width: contained_w,
        height: contained_h,
    }
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
        let lines = shaping::layout_zone(
            text,
            style.size,
            max_width,
            (0, 0),
            faces,
            style.align,
            style.weight,
        );
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
        (0, 0),
        faces,
        lead_style.align,
        lead_style.weight,
    );
    let lead_height =
        (lead_lines.len().max(1) as u32).saturating_mul(shaping::line_height_for(lead_style.size));
    let mut rest_lines = shaping::layout_zone(
        &rest_part.join("\n"),
        style.size,
        max_width,
        (0, lead_height as i32),
        faces,
        style.align,
        style.weight,
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
/// Every zone measures at the candidate 90px baseline with left-origin lines starting
/// at the top of each zone; the template corpus (see `templates`) varies
/// size, alignment, and vertical placement per zone through the same
/// composition core.
pub fn render(
    content: &SemanticContent,
    theme: &Theme,
    density: TargetDensity,
) -> Result<(Frame, Diagnostics), RenderError> {
    compose_zoned(content, &CompositionPlan::neutral(), None, theme, density)
}
