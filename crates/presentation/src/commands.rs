//! The operator and platform commands the Presentation Engine accepts.

use domain::presentation::FrameId;
use rendering::UnavailableReason;

/// One input to [`crate::engine::Engine::apply`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    /// Right / Down: moves Preview one Cue forward, clamped at the last Cue (no wrap).
    Next,
    /// Left / Up: moves Preview one Cue back, clamped at the first Cue (no wrap).
    Previous,
    /// Page Down: the first Cue of the next Mass item; no-op in the last item.
    NextItem,
    /// Page Up: the first Cue of the previous Mass item, from anywhere inside
    /// the current item (including its first Cue). No-op in the first item.
    PreviousItem,
    /// Home: the first Cue.
    First,
    /// End: the last Cue.
    Last,
    /// Moves Preview to a flat Cue index, clamped to the last Cue.
    GoTo(usize),
    /// Space: take Preview live.
    Take,
    /// F: freeze Program, or release the freeze exactly as `Take` does.
    ToggleFreeze,
    /// B: black out Program, or release it exactly as `Take` does.
    ToggleBlackout,
    /// Open the output, black.
    EnableOutput,
    /// Close the output.
    DisableOutput,
    /// The output display disappeared. In `Disconnected` it also means a
    /// returned display went away again: `display_back` becomes false (one
    /// event per physical fact).
    DisplayLost,
    /// A display appeared; `recognized` is the platform's identity verdict.
    DisplayReturned {
        /// Whether it is the same display the output used.
        recognized: bool,
    },
    /// Start auto-play on the Preview Cue's item, or stop whatever auto-play is
    /// active.
    ToggleAutoPlay,
    /// Loop the Preview Cue's whole section (menu). While an item run is
    /// active it widens that run to the section; on the same section it is a
    /// no-op.
    LoopSection,
    /// Stop auto-play (menu); a no-op when none is active.
    StopAutoPlay,
    /// Recovery or session restore: stops auto-play and changes nothing else.
    /// Restored sessions never resume auto-play silently. Unrelated to
    /// [`crate::engine::Engine::recover`], which reopens an interrupted
    /// session from the journal.
    Recover,
    /// A frame failed at runtime (render, asset, or decode failure reported by
    /// the frame scheduler). Every Cue projecting it becomes unavailable.
    /// Idempotent: the first reason for a frame is kept; a frame no Project
    /// Cue uses changes nothing. Nothing from a failed render reaches Program
    /// (Decision 274): a frame already on Program was never drawn, so linked
    /// Live goes black with the unavailable flag and Frozen goes black.
    FrameUnavailable {
        /// The failed frame.
        frame: FrameId,
        /// Why it cannot be produced.
        reason: UnavailableReason,
    },
}

impl Command {
    /// True for the commands that only move Preview (and, in linked Live,
    /// Program with it).
    pub fn is_navigation(&self) -> bool {
        matches!(
            self,
            Self::Next
                | Self::Previous
                | Self::NextItem
                | Self::PreviousItem
                | Self::First
                | Self::Last
                | Self::GoTo(_)
        )
    }
}
