//! The persistence seam of an edit session (Decision 140).

use std::{future::Future, pin::Pin};

use domain::{identity::HistorySessionId, revisions::RevisionKind};
use gpui::SharedString;

/// What one durable revision is: its kind and, for an `Edit`, its History
/// session (Decision 141).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitPoint {
    /// Why the revision exists.
    pub kind: RevisionKind,
    /// The History session. An `Edit` always carries `Some`; every standalone
    /// kind carries `None`.
    pub session: Option<HistorySessionId>,
}

/// A commit the port could not make durable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveFailure {
    /// A short, operator-readable reason.
    pub message: SharedString,
}

/// Persistence seam: every committed point is one durable revision.
///
/// The session never sees SQL or a connection. An adapter over `storage`
/// lives with the caller, because `ordo` takes no normal dependency on
/// `storage` while the Presenter runtime ships without SQLite.
pub trait RevisionPort<C>: 'static {
    /// Commit `content` as one revision. The adapter owns the
    /// expected-parent bookkeeping. The future is polled on the GPUI
    /// foreground executor and must not block it.
    fn commit(
        &self,
        content: C,
        point: CommitPoint,
    ) -> Pin<Box<dyn Future<Output = Result<(), SaveFailure>>>>;
}
