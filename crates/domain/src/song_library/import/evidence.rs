//! Evidence an import review shows: why a Song or an Arrangement was
//! classified as it was (Decisions 4-6).
//!
//! Evidence is data, never prose: the review surface composes the sentences.
//! Nothing here decides anything; the operator does.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::song_library::{MatchedOn, Section, match_key};

use super::{ImportArrangement, ImportSection, SequenceEntry};

/// One reason a Song work was, or may be, the document's Song.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SongEvidence {
    /// The document names a local Song's identity: the same Song work.
    SameId,
    /// A contained Arrangement identity is already one of this Song's.
    SameArrangementId,
    /// A document title or alias and a Song title or alias share a key.
    Name {
        /// Which document name matched.
        candidate_on: MatchedOn,
        /// Which Song name matched.
        existing_on: MatchedOn,
        /// Whether the names are equal after case and spacing folding.
        exact: bool,
    },
    /// The document and the Song credit the same original attribution.
    /// Supporting evidence only: never a candidate on its own.
    SameAttribution,
    /// The document and the Song open on the same first line of the first
    /// sung section. Supporting evidence only.
    SameOpeningLine,
    /// The file names the Song something other than the Song's own title or
    /// aliases ("The file names it {title}").
    FileTitle(String),
    /// The file also calls the Song this, which the Song does not (Decision
    /// 12: shown, never written).
    AlsoCalled(String),
}

/// One reason an imported Arrangement was classified as it was.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrangementEvidence {
    /// The Arrangement identity is already one of the Song's.
    SameId,
    /// Lyrics, Sequence, slide breaks and details all match.
    Identical,
    /// A local Arrangement resembles it; these are the differences.
    Resembles(Vec<ArrangementDifference>),
    /// The Song has no Arrangement for this Choir.
    NoArrangementForChoir(String),
    /// The Song has no Arrangement in this language set (the languages
    /// joined with " + ").
    NewLanguage(String),
    /// Nothing the Song has resembles it.
    NoMatchingArrangement,
    /// Creates the Song work and this many Arrangements with it.
    CreatesSong {
        /// How many Arrangements the document brings.
        arrangements: usize,
    },
    /// The Song is added by an earlier document of the same batch.
    AddsToStagedSong,
    /// This Choir name matches no Choir, so a Choir is added.
    AddsChoir(String),
    /// This Choir name matches only an archived Choir, so it is not linked.
    ArchivedChoir(String),
    /// The Arrangement it resembles, or replaces, is archived.
    ExistingArchived,
    /// A doubt the extracting tool recorded; shown, never stored.
    FileWarning(String),
}

/// An Arrangement detail an import can differ in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetailField {
    /// The Arrangement title.
    Title,
    /// The language list.
    Languages,
    /// The Choirs.
    Choirs,
    /// The translator or adapter credit.
    RenditionAttribution,
    /// The occasion tags.
    Occasions,
    /// The notes.
    Notes,
}

/// One way an imported Arrangement differs from the local one it resembles.
///
/// Sections are paired by label, then by position among the unlabeled; an
/// unlabeled section is named "Section {n}" by its 1-based position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrangementDifference {
    /// These details differ.
    Details(Vec<DetailField>),
    /// The section's words differ.
    SectionReworded(String),
    /// The words match but the slide breaks differ.
    SlideBreaks(String),
    /// The file has a section the local Arrangement lacks.
    SectionAdded(String),
    /// The local Arrangement has a section the file lacks.
    SectionRemoved(String),
    /// The Sequence differs.
    SequenceChanged,
    /// Only the order of the sections differs.
    SectionsReordered,
}

/// The key a section is paired by: its folded label, `None` when unlabeled.
fn pair_key(label: Option<&str>) -> Option<String> {
    label.map(match_key)
}

/// Pairs each imported section with a local section: the k-th imported
/// section of a label with the k-th local one, and the k-th unlabeled with
/// the k-th unlabeled. Each local section is paired at most once.
pub(super) fn pair_sections(imported: &[ImportSection], local: &[Section]) -> Vec<Option<usize>> {
    let mut pool: BTreeMap<Option<String>, VecDeque<usize>> = BTreeMap::new();
    for (index, section) in local.iter().enumerate() {
        pool.entry(pair_key(section.label()))
            .or_default()
            .push_back(index);
    }
    imported
        .iter()
        .map(|section| {
            pool.get_mut(&pair_key(section.label.as_deref()))
                .and_then(VecDeque::pop_front)
        })
        .collect()
}

fn folded_set<'a>(values: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    values.into_iter().map(match_key).collect()
}

fn section_name(label: Option<&str>, index: usize) -> String {
    label.map_or_else(|| format!("Section {}", index + 1), str::to_owned)
}

/// The local Sequence as section positions.
fn local_positions(local: &LocalView<'_>) -> Vec<Option<usize>> {
    local
        .sequence
        .iter()
        .map(|id| {
            local
                .sections
                .iter()
                .position(|section| section.id() == *id)
        })
        .collect()
}

/// The parts of a local Arrangement a comparison reads.
pub(super) struct LocalView<'a> {
    pub(super) title: Option<&'a str>,
    pub(super) languages: &'a [String],
    pub(super) choir_names: Vec<String>,
    pub(super) rendition_attribution: Option<&'a str>,
    pub(super) occasions: Vec<&'a str>,
    pub(super) notes: Option<&'a str>,
    pub(super) sections: &'a [Section],
    pub(super) sequence: Vec<crate::identity::SectionId>,
}

fn imported_positions(imported: &ImportArrangement, pairs: &[Option<usize>]) -> Vec<Option<usize>> {
    imported
        .sequence
        .iter()
        .map(|entry| match entry {
            SequenceEntry::Section(index) => pairs.get(*index).copied().flatten(),
            SequenceEntry::Unresolved { .. } => None,
        })
        .collect()
}

/// Which details differ.
fn detail_differences(
    imported: &ImportArrangement,
    imported_choirs: &[String],
    local: &LocalView<'_>,
) -> Vec<DetailField> {
    let mut fields = Vec::new();
    if imported.title.as_deref() != local.title {
        fields.push(DetailField::Title);
    }
    if imported.languages != local.languages {
        fields.push(DetailField::Languages);
    }
    if folded_set(imported_choirs.iter().map(String::as_str))
        != folded_set(local.choir_names.iter().map(String::as_str))
    {
        fields.push(DetailField::Choirs);
    }
    if imported.rendition_attribution.as_deref() != local.rendition_attribution {
        fields.push(DetailField::RenditionAttribution);
    }
    if imported
        .occasions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != local.occasions.iter().copied().collect::<BTreeSet<_>>()
    {
        fields.push(DetailField::Occasions);
    }
    if imported.notes.as_deref() != local.notes {
        fields.push(DetailField::Notes);
    }
    fields
}

/// Decision 5's equal content: details, sections (labels, lyrics and slide
/// breaks, in order) and the Sequence as section positions. Pagination states
/// are not part of it.
pub(super) fn equal_content(
    imported: &ImportArrangement,
    imported_choirs: &[String],
    local: &LocalView<'_>,
) -> bool {
    if !detail_differences(imported, imported_choirs, local).is_empty()
        || imported.sections.len() != local.sections.len()
    {
        return false;
    }
    let sections_equal = imported
        .sections
        .iter()
        .zip(local.sections)
        .all(|(section, stored)| {
            section.label.as_deref() == stored.label() && section.lyrics == stored.lyrics()
        });
    if !sections_equal {
        return false;
    }
    let imported_sequence: Vec<Option<usize>> = imported
        .sequence
        .iter()
        .map(|entry| match entry {
            SequenceEntry::Section(index) => Some(*index),
            SequenceEntry::Unresolved { .. } => None,
        })
        .collect();
    imported_sequence == local_positions(local)
}

/// Every difference between an imported Arrangement and a local one; empty
/// exactly when [`equal_content`] holds.
pub(super) fn differences(
    imported: &ImportArrangement,
    imported_choirs: &[String],
    local: &LocalView<'_>,
) -> Vec<ArrangementDifference> {
    let mut found = Vec::new();
    let details = detail_differences(imported, imported_choirs, local);
    if !details.is_empty() {
        found.push(ArrangementDifference::Details(details));
    }
    let pairs = pair_sections(&imported.sections, local.sections);
    for (index, section) in imported.sections.iter().enumerate() {
        let name = section_name(section.label.as_deref(), index);
        match pairs[index] {
            Some(other) => {
                let stored = &local.sections[other];
                if section.lyrics != stored.lyrics() {
                    let words = |text: &str| {
                        text.split_whitespace()
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    };
                    found.push(if words(&section.lyrics) == words(stored.lyrics()) {
                        ArrangementDifference::SlideBreaks(name)
                    } else {
                        ArrangementDifference::SectionReworded(name)
                    });
                }
            }
            None => found.push(ArrangementDifference::SectionAdded(name)),
        }
    }
    let paired: BTreeSet<usize> = pairs.iter().flatten().copied().collect();
    for (index, section) in local.sections.iter().enumerate() {
        if !paired.contains(&index) {
            found.push(ArrangementDifference::SectionRemoved(section_name(
                section.label(),
                index,
            )));
        }
    }
    if imported_positions(imported, &pairs) != local_positions(local) {
        found.push(ArrangementDifference::SequenceChanged);
    }
    if found.is_empty() && !equal_content(imported, imported_choirs, local) {
        found.push(ArrangementDifference::SectionsReordered);
    }
    found
}
