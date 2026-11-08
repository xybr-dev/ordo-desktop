//! Turns Engine state into recovery points for one Presentation session.
//!
//! The recorder is pure. The caller supplies the time of each point (as a
//! closure, asked for only when a point is emitted) and the display evidence; the recorder owns the sequence number and the last valid
//! Program cue, which the Engine forgets once Program goes black. Points are
//! emitted only when the recorded content changes, so ticks and fades never
//! write.

use crate::engine::Engine;
use crate::journal::{
    DisplayRecord, LiveIntent, OutputEvidence, RecoveryPoint, SessionId, WrittenAt,
};
use crate::snapshot_reader::LoadedSnapshot;
use crate::state::{LiveState, ProgramContent};
use domain::presentation::{ContentHash, CueId, SnapshotId};

/// The identity every point of one session carries. The shell builds it from
/// the directory it loaded, because `LoadedSnapshot` does not know that path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionIdentity {
    pub session_id: SessionId,
    pub snapshot_id: SnapshotId,
    pub manifest_hash: ContentHash,
    /// Directory the snapshot was loaded from. It must not be empty:
    /// [`crate::journal::decode`] rejects an empty `source_dir`, so such a
    /// point could be written but never reopened.
    pub source_dir: String,
}

impl SessionIdentity {
    /// Identity of a session over `snapshot`, loaded from `source_dir`.
    pub fn of(
        session_id: SessionId,
        snapshot: &LoadedSnapshot,
        source_dir: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            snapshot_id: snapshot.snapshot().id(),
            manifest_hash: snapshot.manifest_hash(),
            source_dir: source_dir.into(),
        }
    }
}

/// The recorded content: everything a point holds except the identity, the
/// sequence number, the time and the `ended` flag. Two points with equal
/// content describe the same session state.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Content {
    preview: CueId,
    program: Option<CueId>,
    state: LiveIntent,
    output: OutputEvidence,
}

/// Turns Engine state into recovery points for one session. Pure: the caller
/// supplies the time; the recorder owns `seq` and the last valid Program.
#[derive(Debug)]
pub struct JournalRecorder {
    identity: SessionIdentity,
    /// Sequence number of the last emitted point; 0 before the first.
    seq: u64,
    /// The cue of the last `ProgramContent::Frame` this session showed.
    last_valid_program: Option<CueId>,
    /// Content of the last emitted point, for change detection.
    last: Option<Content>,
    /// Set by [`JournalRecorder::end`]: a session that ended cleanly never
    /// records again, so its journal cannot turn back into a live one.
    ended: bool,
}

impl JournalRecorder {
    /// A recorder for one session that has emitted no point yet.
    pub fn new(identity: SessionIdentity) -> Self {
        Self {
            identity,
            seq: 0,
            last_valid_program: None,
            last: None,
            ended: false,
        }
    }

    /// A point when the recorded content changed since the last emitted point,
    /// else `None`. `display` is the assigned display's evidence, if any.
    /// After [`JournalRecorder::end`] it always returns `None`.
    pub fn observe(
        &mut self,
        engine: &Engine,
        display: Option<DisplayRecord>,
        written_at: impl FnOnce() -> WrittenAt,
    ) -> Option<RecoveryPoint> {
        if self.ended {
            return None;
        }
        let content = self.content(engine, display);
        if self.last.as_ref() == Some(&content) {
            return None;
        }
        Some(self.emit(content, written_at(), false))
    }

    /// The clean-end point: the current content with `ended: true`, emitted
    /// even when nothing changed. A session ends once, so every later call
    /// returns `None`.
    pub fn end(
        &mut self,
        engine: &Engine,
        display: Option<DisplayRecord>,
        written_at: impl FnOnce() -> WrittenAt,
    ) -> Option<RecoveryPoint> {
        if self.ended {
            return None;
        }
        let content = self.content(engine, display);
        self.ended = true;
        Some(self.emit(content, written_at(), true))
    }

    /// The content of `engine` now. Remembers the Program cue whenever Program
    /// shows a frame, so a later black Program still names it.
    ///
    /// A recovered session starts with Program black, so its recorder is
    /// seeded once from [`crate::state::RecoveredContext::program`]: a second
    /// crash before the first `Take` still names what Program showed before
    /// the interruption. A frame that later reaches Program replaces it.
    ///
    /// While that context exists, `state` is its journaled interrupted intent,
    /// not the current live state: the session has not acted on the recovered
    /// label yet, so a second crash keeps the original `frozen`/`blackout`
    /// wording. `output` is still the real evidence. Once the context clears,
    /// the live state is recorded as before.
    fn content(&mut self, engine: &Engine, display: Option<DisplayRecord>) -> Content {
        let state = engine.state();
        if self.last_valid_program.is_none()
            && let Some(program) = state.recovered().and_then(|context| context.program())
        {
            self.last_valid_program = Some(cue_id_at(engine, program));
        }
        if let ProgramContent::Frame { cue, .. } = state.program() {
            self.last_valid_program = Some(cue_id_at(engine, cue));
        }
        let live = state.live();
        let recorded_state = state
            .recovered()
            .map_or_else(|| LiveIntent::of(live), |context| context.interrupted());
        Content {
            preview: cue_id_at(engine, state.preview()),
            program: self.last_valid_program,
            state: recorded_state,
            output: OutputEvidence {
                enabled: live != LiveState::OutputDisabled,
                display,
            },
        }
    }

    /// Stamps `content` with the next sequence number and makes it the
    /// change-detection baseline.
    fn emit(&mut self, content: Content, written_at: WrittenAt, ended: bool) -> RecoveryPoint {
        self.seq += 1;
        let point = RecoveryPoint {
            session_id: self.identity.session_id.clone(),
            seq: self.seq,
            written_at,
            snapshot_id: self.identity.snapshot_id,
            manifest_hash: self.identity.manifest_hash,
            source_dir: self.identity.source_dir.clone(),
            preview: content.preview,
            program: content.program,
            state: content.state,
            output: content.output.clone(),
            ended,
        };
        self.last = Some(content);
        point
    }
}

/// The CueId at flat index `flat`. The Engine stores only indexes inside the
/// snapshot (its own invariant, the same one `Engine::preview_cue` relies on),
/// so the lookup cannot fail.
fn cue_id_at(engine: &Engine, flat: usize) -> CueId {
    engine.snapshot().positions()[flat].cue_id
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
