//! The output controller: what the operator's intent means for the
//! congregation window, given a fresh [`Topology`] (T1.6 WP3).
//!
//! It wraps [`DisplayControl`] and holds no window types. The owner of the
//! window asks it what to do and does exactly that: open one passive window
//! on a placement, close it, or tell the operator why not. Refusals happen
//! here, before the Engine is called, because the Engine itself accepts
//! `EnableOutput` with no display (Decisions 48, 50 and 54).
//!
//! The order the owner follows on success is `Open` then `EnableOutput`, so
//! Program is black before anything can be taken live.

use super::{
    AssignError, DisplayControl, DisplayEvent, DisplayEvidence, DisplayId, DisplayStatus,
    EnableError, OutputLifecycle, OutputPlacement, Recognition, Topology,
};

/// Why output was not opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputRefusal {
    /// There is no display other than the operator display.
    NoDisplay,
    /// Several displays could carry output and none is assigned.
    SelectDisplay,
    /// The assigned display cannot be used now: it is missing, ambiguous, or
    /// the operator display cannot be told apart from the rest.
    Unavailable,
    /// Output was lost with its display, and the display now connected is not
    /// that one (Decision 272). Recovery goes through Select display.
    DifferentDisplay,
}

impl OutputRefusal {
    /// The operator-facing words.
    pub fn message(&self) -> &'static str {
        match self {
            Self::NoDisplay => "No congregation display found — rehearsing on Preview",
            Self::SelectDisplay => "Select the congregation display before enabling output",
            Self::Unavailable => "The assigned congregation display is not available",
            Self::DifferentDisplay => "A different display is connected — choose Select display",
        }
    }
}

/// What the owner of the congregation window must do.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputAction {
    Open(OutputPlacement),
    Close,
    Refuse(OutputRefusal),
}

/// Display Control plus the rules for enabling and disabling output.
#[derive(Clone, Debug)]
pub struct OutputController {
    control: DisplayControl,
}

impl OutputController {
    pub fn new(topology: Topology) -> Self {
        Self {
            control: DisplayControl::new(topology),
        }
    }

    /// Output as it stands after a restart that interrupted an enabled
    /// output: a display loss across the restart. The journaled `evidence`
    /// is assigned and the lifecycle is `Disconnected`, so output resumes
    /// only through [`Self::enable`] or [`Self::resume`] on the same
    /// recognized display (Decision 272), and refuses any other.
    pub fn recovered(topology: Topology, evidence: DisplayEvidence) -> Self {
        Self {
            control: DisplayControl::recovered(topology, evidence),
        }
    }

    pub fn control(&self) -> &DisplayControl {
        &self.control
    }

    /// Replaces the topology (see [`DisplayControl::refresh`]).
    pub fn refresh(&mut self, topology: Topology) -> Vec<DisplayEvent> {
        self.control.refresh(topology)
    }

    /// Assigns the congregation display. Never enables output.
    pub fn assign(&mut self, display: DisplayId) -> Result<(), AssignError> {
        self.control.assign(display)
    }

    /// Decides whether output can open on `topology`, a fresh snapshot.
    ///
    /// No candidate display refuses. With none assigned, one candidate is
    /// assigned and used, and several refuse. An assigned display that is
    /// missing refuses. The operator display is never opened on. On `Open`
    /// the lifecycle is already enabled; a caller whose window then fails to
    /// open must call [`Self::disable`].
    pub fn enable(&mut self, topology: Topology) -> OutputAction {
        if self.is_disconnected() {
            return self.resume(topology);
        }
        self.control.refresh(topology);
        let topology = self.control.topology();
        if topology.displays().is_empty() {
            return OutputAction::Refuse(OutputRefusal::NoDisplay);
        }
        if topology.operator().is_none() {
            // Without the operator display the others cannot be told apart
            // from it, so nothing may open.
            return OutputAction::Refuse(OutputRefusal::Unavailable);
        }
        let (first, several) = {
            let mut candidates = topology.candidates();
            (
                candidates.next().map(|display| display.id),
                candidates.next().is_some(),
            )
        };
        let Some(only) = first else {
            return OutputAction::Refuse(OutputRefusal::NoDisplay);
        };
        if self.control.status() == DisplayStatus::NotAssigned {
            if several {
                return OutputAction::Refuse(OutputRefusal::SelectDisplay);
            }
            if self.control.assign(only).is_err() {
                return OutputAction::Refuse(OutputRefusal::Unavailable);
            }
        }
        match self.control.enable_output() {
            Ok(placement) => OutputAction::Open(placement),
            Err(EnableError::NotAssigned) => OutputAction::Refuse(OutputRefusal::SelectDisplay),
            Err(_) => OutputAction::Refuse(OutputRefusal::Unavailable),
        }
    }

    /// Whether output was lost with its display and not yet resumed.
    pub fn is_disconnected(&self) -> bool {
        matches!(self.control.output(), OutputLifecycle::Disconnected { .. })
    }

    /// Resumes output lost with its display (Decision 272), given a fresh
    /// snapshot. Only the same recognized congregation display opens. Any
    /// other display refuses with `DifferentDisplay`, including one with the
    /// same resolution but another monitor identity, and the lifecycle stays
    /// disconnected. Without a connected candidate it refuses as
    /// `Unavailable`. Several matches refuse with `SelectDisplay`. Without a
    /// disconnected output this is [`Self::enable`].
    pub fn resume(&mut self, topology: Topology) -> OutputAction {
        if !self.is_disconnected() {
            return self.enable(topology);
        }
        self.control.refresh(topology);
        let topology = self.control.topology();
        if topology.operator().is_none() || topology.candidates().next().is_none() {
            return OutputAction::Refuse(OutputRefusal::Unavailable);
        }
        match self.control.recognition() {
            Recognition::Recognized(_) => match self.control.enable_output() {
                Ok(placement) => OutputAction::Open(placement),
                Err(_) => OutputAction::Refuse(OutputRefusal::Unavailable),
            },
            Recognition::Ambiguous(_) => OutputAction::Refuse(OutputRefusal::SelectDisplay),
            Recognition::Missing => OutputAction::Refuse(OutputRefusal::DifferentDisplay),
        }
    }

    /// Output is over: the window closes.
    pub fn disable(&mut self) -> OutputAction {
        self.control.disable_output();
        OutputAction::Close
    }

    /// Whether the controller believes output is enabled.
    pub fn is_enabled(&self) -> bool {
        matches!(self.control.output(), OutputLifecycle::Enabled { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::super::DetectedDisplay;
    use super::super::test_support::display;
    use super::*;

    fn topology(displays: Vec<DetectedDisplay>, operator: u64) -> Topology {
        Topology::new(displays, Some(DisplayId::new(operator)))
    }

    fn controller() -> OutputController {
        OutputController::new(Topology::default())
    }

    fn pair() -> Topology {
        topology(
            vec![display(1, 0, 0, 1920, 1080), display(2, 1920, 0, 1280, 720)],
            1,
        )
    }

    #[test]
    fn no_candidate_display_refuses_with_the_rehearsal_text() {
        let alone = topology(vec![display(1, 0, 0, 1920, 1080)], 1);
        let mut controller = controller();
        assert_eq!(
            controller.enable(alone),
            OutputAction::Refuse(OutputRefusal::NoDisplay)
        );
        assert_eq!(
            OutputRefusal::NoDisplay.message(),
            "No congregation display found — rehearsing on Preview"
        );
        assert!(!controller.is_enabled());
        assert_eq!(
            controller.enable(Topology::default()),
            OutputAction::Refuse(OutputRefusal::NoDisplay)
        );
    }

    #[test]
    fn one_candidate_is_assigned_and_opened() {
        let mut controller = controller();
        let OutputAction::Open(placement) = controller.enable(pair()) else {
            panic!("a lone candidate opens");
        };
        assert_eq!(placement.display, DisplayId::new(2));
        assert!(controller.is_enabled());
    }

    #[test]
    fn several_candidates_and_none_assigned_refuse_with_select_display() {
        let three = topology(
            vec![
                display(1, 0, 0, 1920, 1080),
                display(2, 1920, 0, 1280, 720),
                display(3, -1280, 0, 1280, 720),
            ],
            1,
        );
        let mut controller = controller();
        assert_eq!(
            controller.enable(three.clone()),
            OutputAction::Refuse(OutputRefusal::SelectDisplay)
        );
        assert!(!controller.is_enabled());
        controller.assign(DisplayId::new(3)).unwrap();
        let OutputAction::Open(placement) = controller.enable(three) else {
            panic!("an assigned display opens");
        };
        assert_eq!(placement.display, DisplayId::new(3));
    }

    #[test]
    fn an_assigned_display_that_is_gone_is_unavailable() {
        let mut controller = OutputController::new(pair());
        controller.assign(DisplayId::new(2)).unwrap();
        let other = topology(
            vec![display(1, 0, 0, 1920, 1080), display(3, 0, -720, 800, 600)],
            1,
        );
        assert_eq!(
            controller.enable(other),
            OutputAction::Refuse(OutputRefusal::Unavailable)
        );
        assert!(!controller.is_enabled());
    }

    #[test]
    fn output_never_opens_on_the_operator_display() {
        let mut controller = controller();
        // Several displays but no known operator: nothing can be told apart.
        let unknown = Topology::new(
            vec![display(1, 0, 0, 1920, 1080), display(2, 1920, 0, 1280, 720)],
            None,
        );
        assert_eq!(
            controller.enable(unknown),
            OutputAction::Refuse(OutputRefusal::Unavailable)
        );
        controller.refresh(pair());
        assert_eq!(
            controller.assign(DisplayId::new(1)),
            Err(AssignError::OperatorDisplay(DisplayId::new(1)))
        );
        let OutputAction::Open(placement) = controller.enable(pair()) else {
            panic!("the other display opens");
        };
        assert_ne!(placement.display, DisplayId::new(1));
    }

    fn lost_projector() -> OutputController {
        let mut controller = OutputController::new(pair());
        assert!(matches!(controller.enable(pair()), OutputAction::Open(_)));
        let alone = topology(vec![display(1, 0, 0, 1920, 1080)], 1);
        let events = controller.refresh(alone);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DisplayEvent::OutputDisconnected { .. }))
        );
        assert!(controller.is_disconnected());
        controller
    }

    #[test]
    fn resume_reopens_on_the_same_recognized_display() {
        let mut controller = lost_projector();
        // The same display comes back under a new operating-system id.
        let back = topology(
            vec![display(1, 0, 0, 1920, 1080), display(7, 1920, 0, 1280, 720)],
            1,
        );
        let OutputAction::Open(placement) = controller.resume(back) else {
            panic!("the same display resumes");
        };
        assert_eq!(placement.display, DisplayId::new(7));
        assert!(controller.is_enabled());
    }

    #[test]
    fn resume_refuses_a_different_display_and_stays_disconnected() {
        use super::super::test_support::named;
        let mut controller = OutputController::new(Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(2, 1920, 0, 1280, 720), "Projector", None),
            ],
            Some(DisplayId::new(1)),
        ));
        assert!(matches!(
            controller.enable(controller.control().topology().clone()),
            OutputAction::Open(_)
        ));
        controller.refresh(topology(vec![display(1, 0, 0, 1920, 1080)], 1));
        let other = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(5, 1920, 0, 1280, 720), "Other TV", None),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(
            controller.resume(other.clone()),
            OutputAction::Refuse(OutputRefusal::DifferentDisplay)
        );
        // Enable from Disconnected obeys the same rule.
        assert_eq!(
            controller.enable(other),
            OutputAction::Refuse(OutputRefusal::DifferentDisplay)
        );
        assert!(controller.is_disconnected());
        assert!(!controller.is_enabled());
    }

    #[test]
    fn resume_without_a_candidate_is_unavailable() {
        let mut controller = lost_projector();
        assert_eq!(
            controller.resume(topology(vec![display(1, 0, 0, 1920, 1080)], 1)),
            OutputAction::Refuse(OutputRefusal::Unavailable)
        );
        assert!(controller.is_disconnected());
    }

    #[test]
    fn resume_never_opens_on_the_operator_display() {
        let mut controller = lost_projector();
        // Only the operator display is left, and it has the projector's
        // resolution: it must not be taken for the congregation display.
        let lookalike = topology(vec![display(1, 1920, 0, 1280, 720)], 1);
        assert!(!matches!(
            controller.resume(lookalike),
            OutputAction::Open(_)
        ));
    }

    #[test]
    fn disable_closes_and_marks_output_disabled() {
        let mut controller = controller();
        assert!(matches!(controller.enable(pair()), OutputAction::Open(_)));
        assert_eq!(controller.disable(), OutputAction::Close);
        assert!(!controller.is_enabled());
    }

    #[test]
    fn recovered_output_resumes_on_the_same_display_only() {
        use super::super::test_support::named;
        let projector = named(display(2, 1920, 0, 1280, 720), "Projector", None);
        let with = |projector: DetectedDisplay| {
            Topology::new(
                vec![display(1, 0, 0, 1920, 1080), projector],
                Some(DisplayId::new(1)),
            )
        };
        let evidence = projector.evidence.clone();

        let mut same = OutputController::recovered(with(projector.clone()), evidence.clone());
        assert!(same.is_disconnected());
        let OutputAction::Open(placement) = same.enable(with(projector.clone())) else {
            panic!("the same display resumes");
        };
        assert_eq!(placement.display, DisplayId::new(2));
        assert!(same.is_enabled());

        let other = named(display(5, 1920, 0, 1280, 720), "Other TV", None);
        let mut different = OutputController::recovered(with(other.clone()), evidence.clone());
        assert_eq!(
            different.enable(with(other)),
            OutputAction::Refuse(OutputRefusal::DifferentDisplay)
        );
        assert!(different.is_disconnected());

        let mut none =
            OutputController::recovered(topology(vec![display(1, 0, 0, 1920, 1080)], 1), evidence);
        assert_eq!(
            none.enable(topology(vec![display(1, 0, 0, 1920, 1080)], 1)),
            OutputAction::Refuse(OutputRefusal::Unavailable)
        );
        assert!(none.is_disconnected());
        assert!(!none.is_enabled());
    }
}
