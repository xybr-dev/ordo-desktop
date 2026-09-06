//! Placeholder records for the shell's primary list.
//!
//! T1 replaces every one of these with real Parish records read through the
//! Storage Module, so they live in their own file: the removal is then a
//! deleted file and a deleted `mod` line rather than surgery on the shell.
//! Nothing here is domain vocabulary — `CONTEXT.md` owns that — and nothing
//! outside the shell may consume it.

use gpui::SharedString;
use ui::tokens;

/// One row is long and multilingual on purpose: a workspace that cannot hold it
/// would report the horizontal scroll `DESIGN.md` forbids, and there are enough
/// of them that the column needs the shared scroll indicator.
const LONG_TITLE: &str =
    "Pasidungog sa Mahal nga Birhen Maria sa Kaputli ug sa Kalinaw sa Katilingban";

/// A liturgical season is real domain metadata, which is the one thing
/// `DESIGN.md` lets the liturgical colors say.
const SEASONS: [(&str, u32); 4] = [
    ("Lent", tokens::color::LITURGICAL_PURPLE),
    ("Easter", tokens::color::LITURGICAL_WHITE),
    ("Ordinary Time", tokens::color::LITURGICAL_GREEN),
    ("Pentecost", tokens::color::LITURGICAL_RED),
];

pub struct Entry {
    pub title: SharedString,
    pub detail: SharedString,
    pub season: Option<(&'static str, u32)>,
}

pub fn rows(view: &str) -> Vec<Entry> {
    let times = ["6:00 AM", "7:30 AM", "9:30 AM", "5:00 PM"];
    let languages = ["Cebuano", "English", "Filipino", "Cebuano"];
    (0..8)
        .map(|index| Entry {
            title: if index == 7 {
                LONG_TITLE.into()
            } else {
                format!("{view} \u{b7} entry {}", index + 1).into()
            },
            detail: format!(
                "{} \u{b7} {}",
                times[index % times.len()],
                languages[index % languages.len()]
            )
            .into(),
            // Not every entry carries a season, so a row without a badge has to
            // read as an ordinary row rather than a broken one.
            season: (index % 3 != 2).then(|| SEASONS[index % SEASONS.len()]),
        })
        .collect()
}
