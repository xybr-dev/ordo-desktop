//! Parish store: typed creation and reading of the Parish, Choirs, Songs and
//! Arrangements.
//!
//! Callers pass plain input values and receive plain records. Every
//! multi-row create is one immediate transaction that commits only after the
//! last row is written, so a failure leaves earlier committed state unchanged.

mod read;
mod records;
pub(crate) mod write;

pub(crate) use read::{read_arrangement, read_song};
pub use records::{
    ArrangementRecord, AssetReport, AssetUpload, ChoirRecord, ManagedAssetRecord, NewArrangement,
    NewChoir, NewParish, NewSection, NewSong, NewTheme, OccurrenceRecord, ParishRecord,
    SectionRecord, SongRecord, ThemeRecord,
};

#[cfg(test)]
mod tests {
    #[test]
    fn sql_is_never_built_from_strings() {
        let sources = [
            ("records.rs", include_str!("records.rs")),
            ("write.rs", include_str!("write.rs")),
            ("read.rs", include_str!("read.rs")),
            (
                "revisions/content.rs",
                include_str!("../revisions/content.rs"),
            ),
            ("revisions/write.rs", include_str!("../revisions/write.rs")),
            ("revisions/read.rs", include_str!("../revisions/read.rs")),
        ];
        for (name, text) in sources {
            for needle in ["format!", "execute_batch", "concat!", "push_str"] {
                assert!(!text.contains(needle), "{name} contains {needle}");
            }
        }
    }
}
