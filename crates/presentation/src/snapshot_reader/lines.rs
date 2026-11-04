//! The strict LF line cursor of the frame parser.
//!
//! A frame is LF-only text. (The manifest is JSON and has its own byte-level
//! checks in `manifest.rs`.) Lines split on `\n` alone, a carriage return
//! anywhere is rejected (it names its line), and a non-empty text must end with
//! a final LF. [`str::lines`] is deliberately not used: it would silently strip
//! a `\r` and accept a CRLF checkout whose bytes no longer match the digest the
//! author computed.

use super::{FrameParseError, FrameReason};

/// Why the text could not be split into strict LF lines.
pub(super) enum LexError {
    /// The 1-based `line` holds a carriage return.
    CarriageReturn { line: usize },
    /// The text ends without a final LF; `line` is the last, unterminated line.
    MissingFinalNewline { line: usize },
}

impl LexError {
    pub(super) fn into_frame(self) -> FrameParseError {
        match self {
            Self::CarriageReturn { line } => FrameParseError::Line {
                line,
                reason: FrameReason::CarriageReturn,
            },
            Self::MissingFinalNewline { line } => FrameParseError::Line {
                line,
                reason: FrameReason::MissingFinalNewline,
            },
        }
    }
}

/// One line that is not the record the grammar expects at this position.
pub(super) struct LineError {
    pub(super) line: usize,
    pub(super) expected: &'static str,
    pub(super) found: Option<String>,
}

impl LineError {
    pub(super) fn into_frame(self) -> FrameParseError {
        FrameParseError::Line {
            line: self.line,
            reason: match self.found {
                Some(found) => FrameReason::Unexpected {
                    expected: self.expected,
                    found,
                },
                None => FrameReason::Missing {
                    expected: self.expected,
                },
            },
        }
    }
}

/// The text's lines, read forward exactly once.
pub(super) struct Cursor<'a> {
    lines: Vec<&'a str>,
    next: usize,
}

impl<'a> Cursor<'a> {
    /// Split `text` into strict LF lines. Empty text has no lines, so the
    /// caller reports the missing format record instead.
    pub(super) fn new(text: &'a str) -> Result<Self, LexError> {
        if text.is_empty() {
            return Ok(Self {
                lines: Vec::new(),
                next: 0,
            });
        }
        let (body, terminated) = match text.strip_suffix('\n') {
            Some(body) => (body, true),
            None => (text, false),
        };
        let lines: Vec<&str> = body.split('\n').collect();
        if let Some(index) = lines.iter().position(|line| line.contains('\r')) {
            return Err(LexError::CarriageReturn { line: index + 1 });
        }
        if !terminated {
            return Err(LexError::MissingFinalNewline { line: lines.len() });
        }
        Ok(Self { lines, next: 0 })
    }

    /// The next line with its 1-based number, consuming it.
    pub(super) fn take(&mut self) -> Option<(usize, &'a str)> {
        let line = self.lines.get(self.next).copied()?;
        self.next += 1;
        Some((self.next, line))
    }

    /// The next line without consuming it.
    pub(super) fn peek(&self) -> Option<(usize, &'a str)> {
        self.lines
            .get(self.next)
            .copied()
            .map(|line| (self.next + 1, line))
    }

    /// How many lines are left; an upper bound for any count read from the
    /// input, so an untrusted count never sizes an allocation.
    pub(super) fn remaining(&self) -> usize {
        self.lines.len() - self.next
    }

    pub(super) fn eof_line(&self) -> usize {
        self.lines.len() + 1
    }
}

pub(super) fn take_raw<'a>(
    cursor: &mut Cursor<'a>,
    expected: &'static str,
) -> Result<(usize, &'a str), LineError> {
    cursor.take().ok_or(LineError {
        line: cursor.eof_line(),
        expected,
        found: None,
    })
}

/// Consume the next line, requiring it to start with `prefix`; the value is
/// the raw remainder, so `kind:Lyrics` and `header:` both read without
/// inventing a separator.
pub(super) fn take_after<'a>(
    cursor: &mut Cursor<'a>,
    prefix: &'static str,
) -> Result<(usize, &'a str), LineError> {
    let (line, text) = take_raw(cursor, prefix)?;
    let Some(value) = text.strip_prefix(prefix) else {
        return Err(LineError {
            line,
            expected: prefix,
            found: Some(text.to_owned()),
        });
    };
    Ok((line, value))
}

/// A plain decimal integer: digits only, no sign, no whitespace, and no
/// leading zero, so one number has one spelling.
pub(super) fn strict_u32(text: &str) -> Option<u32> {
    let canonical = !text.is_empty()
        && (text.len() == 1 || !text.starts_with('0'))
        && text.bytes().all(|byte| byte.is_ascii_digit());
    if canonical { text.parse().ok() } else { None }
}
