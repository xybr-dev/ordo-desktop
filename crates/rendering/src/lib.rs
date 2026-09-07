//! Canonical congregation-slide rendering.
//!
//! Rendering behavior begins in T0.4. This crate may consume domain values but
//! must not acquire GPUI operator-layout behavior.

pub mod fonts;
pub mod interface;
pub mod shaping;
pub mod templates;

pub use interface::{
    Diagnostics, Frame, LOGICAL_HEIGHT, LOGICAL_WIDTH, MIN_SAFE_INSET_X, MIN_SAFE_INSET_Y, Rect,
    RenderError, SemanticContent, TargetDensity, Theme, render,
};
pub use shaping::{
    Alignment, BODY_FONT_SIZE, BODY_LINE_HEIGHT, Fit, PaginationInputs, PlacedGlyph, ShapedLine,
    line_height_for,
};
pub use templates::{
    PROVISIONAL_DISPLAY_SIZE, PROVISIONAL_META_SIZE, PROVISIONAL_PASSAGE_SIZE, TemplateContent,
    TemplateKind, render_template,
};
