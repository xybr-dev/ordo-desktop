//! Ordo's built-in language list (Decision 277).
//!
//! An Arrangement language is picked from this fixed list and never typed in
//! as a new value, so one language cannot end up under several spellings.
//! Each entry has one display name, which is the value stored on an
//! Arrangement, and other names people search by. Lookup folds case and
//! diacritics like [`super::search_key`].

use super::search::search_key;

/// One built-in language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Language {
    name: &'static str,
    other_names: &'static [&'static str],
}

impl Language {
    /// The display name; this is the value an Arrangement stores.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Other names people search by, such as Ilonggo for Hiligaynon.
    pub fn other_names(&self) -> &'static [&'static str] {
        self.other_names
    }
}

/// One language found by [`find_language`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageMatch {
    /// The language that matched.
    pub language: Language,
    /// The other name that matched when the display name did not, such as
    /// "Ilonggo" for a search of "ilong"; `None` for a display-name match.
    pub matched_alias: Option<&'static str>,
}

const fn lang(name: &'static str, other_names: &'static [&'static str]) -> Language {
    Language { name, other_names }
}

/// The fixed list, in the order the picker shows it.
static LANGUAGES: [Language; 15] = [
    lang("Cebuano", &["Bisaya", "Binisaya", "Sinugbuanon", "Visayan"]),
    lang("English", &[]),
    lang("Tagalog", &["Filipino", "Pilipino"]),
    lang("Latin", &[]),
    lang("Hiligaynon", &["Ilonggo"]),
    lang("Waray", &["Winaray", "Samar-Leyte"]),
    lang("Ilocano", &["Iloko"]),
    lang("Kapampangan", &["Pampango"]),
    lang("Bikol", &["Bicolano"]),
    lang("Pangasinan", &[]),
    lang("Chavacano", &["Chabacano", "Zamboangueño"]),
    lang("Spanish", &["Español"]),
    lang("Italian", &[]),
    lang("Korean", &[]),
    lang("Chinese (Mandarin)", &[]),
];

/// Every built-in language, in picker order.
pub fn languages() -> &'static [Language] {
    &LANGUAGES
}

/// The built-in languages a search matches, in picker order.
///
/// A language matches when the folded query is contained in its folded display
/// name, or else in one of its folded other names; the latter reports the
/// alias that matched. A blank query matches every language with no alias.
pub fn find_language(query: &str) -> Vec<LanguageMatch> {
    let needle = search_key(query);
    LANGUAGES
        .iter()
        .filter_map(|language| {
            if search_key(language.name).contains(&needle) {
                return Some(LanguageMatch {
                    language: *language,
                    matched_alias: None,
                });
            }
            language
                .other_names
                .iter()
                .find(|alias| search_key(alias).contains(&needle))
                .map(|alias| LanguageMatch {
                    language: *language,
                    matched_alias: Some(*alias),
                })
        })
        .collect()
}

/// The built-in language whose display name is `name`, ignoring case and
/// diacritics; other names do not count.
pub(crate) fn canonical(name: &str) -> Option<&'static Language> {
    let key = search_key(name);
    LANGUAGES
        .iter()
        .find(|language| search_key(language.name) == key)
}

/// Whether `name` is the display name of a built-in language, ignoring case
/// and diacritics. Other names such as "Ilonggo" are not listed.
pub fn is_listed(name: &str) -> bool {
    canonical(name).is_some()
}
