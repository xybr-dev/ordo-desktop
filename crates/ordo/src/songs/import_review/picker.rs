//! The seam through which the review asks the operator for files.

use std::path::PathBuf;

use gpui::{App, Task};

/// Picks files. Production wraps `App::prompt_for_paths`; tests install a
/// fixed answer.
pub trait FilePicker {
    /// Asks for files, or for exactly one when `multiple` is false. `None`
    /// when the operator cancelled.
    fn pick(&self, multiple: bool, cx: &mut App) -> Task<Option<Vec<PathBuf>>>;
}
