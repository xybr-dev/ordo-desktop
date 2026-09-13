//! Canonical congregation-slide rendering.
//!
//! Rendering behavior begins in T0.4. This crate may consume domain values but
//! must not acquire GPUI operator-layout behavior.

pub mod fonts;
pub mod interface;
pub mod raster;
pub mod shaping;
pub mod templates;

pub use interface::{
    Diagnostics, ForegroundImage, Frame, LOGICAL_HEIGHT, LOGICAL_WIDTH, MIN_SAFE_INSET_X,
    MIN_SAFE_INSET_Y, PlacedImage, Rect, RenderError, SemanticContent, SlideBackground,
    TargetDensity, Theme, render,
};
pub use raster::{RasterError, RasterizedFrame, rasterize};
pub use shaping::{
    Alignment, BODY_FONT_SIZE, BODY_LINE_HEIGHT, CELEBRATION_SUBTITLE_WEIGHT,
    CELEBRATION_TITLE_WEIGHT, Fit, PaginationInputs, PlacedGlyph, REGULAR_WEIGHT, ShapedLine,
    line_height_for,
};
pub use templates::{
    CELEBRATION_SUBTITLE_SIZE, CELEBRATION_TITLE_SIZE, PROVISIONAL_DISPLAY_SIZE,
    PROVISIONAL_META_SIZE, PROVISIONAL_PASSAGE_SIZE, READING_PASSAGE_SIZE, RESPONSE_SIZE,
    TemplateContent, TemplateKind, render_template,
};
