//! The operator and platform commands the Presentation Engine accepts.

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
    /// The output display disappeared.
    DisplayLost,
    /// A display appeared; `recognized` is the platform's identity verdict.
    DisplayReturned {
        /// Whether it is the same display the output used.
        recognized: bool,
    },
    /// The returned display went away again.
    DisplayGone,
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
