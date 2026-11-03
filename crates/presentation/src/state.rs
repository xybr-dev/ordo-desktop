//! Observable Presentation Engine state: what the operator and the
//! congregation can currently see.
//!
//! These are plain values. Every transition lives in [`crate::engine`], so a
//! view can read state without being able to invent a state the Engine never
//! reached.

use domain::presentation::FrameId;

/// A monotonic timestamp in milliseconds.
///
/// The Engine never reads a clock. The caller supplies the time of every
/// command, which keeps replay deterministic: the same snapshot and the same
/// (command, time) sequence always produce the same states.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Millis(pub u64);

/// The live-output state machine of MVP-PLAN section 8.1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveState {
    /// No output is open. The default after startup and after Disable.
    OutputDisabled,
    /// Output is open and black; Preview does not drive Program.
    EnabledBlack,
    /// Linked Live: navigation moves Preview and Program together.
    Live,
    /// Program is held; navigation moves Preview only.
    Frozen,
    /// Program is black on purpose; navigation moves Preview only.
    Blackout,
    /// The output display was lost. `display_back` is true once the same
    /// recognized display has returned; output still stays black until the
    /// operator acts (Decision 272).
    Disconnected {
        /// Whether the recognized display is available again.
        display_back: bool,
    },
}

/// What the congregation sees.
///
/// There is deliberately no variant for an error or placeholder frame, so
/// "Program is black or one complete valid frame" holds by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramContent {
    /// The intentional black output.
    Black,
    /// One complete canonical frame, shown for the Cue at flat index `cue`.
    Frame {
        /// The canonical frame the caller resolves and displays.
        frame: FrameId,
        /// Flat index of the Cue that put the frame on Program.
        cue: usize,
    },
}

/// The Engine's complete state after one command.
///
/// Invariant: while `live` is `OutputDisabled`, `EnabledBlack`, `Blackout`, or
/// `Disconnected`, `program` is [`ProgramContent::Black`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineState {
    pub(crate) live: LiveState,
    pub(crate) preview: usize,
    pub(crate) program: ProgramContent,
    pub(crate) unavailable_black: bool,
    pub(crate) revision: u64,
}

impl EngineState {
    /// The live-output state.
    pub fn live(&self) -> LiveState {
        self.live
    }

    /// Flat index of the Preview Cue.
    pub fn preview(&self) -> usize {
        self.preview
    }

    /// What Program shows.
    pub fn program(&self) -> ProgramContent {
        self.program
    }

    /// True when Program is black because linked Live landed on an
    /// unavailable Cue (status "LIVE · black · Cue unavailable", Decision 274).
    pub fn unavailable_black(&self) -> bool {
        self.unavailable_black
    }

    /// Counts state changes. It increments exactly once per
    /// [`crate::engine::Outcome::Changed`] and never otherwise.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}
