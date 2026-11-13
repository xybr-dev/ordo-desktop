//! Shared rules for authored text.
//!
//! Authored text is kept exactly as the caller wrote it, including case and
//! spacing. Validation only trims to decide whether a value is present; the
//! stored value is never rewritten.

use crate::song_library::interface::{Field, SongLibraryError};

/// Rejects an authored value that is empty or only whitespace.
pub(crate) fn require_present(field: Field, value: &str) -> Result<(), SongLibraryError> {
    if value.trim().is_empty() {
        Err(SongLibraryError::BlankField(field))
    } else {
        Ok(())
    }
}

/// Rejects an optional authored value that is present but blank.
pub(crate) fn require_present_when_given(
    field: Field,
    value: Option<&str>,
) -> Result<(), SongLibraryError> {
    match value {
        Some(value) => require_present(field, value),
        None => Ok(()),
    }
}

/// Rejects a repeated value in a set-shaped list.
pub(crate) fn require_distinct<T: PartialEq>(
    field: Field,
    values: &[T],
) -> Result<(), SongLibraryError> {
    for (index, value) in values.iter().enumerate() {
        if values[..index].contains(value) {
            return Err(SongLibraryError::DuplicateField(field));
        }
    }
    Ok(())
}
