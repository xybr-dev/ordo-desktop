//! The pure, snapshot-driven Presentation Engine.
//!
//! The Engine owns every live-state transition (MVP-PLAN section 8.1) so UI
//! and tests drive one Interface. It works on Cue positions, never pixels, and
//! never reads a clock: time arrives as [`Millis`] with each command.
//!
//! Fades and auto-play are time-driven views over that state. A state change
//! is always immediate: [`Engine::program_view`] and [`Engine::preview_view`]
//! describe what each monitor draws while the 500 ms fade runs, and
//! [`Engine::tick`] is how the owner of the clock lets auto-play advance.
//! Leaving Presenter drops the Engine, which ends auto-play with it.
//! [`Engine::next_deadline`] tells the owner of the clock when to call back.
//!
//! Runtime availability changes (T1.3) must enter as an Engine input through
//! the same `evaluate`/commit path as a command, so auto-play settling and
//! fade classification run; never swap the snapshot underneath a live Engine.
//!
//! Decisions applied: 272 (one-step resume on the recognized display), 273
//! (Freeze and Blackout toggle off exactly as Space does), 274 (an unavailable
//! Cue makes linked Program black instead of freezing).

use std::fmt;
use std::sync::Arc;

use domain::presentation::{
    Cue, CueBehavior, CueId, FrameId, MassItem, MassItemKind, PresentationSnapshot, ProgramEffect,
};

use crate::commands::Command;
pub use crate::state::{AutoPlayScope, AutoPlayStatus, FADE_MS, ProgramView};
use crate::state::{EngineState, FadeTimeline, LiveState, Millis, ProgramContent};

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
    /// Auto-play cannot run on the requested scope.
    AutoPlayIneligible {
        /// Why the scope is ineligible.
        reason: AutoPlayRefusal,
    },
    /// Auto-play cannot start while Program is Frozen, in Blackout, or the
    /// display is Disconnected: each of them stops a run.
    AutoPlayBlocked,
}

/// Why a scope is not eligible for auto-play (MVP-PLAN section 8.4), so the
/// UI can explain "ineligible liturgical content".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutoPlayRefusal {
    /// The Preview Cue's item is not a General Presentation.
    NotGeneralPresentation,
    /// The item is a General Presentation without auto-play durations.
    NoDurations,
    /// Nothing in the scope can be projected.
    NothingProjectable,
    /// The section holds an item with projectable Cues that is not an
    /// auto-play General Presentation.
    SectionHasIneligibleItem,
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
            Self::AutoPlayIneligible { reason } => {
                write!(f, "auto-play is not available here: {reason:?}")
            }
            Self::AutoPlayBlocked => {
                f.write_str("auto-play cannot start while frozen, blacked out, or disconnected")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// The Presentation Engine over one immutable snapshot.
#[derive(Clone, Debug)]
pub struct Engine {
    snapshot: Arc<PresentationSnapshot>,
    state: EngineState,
    /// Latest time seen (command or tick); times are clamped to it.
    last_now: Millis,
    /// Fade of the Program monitor. Derived view state, outside `EngineState`.
    program_fade: FadeTimeline,
    /// Fade of the Preview monitor's own black-and-content changes.
    preview_fade: FadeTimeline,
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
                auto_play: None,
                revision: 0,
            },
            last_now: Millis(0),
            program_fade: FadeTimeline::default(),
            preview_fade: FadeTimeline::default(),
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
    /// A running fade never delays or refuses a command.
    pub fn apply(&mut self, command: Command, now: Millis) -> Result<Outcome, Refusal> {
        let now = now.max(self.last_now);
        let (next, changed) = self.evaluate(command, now)?;
        self.last_now = now;
        Ok(self.commit(next, changed, now))
    }

    /// Dry run of [`Engine::apply`]: what the command would return, without
    /// changing state, revision, fades, or `last_now`.
    pub fn check(&self, command: Command, now: Millis) -> Result<Outcome, Refusal> {
        let (_, changed) = self.evaluate(command, now.max(self.last_now))?;
        Ok(if changed {
            Outcome::Changed
        } else {
            Outcome::Unchanged
        })
    }

    /// Lets time pass: drops finished fades and, when the current auto-play
    /// Cue is due (`now >= cue_started + duration`), advances to the next
    /// projectable Cue in scope, wrapping last to first. The advance is an
    /// ordinary navigation (linked Live moves Program with it) and the new Cue
    /// gets its full duration counted from `now`, not from its due time, so a
    /// late tick advances once and never catches up.
    ///
    /// Returns [`Outcome::Changed`] only when observable state changed; the
    /// timer restart of a one-Cue loop and dropped fades are not changes.
    pub fn tick(&mut self, now: Millis) -> Outcome {
        let now = now.max(self.last_now);
        self.last_now = now;
        self.program_fade.settle(now);
        self.preview_fade.settle(now);
        let Some(run) = self.state.auto_play else {
            return Outcome::Unchanged;
        };
        // A run parked on a Cue that cannot advance has no deadline.
        let Some(duration_ms) = run.duration_ms else {
            return Outcome::Unchanged;
        };
        if now.0 < run.cue_started.0.saturating_add(duration_ms) {
            return Outcome::Unchanged;
        }
        let mut next = self.state.clone();
        if let Some(target) = self.next_in_scope(run.scope, next.preview) {
            self.navigate(&mut next, target);
        }
        // Restart the clock even when the loop has one Cue and Preview stays.
        if let Some(run) = &mut next.auto_play {
            run.cue_started = now;
        }
        self.settle_auto_play(&self.state, &mut next, now);
        let changed = !next.same_observable(&self.state);
        self.commit(next, changed, now)
    }

    /// The next time the owner of the clock should call [`Engine::tick`] or
    /// redraw: the earliest of the running Program or Preview fade end and the
    /// auto-play Cue deadline. `None` when nothing is pending. An overdue
    /// auto-play deadline is returned as is, so a late owner still ticks.
    pub fn next_deadline(&self) -> Option<Millis> {
        let fades = [self.program_fade.end(), self.preview_fade.end()]
            .into_iter()
            .flatten()
            .filter(|end| *end > self.last_now);
        let auto_play = self.state.auto_play.and_then(|run| {
            run.duration_ms
                .map(|ms| Millis(run.cue_started.0.saturating_add(ms)))
        });
        fades.chain(auto_play).min()
    }

    /// What the Program monitor draws at `now`. Instant state changes are
    /// shown as a 500 ms fade for content to or from black; content to other
    /// content is a cut. Disabling the output or losing the display is instant.
    pub fn program_view(&self, now: Millis) -> ProgramView {
        self.program_fade
            .view(Self::program_frame(&self.state), now.max(self.last_now))
    }

    /// What the Preview monitor draws at `now`, with the same fade rules
    /// applied to Preview's own black-and-content changes. Preview is black
    /// when its Cue is a trigger, Hold, unavailable, or a Wait.
    ///
    /// `shown: None` means "no congregation frame", not "nothing to show the
    /// operator": when [`Engine::preview_cue`]'s `program_effect()` is
    /// `NotPresentable` the UI overlays the operator-only "Output unavailable"
    /// placeholder on the black Preview (Decision 274).
    pub fn preview_view(&self, now: Millis) -> ProgramView {
        self.preview_fade.view(
            self.preview_frame(self.state.preview),
            now.max(self.last_now),
        )
    }

    fn program_frame(state: &EngineState) -> Option<FrameId> {
        match state.program {
            ProgramContent::Black => None,
            ProgramContent::Frame { frame, .. } => Some(frame),
        }
    }

    fn preview_frame(&self, index: usize) -> Option<FrameId> {
        match self.cue_at(index).program_effect() {
            ProgramEffect::Frame(frame) => Some(frame),
            _ => None,
        }
    }

    /// Stores `next` and moves the fades. The revision moves only on a change.
    fn commit(&mut self, mut next: EngineState, changed: bool, now: Millis) -> Outcome {
        let old_program = Self::program_frame(&self.state);
        let old_preview = self.state.preview;
        // Disabling the output or losing the display removes the window, so
        // there is nothing left to fade.
        let instant = matches!(
            next.live,
            LiveState::OutputDisabled | LiveState::Disconnected { .. }
        );
        self.program_fade
            .changed(old_program, Self::program_frame(&next), now, instant);
        if old_preview != next.preview {
            self.preview_fade.changed(
                self.preview_frame(old_preview),
                self.preview_frame(next.preview),
                now,
                false,
            );
        }
        if changed {
            next.revision = self.state.revision + 1;
        } else {
            next.revision = self.state.revision;
        }
        self.state = next;
        if changed {
            Outcome::Changed
        } else {
            Outcome::Unchanged
        }
    }

    /// The state `command` leads to and whether it is an observable change.
    /// The revision is not bumped here. Change detection ignores revision and
    /// the auto-play clock ([`EngineState::same_observable`]), and fade
    /// timelines live outside the state, so neither makes a command look like
    /// a change.
    fn evaluate(&self, command: Command, now: Millis) -> Result<(EngineState, bool), Refusal> {
        let mut next = self.state.clone();
        self.transition(&mut next, command, now)?;
        self.settle_auto_play(&self.state, &mut next, now);
        let changed = !next.same_observable(&self.state);
        Ok((next, changed))
    }

    /// The one place auto-play stops or restarts its clock, keyed on the
    /// (old, new) difference rather than on individual commands: it stops
    /// when the live state newly enters Frozen, Blackout, OutputDisabled, or
    /// Disconnected, or when Preview is not on a projectable Cue of the scope;
    /// it restarts the clock when Preview moved to another Cue inside scope
    /// (and drops the deadline when that Cue cannot advance).
    fn settle_auto_play(&self, old: &EngineState, next: &mut EngineState, now: Millis) {
        let Some(mut run) = next.auto_play else {
            return;
        };
        let halted = std::mem::discriminant(&old.live) != std::mem::discriminant(&next.live)
            && matches!(
                next.live,
                LiveState::Frozen
                    | LiveState::Blackout
                    | LiveState::OutputDisabled
                    | LiveState::Disconnected { .. }
            );
        if halted || !self.in_scope(run.scope, next.preview) {
            next.auto_play = None;
            return;
        }
        if old.preview != next.preview {
            run.cue_started = now;
            // `None` on an in-scope Cue that cannot advance: the run stays
            // without a deadline until the operator lands on a projectable Cue.
            run.duration_ms = self.cue_duration_ms(next.preview);
            next.auto_play = Some(run);
        }
    }

    /// Duration of the Cue at `flat` when it is projectable, in milliseconds.
    fn cue_duration_ms(&self, flat: usize) -> Option<u64> {
        let item = self.snapshot.item_of(flat)?;
        let seconds = item.cue_duration_seconds(self.snapshot.cue(flat)?)?;
        Some(u64::from(seconds) * 1000)
    }

    /// Whether `flat` belongs to `scope`: any Cue of the item or section,
    /// projectable or not. Only navigation outside this stops a run
    /// (CONTEXT "Auto-play scope").
    fn in_scope(&self, scope: AutoPlayScope, flat: usize) -> bool {
        let Some(position) = self.snapshot.positions().get(flat) else {
            return false;
        };
        match scope {
            AutoPlayScope::Item(item) => position.item_id == item,
            AutoPlayScope::Section(section) => self
                .snapshot
                .sections()
                .get(position.section)
                .is_some_and(|candidate| candidate.id() == section),
        }
    }

    /// Whether `flat` is a Cue of `scope` that auto-play can show and time:
    /// projectable, with a duration.
    fn advancing_in_scope(&self, scope: AutoPlayScope, flat: usize) -> bool {
        self.in_scope(scope, flat) && self.cue_duration_ms(flat).is_some()
    }

    /// The projectable Cue after `from` in `scope`, wrapping last to first.
    /// Non-projectable Cues of the scope are skipped.
    fn next_in_scope(&self, scope: AutoPlayScope, from: usize) -> Option<usize> {
        let count = self.snapshot.cue_count();
        (1..=count)
            .map(|step| (from + step) % count)
            .find(|&flat| self.advancing_in_scope(scope, flat))
    }

    /// Checks `scope` is eligible, as seen from the Preview Cue's position.
    fn check_eligible(&self, scope: AutoPlayScope, preview: usize) -> Result<(), Refusal> {
        let ineligible = |reason| Err(Refusal::AutoPlayIneligible { reason });
        let projectable = |item: &MassItem| {
            item.cues()
                .iter()
                .any(|cue| matches!(cue.behavior(), CueBehavior::Project { .. }))
        };
        // Content of any kind: an item whose slides all failed still counts
        // as the wrong kind of item in a section, so it blocks the loop
        // instead of letting a Song look like an empty one.
        let has_content = |item: &&MassItem| {
            item.cues().iter().any(|cue| {
                matches!(
                    cue.behavior(),
                    CueBehavior::Project { .. } | CueBehavior::Unavailable { .. }
                )
            })
        };
        let position = self.snapshot.positions()[preview];
        match scope {
            AutoPlayScope::Item(_) => {
                let item = &self.snapshot.sections()[position.section].items()[position.item];
                if item.kind() != MassItemKind::GeneralPresentation {
                    return ineligible(AutoPlayRefusal::NotGeneralPresentation);
                }
                if item.auto_play().is_none() {
                    return ineligible(AutoPlayRefusal::NoDurations);
                }
                if !projectable(item) {
                    return ineligible(AutoPlayRefusal::NothingProjectable);
                }
            }
            AutoPlayScope::Section(_) => {
                let section = &self.snapshot.sections()[position.section];
                let mut any = false;
                for item in section.items().iter().filter(has_content) {
                    if item.kind() != MassItemKind::GeneralPresentation
                        || item.auto_play().is_none()
                    {
                        return ineligible(AutoPlayRefusal::SectionHasIneligibleItem);
                    }
                    any |= projectable(item);
                }
                if !any {
                    return ineligible(AutoPlayRefusal::NothingProjectable);
                }
            }
        }
        Ok(())
    }

    /// Starts auto-play on `scope`. Moves Preview to the scope's first
    /// projectable Cue when it is not already on one (an ordinary navigation).
    fn start_auto_play(
        &self,
        next: &mut EngineState,
        scope: AutoPlayScope,
        now: Millis,
    ) -> Result<(), Refusal> {
        self.check_eligible(scope, next.preview)?;
        if matches!(
            next.live,
            LiveState::Frozen | LiveState::Blackout | LiveState::Disconnected { .. }
        ) {
            return Err(Refusal::AutoPlayBlocked);
        }
        if !self.advancing_in_scope(scope, next.preview) {
            let first =
                (0..self.snapshot.cue_count()).find(|&flat| self.advancing_in_scope(scope, flat));
            if let Some(first) = first {
                self.navigate(next, first);
            }
        }
        // Widening a run keeps the current Cue's clock when that Cue is still
        // in the new scope: the Cue is not shown anew.
        let kept = next
            .auto_play
            .filter(|_| self.in_scope(scope, next.preview) && next.preview == self.state.preview);
        next.auto_play = Some(AutoPlayStatus {
            scope,
            cue_started: kept.map_or(now, |run| run.cue_started),
            duration_ms: match kept {
                Some(run) => run.duration_ms,
                None => self.cue_duration_ms(next.preview),
            },
        });
        Ok(())
    }

    /// Moves Preview to `target`, and Program with it while linked Live.
    fn navigate(&self, next: &mut EngineState, target: usize) {
        next.preview = target;
        if next.live == LiveState::Live {
            let keep = next.program;
            self.apply_effect(next, target, keep);
        }
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

    fn transition(
        &self,
        next: &mut EngineState,
        command: Command,
        now: Millis,
    ) -> Result<(), Refusal> {
        match command {
            c if c.is_navigation() => {
                let target = self.navigation_target(c);
                self.navigate(next, target);
            }
            Command::ToggleAutoPlay => {
                if next.auto_play.is_some() {
                    next.auto_play = None;
                } else {
                    let item = self.snapshot.positions()[next.preview].item_id;
                    self.start_auto_play(next, AutoPlayScope::Item(item), now)?;
                }
            }
            Command::LoopSection => {
                let section = self.snapshot.positions()[next.preview].section;
                let scope = AutoPlayScope::Section(self.snapshot.sections()[section].id());
                if next.auto_play.map(|run| run.scope) != Some(scope) {
                    self.start_auto_play(next, scope, now)?;
                }
            }
            Command::StopAutoPlay | Command::Recover => next.auto_play = None,
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
            Command::DisableOutput => {
                Self::go_black(next, LiveState::OutputDisabled);
                // Also ends a rehearsal run, where the live state does not move.
                next.auto_play = None;
            }
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

#[cfg(test)]
mod timed_tests {
    //! Fades and auto-play, driven with explicit times.
    use super::*;
    use domain::presentation::{AutoPlay, MassSection, SnapshotId, ThemeId, ThemeKind, ThemeRef};

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

    fn project(n: u16, seconds: Option<u32>) -> Cue {
        cue(
            n,
            None,
            CueBehavior::Project {
                frame: frame(n),
                duration_seconds: seconds,
            },
        )
    }

    fn trigger(n: u16) -> Cue {
        cue(n, Some("Wait for amen"), CueBehavior::OperatorTrigger)
    }

    fn item(n: u16, kind: MassItemKind, auto_seconds: Option<u32>, cues: Vec<Cue>) -> MassItem {
        MassItem::new(
            uuid(0x200 + n).parse().unwrap(),
            format!("Item {n}"),
            kind,
            None,
            auto_seconds.map(|default_seconds| AutoPlay { default_seconds }),
            cues,
        )
        .unwrap()
    }

    fn gp(n: u16, auto_seconds: u32, cues: Vec<Cue>) -> MassItem {
        item(
            n,
            MassItemKind::GeneralPresentation,
            Some(auto_seconds),
            cues,
        )
    }

    fn item_scope(n: u16) -> AutoPlayScope {
        AutoPlayScope::Item(uuid(0x200 + n).parse().unwrap())
    }

    fn section_scope(n: u16) -> AutoPlayScope {
        AutoPlayScope::Section(uuid(0x100 + n).parse().unwrap())
    }

    /// Flat layout:
    /// section 0: item 0 Song 0,1 | item 1 General Presentation without auto-play 2;
    /// section 1: item 2 (8s) 3 project(8s), 4 project(3s override), 5 trigger, 6 project(8s)
    ///            | item 3 (4s) 7 project, 8 hold, 9 unavailable
    ///            | item 4 (5s) 10 trigger, 11 wait;
    /// section 2: item 5 (2s) 12 project (a one-Cue loop);
    /// section 3: item 6 (5s) 13 trigger.
    fn engine() -> Engine {
        let sections = vec![
            vec![
                item(
                    0,
                    MassItemKind::Song,
                    None,
                    vec![project(0, None), project(1, None)],
                ),
                item(
                    1,
                    MassItemKind::GeneralPresentation,
                    None,
                    vec![project(2, None)],
                ),
            ],
            vec![
                gp(
                    2,
                    8,
                    vec![
                        project(10, None),
                        project(11, Some(3)),
                        trigger(12),
                        project(13, None),
                    ],
                ),
                gp(
                    3,
                    4,
                    vec![
                        project(20, None),
                        cue(21, None, CueBehavior::Hold),
                        cue(
                            22,
                            None,
                            CueBehavior::Unavailable {
                                reason: "no font".to_owned(),
                            },
                        ),
                    ],
                ),
                gp(
                    4,
                    5,
                    vec![
                        trigger(30),
                        cue(31, Some("Priest speaks"), CueBehavior::Wait),
                    ],
                ),
            ],
            vec![gp(5, 2, vec![project(40, None)])],
            vec![gp(6, 5, vec![trigger(41)])],
        ];
        build(sections)
    }

    fn build(sections: Vec<Vec<MassItem>>) -> Engine {
        let sections = sections
            .into_iter()
            .enumerate()
            .map(|(n, items)| {
                MassSection::new(
                    uuid(0x100 + u16::try_from(n).unwrap()).parse().unwrap(),
                    format!("Section {n}"),
                    items,
                )
                .unwrap()
            })
            .collect();
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
            sections,
        )
        .unwrap();
        Engine::new(Arc::new(snapshot))
    }

    fn at(e: &mut Engine, command: Command, ms: u64) -> Result<Outcome, Refusal> {
        e.apply(command, Millis(ms))
    }

    /// Output enabled, Preview on `flat`, taken live at time 0.
    fn live_at(flat: usize) -> Engine {
        let mut e = engine();
        at(&mut e, Command::EnableOutput, 0).unwrap();
        at(&mut e, Command::GoTo(flat), 0).unwrap();
        at(&mut e, Command::Take, 0).unwrap();
        e
    }

    fn view(e: &Engine, ms: u64) -> ProgramView {
        e.program_view(Millis(ms))
    }

    fn preview_of(e: &Engine, ms: u64) -> ProgramView {
        e.preview_view(Millis(ms))
    }

    /// Fade tests count from here: `live_at` takes Cue 3 live at time 0, which
    /// is itself a fade-in, so they start once it has finished.
    const T0: u64 = 1_000;

    fn fat(e: &mut Engine, command: Command, ms: u64) -> Result<Outcome, Refusal> {
        at(e, command, T0 + ms)
    }

    fn fview(e: &Engine, ms: u64) -> ProgramView {
        view(e, T0 + ms)
    }

    fn approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.01,
            "opacity {actual} is not about {expected}"
        );
    }

    #[test]
    fn blackout_fades_out_and_release_reverses_from_the_current_opacity() {
        let mut e = live_at(3);
        assert_eq!(
            fview(&e, 0),
            ProgramView {
                shown: Some(frame(10)),
                opacity: 1.0,
                fading: false
            }
        );
        fat(&mut e, Command::ToggleBlackout, 0).unwrap();
        // The state change is immediate; only the view fades.
        assert_eq!(e.state().program(), ProgramContent::Black);
        let at_250 = fview(&e, 250);
        assert_eq!(at_250.shown, Some(frame(10)));
        approx(at_250.opacity, 0.5);
        assert!(at_250.fading);
        // Release with a different Cue: it fades in from 0.5, not from 0.
        fat(&mut e, Command::GoTo(6), 250).unwrap();
        fat(&mut e, Command::ToggleBlackout, 250).unwrap();
        let reversed = fview(&e, 250);
        assert_eq!(reversed.shown, Some(frame(13)));
        approx(reversed.opacity, 0.5);
        approx(fview(&e, 375).opacity, 0.75);
        let done = fview(&e, 500);
        assert_eq!(done.opacity, 1.0);
        assert!(!done.fading);
        assert_eq!(done.shown, Some(frame(13)));
    }

    #[test]
    fn content_to_content_is_a_cut() {
        let mut e = live_at(3);
        fat(&mut e, Command::Next, 100).unwrap();
        assert_eq!(
            fview(&e, 100),
            ProgramView {
                shown: Some(frame(11)),
                opacity: 1.0,
                fading: false
            }
        );
    }

    #[test]
    fn a_cut_during_fade_in_keeps_the_fade_going_on_the_new_frame() {
        let mut e = live_at(3);
        fat(&mut e, Command::ToggleBlackout, 0).unwrap();
        fat(&mut e, Command::ToggleBlackout, 250).unwrap();
        // Blackout released onto Cue 3 at opacity 0.5; a cut to Cue 4 mid-fade.
        fat(&mut e, Command::Next, 300).unwrap();
        let cut = fview(&e, 300);
        assert_eq!(cut.shown, Some(frame(11)));
        approx(cut.opacity, 0.6);
        assert!(cut.fading);
        approx(fview(&e, 400).opacity, 0.8);
        assert_eq!(fview(&e, 500).opacity, 1.0);
    }

    #[test]
    fn hold_trigger_and_unavailable_fade_out() {
        // 5 trigger, 8 hold, 9 unavailable.
        for target in [5, 8, 9] {
            let mut e = live_at(3);
            fat(&mut e, Command::GoTo(target), 100).unwrap();
            assert_eq!(e.state().program(), ProgramContent::Black);
            let start = fview(&e, 100);
            assert_eq!(start.shown, Some(frame(10)));
            assert_eq!(start.opacity, 1.0);
            assert!(start.fading);
            approx(fview(&e, 350).opacity, 0.5);
            let end = fview(&e, 600);
            assert_eq!(end.shown, None);
            assert_eq!(end.opacity, 0.0);
            assert!(!end.fading);
        }
    }

    #[test]
    fn wait_keeps_program_and_does_not_fade() {
        let mut e = live_at(3);
        fat(&mut e, Command::GoTo(11), 100).unwrap();
        assert_eq!(
            fview(&e, 100),
            ProgramView {
                shown: Some(frame(10)),
                opacity: 1.0,
                fading: false
            }
        );
    }

    #[test]
    fn disable_output_and_display_loss_are_instant() {
        for command in [Command::DisableOutput, Command::DisplayLost] {
            let mut e = live_at(3);
            fat(&mut e, command, 100).unwrap();
            assert_eq!(
                fview(&e, 100),
                ProgramView {
                    shown: None,
                    opacity: 0.0,
                    fading: false
                }
            );
        }
    }

    #[test]
    fn black_to_black_does_not_restart_a_fade() {
        let mut e = live_at(3);
        fat(&mut e, Command::ToggleBlackout, 0).unwrap();
        // Freeze is refused during Blackout; a navigation keeps Program black.
        fat(&mut e, Command::Next, 250).unwrap();
        approx(fview(&e, 250).opacity, 0.5);
        assert_eq!(fview(&e, 500).shown, None);
    }

    #[test]
    fn a_running_fade_never_delays_or_refuses_a_command() {
        let mut e = live_at(3);
        fat(&mut e, Command::ToggleBlackout, 0).unwrap();
        fat(&mut e, Command::ToggleBlackout, 10).unwrap();
        fat(&mut e, Command::ToggleFreeze, 20).unwrap();
        fat(&mut e, Command::Next, 30).unwrap();
        fat(&mut e, Command::ToggleFreeze, 40).unwrap();
        fat(&mut e, Command::ToggleBlackout, 50).unwrap();
        fat(&mut e, Command::ToggleBlackout, 60).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(e.check(Command::ToggleBlackout, Millis(1_000 + 70)).is_ok());
    }

    #[test]
    fn opacity_is_monotonic_within_one_fade() {
        let mut out = live_at(3);
        fat(&mut out, Command::ToggleBlackout, 0).unwrap();
        let mut last = 1.0;
        for ms in (0..=600).step_by(10) {
            let opacity = fview(&out, ms).opacity;
            assert!(opacity <= last, "fade-out rose at {ms}ms");
            last = opacity;
        }
        let mut incoming = live_at(3);
        fat(&mut incoming, Command::ToggleBlackout, 0).unwrap();
        fat(&mut incoming, Command::ToggleBlackout, 500).unwrap();
        let mut last = 0.0;
        for ms in (500..=1100).step_by(10) {
            let opacity = fview(&incoming, ms).opacity;
            assert!(opacity >= last, "fade-in fell at {ms}ms");
            last = opacity;
        }
        assert_eq!(last, 1.0);
    }

    #[test]
    fn repeating_a_command_later_is_unchanged_and_check_leaves_fades_alone() {
        let mut e = live_at(3);
        let revision = e.state().revision();
        assert_eq!(fat(&mut e, Command::Take, 5_000), Ok(Outcome::Unchanged));
        assert_eq!(e.state().revision(), revision);
        assert_eq!(
            e.check(Command::ToggleBlackout, Millis(6_000)),
            Ok(Outcome::Changed)
        );
        assert!(!fview(&e, 6_000).fading);
    }

    #[test]
    fn preview_monitor_fades_its_own_black_and_content_changes() {
        let mut e = engine();
        assert_eq!(preview_of(&e, 0).shown, Some(frame(0)));
        // Preview moves onto a trigger Cue: black, with a fade-out.
        at(&mut e, Command::GoTo(5), 0).unwrap();
        approx(preview_of(&e, 250).opacity, 0.5);
        assert_eq!(preview_of(&e, 250).shown, Some(frame(0)));
        assert_eq!(preview_of(&e, 500).shown, None);
        // Content to content is a cut.
        at(&mut e, Command::GoTo(6), 1_000).unwrap();
        at(&mut e, Command::GoTo(7), 1_000).unwrap();
        assert_eq!(preview_of(&e, 1_000).shown, Some(frame(20)));
        // A Wait is black in Preview.
        at(&mut e, Command::GoTo(11), 2_000).unwrap();
        assert!(preview_of(&e, 2_000).fading);
        assert_eq!(preview_of(&e, 2_600).shown, None);
        // Preview fades independently of Program.
        assert_eq!(view(&e, 2_000).shown, None);
    }

    // Auto-play.

    #[test]
    fn item_auto_play_advances_skips_and_wraps() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        assert_eq!(
            at(&mut e, Command::ToggleAutoPlay, 1_000),
            Ok(Outcome::Changed)
        );
        assert_eq!(
            e.state().auto_play(),
            Some(AutoPlayStatus {
                scope: item_scope(2),
                cue_started: Millis(1_000),
                duration_ms: Some(8_000)
            })
        );
        assert_eq!(e.tick(Millis(8_999)), Outcome::Unchanged);
        assert_eq!(e.tick(Millis(9_000)), Outcome::Changed);
        assert_eq!(e.state().preview(), 4);
        // The per-Cue override wins over the item default.
        assert_eq!(e.state().auto_play().unwrap().duration_ms, Some(3_000));
        assert_eq!(e.state().auto_play().unwrap().cue_started, Millis(9_000));
        assert_eq!(e.tick(Millis(11_999)), Outcome::Unchanged);
        // Cue 5 is a trigger: skipped.
        assert_eq!(e.tick(Millis(12_000)), Outcome::Changed);
        assert_eq!(e.state().preview(), 6);
        assert_eq!(e.state().auto_play().unwrap().duration_ms, Some(8_000));
        // Wrap from the last to the first projectable Cue.
        assert_eq!(e.tick(Millis(20_000)), Outcome::Changed);
        assert_eq!(e.state().preview(), 3);
        assert!(e.state().auto_play().is_some());
        // Output was never enabled: Preview only, Program stays black.
        assert_eq!(e.state().program(), ProgramContent::Black);
    }

    #[test]
    fn auto_play_drives_linked_program() {
        let mut e = live_at(3);
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        e.tick(Millis(8_000));
        assert_eq!(
            e.state().program(),
            ProgramContent::Frame {
                frame: frame(11),
                cue: 4
            }
        );
        assert_eq!(view(&e, 8_000).shown, Some(frame(11)));
        assert!(!view(&e, 8_000).fading);
    }

    #[test]
    fn starting_moves_preview_to_the_first_projectable_cue() {
        let mut e = engine();
        at(&mut e, Command::GoTo(5), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        assert_eq!(e.state().preview(), 3);
        let mut e = live_at(5);
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        assert_eq!(e.state().preview(), 3);
        // The linked effect of that navigation applies.
        assert_eq!(
            e.state().program(),
            ProgramContent::Frame {
                frame: frame(10),
                cue: 3
            }
        );
    }

    #[test]
    fn manual_next_within_scope_keeps_running_and_resets_the_timer() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        at(&mut e, Command::Next, 5_000).unwrap();
        let run = e.state().auto_play().unwrap();
        assert_eq!(
            (run.cue_started, run.duration_ms),
            (Millis(5_000), Some(3_000))
        );
        assert_eq!(e.tick(Millis(7_999)), Outcome::Unchanged);
        assert_eq!(e.tick(Millis(8_000)), Outcome::Changed);
        assert_eq!(e.state().preview(), 6);
    }

    #[test]
    fn take_does_not_stop_auto_play() {
        let mut e = engine();
        at(&mut e, Command::EnableOutput, 0).unwrap();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        at(&mut e, Command::Take, 100).unwrap();
        assert_eq!(e.state().live(), LiveState::Live);
        assert!(e.state().auto_play().is_some());
    }

    #[test]
    fn section_loop_runs_across_items() {
        let mut e = engine();
        at(&mut e, Command::GoTo(4), 0).unwrap();
        at(&mut e, Command::LoopSection, 0).unwrap();
        assert_eq!(e.state().auto_play().unwrap().scope, section_scope(1));
        // From item 2's Cue 4: 6, then item 3's Cue 7 (4s), then wrap to 3.
        let mut seen = Vec::new();
        for ms in [3_000, 11_000, 15_000] {
            e.tick(Millis(ms));
            seen.push(e.state().preview());
        }
        assert_eq!(seen, [6, 7, 3]);
        // Repeating the menu command on the same section changes nothing.
        assert_eq!(
            at(&mut e, Command::LoopSection, 15_100),
            Ok(Outcome::Unchanged)
        );
    }

    #[test]
    fn loop_section_widens_an_item_run_and_toggle_stops_either() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        assert_eq!(e.state().auto_play().unwrap().scope, item_scope(2));
        assert_eq!(at(&mut e, Command::LoopSection, 100), Ok(Outcome::Changed));
        assert_eq!(e.state().auto_play().unwrap().scope, section_scope(1));
        assert_eq!(
            at(&mut e, Command::ToggleAutoPlay, 200),
            Ok(Outcome::Changed)
        );
        assert_eq!(e.state().auto_play(), None);
        assert_eq!(
            at(&mut e, Command::StopAutoPlay, 300),
            Ok(Outcome::Unchanged)
        );
    }

    #[test]
    fn one_cue_loop_restarts_its_clock_without_a_change() {
        let mut e = engine();
        at(&mut e, Command::GoTo(12), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        let revision = e.state().revision();
        assert_eq!(e.tick(Millis(2_000)), Outcome::Unchanged);
        assert_eq!(e.state().revision(), revision);
        assert_eq!(e.state().auto_play().unwrap().cue_started, Millis(2_000));
        assert_eq!(e.tick(Millis(3_999)), Outcome::Unchanged);
        assert_eq!(e.state().auto_play().unwrap().cue_started, Millis(2_000));
    }

    #[test]
    fn ineligible_scopes_are_refused_with_a_reason() {
        let cases = [
            (
                0,
                Command::ToggleAutoPlay,
                AutoPlayRefusal::NotGeneralPresentation,
            ),
            (2, Command::ToggleAutoPlay, AutoPlayRefusal::NoDurations),
            (
                10,
                Command::ToggleAutoPlay,
                AutoPlayRefusal::NothingProjectable,
            ),
            (
                0,
                Command::LoopSection,
                AutoPlayRefusal::SectionHasIneligibleItem,
            ),
            (
                13,
                Command::LoopSection,
                AutoPlayRefusal::NothingProjectable,
            ),
        ];
        for (flat, command, reason) in cases {
            let mut e = engine();
            at(&mut e, Command::GoTo(flat), 0).unwrap();
            let state = e.state().clone();
            let refusal = Refusal::AutoPlayIneligible { reason };
            assert_eq!(e.check(command, Millis(5)), Err(refusal));
            assert_eq!(e.apply(command, Millis(5)), Err(refusal));
            assert_eq!(e.state(), &state);
            assert_eq!(e.last_now(), Millis(0));
        }
    }

    #[test]
    fn explicit_stops_and_other_events_end_auto_play_for_good() {
        let events = [
            Command::StopAutoPlay,
            Command::ToggleAutoPlay,
            Command::GoTo(0),
            Command::First,
            Command::Last,
            Command::NextItem,
            Command::PreviousItem,
            Command::ToggleFreeze,
            Command::ToggleBlackout,
            Command::DisableOutput,
            Command::DisplayLost,
            Command::Recover,
        ];
        for event in events {
            let mut e = live_at(3);
            at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
            assert_eq!(at(&mut e, event, 100), Ok(Outcome::Changed), "{event:?}");
            assert_eq!(e.state().auto_play(), None, "{event:?}");
            // Nothing resumes it silently: releasing, returning, or time.
            for release in [
                Command::ToggleFreeze,
                Command::ToggleBlackout,
                Command::EnableOutput,
                Command::DisplayReturned { recognized: true },
                Command::Take,
            ] {
                let _ = at(&mut e, release, 200);
            }
            e.tick(Millis(1_000_000));
            assert_eq!(e.state().auto_play(), None, "{event:?}");
        }
    }

    #[test]
    fn recover_changes_nothing_else() {
        let mut e = live_at(3);
        assert_eq!(at(&mut e, Command::Recover, 10), Ok(Outcome::Unchanged));
        at(&mut e, Command::ToggleAutoPlay, 20).unwrap();
        let (live, preview, program) = (e.state().live(), e.state().preview(), e.state().program());
        at(&mut e, Command::Recover, 30).unwrap();
        assert_eq!(
            (e.state().live(), e.state().preview(), e.state().program()),
            (live, preview, program)
        );
    }

    #[test]
    fn freeze_and_blackout_block_starting() {
        for command in [Command::ToggleFreeze, Command::ToggleBlackout] {
            let mut e = live_at(3);
            at(&mut e, command, 0).unwrap();
            let state = e.state().clone();
            assert_eq!(
                e.check(Command::ToggleAutoPlay, Millis(5)),
                Err(Refusal::AutoPlayBlocked)
            );
            assert_eq!(
                e.apply(Command::LoopSection, Millis(5)),
                Err(Refusal::AutoPlayBlocked)
            );
            assert_eq!(e.state(), &state);
        }
    }

    #[test]
    fn rehearsal_and_idle_states_can_start_auto_play() {
        let mut disabled = engine();
        at(&mut disabled, Command::GoTo(3), 0).unwrap();
        at(&mut disabled, Command::ToggleAutoPlay, 0).unwrap();
        assert!(disabled.state().auto_play().is_some());
        // Enabling the output while running does not stop it.
        at(&mut disabled, Command::EnableOutput, 10).unwrap();
        assert!(disabled.state().auto_play().is_some());

        let mut black = engine();
        at(&mut black, Command::EnableOutput, 0).unwrap();
        at(&mut black, Command::GoTo(3), 0).unwrap();
        at(&mut black, Command::ToggleAutoPlay, 0).unwrap();
        assert!(black.state().auto_play().is_some());
    }

    #[test]
    fn a_late_tick_advances_once() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        assert_eq!(e.tick(Millis(100_000)), Outcome::Changed);
        assert_eq!(e.state().preview(), 4);
        assert_eq!(e.state().auto_play().unwrap().cue_started, Millis(100_000));
        assert_eq!(e.tick(Millis(100_001)), Outcome::Unchanged);
        assert_eq!(e.state().preview(), 4);
    }

    #[test]
    fn tick_without_auto_play_changes_nothing_and_settles_fades() {
        let mut e = live_at(3);
        fat(&mut e, Command::ToggleBlackout, 0).unwrap();
        let revision = e.state().revision();
        assert_eq!(e.tick(Millis(T0 + 600)), Outcome::Unchanged);
        assert_eq!(e.state().revision(), revision);
        assert_eq!(fview(&e, 600).shown, None);
    }

    #[test]
    fn disable_output_and_display_loss_drop_a_running_fade_out() {
        for command in [Command::DisableOutput, Command::DisplayLost] {
            let mut e = live_at(3);
            fat(&mut e, Command::ToggleBlackout, 0).unwrap();
            assert!(fview(&e, 50).fading);
            fat(&mut e, command, 100).unwrap();
            assert_eq!(
                fview(&e, 100),
                ProgramView {
                    shown: None,
                    opacity: 0.0,
                    fading: false
                },
                "{command:?}"
            );
        }
    }

    #[test]
    fn every_black_to_content_change_fades_in() {
        // First Take from an open, black output.
        let mut e = engine();
        at(&mut e, Command::EnableOutput, T0).unwrap();
        at(&mut e, Command::GoTo(3), T0).unwrap();
        at(&mut e, Command::Take, T0).unwrap();
        let view = fview(&e, 250);
        assert_eq!(view.shown, Some(frame(10)));
        approx(view.opacity, 0.5);
        assert!(view.fading);

        // A recreated window after Decision 272 resume.
        let mut e = live_at(3);
        fat(&mut e, Command::DisplayLost, 0).unwrap();
        fat(&mut e, Command::DisplayReturned { recognized: true }, 100).unwrap();
        assert_eq!(fview(&e, 100).shown, None);
        fat(&mut e, Command::Take, 200).unwrap();
        let view = fview(&e, 450);
        assert_eq!(view.shown, Some(frame(10)));
        approx(view.opacity, 0.5);
        assert_eq!(fview(&e, 700).opacity, 1.0);
    }

    #[test]
    fn a_fade_in_reverses_back_to_black_from_where_it_is() {
        let mut e = engine();
        at(&mut e, Command::EnableOutput, T0).unwrap();
        at(&mut e, Command::GoTo(3), T0).unwrap();
        at(&mut e, Command::Take, T0).unwrap();
        // 100 ms into the 500 ms fade-in the opacity is 0.2.
        approx(fview(&e, 100).opacity, 0.2);
        fat(&mut e, Command::ToggleBlackout, 100).unwrap();
        let reversed = fview(&e, 100);
        assert_eq!(reversed.shown, Some(frame(10)));
        approx(reversed.opacity, 0.2);
        // The fade-out takes 0.2 * 500 ms = 100 ms.
        approx(fview(&e, 150).opacity, 0.1);
        let done = fview(&e, 200);
        assert_eq!(
            done,
            ProgramView {
                shown: None,
                opacity: 0.0,
                fading: false
            }
        );
    }

    #[test]
    fn hold_to_content_by_linked_navigation_fades_in() {
        let mut e = live_at(8);
        assert_eq!(fview(&e, 0).shown, None);
        fat(&mut e, Command::Previous, 0).unwrap();
        let view = fview(&e, 250);
        assert_eq!(view.shown, Some(frame(20)));
        approx(view.opacity, 0.5);
        assert!(view.fading);
    }

    #[test]
    fn the_preview_monitor_reverses_mid_fade_too() {
        let mut e = engine();
        // Cue 5 is a trigger: Preview fades out, then reverses at 0.5.
        at(&mut e, Command::GoTo(5), T0).unwrap();
        approx(preview_of(&e, T0 + 250).opacity, 0.5);
        at(&mut e, Command::GoTo(0), T0 + 250).unwrap();
        let reversed = preview_of(&e, T0 + 250);
        assert_eq!(reversed.shown, Some(frame(0)));
        approx(reversed.opacity, 0.5);
        approx(preview_of(&e, T0 + 375).opacity, 0.75);
        let done = preview_of(&e, T0 + 500);
        assert_eq!((done.opacity, done.fading), (1.0, false));
    }

    #[test]
    fn next_deadline_is_the_earliest_fade_end_or_auto_play_due_time() {
        let mut e = engine();
        assert_eq!(e.next_deadline(), None);
        at(&mut e, Command::EnableOutput, 0).unwrap();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::Take, 0).unwrap();
        // The Take fade-in ends at 500.
        assert_eq!(e.next_deadline(), Some(Millis(500)));
        e.tick(Millis(T0));
        assert_eq!(e.next_deadline(), None);

        fat(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        assert_eq!(e.next_deadline(), Some(Millis(T0 + 8_000)));
        fat(&mut e, Command::Next, 100).unwrap();
        assert_eq!(e.next_deadline(), Some(Millis(T0 + 3_100)));
        // Onto the in-scope trigger: both monitors fade out to 700, and the
        // run has no deadline of its own.
        fat(&mut e, Command::Next, 200).unwrap();
        assert_eq!(e.next_deadline(), Some(Millis(T0 + 700)));
        // Reversing at 0.8 makes the fade-in short; it ends before the due time.
        fat(&mut e, Command::Next, 300).unwrap();
        assert_eq!(e.next_deadline(), Some(Millis(T0 + 400)));
        e.tick(Millis(T0 + 500));
        assert_eq!(e.next_deadline(), Some(Millis(T0 + 8_300)));
        // An overdue auto-play deadline stays visible until ticked.
        assert_eq!(e.tick(Millis(T0 + 9_000)), Outcome::Changed);
        assert_eq!(e.next_deadline(), Some(Millis(T0 + 17_000)));
        e.apply(Command::StopAutoPlay, Millis(T0 + 9_100)).unwrap();
        assert_eq!(e.next_deadline(), None);
    }

    #[test]
    fn in_scope_cues_that_cannot_advance_keep_the_run_without_a_deadline() {
        // Item scope: Cue 5 is an in-scope trigger.
        let mut e = engine();
        at(&mut e, Command::GoTo(4), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        at(&mut e, Command::Next, 1_000).unwrap();
        assert_eq!(e.state().preview(), 5);
        let run = e.state().auto_play().expect("the run survives");
        assert_eq!((run.scope, run.duration_ms), (item_scope(2), None));
        // Only Preview's fade-out is pending: the run itself has no deadline.
        assert_eq!(e.next_deadline(), Some(Millis(1_500)));
        // Time does nothing until the operator moves again.
        assert_eq!(e.tick(Millis(1_000_000)), Outcome::Unchanged);
        assert_eq!(e.next_deadline(), None);
        assert_eq!(e.state().preview(), 5);
        at(&mut e, Command::Next, 2_000_000).unwrap();
        let run = e.state().auto_play().unwrap();
        assert_eq!(
            (run.cue_started, run.duration_ms),
            (Millis(2_000_000), Some(8_000))
        );
        assert_eq!(e.tick(Millis(2_008_000)), Outcome::Changed);
        // The automatic advance still skips the trigger: 6 wraps to 3.
        assert_eq!(e.state().preview(), 3);

        // Hold and unavailable Cues of the item are in scope too.
        let mut e = engine();
        at(&mut e, Command::GoTo(7), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        for flat in [8, 9] {
            at(&mut e, Command::Next, 100).unwrap();
            assert_eq!(e.state().preview(), flat);
            assert_eq!(e.state().auto_play().unwrap().duration_ms, None);
        }
        at(&mut e, Command::Previous, 200).unwrap();
        at(&mut e, Command::Previous, 200).unwrap();
        assert_eq!(e.state().auto_play().unwrap().duration_ms, Some(4_000));
    }

    #[test]
    fn auto_play_cannot_start_while_disconnected() {
        let mut e = live_at(3);
        at(&mut e, Command::DisplayLost, 10).unwrap();
        for back in [false, true] {
            if back {
                at(&mut e, Command::DisplayReturned { recognized: true }, 20).unwrap();
            }
            let state = e.state().clone();
            for command in [Command::ToggleAutoPlay, Command::LoopSection] {
                assert_eq!(e.check(command, Millis(30)), Err(Refusal::AutoPlayBlocked));
                assert_eq!(e.apply(command, Millis(30)), Err(Refusal::AutoPlayBlocked));
            }
            assert_eq!(e.state(), &state);
        }
    }

    #[test]
    fn disabling_the_output_ends_a_rehearsal_run() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        assert_eq!(at(&mut e, Command::DisableOutput, 10), Ok(Outcome::Changed));
        assert_eq!(e.state().auto_play(), None);
    }

    #[test]
    fn navigation_landing_outside_an_item_scope_stops_the_run() {
        // Cues 3..=6 are item 2.
        for (start, command) in [
            (6, Command::Next),
            (3, Command::Previous),
            (4, Command::GoTo(7)),
        ] {
            let mut e = engine();
            at(&mut e, Command::GoTo(start), 0).unwrap();
            at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
            at(&mut e, command, 10).unwrap();
            assert_eq!(e.state().auto_play(), None, "{command:?} from {start}");
        }
    }

    #[test]
    fn a_section_run_survives_other_items_of_the_section_and_stops_outside() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::LoopSection, 0).unwrap();
        // Item 3, then item 4 (a trigger, no deadline): still the section run.
        at(&mut e, Command::NextItem, 100).unwrap();
        assert_eq!(e.state().preview(), 7);
        assert_eq!(e.state().auto_play().unwrap().scope, section_scope(1));
        assert_eq!(e.state().auto_play().unwrap().duration_ms, Some(4_000));
        at(&mut e, Command::NextItem, 200).unwrap();
        assert_eq!(e.state().preview(), 10);
        assert_eq!(e.state().auto_play().unwrap().scope, section_scope(1));
        assert_eq!(e.state().auto_play().unwrap().duration_ms, None);
        // Section 2 is outside.
        at(&mut e, Command::NextItem, 300).unwrap();
        assert_eq!(e.state().preview(), 12);
        assert_eq!(e.state().auto_play(), None);
    }

    #[test]
    fn previous_within_scope_resets_the_timer() {
        let mut e = engine();
        at(&mut e, Command::GoTo(4), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        at(&mut e, Command::Previous, 2_000).unwrap();
        let run = e.state().auto_play().unwrap();
        assert_eq!(e.state().preview(), 3);
        assert_eq!(
            (run.cue_started, run.duration_ms),
            (Millis(2_000), Some(8_000))
        );
        assert_eq!(e.tick(Millis(9_999)), Outcome::Unchanged);
        assert_eq!(e.tick(Millis(10_000)), Outcome::Changed);
    }

    #[test]
    fn widening_a_run_keeps_the_current_cues_clock() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        at(&mut e, Command::ToggleAutoPlay, 0).unwrap();
        at(&mut e, Command::Next, 5_000).unwrap();
        at(&mut e, Command::LoopSection, 6_000).unwrap();
        let run = e.state().auto_play().unwrap();
        assert_eq!(run.scope, section_scope(1));
        assert_eq!(
            (run.cue_started, run.duration_ms),
            (Millis(5_000), Some(3_000))
        );
        assert_eq!(e.tick(Millis(7_999)), Outcome::Unchanged);
        assert_eq!(e.tick(Millis(8_000)), Outcome::Changed);
        assert_eq!(e.state().preview(), 6);
    }

    #[test]
    fn section_eligibility_counts_unavailable_content() {
        let failed = |n: u16| {
            cue(
                n,
                None,
                CueBehavior::Unavailable {
                    reason: "no font".to_owned(),
                },
            )
        };
        // A Song whose slides all failed still blocks "Loop this section".
        let mut e = build(vec![vec![
            item(0, MassItemKind::Song, None, vec![failed(0)]),
            gp(1, 5, vec![project(1, None)]),
        ]]);
        let reason = AutoPlayRefusal::SectionHasIneligibleItem;
        assert_eq!(
            at(&mut e, Command::LoopSection, 0),
            Err(Refusal::AutoPlayIneligible { reason })
        );
        // An auto-play item with only failed Cues is not itself a blocker, but
        // it does not make the section eligible either.
        let mut e = build(vec![vec![gp(0, 5, vec![failed(0)])]]);
        assert_eq!(
            at(&mut e, Command::LoopSection, 0),
            Err(Refusal::AutoPlayIneligible {
                reason: AutoPlayRefusal::NothingProjectable
            })
        );
        let mut e = build(vec![vec![
            gp(0, 5, vec![failed(0)]),
            gp(1, 5, vec![project(1, None)]),
        ]]);
        assert_eq!(at(&mut e, Command::LoopSection, 0), Ok(Outcome::Changed));
        assert_eq!(e.state().preview(), 1);
    }

    #[test]
    fn check_matches_apply_for_the_auto_play_commands() {
        let setups: [&[Command]; 4] = [
            &[],
            &[Command::GoTo(3), Command::ToggleAutoPlay],
            &[Command::GoTo(3), Command::LoopSection],
            &[Command::GoTo(0)],
        ];
        for setup in setups {
            for command in [
                Command::LoopSection,
                Command::StopAutoPlay,
                Command::Recover,
                Command::ToggleAutoPlay,
            ] {
                let mut e = engine();
                for c in setup {
                    at(&mut e, *c, 0).unwrap();
                }
                let before = e.state().clone();
                let predicted = e.check(command, Millis(50));
                assert_eq!(e.state(), &before);
                assert_eq!(e.last_now(), Millis(0));
                assert_eq!(
                    predicted,
                    e.apply(command, Millis(50)),
                    "{command:?} after {setup:?}"
                );
            }
        }
    }

    #[test]
    fn check_never_changes_the_engine() {
        let mut e = engine();
        at(&mut e, Command::GoTo(3), 0).unwrap();
        assert_eq!(
            e.check(Command::ToggleAutoPlay, Millis(9)),
            Ok(Outcome::Changed)
        );
        assert_eq!(e.state().auto_play(), None);
        assert_eq!(e.last_now(), Millis(0));
    }
}
