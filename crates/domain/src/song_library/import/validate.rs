//! Whole-document validity, shared by the `.ordo` and Markdown routes.

use std::collections::BTreeSet;

use crate::identity::ArrangementId;
use crate::song_library::{Field, is_listed};

use super::{ImportArrangement, ImportDocument, SequenceEntry};

/// One reason an [`ImportDocument`] cannot be imported.
///
/// Arrangement positions are 0-based indexes into
/// [`ImportDocument::arrangements`]. Any problem excludes the whole
/// document (Decision 137): an import never takes part of a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentProblem {
    /// The Song title is blank.
    BlankTitle,
    /// An Arrangement names a language that is not one of Ordo's.
    UnlistedLanguage {
        /// The Arrangement's position.
        arrangement: usize,
        /// The language as written.
        value: String,
    },
    /// An Arrangement names no language at all.
    NoLanguage {
        /// The Arrangement's position.
        arrangement: usize,
    },
    /// A Sequence position names a section the Arrangement does not have.
    UnknownSection {
        /// The Arrangement's position.
        arrangement: usize,
        /// The Sequence position, 0-based.
        position: usize,
    },
    /// An optional text value is present but blank.
    BlankField {
        /// The Arrangement, or `None` for a Song field.
        arrangement: Option<usize>,
        /// Which value.
        field: Field,
    },
    /// A set-shaped value repeats, or an alias equals the title.
    Duplicate {
        /// The Arrangement, or `None` for a Song field.
        arrangement: Option<usize>,
        /// Which value.
        field: Field,
    },
    /// Two Arrangements of the document share one identity.
    DuplicateArrangement(ArrangementId),
}

/// Every rule beyond the JSON shape; an empty result means the document is
/// valid. Problems are in document order.
pub fn validate_document(document: &ImportDocument) -> Vec<DocumentProblem> {
    let mut problems = Vec::new();
    let song = &document.song;
    if blank(&song.title) {
        problems.push(DocumentProblem::BlankTitle);
    }
    for (field, value) in [
        (Field::OriginalAttribution, &song.original_attribution),
        (Field::Provenance, &song.provenance),
        (Field::RightsNotes, &song.rights_notes),
    ] {
        if value.as_deref().is_some_and(blank) {
            problems.push(DocumentProblem::BlankField {
                arrangement: None,
                field,
            });
        }
    }
    list_problems(&mut problems, None, Field::Aliases, &song.aliases);
    if song.aliases.iter().any(|alias| alias == &song.title) {
        problems.push(DocumentProblem::Duplicate {
            arrangement: None,
            field: Field::Aliases,
        });
    }
    list_problems(
        &mut problems,
        None,
        Field::LiturgicalMoment,
        &song.liturgical_moments,
    );
    list_problems(&mut problems, None, Field::Occasion, &song.occasions);

    let mut seen = BTreeSet::new();
    for (index, arrangement) in document.arrangements.iter().enumerate() {
        if !seen.insert(arrangement.id) {
            problems.push(DocumentProblem::DuplicateArrangement(arrangement.id));
        }
        arrangement_problems(&mut problems, index, arrangement);
    }
    problems
}

fn arrangement_problems(
    problems: &mut Vec<DocumentProblem>,
    index: usize,
    arrangement: &ImportArrangement,
) {
    let at = Some(index);
    if arrangement.languages.is_empty() {
        problems.push(DocumentProblem::NoLanguage { arrangement: index });
    }
    for language in &arrangement.languages {
        if !is_listed(language) {
            problems.push(DocumentProblem::UnlistedLanguage {
                arrangement: index,
                value: language.clone(),
            });
        }
    }
    list_problems(problems, at, Field::Language, &arrangement.languages);
    for (field, value) in [
        (Field::ArrangementTitle, &arrangement.title),
        (
            Field::RenditionAttribution,
            &arrangement.rendition_attribution,
        ),
        (Field::ArrangementNotes, &arrangement.notes),
    ] {
        if value.as_deref().is_some_and(blank) {
            problems.push(DocumentProblem::BlankField {
                arrangement: at,
                field,
            });
        }
    }
    list_problems(problems, at, Field::Choir, &arrangement.choirs);
    list_problems(problems, at, Field::Occasion, &arrangement.occasions);
    for section in &arrangement.sections {
        if section.label.as_deref().is_some_and(blank) {
            problems.push(DocumentProblem::BlankField {
                arrangement: at,
                field: Field::SectionLabel,
            });
        }
    }
    for (position, entry) in arrangement.sequence.iter().enumerate() {
        if let SequenceEntry::Section(section) = entry
            && *section >= arrangement.sections.len()
        {
            problems.push(DocumentProblem::UnknownSection {
                arrangement: index,
                position,
            });
        }
    }
}

/// Blank and repeated entries of a set-shaped list.
fn list_problems(
    problems: &mut Vec<DocumentProblem>,
    arrangement: Option<usize>,
    field: Field,
    values: &[String],
) {
    if values.iter().any(|value| blank(value)) {
        problems.push(DocumentProblem::BlankField { arrangement, field });
    }
    if values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
    {
        problems.push(DocumentProblem::Duplicate { arrangement, field });
    }
}

fn blank(value: &str) -> bool {
    value.trim().is_empty()
}
