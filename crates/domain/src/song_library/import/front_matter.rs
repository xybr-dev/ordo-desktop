//! The strict YAML subset Markdown front matter uses (Decision 22).
//!
//! Supported: `key: value`, `key: [a, b]`, block lists (`- item` lines under
//! `key:`), single- and double-quoted scalars, and `#` comments. Nothing else
//! is YAML here; anything outside the subset is a [`IssueKind::BadFrontMatter`]
//! with the offending line.

use super::markdown::{IssueKind, MarkdownIssue};

/// A front matter value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Value {
    /// One scalar; empty when the key has no value.
    Text(String),
    /// A flow or block list.
    List(Vec<String>),
}

/// One `key: value` entry and where it was written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    /// The key as written.
    pub(crate) key: String,
    /// Its value.
    pub(crate) value: Value,
    /// The 1-based line of the key.
    pub(crate) line: usize,
}

/// Splits a document into its front matter entries and the 1-based line the
/// body starts on. `text` has no BOM; its first line must be `---`.
pub(crate) fn front_matter(text: &str) -> Result<(Vec<Entry>, usize), MarkdownIssue> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|line| line.trim_end()) != Some("---") {
        return Err(issue(1, IssueKind::NoFrontMatter));
    }
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|line| line.trim_end() == "---")
        .map(|offset| offset + 1)
    else {
        return Err(bad(1, "The front matter isn't closed with ---."));
    };

    let mut entries: Vec<Entry> = Vec::new();
    // The key a block list is being collected for, while its items arrive.
    let mut open_list: Option<usize> = None;
    // Keys written with no value; one that collects no items is an empty scalar.
    let mut valueless: Vec<usize> = Vec::new();
    for (index, raw) in lines.iter().enumerate().take(close).skip(1) {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(item) = trimmed
            .strip_prefix('-')
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        {
            let Some(entry) = open_list.map(|slot| &mut entries[slot]) else {
                return Err(bad(line, "A list item needs a key above it."));
            };
            let item = scalar(item, line)?;
            match &mut entry.value {
                Value::List(items) => items.push(item),
                Value::Text(_) => return Err(bad(line, "A list item needs a key above it.")),
            }
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            return Err(bad(line, "Indentation is only for list items."));
        }
        let Some((key, rest)) = trimmed.split_once(':') else {
            return Err(bad(line, "Expected key: value."));
        };
        let key = key.trim();
        if !valid_key(key) {
            return Err(bad(line, "Expected key: value."));
        }
        if entries.iter().any(|entry| entry.key == key) {
            return Err(bad(line, &format!("{key} appears twice.")));
        }
        let rest = rest.trim();
        let value = if rest.is_empty() || rest.starts_with('#') {
            // A key with no value opens a block list if items follow; it
            // stays an empty scalar if none do.
            open_list = Some(entries.len());
            valueless.push(entries.len());
            Value::List(Vec::new())
        } else {
            open_list = None;
            if rest.starts_with('[') {
                Value::List(flow_list(rest, line)?)
            } else {
                Value::Text(scalar(rest, line)?)
            }
        };
        entries.push(Entry {
            key: key.to_owned(),
            value,
            line,
        });
    }
    for slot in valueless {
        if entries[slot].value == Value::List(Vec::new()) {
            entries[slot].value = Value::Text(String::new());
        }
    }
    Ok((entries, close + 2))
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn issue(line: usize, kind: IssueKind) -> MarkdownIssue {
    MarkdownIssue {
        line,
        kind,
        blocking: true,
    }
}

fn bad(line: usize, message: &str) -> MarkdownIssue {
    issue(line, IssueKind::BadFrontMatter(message.to_owned()))
}

/// One scalar: quoted, or plain with a trailing ` #comment` removed.
fn scalar(text: &str, line: usize) -> Result<String, MarkdownIssue> {
    let text = text.trim();
    let (value, rest) = scalar_prefix(text, line)?;
    let rest = rest.trim();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return Err(bad(line, "Unexpected text after a quoted value."));
    }
    Ok(value)
}

/// The scalar at the start of `text` and what follows it.
fn scalar_prefix(text: &str, line: usize) -> Result<(String, &str), MarkdownIssue> {
    let mut chars = text.char_indices();
    match chars.next() {
        Some((_, quote @ ('"' | '\''))) => {
            let mut value = String::new();
            let mut escaped = false;
            let mut rest = text[1..].char_indices().peekable();
            while let Some((offset, c)) = rest.next() {
                if quote == '"' && escaped {
                    value.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                    escaped = false;
                } else if quote == '"' && c == '\\' {
                    escaped = true;
                } else if c == quote {
                    if quote == '\'' && rest.peek().is_some_and(|(_, next)| *next == '\'') {
                        value.push('\'');
                        rest.next();
                    } else {
                        return Ok((value, &text[offset + 2..]));
                    }
                } else {
                    value.push(c);
                }
            }
            Err(bad(line, "A quoted value isn't closed."))
        }
        _ => {
            let end = plain_end(text);
            Ok((text[..end].trim_end().to_owned(), &text[end..]))
        }
    }
}

/// Where a plain scalar ends: at a `#` that starts a comment.
fn plain_end(text: &str) -> usize {
    let mut previous_space = true;
    for (offset, c) in text.char_indices() {
        if c == '#' && previous_space {
            return offset;
        }
        previous_space = c.is_whitespace();
    }
    text.len()
}

/// A `[a, b]` list on one line.
fn flow_list(text: &str, line: usize) -> Result<Vec<String>, MarkdownIssue> {
    let mut rest = text[1..].trim_start();
    let mut items = Vec::new();
    loop {
        if let Some(after) = rest.strip_prefix(']') {
            let after = after.trim();
            return if after.is_empty() || after.starts_with('#') {
                Ok(items)
            } else {
                Err(bad(line, "Unexpected text after the list."))
            };
        }
        let (item, after) = if rest.starts_with(['"', '\'']) {
            scalar_prefix(rest, line)?
        } else {
            let end = rest
                .find([',', ']'])
                .ok_or_else(|| bad(line, "A list isn't closed with ]."))?;
            (rest[..end].trim().to_owned(), &rest[end..])
        };
        items.push(item);
        let after = after.trim_start();
        if let Some(next) = after.strip_prefix(',') {
            rest = next.trim_start();
        } else if after.starts_with(']') {
            rest = after;
        } else {
            return Err(bad(line, "A list isn't closed with ]."));
        }
    }
}
