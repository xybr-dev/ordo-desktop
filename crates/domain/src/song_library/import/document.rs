//! The typed document both import routes produce.

use crate::identity::{ArrangementId, SongId};
use crate::song_library::{SongDraft, languages, search_key};

/// One Song work with its Arrangements, as an import carries it.
///
/// A machine document brings its own identities; a Markdown draft takes the
/// ones the caller supplies. Nothing here has been checked against the
/// Library: that is classification's job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDocument {
    /// The stable identity of the Song work.
    pub id: SongId,
    /// The Song work's details, exactly as authored.
    pub song: SongDraft,
    /// The contained Arrangements in authored order.
    pub arrangements: Vec<ImportArrangement>,
    /// Document-level notes from the extracting tool; shown, never stored.
    pub warnings: Vec<String>,
}

/// One Arrangement inside an [`ImportDocument`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportArrangement {
    /// The stable identity of the Arrangement.
    pub id: ArrangementId,
    /// The Arrangement's title, when it has one.
    pub title: Option<String>,
    /// Languages as display names of built-in languages (Decision 11).
    pub languages: Vec<String>,
    /// Choir names, matched to Choirs during classification (Decision 10).
    pub choirs: Vec<String>,
    /// The translator or adapter credit, joined by [`rendition_attribution`].
    pub rendition_attribution: Option<String>,
    /// Occasion tags of the Arrangement.
    pub occasions: Vec<String>,
    /// Notes about the Arrangement.
    pub notes: Option<String>,
    /// The reusable sections in authored order.
    pub sections: Vec<ImportSection>,
    /// The performance order.
    pub sequence: Vec<SequenceEntry>,
    /// Doubts from the extracting tool (`extraction_warnings`); never stored.
    pub warnings: Vec<String>,
}

/// One reusable section of an imported Arrangement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportSection {
    /// The section's name; `None` for an unlabeled section.
    pub label: Option<String>,
    /// The words: slides joined by one blank line, lines by a line feed.
    pub lyrics: String,
}

/// One position of an imported Sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SequenceEntry {
    /// The section at this index of [`ImportArrangement::sections`].
    Section(usize),
    /// A Markdown `[Repeat: label]` that names no single section; never
    /// produced for a machine document.
    Unresolved {
        /// The label the author wrote, trimmed.
        label: String,
        /// The 1-based line of the repeat in the file.
        line: usize,
        /// How many sections carry that label: 0 (unresolved) or 2 and more
        /// (ambiguous).
        candidates: usize,
    },
}

/// The built-in language an import value names, or `None`.
///
/// Matches when the folded text *equals* a display name or one of its other
/// names (never a substring, unlike the picker), and returns the display name
/// to store.
pub fn resolve_language(text: &str) -> Option<&'static str> {
    let key = search_key(text);
    if key.is_empty() {
        return None;
    }
    languages().iter().find_map(|language| {
        let named = search_key(language.name()) == key
            || language
                .other_names()
                .iter()
                .any(|other| search_key(other) == key);
        named.then(|| language.name())
    })
}

/// The rendition attribution an import's translator and adapter credits make.
///
/// One credit stands alone; both join as "{translator}; {adapter}". Blank
/// credits count as absent, and each credit is trimmed.
pub fn rendition_attribution(translator: Option<&str>, adapter: Option<&str>) -> Option<String> {
    let present = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    match (present(translator), present(adapter)) {
        (Some(translator), Some(adapter)) => Some(format!("{translator}; {adapter}")),
        (one, other) => one.or(other),
    }
}
