//! Slide boundaries inside a section's lyrics.
//!
//! T2.7 makes the blank source line the manual slide boundary (Decision 116).
//! [`manual_boundaries`] parses the authored text into slides and applies the
//! Split and Join edits without normalizing anything else. T3.1 later adds the
//! pagination proposal and fit checks beside it, reading the same rule.

pub mod manual_boundaries;

pub use manual_boundaries::{
    SlideSource, join_with_previous, slide_at, slide_body, slides, split_after, split_point,
};
