//! Identity resolution and classification of staged import documents
//! (Decisions 4-6, 10 and 12).
//!
//! Classification reads the Library and the batch and nothing else. It is
//! evidence for the operator, never a decision: no title or alias match ever
//! overwrites anything, and every suggested action can be changed.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{ArrangementId, ChoirId, SongId};
use crate::song_library::{
    Arrangement, IdentityEvidence, MatchStrength, PossibleSong, SongLibrary, SongWork, match_key,
};

use super::evidence::{LocalView, differences, equal_content};
use super::{
    ArrangementDifference, ArrangementEvidence, ImportArrangement, ImportDocument, Outcome,
    SequenceEntry, SongEvidence, validate_document,
};

/// One finding that makes a document unimportable, as data.
///
/// The decoder's findings arrive in this shape; the review shows operator
/// copy mapped from `code` and keeps `message` behind its technical details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidReason {
    /// A stable machine code, such as `schema` or `duplicate-arrangement-id`.
    pub code: String,
    /// The finding in words, for technical details.
    pub message: String,
}

/// One document of a batch, as staged from a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedDocument {
    /// The file name the document came from.
    pub source: String,
    /// What staging made of it.
    pub content: Staged,
}

/// A staged file's content.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)] // one value per staged file; boxing would only burden callers
pub enum Staged {
    /// A decoded document; classification still checks whole-document
    /// validity.
    Valid(ImportDocument),
    /// The file could not be decoded or inspected.
    Invalid(Vec<InvalidReason>),
}

/// Which Song a document belongs to, by Decision 4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SongIdentity {
    /// A local Song (suggested, changeable) with the evidence for it.
    Existing {
        /// The Song.
        song: SongId,
        /// Why.
        evidence: Vec<SongEvidence>,
    },
    /// No candidate: add a new Song under the document's identity.
    New,
    /// The Song is added by this earlier document of the batch (index into
    /// the batch).
    StagedBy {
        /// The earlier document.
        document: usize,
    },
    /// The operator must choose.
    NeedsDecision {
        /// The candidate Songs with their evidence, strongest first.
        candidates: Vec<(SongId, Vec<SongEvidence>)>,
        /// The only candidate, when it is archived: it cannot take an
        /// Arrangement, so only a new Song or skipping is offered.
        archived: Option<SongId>,
    },
}

/// How an imported Arrangement relates to the Library.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    /// The Song is new; it is created with the Arrangement.
    AddNewSong,
    /// The Arrangement is new to an existing Song.
    AttachArrangement,
    /// The Library already has this content.
    SkipUnchanged,
    /// A resembling or same-identity Arrangement differs; the operator
    /// chooses.
    NeedsDecision,
}

/// One imported Arrangement's classification and the actions it offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrangementReview {
    /// The classification.
    pub classification: Classification,
    /// The local Arrangement it matched or resembles.
    pub nearest: Option<ArrangementId>,
    /// Whether the imported identity is the Song's own Arrangement.
    pub same_id: bool,
    /// The suggested outcome; a suggestion is never a choice.
    pub recommended: Option<Outcome>,
    /// Every outcome the operator may choose.
    pub offered: Vec<Outcome>,
    /// Why, in order: reasons first, then Choir facts, then file warnings.
    pub evidence: Vec<ArrangementEvidence>,
}

/// One valid document's review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentReview {
    /// Which Song it belongs to.
    pub identity: SongIdentity,
    /// One review per contained Arrangement, in document order. While the
    /// identity needs a decision these are placeholders: re-run
    /// [`classify_arrangements`] once the operator chooses the Song.
    pub arrangements: Vec<ArrangementReview>,
}

/// One document's review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Review {
    /// Excluded whole (Decision 137).
    Invalid(Vec<InvalidReason>),
    /// A valid document.
    Valid(DocumentReview),
}

/// The Song an Arrangement is classified against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedSong {
    /// A Library Song.
    Existing(SongId),
    /// A Song this document adds.
    New,
    /// A Song an earlier document of the batch adds.
    Staged,
}

/// How a Choir name resolves (Decision 10).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChoirLookup {
    /// An active Choir with the same name or short label.
    Active(ChoirId),
    /// Only an archived Choir matches: not linked, not recreated.
    Archived(ChoirId),
    /// No Choir matches.
    Missing,
}

/// Resolves one imported Choir name against the Library.
pub(super) fn lookup_choir(library: &SongLibrary, name: &str) -> ChoirLookup {
    let key = match_key(name);
    let named = |archived: bool| {
        library
            .choirs()
            .filter(|choir| choir.is_archived() == archived)
            .find(|choir| {
                match_key(choir.name()) == key
                    || choir
                        .short_label()
                        .is_some_and(|label| match_key(label) == key)
            })
            .map(|choir| choir.id())
    };
    named(false).map_or_else(
        || named(true).map_or(ChoirLookup::Missing, ChoirLookup::Archived),
        ChoirLookup::Active,
    )
}

fn problem_reason(problem: &super::DocumentProblem) -> InvalidReason {
    InvalidReason {
        code: "invalid-document".to_owned(),
        message: format!("{} · {problem}", problem.pointer()),
    }
}

/// Classifies every document in file order against the library and the Songs
/// earlier documents add (Decision 6).
///
/// A document is `Invalid` when staging failed, when whole-document
/// validation finds a problem, or when it repeats an Arrangement identity an
/// earlier valid document already uses. A later document with the Song
/// identity of an earlier one that adds (or still has to decide) that Song is
/// `StagedBy` it: the earlier document's Song details win.
pub fn classify_batch(library: &SongLibrary, documents: &[StagedDocument]) -> Vec<Review> {
    let mut reviews = Vec::with_capacity(documents.len());
    let mut arrangement_owner: BTreeMap<ArrangementId, usize> = BTreeMap::new();
    let mut song_creator: BTreeMap<SongId, usize> = BTreeMap::new();
    for (index, staged) in documents.iter().enumerate() {
        let document = match &staged.content {
            Staged::Invalid(reasons) => {
                reviews.push(Review::Invalid(reasons.clone()));
                continue;
            }
            Staged::Valid(document) => document,
        };
        let mut reasons: Vec<InvalidReason> = validate_document(document)
            .iter()
            .map(problem_reason)
            .collect();
        for arrangement in &document.arrangements {
            if let Some(&other) = arrangement_owner.get(&arrangement.id) {
                reasons.push(InvalidReason {
                    code: "duplicate-arrangement-id".to_owned(),
                    message: format!(
                        "Arrangement {} is also in {}.",
                        arrangement.id, documents[other].source
                    ),
                });
            }
        }
        if !reasons.is_empty() {
            reviews.push(Review::Invalid(reasons));
            continue;
        }
        for arrangement in &document.arrangements {
            arrangement_owner.insert(arrangement.id, index);
        }
        let local = library.song(document.id).is_ok();
        let identity = match song_creator.get(&document.id) {
            Some(&creator) if !local => SongIdentity::StagedBy { document: creator },
            _ => resolve_song(library, document),
        };
        if !local
            && matches!(
                identity,
                SongIdentity::New | SongIdentity::NeedsDecision { .. }
            )
        {
            song_creator.entry(document.id).or_insert(index);
        }
        let arrangements = match &identity {
            SongIdentity::Existing { song, .. } => {
                classify_arrangements(library, document, ResolvedSong::Existing(*song))
            }
            SongIdentity::New => classify_arrangements(library, document, ResolvedSong::New),
            SongIdentity::StagedBy { .. } => {
                classify_arrangements(library, document, ResolvedSong::Staged)
            }
            SongIdentity::NeedsDecision { .. } => provisional(document),
        };
        reviews.push(Review::Valid(DocumentReview {
            identity,
            arrangements,
        }));
    }
    reviews
}

/// Placeholders until the operator resolves the document's Song.
fn provisional(document: &ImportDocument) -> Vec<ArrangementReview> {
    document
        .arrangements
        .iter()
        .map(|_| ArrangementReview {
            classification: Classification::NeedsDecision,
            nearest: None,
            same_id: false,
            recommended: None,
            offered: Vec::new(),
            evidence: Vec::new(),
        })
        .collect()
}

/// Decision 4's resolution order for one document.
fn resolve_song(library: &SongLibrary, document: &ImportDocument) -> SongIdentity {
    let possible = library.possible_existing_songs(&document.song);
    let settle = |song: SongId, lead: SongEvidence| {
        if library.song(song).is_ok_and(SongWork::is_archived) {
            SongIdentity::NeedsDecision {
                candidates: Vec::new(),
                archived: Some(song),
            }
        } else {
            SongIdentity::Existing {
                song,
                evidence: song_evidence(library, document, song, Some(lead), &possible),
            }
        }
    };
    // 1. The document's own identity is a local Song.
    if library.song(document.id).is_ok() {
        return settle(document.id, SongEvidence::SameId);
    }
    // 2. A contained Arrangement identity is a local Arrangement.
    let owners: BTreeSet<SongId> = document
        .arrangements
        .iter()
        .filter_map(|arrangement| library.arrangement(arrangement.id).ok())
        .map(Arrangement::song)
        .collect();
    match owners.len() {
        0 => {}
        1 => {
            if let Some(&song) = owners.iter().next() {
                return settle(song, SongEvidence::SameArrangementId);
            }
        }
        _ => {
            return SongIdentity::NeedsDecision {
                candidates: owners
                    .iter()
                    .map(|&song| {
                        (
                            song,
                            song_evidence(
                                library,
                                document,
                                song,
                                Some(SongEvidence::SameArrangementId),
                                &possible,
                            ),
                        )
                    })
                    .collect(),
                archived: None,
            };
        }
    }
    // 3-5. Names.
    let exact: Vec<&PossibleSong> = possible
        .iter()
        .filter(|entry| {
            matches!(
                entry.strength,
                MatchStrength::ExactTitle | MatchStrength::ExactAlias
            )
        })
        .collect();
    if let [only] = exact.as_slice() {
        if library.song(only.song).is_ok_and(SongWork::is_archived) {
            return SongIdentity::NeedsDecision {
                candidates: Vec::new(),
                archived: Some(only.song),
            };
        }
        return SongIdentity::Existing {
            song: only.song,
            evidence: song_evidence(library, document, only.song, None, &possible),
        };
    }
    if possible.is_empty() {
        return SongIdentity::New;
    }
    SongIdentity::NeedsDecision {
        candidates: possible
            .iter()
            .map(|entry| {
                (
                    entry.song,
                    song_evidence(library, document, entry.song, None, &possible),
                )
            })
            .collect(),
        archived: None,
    }
}

/// The evidence for one candidate Song: the lead reason, the name and
/// attribution evidence, then the supporting and difference evidence.
fn song_evidence(
    library: &SongLibrary,
    document: &ImportDocument,
    song: SongId,
    lead: Option<SongEvidence>,
    possible: &[PossibleSong],
) -> Vec<SongEvidence> {
    let mut evidence: Vec<SongEvidence> = lead.into_iter().collect();
    let Ok(stored) = library.song(song) else {
        return evidence;
    };
    if let Some(entry) = possible.iter().find(|entry| entry.song == song) {
        evidence.extend(entry.evidence.iter().map(|item| match item {
            IdentityEvidence::Name {
                candidate_on,
                existing_on,
                exact,
            } => SongEvidence::Name {
                candidate_on: *candidate_on,
                existing_on: *existing_on,
                exact: *exact,
            },
            IdentityEvidence::SameAttribution => SongEvidence::SameAttribution,
        }));
    }
    let opening: BTreeSet<String> = document
        .arrangements
        .iter()
        .filter_map(imported_opening)
        .collect();
    let shares_opening = library
        .arrangements_for_song(song)
        .unwrap_or_default()
        .into_iter()
        .filter_map(local_opening)
        .any(|line| opening.contains(&line));
    if shares_opening {
        evidence.push(SongEvidence::SameOpeningLine);
    }
    let known: BTreeSet<String> = std::iter::once(stored.title())
        .chain(stored.aliases().iter().map(String::as_str))
        .map(match_key)
        .collect();
    if !known.contains(&match_key(&document.song.title)) {
        evidence.push(SongEvidence::FileTitle(document.song.title.clone()));
    }
    for alias in &document.song.aliases {
        if !known.contains(&match_key(alias)) {
            evidence.push(SongEvidence::AlsoCalled(alias.clone()));
        }
    }
    evidence
}

/// The folded first non-blank line of a text.
fn first_line(lyrics: &str) -> Option<String> {
    lyrics.lines().map(match_key).find(|line| !line.is_empty())
}

/// The first line of the first sung section of an imported Arrangement.
fn imported_opening(arrangement: &ImportArrangement) -> Option<String> {
    let first = arrangement
        .sequence
        .iter()
        .find_map(|entry| match entry {
            SequenceEntry::Section(index) => Some(*index),
            SequenceEntry::Unresolved { .. } => None,
        })
        .or(if arrangement.sections.is_empty() {
            None
        } else {
            Some(0)
        })?;
    first_line(&arrangement.sections.get(first)?.lyrics)
}

/// The first line of the first sung section of a local Arrangement.
fn local_opening(arrangement: &Arrangement) -> Option<String> {
    let section = arrangement
        .sequence()
        .first()
        .and_then(|occurrence| {
            arrangement
                .sections()
                .iter()
                .find(|section| section.id() == occurrence.section())
        })
        .or_else(|| arrangement.sections().first())?;
    first_line(section.lyrics())
}

/// The Choir names of an imported Arrangement as the Library knows them:
/// a matched Choir by its own name, an unmatched name as written.
fn normalised_choirs(library: &SongLibrary, arrangement: &ImportArrangement) -> Vec<String> {
    arrangement
        .choirs
        .iter()
        .filter_map(|name| match lookup_choir(library, name) {
            ChoirLookup::Active(id) => Some(
                library
                    .choir(id)
                    .map_or_else(|_| name.clone(), |choir| choir.name().to_owned()),
            ),
            // The plan drops archived Choirs, so they never differ.
            ChoirLookup::Archived(_) => None,
            ChoirLookup::Missing => Some(name.clone()),
        })
        .collect()
}

fn local_view<'a>(library: &'a SongLibrary, local: &'a Arrangement) -> LocalView<'a> {
    LocalView {
        title: local.title(),
        languages: local.languages(),
        choir_names: local
            .choirs()
            .iter()
            .filter_map(|id| library.choir(*id).ok())
            .map(|choir| choir.name().to_owned())
            .collect(),
        rendition_attribution: local.rendition_attribution(),
        occasions: local.occasions().iter().map(String::as_str).collect(),
        notes: local.notes(),
        sections: local.sections(),
        sequence: local.sequence().iter().map(|o| o.section()).collect(),
    }
}

/// Decision 5's equal content: same title, language list, Choir names
/// (case-insensitive set), rendition attribution, occasions (set), notes,
/// section labels and lyrics in order (slide breaks included) and the same
/// Sequence as section positions. Pagination states are ignored.
pub fn same_content(
    imported: &ImportArrangement,
    local: &Arrangement,
    choir_name: &dyn Fn(ChoirId) -> String,
) -> bool {
    let choirs = &imported.choirs;
    let view = LocalView {
        title: local.title(),
        languages: local.languages(),
        choir_names: local.choirs().iter().map(|id| choir_name(*id)).collect(),
        rendition_attribution: local.rendition_attribution(),
        occasions: local.occasions().iter().map(String::as_str).collect(),
        notes: local.notes(),
        sections: local.sections(),
        sequence: local.sequence().iter().map(|o| o.section()).collect(),
    };
    equal_content(imported, choirs, &view)
}

/// Re-runs one document's Arrangements after the operator resolved its Song.
///
/// A Song unknown to the Library is treated as new.
pub fn classify_arrangements(
    library: &SongLibrary,
    document: &ImportDocument,
    song: ResolvedSong,
) -> Vec<ArrangementReview> {
    let song = match song {
        ResolvedSong::Existing(id) if library.song(id).is_err() => ResolvedSong::New,
        other => other,
    };
    document
        .arrangements
        .iter()
        .map(|arrangement| {
            let mut review = match song {
                ResolvedSong::New => adds_song(document.arrangements.len()),
                ResolvedSong::Staged => attaches(ArrangementEvidence::AddsToStagedSong),
                ResolvedSong::Existing(id) => classify_against(library, id, arrangement),
            };
            if review.classification != Classification::SkipUnchanged {
                choir_evidence(library, arrangement, &mut review.evidence);
            }
            review.evidence.extend(
                arrangement
                    .warnings
                    .iter()
                    .map(|warning| ArrangementEvidence::FileWarning(warning.clone())),
            );
            review
        })
        .collect()
}

fn adds_song(arrangements: usize) -> ArrangementReview {
    ArrangementReview {
        classification: Classification::AddNewSong,
        nearest: None,
        same_id: false,
        recommended: Some(Outcome::AddNewSong),
        offered: vec![Outcome::AddNewSong, Outcome::Skip],
        evidence: vec![ArrangementEvidence::CreatesSong { arrangements }],
    }
}

fn attaches(reason: ArrangementEvidence) -> ArrangementReview {
    ArrangementReview {
        classification: Classification::AttachArrangement,
        nearest: None,
        same_id: false,
        recommended: Some(Outcome::Attach),
        offered: vec![Outcome::Attach, Outcome::Skip],
        evidence: vec![reason],
    }
}

fn choir_evidence(
    library: &SongLibrary,
    arrangement: &ImportArrangement,
    evidence: &mut Vec<ArrangementEvidence>,
) {
    for name in &arrangement.choirs {
        match lookup_choir(library, name) {
            ChoirLookup::Active(_) => {}
            ChoirLookup::Archived(_) => {
                evidence.push(ArrangementEvidence::ArchivedChoir(name.clone()));
            }
            ChoirLookup::Missing => evidence.push(ArrangementEvidence::AddsChoir(name.clone())),
        }
    }
}

/// The Choirs an imported Arrangement names that the Library has, active or
/// archived.
fn known_choirs(library: &SongLibrary, arrangement: &ImportArrangement) -> BTreeSet<ChoirId> {
    arrangement
        .choirs
        .iter()
        .filter_map(|name| match lookup_choir(library, name) {
            ChoirLookup::Active(id) | ChoirLookup::Archived(id) => Some(id),
            ChoirLookup::Missing => None,
        })
        .collect()
}

fn language_set(languages: &[String]) -> BTreeSet<String> {
    languages
        .iter()
        .map(|language| match_key(language))
        .collect()
}

/// Classifies one imported Arrangement against an existing Song (Decision 5).
fn classify_against(
    library: &SongLibrary,
    song: SongId,
    imported: &ImportArrangement,
) -> ArrangementReview {
    let local = library.arrangements_for_song(song).unwrap_or_default();
    let choirs = normalised_choirs(library, imported);
    let views: Vec<LocalView<'_>> = local.iter().map(|a| local_view(library, a)).collect();
    let equal = |index: usize| equal_content(imported, &choirs, &views[index]);

    let same_id = local.iter().position(|a| a.id() == imported.id);
    let nearest = same_id
        .or_else(|| (0..local.len()).find(|&index| equal(index)))
        .or_else(|| nearest_resembling(library, imported, &local));

    let Some(index) = nearest else {
        return attach_review(library, imported, &local);
    };
    let found = local[index];
    let archived = found.is_archived();
    let mut evidence = Vec::new();
    if same_id.is_some() {
        evidence.push(ArrangementEvidence::SameId);
    }
    let (classification, recommended, offered) = if equal(index) {
        evidence.push(ArrangementEvidence::Identical);
        (
            Classification::SkipUnchanged,
            Some(Outcome::Skip),
            vec![Outcome::Skip, Outcome::AttachAsAnother],
        )
    } else {
        let found_differences: Vec<ArrangementDifference> =
            differences(imported, &choirs, &views[index]);
        evidence.push(ArrangementEvidence::Resembles(found_differences));
        match (same_id.is_some(), archived) {
            (_, true) => (
                Classification::NeedsDecision,
                Some(Outcome::AddAsNew),
                vec![Outcome::AddAsNew, Outcome::Skip],
            ),
            (true, false) => (
                Classification::NeedsDecision,
                Some(Outcome::UpdateExisting),
                vec![Outcome::UpdateExisting, Outcome::AddAsNew, Outcome::Skip],
            ),
            (false, false) => (
                Classification::NeedsDecision,
                Some(Outcome::AddAsNew),
                vec![Outcome::AddAsNew, Outcome::UpdateExisting, Outcome::Skip],
            ),
        }
    };
    if archived {
        evidence.push(ArrangementEvidence::ExistingArchived);
    }
    ArrangementReview {
        classification,
        nearest: Some(found.id()),
        same_id: same_id.is_some(),
        recommended,
        offered,
        evidence,
    }
}

/// The nearest resembling local Arrangement, by Decision 5's order: same
/// language set and a shared Choir, then same language set and opening line,
/// then same title.
fn nearest_resembling(
    library: &SongLibrary,
    imported: &ImportArrangement,
    local: &[&Arrangement],
) -> Option<usize> {
    let languages = language_set(&imported.languages);
    let same_languages = |a: &Arrangement| language_set(a.languages()) == languages;
    let choirs = known_choirs(library, imported);
    let opening = imported_opening(imported);
    local
        .iter()
        .position(|a| same_languages(a) && a.choirs().iter().any(|id| choirs.contains(id)))
        .or_else(|| {
            let opening = opening.as_ref()?;
            local
                .iter()
                .position(|a| same_languages(a) && local_opening(a).as_ref() == Some(opening))
        })
        .or_else(|| {
            let title = imported.title.as_deref().map(match_key)?;
            local
                .iter()
                .position(|a| a.title().map(match_key).as_ref() == Some(&title))
        })
}

/// A new Arrangement for an existing Song, with the reason it is new.
fn attach_review(
    library: &SongLibrary,
    imported: &ImportArrangement,
    local: &[&Arrangement],
) -> ArrangementReview {
    let mut review = attaches(ArrangementEvidence::NoMatchingArrangement);
    review.evidence.clear();
    let sung: BTreeSet<ChoirId> = local
        .iter()
        .flat_map(|a| a.choirs().iter().copied())
        .collect();
    for name in &imported.choirs {
        let covered = match lookup_choir(library, name) {
            ChoirLookup::Active(id) | ChoirLookup::Archived(id) => sung.contains(&id),
            ChoirLookup::Missing => false,
        };
        if !covered {
            review
                .evidence
                .push(ArrangementEvidence::NoArrangementForChoir(name.clone()));
        }
    }
    let languages = language_set(&imported.languages);
    if local
        .iter()
        .all(|a| language_set(a.languages()) != languages)
    {
        review.evidence.push(ArrangementEvidence::NewLanguage(
            imported.languages.join(" + "),
        ));
    }
    if review.evidence.is_empty() {
        review
            .evidence
            .push(ArrangementEvidence::NoMatchingArrangement);
    }
    review
}
