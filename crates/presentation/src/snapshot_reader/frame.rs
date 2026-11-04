//! The canonical `ordo-fixture/1` frame grammar.

use rendering::{ForegroundImage, TemplateContent, TemplateKind};

use super::lines::{Cursor, LexError, LineError, strict_u32, take_after, take_raw};
use super::{FRAME_FORMAT, FrameParseError, FrameReason};

/// Parse one canonical `ordo-fixture/1` frame file.
///
/// This is deliberately the same grammar the golden-frame harness reads in
/// `crates/rendering/tests/golden.rs`: the harness proves composition stability,
/// and the reader proves a snapshot's frames are exactly that content. The
/// parser does not render; [`read_snapshot`] does that once the digest matched.
pub(super) fn parse_frame(text: &str) -> Result<TemplateContent, FrameParseError> {
    let mut cursor = Cursor::new(text).map_err(LexError::into_frame)?;
    let (line, format) = take_raw(&mut cursor, FRAME_FORMAT).map_err(LineError::into_frame)?;
    if format != FRAME_FORMAT {
        return Err(FrameParseError::Line {
            line,
            reason: FrameReason::UnknownFormat {
                found: format.to_owned(),
            },
        });
    }

    let (at, kind) = take_after(&mut cursor, "kind:").map_err(LineError::into_frame)?;
    let kind = match kind {
        "Lyrics" => TemplateKind::Lyrics,
        "Response" => TemplateKind::Response,
        "ReadingIdentification" => TemplateKind::ReadingIdentification,
        "ReadingPassage" => TemplateKind::ReadingPassage,
        "Psalm" => TemplateKind::Psalm,
        "SongIdentification" => TemplateKind::SongIdentification,
        "CelebrationTitle" => TemplateKind::CelebrationTitle,
        "GeneralText" => TemplateKind::GeneralText,
        "GeneralImage" => TemplateKind::GeneralImage,
        "Black" => TemplateKind::Black,
        _ => {
            return Err(FrameParseError::Line {
                line: at,
                reason: FrameReason::InvalidKind {
                    value: kind.to_owned(),
                },
            });
        }
    };

    let (at, header) = take_raw(&mut cursor, "header").map_err(LineError::into_frame)?;
    let header = if header == "header-absent" {
        None
    } else if let Some(text) = header.strip_prefix("header:") {
        Some(text.to_owned())
    } else {
        return Err(FrameParseError::Line {
            line: at,
            reason: FrameReason::Unexpected {
                expected: "`header:<text>` or `header-absent`",
                found: header.to_owned(),
            },
        });
    };

    let body = take_lines(&mut cursor, "body-lines:")?;

    let (at, footer) = take_raw(&mut cursor, "footer").map_err(LineError::into_frame)?;
    let footer = if footer == "footer-absent" {
        None
    } else if let Some(text) = footer.strip_prefix("footer-lines:") {
        Some(take_counted_lines(&mut cursor, text, "footer-lines", at)?.join("\n"))
    } else {
        return Err(FrameParseError::Line {
            line: at,
            reason: FrameReason::Unexpected {
                expected: "`footer-lines:<n>` or `footer-absent`",
                found: footer.to_owned(),
            },
        });
    };

    let (at, image) = take_raw(&mut cursor, "image").map_err(LineError::into_frame)?;
    let image = if image == "image-absent" {
        None
    } else if let Some(id) = image.strip_prefix("image:") {
        // Only the General Image template composes a foreground image; on any
        // other kind the record would be silently ignored by composition, so
        // it is refused here rather than carried as a dead declaration.
        if kind != TemplateKind::GeneralImage {
            return Err(FrameParseError::Line {
                line: at,
                reason: FrameReason::ImageOnNonImageFrame,
            });
        }
        if id.is_empty() {
            return Err(FrameParseError::Line {
                line: at,
                reason: FrameReason::Unexpected {
                    expected: "a non-empty `image:<id>`",
                    found: image.to_owned(),
                },
            });
        }
        let (size_at, size) =
            take_after(&mut cursor, "image-size:").map_err(LineError::into_frame)?;
        let (natural_width, natural_height) =
            parse_image_size(size).ok_or_else(|| FrameParseError::Line {
                line: size_at,
                reason: FrameReason::InvalidSize {
                    value: size.to_owned(),
                },
            })?;
        Some(ForegroundImage {
            id: id.to_owned(),
            natural_width,
            natural_height,
        })
    } else {
        return Err(FrameParseError::Line {
            line: at,
            reason: FrameReason::Unexpected {
                expected: "`image:<id>` or `image-absent`",
                found: image.to_owned(),
            },
        });
    };

    if let Some((line, _)) = cursor.peek() {
        return Err(FrameParseError::Line {
            line,
            reason: FrameReason::Trailing,
        });
    }

    Ok(TemplateContent {
        kind,
        header,
        body,
        footer,
        image,
    })
}

fn take_lines(cursor: &mut Cursor<'_>, prefix: &'static str) -> Result<String, FrameParseError> {
    let (at, count) = take_after(cursor, prefix).map_err(LineError::into_frame)?;
    take_counted_lines(cursor, count, prefix, at).map(|lines| lines.join("\n"))
}

fn take_counted_lines(
    cursor: &mut Cursor<'_>,
    count: &str,
    key: &'static str,
    at: usize,
) -> Result<Vec<String>, FrameParseError> {
    let count = strict_u32(count).ok_or_else(|| FrameParseError::Line {
        line: at,
        reason: FrameReason::InvalidCount {
            value: count.to_owned(),
        },
    })? as usize;
    // The count is untrusted: it may only bound the loop, never an allocation.
    // Reserving what is actually left keeps a hostile `body-lines:4000000000`
    // from aborting the process; the loop then fails at the end of the text.
    let mut lines = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let (_, line) = take_raw(cursor, key).map_err(LineError::into_frame)?;
        lines.push(line.to_owned());
    }
    Ok(lines)
}

fn parse_image_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once('x')?;
    Some((strict_u32(width)?, strict_u32(height)?))
}
