//! Duplicate and identity evidence for possible existing Songs (Decisions 38
//! and 39, T2.5).
//!
//! Two keys answer two different questions, and neither is ever stored:
//!
//! - [`match_key`] folds case and spacing only. It guards exact duplicates and
//!   stays strict about punctuation, digits and diacritics because each of
//!   those can distinguish two works.
//! - [`identity_key`] also folds diacritics and drops punctuation and spaces,
//!   except between two digits, where one space is kept. It is a looser key used
//!   only to rank likely duplicates as near matches. It keeps digits and letters
//!   as written, so numbered settings such as "Kyrie I", "Kyrie II" and
//!   "Kyrie 1", or "Psalm 1-2" and "Psalm 12", stay separate works
//!   (Decision 39).
//!
//! Both operations report only. They take `&SongLibrary`, change nothing and
//! never unite two Song works: the operator decides (Decision 39).

use crate::identity::SongId;

use super::interface::{SongLibrary, StatusFilter, match_key};
use super::model::SongDraft;
use super::search::search_key;

/// Which title of a Song matched.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatchedOn {
    /// The primary title.
    Title,
    /// The alias at this index of the aliases list.
    Alias(usize),
}

/// One shared match key between a candidate Song draft and a stored Song.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplicateEvidence {
    /// The stored Song that shares a key.
    pub song: SongId,
    /// Which candidate title matched.
    pub candidate_on: MatchedOn,
    /// Which stored title matched.
    pub existing_on: MatchedOn,
}

/// Case-, spacing-, diacritic- and punctuation-insensitive identity key for
/// near-duplicate evidence.
///
/// Applies [`search_key`] (case, spacing and diacritic fold) and then keeps only
/// alphanumeric characters, so punctuation and spaces are dropped. A dropped run
/// that sits between two digits leaves one space, so neighbouring numbers never
/// join: "Psalm 1-2" and "Psalm 1 2" share a key, "Psalm 12" does not, and
/// "Kaloy-i", "Kaloy i" and "Kaloyi" all share one. Digits and letters stay as
/// written: "Kyrie I" and "Kyrie II" differ, and so do "Kyrie 1" and "Kyrie I".
///
/// The key is never stored and never rewrites authored text. It is used only to
/// rank possible existing Songs; an empty result means the text carries no
/// identity at all (for example a blank or punctuation-only title).
pub fn identity_key(text: &str) -> String {
    let mut key = String::new();
    let mut dropped = false;
    for character in search_key(text).chars() {
        if !character.is_alphanumeric() {
            dropped = true;
            continue;
        }
        if dropped
            && character.is_numeric()
            && key.chars().next_back().is_some_and(char::is_numeric)
        {
            key.push(' ');
        }
        dropped = false;
        key.push(character);
    }
    key
}

/// How strongly one existing Song resembles a candidate draft.
///
/// Variants are declared strongest first, so the derived ordering puts the
/// strongest match smallest. A Song's strength is the strongest (smallest) of
/// its matching name pairs.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MatchStrength {
    /// The candidate's title and the Song's title share a [`match_key`].
    ExactTitle,
    /// Some exact pair involves an alias: a title against an alias, or an alias
    /// against either.
    ExactAlias,
    /// The titles share an [`identity_key`] but not a [`match_key`].
    NearTitle,
    /// Some near pair involves an alias.
    NearAlias,
}

/// One reason an existing Song is a possible duplicate of the candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityEvidence {
    /// One candidate name and one existing name share an [`identity_key`].
    Name {
        /// Which candidate title matched.
        candidate_on: MatchedOn,
        /// Which existing title matched.
        existing_on: MatchedOn,
        /// Whether the names also share a [`match_key`], so the pair is exact.
        exact: bool,
    },
    /// The candidate and the Song name the same original attribution by
    /// [`identity_key`]. Always last, and never reported without a name match.
    SameAttribution,
}

/// One existing Song that resembles a candidate draft, with its reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PossibleSong {
    /// The existing Song work.
    pub song: SongId,
    /// Its strongest matching name pair.
    pub strength: MatchStrength,
    /// Every matching name pair in candidate-name then existing-name order,
    /// plus [`IdentityEvidence::SameAttribution`] last when it applies.
    pub evidence: Vec<IdentityEvidence>,
}

impl SongLibrary {
    /// Existing Songs whose title or alias shares a [`match_key`] with the
    /// candidate's title or alias.
    ///
    /// Reports only: it never mutates, unites, or blocks anything, and creating
    /// or editing a Song never consults it. Archived Songs are included (the
    /// caller can flag them via [`Self::song`]); `except` omits the Song being
    /// edited. Ordered by Song creation time then identity, then candidate
    /// title, then stored title. Blank values never match.
    pub fn duplicate_evidence(
        &self,
        candidate: &SongDraft,
        except: Option<SongId>,
    ) -> Vec<DuplicateEvidence> {
        let keys = |title: &str, aliases: &[String]| -> Vec<(MatchedOn, String)> {
            std::iter::once((MatchedOn::Title, match_key(title)))
                .chain(
                    aliases
                        .iter()
                        .enumerate()
                        .map(|(index, alias)| (MatchedOn::Alias(index), match_key(alias))),
                )
                .filter(|(_, key)| !key.is_empty())
                .collect()
        };
        let candidate_keys = keys(&candidate.title, &candidate.aliases);
        let mut evidence = Vec::new();
        for song in self.songs_with_status(StatusFilter::All) {
            if Some(song.id()) == except {
                continue;
            }
            let existing_keys = keys(song.title(), song.aliases());
            for (candidate_on, candidate_key) in &candidate_keys {
                for (existing_on, existing_key) in &existing_keys {
                    if candidate_key == existing_key {
                        evidence.push(DuplicateEvidence {
                            song: song.id(),
                            candidate_on: *candidate_on,
                            existing_on: *existing_on,
                        });
                    }
                }
            }
        }
        evidence
    }

    /// Existing Songs ranked as possible duplicates of `candidate`, strongest
    /// first. Reports only: it reads `&self`, changes nothing, selects nothing
    /// and never blocks creation.
    ///
    /// Candidate names are the draft's title and aliases; a Song's names are its
    /// title and aliases, archived Songs included. A pair matches when their
    /// [`identity_key`]s are equal, and reports [`IdentityEvidence::Name`] with
    /// `exact` set when their [`match_key`]s are equal too. A name whose
    /// identity key is empty (punctuation or symbols only, such as "?!") still
    /// matches an equal [`match_key`], as an exact pair. A blank name matches
    /// nothing, so a blank draft returns an empty list.
    ///
    /// [`IdentityEvidence::SameAttribution`] is appended when the draft and the
    /// Song name the same original attribution by [`identity_key`], but only
    /// for a Song that already has a name match: attribution alone never
    /// suggests a Song.
    ///
    /// There is one entry per Song, ordered by [`MatchStrength`] (strongest
    /// first), then Songs sharing the attribution, then the existing Song's
    /// title [`match_key`], then identity.
    ///
    /// The draft is not validated: a draft the library would refuse still gets
    /// evidence.
    pub fn possible_existing_songs(&self, candidate: &SongDraft) -> Vec<PossibleSong> {
        let candidate_names = name_keys(&candidate.title, &candidate.aliases);
        if candidate_names.is_empty() {
            return Vec::new();
        }
        let candidate_attribution = attribution_key(candidate.original_attribution.as_deref());

        // (strength, attribution rank, existing title key, Song)
        let mut ranked: Vec<(MatchStrength, u8, String, PossibleSong)> = Vec::new();
        for song in self.songs_with_status(StatusFilter::All) {
            let existing_names = name_keys(song.title(), song.aliases());
            let mut evidence = Vec::new();
            let mut strength: Option<MatchStrength> = None;
            for candidate_name in &candidate_names {
                for existing_name in &existing_names {
                    // A name with no identity at all can only match exactly, so
                    // the pair falls back to the exact key. The exact key is
                    // computed only for pairs that already matched loosely.
                    let loose = candidate_name.identity.is_empty()
                        || existing_name.identity.is_empty()
                        || candidate_name.identity == existing_name.identity;
                    if !loose {
                        continue;
                    }
                    let exact = match_key(candidate_name.text) == match_key(existing_name.text);
                    if !exact
                        && (candidate_name.identity.is_empty() || existing_name.identity.is_empty())
                    {
                        continue;
                    }
                    evidence.push(IdentityEvidence::Name {
                        candidate_on: candidate_name.on,
                        existing_on: existing_name.on,
                        exact,
                    });
                    let pair = pair_strength(candidate_name.on, existing_name.on, exact);
                    strength = Some(strength.map_or(pair, |best| best.min(pair)));
                }
            }
            // No name pair matched, so the Song is not a candidate whatever its
            // attribution says.
            let Some(strength) = strength else { continue };
            let same_attribution = candidate_attribution
                .as_ref()
                .zip(attribution_key(song.original_attribution()))
                .is_some_and(|(candidate_key, existing_key)| candidate_key == &existing_key);
            if same_attribution {
                evidence.push(IdentityEvidence::SameAttribution);
            }
            ranked.push((
                strength,
                u8::from(!same_attribution),
                match_key(song.title()),
                PossibleSong {
                    song: song.id(),
                    strength,
                    evidence,
                },
            ));
        }
        ranked.sort_by(|left, right| {
            (left.0, left.1, &left.2, left.3.song).cmp(&(right.0, right.1, &right.2, right.3.song))
        });
        ranked
            .into_iter()
            .map(|(_, _, _, possible)| possible)
            .collect()
    }
}

/// One title or alias together with the near-match key it is compared by.
struct NameKey<'a> {
    /// Which title this is.
    on: MatchedOn,
    /// The name as authored; its [`match_key`] is computed only when a pair
    /// needs it.
    text: &'a str,
    /// The near-match key; empty when the name has no letters or digits.
    identity: String,
}

/// The title then aliases of a Song or draft, skipping blank names.
fn name_keys<'a>(title: &'a str, aliases: &'a [String]) -> Vec<NameKey<'a>> {
    std::iter::once((MatchedOn::Title, title))
        .chain(
            aliases
                .iter()
                .enumerate()
                .map(|(index, alias)| (MatchedOn::Alias(index), alias.as_str())),
        )
        .filter(|(_, text)| text.split_whitespace().next().is_some())
        .map(|(on, text)| NameKey {
            on,
            text,
            identity: identity_key(text),
        })
        .collect()
}

/// The strength of one matching name pair; `exact` means the keys also match.
const fn pair_strength(
    candidate_on: MatchedOn,
    existing_on: MatchedOn,
    exact: bool,
) -> MatchStrength {
    let titles = matches!(
        (candidate_on, existing_on),
        (MatchedOn::Title, MatchedOn::Title)
    );
    match (exact, titles) {
        (true, true) => MatchStrength::ExactTitle,
        (true, false) => MatchStrength::ExactAlias,
        (false, true) => MatchStrength::NearTitle,
        (false, false) => MatchStrength::NearAlias,
    }
}

/// The non-empty [`identity_key`] of an optional attribution.
fn attribution_key(text: Option<&str>) -> Option<String> {
    text.map(identity_key).filter(|key| !key.is_empty())
}
