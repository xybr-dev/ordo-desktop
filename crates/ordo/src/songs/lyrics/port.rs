//! The Lyrics editor's [`RevisionPort`] over the one [`LibraryStore`].
//!
//! The port holds no database handle and no connection: every commit goes
//! through [`LibraryStore::save_arrangement_lyrics`], whose queue serializes it
//! with the other library writes and whose storage call runs on the worker
//! thread. A released store becomes a [`SaveFailure`], never a panic, and a
//! failed save retries through [`crate::edit_session::EditSession::retry_save`]
//! like any other port.

use std::{future::Future, pin::Pin};

use domain::{identity::ArrangementId, revisions::RevisionKind, song_library::ArrangementLyrics};
use gpui::{AsyncApp, WeakEntity};

use crate::{
    edit_session::{CommitPoint, RevisionPort, SaveFailure},
    songs::LibraryStore,
};

/// The text shown when the store entity has already been released.
const STORE_CLOSED: &str = "The parish store is not open.";

/// The text shown when a commit is not an `Edit` with a History session.
const NOT_AN_EDIT: &str = "The Lyrics editor writes Edit revisions with a History session.";

/// Persists [`ArrangementLyrics`] through the app's one [`LibraryStore`].
///
/// One port belongs to one Arrangement and one editor visit. The session ids
/// come from the [`crate::edit_session::EditSession`], so Lyrics and Sequence
/// work share one History session (MVP-PLAN 6.18).
pub struct LyricsPort {
    store: WeakEntity<LibraryStore>,
    arrangement: ArrangementId,
    cx: AsyncApp,
}

impl LyricsPort {
    /// A port for `arrangement` over `store`.
    pub fn new(store: WeakEntity<LibraryStore>, arrangement: ArrangementId, cx: AsyncApp) -> Self {
        Self {
            store,
            arrangement,
            cx,
        }
    }
}

impl RevisionPort<ArrangementLyrics> for LyricsPort {
    fn commit(
        &self,
        content: ArrangementLyrics,
        point: CommitPoint,
    ) -> Pin<Box<dyn Future<Output = Result<(), SaveFailure>>>> {
        if point.kind != RevisionKind::Edit {
            return Box::pin(async {
                Err(SaveFailure {
                    message: NOT_AN_EDIT.into(),
                })
            });
        }
        let Some(session) = point.session else {
            return Box::pin(async {
                Err(SaveFailure {
                    message: NOT_AN_EDIT.into(),
                })
            });
        };
        let mut cx = self.cx.clone();
        let store = self.store.clone();
        let arrangement = self.arrangement;
        // The store update happens when the future is polled, never inside
        // `commit` itself: `EditSession` calls `commit` while it holds its own
        // entity update, and borrowing the app again there would panic.
        Box::pin(async move {
            let started = store
                .update(&mut cx, |store, cx| {
                    store.save_arrangement_lyrics(arrangement, content, session, cx)
                })
                .map_err(|_| SaveFailure {
                    message: STORE_CLOSED.into(),
                })?;
            started
                .await
                .map(|_| ())
                .map_err(|message| SaveFailure { message })
        })
    }
}
