//! Deterministic Presenter state and command handling.
//!
//! The Presentation Engine begins in T0.2. It depends on domain values and
//! completed canonical rendering values, never on GPUI views.

use std::sync::Arc;

/// A complete immutable RGBA Program frame ready for every presentation view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramFrame {
    revision: u64,
    rgba: Arc<[u8]>,
}

impl ProgramFrame {
    pub fn new(revision: u64, rgba: Vec<u8>) -> Result<Self, FrameError> {
        if rgba.is_empty() {
            return Err(FrameError::Empty);
        }
        if !rgba.len().is_multiple_of(4) {
            return Err(FrameError::InvalidRgbaLength);
        }
        Ok(Self {
            revision,
            rgba: rgba.into(),
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    Empty,
    InvalidRgbaLength,
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
        let black = ProgramFrame::new(1, vec![0, 0, 0, 255]).unwrap();
        let live = ProgramFrame::new(2, vec![255, 255, 255, 255]).unwrap();
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
        assert_eq!(ProgramFrame::new(1, Vec::new()), Err(FrameError::Empty));
        assert_eq!(
            ProgramFrame::new(1, vec![0, 0, 0]),
            Err(FrameError::InvalidRgbaLength)
        );
    }
}
