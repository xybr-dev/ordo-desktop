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

/// The backup reminder Masses → Upcoming shows in its header: days since the
/// last backup and the changes since. T1 reads it from the Storage Module.
pub const BACKUP_REMINDER: (u32, u32) = (9, 46);

pub struct Mass {
    pub time: &'static str,
    pub anticipated: bool,
    pub language: &'static str,
    pub choir: Option<&'static str>,
    pub presider: &'static str,
    pub incomplete: u32,
    pub warnings: u32,
}

pub struct Celebration {
    pub title: &'static str,
    pub date: &'static str,
    pub rank: &'static str,
    pub color: &'static str,
    pub swatch: u32,
    pub installed: bool,
    pub masses: Vec<Mass>,
}

const PRESIDER: &str = "Fr. Ramon Abellana";

fn mass(
    time: &'static str,
    language: &'static str,
    choir: Option<&'static str>,
    incomplete: u32,
    warnings: u32,
) -> Mass {
    Mass {
        time,
        anticipated: false,
        language,
        choir,
        presider: PRESIDER,
        incomplete,
        warnings,
    }
}

/// Masses → Upcoming, as the `masses-index` mockup shows it: Celebrations in
/// date order with their Mass plans, one with an anticipated Mass, one from an
/// installed package, and one with no plans yet.
pub fn upcoming() -> Vec<Celebration> {
    vec![
        Celebration {
            title: "All Saints",
            date: "Sun 1 Nov 2026",
            rank: "Solemnity",
            color: "White",
            swatch: tokens::color::LITURGICAL_WHITE,
            installed: false,
            masses: vec![
                Mass {
                    anticipated: true,
                    ..mass(
                        "Sat 31 Oct \u{b7} 18:00",
                        "Cebuano",
                        Some("Koro San Roque"),
                        0,
                        0,
                    )
                },
                mass(
                    "Sun 1 Nov \u{b7} 06:00",
                    "Cebuano",
                    Some("Koro Sta. Cecilia"),
                    0,
                    1,
                ),
                Mass {
                    presider: "Fr. Jun Tabada",
                    ..mass(
                        "Sun 1 Nov \u{b7} 09:00",
                        "English",
                        Some("Youth Choir"),
                        1,
                        2,
                    )
                },
                mass("Sun 1 Nov \u{b7} 17:00", "Tagalog", None, 0, 0),
            ],
        },
        Celebration {
            title: "All Souls",
            date: "Mon 2 Nov 2026",
            rank: "Commemoration",
            color: "Violet",
            swatch: tokens::color::LITURGICAL_PURPLE,
            installed: false,
            masses: vec![
                mass(
                    "Mon 2 Nov \u{b7} 06:00",
                    "Cebuano",
                    Some("Koro San Roque"),
                    0,
                    2,
                ),
                mass("Mon 2 Nov \u{b7} 18:00", "Cebuano", None, 3, 0),
            ],
        },
        Celebration {
            title: "Ika-32 nga Domingo sa Ordinaryong Panahon",
            date: "Sun 8 Nov 2026",
            rank: "Ordinary Time",
            color: "Green",
            swatch: tokens::color::LITURGICAL_GREEN,
            installed: true,
            masses: vec![
                mass(
                    "Sun 8 Nov \u{b7} 06:00",
                    "Cebuano",
                    Some("Koro Sta. Cecilia"),
                    0,
                    0,
                ),
                mass(
                    "Sun 8 Nov \u{b7} 09:00",
                    "Cebuano",
                    Some("Koro San Roque"),
                    0,
                    0,
                ),
            ],
        },
        Celebration {
            title: "Ika-33 nga Domingo sa Ordinaryong Panahon",
            date: "Sun 15 Nov 2026",
            rank: "Ordinary Time",
            color: "Green",
            swatch: tokens::color::LITURGICAL_GREEN,
            installed: false,
            masses: Vec::new(),
        },
    ]
}
