//! Compile-time catalog of Ordo's bundled operator assets.

pub const ATKINSON_HYPERLEGIBLE_NEXT: &[u8] =
    include_bytes!("../assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf");
pub const NOTO_SANS: &[u8] = include_bytes!("../assets/fonts/noto-sans/NotoSans[wdth,wght].ttf");

pub mod icon {
    /// Declares each icon constant and the `ALL` catalog from one list, so a
    /// constant cannot be dropped while its file and manifest entry remain —
    /// `include_str!` only catches the file disappearing, not the reverse.
    macro_rules! lucide {
        ($($konst:ident => $file:literal),+ $(,)?) => {
            $(pub const $konst: &str = include_str!(concat!("../assets/icons/lucide/", $file));)+

            /// Every bundled Lucide icon as `(file stem, SVG source)`.
            pub const ALL: &[(&str, &str)] = &[$(($file, $konst)),+];
        };
    }

    lucide! {
        ACTIVITY => "activity.svg",
        CALENDAR_DAYS => "calendar-days.svg",
        CHECK => "check.svg",
        CHEVRON_DOWN => "chevron-down.svg",
        CHEVRON_RIGHT => "chevron-right.svg",
        CIRCLE_ALERT => "circle-alert.svg",
        CIRCLE_QUESTION_MARK => "circle-question-mark.svg",
        DATABASE_BACKUP => "database-backup.svg",
        ELLIPSIS => "ellipsis.svg",
        GRIP_VERTICAL => "grip-vertical.svg",
        INFO => "info.svg",
        LIBRARY => "library.svg",
        SETTINGS => "settings.svg",
        SWATCH_BOOK => "swatch-book.svg",
        TRIANGLE_ALERT => "triangle-alert.svg",
        X => "x.svg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closes the direction `include_str!` cannot: a constant removed from the
    /// catalog while its file and manifest entry stay behind.
    #[test]
    fn catalog_covers_every_bundled_icon() {
        let bundled: Vec<_> = std::fs::read_dir("assets/icons/lucide")
            .expect("bundled Lucide directory must exist")
            .map(|entry| entry.expect("readable entry").file_name())
            .collect();

        assert_eq!(
            icon::ALL.len(),
            bundled.len(),
            "assets/icons/lucide holds {} files but the catalog declares {}",
            bundled.len(),
            icon::ALL.len()
        );
        for (file, svg) in icon::ALL {
            assert!(
                bundled.iter().any(|name| name == file),
                "catalog names {file}, which is not bundled"
            );
            assert!(!svg.trim().is_empty(), "{file} is empty");
        }
    }

    #[test]
    fn bundled_fonts_are_truetype() {
        for (name, font) in [
            ("Atkinson Hyperlegible Next", ATKINSON_HYPERLEGIBLE_NEXT),
            ("Noto Sans", NOTO_SANS),
        ] {
            assert_eq!(
                &font[..4],
                b"\x00\x01\x00\x00",
                "{name} is not a TrueType outline font"
            );
        }
    }
}
