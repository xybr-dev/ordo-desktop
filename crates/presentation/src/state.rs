//! Observable Presentation Engine state: what the operator and the
//! congregation can currently see.
//!
//! These are plain values. Every transition lives in [`crate::engine`], so a
//! view can read state without being able to invent a state the Engine never
//! reached.

use domain::presentation::{FrameId, MassItemId, MassSectionId};

use crate::journal::LiveIntent;

/// A monotonic timestamp in milliseconds.
///
/// The Engine never reads a clock. The caller supplies the time of every
/// command, which keeps replay deterministic: the same snapshot and the same
/// (command, time) sequence always produce the same states.
///
/// Time must not run backwards, but the Engine does not trust the caller: it
/// clamps each command's time to the latest time it has already seen, so a
/// late or reordered timestamp can never move the Engine's clock back.
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

/// Milliseconds a full fade between black and content takes
/// (`output-blank-duration: 500ms` in DESIGN.md). A partial fade takes the
/// matching fraction of it.
pub const FADE_MS: u64 = 500;

/// What bounds one auto-play run (MVP-PLAN section 8.4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutoPlayScope {
    /// The projectable Cues of one auto-play General Presentation item.
    Item(MassItemId),
    /// The projectable Cues of every item in one Mass section.
    Section(MassSectionId),
}

/// An active auto-play run: its scope and the clock of the Cue it is showing.
///
/// The timing fields are bookkeeping, not observable state: the Engine's
/// change detection compares only the scope (see
/// [`EngineState::same_observable`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutoPlayStatus {
    /// What the run is confined to.
    pub scope: AutoPlayScope,
    /// When the current Cue started showing (the Engine's clamped time).
    pub cue_started: Millis,
    /// How long the current Cue shows before auto-play advances. `None` while
    /// Preview sits on an in-scope Cue that cannot advance (trigger, Hold,
    /// Wait, unavailable): the run stays alive but has no deadline until the
    /// operator moves onto a projectable Cue again.
    pub duration_ms: Option<u64>,
}

/// The Program (or Preview) monitor at one instant: what is drawn and how
/// opaque it is. The Engine state changes immediately; the fade is only this
/// view of it over time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgramView {
    /// The frame being drawn: the outgoing frame while fading to black, the
    /// incoming one while fading in. `None` is solid black.
    pub shown: Option<FrameId>,
    /// Opacity of `shown` over black, `0.0..=1.0`. `0.0` whenever `shown` is
    /// `None`.
    pub opacity: f32,
    /// True while a fade is still running.
    pub fading: bool,
}

/// One linear opacity ramp of `frame` over black.
#[derive(Clone, Copy, Debug)]
struct Fade {
    frame: FrameId,
    from: f32,
    to: f32,
    start: Millis,
    duration_ms: u64,
}

impl Fade {
    fn elapsed(&self, now: Millis) -> u64 {
        now.0.saturating_sub(self.start.0)
    }

    fn finished(&self, now: Millis) -> bool {
        self.elapsed(now) >= self.duration_ms
    }

    fn opacity_at(&self, now: Millis) -> f32 {
        if self.finished(now) {
            return self.to;
        }
        let progress = self.elapsed(now) as f64 / self.duration_ms as f64;
        let opacity = f64::from(self.from) + f64::from(self.to - self.from) * progress;
        (opacity as f32).clamp(0.0, 1.0)
    }
}

/// The fade of one monitor. It is derived presentation state: it never
/// enters [`EngineState`] equality and never delays a command.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FadeTimeline {
    fade: Option<Fade>,
}

impl FadeTimeline {
    /// The monitor at `now`. `steady` is what the Engine state shows once any
    /// fade is over.
    pub(crate) fn view(&self, steady: Option<FrameId>, now: Millis) -> ProgramView {
        match self.fade {
            Some(fade) if !fade.finished(now) => ProgramView {
                shown: Some(fade.frame),
                opacity: fade.opacity_at(now),
                fading: true,
            },
            _ => ProgramView {
                shown: steady,
                opacity: if steady.is_some() { 1.0 } else { 0.0 },
                fading: false,
            },
        }
    }

    /// Drops a fade that has completed.
    pub(crate) fn settle(&mut self, now: Millis) {
        if self.fade.is_some_and(|fade| fade.finished(now)) {
            self.fade = None;
        }
    }

    /// The shown frame went from `old` to `new` at `now`.
    ///
    /// * `instant`: the window is gone, so any fade is dropped, even when the
    ///   shown frame did not change (a fade-out was still running);
    /// * same frame (including black to black): nothing changes;
    /// * content to black: fade out from the current opacity over that
    ///   fraction of [`FADE_MS`];
    /// * black to content: fade in from the current opacity likewise, so a
    ///   contrary change mid-fade reverses from where it is, never from 0 or 1;
    /// * content to other content: a cut. Mid fade-in the new frame takes
    ///   over at the current opacity and the fade-in carries on to 1.0.
    pub(crate) fn changed(
        &mut self,
        old: Option<FrameId>,
        new: Option<FrameId>,
        now: Millis,
        instant: bool,
    ) {
        self.settle(now);
        // Checked before the equality test: a fade-out still running when the
        // window disappears has `old == new == None` yet must be dropped.
        if instant {
            self.fade = None;
            return;
        }
        if old == new {
            return;
        }
        let current = self.view(old, now);
        match (current.shown, new) {
            (Some(frame), None) => {
                self.fade = Some(Fade {
                    frame,
                    from: current.opacity,
                    to: 0.0,
                    start: now,
                    duration_ms: Self::scaled(current.opacity),
                });
            }
            (_, Some(frame)) if old.is_none() => {
                self.fade = Some(Fade {
                    frame,
                    from: current.opacity,
                    to: 1.0,
                    start: now,
                    duration_ms: Self::scaled(1.0 - current.opacity),
                });
            }
            (_, Some(frame)) => {
                if let Some(fade) = &mut self.fade {
                    fade.frame = frame;
                }
            }
            (None, None) => {}
        }
    }

    /// When the running fade ends, if there is one.
    pub(crate) fn end(&self) -> Option<Millis> {
        self.fade
            .map(|fade| Millis(fade.start.0.saturating_add(fade.duration_ms)))
    }

    /// The fade length for `fraction` of a full fade. Rounding only affects
    /// the length of the ramp: the end opacity is exact (`opacity_at` returns
    /// `to` once finished), so rounding never changes the settled state.
    fn scaled(fraction: f32) -> u64 {
        (f64::from(fraction) * FADE_MS as f64).round() as u64
    }
}

/// Private operator context of a session reopened after an unexpected end
/// (Decision 228). It never drives Program: it only names what Program showed
/// before the interruption, so the operator can recognise where they were
/// without Ordo reprojecting stale content.
///
/// It survives ordinary navigation and output changes, and is cleared when
/// the live state newly becomes `Live` (a successful `Take`, including a
/// Freeze or Blackout release; a Take on a Hold or Wait Cue clears it too
/// while Program stays black) or by `DisableOutput`, the ordinary safe exit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveredContext {
    /// Flat index of the last valid Program Cue, `None` when nothing had been
    /// live before the interruption.
    pub(crate) program: Option<usize>,
    /// The live state the session was in when it was interrupted.
    pub(crate) interrupted: LiveIntent,
}

impl RecoveredContext {
    /// Flat index of the last valid Program Cue before the interruption.
    pub fn program(&self) -> Option<usize> {
        self.program
    }

    /// The live state the session was in when it was interrupted.
    pub fn interrupted(&self) -> LiveIntent {
        self.interrupted
    }
}

/// The Engine's complete state after one command.
///
/// Invariants: while `live` is `OutputDisabled`, `EnabledBlack`, `Blackout`, or
/// `Disconnected`, `program` is [`ProgramContent::Black`]; and
/// `unavailable_black` is true only while `live` is `Live`. Every transition
/// out of `Live` (Freeze, Blackout, Disable, display loss) clears it.
/// `recovered` is `Some` only while `live` is neither `Live` nor `Frozen`, so
/// Program is black.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineState {
    pub(crate) live: LiveState,
    pub(crate) preview: usize,
    pub(crate) program: ProgramContent,
    pub(crate) unavailable_black: bool,
    pub(crate) auto_play: Option<AutoPlayStatus>,
    /// Set only by [`crate::engine::Engine::recover`]; see
    /// [`RecoveredContext`].
    pub(crate) recovered: Option<RecoveredContext>,
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
    /// Only ever true while the live state is `Live`.
    pub fn unavailable_black(&self) -> bool {
        self.unavailable_black
    }

    /// The active auto-play run, if any. Auto-play is bounded to its scope.
    /// Starting is refused while Frozen, in Blackout, or Disconnected, and
    /// entering any of those (or disabling the output) stops a run. A
    /// rehearsal run may exist while the output is disabled or black.
    pub fn auto_play(&self) -> Option<AutoPlayStatus> {
        self.auto_play
    }

    /// The private context of a recovered session, or `None` for an ordinary
    /// one. It is observable: clearing it after the first Take is a real
    /// change, so the revision moves exactly once for that command.
    pub fn recovered(&self) -> Option<RecoveredContext> {
        self.recovered
    }

    /// Equality for change detection: everything but the revision and the
    /// auto-play clock, so a restarted timer alone is not a state change.
    pub(crate) fn same_observable(&self, other: &Self) -> bool {
        self.live == other.live
            && self.preview == other.preview
            && self.program == other.program
            && self.unavailable_black == other.unavailable_black
            && self.auto_play.map(|run| run.scope) == other.auto_play.map(|run| run.scope)
            && self.recovered == other.recovered
    }

    /// Counts state changes. It increments exactly once per
    /// [`crate::engine::Outcome::Changed`] and never otherwise.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}
