//! Production Adapter: translates GPUI `PlatformDisplay` snapshots into the
//! Display Control value types. GPUI display types stop here.
//!
//! On Windows (`gpui-pre-windows` `0.3.8`) the display id is the `HMONITOR`
//! handle, which can change or be reused across reconnects, and the uuid is
//! derived from the adapter output's device name, so it identifies a port
//! rather than a physical projector. Neither exposes a monitor name.
//!
//! Logical bounds are fractional at scales such as 150%. They are kept exact
//! in [`PlacementBounds`] for placing windows, and only rounded for evidence.

use std::rc::Rc;

use gpui::{App, Bounds, Pixels, PlatformDisplay, point, px, size};

use super::{
    DetectedDisplay, DisplayAdapter, DisplayEvidence, DisplayId, PlacementBounds, Topology,
};

impl From<gpui::DisplayId> for DisplayId {
    fn from(id: gpui::DisplayId) -> Self {
        Self::new(u64::from(id))
    }
}

impl From<DisplayId> for gpui::DisplayId {
    fn from(id: DisplayId) -> Self {
        gpui::DisplayId::new(id.raw())
    }
}

impl From<Bounds<Pixels>> for PlacementBounds {
    fn from(bounds: Bounds<Pixels>) -> Self {
        Self::new(
            bounds.origin.x.as_f32(),
            bounds.origin.y.as_f32(),
            bounds.size.width.as_f32(),
            bounds.size.height.as_f32(),
        )
    }
}

impl From<PlacementBounds> for Bounds<Pixels> {
    fn from(placement: PlacementBounds) -> Self {
        Bounds {
            origin: point(px(placement.x), px(placement.y)),
            size: size(px(placement.width), px(placement.height)),
        }
    }
}

/// Reads the current topology from GPUI. `operator` is the operator display
/// and is never inferred: when it is `None` the topology has no operator
/// display, so nothing is recognized and no output can be placed.
pub fn topology_from_gpui(cx: &App, operator: Option<gpui::DisplayId>) -> Topology {
    topology_from_displays(&cx.displays(), operator)
}

/// Translates platform displays, for callers holding a `Platform` directly.
pub fn topology_from_displays(
    displays: &[Rc<dyn PlatformDisplay>],
    operator: Option<gpui::DisplayId>,
) -> Topology {
    Topology::new(
        displays
            .iter()
            .map(|display| detected(display.id(), display.uuid().ok(), display.bounds()))
            .collect(),
        operator.map(DisplayId::from),
    )
}

/// The one display whose bounds contain the centre of `window`, or `None`
/// when no display or more than one display does.
///
/// GPUI display ids are `HMONITOR` values on Windows, reissued when the
/// topology changes, so an id cached on a window can be stale or name the
/// wrong monitor. Bounds are read fresh and carry no such identity problem.
/// A window's logical origin and each monitor's logical bounds are both
/// computed at that monitor's scale, so the true monitor contains the
/// window's centre even with mixed DPI. Overlap is ambiguous and is refused
/// rather than guessed.
pub fn display_containing(
    displays: impl IntoIterator<Item = (gpui::DisplayId, Bounds<Pixels>)>,
    window: Bounds<Pixels>,
) -> Option<gpui::DisplayId> {
    let centre = window.center();
    let mut containing = displays
        .into_iter()
        .filter(|(_, bounds)| bounds.contains(&centre))
        .map(|(id, _)| id);
    let found = containing.next()?;
    containing.next().is_none().then_some(found)
}

/// [`display_containing`] over platform displays read now.
pub fn display_containing_window(
    displays: &[Rc<dyn PlatformDisplay>],
    window: Bounds<Pixels>,
) -> Option<gpui::DisplayId> {
    display_containing(
        displays
            .iter()
            .map(|display| (display.id(), display.bounds())),
        window,
    )
}

fn detected(
    id: gpui::DisplayId,
    uuid: Option<impl ToString>,
    bounds: Bounds<Pixels>,
) -> DetectedDisplay {
    let placement = PlacementBounds::from(bounds);
    DetectedDisplay {
        id: id.into(),
        evidence: DisplayEvidence {
            name: None,
            uuid: uuid.map(|uuid| uuid.to_string()),
            bounds: placement.rounded(),
        },
        placement,
    }
}

/// [`DisplayAdapter`] over a borrowed GPUI `App`.
pub struct GpuiDisplayAdapter<'a> {
    cx: &'a App,
    operator: Option<gpui::DisplayId>,
}

impl<'a> GpuiDisplayAdapter<'a> {
    pub fn new(cx: &'a App, operator: Option<gpui::DisplayId>) -> Self {
        Self { cx, operator }
    }
}

impl DisplayAdapter for GpuiDisplayAdapter<'_> {
    fn snapshot(&self) -> Topology {
        topology_from_gpui(self.cx, self.operator)
    }
}

#[cfg(test)]
mod tests {
    use super::super::DisplayBounds;
    use super::*;

    #[test]
    fn display_ids_round_trip_through_gpui() {
        let id = DisplayId::new(0x1_0001_0003);
        assert_eq!(DisplayId::from(gpui::DisplayId::from(id)), id);
    }

    #[test]
    fn whole_pixel_placement_round_trips_through_gpui() {
        let placement = PlacementBounds::new(-1280.0, -720.0, 1280.0, 720.0);
        assert_eq!(
            PlacementBounds::from(Bounds::<Pixels>::from(placement)),
            placement
        );
    }

    #[test]
    fn fractional_placement_survives_exactly_and_rounds_only_for_evidence() {
        // 1706.6666 is what 150% scaling produces for a display to the right of
        // a 1280-wide one. Rounding it to 1707 would move the window by a pixel.
        let scaled = Bounds {
            origin: point(px(1706.6666), px(-0.4)),
            size: size(px(1365.3334), px(767.6)),
        };
        let placement = PlacementBounds::from(scaled);
        assert_eq!(placement.x, 1706.6666);
        assert_eq!(placement.y, -0.4);
        assert_eq!(placement.width, 1365.3334);
        assert_eq!(placement.height, 767.6);
        assert_eq!(Bounds::<Pixels>::from(placement), scaled);
        assert_eq!(placement.rounded(), DisplayBounds::new(1707, 0, 1365, 768));
    }

    fn area(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    fn id(raw: u64) -> gpui::DisplayId {
        gpui::DisplayId::new(raw)
    }

    fn layout() -> Vec<(gpui::DisplayId, Bounds<Pixels>)> {
        vec![
            (id(1), area(0.0, 0.0, 1920.0, 1080.0)),
            (id(2), area(-1280.0, 0.0, 1280.0, 720.0)),
            (id(3), area(0.0, -1080.0, 1920.0, 1080.0)),
        ]
    }

    #[test]
    fn window_centre_selects_the_single_containing_display() {
        let window = area(100.0, 100.0, 800.0, 600.0);
        assert_eq!(display_containing(layout(), window), Some(id(1)));
    }

    #[test]
    fn window_on_displays_left_or_above_resolves_at_negative_coordinates() {
        let left = area(-1200.0, 50.0, 800.0, 600.0);
        assert_eq!(display_containing(layout(), left), Some(id(2)));
        let above = area(100.0, -900.0, 800.0, 600.0);
        assert_eq!(display_containing(layout(), above), Some(id(3)));
    }

    #[test]
    fn off_screen_window_resolves_to_nothing() {
        let window = area(5000.0, 5000.0, 800.0, 600.0);
        assert_eq!(display_containing(layout(), window), None);
        assert_eq!(display_containing([], window), None);
    }

    #[test]
    fn overlapping_displays_are_ambiguous() {
        let displays = vec![
            (id(1), area(0.0, 0.0, 1920.0, 1080.0)),
            (id(2), area(960.0, 0.0, 1920.0, 1080.0)),
        ];
        let window = area(1000.0, 100.0, 400.0, 300.0);
        assert_eq!(display_containing(displays, window), None);
    }

    #[test]
    fn translation_keeps_uuid_placement_and_has_no_name() {
        let placement = PlacementBounds::new(0.0, 0.0, 1920.0, 1080.0);
        let display = detected(gpui::DisplayId::new(7), Some("abc"), placement.into());
        assert_eq!(display.id, DisplayId::new(7));
        assert_eq!(display.evidence.uuid.as_deref(), Some("abc"));
        assert_eq!(display.evidence.name, None);
        assert_eq!(
            display.evidence.bounds,
            DisplayBounds::new(0, 0, 1920, 1080)
        );
        assert_eq!(display.placement, placement);
    }
}
