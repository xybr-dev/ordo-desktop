//! Development-only static-image `.pptx` spike (T0.9 WP1, plan "Deck shape").
//!
//! The PresentationML parts are written by hand into a ZIP. [`build`] produces
//! the bytes, [`validate`] reopens them and checks them against a [`DeckPlan`],
//! and [`export`] publishes them atomically. No production crate may depend on
//! this crate.

mod fixture;
mod package;
mod plan;
mod validate;
mod xmlscan;

#[cfg(test)]
mod tests;

use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File},
    io::{self, Cursor, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub use fixture::{fixture_plan, frame};
pub use validate::validate;

/// Width of every congregation frame, in pixels.
pub const FRAME_WIDTH: u32 = 1920;
/// Height of every congregation frame, in pixels.
pub const FRAME_HEIGHT: u32 = 1080;
/// Slide width in EMU (16:9).
pub const SLIDE_CX: u64 = 12_192_000;
/// Slide height in EMU (16:9).
pub const SLIDE_CY: u64 = 6_858_000;

/// A deck to write: Mass items in order, each becoming one PowerPoint section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckPlan {
    pub items: Vec<MassItem>,
}

/// One top-level Mass item and its Cues in presentation order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MassItem {
    pub title: String,
    pub cues: Vec<Cue>,
}

/// A single presentable step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cue {
    /// A congregation frame: an already-rendered 1920x1080 PNG.
    Image { png: Vec<u8> },
    /// An operator-only step: a black slide whose text lives in the notes.
    OperatorOnly { notes: String },
}

/// Why a deck was not published.
#[derive(Debug)]
pub enum ExportError {
    /// Image Cue `cue` of item `item` (both zero-based) is not a usable frame.
    BadFrame {
        item: usize,
        cue: usize,
        reason: String,
    },
    /// A filesystem error. The destination was not touched.
    Io(io::Error),
    /// The written archive failed validation. Nothing was published.
    Validation(Vec<String>),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::BadFrame { item, cue, reason } => {
                write!(f, "item {item} cue {cue}: frame rejected: {reason}")
            }
            ExportError::Io(error) => write!(f, "I/O error: {error}"),
            ExportError::Validation(failures) => {
                write!(f, "validation failed: {}", failures.join("; "))
            }
        }
    }
}

impl std::error::Error for ExportError {}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        ExportError::Io(error)
    }
}

/// Deck title written to `docProps/core.xml`.
const DECK_TITLE: &str = "Ordo emergency deck";

/// Builds the complete `.pptx` bytes for `plan`.
///
/// Every image Cue must decode as a PNG of exactly 1920x1080; otherwise the
/// first offending Cue is reported as [`ExportError::BadFrame`].
pub fn build(plan: &DeckPlan) -> Result<Vec<u8>, ExportError> {
    for (item_index, item) in plan.items.iter().enumerate() {
        for (cue_index, cue) in item.cues.iter().enumerate() {
            if let Cue::Image { png } = cue {
                let reason = match png_dims(png) {
                    Ok((FRAME_WIDTH, FRAME_HEIGHT)) => continue,
                    Ok((width, height)) => {
                        format!("expected {FRAME_WIDTH}x{FRAME_HEIGHT}, got {width}x{height}")
                    }
                    Err(reason) => format!("not a decodable PNG: {reason}"),
                };
                return Err(ExportError::BadFrame {
                    item: item_index,
                    cue: cue_index,
                    reason,
                });
            }
        }
    }
    let (slides, sections) = plan::plan_slides(plan);
    let parts = package::package(&slides, &sections, DECK_TITLE);
    zip_parts(&parts).map_err(ExportError::Io)
}

/// Writes `plan` to `dest` atomically.
///
/// The deck is written to a temporary sibling, synced, read back from disk and
/// validated, then renamed over `dest`. On any failure the temporary is removed
/// and `dest` is left as it was.
pub fn export(dest: &Path, plan: &DeckPlan) -> Result<(), ExportError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let bytes = build(plan)?;
    let file_name = dest
        .file_name()
        .ok_or_else(|| ExportError::Io(io::Error::other("destination has no file name")))?;
    // A bare file name has an empty parent; the temporary goes in the cwd.
    let parent = match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let temp = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name.to_string_lossy(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // `create_new` never overwrites. If it fails the path is not ours, so this
    // early return must not reach the cleanup below.
    let file = File::create_new(&temp)?;
    let result = publish(file, &temp, dest, &bytes, plan);
    if result.is_err() {
        // Best effort: this call created the temporary, and a failed removal
        // must not mask the error that caused the cleanup.
        let _ = fs::remove_file(&temp);
    }
    result
}

fn publish(
    file: File,
    temp: &Path,
    dest: &Path,
    bytes: &[u8],
    plan: &DeckPlan,
) -> Result<(), ExportError> {
    let mut file = file;
    file.write_all(bytes)?;
    file.sync_all()?;
    // The handle must close before the rename on Windows.
    drop(file);
    let written = fs::read(temp)?;
    validate(&written, plan).map_err(ExportError::Validation)?;
    fs::rename(temp, dest)?;
    Ok(())
}

fn zip_parts(parts: &[(String, Vec<u8>)]) -> io::Result<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in parts {
        zip.start_file(name.as_str(), options)
            .map_err(io::Error::other)?;
        zip.write_all(bytes)?;
    }
    let cursor = zip.finish().map_err(io::Error::other)?;
    Ok(cursor.into_inner())
}

/// Decodes a PNG fully and returns its dimensions.
pub(crate) fn png_dims(bytes: &[u8]) -> Result<(u32, u32), String> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|error| error.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "image too large to decode".to_owned())?;
    let mut buffer = vec![0; size];
    reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let info = reader.info();
    Ok((info.width, info.height))
}

/// Lowercase hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
