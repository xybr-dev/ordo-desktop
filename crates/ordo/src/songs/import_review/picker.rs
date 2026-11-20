//! The seam through which the review asks the operator for files, and the
//! routing of what was picked (Decision 21).

use std::path::{Path, PathBuf};

use gpui::{App, PathPromptOptions, Task};

/// Picks files. Production wraps `App::prompt_for_paths`; tests install a
/// fixed answer.
pub trait FilePicker {
    /// Asks for files, or for exactly one when `multiple` is false. `None`
    /// when the operator cancelled.
    fn pick(&self, multiple: bool, cx: &mut App) -> Task<Option<Vec<PathBuf>>>;
}

/// The system file dialog: files only, no directories. GPUI's prompt has no
/// file-type filter at the pinned revision, so [`route`] checks the choice.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemPicker;

impl FilePicker for SystemPicker {
    fn pick(&self, multiple: bool, cx: &mut App) -> Task<Option<Vec<PathBuf>>> {
        let answer = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |_| match answer.await {
            Ok(Ok(paths)) => paths,
            _ => None,
        })
    }
}

/// Where a picked set of files goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Picked {
    /// One or more `.ordo` files: Review Song import.
    Ordo(Vec<PathBuf>),
    /// One Markdown or text file: the Markdown draft.
    Markdown(PathBuf),
    /// Anything else: nothing opens.
    Refused,
}

/// The lower-cased extension of `path`, if it has one.
fn extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
}

/// Routes by extension, case-insensitively: every file `.ordo` is a review,
/// exactly one `.md` or `.txt` file is a draft, and every other mix
/// (several Markdown files, Markdown with `.ordo`, other extensions, no
/// files) is refused.
pub fn route(paths: &[PathBuf]) -> Picked {
    let extensions: Vec<Option<String>> = paths.iter().map(|path| extension(path)).collect();
    if !paths.is_empty()
        && extensions
            .iter()
            .all(|extension| extension.as_deref() == Some("ordo"))
    {
        return Picked::Ordo(paths.to_vec());
    }
    match (paths, extensions.as_slice()) {
        ([path], [Some(extension)]) if matches!(extension.as_str(), "md" | "txt") => {
            Picked::Markdown(path.clone())
        }
        _ => Picked::Refused,
    }
}
