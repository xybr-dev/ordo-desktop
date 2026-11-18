//! The findings a read of an `.ordo` file reports.

/// What kind of problem a [`Finding`] names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindingCode {
    /// The archive or its manifest is malformed, unsafe, unlisted or tampered
    /// with (and anything else about the container itself).
    Container,
    /// The payload does not match the song-import contract.
    Schema,
    /// The payload names a contract version this build does not read.
    UnsupportedVersion,
    /// A Sequence entry names a section the Arrangement does not have.
    MissingReference,
    /// An Arrangement's recorded content hash differs from the recomputed one.
    HashMismatch,
    /// The container is a well-formed Ordo file of another kind.
    WrongKind,
    /// A song-import container lists assets, which it never carries.
    Assets,
}

/// One reason a file cannot be imported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    /// What kind of problem it is.
    pub code: FindingCode,
    /// An RFC 6901 JSON Pointer into the payload; empty for a container-level
    /// finding.
    pub pointer: String,
    /// The technical detail, for **View technical details**.
    pub message: String,
}

impl Finding {
    pub(crate) fn new(
        code: FindingCode,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    pub(crate) fn container(message: impl Into<String>) -> Self {
        Self::new(FindingCode::Container, "", message)
    }
}
