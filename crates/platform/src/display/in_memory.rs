use super::{DetectedDisplay, DisplayAdapter, DisplayBounds, DisplayId, PlacementBounds, Topology};

/// Programmable topology source for deterministic tests.
#[derive(Clone, Debug, Default)]
pub struct InMemoryDisplayAdapter {
    displays: Vec<DetectedDisplay>,
    operator: Option<DisplayId>,
}

impl InMemoryDisplayAdapter {
    pub fn new(displays: Vec<DetectedDisplay>, operator: Option<DisplayId>) -> Self {
        Self { displays, operator }
    }

    /// Adds a display, replacing any existing display with the same id.
    pub fn connect(&mut self, display: DetectedDisplay) {
        self.disconnect(display.id);
        self.displays.push(display);
    }

    /// Removes a display and returns it, as a cable pull would.
    pub fn disconnect(&mut self, id: DisplayId) -> Option<DetectedDisplay> {
        let index = self.displays.iter().position(|display| display.id == id)?;
        Some(self.displays.remove(index))
    }

    /// Gives a display a new operating-system id, keeping its evidence. Returns
    /// `false` and changes nothing when `from` is not connected or `to` is
    /// already in use by another display.
    pub fn reidentify(&mut self, from: DisplayId, to: DisplayId) -> bool {
        if from != to && self.displays.iter().any(|display| display.id == to) {
            return false;
        }
        let Some(display) = self.disconnect(from) else {
            return false;
        };
        if self.operator == Some(from) {
            self.operator = Some(to);
        }
        self.connect(DetectedDisplay { id: to, ..display });
        true
    }

    pub fn set_operator(&mut self, operator: Option<DisplayId>) {
        self.operator = operator;
    }

    /// Sets a display's bounds to exact whole-pixel values. Returns `false`
    /// when the display is not connected.
    pub fn set_bounds(&mut self, id: DisplayId, bounds: DisplayBounds) -> bool {
        match self.displays.iter_mut().find(|display| display.id == id) {
            Some(display) => {
                display.evidence.bounds = bounds;
                display.placement = PlacementBounds::from(bounds);
                true
            }
            None => false,
        }
    }
}

impl DisplayAdapter for InMemoryDisplayAdapter {
    fn snapshot(&self) -> Topology {
        Topology::new(self.displays.clone(), self.operator)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{display, named};
    use super::super::{DisplayControl, DisplayEvent, OutputLifecycle};
    use super::*;

    #[test]
    fn drives_disconnect_and_reidentified_reconnect() {
        let operator = DisplayId::new(1);
        let projector = DisplayId::new(2);
        let mut adapter = InMemoryDisplayAdapter::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(2, 0, -1080, 1920, 1080), "Projector", None),
            ],
            Some(operator),
        );
        let mut control = DisplayControl::new(adapter.snapshot());
        control.assign(projector).unwrap();
        control.enable_output().unwrap();

        let unplugged = adapter.disconnect(projector).unwrap();
        control.refresh(adapter.snapshot());
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));

        let renamed = DisplayId::new(20);
        adapter.connect(DetectedDisplay {
            id: renamed,
            ..unplugged
        });
        let events = control.refresh(adapter.snapshot());
        assert!(events.contains(&DisplayEvent::AssignedDisplayRecognized(renamed)));
        assert!(matches!(
            control.output(),
            OutputLifecycle::Disconnected { .. }
        ));
    }

    #[test]
    fn programmable_operations_shape_the_snapshot() {
        let mut adapter = InMemoryDisplayAdapter::default();
        assert_eq!(adapter.snapshot(), Topology::default());

        adapter.connect(display(1, 0, 0, 1920, 1080));
        adapter.set_operator(Some(DisplayId::new(1)));
        assert!(adapter.reidentify(DisplayId::new(1), DisplayId::new(5)));
        assert!(!adapter.reidentify(DisplayId::new(1), DisplayId::new(6)));
        assert!(adapter.set_bounds(DisplayId::new(5), DisplayBounds::new(10, 0, 1280, 720)));

        let snapshot = adapter.snapshot();
        assert_eq!(snapshot.operator(), Some(DisplayId::new(5)));
        assert_eq!(
            snapshot.display(DisplayId::new(5)).unwrap().evidence.bounds,
            DisplayBounds::new(10, 0, 1280, 720)
        );
        assert_eq!(
            snapshot.display(DisplayId::new(5)).unwrap().placement,
            PlacementBounds::new(10.0, 0.0, 1280.0, 720.0)
        );
    }

    #[test]
    fn reidentify_refuses_an_id_already_in_use() {
        let mut adapter = InMemoryDisplayAdapter::new(
            vec![display(1, 0, 0, 1920, 1080), display(2, 1920, 0, 1280, 720)],
            Some(DisplayId::new(1)),
        );
        let before = adapter.snapshot();

        assert!(!adapter.reidentify(DisplayId::new(1), DisplayId::new(2)));
        assert_eq!(adapter.snapshot(), before);
    }
}
