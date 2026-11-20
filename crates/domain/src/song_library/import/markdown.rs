//! The Markdown song parser (Decision 22).
//!
//! A human writes a song as a Markdown or text file: YAML front matter for the
//! Song and Arrangement details, then the words with `[Label]` section
//! markers. [`parse`] turns the bytes into the same [`ImportDocument`] a
//! machine import produces, plus the issues a writer should see. It is pure:
//! the caller supplies the new draft's identities.

use crate::identity::{ArrangementId, SongId};
use crate::song_library::{SongDraft, match_key};

use super::front_matter::{Entry, Value, front_matter};
use super::{
    ImportArrangement, ImportDocument, ImportSection, SequenceEntry, rendition_attribution,
    resolve_language,
};

/// A parsed Markdown song and the issues found while parsing it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownDraft {
    /// The song as far as it could be read; its single Arrangement carries
    /// the caller's identity.
    pub document: ImportDocument,
    /// Every issue, in file order.
    pub issues: Vec<MarkdownIssue>,
}

/// One thing a writer should fix or know about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownIssue {
    /// The 1-based line it is about.
    pub line: usize,
    /// What it is.
    pub kind: IssueKind,
    /// Whether it stops the draft from being saved; follows from the kind.
    pub blocking: bool,
}

/// What an issue is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssueKind {
    /// The file isn't UTF-8 text (blocking).
    NotUtf8,
    /// The file has no front matter (blocking).
    NoFrontMatter,
    /// The front matter is malformed, with the reason (blocking).
    BadFrontMatter(String),
    /// There is no title, or it is blank (blocking).
    MissingTitle,
    /// A front matter key Ordo doesn't use; ignored.
    UnknownKey(String),
    /// A language that isn't one of Ordo's, as written (blocking).
    UnlistedLanguage(String),
    /// No language is given (blocking): an Arrangement needs one.
    NoLanguage,
    /// A `[Repeat: label]` that names no section.
    UnresolvedRepeat(String),
    /// A `[Repeat: label]` that names several sections.
    AmbiguousRepeat {
        /// The label as written.
        label: String,
        /// How many sections carry it.
        sections: usize,
    },
    /// A section with no words; `None` for an unlabeled one, or when the file
    /// has no sections at all.
    NoWords(Option<String>),
    /// A bracket line that isn't a section marker; kept as lyrics.
    NotALabel(String),
}

impl IssueKind {
    /// Whether this kind of issue stops a draft from being saved.
    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            Self::NotUtf8
                | Self::NoFrontMatter
                | Self::BadFrontMatter(_)
                | Self::MissingTitle
                | Self::UnlistedLanguage(_)
                | Self::NoLanguage
        )
    }
}

impl MarkdownDraft {
    /// Whether the draft can be saved: no blocking issue.
    pub fn can_save(&self) -> bool {
        !self.issues.iter().any(|issue| issue.blocking)
    }
}

fn issue(line: usize, kind: IssueKind) -> MarkdownIssue {
    MarkdownIssue {
        line,
        blocking: kind.is_blocking(),
        kind,
    }
}

/// Parses a Markdown or text song. The caller supplies the new draft's Song
/// and Arrangement ids (Markdown carries none).
pub fn parse(bytes: &[u8], ids: (SongId, ArrangementId)) -> MarkdownDraft {
    let (song_id, arrangement_id) = ids;
    let mut document = ImportDocument {
        id: song_id,
        song: SongDraft::default(),
        arrangements: vec![ImportArrangement {
            id: arrangement_id,
            title: None,
            languages: Vec::new(),
            choirs: Vec::new(),
            rendition_attribution: None,
            occasions: Vec::new(),
            notes: None,
            sections: Vec::new(),
            sequence: Vec::new(),
            warnings: Vec::new(),
        }],
        warnings: Vec::new(),
    };
    let mut issues = Vec::new();
    let Ok(text) = std::str::from_utf8(bytes) else {
        issues.push(issue(1, IssueKind::NotUtf8));
        return MarkdownDraft { document, issues };
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let body_start = match front_matter(text) {
        Ok((entries, start)) => {
            apply_front_matter(&entries, &mut document, &mut issues);
            Some(start)
        }
        Err(found) => {
            // Without front matter the whole file is still the body, so the
            // preview shows it; a broken front matter hides the body.
            let whole_file = found.kind == IssueKind::NoFrontMatter;
            issues.push(found);
            whole_file.then_some(1)
        }
    };
    if let Some(start) = body_start {
        let arrangement = &mut document.arrangements[0];
        parse_body(text, start, arrangement, &mut issues);
    }
    issues.sort_by_key(|found| found.line);
    MarkdownDraft { document, issues }
}

/// A scalar key's text; `None` when blank. A list is a blocking issue.
fn scalar(entry: &Entry, issues: &mut Vec<MarkdownIssue>) -> Option<String> {
    match &entry.value {
        Value::Text(text) => Some(text.trim().to_owned()).filter(|text| !text.is_empty()),
        Value::List(_) => {
            issues.push(issue(
                entry.line,
                IssueKind::BadFrontMatter(format!("{} takes one value, not a list.", entry.key)),
            ));
            None
        }
    }
}

/// A list key's items; one scalar counts as a list of one. Blanks are dropped.
fn list(entry: &Entry) -> Vec<String> {
    let items = match &entry.value {
        Value::Text(text) => vec![text.as_str()],
        Value::List(items) => items.iter().map(String::as_str).collect(),
    };
    items
        .into_iter()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn apply_front_matter(
    entries: &[Entry],
    document: &mut ImportDocument,
    issues: &mut Vec<MarkdownIssue>,
) {
    let song = &mut document.song;
    let arrangement = &mut document.arrangements[0];
    let (mut translator, mut adapter) = (None, None);
    let mut title_line = None;
    let mut language_line = None;
    let mut unlisted_language = false;
    for entry in entries {
        match entry.key.as_str() {
            "title" => {
                title_line = Some(entry.line);
                song.title = scalar(entry, issues).unwrap_or_default();
            }
            "aliases" => song.aliases = list(entry),
            "composer" => song.original_attribution = scalar(entry, issues),
            "source" => song.provenance = scalar(entry, issues),
            "rights" => song.rights_notes = scalar(entry, issues),
            "arrangement" => arrangement.title = scalar(entry, issues),
            "language" => {
                language_line = Some(entry.line);
                for value in list(entry) {
                    match resolve_language(&value) {
                        Some(name) if arrangement.languages.iter().any(|have| have == name) => {}
                        Some(name) => arrangement.languages.push(name.to_owned()),
                        None => {
                            unlisted_language = true;
                            issues.push(issue(entry.line, IssueKind::UnlistedLanguage(value)));
                        }
                    }
                }
            }
            "choirs" => arrangement.choirs = list(entry),
            "translator" => translator = scalar(entry, issues),
            "adapter" => adapter = scalar(entry, issues),
            "liturgical_moments" => song.liturgical_moments = list(entry),
            "occasions" => arrangement.occasions = list(entry),
            "notes" => arrangement.notes = scalar(entry, issues),
            other => issues.push(issue(entry.line, IssueKind::UnknownKey(other.to_owned()))),
        }
    }
    arrangement.rendition_attribution =
        rendition_attribution(translator.as_deref(), adapter.as_deref());
    if song.title.trim().is_empty() {
        issues.push(issue(title_line.unwrap_or(1), IssueKind::MissingTitle));
    }
    if arrangement.languages.is_empty() && !unlisted_language {
        issues.push(issue(language_line.unwrap_or(1), IssueKind::NoLanguage));
    }
}

/// One section while the body is being read.
struct Building {
    label: Option<String>,
    line: usize,
    slides: Vec<Vec<String>>,
    current: Vec<String>,
}

impl Building {
    fn new(label: Option<String>, line: usize) -> Self {
        Self {
            label,
            line,
            slides: Vec::new(),
            current: Vec::new(),
        }
    }

    /// Ends the slide being collected, if it has lines.
    fn close_slide(&mut self) {
        if !self.current.is_empty() {
            self.slides.push(std::mem::take(&mut self.current));
        }
    }
}

/// One Sequence position before repeats are resolved.
enum Item {
    Section(usize),
    Repeat { label: String, line: usize },
}

/// What a trimmed body line is.
enum Marker {
    Repeat(String),
    Unlabeled,
    Label(String),
    Malformed,
    Text,
}

fn classify(trimmed: &str) -> Marker {
    if !trimmed.starts_with('[') {
        return Marker::Text;
    }
    let Some(inner) = trimmed.strip_suffix(']').map(|without| without[1..].trim()) else {
        // An opening bracket with no closing one anywhere is a broken marker;
        // "[Instrumental] la la" is lyrics.
        return if trimmed.contains(']') {
            Marker::Text
        } else {
            Marker::Malformed
        };
    };
    if inner.is_empty() || inner.contains(['[', ']']) {
        return Marker::Malformed;
    }
    if let Some(head) = inner.get(..6)
        && head.eq_ignore_ascii_case("repeat")
        && let Some(label) = inner[6..].trim_start().strip_prefix(':')
    {
        let label = label.trim();
        return if label.is_empty() {
            Marker::Malformed
        } else {
            Marker::Repeat(label.to_owned())
        };
    }
    if inner.eq_ignore_ascii_case("unlabeled") {
        return Marker::Unlabeled;
    }
    Marker::Label(inner.to_owned())
}

fn parse_body(
    text: &str,
    body_start: usize,
    arrangement: &mut ImportArrangement,
    issues: &mut Vec<MarkdownIssue>,
) {
    let mut sections: Vec<Building> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut current: Option<usize> = None;

    for (index, raw) in text.lines().enumerate().skip(body_start - 1) {
        let line = index + 1;
        let trimmed = raw.trim();
        match classify(trimmed) {
            Marker::Label(label) => {
                start_section(&mut sections, &mut items, &mut current, Some(label), line);
            }
            Marker::Unlabeled => {
                start_section(&mut sections, &mut items, &mut current, None, line);
            }
            Marker::Repeat(label) => {
                if let Some(slot) = current.take() {
                    sections[slot].close_slide();
                }
                items.push(Item::Repeat { label, line });
            }
            marker => {
                if matches!(marker, Marker::Malformed) {
                    issues.push(issue(line, IssueKind::NotALabel(trimmed.to_owned())));
                }
                if trimmed.is_empty() {
                    if let Some(slot) = current {
                        sections[slot].close_slide();
                    }
                    continue;
                }
                let slot = match current {
                    Some(slot) => slot,
                    None => {
                        start_section(&mut sections, &mut items, &mut current, None, line);
                        sections.len() - 1
                    }
                };
                sections[slot].current.push(raw.trim_end().to_owned());
            }
        }
    }
    if let Some(slot) = current {
        sections[slot].close_slide();
    }

    if sections.is_empty() && items.is_empty() {
        issues.push(issue(body_start, IssueKind::NoWords(None)));
    }
    for building in &sections {
        if building.slides.is_empty() {
            issues.push(issue(
                building.line,
                IssueKind::NoWords(building.label.clone()),
            ));
        }
    }
    let keys: Vec<Option<String>> = sections
        .iter()
        .map(|building| building.label.as_deref().map(match_key))
        .collect();
    arrangement.sequence = items
        .into_iter()
        .map(|item| match item {
            Item::Section(slot) => SequenceEntry::Section(slot),
            Item::Repeat { label, line } => {
                let wanted = match_key(&label);
                let matches: Vec<usize> = keys
                    .iter()
                    .enumerate()
                    .filter(|(_, key)| key.as_deref() == Some(wanted.as_str()))
                    .map(|(slot, _)| slot)
                    .collect();
                match matches.as_slice() {
                    [only] => SequenceEntry::Section(*only),
                    [] => {
                        issues.push(issue(line, IssueKind::UnresolvedRepeat(label.clone())));
                        SequenceEntry::Unresolved {
                            label,
                            line,
                            candidates: 0,
                        }
                    }
                    several => {
                        issues.push(issue(
                            line,
                            IssueKind::AmbiguousRepeat {
                                label: label.clone(),
                                sections: several.len(),
                            },
                        ));
                        SequenceEntry::Unresolved {
                            label,
                            line,
                            candidates: several.len(),
                        }
                    }
                }
            }
        })
        .collect();
    arrangement.sections = sections
        .into_iter()
        .map(|building| ImportSection {
            label: building.label,
            lyrics: building
                .slides
                .iter()
                .map(|slide| slide.join("\n"))
                .collect::<Vec<_>>()
                .join("\n\n"),
        })
        .collect();
}

/// Ends the open section and opens a new one that also takes a Sequence
/// position.
fn start_section(
    sections: &mut Vec<Building>,
    items: &mut Vec<Item>,
    current: &mut Option<usize>,
    label: Option<String>,
    line: usize,
) {
    if let Some(slot) = *current {
        sections[slot].close_slide();
    }
    sections.push(Building::new(label, line));
    *current = Some(sections.len() - 1);
    items.push(Item::Section(sections.len() - 1));
}
