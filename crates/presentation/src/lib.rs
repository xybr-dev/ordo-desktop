//! Deterministic Presenter state and command handling.
//!
//! The Presentation Engine begins in T0.2. It depends on domain values and
//! completed canonical rendering values, never on GPUI views.

use std::sync::Arc;

/// A complete immutable RGBA Program frame ready for every presentation view.
///
/// The pixel extent travels with the bytes: density-dependent `width` and
/// `height` say whether the buffer holds `1920x1080` or a sharper sampling
/// of the same composition, so no consumer has to guess from the length.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramFrame {
    revision: u64,
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
    /// Set only by [`ProgramFrame::black`]: marks the intentional black frame,
    /// not a frame whose pixels merely happen to be black.
    black: bool,
}

impl ProgramFrame {
    /// Build a frame from explicit dimensions and row-major RGBA bytes.
    ///
    /// Rejects zero extents and empty buffers, non-RGBA lengths, and buffers
    /// whose length is not exactly `width * height * 4`.
    pub fn new(revision: u64, width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, FrameError> {
        if width == 0 || height == 0 || rgba.is_empty() {
            return Err(FrameError::Empty);
        }
        if !rgba.len().is_multiple_of(4) {
            return Err(FrameError::InvalidRgbaLength);
        }
        let expected = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(4));
        if expected != Some(rgba.len() as u64) {
            return Err(FrameError::DimensionMismatch {
                width,
                height,
                len: rgba.len(),
            });
        }
        Ok(Self {
            revision,
            width,
            height,
            rgba: rgba.into(),
            black: false,
        })
    }

    /// Feed a rasterized canonical frame into the Program seam, moving its
    /// density-dependent dimensions and bytes across without copying them.
    pub fn from_raster(
        revision: u64,
        raster: rendering::RasterizedFrame,
    ) -> Result<Self, FrameError> {
        let (width, height, rgba) = raster.into_parts();
        Self::new(revision, width, height, rgba)
    }

    /// A cheap 1x1 opaque black frame: what the congregation sees whenever
    /// output is not showing content.
    pub fn black(revision: u64) -> Self {
        Self {
            revision,
            width: 1,
            height: 1,
            rgba: Arc::from([0, 0, 0, 255].as_slice()),
            black: true,
        }
    }

    /// Whether this is the intentional black frame built by
    /// [`ProgramFrame::black`]. O(1): it reads a flag and never scans pixels.
    ///
    /// It does not mean "the pixels happen to be black": a black slide
    /// rasterized through [`ProgramFrame::from_raster`] or a frame built with
    /// [`ProgramFrame::new`] from black bytes is not `is_black`.
    pub fn is_black(&self) -> bool {
        self.black
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    Empty,
    InvalidRgbaLength,
    DimensionMismatch { width: u32, height: u32, len: usize },
}

/// Congregation-output lifecycle as the Presentation Engine sees it.
///
/// Only `Connected` lets content reach the congregation. `Disabled` is the
/// ordinary **Congregation output disabled** state (no output window);
/// `Disconnected` means the enabled display disappeared. In both non-connected
/// states the external Program is black and Preview stays usable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStatus {
    Disabled,
    Connected,
    Disconnected,
}

/// The complete state exposed to the passive congregation surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CongregationState {
    program: ProgramFrame,
    output_status: OutputStatus,
}

impl CongregationState {
    pub fn new(program: ProgramFrame, output_status: OutputStatus) -> Self {
        Self {
            program,
            output_status,
        }
    }

    pub fn program(&self) -> &ProgramFrame {
        &self.program
    }

    pub fn output_status(&self) -> OutputStatus {
        self.output_status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationCommand {
    /// Select the operator's Preview. Always accepted; never touches Program.
    SelectPreview(ProgramFrame),
    /// The `Space` action: take the current Preview to Program. Accepted only
    /// while output is `Connected`; this is also the restore path after a
    /// reconnect, which leaves Program black until it runs.
    TakePreviewLive,
    /// Select `frame` as Preview and take it live in one step. Same
    /// acceptance rule as [`PresentationCommand::TakePreviewLive`]; when
    /// rejected, Preview is left unchanged too.
    TakeLive(ProgramFrame),
    /// Explicitly enable congregation output from `Disabled` or
    /// `Disconnected`. Program becomes black first, regardless of what was
    /// live before. Enabling while already `Connected` is a no-op.
    EnableOutput,
    /// The enabled congregation display disappeared: `Connected` becomes
    /// `Disconnected` and Program drops to black. Preview is preserved.
    OutputDisconnected,
    /// Explicitly disable congregation output: status `Disabled`, Program
    /// black. Preview is preserved.
    DisableOutput,
    /// Compatibility mapping onto the lifecycle commands:
    /// `Connected` → [`EnableOutput`](PresentationCommand::EnableOutput),
    /// `Disconnected` → [`OutputDisconnected`](PresentationCommand::OutputDisconnected),
    /// `Disabled` → [`DisableOutput`](PresentationCommand::DisableOutput).
    SetOutputStatus(OutputStatus),
}

/// Why a command was rejected. A rejected command changes no state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRejected {
    /// Program changes need `Connected` output; the current status is given.
    OutputNotConnected(OutputStatus),
    /// `TakePreviewLive` ran before any Preview was selected.
    NoPreview,
    /// `OutputDisconnected` arrived while output was `Disabled`.
    OutputNotEnabled,
    /// The Program revision is already `u64::MAX`, so the next Program change
    /// cannot take a strictly newer revision. Nothing changed.
    RevisionExhausted,
}

/// In-memory owner of deterministic Preview, Program, and congregation-output
/// state.
///
/// Invariant: unless output is `Connected`, the external Program is black.
/// Program revisions strictly increase with every Program change so consumers
/// can always tell successive frames apart. A Program change that would need a
/// revision past `u64::MAX` is rejected with
/// [`CommandRejected::RevisionExhausted`] instead of reusing a revision.
pub struct PresentationEngine {
    congregation: CongregationState,
    preview: Option<ProgramFrame>,
}

impl PresentationEngine {
    /// Start with `program` on the congregation surface.
    ///
    /// When `output_status` is not `Connected` the given content is not
    /// external, so Program starts black at `program.revision()` to uphold
    /// the black-unless-connected invariant.
    pub fn new(program: ProgramFrame, output_status: OutputStatus) -> Self {
        let program = if output_status == OutputStatus::Connected {
            program
        } else {
            ProgramFrame::black(program.revision())
        };
        Self {
            congregation: CongregationState::new(program, output_status),
            preview: None,
        }
    }

    /// Presenter Mode's ordinary start: output disabled, black Program at
    /// revision 0, no Preview selected.
    pub fn disabled() -> Self {
        Self {
            congregation: CongregationState::new(ProgramFrame::black(0), OutputStatus::Disabled),
            preview: None,
        }
    }

    /// Apply `command`, returning the resulting congregation state or the
    /// typed reason it was rejected. A rejected command changes nothing;
    /// accepted no-ops (such as enabling twice) return `Ok`.
    pub fn try_apply(
        &mut self,
        command: PresentationCommand,
    ) -> Result<&CongregationState, CommandRejected> {
        let status = self.congregation.output_status;
        match command {
            PresentationCommand::SelectPreview(frame) => self.preview = Some(frame),
            PresentationCommand::TakePreviewLive => {
                if status != OutputStatus::Connected {
                    return Err(CommandRejected::OutputNotConnected(status));
                }
                let preview = self.preview.clone().ok_or(CommandRejected::NoPreview)?;
                self.publish(preview)?;
            }
            PresentationCommand::TakeLive(frame) => {
                if status != OutputStatus::Connected {
                    return Err(CommandRejected::OutputNotConnected(status));
                }
                // Publish first: a rejected take must leave Preview unchanged.
                self.publish(frame.clone())?;
                self.preview = Some(frame);
            }
            PresentationCommand::EnableOutput => {
                if status != OutputStatus::Connected {
                    self.black_out(OutputStatus::Connected)?;
                }
            }
            PresentationCommand::OutputDisconnected => match status {
                OutputStatus::Connected => self.black_out(OutputStatus::Disconnected)?,
                OutputStatus::Disconnected => {}
                OutputStatus::Disabled => return Err(CommandRejected::OutputNotEnabled),
            },
            PresentationCommand::DisableOutput => {
                if status != OutputStatus::Disabled {
                    self.black_out(OutputStatus::Disabled)?;
                }
            }
            PresentationCommand::SetOutputStatus(target) => {
                let mapped = match target {
                    OutputStatus::Connected => PresentationCommand::EnableOutput,
                    OutputStatus::Disconnected => PresentationCommand::OutputDisconnected,
                    OutputStatus::Disabled => PresentationCommand::DisableOutput,
                };
                return self.try_apply(mapped);
            }
        }
        Ok(&self.congregation)
    }

    /// Apply `command` and return the resulting congregation state.
    ///
    /// **Drops rejections silently.** A rejected command leaves state unchanged
    /// and this method does not report that. It is kept for compatibility with
    /// callers that never react to a rejection. Lifecycle callers must use
    /// [`try_apply`](Self::try_apply) so `OutputNotConnected`, `NoPreview`,
    /// `OutputNotEnabled`, and `RevisionExhausted` are handled, not lost.
    pub fn apply(&mut self, command: PresentationCommand) -> &CongregationState {
        let _ = self.try_apply(command);
        &self.congregation
    }

    pub fn congregation(&self) -> &CongregationState {
        &self.congregation
    }

    /// The operator's current Preview, preserved across every output change.
    pub fn preview(&self) -> Option<&ProgramFrame> {
        self.preview.as_ref()
    }

    /// The smallest revision strictly newer than the current Program, or
    /// `RevisionExhausted` when the current Program is already at `u64::MAX`.
    fn next_revision(&self) -> Result<u64, CommandRejected> {
        self.congregation
            .program
            .revision
            .checked_add(1)
            .ok_or(CommandRejected::RevisionExhausted)
    }

    /// Put `frame` on Program, restamping its revision when needed so the
    /// Program revision strictly increases. Mutates nothing when rejected.
    fn publish(&mut self, mut frame: ProgramFrame) -> Result<(), CommandRejected> {
        frame.revision = frame.revision.max(self.next_revision()?);
        self.congregation.program = frame;
        Ok(())
    }

    /// Set Program black under a fresh revision and the new status. Mutates
    /// nothing when rejected.
    fn black_out(&mut self, status: OutputStatus) -> Result<(), CommandRejected> {
        let revision = self.next_revision()?;
        self.congregation.program = ProgramFrame::black(revision);
        self.congregation.output_status = status;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_program_frames_replace_atomically() {
        let black = ProgramFrame::new(1, 1, 1, vec![0, 0, 0, 255]).unwrap();
        let live = ProgramFrame::new(2, 1, 1, vec![255, 255, 255, 255]).unwrap();
        let mut engine = PresentationEngine::new(black, OutputStatus::Connected);

        let state = engine.apply(PresentationCommand::TakeLive(live));

        assert_eq!(state.program().revision(), 2);
        assert_eq!(state.program().rgba(), &[255, 255, 255, 255]);
        assert_eq!(state.output_status(), OutputStatus::Connected);

        // T0.5: disconnection no longer leaves the last live frame claimed as
        // visible; Program drops to black under a newer revision.
        let state = engine.apply(PresentationCommand::SetOutputStatus(
            OutputStatus::Disconnected,
        ));
        assert_eq!(state.program().revision(), 3);
        assert!(state.program().is_black());
        assert_eq!(state.output_status(), OutputStatus::Disconnected);
    }

    fn frame(revision: u64, value: u8) -> ProgramFrame {
        ProgramFrame::new(revision, 1, 1, vec![value, value, value, 255]).unwrap()
    }

    fn live_engine(content: &ProgramFrame) -> PresentationEngine {
        let mut engine = PresentationEngine::disabled();
        engine.try_apply(PresentationCommand::EnableOutput).unwrap();
        engine
            .try_apply(PresentationCommand::TakeLive(content.clone()))
            .unwrap();
        engine
    }

    #[test]
    fn presenter_mode_starts_disabled_and_black() {
        let engine = PresentationEngine::disabled();
        let state = engine.congregation();
        assert_eq!(state.output_status(), OutputStatus::Disabled);
        assert!(state.program().is_black());
        assert_eq!(engine.preview(), None);
    }

    #[test]
    fn enabling_output_is_black_first_even_after_prior_live_content() {
        let content = frame(10, 200);
        let mut engine = live_engine(&content);
        assert_eq!(engine.congregation().program().rgba(), content.rgba());

        engine
            .try_apply(PresentationCommand::DisableOutput)
            .unwrap();
        let state = engine.try_apply(PresentationCommand::EnableOutput).unwrap();

        assert_eq!(state.output_status(), OutputStatus::Connected);
        assert!(state.program().is_black());
        assert_eq!(engine.preview(), Some(&content));
    }

    #[test]
    fn enabling_twice_does_not_black_out_live_content() {
        let content = frame(10, 200);
        let mut engine = live_engine(&content);
        let before = engine.congregation().clone();

        let state = engine.try_apply(PresentationCommand::EnableOutput).unwrap();

        assert_eq!(state, &before);
        assert_eq!(state.program().rgba(), content.rgba());
    }

    #[test]
    fn disconnect_blacks_program_and_preserves_preview() {
        let content = frame(10, 200);
        let mut engine = live_engine(&content);
        let next = frame(11, 90);
        engine
            .try_apply(PresentationCommand::SelectPreview(next.clone()))
            .unwrap();

        let state = engine
            .try_apply(PresentationCommand::OutputDisconnected)
            .unwrap();

        assert_eq!(state.output_status(), OutputStatus::Disconnected);
        assert!(state.program().is_black());
        assert_eq!(engine.preview(), Some(&next));
    }

    #[test]
    fn reconnect_stays_black_until_explicit_restore() {
        let content = frame(10, 200);
        let mut engine = live_engine(&content);
        engine
            .try_apply(PresentationCommand::OutputDisconnected)
            .unwrap();

        let state = engine.try_apply(PresentationCommand::EnableOutput).unwrap();
        assert_eq!(state.output_status(), OutputStatus::Connected);
        assert!(
            state.program().is_black(),
            "reconnect never resumes content"
        );

        let state = engine
            .try_apply(PresentationCommand::TakePreviewLive)
            .unwrap();
        assert_eq!(state.program().rgba(), content.rgba());
    }

    #[test]
    fn program_changes_are_rejected_unless_connected() {
        let content = frame(10, 200);
        let mut engine = PresentationEngine::disabled();
        engine
            .try_apply(PresentationCommand::SelectPreview(content.clone()))
            .unwrap();
        let before = engine.congregation().clone();

        assert_eq!(
            engine.try_apply(PresentationCommand::TakePreviewLive),
            Err(CommandRejected::OutputNotConnected(OutputStatus::Disabled))
        );
        assert_eq!(
            engine.try_apply(PresentationCommand::TakeLive(frame(12, 7))),
            Err(CommandRejected::OutputNotConnected(OutputStatus::Disabled))
        );
        assert_eq!(engine.congregation(), &before);
        assert_eq!(
            engine.preview(),
            Some(&content),
            "rejection changes nothing"
        );

        let mut engine = live_engine(&content);
        engine
            .try_apply(PresentationCommand::OutputDisconnected)
            .unwrap();
        let before = engine.congregation().clone();
        assert_eq!(
            engine.try_apply(PresentationCommand::TakePreviewLive),
            Err(CommandRejected::OutputNotConnected(
                OutputStatus::Disconnected
            ))
        );
        assert_eq!(engine.congregation(), &before);
    }

    #[test]
    fn restore_without_preview_and_disconnect_while_disabled_are_typed() {
        let mut engine = PresentationEngine::disabled();
        assert_eq!(
            engine.try_apply(PresentationCommand::OutputDisconnected),
            Err(CommandRejected::OutputNotEnabled)
        );
        engine.try_apply(PresentationCommand::EnableOutput).unwrap();
        assert_eq!(
            engine.try_apply(PresentationCommand::TakePreviewLive),
            Err(CommandRejected::NoPreview)
        );
    }

    #[test]
    fn program_revisions_increase_monotonically() {
        // Content revisions lower than the current Program are restamped.
        let content = frame(1, 200);
        let mut engine = PresentationEngine::disabled();
        let mut last = engine.congregation().program().revision();
        let mut step = |engine: &mut PresentationEngine, command| {
            engine.try_apply(command).unwrap();
            let revision = engine.congregation().program().revision();
            assert!(revision > last, "{revision} must exceed {last}");
            last = revision;
        };

        step(&mut engine, PresentationCommand::EnableOutput);
        step(&mut engine, PresentationCommand::TakeLive(content.clone()));
        step(&mut engine, PresentationCommand::OutputDisconnected);
        step(&mut engine, PresentationCommand::EnableOutput);
        step(&mut engine, PresentationCommand::TakePreviewLive);
        step(&mut engine, PresentationCommand::TakePreviewLive);
        step(&mut engine, PresentationCommand::DisableOutput);
    }

    #[test]
    fn revisions_stop_at_u64_max_with_typed_rejection_and_no_state_change() {
        // The last representable revision is still reachable...
        let mut engine = PresentationEngine::new(frame(u64::MAX - 1, 200), OutputStatus::Connected);
        engine
            .try_apply(PresentationCommand::SelectPreview(frame(1, 90)))
            .unwrap();
        let state = engine
            .try_apply(PresentationCommand::TakePreviewLive)
            .unwrap();
        assert_eq!(state.program().revision(), u64::MAX);

        // ...and nothing past it is: every Program change is rejected whole.
        let before = engine.congregation().clone();
        let preview = engine.preview().cloned();
        assert_eq!(
            engine.try_apply(PresentationCommand::TakePreviewLive),
            Err(CommandRejected::RevisionExhausted)
        );
        assert_eq!(
            engine.try_apply(PresentationCommand::TakeLive(frame(2, 7))),
            Err(CommandRejected::RevisionExhausted)
        );
        assert_eq!(
            engine.try_apply(PresentationCommand::OutputDisconnected),
            Err(CommandRejected::RevisionExhausted)
        );
        assert_eq!(
            engine.try_apply(PresentationCommand::DisableOutput),
            Err(CommandRejected::RevisionExhausted)
        );
        assert_eq!(engine.congregation(), &before);
        assert_eq!(engine.preview().cloned(), preview);

        // Enabling from Disabled needs a black Program under a new revision,
        // so it is rejected too and the status stays Disabled.
        let mut disabled =
            PresentationEngine::new(ProgramFrame::black(u64::MAX), OutputStatus::Disabled);
        assert_eq!(
            disabled.try_apply(PresentationCommand::EnableOutput),
            Err(CommandRejected::RevisionExhausted)
        );
        assert_eq!(
            disabled.congregation().output_status(),
            OutputStatus::Disabled
        );
        assert_eq!(disabled.congregation().program().revision(), u64::MAX);
    }

    #[test]
    fn black_is_a_structural_flag_not_a_pixel_scan() {
        // Black bytes built by hand are content, not the intentional black frame.
        let hand_built = ProgramFrame::new(1, 1, 1, vec![0, 0, 0, 255]).unwrap();
        assert!(!hand_built.is_black());
        assert!(ProgramFrame::black(1).is_black());

        // A black frame stays intentional when Program restamps its revision.
        let mut engine = live_engine(&frame(10, 200));
        let state = engine
            .try_apply(PresentationCommand::TakeLive(ProgramFrame::black(0)))
            .unwrap();
        assert_eq!(state.program().revision(), 11);
        assert!(state.program().is_black());
    }

    #[test]
    fn non_connected_construction_keeps_program_black() {
        let engine = PresentationEngine::new(frame(4, 200), OutputStatus::Disconnected);
        assert!(engine.congregation().program().is_black());
        assert_eq!(engine.congregation().program().revision(), 4);
    }

    #[test]
    fn incomplete_program_frames_are_rejected() {
        assert_eq!(
            ProgramFrame::new(1, 1, 1, Vec::new()),
            Err(FrameError::Empty)
        );
        assert_eq!(
            ProgramFrame::new(1, 0, 0, vec![0, 0, 0, 255]),
            Err(FrameError::Empty)
        );
        assert_eq!(
            ProgramFrame::new(1, 1, 1, vec![0, 0, 0]),
            Err(FrameError::InvalidRgbaLength)
        );
        assert_eq!(
            ProgramFrame::new(1, 2, 1, vec![0, 0, 0, 255]),
            Err(FrameError::DimensionMismatch {
                width: 2,
                height: 1,
                len: 4,
            })
        );
        // A dimension product that overflows is a mismatch, never a panic.
        assert_eq!(
            ProgramFrame::new(1, u32::MAX, u32::MAX, vec![0, 0, 0, 255]),
            Err(FrameError::DimensionMismatch {
                width: u32::MAX,
                height: u32::MAX,
                len: 4,
            })
        );
    }

    #[test]
    fn raster_feeds_program_with_its_dimensions() {
        use std::collections::HashMap;

        use rendering::{TargetDensity, TemplateContent, TemplateKind, Theme};

        let (frame, _) = rendering::render_template(
            &TemplateContent {
                kind: TemplateKind::Black,
                header: None,
                body: String::new(),
                footer: None,
                image: None,
            },
            &Theme::default(),
            TargetDensity::SCALE_100,
        )
        .unwrap();
        let raster = rendering::rasterize(&frame, TargetDensity::SCALE_100, &HashMap::new())
            .expect("black slide must rasterize");
        let program =
            ProgramFrame::from_raster(3, raster).expect("a real raster must feed the seam");

        assert_eq!(program.revision(), 3);
        assert_eq!((program.width(), program.height()), (1920, 1080));
        assert!(
            !program.is_black(),
            "a rasterized slide is content, not the intentional black frame"
        );
        assert!(
            program
                .rgba()
                .chunks_exact(4)
                .all(|pixel| pixel == [0, 0, 0, 255]),
            "a black slide feeds black bytes"
        );
    }
}
