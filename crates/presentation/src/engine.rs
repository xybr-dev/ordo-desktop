//! The pure, snapshot-driven Presentation Engine.
//!
//! The Engine owns every live-state transition (MVP-PLAN section 8.1) so UI
//! and tests drive one Interface. It works on Cue positions, never pixels, and
//! never reads a clock: time arrives as [`Millis`] with each command.
//!
//! Decisions applied: 272 (one-step resume on the recognized display), 273
//! (Freeze and Blackout toggle off exactly as Space does), 274 (an unavailable
//! Cue makes linked Program black instead of freezing).

use std::fmt;
use std::sync::Arc;

use domain::presentation::{Cue, CueId, PresentationSnapshot, ProgramEffect};

use crate::commands::Command;
use crate::state::{EngineState, LiveState, Millis, ProgramContent};

/// Whether a command changed the Engine state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// State changed; `revision` increased by exactly one.
    Changed,
    /// The command was valid and a no-op; `revision` did not move.
    Unchanged,
}

/// Why a command was refused. A refusal changes nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The Preview Cue is unavailable and cannot be taken live.
    UnavailablePreview {
        /// The unavailable Preview Cue.
        cue: CueId,
    },
    /// Output is disabled; enable it first.
    OutputDisabled,
    /// The display is disconnected and the recognized display is not back.
    DisplayNotBack,
    /// Freeze is meaningless while Program is blacked out.
    FreezeDuringBlackout,
    /// The command needs output that is live (or at least open).
    NotLive,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnavailablePreview { cue } => {
                write!(f, "Cue {cue} is unavailable and cannot go live")
            }
            Self::OutputDisabled => f.write_str("output is disabled"),
            Self::DisplayNotBack => f.write_str("the output display is not back"),
            Self::FreezeDuringBlackout => f.write_str("cannot freeze during Blackout"),
            Self::NotLive => f.write_str("output is not live"),
        }
    }
}

impl std::error::Error for Refusal {}

/// The Presentation Engine over one immutable snapshot.
#[derive(Clone, Debug)]
pub struct Engine {
    snapshot: Arc<PresentationSnapshot>,
    state: EngineState,
    /// Time of the last applied command. WP2 reads it for fades and auto-play.
    last_now: Millis,
}

impl Engine {
    /// A new Engine: output disabled, Preview on the first Cue, Program black.
    pub fn new(snapshot: Arc<PresentationSnapshot>) -> Self {
        Self {
            snapshot,
            state: EngineState {
                live: LiveState::OutputDisabled,
                preview: 0,
                program: ProgramContent::Black,
                unavailable_black: false,
                revision: 0,
            },
            last_now: Millis(0),
        }
    }

    /// The current state.
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// The snapshot being presented.
    pub fn snapshot(&self) -> &PresentationSnapshot {
        &self.snapshot
    }

    /// The Preview Cue.
    pub fn preview_cue(&self) -> &Cue {
        // A snapshot always has a Cue (sections, items, and Cues are non-empty)
        // and Preview is clamped to its range, so this lookup cannot fail.
        self.snapshot
            .cue(self.state.preview)
            .expect("preview index is always inside the snapshot")
    }

    /// The operator-only context of the Preview Cue. Never part of Program.
    pub fn preview_context(&self) -> Option<&str> {
        self.preview_cue().private_context()
    }

    /// Time of the last applied command.
    pub fn last_now(&self) -> Millis {
        self.last_now
    }

    /// Apply one command at time `now`.
    ///
    /// A refused command changes nothing, including `last_now`.
    pub fn apply(&mut self, command: Command, now: Millis) -> Result<Outcome, Refusal> {
        let mut next = self.state.clone();
        self.transition(&mut next, command)?;
        self.last_now = now;
        if next == self.state {
            return Ok(Outcome::Unchanged);
        }
        next.revision = self.state.revision + 1;
        self.state = next;
        Ok(Outcome::Changed)
    }

    fn last_index(&self) -> usize {
        self.snapshot.cue_count() - 1
    }

    /// Where a navigation command lands from the current Preview.
    fn navigation_target(&self, command: Command) -> usize {
        let preview = self.state.preview;
        let last = self.last_index();
        let item = self.snapshot.positions()[preview].item_flat;
        match command {
            Command::Next => (preview + 1).min(last),
            Command::Previous => preview.saturating_sub(1),
            Command::NextItem => self.snapshot.item_start(item + 1).unwrap_or(preview),
            // Always the previous item's first Cue, even from mid-item: Page Up
            // leaves the current item. In the first item there is none, so stay.
            Command::PreviousItem => item
                .checked_sub(1)
                .and_then(|previous| self.snapshot.item_start(previous))
                .unwrap_or(preview),
            Command::First => 0,
            Command::Last => last,
            Command::GoTo(index) => index.min(last),
            _ => preview,
        }
    }

    /// Put the linked effect of Cue `cue` onto `next`, with `keep` as the
    /// Program a `Wait` leaves alone.
    fn apply_effect(&self, next: &mut EngineState, cue: usize, keep: ProgramContent) {
        let effect = self
            .snapshot
            .cue(cue)
            .map_or(ProgramEffect::Black, Cue::program_effect);
        match effect {
            ProgramEffect::Frame(frame) => {
                next.program = ProgramContent::Frame { frame, cue };
                next.unavailable_black = false;
            }
            ProgramEffect::Black => {
                next.program = ProgramContent::Black;
                next.unavailable_black = false;
            }
            ProgramEffect::Unchanged => next.program = keep,
            ProgramEffect::NotPresentable => {
                next.program = ProgramContent::Black;
                next.unavailable_black = true;
            }
        }
    }

    fn go_black(next: &mut EngineState, live: LiveState) {
        next.live = live;
        next.program = ProgramContent::Black;
        next.unavailable_black = false;
    }

    /// Space semantics; also how Freeze and Blackout release.
    fn take(&self, next: &mut EngineState) -> Result<(), Refusal> {
        let preview = next.preview;
        let cue = self.preview_cue();
        if !cue.is_presentable() {
            return Err(Refusal::UnavailablePreview { cue: cue.id() });
        }
        let keep = match next.live {
            LiveState::OutputDisabled => return Err(Refusal::OutputDisabled),
            LiveState::Disconnected {
                display_back: false,
            } => return Err(Refusal::DisplayNotBack),
            // Nothing was held on a recreated or blacked output.
            LiveState::Disconnected { display_back: true }
            | LiveState::EnabledBlack
            | LiveState::Blackout => ProgramContent::Black,
            LiveState::Live | LiveState::Frozen => next.program,
        };
        next.live = LiveState::Live;
        self.apply_effect(next, preview, keep);
        Ok(())
    }

    fn transition(&self, next: &mut EngineState, command: Command) -> Result<(), Refusal> {
        match command {
            Command::Next
            | Command::Previous
            | Command::NextItem
            | Command::PreviousItem
            | Command::First
            | Command::Last
            | Command::GoTo(_) => {
                next.preview = self.navigation_target(command);
                if next.live == LiveState::Live {
                    let (preview, keep) = (next.preview, next.program);
                    self.apply_effect(next, preview, keep);
                }
            }
            Command::Take => self.take(next)?,
            Command::ToggleFreeze => match next.live {
                LiveState::Live => next.live = LiveState::Frozen,
                LiveState::Frozen => self.take(next)?,
                LiveState::Blackout => return Err(Refusal::FreezeDuringBlackout),
                _ => return Err(Refusal::NotLive),
            },
            Command::ToggleBlackout => match next.live {
                LiveState::Live | LiveState::Frozen | LiveState::EnabledBlack => {
                    Self::go_black(next, LiveState::Blackout);
                }
                LiveState::Blackout => self.take(next)?,
                _ => return Err(Refusal::NotLive),
            },
            Command::EnableOutput => match next.live {
                LiveState::OutputDisabled | LiveState::Disconnected { display_back: true } => {
                    Self::go_black(next, LiveState::EnabledBlack);
                }
                LiveState::Disconnected {
                    display_back: false,
                } => return Err(Refusal::DisplayNotBack),
                _ => {}
            },
            Command::DisableOutput => Self::go_black(next, LiveState::OutputDisabled),
            Command::DisplayLost => match next.live {
                LiveState::OutputDisabled | LiveState::Disconnected { .. } => {}
                _ => Self::go_black(
                    next,
                    LiveState::Disconnected {
                        display_back: false,
                    },
                ),
            },
            Command::DisplayReturned { recognized } => {
                if let LiveState::Disconnected { .. } = next.live {
                    next.live = LiveState::Disconnected {
                        display_back: recognized,
                    };
                }
            }
            Command::DisplayGone => {
                if let LiveState::Disconnected { .. } = next.live {
                    next.live = LiveState::Disconnected {
                        display_back: false,
                    };
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::presentation::{
        CueBehavior, FrameId, MassItem, MassItemKind, MassSection, SnapshotId, ThemeId, ThemeKind,
        ThemeRef,
    };

    fn uuid(n: u16) -> String {
        format!("00000000-0000-4000-8000-{n:012x}")
    }

    fn frame(n: u16) -> FrameId {
        uuid(0x500 + n).parse().unwrap()
    }

    fn cue(n: u16, context: Option<&str>, behavior: CueBehavior) -> Cue {
        Cue::new(
            uuid(0x300 + n).parse().unwrap(),
            format!("Cue {n}"),
            context.map(str::to_owned),
            behavior,
        )
        .unwrap()
    }

    fn project(n: u16) -> Cue {
        cue(
            n,
            None,
            CueBehavior::Project {
                frame: frame(n),
                duration_seconds: None,
            },
        )
    }

    fn item(n: u16, kind: MassItemKind, cues: Vec<Cue>) -> MassItem {
        MassItem::new(
            uuid(0x200 + n).parse().unwrap(),
            format!("Item {n}"),
            kind,
            None,
            None,
            cues,
        )
        .unwrap()
    }

    /// Flat layout: item 0 = 0 project, 1 project, 2 unavailable;
    /// item 1 = 3 project, 4 trigger, 5 wait, 6 hold;
    /// item 2 = 7 project, 8 project.
    fn engine() -> Engine {
        let unavailable = cue(
            2,
            None,
            CueBehavior::Unavailable {
                reason: "no font".to_owned(),
            },
        );
        let section = MassSection::new(
            uuid(0x100).parse().unwrap(),
            "Section".to_owned(),
            vec![
                item(
                    0,
                    MassItemKind::Song,
                    vec![project(0), project(1), unavailable],
                ),
                item(
                    1,
                    MassItemKind::Reading,
                    vec![
                        project(3),
                        cue(4, Some("Wait for amen"), CueBehavior::OperatorTrigger),
                        cue(5, Some("Priest speaks"), CueBehavior::Wait),
                        cue(6, None, CueBehavior::Hold),
                    ],
                ),
                item(
                    2,
                    MassItemKind::GeneralPresentation,
                    vec![project(7), project(8)],
                ),
            ],
        )
        .unwrap();
        let theme: ThemeId = uuid(0x400).parse().unwrap();
        let snapshot = PresentationSnapshot::new(
            uuid(1).parse::<SnapshotId>().unwrap(),
            "Test".to_owned(),
            theme,
            vec![ThemeRef {
                id: theme,
                name: "T".to_owned(),
                kind: ThemeKind::Fallback,
            }],
            vec![section],
        )
        .unwrap();
        Engine::new(Arc::new(snapshot))
    }

    fn run(e: &mut Engine, c: Command) -> Result<Outcome, Refusal> {
        e.apply(c, Millis(0))
    }

    fn shows(e: &Engine, cue: u16) -> bool {
        e.state().program()
            == ProgramContent::Frame {
                frame: frame(cue),
                cue: usize::from(cue),
            }
    }

    fn enabled() -> Engine {
        let mut e = engine();
        run(&mut e, Command::EnableOutput).unwrap();
        e
    }

    fn live_at(index: usize) -> Engine {
        let mut e = enabled();
        run(&mut e, Command::GoTo(index)).unwrap();
        run(&mut e, Command::Take).unwrap();
        e
    }

    fn setup(commands: &[Command]) -> Engine {
        let mut e = engine();
        for c in commands {
            run(&mut e, *c).unwrap();
        }
        e
    }

    #[test]
    fn new_engine_is_disabled_black_at_first_cue() {
        let e = engine();
        assert_eq!(e.state().live(), LiveState::OutputDisabled);
        assert_eq!(e.state().preview(), 0);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
        assert_eq!(e.state().revision(), 0);
    }

    #[test]
    fn navigation_clamps_without_wrapping() {
        let mut e = engine();
        assert_eq!(run(&mut e, Command::Previous), Ok(Outcome::Unchanged));
        run(&mut e, Command::Last).unwrap();
        assert_eq!(e.state().preview(), 8);
        assert_eq!(run(&mut e, Command::Next), Ok(Outcome::Unchanged));
        assert_eq!(run(&mut e, Command::GoTo(99)), Ok(Outcome::Unchanged));
        assert_eq!(e.state().preview(), 8);
        run(&mut e, Command::GoTo(3)).unwrap();
        assert_eq!(e.state().preview(), 3);
        run(&mut e, Command::Previous).unwrap();
        assert_eq!(e.state().preview(), 2);
        run(&mut e, Command::First).unwrap();
        assert_eq!(e.state().preview(), 0);
    }

    #[test]
    fn page_navigation_moves_between_item_starts() {
        let mut e = engine();
        run(&mut e, Command::NextItem).unwrap();
        assert_eq!(e.state().preview(), 3);
        run(&mut e, Command::Next).unwrap();
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().preview(), 5);
        // From mid-item, Page Up goes to the previous item's first Cue.
        run(&mut e, Command::PreviousItem).unwrap();
        assert_eq!(e.state().preview(), 0);
        // First item: nowhere to go.
        run(&mut e, Command::Next).unwrap();
        assert_eq!(run(&mut e, Command::PreviousItem), Ok(Outcome::Unchanged));
        assert_eq!(e.state().preview(), 1);
        run(&mut e, Command::GoTo(8)).unwrap();
        assert_eq!(run(&mut e, Command::NextItem), Ok(Outcome::Unchanged));
        assert_eq!(e.state().preview(), 8);
        run(&mut e, Command::PreviousItem).unwrap();
        assert_eq!(e.state().preview(), 3);
    }

    #[test]
    fn is_navigation_classifies_commands() {
        assert!(Command::Next.is_navigation());
        assert!(Command::GoTo(3).is_navigation());
        assert!(!Command::Take.is_navigation());
        assert!(!Command::DisplayLost.is_navigation());
    }

    #[test]
    fn navigation_outside_live_moves_preview_only() {
        let setups: [&[Command]; 5] = [
            &[],
            &[Command::EnableOutput],
            &[Command::EnableOutput, Command::Take, Command::ToggleFreeze],
            &[
                Command::EnableOutput,
                Command::Take,
                Command::ToggleBlackout,
            ],
            &[Command::EnableOutput, Command::DisplayLost],
        ];
        for commands in setups {
            let mut e = setup(commands);
            let before = e.state().program();
            run(&mut e, Command::Next).unwrap();
            assert_eq!(e.state().preview(), 1);
            assert_eq!(e.state().program(), before);
        }
    }

    #[test]
    fn linked_navigation_follows_cue_effects() {
        let mut e = live_at(0);
        assert!(shows(&e, 0));
        run(&mut e, Command::Next).unwrap();
        assert!(shows(&e, 1));
        // Unavailable: black plus flag, still Live.
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(e.state().unavailable_black());
        // The next projectable Cue goes live again and clears the flag.
        run(&mut e, Command::Next).unwrap();
        assert!(shows(&e, 3));
        assert!(!e.state().unavailable_black());
        // Trigger: black, no flag.
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
        // Wait leaves Program as it was, then Hold is black.
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
    }

    #[test]
    fn wait_keeps_program_and_unavailable_flag() {
        let mut e = live_at(3);
        run(&mut e, Command::GoTo(5)).unwrap();
        assert!(shows(&e, 3), "Wait leaves the frame on Program");
        run(&mut e, Command::GoTo(2)).unwrap();
        run(&mut e, Command::GoTo(5)).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(e.state().unavailable_black());
    }

    #[test]
    fn take_refused_on_unavailable_preview_in_any_state() {
        let setups: [&[Command]; 4] = [
            &[],
            &[Command::EnableOutput],
            &[Command::EnableOutput, Command::Take, Command::ToggleFreeze],
            &[Command::EnableOutput, Command::ToggleBlackout],
        ];
        for commands in setups {
            let mut e = setup(commands);
            run(&mut e, Command::GoTo(2)).unwrap();
            let before = e.state().clone();
            let cue = e.preview_cue().id();
            assert_eq!(
                run(&mut e, Command::Take),
                Err(Refusal::UnavailablePreview { cue })
            );
            assert_eq!(e.state(), &before);
        }
    }

    #[test]
    fn take_from_each_state() {
        let mut e = engine();
        assert_eq!(run(&mut e, Command::Take), Err(Refusal::OutputDisabled));
        run(&mut e, Command::EnableOutput).unwrap();
        assert_eq!(run(&mut e, Command::Take), Ok(Outcome::Changed));
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 0));
        assert_eq!(run(&mut e, Command::Take), Ok(Outcome::Unchanged));
        // Blackout -> Live with Preview's effect.
        run(&mut e, Command::ToggleBlackout).unwrap();
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::Take).unwrap();
        assert!(shows(&e, 1));
        // Frozen -> Live with Preview's effect.
        run(&mut e, Command::ToggleFreeze).unwrap();
        run(&mut e, Command::GoTo(3)).unwrap();
        assert!(shows(&e, 1));
        run(&mut e, Command::Take).unwrap();
        assert!(shows(&e, 3));
        // Disconnected, display not back.
        run(&mut e, Command::DisplayLost).unwrap();
        assert_eq!(run(&mut e, Command::Take), Err(Refusal::DisplayNotBack));
    }

    #[test]
    fn take_with_wait_preview() {
        // Frozen keeps the frozen frame.
        let mut e = live_at(1);
        run(&mut e, Command::ToggleFreeze).unwrap();
        run(&mut e, Command::GoTo(5)).unwrap();
        run(&mut e, Command::Take).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 1));
        // EnabledBlack has nothing to keep.
        let mut e = enabled();
        run(&mut e, Command::GoTo(5)).unwrap();
        run(&mut e, Command::Take).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert_eq!(e.state().program(), ProgramContent::Black);
    }

    #[test]
    fn freeze_holds_program_and_releases_like_take() {
        let mut e = live_at(0);
        run(&mut e, Command::ToggleFreeze).unwrap();
        assert_eq!(e.state().live(), LiveState::Frozen);
        assert!(shows(&e, 0));
        run(&mut e, Command::Next).unwrap();
        assert!(shows(&e, 0));
        run(&mut e, Command::ToggleFreeze).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 1));
    }

    #[test]
    fn freeze_release_refused_on_unavailable_preview_and_stays_frozen() {
        let mut e = live_at(1);
        run(&mut e, Command::ToggleFreeze).unwrap();
        run(&mut e, Command::Next).unwrap();
        let before = e.state().clone();
        assert!(matches!(
            run(&mut e, Command::ToggleFreeze),
            Err(Refusal::UnavailablePreview { .. })
        ));
        assert_eq!(e.state(), &before);
        assert_eq!(e.state().live(), LiveState::Frozen);
    }

    #[test]
    fn blackout_blacks_and_releases_like_take() {
        let mut e = live_at(0);
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Blackout);
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 1));
    }

    #[test]
    fn blackout_release_refused_on_unavailable_preview_and_stays_blackout() {
        let mut e = live_at(1);
        run(&mut e, Command::ToggleBlackout).unwrap();
        run(&mut e, Command::Next).unwrap();
        let before = e.state().clone();
        assert!(matches!(
            run(&mut e, Command::ToggleBlackout),
            Err(Refusal::UnavailablePreview { .. })
        ));
        assert_eq!(e.state(), &before);
        assert_eq!(e.state().live(), LiveState::Blackout);
    }

    #[test]
    fn blackout_from_frozen_and_enabled_black() {
        let mut e = live_at(0);
        run(&mut e, Command::ToggleFreeze).unwrap();
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Blackout);
        let mut e = enabled();
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Blackout);
    }

    #[test]
    fn freeze_refusals() {
        let mut e = live_at(0);
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(
            run(&mut e, Command::ToggleFreeze),
            Err(Refusal::FreezeDuringBlackout)
        );
        assert_eq!(e.state().live(), LiveState::Blackout);

        let mut e = engine();
        assert_eq!(run(&mut e, Command::ToggleFreeze), Err(Refusal::NotLive));
        run(&mut e, Command::EnableOutput).unwrap();
        assert_eq!(run(&mut e, Command::ToggleFreeze), Err(Refusal::NotLive));
        run(&mut e, Command::DisplayLost).unwrap();
        assert_eq!(run(&mut e, Command::ToggleFreeze), Err(Refusal::NotLive));
    }

    #[test]
    fn blackout_not_live_refusals() {
        let mut e = engine();
        assert_eq!(run(&mut e, Command::ToggleBlackout), Err(Refusal::NotLive));
        run(&mut e, Command::EnableOutput).unwrap();
        run(&mut e, Command::DisplayLost).unwrap();
        assert_eq!(run(&mut e, Command::ToggleBlackout), Err(Refusal::NotLive));
    }

    #[test]
    fn repeated_blackout_is_blackout_then_take() {
        let mut e = live_at(0);
        run(&mut e, Command::ToggleBlackout).unwrap();
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 0));
    }

    #[test]
    fn output_lifecycle() {
        let mut e = engine();
        assert_eq!(run(&mut e, Command::DisableOutput), Ok(Outcome::Unchanged));
        assert_eq!(run(&mut e, Command::EnableOutput), Ok(Outcome::Changed));
        assert_eq!(e.state().live(), LiveState::EnabledBlack);
        assert_eq!(run(&mut e, Command::EnableOutput), Ok(Outcome::Unchanged));
        run(&mut e, Command::Take).unwrap();
        assert_eq!(run(&mut e, Command::EnableOutput), Ok(Outcome::Unchanged));
        assert!(shows(&e, 0));
        run(&mut e, Command::DisableOutput).unwrap();
        assert_eq!(e.state().live(), LiveState::OutputDisabled);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert_eq!(e.state().preview(), 0);
    }

    #[test]
    fn disable_from_blackout_and_disconnected() {
        let mut e = live_at(0);
        run(&mut e, Command::ToggleBlackout).unwrap();
        run(&mut e, Command::DisableOutput).unwrap();
        assert_eq!(e.state().live(), LiveState::OutputDisabled);
        let mut e = live_at(0);
        run(&mut e, Command::DisplayLost).unwrap();
        run(&mut e, Command::DisableOutput).unwrap();
        assert_eq!(e.state().live(), LiveState::OutputDisabled);
    }

    #[test]
    fn display_lost_blacks_every_enabled_state() {
        let setups: [&[Command]; 4] = [
            &[Command::EnableOutput],
            &[Command::EnableOutput, Command::Take],
            &[Command::EnableOutput, Command::Take, Command::ToggleFreeze],
            &[
                Command::EnableOutput,
                Command::Take,
                Command::ToggleBlackout,
            ],
        ];
        for commands in setups {
            let mut e = setup(commands);
            assert_eq!(run(&mut e, Command::DisplayLost), Ok(Outcome::Changed));
            assert_eq!(
                e.state().live(),
                LiveState::Disconnected {
                    display_back: false
                }
            );
            assert_eq!(e.state().program(), ProgramContent::Black);
            assert_eq!(run(&mut e, Command::DisplayLost), Ok(Outcome::Unchanged));
        }
        let mut e = engine();
        assert_eq!(run(&mut e, Command::DisplayLost), Ok(Outcome::Unchanged));
        assert_eq!(e.state().live(), LiveState::OutputDisabled);
    }

    #[test]
    fn reconnection_alone_never_resumes() {
        let mut e = live_at(0);
        run(&mut e, Command::DisplayLost).unwrap();
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        assert_eq!(
            e.state().live(),
            LiveState::Disconnected { display_back: true }
        );
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::DisplayGone).unwrap();
        assert_eq!(
            e.state().live(),
            LiveState::Disconnected {
                display_back: false
            }
        );
        // An unrecognized display is not "back".
        assert_eq!(
            run(&mut e, Command::DisplayReturned { recognized: false }),
            Ok(Outcome::Unchanged)
        );
        assert_eq!(
            run(&mut e, Command::EnableOutput),
            Err(Refusal::DisplayNotBack)
        );
    }

    #[test]
    fn display_events_outside_disconnected_are_unchanged() {
        let mut e = live_at(0);
        assert_eq!(
            run(&mut e, Command::DisplayReturned { recognized: true }),
            Ok(Outcome::Unchanged)
        );
        assert_eq!(run(&mut e, Command::DisplayGone), Ok(Outcome::Unchanged));
        assert!(shows(&e, 0));
    }

    #[test]
    fn space_resumes_in_one_step_when_display_is_back() {
        let mut e = live_at(1);
        run(&mut e, Command::DisplayLost).unwrap();
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        run(&mut e, Command::Take).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 1));
    }

    #[test]
    fn space_resume_with_wait_preview_is_black() {
        let mut e = live_at(1);
        run(&mut e, Command::DisplayLost).unwrap();
        run(&mut e, Command::GoTo(5)).unwrap();
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        run(&mut e, Command::Take).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert_eq!(e.state().program(), ProgramContent::Black);
    }

    #[test]
    fn enable_with_display_back_opens_black() {
        let mut e = live_at(0);
        run(&mut e, Command::DisplayLost).unwrap();
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        run(&mut e, Command::EnableOutput).unwrap();
        assert_eq!(e.state().live(), LiveState::EnabledBlack);
        assert_eq!(e.state().program(), ProgramContent::Black);
    }

    #[test]
    fn revision_bumps_once_per_change_only() {
        let mut e = engine();
        assert_eq!(e.state().revision(), 0);
        run(&mut e, Command::EnableOutput).unwrap();
        assert_eq!(e.state().revision(), 1);
        run(&mut e, Command::EnableOutput).unwrap();
        assert_eq!(e.state().revision(), 1);
        assert!(run(&mut e, Command::ToggleFreeze).is_err());
        assert_eq!(e.state().revision(), 1);
        run(&mut e, Command::Take).unwrap();
        assert_eq!(e.state().revision(), 2);
        run(&mut e, Command::Take).unwrap();
        assert_eq!(e.state().revision(), 2);
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().revision(), 3);
    }

    #[test]
    fn refused_command_does_not_store_time() {
        let mut e = engine();
        e.apply(Command::EnableOutput, Millis(5)).unwrap();
        assert!(e.apply(Command::ToggleFreeze, Millis(9)).is_err());
        assert_eq!(e.last_now(), Millis(5));
    }

    #[test]
    fn private_context_never_reaches_program() {
        let mut e = live_at(3);
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.preview_context(), Some("Wait for amen"));
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.preview_context(), Some("Priest speaks"));
        run(&mut e, Command::First).unwrap();
        assert_eq!(e.preview_context(), None);
    }

    #[test]
    fn refusal_messages_are_readable() {
        assert_eq!(Refusal::OutputDisabled.to_string(), "output is disabled");
        let e = engine();
        let text = Refusal::UnavailablePreview {
            cue: e.snapshot().positions()[2].cue_id,
        }
        .to_string();
        assert!(text.contains("unavailable"));
    }
}
