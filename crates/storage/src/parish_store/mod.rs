//! Parish store: typed creation and reading of the Parish, Choirs, Songs and
//! Arrangements.
//!
//! Callers pass plain input values and receive plain records. Every
//! multi-row create is one immediate transaction that commits only after the
//! last row is written, so a failure leaves earlier committed state unchanged.

mod read;
mod records;
mod write;

pub use records::{
    ArrangementRecord, ChoirRecord, NewArrangement, NewChoir, NewParish, NewSection, NewSong,
    OccurrenceRecord, ParishRecord, SectionRecord, SongRecord,
};

#[cfg(test)]
mod tests {
    #[test]
    fn sql_is_never_built_from_strings() {
        let sources = [
            ("records.rs", include_str!("records.rs")),
            ("write.rs", include_str!("write.rs")),
            ("read.rs", include_str!("read.rs")),
        ];
        for (name, text) in sources {
            for needle in ["format!", "execute_batch", "concat!", "push_str"] {
                assert!(!text.contains(needle), "{name} contains {needle}");
            }
        }
    }
}
