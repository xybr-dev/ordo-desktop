//! Canonical Slide Renderer Interface (T0.4 PR 3).
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
//! Proportional zone reservation below is provisional: PR 4 replaces it with
//! measured content heights. The pinned invariant is the space return, not
//! the proportion.

/// Logical canvas width every slide composes in.
pub const LOGICAL_WIDTH: u32 = 1920;
/// Logical canvas height every slide composes in.
pub const LOGICAL_HEIGHT: u32 = 1080;
/// Minimum horizontal safe-area inset (left and right).
pub const MIN_SAFE_INSET_X: u32 = 120;
/// Minimum vertical safe-area inset (top and bottom).
pub const MIN_SAFE_INSET_Y: u32 = 80;

/// Bands the safe-area height is divided into for zone reservation.
const BAND_COUNT: u32 = 6;
/// Bands reserved by a present header.
const HEADER_BANDS: u32 = 1;
/// Bands reserved by a present footer.
const FOOTER_BANDS: u32 = 1;

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
/// `body` carries the resolved text with authored line breaks preserved.
/// `response_header` and `psalm_header` each carry an already-resolved
/// authored header string or nothing; `Some` reserves the header zone even
/// when the string is empty, while `None` collapses it so the body receives
/// the space. At most one of the two headers may be present. `footer` works
/// the same way for the footer zone (for example a reference or caption).
/// No field here names a language, and no constructor derives text from one.
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
/// which is always whole. PR 3 emits none; later PRs add readiness warnings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    pub warnings: Vec<String>,
}

/// One complete logical composition.
///
/// Fields are private so a partially composed frame cannot be named: the
/// only way to obtain one is [`render`], which returns every zone at once.
/// `header` and `footer` are `None` exactly when the corresponding input was
/// absent; the body then covers their share.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    safe_area: Rect,
    header: Option<Rect>,
    body: Rect,
    footer: Option<Rect>,
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

    /// The logical canvas size this frame was composed for.
    pub fn logical_size(&self) -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
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

    let safe_area = Rect {
        x: theme.safe_inset_x,
        y: theme.safe_inset_y,
        width: safe_width,
        height: safe_height,
    };
    let band = safe_height / BAND_COUNT;
    let has_header = content.response_header.is_some() || content.psalm_header.is_some();
    let has_footer = content.footer.is_some();
    // ponytail: proportional reservation until PR 4 measures content; the
    // space-return rule it implements is the pinned part.
    let header_height = if has_header { HEADER_BANDS * band } else { 0 };
    let footer_height = if has_footer { FOOTER_BANDS * band } else { 0 };
    let header = has_header.then_some(Rect {
        x: safe_area.x,
        y: safe_area.y,
        width: safe_area.width,
        height: header_height,
    });
    let footer_y = safe_area.y + safe_height - footer_height;
    let footer = has_footer.then_some(Rect {
        x: safe_area.x,
        y: footer_y,
        width: safe_area.width,
        height: footer_height,
    });
    let body = Rect {
        x: safe_area.x,
        y: safe_area.y + header_height,
        width: safe_area.width,
        height: safe_height - header_height - footer_height,
    };
    Ok((
        Frame {
            safe_area,
            header,
            body,
            footer,
        },
        Diagnostics::default(),
    ))
}
