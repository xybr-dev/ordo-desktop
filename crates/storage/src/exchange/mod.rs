//! `.ordo` inspection and song-import v2 decoding (T2.10).
//!
//! A read takes a file's bytes to a typed [`domain::song_library::ImportDocument`]
//! or a list of findings. It reads the archive into memory, never extracts to
//! disk, and takes no handle on any Parish: reading cannot change anything.
//!
//! The container rules are ported from the T0.8 spike (entry-name rules,
//! duplicate and unlisted entries, links, size and ratio ceilings, manifest
//! strictness, payload size and hash) with the kind set narrowed to
//! `song-import`; the production code does not depend on the spike.

mod container;
mod findings;
mod song_import;

pub use container::InspectLimits;
#[cfg(feature = "fault-injection")]
pub use container::{ManifestOverrides, RawEntry, pack_for_tests, pack_raw_for_tests};
pub use findings::{Finding, FindingCode};
pub use song_import::{
    SongImportRead, encode_song_import, read_song_import, read_song_import_bytes,
};
