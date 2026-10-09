//! Pure Presenter Engine: deterministic Preview, Program, Freeze, and Blackout
//! over the Cues of one Mass item.
//!
//! The engine works on Cue indices only. It never touches GPUI views, storage,
//! the filesystem, or platform windows, so any command sequence is a pure
//! function of its input and can be replayed and tested exactly.

/// One operator action against the Presenter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenterCommand {
    /// Move Preview to the next Cue, stopping at the last one.
    Next,
    /// Move Preview to the previous Cue, stopping at the first one.
    Previous,
    /// Move Preview to a Cue index, clamped to the valid range.
    GoTo(usize),
    /// The `Space` action: Program becomes Preview. It also releases Freeze
    /// and Blackout, so the taken Cue is what the congregation sees.
    TakeLive,
    /// Hold Program while Preview navigates, or release that hold.
    ToggleFreeze,
    /// Force Program black while Preview navigates, or release that black.
    ToggleBlackout,
}

impl PresenterCommand {
    /// Whether this command only moves Preview. Navigation never reads or
    /// writes Program, Freeze, or Blackout.
    pub fn is_navigation(self) -> bool {
        matches!(self, Self::Next | Self::Previous | Self::GoTo(_))
    }
}

/// The complete Presenter state, returned by every
/// [`PresenterEngine::apply`] call.
///
/// `revision` counts state changes: it increases whenever any field other
/// than itself changes, and never on a command that leaves state as it was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresenterState {
    preview: usize,
    program: Option<usize>,
    frozen: bool,
    blackout: bool,
    revision: u64,
}

impl PresenterState {
    /// The Cue the operator is looking at. Always a valid index unless the
    /// deck is empty, in which case it is `0`.
    pub fn preview(&self) -> usize {
        self.preview
    }

    /// The Cue taken to Program, or `None` before the first take. Blackout
    /// does not clear it; use [`PresenterState::program_visible`] for what the
    /// congregation sees.
    pub fn program(&self) -> Option<usize> {
        self.program
    }

    /// Whether Program is held while Preview navigates.
    pub fn frozen(&self) -> bool {
        self.frozen
    }

    /// Whether Program is forced black.
    pub fn blackout(&self) -> bool {
        self.blackout
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The Cue the congregation sees: `None` while Blackout is active or
    /// before any Program has been taken.
    pub fn program_visible(&self) -> Option<usize> {
        if self.blackout { None } else { self.program }
    }
}

/// Owner of one Presenter's state over a deck of `cue_count` Cues.
///
/// Invariant: `preview` is always a valid Cue index (`0` for an empty deck),
/// and `program`, once set, always names a Cue that existed when it was taken.
#[derive(Debug)]
pub struct PresenterEngine {
    cue_count: usize,
    state: PresenterState,
}

impl PresenterEngine {
    /// Start with Preview on the first Cue, nothing on Program, and neither
    /// Freeze nor Blackout active.
    pub fn new(cue_count: usize) -> Self {
        Self {
            cue_count,
            state: PresenterState {
                preview: 0,
                program: None,
                frozen: false,
                blackout: false,
                revision: 0,
            },
        }
    }

    /// Apply `command` and return the complete resulting state.
    ///
    /// Every command is accepted; a command that cannot change anything (for
    /// example `TakeLive` on an empty deck, or `Next` on the last Cue) leaves
    /// the state and its revision untouched.
    pub fn apply(&mut self, command: PresenterCommand) -> &PresenterState {
        let before = self.state;
        let mut next = before;
        match command {
            PresenterCommand::Next | PresenterCommand::Previous | PresenterCommand::GoTo(_) => {
                next.preview = navigate(before.preview, command, self.cue_count);
            }
            PresenterCommand::TakeLive => {
                if self.cue_count > 0 {
                    next.program = Some(before.preview);
                    next.frozen = false;
                    next.blackout = false;
                }
            }
            PresenterCommand::ToggleFreeze => next.frozen = !before.frozen,
            PresenterCommand::ToggleBlackout => next.blackout = !before.blackout,
        }
        let changed = PresenterState {
            revision: before.revision,
            ..next
        } != before;
        if changed {
            // Saturating: a Presenter cannot reach u64::MAX state changes in
            // practice, and reusing a revision would be the only wrong outcome.
            next.revision = before.revision.saturating_add(1);
            self.state = next;
        }
        &self.state
    }

    /// The current state without applying a command.
    pub fn state(&self) -> &PresenterState {
        &self.state
    }
}

/// Clamp a Cue index into `[0, cue_count - 1]`, or `0` for an empty deck.
fn clamp_index(index: usize, cue_count: usize) -> usize {
    cue_count.checked_sub(1).map_or(0, |last| index.min(last))
}

/// Where a navigation command moves Preview. Shared by the engine and
/// [`coalesce`] so the two cannot disagree about where a run ends.
fn navigate(preview: usize, command: PresenterCommand, cue_count: usize) -> usize {
    let preview = clamp_index(preview, cue_count);
    let last = clamp_index(usize::MAX, cue_count);
    match command {
        PresenterCommand::Next => (preview + 1).min(last),
        PresenterCommand::Previous => preview.saturating_sub(1),
        PresenterCommand::GoTo(index) => clamp_index(index, cue_count),
        PresenterCommand::TakeLive
        | PresenterCommand::ToggleFreeze
        | PresenterCommand::ToggleBlackout => preview,
    }
}

/// Collapse each maximal run of navigation commands into one `GoTo` to the
/// run's final Preview index, keeping every other command in its original
/// order.
///
/// Why this is safe: navigation reads and writes only Preview, and the
/// intermediate Preview positions of a run are visible to nothing but the
/// per-command states that `apply` returns. The next barrier
/// (`TakeLive`, `ToggleFreeze`, `ToggleBlackout`) observes only the final
/// Preview, and so does the end state, so the coalesced sequence reaches the
/// same Preview, Program, Freeze, and Blackout as the raw one. Barriers are
/// never reordered or dropped, so the operator's final take and toggles are
/// never lost.
///
/// `revision` is the one field that differs: it counts state changes, and a
/// collapsed run performs fewer of them. Compare states without it.
pub fn coalesce(
    commands: &[PresenterCommand],
    cue_count: usize,
    start_preview: usize,
) -> Vec<PresenterCommand> {
    let mut out = Vec::with_capacity(commands.len());
    let mut preview = clamp_index(start_preview, cue_count);
    let mut in_run = false;
    for &command in commands {
        if command.is_navigation() {
            preview = navigate(preview, command, cue_count);
            in_run = true;
        } else {
            if in_run {
                out.push(PresenterCommand::GoTo(preview));
                in_run = false;
            }
            out.push(command);
        }
    }
    if in_run {
        out.push(PresenterCommand::GoTo(preview));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(engine: &mut PresenterEngine, commands: &[PresenterCommand]) -> PresenterState {
        for &command in commands {
            engine.apply(command);
        }
        *engine.state()
    }

    #[test]
    fn starts_on_first_cue_with_nothing_on_program() {
        let engine = PresenterEngine::new(3);
        let state = engine.state();
        assert_eq!(state.preview(), 0);
        assert_eq!(state.program(), None);
        assert_eq!(state.program_visible(), None);
        assert!(!state.frozen());
        assert!(!state.blackout());
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn navigation_clamps_to_the_deck_and_moves_only_preview() {
        let mut engine = PresenterEngine::new(3);
        assert_eq!(engine.apply(PresenterCommand::Previous).preview(), 0);
        assert_eq!(engine.apply(PresenterCommand::Next).preview(), 1);
        assert_eq!(engine.apply(PresenterCommand::Next).preview(), 2);
        assert_eq!(engine.apply(PresenterCommand::Next).preview(), 2);
        assert_eq!(engine.apply(PresenterCommand::GoTo(99)).preview(), 2);
        assert_eq!(engine.apply(PresenterCommand::GoTo(0)).preview(), 0);
        assert_eq!(engine.state().program(), None);
    }

    #[test]
    fn take_live_puts_preview_on_program() {
        let mut engine = PresenterEngine::new(3);
        engine.apply(PresenterCommand::GoTo(2));
        let state = engine.apply(PresenterCommand::TakeLive);
        assert_eq!(state.program(), Some(2));
        assert_eq!(state.program_visible(), Some(2));
    }

    #[test]
    fn freeze_holds_program_while_preview_navigates() {
        let mut engine = PresenterEngine::new(4);
        engine.apply(PresenterCommand::TakeLive);
        engine.apply(PresenterCommand::ToggleFreeze);
        let state = engine.apply(PresenterCommand::Next);
        assert!(state.frozen());
        assert_eq!(state.preview(), 1);
        assert_eq!(state.program(), Some(0));
    }

    #[test]
    fn take_live_while_frozen_releases_freeze_then_takes_live() {
        let mut engine = PresenterEngine::new(4);
        engine.apply(PresenterCommand::TakeLive);
        engine.apply(PresenterCommand::ToggleFreeze);
        engine.apply(PresenterCommand::GoTo(3));
        let state = engine.apply(PresenterCommand::TakeLive);
        assert!(!state.frozen());
        assert_eq!(state.program(), Some(3));
    }

    #[test]
    fn blackout_forces_program_black_while_preview_navigates() {
        let mut engine = PresenterEngine::new(4);
        engine.apply(PresenterCommand::TakeLive);
        engine.apply(PresenterCommand::ToggleBlackout);
        let state = engine.apply(PresenterCommand::Next);
        assert!(state.blackout());
        assert_eq!(state.program(), Some(0));
        assert_eq!(state.program_visible(), None);
        assert_eq!(state.preview(), 1);
    }

    #[test]
    fn take_live_clears_blackout_and_shows_the_taken_cue() {
        let mut engine = PresenterEngine::new(4);
        engine.apply(PresenterCommand::ToggleBlackout);
        engine.apply(PresenterCommand::Next);
        let state = engine.apply(PresenterCommand::TakeLive);
        assert!(!state.blackout());
        assert_eq!(state.program_visible(), Some(1));
    }

    #[test]
    fn revision_counts_only_state_changes() {
        let mut engine = PresenterEngine::new(2);
        assert_eq!(engine.apply(PresenterCommand::Previous).revision(), 0);
        assert_eq!(engine.apply(PresenterCommand::Next).revision(), 1);
        assert_eq!(engine.apply(PresenterCommand::Next).revision(), 1);
        assert_eq!(engine.apply(PresenterCommand::TakeLive).revision(), 2);
        assert_eq!(engine.apply(PresenterCommand::TakeLive).revision(), 2);
        assert_eq!(engine.apply(PresenterCommand::ToggleFreeze).revision(), 3);
    }

    #[test]
    fn empty_deck_ignores_navigation_and_take() {
        let mut engine = PresenterEngine::new(0);
        for command in [
            PresenterCommand::Next,
            PresenterCommand::Previous,
            PresenterCommand::GoTo(5),
            PresenterCommand::TakeLive,
        ] {
            let state = engine.apply(command);
            assert_eq!(state.preview(), 0);
            assert_eq!(state.program(), None);
            assert_eq!(state.revision(), 0);
        }
    }

    #[test]
    fn coalesce_collapses_navigation_runs_and_keeps_barriers_in_order() {
        use PresenterCommand::*;
        let raw = [
            Next,
            Next,
            TakeLive,
            Previous,
            GoTo(4),
            ToggleBlackout,
            Next,
        ];
        assert_eq!(
            coalesce(&raw, 5, 0),
            vec![GoTo(2), TakeLive, GoTo(4), ToggleBlackout, GoTo(4)]
        );
    }

    #[test]
    fn coalesce_clamps_the_run_end_like_the_engine() {
        use PresenterCommand::*;
        // Ten Nexts on a 3-Cue deck end on the last Cue, not on index 10.
        assert_eq!(coalesce(&[Next; 10], 3, 0), vec![GoTo(2)]);
        assert_eq!(coalesce(&[Previous], 3, 0), vec![GoTo(0)]);
        assert_eq!(coalesce(&[], 3, 0), Vec::new());
    }

    #[test]
    fn coalesced_run_reaches_the_same_state_as_the_raw_run() {
        use PresenterCommand::*;
        let raw = [
            Next,
            GoTo(7),
            Previous,
            Previous,
            ToggleFreeze,
            Next,
            TakeLive,
        ];
        let mut raw_engine = PresenterEngine::new(5);
        let mut coalesced_engine = PresenterEngine::new(5);
        let collapsed = coalesce(&raw, 5, 0);
        let a = run(&mut raw_engine, &raw);
        let b = run(&mut coalesced_engine, &collapsed);
        assert_eq!(a.preview(), b.preview());
        assert_eq!(a.program(), b.program());
        assert_eq!(a.frozen(), b.frozen());
        assert_eq!(a.blackout(), b.blackout());
    }
}
