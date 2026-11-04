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
    /// Output is disabled; enable it first. Returned for Space, F, and B.
    OutputDisabled,
    /// The display is disconnected and the recognized display is not back.
    /// Returned for Space and EnableOutput only; F and B report
    /// [`Refusal::OutputDisconnected`].
    DisplayNotBack,
    /// F or B arrived while the output display is disconnected, whether or not
    /// the recognized display is back: neither can act on a lost output.
    OutputDisconnected,
    /// Freeze is meaningless while Program is blacked out.
    FreezeDuringBlackout,
    /// F needs Live or Frozen; B needs an open output (EnabledBlack, Live,
    /// Frozen, or Blackout). Returned only for F from EnabledBlack, since the
    /// closed and disconnected cases have their own reasons.
    NotLive,
}

/// The `Display` text is a developer-facing log string, not operator copy.
/// Operator-visible wording belongs to the T1.4 UI.
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnavailablePreview { cue } => {
                write!(f, "Cue {cue} is unavailable and cannot go live")
            }
            Self::OutputDisabled => f.write_str("output is disabled"),
            Self::DisplayNotBack => f.write_str("the output display is not back"),
            Self::FreezeDuringBlackout => f.write_str("cannot freeze during Blackout"),
            Self::OutputDisconnected => f.write_str("the output display is disconnected"),
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
    pub fn snapshot(&self) -> &Arc<PresentationSnapshot> {
        &self.snapshot
    }

    /// The Preview Cue.
    pub fn preview_cue(&self) -> &Cue {
        self.cue_at(self.state.preview)
    }

    /// The Cue at flat index `index`.
    fn cue_at(&self, index: usize) -> &Cue {
        // A snapshot always has a Cue (sections, items, and Cues are non-empty)
        // and every index the Engine stores is clamped to its range, so this
        // lookup cannot fail.
        self.snapshot
            .cue(index)
            .expect("Cue index is always inside the snapshot")
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
    /// A refused command changes nothing, including `last_now`. Time never
    /// runs backwards: `now` is clamped to the latest time already seen.
    pub fn apply(&mut self, command: Command, now: Millis) -> Result<Outcome, Refusal> {
        let next = self.evaluate(command)?;
        self.last_now = now.max(self.last_now);
        Ok(match next {
            Some(next) => {
                self.state = next;
                Outcome::Changed
            }
            None => Outcome::Unchanged,
        })
    }

    /// Dry run of [`Engine::apply`]: what the command would return, without
    /// changing state, revision, or `last_now`. `_now` is reserved for the
    /// time-dependent commands WP2 adds.
    pub fn check(&self, command: Command, _now: Millis) -> Result<Outcome, Refusal> {
        Ok(match self.evaluate(command)? {
            Some(_) => Outcome::Changed,
            None => Outcome::Unchanged,
        })
    }

    /// The state `command` leads to (revision already bumped), or `None` when
    /// it changes nothing.
    fn evaluate(&self, command: Command) -> Result<Option<EngineState>, Refusal> {
        let mut next = self.state.clone();
        self.transition(&mut next, command)?;
        // Change detection compares the whole `EngineState`, so everything in
        // it must be observable state. Timestamps WP2 adds (fade starts,
        // auto-play deadlines) must stay out of this equality, or every
        // command would look like a change.
        if next == self.state {
            return Ok(None);
        }
        next.revision = self.state.revision + 1;
        Ok(Some(next))
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
            _ => unreachable!("navigation_target is only called for navigation commands"),
        }
    }

    /// Put the linked effect of Cue `cue` onto `next`, with `keep` as the
    /// Program a `Wait` leaves alone.
    fn apply_effect(&self, next: &mut EngineState, cue: usize, keep: ProgramContent) {
        match self.cue_at(cue).program_effect() {
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
        let cue = self.cue_at(preview);
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
            c if c.is_navigation() => {
                next.preview = self.navigation_target(c);
                if next.live == LiveState::Live {
                    let (preview, keep) = (next.preview, next.program);
                    self.apply_effect(next, preview, keep);
                }
            }
            Command::Take => self.take(next)?,
            Command::ToggleFreeze => match next.live {
                LiveState::Live => {
                    next.live = LiveState::Frozen;
                    // The flag describes linked Live only; Frozen holds Black.
                    next.unavailable_black = false;
                }
                LiveState::Frozen => self.take(next)?,
                LiveState::Blackout => return Err(Refusal::FreezeDuringBlackout),
                LiveState::EnabledBlack => return Err(Refusal::NotLive),
                LiveState::OutputDisabled => return Err(Refusal::OutputDisabled),
                LiveState::Disconnected { .. } => return Err(Refusal::OutputDisconnected),
            },
            Command::ToggleBlackout => match next.live {
                LiveState::Live | LiveState::Frozen | LiveState::EnabledBlack => {
                    Self::go_black(next, LiveState::Blackout);
                }
                LiveState::Blackout => self.take(next)?,
                LiveState::OutputDisabled => return Err(Refusal::OutputDisabled),
                LiveState::Disconnected { .. } => return Err(Refusal::OutputDisconnected),
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
                LiveState::OutputDisabled => {}
                // The returned display went away again.
                LiveState::Disconnected { .. } => {
                    next.live = LiveState::Disconnected {
                        display_back: false,
                    };
                }
                _ => Self::go_black(
                    next,
                    LiveState::Disconnected {
                        display_back: false,
                    },
                ),
            },
            Command::DisplayReturned { recognized } => {
                // An unrecognized display never revokes one already recognized.
                if let LiveState::Disconnected { display_back } = next.live {
                    next.live = LiveState::Disconnected {
                        display_back: display_back || recognized,
                    };
                }
            }
            // The guard arm above takes every navigation command; the compiler
            // cannot see that through the guard.
            _ => unreachable!("navigation commands are handled by the first arm"),
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
        build(vec![
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
        ])
    }

    /// Flat layout: 0 project, 1 project; 2 hold; 3 project, 4 trigger.
    /// Item starts are 0, 2 (Hold), and 3; the last Cue is a trigger.
    fn black_edges_engine() -> Engine {
        build(vec![
            item(0, MassItemKind::Song, vec![project(0), project(1)]),
            item(
                1,
                MassItemKind::Reading,
                vec![cue(6, None, CueBehavior::Hold)],
            ),
            item(
                2,
                MassItemKind::GeneralPresentation,
                vec![
                    project(7),
                    cue(4, Some("Wait for amen"), CueBehavior::OperatorTrigger),
                ],
            ),
        ])
    }

    fn build(items: Vec<MassItem>) -> Engine {
        let section =
            MassSection::new(uuid(0x100).parse().unwrap(), "Section".to_owned(), items).unwrap();
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

    /// Asserts `command` is refused with `reason` and that nothing changed:
    /// the whole state and the stored time.
    fn assert_refused(e: &mut Engine, command: Command, reason: Refusal) {
        let (state, now) = (e.state().clone(), e.last_now());
        assert_eq!(e.check(command, Millis(99)), Err(reason));
        assert_eq!(e.apply(command, Millis(99)), Err(reason));
        assert_eq!(e.state(), &state);
        assert_eq!(e.last_now(), now);
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
    fn page_up_from_an_items_first_cue_goes_to_the_previous_items_first_cue() {
        let mut e = engine();
        run(&mut e, Command::GoTo(7)).unwrap();
        run(&mut e, Command::PreviousItem).unwrap();
        assert_eq!(e.state().preview(), 3);
        run(&mut e, Command::PreviousItem).unwrap();
        assert_eq!(e.state().preview(), 0);
        // From the first item's first Cue there is nowhere to go.
        assert_eq!(run(&mut e, Command::PreviousItem), Ok(Outcome::Unchanged));
        assert_eq!(e.state().preview(), 0);
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
    fn linked_landing_on_hold_or_trigger_from_a_frame_goes_black() {
        let mut e = engine();
        run(&mut e, Command::EnableOutput).unwrap();
        run(&mut e, Command::Take).unwrap();
        assert!(shows(&e, 0));
        run(&mut e, Command::GoTo(6)).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());

        let mut e = black_edges_engine();
        run(&mut e, Command::EnableOutput).unwrap();
        run(&mut e, Command::Take).unwrap();
        assert!(shows(&e, 0));
        // NextItem lands on the Hold at flat 2.
        run(&mut e, Command::NextItem).unwrap();
        assert_eq!(e.state().preview(), 2);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
        // Back onto a frame, then Last lands on a trigger.
        run(&mut e, Command::First).unwrap();
        assert!(shows(&e, 0));
        run(&mut e, Command::Last).unwrap();
        assert_eq!(e.state().preview(), 4);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
        // GoTo a trigger from a frame.
        run(&mut e, Command::GoTo(3)).unwrap();
        assert_eq!(
            e.state().program(),
            ProgramContent::Frame {
                frame: frame(7),
                cue: 3
            }
        );
        run(&mut e, Command::GoTo(4)).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
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
        let setups: [&[Command]; 6] = [
            &[],
            &[Command::EnableOutput],
            &[Command::EnableOutput, Command::Take, Command::ToggleFreeze],
            &[Command::EnableOutput, Command::ToggleBlackout],
            // Decision 272: Space on a Disconnected output still refuses an
            // unavailable Preview, whether or not the display is back.
            &[Command::EnableOutput, Command::DisplayLost],
            &[
                Command::EnableOutput,
                Command::DisplayLost,
                Command::DisplayReturned { recognized: true },
            ],
        ];
        for commands in setups {
            let mut e = setup(commands);
            run(&mut e, Command::GoTo(2)).unwrap();
            let cue = e.preview_cue().id();
            assert_refused(&mut e, Command::Take, Refusal::UnavailablePreview { cue });
        }
    }

    #[test]
    fn take_from_each_state() {
        let mut e = engine();
        assert_refused(&mut e, Command::Take, Refusal::OutputDisabled);
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
        assert_refused(&mut e, Command::Take, Refusal::DisplayNotBack);
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
    fn freeze_on_a_trigger_black_program_holds_black_without_the_flag() {
        let mut e = live_at(3);
        run(&mut e, Command::Next).unwrap();
        assert_eq!(e.state().program(), ProgramContent::Black);
        run(&mut e, Command::ToggleFreeze).unwrap();
        assert_eq!(e.state().live(), LiveState::Frozen);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
    }

    #[test]
    fn freeze_on_an_unavailable_black_program_clears_the_flag() {
        let mut e = live_at(1);
        run(&mut e, Command::Next).unwrap();
        assert!(e.state().unavailable_black());
        run(&mut e, Command::ToggleFreeze).unwrap();
        assert_eq!(e.state().live(), LiveState::Frozen);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
        // Releasing onto a Wait keeps the held Black; the flag stays clear.
        run(&mut e, Command::GoTo(5)).unwrap();
        run(&mut e, Command::ToggleFreeze).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert_eq!(e.state().program(), ProgramContent::Black);
        assert!(!e.state().unavailable_black());
    }

    #[test]
    fn unavailable_black_only_while_live() {
        let mut e = live_at(1);
        run(&mut e, Command::Next).unwrap();
        assert!(e.state().unavailable_black());
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert!(!e.state().unavailable_black());
        let mut e = live_at(1);
        run(&mut e, Command::Next).unwrap();
        run(&mut e, Command::DisplayLost).unwrap();
        assert!(!e.state().unavailable_black());
        let mut e = live_at(1);
        run(&mut e, Command::Next).unwrap();
        run(&mut e, Command::DisableOutput).unwrap();
        assert!(!e.state().unavailable_black());
    }

    #[test]
    fn unavailable_cue_leaves_frozen_and_blackout_unchanged() {
        for toggle in [Command::ToggleFreeze, Command::ToggleBlackout] {
            let mut e = live_at(1);
            run(&mut e, toggle).unwrap();
            let (live, program) = (e.state().live(), e.state().program());
            run(&mut e, Command::GoTo(2)).unwrap();
            assert_eq!(e.state().preview(), 2);
            assert_eq!(e.state().live(), live);
            assert_eq!(e.state().program(), program);
            assert!(!e.state().unavailable_black());
        }
    }

    #[test]
    fn freeze_release_refused_on_unavailable_preview_and_stays_frozen() {
        let mut e = live_at(1);
        run(&mut e, Command::ToggleFreeze).unwrap();
        run(&mut e, Command::Next).unwrap();
        let before = e.state().clone();
        let cue = e.preview_cue().id();
        assert_refused(
            &mut e,
            Command::ToggleFreeze,
            Refusal::UnavailablePreview { cue },
        );
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
        let cue = e.preview_cue().id();
        assert_refused(
            &mut e,
            Command::ToggleBlackout,
            Refusal::UnavailablePreview { cue },
        );
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
        assert_refused(&mut e, Command::ToggleFreeze, Refusal::FreezeDuringBlackout);
        assert_eq!(e.state().live(), LiveState::Blackout);

        let mut e = engine();
        assert_refused(&mut e, Command::ToggleFreeze, Refusal::OutputDisabled);
        run(&mut e, Command::EnableOutput).unwrap();
        assert_refused(&mut e, Command::ToggleFreeze, Refusal::NotLive);
        run(&mut e, Command::DisplayLost).unwrap();
        assert_refused(&mut e, Command::ToggleFreeze, Refusal::OutputDisconnected);
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        assert_refused(&mut e, Command::ToggleFreeze, Refusal::OutputDisconnected);
    }

    #[test]
    fn blackout_refusals_when_output_is_not_open() {
        let mut e = engine();
        assert_refused(&mut e, Command::ToggleBlackout, Refusal::OutputDisabled);
        run(&mut e, Command::EnableOutput).unwrap();
        run(&mut e, Command::DisplayLost).unwrap();
        assert_refused(&mut e, Command::ToggleBlackout, Refusal::OutputDisconnected);
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        assert_refused(&mut e, Command::ToggleBlackout, Refusal::OutputDisconnected);
    }

    #[test]
    fn blackout_twice_from_enabled_black_goes_live() {
        let mut e = enabled();
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Blackout);
        run(&mut e, Command::ToggleBlackout).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(shows(&e, 0));
    }

    #[test]
    fn enable_output_is_unchanged_in_frozen_and_blackout() {
        for toggle in [Command::ToggleFreeze, Command::ToggleBlackout] {
            let mut e = live_at(0);
            run(&mut e, toggle).unwrap();
            let before = e.state().clone();
            assert_eq!(run(&mut e, Command::EnableOutput), Ok(Outcome::Unchanged));
            assert_eq!(e.state(), &before);
        }
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
        // The returned display going away again is one more DisplayLost.
        assert_eq!(run(&mut e, Command::DisplayLost), Ok(Outcome::Changed));
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
        assert_refused(&mut e, Command::EnableOutput, Refusal::DisplayNotBack);
    }

    #[test]
    fn unrecognized_display_never_revokes_a_recognized_one() {
        let mut e = live_at(0);
        run(&mut e, Command::DisplayLost).unwrap();
        run(&mut e, Command::DisplayReturned { recognized: true }).unwrap();
        assert_eq!(
            run(&mut e, Command::DisplayReturned { recognized: false }),
            Ok(Outcome::Unchanged)
        );
        assert_eq!(
            e.state().live(),
            LiveState::Disconnected { display_back: true }
        );
    }

    #[test]
    fn display_events_outside_disconnected_are_unchanged() {
        let mut e = live_at(0);
        assert_eq!(
            run(&mut e, Command::DisplayReturned { recognized: true }),
            Ok(Outcome::Unchanged)
        );
        assert!(shows(&e, 0));
        assert_eq!(e.state().live(), LiveState::Live);
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
    fn time_never_runs_backwards() {
        let mut e = engine();
        e.apply(Command::EnableOutput, Millis(9)).unwrap();
        e.apply(Command::Take, Millis(3)).unwrap();
        assert_eq!(e.last_now(), Millis(9));
        e.apply(Command::Next, Millis(12)).unwrap();
        assert_eq!(e.last_now(), Millis(12));
    }

    #[test]
    fn check_is_a_dry_run_that_matches_apply() {
        let mut e = enabled();
        let (state, now) = (e.state().clone(), e.last_now());
        assert_eq!(e.check(Command::Take, Millis(50)), Ok(Outcome::Changed));
        assert_eq!(
            e.check(Command::EnableOutput, Millis(50)),
            Ok(Outcome::Unchanged)
        );
        assert_eq!(
            e.check(Command::ToggleFreeze, Millis(50)),
            Err(Refusal::NotLive)
        );
        assert_eq!(e.state(), &state);
        assert_eq!(e.last_now(), now);
        assert_eq!(e.apply(Command::Take, Millis(50)), Ok(Outcome::Changed));
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
