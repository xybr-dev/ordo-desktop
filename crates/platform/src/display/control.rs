use super::{DisplayEvidence, DisplayId, PlacementBounds, Recognition, Topology, recognize};

/// The installation's congregation-display choice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Assignment {
    #[default]
    NotAssigned,
    Assigned(DisplayEvidence),
}

/// Congregation-output window lifecycle as Display Control sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OutputLifecycle {
    #[default]
    Disabled,
    /// Output is placed on `display`. `evidence` is that display as it was
    /// when output was placed, so a later change to it can be detected.
    Enabled {
        display: DisplayId,
        evidence: DisplayEvidence,
    },
    /// The enabled display disappeared or changed. `evidence` is what the
    /// output was placed on. Output stays here, never retargeted or moved,
    /// until the operator explicitly enables it again.
    Disconnected { evidence: DisplayEvidence },
}

/// Where the congregation window must open. `bounds` is exact, so a window
/// placed at a fractional logical origin does not shift by a pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputPlacement {
    pub display: DisplayId,
    pub bounds: PlacementBounds,
}

/// Operator-facing summary of the congregation display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayStatus {
    NotAssigned,
    /// Assigned but not present in the current topology, or the operator
    /// display is unknown so no assigned display can be recognized.
    Unavailable,
    Ambiguous(Vec<DisplayId>),
    Ready(DisplayId),
    Live(DisplayId),
    /// Output was live when its display disappeared or changed. `recognized`
    /// names the display once the assigned evidence is seen again.
    Disconnected {
        recognized: Option<DisplayId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayEvent {
    DisplayAdded(DisplayId),
    DisplayRemoved(DisplayId),
    OperatorChanged {
        from: Option<DisplayId>,
        to: Option<DisplayId>,
    },
    /// Enabled output lost its display, or the display changed, and entered
    /// `Disconnected`.
    OutputDisconnected {
        evidence: DisplayEvidence,
    },
    AssignedDisplayRecognized(DisplayId),
    AssignedDisplayAmbiguous(Vec<DisplayId>),
    AssignedDisplayMissing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignError {
    UnknownDisplay(DisplayId),
    OperatorDisplay(DisplayId),
    /// No operator display is known, so the congregation display cannot be
    /// told apart from the operator display.
    OperatorUnknown,
    OutputEnabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnableError {
    NotAssigned,
    AssignedDisplayMissing,
    Ambiguous(Vec<DisplayId>),
    /// No operator display is known. Output is never placed without one,
    /// because it could land on the operator's screen.
    OperatorUnknown,
    AlreadyEnabled,
}

/// Owns the current topology, congregation assignment, and output lifecycle.
#[derive(Clone, Debug)]
pub struct DisplayControl {
    topology: Topology,
    assignment: Assignment,
    output: OutputLifecycle,
    recognition: Recognition,
}

impl DisplayControl {
    pub fn new(topology: Topology) -> Self {
        Self::with_assignment(topology, Assignment::NotAssigned)
    }

    /// Restores a saved assignment. The saved evidence is only a suggestion:
    /// output stays disabled and recognition runs against `topology`.
    pub fn with_assignment(topology: Topology, assignment: Assignment) -> Self {
        let recognition = recognition_of(&assignment, &topology);
        Self {
            topology,
            assignment,
            output: OutputLifecycle::Disabled,
            recognition,
        }
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    pub fn assignment(&self) -> &Assignment {
        &self.assignment
    }

    pub fn output(&self) -> &OutputLifecycle {
        &self.output
    }

    /// Recognition of the assigned display in the current topology;
    /// `Missing` when nothing is assigned or no operator display is known.
    pub fn recognition(&self) -> &Recognition {
        &self.recognition
    }

    pub fn status(&self) -> DisplayStatus {
        if let OutputLifecycle::Enabled { display, .. } = self.output {
            return DisplayStatus::Live(display);
        }
        let recognized = match &self.recognition {
            Recognition::Recognized(id) => Some(*id),
            _ => None,
        };
        if matches!(self.output, OutputLifecycle::Disconnected { .. }) {
            return DisplayStatus::Disconnected { recognized };
        }
        match (&self.assignment, &self.recognition) {
            (Assignment::NotAssigned, _) => DisplayStatus::NotAssigned,
            (_, Recognition::Recognized(id)) => DisplayStatus::Ready(*id),
            (_, Recognition::Ambiguous(ids)) => DisplayStatus::Ambiguous(ids.clone()),
            (_, Recognition::Missing) => DisplayStatus::Unavailable,
        }
    }

    /// The display to preselect: the recognized assignment, else the only
    /// non-operator display.
    pub fn suggestion(&self) -> Option<DisplayId> {
        if let Recognition::Recognized(id) = self.recognition {
            return Some(id);
        }
        // Without a known operator display the lone display may be the
        // operator, so nothing is suggested.
        self.topology.operator()?;
        let mut candidates = self.topology.candidates();
        match (candidates.next(), candidates.next()) {
            (Some(only), None) => Some(only.id),
            _ => None,
        }
    }

    /// Assigns a detected non-operator display. Ends a `Disconnected`
    /// lifecycle because the lost display is no longer the target.
    pub fn assign(&mut self, id: DisplayId) -> Result<(), AssignError> {
        if matches!(self.output, OutputLifecycle::Enabled { .. }) {
            return Err(AssignError::OutputEnabled);
        }
        if self.topology.operator().is_none() {
            return Err(AssignError::OperatorUnknown);
        }
        if self.topology.operator() == Some(id) {
            return Err(AssignError::OperatorDisplay(id));
        }
        let evidence = self
            .topology
            .display(id)
            .ok_or(AssignError::UnknownDisplay(id))?
            .evidence
            .clone();
        self.output = OutputLifecycle::Disabled;
        self.recognition = Recognition::Recognized(id);
        self.assignment = Assignment::Assigned(evidence);
        Ok(())
    }

    pub fn clear_assignment(&mut self) -> Result<(), AssignError> {
        if matches!(self.output, OutputLifecycle::Enabled { .. }) {
            return Err(AssignError::OutputEnabled);
        }
        self.output = OutputLifecycle::Disabled;
        self.assignment = Assignment::NotAssigned;
        self.recognition = Recognition::Missing;
        Ok(())
    }

    /// Explicitly enables output on the recognized assigned display. This is
    /// the only path to `Enabled`.
    pub fn enable_output(&mut self) -> Result<OutputPlacement, EnableError> {
        if matches!(self.output, OutputLifecycle::Enabled { .. }) {
            return Err(EnableError::AlreadyEnabled);
        }
        if self.assignment == Assignment::NotAssigned {
            return Err(EnableError::NotAssigned);
        }
        if self.topology.operator().is_none() {
            return Err(EnableError::OperatorUnknown);
        }
        let display = match &self.recognition {
            Recognition::Recognized(id) => *id,
            Recognition::Ambiguous(ids) => return Err(EnableError::Ambiguous(ids.clone())),
            Recognition::Missing => return Err(EnableError::AssignedDisplayMissing),
        };
        let detected = self
            .topology
            .display(display)
            .ok_or(EnableError::AssignedDisplayMissing)?;
        let placement = OutputPlacement {
            display,
            bounds: detected.placement,
        };
        self.output = OutputLifecycle::Enabled {
            display,
            evidence: detected.evidence.clone(),
        };
        Ok(placement)
    }

    pub fn disable_output(&mut self) {
        self.output = OutputLifecycle::Disabled;
    }

    /// Replaces the topology with a new discovery snapshot.
    ///
    /// Enabled output moves to `Disconnected` when its display vanished,
    /// became the operator display, or differs in any observed way from the
    /// evidence it was placed with. A moved display counts as changed, because
    /// the window would otherwise stay on stale bounds. Such output is never
    /// moved to another display. A returning assigned display is reported, but
    /// the output stays disconnected until it is explicitly enabled.
    ///
    /// Assignment cannot change while output is enabled, so the live check
    /// needs only the placed evidence.
    pub fn refresh(&mut self, topology: Topology) -> Vec<DisplayEvent> {
        let mut events = Vec::new();
        for display in topology.displays() {
            if self.topology.display(display.id).is_none() {
                events.push(DisplayEvent::DisplayAdded(display.id));
            }
        }
        for display in self.topology.displays() {
            if topology.display(display.id).is_none() {
                events.push(DisplayEvent::DisplayRemoved(display.id));
            }
        }
        if topology.operator() != self.topology.operator() {
            events.push(DisplayEvent::OperatorChanged {
                from: self.topology.operator(),
                to: topology.operator(),
            });
        }

        let output = std::mem::take(&mut self.output);
        self.output = match output {
            OutputLifecycle::Enabled { display, evidence }
                if !still_placed(&topology, display, &evidence) =>
            {
                events.push(DisplayEvent::OutputDisconnected {
                    evidence: evidence.clone(),
                });
                OutputLifecycle::Disconnected { evidence }
            }
            kept => kept,
        };

        let recognition = recognition_of(&self.assignment, &topology);
        let live = matches!(self.output, OutputLifecycle::Enabled { .. });
        if recognition != self.recognition && !live && self.assignment != Assignment::NotAssigned {
            events.push(match &recognition {
                Recognition::Recognized(id) => DisplayEvent::AssignedDisplayRecognized(*id),
                Recognition::Ambiguous(ids) => DisplayEvent::AssignedDisplayAmbiguous(ids.clone()),
                Recognition::Missing => DisplayEvent::AssignedDisplayMissing,
            });
        }
        self.recognition = recognition;
        self.topology = topology;
        events
    }
}

/// Enabled output stays placed only while its display is still a known
/// non-operator display showing exactly the evidence it was placed with.
/// This is deliberately stricter than recognition, which is the identity
/// predicate: any change in observed evidence counts as lost output.
fn still_placed(topology: &Topology, display: DisplayId, placed: &DisplayEvidence) -> bool {
    topology
        .operator()
        .is_some_and(|operator| operator != display)
        && topology
            .display(display)
            .is_some_and(|seen| seen.evidence == *placed)
}

fn recognition_of(assignment: &Assignment, topology: &Topology) -> Recognition {
    match assignment {
        Assignment::NotAssigned => Recognition::Missing,
        Assignment::Assigned(evidence) => recognize(evidence, topology),
    }
}

#[cfg(test)]
mod tests {
    use super::super::DetectedDisplay;
    use super::super::test_support::{display, named};
    use super::*;

    const OPERATOR: DisplayId = DisplayId::new(1);
    const PROJECTOR: DisplayId = DisplayId::new(2);
    const SPARE: DisplayId = DisplayId::new(3);

    fn projector(id: u64) -> DetectedDisplay {
        named(
            display(id, -1280, 0, 1280, 720),
            "Projector",
            Some("projector"),
        )
    }

    fn two_displays() -> Topology {
        Topology::new(
            vec![display(1, 0, 0, 1920, 1080), projector(2)],
            Some(OPERATOR),
        )
    }

    fn live() -> DisplayControl {
        let mut control = DisplayControl::new(two_displays());
        control.assign(PROJECTOR).unwrap();
        control.enable_output().unwrap();
        control
    }

    #[test]
    fn starts_disabled_and_unassigned() {
        let control = DisplayControl::new(two_displays());
        assert_eq!(control.output(), &OutputLifecycle::Disabled);
        assert_eq!(control.status(), DisplayStatus::NotAssigned);
        assert_eq!(control.suggestion(), Some(PROJECTOR));
    }

    #[test]
    fn assignment_rejects_operator_and_unknown_displays() {
        let mut control = DisplayControl::new(two_displays());
        assert_eq!(
            control.assign(OPERATOR),
            Err(AssignError::OperatorDisplay(OPERATOR))
        );
        assert_eq!(
            control.assign(SPARE),
            Err(AssignError::UnknownDisplay(SPARE))
        );
        assert_eq!(control.assignment(), &Assignment::NotAssigned);

        control.assign(PROJECTOR).unwrap();
        assert_eq!(control.status(), DisplayStatus::Ready(PROJECTOR));
        control.clear_assignment().unwrap();
        assert_eq!(control.status(), DisplayStatus::NotAssigned);
    }

    #[test]
    fn enable_requires_an_assignment() {
        let mut control = DisplayControl::new(two_displays());
        assert_eq!(control.enable_output(), Err(EnableError::NotAssigned));
        assert_eq!(control.output(), &OutputLifecycle::Disabled);
    }

    #[test]
    fn without_an_operator_display_nothing_is_assigned_or_placed() {
        let topology = Topology::new(vec![display(1, 0, 0, 1920, 1080), projector(2)], None);
        assert_eq!(DisplayControl::new(topology.clone()).suggestion(), None);

        let mut control = DisplayControl::new(topology.clone());
        assert_eq!(control.assign(PROJECTOR), Err(AssignError::OperatorUnknown));
        assert_eq!(control.assignment(), &Assignment::NotAssigned);

        // A restored assignment is not recognized without an operator display,
        // so enabling is refused rather than placing output on an unknown screen.
        let mut restored =
            DisplayControl::with_assignment(topology, Assignment::Assigned(projector(2).evidence));
        assert_eq!(restored.recognition(), &Recognition::Missing);
        assert_eq!(restored.enable_output(), Err(EnableError::OperatorUnknown));
        assert_eq!(restored.output(), &OutputLifecycle::Disabled);
    }

    #[test]
    fn enable_places_output_on_the_assigned_display_once() {
        let mut control = DisplayControl::new(two_displays());
        control.assign(PROJECTOR).unwrap();
        let placement = control.enable_output().unwrap();
        assert_eq!(placement.display, PROJECTOR);
        assert_eq!(placement.bounds, projector(2).placement);
        assert_eq!(control.status(), DisplayStatus::Live(PROJECTOR));
        assert_eq!(control.enable_output(), Err(EnableError::AlreadyEnabled));
        assert_eq!(control.assign(PROJECTOR), Err(AssignError::OutputEnabled));
        assert_eq!(control.clear_assignment(), Err(AssignError::OutputEnabled));

        control.disable_output();
        assert_eq!(control.status(), DisplayStatus::Ready(PROJECTOR));
    }

    #[test]
    fn unchanged_refresh_keeps_output_live() {
        let mut control = live();
        let events = control.refresh(two_displays());
        assert!(events.is_empty());
        assert_eq!(control.status(), DisplayStatus::Live(PROJECTOR));
    }

    #[test]
    fn disappearance_disconnects_without_retargeting() {
        let mut control = live();
        let events = control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080), display(3, 1920, 0, 1280, 720)],
            Some(OPERATOR),
        ));

        let evidence = projector(2).evidence;
        assert!(events.contains(&DisplayEvent::DisplayRemoved(PROJECTOR)));
        assert!(events.contains(&DisplayEvent::DisplayAdded(SPARE)));
        assert!(events.contains(&DisplayEvent::OutputDisconnected {
            evidence: evidence.clone()
        }));
        assert_eq!(
            control.output(),
            &OutputLifecycle::Disconnected { evidence }
        );
        assert_eq!(
            control.status(),
            DisplayStatus::Disconnected { recognized: None }
        );
        assert_eq!(
            control.enable_output(),
            Err(EnableError::AssignedDisplayMissing)
        );
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
    }

    #[test]
    fn moved_live_display_disconnects_and_is_not_placed_on_stale_bounds() {
        let mut control = live();
        let moved = named(
            display(2, -1920, 0, 1280, 720),
            "Projector",
            Some("projector"),
        );
        let events = control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080), moved.clone()],
            Some(OPERATOR),
        ));

        assert!(events.contains(&DisplayEvent::OutputDisconnected {
            evidence: projector(2).evidence
        }));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
        assert_eq!(
            control.status(),
            DisplayStatus::Disconnected {
                recognized: Some(PROJECTOR)
            }
        );

        // Re-enabling is an explicit operator action, and it uses the new placement.
        let placement = control.enable_output().unwrap();
        assert_eq!(placement.bounds, moved.placement);
    }

    #[test]
    fn reconnection_under_a_new_id_waits_for_explicit_enable() {
        let mut control = live();
        control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080)],
            Some(OPERATOR),
        ));

        let reconnected = DisplayId::new(77);
        let events = control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080), projector(77)],
            Some(OPERATOR),
        ));
        assert!(events.contains(&DisplayEvent::AssignedDisplayRecognized(reconnected)));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
        assert_eq!(
            control.status(),
            DisplayStatus::Disconnected {
                recognized: Some(reconnected)
            }
        );

        let placement = control.enable_output().unwrap();
        assert_eq!(placement.display, reconnected);
        assert!(matches!(
            control.output(),
            OutputLifecycle::Enabled { display, .. } if *display == reconnected
        ));
    }

    #[test]
    fn re_id_in_one_refresh_still_disconnects() {
        let mut control = live();
        let events = control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080), projector(9)],
            Some(OPERATOR),
        ));
        assert!(events.contains(&DisplayEvent::OutputDisconnected {
            evidence: projector(2).evidence
        }));
        assert!(events.contains(&DisplayEvent::AssignedDisplayRecognized(DisplayId::new(9))));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
    }

    #[test]
    fn reused_os_id_on_a_different_display_disconnects() {
        let mut control = live();
        // The same port-derived uuid and name, but a different resolution: a
        // different display that has taken over the reused id.
        control.refresh(Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(
                    display(2, 1920, 0, 3840, 2160),
                    "Projector",
                    Some("projector"),
                ),
            ],
            Some(OPERATOR),
        ));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
    }

    #[test]
    fn ambiguous_evidence_never_auto_selects() {
        let mut control = DisplayControl::with_assignment(
            Topology::new(vec![display(1, 0, 0, 1920, 1080)], Some(OPERATOR)),
            Assignment::Assigned(display(2, 1920, 0, 1920, 1080).evidence),
        );
        assert_eq!(control.status(), DisplayStatus::Unavailable);

        let events = control.refresh(Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                display(5, -1920, 0, 1920, 1080),
                display(6, 0, -1080, 1920, 1080),
            ],
            Some(OPERATOR),
        ));
        let candidates = vec![DisplayId::new(5), DisplayId::new(6)];
        assert!(events.contains(&DisplayEvent::AssignedDisplayAmbiguous(candidates.clone())));
        assert_eq!(
            control.status(),
            DisplayStatus::Ambiguous(candidates.clone())
        );
        assert_eq!(control.suggestion(), None);
        assert_eq!(
            control.enable_output(),
            Err(EnableError::Ambiguous(candidates))
        );
        assert_eq!(control.output(), &OutputLifecycle::Disabled);
    }

    #[test]
    fn restored_assignment_is_recognized_but_not_enabled() {
        let control = DisplayControl::with_assignment(
            Topology::new(
                vec![display(1, 0, 0, 1920, 1080), projector(40)],
                Some(OPERATOR),
            ),
            Assignment::Assigned(projector(2).evidence),
        );
        assert_eq!(control.status(), DisplayStatus::Ready(DisplayId::new(40)));
        assert_eq!(control.output(), &OutputLifecycle::Disabled);
    }

    #[test]
    fn refresh_survives_empty_topology_and_operator_changes() {
        let mut control = live();
        let events = control.refresh(Topology::default());
        assert!(events.contains(&DisplayEvent::OperatorChanged {
            from: Some(OPERATOR),
            to: None
        }));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
        assert_eq!(control.suggestion(), None);

        // The projector returns but now carries the operator workspace.
        control.refresh(Topology::new(vec![projector(2)], Some(PROJECTOR)));
        assert_eq!(
            control.enable_output(),
            Err(EnableError::AssignedDisplayMissing)
        );

        let mut control = live();
        control.refresh(Topology::new(vec![projector(2)], Some(PROJECTOR)));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
        // Without a known operator the recovered display is not recognized,
        // so the status reports no recognition and enabling is refused.
        control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080), projector(2)],
            None,
        ));
        assert_eq!(
            control.status(),
            DisplayStatus::Disconnected { recognized: None }
        );
        assert_eq!(control.enable_output(), Err(EnableError::OperatorUnknown));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
    }

    #[test]
    fn reassignment_ends_a_disconnection_without_enabling() {
        let mut control = live();
        control.refresh(Topology::new(
            vec![display(1, 0, 0, 1920, 1080), display(3, 1920, 0, 1280, 720)],
            Some(OPERATOR),
        ));
        control.assign(SPARE).unwrap();
        assert_eq!(control.output(), &OutputLifecycle::Disabled);
        assert_eq!(control.status(), DisplayStatus::Ready(SPARE));
    }
}
