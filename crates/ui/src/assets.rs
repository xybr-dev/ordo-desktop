//! Compile-time catalog of Ordo's bundled operator assets.

pub const ATKINSON_HYPERLEGIBLE_NEXT: &[u8] = include_bytes!(
    "../../../assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf"
);
pub const NOTO_SANS: &[u8] =
    include_bytes!("../../../assets/fonts/noto-sans/NotoSans[wdth,wght].ttf");

pub mod icon {
    /// Declares each icon constant and the `ALL` catalog from one list, so a
    /// constant cannot be dropped while its file and manifest entry remain —
    /// `include_str!` only catches the file disappearing, not the reverse.
    macro_rules! lucide {
        ($($konst:ident => $file:literal),+ $(,)?) => {
            $(pub const $konst: &str = include_str!(concat!("../assets/icons/lucide/", $file));)+

            /// Every bundled Lucide icon as `(file name, SVG source)`.
            pub const ALL: &[(&str, &str)] = &[$(($file, $konst)),+];
        };
    }

    lucide! {
        ACTIVITY => "activity.svg",
        CALENDAR_DAYS => "calendar-days.svg",
        CHECK => "check.svg",
        CHEVRON_DOWN => "chevron-down.svg",
        CHEVRON_LEFT => "chevron-left.svg",
        CHEVRON_RIGHT => "chevron-right.svg",
        CIRCLE_ALERT => "circle-alert.svg",
        CIRCLE_QUESTION_MARK => "circle-question-mark.svg",
        DATABASE_BACKUP => "database-backup.svg",
        ELLIPSIS => "ellipsis.svg",
        GRIP_VERTICAL => "grip-vertical.svg",
        INFO => "info.svg",
        LIBRARY => "library.svg",
        PLUS => "plus.svg",
        SETTINGS => "settings.svg",
        SWATCH_BOOK => "swatch-book.svg",
        TRIANGLE_ALERT => "triangle-alert.svg",
        X => "x.svg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::typography;

    const LUCIDE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/lucide");

    /// Closes the direction `include_str!` cannot: a constant removed from the
    /// catalog while its file and manifest entry stay behind.
    #[test]
    fn catalog_covers_every_bundled_icon() {
        let bundled: Vec<_> = std::fs::read_dir(LUCIDE_DIR)
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

    fn be_u16(bytes: &[u8], at: usize) -> usize {
        u16::from_be_bytes([bytes[at], bytes[at + 1]]).into()
    }

    /// A 16.16 fixed-point value, as `fvar` stores axis bounds.
    fn be_fixed(bytes: &[u8], at: usize) -> f32 {
        i32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes")) as f32 / 65536.0
    }

    /// The inclusive `wght` axis range of a variable font, or `None` when the
    /// face carries no `fvar` table and is therefore a static instance.
    fn weight_axis(font: &[u8]) -> Option<(f32, f32)> {
        let table_count = be_u16(font, 4);
        let fvar = (0..table_count)
            .map(|index| 12 + index * 16)
            .find(|record| &font[*record..record + 4] == b"fvar")
            .map(|record| {
                u32::from_be_bytes(font[record + 8..record + 12].try_into().expect("4 bytes"))
                    as usize
            })?;

        let axes = fvar + be_u16(font, fvar + 4);
        let (axis_count, axis_size) = (be_u16(font, fvar + 8), be_u16(font, fvar + 10));
        (0..axis_count)
            .map(|index| axes + index * axis_size)
            .find(|axis| &font[*axis..axis + 4] == b"wght")
            .map(|axis| (be_fixed(font, axis + 4), be_fixed(font, axis + 12)))
    }

    /// DESIGN.md reserves distinct weights for operator hierarchy and projected
    /// text. A static instance substituted for either face still passes an sfnt
    /// signature check, then silently synthesises every weight it lacks, so the
    /// property worth asserting is the `wght` range, not the file's magic bytes.
    #[test]
    fn bundled_fonts_cover_every_declared_weight() {
        let declared = [
            typography::TITLE.weight,
            typography::BODY.weight,
            typography::LABEL.weight,
            typography::PROJECTED_RESPONSE.weight,
        ];

        for (name, font) in [
            ("Atkinson Hyperlegible Next", ATKINSON_HYPERLEGIBLE_NEXT),
            ("Noto Sans", NOTO_SANS),
        ] {
            assert_eq!(
                &font[..4],
                b"\x00\x01\x00\x00",
                "{name} is not a TrueType outline font"
            );
            let (min, max) = weight_axis(font)
                .unwrap_or_else(|| panic!("{name} has no variable wght axis; it is a static face"));
            for weight in declared {
                let weight = f32::from(weight);
                assert!(
                    (min..=max).contains(&weight),
                    "{name} covers wght {min}-{max}, which cannot render the declared {weight}"
                );
            }
        }
    }
}
