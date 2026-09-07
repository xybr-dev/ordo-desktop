//! Canonical congregation-slide rendering.
//!
//! Rendering behavior begins in T0.4. This crate may consume domain values but
//! must not acquire GPUI operator-layout behavior.

pub mod fonts;
pub mod interface;
pub mod shaping;

pub use interface::{
    Diagnostics, Frame, LOGICAL_HEIGHT, LOGICAL_WIDTH, MIN_SAFE_INSET_X, MIN_SAFE_INSET_Y, Rect,
    RenderError, SemanticContent, TargetDensity, Theme, render,
};
pub use shaping::{
    BODY_FONT_SIZE, BODY_LINE_HEIGHT, Fit, PaginationInputs, PlacedGlyph, ShapedLine,
};
