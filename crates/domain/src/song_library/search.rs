//! Song Library search, filters and match evidence (Decision 41, T2.4).
//!
//! [`search`] is one pure function over a [`SongLibrary`]: it reads no clock,
//! draws no randomness, performs no I/O and never changes archive state (it
//! takes `&SongLibrary`). Authored text is never altered: evidence carries the
//! authored strings verbatim, and normalization ([`search_key`]) is applied
//! only to the copies that are compared.
//!
//! # Fields searched
//!
//! Song-level: the primary title, aliases, original attribution and
//! provenance. Arrangement-level: Arrangement title, rendition attribution,
//! the current names of associated Choirs, languages, and every lyric line
//! (section lyrics split on `'\n'`). Rights notes are **not** searched: they
//! are private rights bookkeeping, not discovery metadata.
//!
//! # Filters
//!
//! Choir, language and occasion filters are Arrangement-scoped (CONTEXT
//! "Arrangement filter match"): a Song is retained only when at least one of
//! its counted Arrangements satisfies every active filter kind at once. Values
//! within one kind are OR-ed; [`FilterValue::Unspecified`] matches only an
//! Arrangement with no value of that kind. An Arrangement's occasions for
//! filtering are the Song's occasion tags together with its own (CONTEXT
//! "Occasion applicability tag"); `Unspecified` matches only when both are
//! empty.
//!
//! # Archive rule
//!
//! Without `include_archived`, archived Songs are excluded and archived
//! Arrangements, and active Arrangements under an archived Song, are neither
//! counted nor matched.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{ArrangementId, ChoirId, SectionId, SongId};

use super::{Arrangement, SongLibrary, SongWork};

/// The most Lyric evidences one Song hit reports.
const MAX_LYRIC_EVIDENCE: usize = 3;

/// The one search-as-you-type query (Decision 41).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryQuery {
    /// The text as typed; normalized internally and never stored back.
    pub text: String,
    /// Choir filter values, OR-ed.
    pub choirs: BTreeSet<FilterValue<ChoirId>>,
    /// Language filter values, OR-ed.
    pub languages: BTreeSet<FilterValue<String>>,
    /// Occasion filter values, OR-ed.
    pub occasions: BTreeSet<FilterValue<String>>,
    /// Whether archived Songs and Arrangements are counted and matched.
    pub include_archived: bool,
}

/// One filter value. `Unspecified` matches only an Arrangement with no value
/// of that kind; it is never a wildcard.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FilterValue<T> {
    /// An Arrangement carrying this value.
    Is(T),
    /// An Arrangement carrying no value of this kind.
    Unspecified,
}

/// Which attribution text matched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributionField {
    /// The Song's original attribution.
    Original,
    /// The Song's provenance.
    Provenance,
    /// One Arrangement's rendition attribution.
    Rendition {
        /// The Arrangement carrying it.
        arrangement: ArrangementId,
    },
}

/// Why a Song matched the query text. Authored text is verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Evidence {
    /// The Song's primary title.
    Title,
    /// An alias, as authored.
    Alias {
        /// The alias text.
        text: String,
    },
    /// An attribution text.
    Attribution {
        /// Which attribution.
        field: AttributionField,
        /// The attribution text.
        text: String,
    },
    /// An Arrangement title.
    ArrangementTitle {
        /// The Arrangement.
        arrangement: ArrangementId,
        /// Its title.
        text: String,
    },
    /// The current name of a Choir associated with an Arrangement.
    Choir {
        /// The Arrangement.
        arrangement: ArrangementId,
        /// The Choir.
        choir: ChoirId,
        /// The Choir's current name.
        name: String,
    },
    /// An Arrangement language.
    Language {
        /// The Arrangement.
        arrangement: ArrangementId,
        /// The language text.
        text: String,
    },
    /// One lyric line.
    Lyric {
        /// The Arrangement.
        arrangement: ArrangementId,
        /// The Section holding the line.
        section: SectionId,
        /// The whole matching line, verbatim except its line terminator.
        line: String,
    },
}

/// One Song in the result list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongHit {
    /// The Song work.
    pub song: SongId,
    /// Why it matched, strongest first (Title, Alias, Attribution,
    /// ArrangementTitle, Choir, Language, Lyric). Empty when the query text is
    /// empty.
    pub evidence: Vec<Evidence>,
    /// The Arrangements satisfying every active filter and, when the text
    /// matched only through Arrangement fields, the text as well.
    pub matching_arrangements: Vec<ArrangementId>,
    /// The Song's Arrangements counted under the archive rule.
    pub total_arrangements: usize,
}

/// Search-only normalization: [`super::match_key`] (case and whitespace) plus
/// diacritic folding of Latin letters. Never applied to stored text.
///
/// The fold is a fixed table, not Unicode decomposition: combining marks
/// (U+0300 to U+036F) are dropped, and the lowercase letters of the Latin-1
/// Supplement and Latin Extended-A blocks map as follows (everything else,
/// including digits and punctuation, is unchanged):
///
/// - a: à á â ã ä å ā ă ą; ae: æ
/// - c: ç ć ĉ ċ č; d: ð ď đ
/// - e: è é ê ë ē ĕ ė ę ě
/// - g: ĝ ğ ġ ģ; h: ĥ ħ
/// - i: ì í î ï ĩ ī ĭ į ı; ij: ĳ; j: ĵ
/// - k: ķ ĸ; l: ĺ ļ ľ ŀ ł
/// - n: ñ ń ņ ň ŉ ŋ
/// - o: ò ó ô õ ö ø ō ŏ ő; oe: œ
/// - r: ŕ ŗ ř; s: ś ŝ ş š ſ; ss: ß
/// - t: ţ ť ŧ; th: þ
/// - u: ù ú û ü ũ ū ŭ ů ű ų; w: ŵ
/// - y: ý ÿ ŷ; z: ź ż ž
///
/// Precomposed letters outside this table (for example `ỹ` or `ǎ`) are
/// kept as they are, while their decomposed forms fold to the base letter.
pub fn search_key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    search_key_into(text, &mut out);
    out
}

/// [`search_key`] into a reusable buffer (cleared first). ASCII text takes a
/// single pass with no intermediate allocation.
fn search_key_into(text: &str, out: &mut String) {
    out.clear();
    if text.is_ascii() {
        let mut pending_space = false;
        for c in text.chars() {
            if c.is_whitespace() {
                pending_space = !out.is_empty();
            } else {
                if pending_space {
                    out.push(' ');
                    pending_space = false;
                }
                out.push(c.to_ascii_lowercase());
            }
        }
        return;
    }
    // One pass: lowercase, drop combining marks, fold, and collapse
    // whitespace. A dropped mark never creates or removes a separator.
    let mut pending_space = false;
    for c in text.to_lowercase().chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        let folded = fold(c);
        if matches!(folded, Fold::Drop) {
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        match folded {
            Fold::Keep => out.push(c),
            Fold::To(s) => out.push_str(s),
            Fold::Drop => {}
        }
    }
}

enum Fold {
    Keep,
    Drop,
    To(&'static str),
}

fn fold(c: char) -> Fold {
    if c.is_ascii() {
        return Fold::Keep;
    }
    Fold::To(match c {
        '\u{300}'..='\u{36f}' => return Fold::Drop,
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ð' | 'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĳ' => "ij",
        'ĵ' => "j",
        'ķ' | 'ĸ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ſ' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return Fold::Keep,
    })
}

/// Whether `field` contains the already-normalized `needle`.
fn contains(field: &str, needle: &str) -> bool {
    if field.is_ascii() && needle.is_ascii() {
        return ascii_contains(field.as_bytes(), needle.as_bytes());
    }
    thread_local! {
        static SCRATCH: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }
    SCRATCH.with(|scratch| {
        let mut key = scratch.borrow_mut();
        search_key_into(field, &mut key);
        key.contains(needle)
    })
}

/// [`search_key`] containment for ASCII text without allocating: the needle
/// is already normalized (lowercase, single spaces, trimmed), so a needle space
/// matches one or more whitespace bytes and letters compare case-insensitively.
fn ascii_contains(field: &[u8], needle: &[u8]) -> bool {
    let Some(&first) = needle.first() else {
        return true;
    };
    let (lower, upper) = (first, first.to_ascii_uppercase());
    let mut start = 0;
    while let Some(offset) = field[start..]
        .iter()
        .position(|&b| b == lower || b == upper)
    {
        if ascii_matches_at(&field[start + offset..], needle) {
            return true;
        }
        start += offset + 1;
    }
    false
}

fn ascii_matches_at(mut field: &[u8], needle: &[u8]) -> bool {
    for &want in needle {
        if want == b' ' {
            let blanks = field
                .iter()
                .take_while(|&&b| b.is_ascii_whitespace() || b == 0x0B)
                .count();
            if blanks == 0 {
                return false;
            }
            field = &field[blanks..];
        } else {
            match field.split_first() {
                Some((&got, rest)) if got.to_ascii_lowercase() == want => field = rest,
                _ => return false,
            }
        }
    }
    true
}

/// What the text found in one Arrangement.
struct ArrangementText {
    evidence: Vec<Evidence>,
    matched: bool,
}

/// Searches `library` for `query`; see the module documentation for the rules.
///
/// Hits rank by strongest evidence (Title, then Alias, then anything else),
/// then by Song title [`search_key`], then by identity.
pub fn search(library: &SongLibrary, query: &LibraryQuery) -> Vec<SongHit> {
    let needle = search_key(&query.text);
    let filters_active =
        !(query.choirs.is_empty() && query.languages.is_empty() && query.occasions.is_empty());

    let mut by_song: BTreeMap<SongId, Vec<&Arrangement>> = BTreeMap::new();
    for arrangement in library.arrangements() {
        by_song
            .entry(arrangement.song())
            .or_default()
            .push(arrangement);
    }

    let mut ranked: Vec<(u8, String, SongHit)> = Vec::new();
    for song in library.songs() {
        if song.is_archived() && !query.include_archived {
            continue;
        }
        let mut counted = by_song.remove(&song.id()).unwrap_or_default();
        counted.retain(|a| query.include_archived || !a.is_archived());
        counted.sort_by_key(|a| (a.created_at(), a.id()));

        let passing: Vec<&Arrangement> = counted
            .iter()
            .copied()
            .filter(|a| passes_filters(song, a, query))
            .collect();
        if filters_active && passing.is_empty() {
            continue;
        }

        let hit = if needle.is_empty() {
            SongHit {
                song: song.id(),
                evidence: Vec::new(),
                matching_arrangements: passing.iter().map(|a| a.id()).collect(),
                total_arrangements: counted.len(),
            }
        } else {
            match text_hit(library, song, &needle, &passing, counted.len()) {
                Some(hit) => hit,
                None => continue,
            }
        };
        let rank = match hit.evidence.first() {
            Some(Evidence::Title) => 0,
            Some(Evidence::Alias { .. }) => 1,
            _ => 2,
        };
        ranked.push((rank, search_key(song.title()), hit));
    }
    ranked.sort_by(|a, b| (a.0, &a.1, a.2.song).cmp(&(b.0, &b.1, b.2.song)));
    ranked.into_iter().map(|(_, _, hit)| hit).collect()
}

/// The hit for a non-empty needle, or `None` when the Song does not match.
fn text_hit(
    library: &SongLibrary,
    song: &SongWork,
    needle: &str,
    passing: &[&Arrangement],
    total: usize,
) -> Option<SongHit> {
    let mut title = Vec::new();
    let mut aliases = Vec::new();
    let mut attributions = Vec::new();
    if contains(song.title(), needle) {
        title.push(Evidence::Title);
    }
    for alias in song.aliases() {
        if contains(alias, needle) {
            aliases.push(Evidence::Alias {
                text: alias.clone(),
            });
        }
    }
    for (field, text) in [
        (AttributionField::Original, song.original_attribution()),
        (AttributionField::Provenance, song.provenance()),
    ] {
        if let Some(text) = text.filter(|t| contains(t, needle)) {
            attributions.push(Evidence::Attribution {
                field,
                text: text.to_owned(),
            });
        }
    }
    let song_matched = !(title.is_empty() && aliases.is_empty() && attributions.is_empty());

    let mut renditions = Vec::new();
    let mut titles = Vec::new();
    let mut choirs = Vec::new();
    let mut languages = Vec::new();
    let mut lyrics = Vec::new();
    let mut matching = Vec::new();
    for arrangement in passing {
        let found = arrangement_text(library, arrangement, needle);
        if song_matched || found.matched {
            matching.push(arrangement.id());
        }
        for evidence in found.evidence {
            match evidence {
                Evidence::Attribution { .. } => renditions.push(evidence),
                Evidence::ArrangementTitle { .. } => titles.push(evidence),
                Evidence::Choir { .. } => choirs.push(evidence),
                Evidence::Language { .. } => languages.push(evidence),
                Evidence::Lyric { .. } => lyrics.push(evidence),
                Evidence::Title | Evidence::Alias { .. } => {}
            }
        }
    }
    if !song_matched && matching.is_empty() {
        return None;
    }
    lyrics.truncate(MAX_LYRIC_EVIDENCE);
    let evidence = [
        title,
        aliases,
        attributions,
        renditions,
        titles,
        choirs,
        languages,
        lyrics,
    ]
    .concat();
    Some(SongHit {
        song: song.id(),
        evidence,
        matching_arrangements: matching,
        total_arrangements: total,
    })
}

/// The evidence one Arrangement's own fields give for `needle`.
fn arrangement_text(
    library: &SongLibrary,
    arrangement: &Arrangement,
    needle: &str,
) -> ArrangementText {
    let id = arrangement.id();
    let mut evidence = Vec::new();
    if let Some(text) = arrangement
        .rendition_attribution()
        .filter(|t| contains(t, needle))
    {
        evidence.push(Evidence::Attribution {
            field: AttributionField::Rendition { arrangement: id },
            text: text.to_owned(),
        });
    }
    if let Some(title) = arrangement.title().filter(|t| contains(t, needle)) {
        evidence.push(Evidence::ArrangementTitle {
            arrangement: id,
            text: title.to_owned(),
        });
    }
    for &choir in arrangement.choirs() {
        if let Ok(found) = library.choir(choir)
            && contains(found.name(), needle)
        {
            evidence.push(Evidence::Choir {
                arrangement: id,
                choir,
                name: found.name().to_owned(),
            });
        }
    }
    for language in arrangement.languages() {
        if contains(language, needle) {
            evidence.push(Evidence::Language {
                arrangement: id,
                text: language.clone(),
            });
        }
    }
    for section in arrangement.sections() {
        for line in section.lyrics().split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if contains(line, needle) {
                evidence.push(Evidence::Lyric {
                    arrangement: id,
                    section: section.id(),
                    line: line.to_owned(),
                });
            }
        }
    }
    ArrangementText {
        matched: !evidence.is_empty(),
        evidence,
    }
}

/// Whether the Arrangement satisfies every active filter kind.
fn passes_filters(song: &SongWork, arrangement: &Arrangement, query: &LibraryQuery) -> bool {
    let occasions: BTreeSet<&String> = song
        .occasions()
        .iter()
        .chain(arrangement.occasions())
        .collect();
    kind_passes(&query.choirs, arrangement.choirs().len(), |c| {
        arrangement.choirs().contains(c)
    }) && kind_passes(&query.languages, arrangement.languages().len(), |value| {
        let key = search_key(value);
        arrangement.languages().iter().any(|l| search_key(l) == key)
    }) && kind_passes(&query.occasions, occasions.len(), |value| {
        let key = search_key(value);
        occasions.iter().any(|o| search_key(o) == key)
    })
}

/// One filter kind: inactive when empty, otherwise OR over its values.
fn kind_passes<T>(
    values: &BTreeSet<FilterValue<T>>,
    carried: usize,
    has: impl Fn(&T) -> bool,
) -> bool {
    values.is_empty()
        || values.iter().any(|value| match value {
            FilterValue::Is(v) => has(v),
            FilterValue::Unspecified => carried == 0,
        })
}
