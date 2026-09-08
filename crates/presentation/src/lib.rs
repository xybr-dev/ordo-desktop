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
    TakeLive(ProgramFrame),
    SetOutputStatus(OutputStatus),
}

/// In-memory owner of deterministic Program and congregation-output state.
pub struct PresentationEngine {
    congregation: CongregationState,
}

impl PresentationEngine {
    pub fn new(program: ProgramFrame, output_status: OutputStatus) -> Self {
        Self {
            congregation: CongregationState::new(program, output_status),
        }
    }

    pub fn apply(&mut self, command: PresentationCommand) -> &CongregationState {
        match command {
            PresentationCommand::TakeLive(program) => self.congregation.program = program,
            PresentationCommand::SetOutputStatus(status) => {
                self.congregation.output_status = status;
            }
        }
        &self.congregation
    }

    pub fn congregation(&self) -> &CongregationState {
        &self.congregation
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

        let state = engine.apply(PresentationCommand::SetOutputStatus(
            OutputStatus::Disconnected,
        ));
        assert_eq!(state.program().revision(), 2);
        assert_eq!(state.output_status(), OutputStatus::Disconnected);
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
            program
                .rgba()
                .chunks_exact(4)
                .all(|pixel| pixel == [0, 0, 0, 255]),
            "a black slide feeds black bytes"
        );
    }
}
