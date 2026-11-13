//! Manual slide boundaries inside one section's lyrics.
//!
//! A slide is a run of lines that hold visible content. One or more
//! whitespace-only lines separate two slides; leading and trailing blank lines
//! produce no slide. Stored lyrics are never normalized: every function here
//! reads the text as authored, and the edit helpers change only the bytes their
//! own documentation names.
//!
//! Every offset is a UTF-8 byte offset into the lyrics, the same unit the
//! editor caret reports. Callers must pass offsets on a character boundary.

use std::ops::Range;

/// One generated slide's source inside a section's lyrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlideSource {
    /// Byte range from the first line's start to the last line's end.
    pub range: Range<usize>,
    /// Byte ranges of each line, in order (blank lines excluded).
    pub lines: Vec<Range<usize>>,
}

/// Splits `lyrics` into the slides its blank lines delimit.
///
/// A newline alone continues the current slide. Any run of whitespace-only
/// lines is one boundary, whether it is one blank line or several, uses spaces,
/// tabs or carriage returns, or separates the first and last slide. Lines are
/// split on `\n`; a `\r` before it belongs to the line and is trimmed from the
/// slide body.
pub fn slides(lyrics: &str) -> Vec<SlideSource> {
    let mut generated = Vec::new();
    let mut current: Option<SlideSource> = None;
    for (range, blank) in lines(lyrics) {
        if blank {
            if let Some(slide) = current.take() {
                generated.push(slide);
            }
            continue;
        }
        match current.as_mut() {
            Some(slide) => {
                slide.range.end = range.end;
                slide.lines.push(range);
            }
            None => {
                current = Some(SlideSource {
                    range: range.clone(),
                    lines: vec![range],
                });
            }
        }
    }
    if let Some(slide) = current {
        generated.push(slide);
    }
    generated
}

/// The body to render for one slide: its lines with trailing whitespace
/// trimmed, joined by one `\n`.
///
/// The returned text is for rendering only; the stored lyrics keep their
/// original spacing. A range that does not belong to `lyrics` contributes
/// nothing rather than panicking.
pub fn slide_body(lyrics: &str, slide: &SlideSource) -> String {
    let mut body = String::new();
    for (index, line) in slide.lines.iter().enumerate() {
        if index > 0 {
            body.push('\n');
        }
        body.push_str(lyrics.get(line.clone()).unwrap_or_default().trim_end());
    }
    body
}

/// The slide the caret at `offset` belongs to.
///
/// An offset inside a slide names that slide. An offset on a blank line, at a
/// line break, or past the end names the nearest slide before it; an offset
/// before the first slide names the first slide. `None` when the lyrics have
/// no slide. Offsets beyond the end of the text are clamped to it.
pub fn slide_at(lyrics: &str, offset: usize) -> Option<usize> {
    let generated = slides(lyrics);
    if generated.is_empty() {
        return None;
    }
    let offset = offset.min(lyrics.len());
    let mut selected = 0;
    for (index, slide) in generated.iter().enumerate() {
        if slide.range.start > offset {
            break;
        }
        selected = index;
    }
    Some(selected)
}

/// The line to split after, counted from 1: `ceil(lines / 2)`.
///
/// `None` when the slide has one line, because a split needs a line on each
/// side.
pub fn split_point(slide: &SlideSource) -> Option<usize> {
    if slide.lines.len() < 2 {
        return None;
    }
    Some(slide.lines.len().div_ceil(2))
}

/// Inserts one `\n` after line `line` (counted from 1) of slide `slide`.
///
/// Every other byte is kept, so the caret and the authored spacing survive.
/// `None` when the slide is out of range or `line` is not an interior line of
/// it: the first line is a line, but the slide's last line cannot be split
/// after.
pub fn split_after(lyrics: &str, slide: usize, line: usize) -> Option<String> {
    let generated = slides(lyrics);
    let target = generated.get(slide)?;
    if line == 0 || line >= target.lines.len() {
        return None;
    }
    let position = target.lines[line - 1].end;
    let mut split = String::with_capacity(lyrics.len() + 1);
    split.push_str(&lyrics[..position]);
    split.push('\n');
    split.push_str(&lyrics[position..]);
    Some(split)
}

/// Joins slide `slide` with the slide before it.
///
/// The whole run of blank lines between the two slides becomes one `\n`; every
/// other byte is kept. `None` for the first slide because it has no previous
/// slide, and for a slide beyond the generated set.
pub fn join_with_previous(lyrics: &str, slide: usize) -> Option<String> {
    let generated = slides(lyrics);
    if slide == 0 {
        return None;
    }
    let previous = generated.get(slide - 1)?;
    let target = generated.get(slide)?;
    let start = previous.range.end;
    let end = target.range.start;
    let mut joined = String::with_capacity(lyrics.len() - (end - start) + 1);
    joined.push_str(&lyrics[..start]);
    joined.push('\n');
    joined.push_str(&lyrics[end..]);
    Some(joined)
}

/// Every line of `lyrics` with its byte range and whether it is blank.
///
/// The range excludes the line's `\n` but keeps any `\r` before it, so the
/// ranges together with their newlines cover the whole text.
fn lines(lyrics: &str) -> Vec<(Range<usize>, bool)> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in lyrics.bytes().enumerate() {
        if byte == b'\n' {
            lines.push((start..index, lyrics[start..index].trim().is_empty()));
            start = index + 1;
        }
    }
    lines.push((start..lyrics.len(), lyrics[start..].trim().is_empty()));
    lines
}
