//! Display Control Interface: discovery, identification, assignment, output
//! lifecycle, topology, and status for the congregation display.
//!
//! Everything here except [`gpui_adapter`] is plain data. The pinned GPUI
//! snapshot has no display-change event, so discovery is a snapshot read and
//! topology changes arrive through an explicit [`DisplayControl::refresh`].

mod control;
pub mod gpui_adapter;
mod in_memory;

pub use control::{
    AssignError, Assignment, DisplayControl, DisplayEvent, DisplayStatus, EnableError,
    OutputLifecycle, OutputPlacement,
};
pub use in_memory::InMemoryDisplayAdapter;

/// Operating-system display identity. It is valid only for the current
/// topology; reconnection may assign a new id to the same physical display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayId(u64);

impl DisplayId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Whole-pixel logical bounds, used for evidence and direction.
///
/// These are the rounding of a [`PlacementBounds`]. The operating system's
/// logical coordinates are not a consistent virtual desktop on mixed-DPI
/// setups: Windows divides each monitor's physical origin by that monitor's
/// own scale, so side-by-side displays can report overlapping logical
/// rectangles. [`RelativeDirection::between`] accounts for that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisplayBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl DisplayBounds {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Width divided by height, or `None` for a degenerate display.
    pub fn aspect_ratio(&self) -> Option<f64> {
        (self.width > 0 && self.height > 0).then(|| f64::from(self.width) / f64::from(self.height))
    }

    fn left(&self) -> i64 {
        i64::from(self.x)
    }

    fn top(&self) -> i64 {
        i64::from(self.y)
    }

    fn right(&self) -> i64 {
        self.left() + i64::from(self.width)
    }

    fn bottom(&self) -> i64 {
        self.top() + i64::from(self.height)
    }

    /// Twice the centre's x coordinate, which keeps the centre integral.
    fn centre_x2(&self) -> i64 {
        2 * self.left() + i64::from(self.width)
    }

    /// Twice the centre's y coordinate, which keeps the centre integral.
    fn centre_y2(&self) -> i64 {
        2 * self.top() + i64::from(self.height)
    }
}

/// Exact logical placement of a display as the operating system reports it.
///
/// At fractional scales such as 150% the exact origin is needed to place a
/// window without a one-pixel error, so windows are placed from this value and
/// [`DisplayBounds`] is only derived from it. `f32` is not `Eq`, so neither
/// this type nor the snapshot types that contain it implement `Eq`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacementBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PlacementBounds {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Rounds each component to whole logical pixels. Negative sizes become
    /// zero; the float-to-integer conversions saturate.
    pub fn rounded(self) -> DisplayBounds {
        DisplayBounds::new(
            self.x.round() as i32,
            self.y.round() as i32,
            self.width.round().max(0.0) as u32,
            self.height.round().max(0.0) as u32,
        )
    }
}

impl From<DisplayBounds> for PlacementBounds {
    fn from(bounds: DisplayBounds) -> Self {
        Self::new(
            bounds.x as f32,
            bounds.y as f32,
            bounds.width as f32,
            bounds.height as f32,
        )
    }
}

/// What Ordo can observe about a display, used to recognize it again after
/// its operating-system id changes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DisplayEvidence {
    pub name: Option<String>,
    pub uuid: Option<String>,
    pub bounds: DisplayBounds,
}

impl DisplayEvidence {
    pub fn aspect_ratio(&self) -> Option<f64> {
        self.bounds.aspect_ratio()
    }

    /// Whether `seen` plausibly is the display this evidence was taken from.
    ///
    /// This is the single identity predicate that [`recognize`] uses. The
    /// resolution must match and any saved name must be confirmed. The uuid
    /// never disqualifies, because Windows derives it from the output port
    /// rather than the monitor; it only narrows several matches.
    fn identifies(&self, seen: &Self) -> bool {
        self.same_resolution(seen) && self.compatible_name(seen)
    }

    fn same_resolution(&self, other: &Self) -> bool {
        (self.bounds.width, self.bounds.height) == (other.bounds.width, other.bounds.height)
    }

    /// A saved name must be confirmed; an unnamed display cannot confirm it.
    fn compatible_name(&self, seen: &Self) -> bool {
        self.name.is_none() || self.name == seen.name
    }
}

/// One display in a discovery snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectedDisplay {
    pub id: DisplayId,
    pub evidence: DisplayEvidence,
    /// Exact placement. `evidence.bounds` is its rounding.
    pub placement: PlacementBounds,
}

/// Where `to` lies relative to `from`, named by the direction the pointer
/// travels from `from` to reach it.
///
/// The four cardinal variants are edge separations. The four diagonal variants
/// name a corner direction, where both axes are separated or, for overlapping
/// rectangles, where the centre offset is comparable on both axes.
/// `Overlapping` means the two centres coincide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RelativeDirection {
    Left,
    Right,
    Above,
    Below,
    AboveLeft,
    AboveRight,
    BelowLeft,
    BelowRight,
    Overlapping,
}

/// A centre offset is cardinal when one axis is more than this many times the
/// other; otherwise it is a corner direction.
const CARDINAL_RATIO: i64 = 2;

impl RelativeDirection {
    /// Classifies `to` against `from`.
    ///
    /// Edge separation decides first. A display entirely left or right of
    /// `from` is `Left` or `Right` (shared edges count as separated), and
    /// likewise `Above` and `Below`. A display separated on both axes is a
    /// corner direction. Separation on one axis only is cardinal, so a display
    /// stacked on top stays `Above` however far it is shifted sideways while
    /// any column is shared.
    ///
    /// When the rectangles overlap on both axes, which mixed-DPI logical
    /// coordinates can produce for side-by-side displays, the centres decide.
    /// A centre offset cardinal along one axis gives that direction, and a
    /// comparable offset on both axes gives a corner. Coincident centres give
    /// `Overlapping`.
    pub fn between(from: &DisplayBounds, to: &DisplayBounds) -> Self {
        let horizontal = if to.left() >= from.right() {
            Some(true)
        } else if to.right() <= from.left() {
            Some(false)
        } else {
            None
        };
        let vertical = if to.top() >= from.bottom() {
            Some(true)
        } else if to.bottom() <= from.top() {
            Some(false)
        } else {
            None
        };
        match (horizontal, vertical) {
            (None, None) => Self::between_centres(from, to),
            (Some(true), None) => Self::Right,
            (Some(false), None) => Self::Left,
            (None, Some(true)) => Self::Below,
            (None, Some(false)) => Self::Above,
            (Some(true), Some(true)) => Self::BelowRight,
            (Some(false), Some(true)) => Self::BelowLeft,
            (Some(true), Some(false)) => Self::AboveRight,
            (Some(false), Some(false)) => Self::AboveLeft,
        }
    }

    fn between_centres(from: &DisplayBounds, to: &DisplayBounds) -> Self {
        let dx = to.centre_x2() - from.centre_x2();
        let dy = to.centre_y2() - from.centre_y2();
        if dx == 0 && dy == 0 {
            return Self::Overlapping;
        }
        let (right, below) = (dx > 0, dy > 0);
        if dx.abs() > CARDINAL_RATIO * dy.abs() {
            if right { Self::Right } else { Self::Left }
        } else if dy.abs() > CARDINAL_RATIO * dx.abs() {
            if below { Self::Below } else { Self::Above }
        } else {
            match (right, below) {
                (true, true) => Self::BelowRight,
                (true, false) => Self::AboveRight,
                (false, true) => Self::BelowLeft,
                (false, false) => Self::AboveLeft,
            }
        }
    }
}

/// One discovery snapshot: every connected display and which one carries the
/// operator workspace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Topology {
    displays: Vec<DetectedDisplay>,
    operator: Option<DisplayId>,
}

impl Topology {
    /// An operator id that is not among `displays` is dropped.
    pub fn new(displays: Vec<DetectedDisplay>, operator: Option<DisplayId>) -> Self {
        let operator = operator.filter(|id| displays.iter().any(|display| display.id == *id));
        Self { displays, operator }
    }

    pub fn displays(&self) -> &[DetectedDisplay] {
        &self.displays
    }

    pub fn operator(&self) -> Option<DisplayId> {
        self.operator
    }

    pub fn display(&self, id: DisplayId) -> Option<&DetectedDisplay> {
        self.displays.iter().find(|display| display.id == id)
    }

    /// Displays that may carry congregation output: every display except the
    /// operator display.
    pub fn candidates(&self) -> impl Iterator<Item = &DetectedDisplay> {
        self.displays
            .iter()
            .filter(move |display| Some(display.id) != self.operator)
    }

    /// Direction from `from` to `to`, or `None` when either id is unknown.
    pub fn direction(&self, from: DisplayId, to: DisplayId) -> Option<RelativeDirection> {
        let from = self.display(from)?;
        let to = self.display(to)?;
        Some(RelativeDirection::between(
            &from.evidence.bounds,
            &to.evidence.bounds,
        ))
    }

    /// Direction from the operator display to `to`.
    pub fn direction_from_operator(&self, to: DisplayId) -> Option<RelativeDirection> {
        self.direction(self.operator?, to)
    }
}

/// Whether saved evidence identifies a display in a topology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recognition {
    Recognized(DisplayId),
    /// Several displays match; the operator must choose. Never auto-selected.
    Ambiguous(Vec<DisplayId>),
    Missing,
}

/// Matches saved evidence against the non-operator displays of `topology`.
///
/// A candidate matches when [`DisplayEvidence::identifies`] holds: the saved
/// resolution is equal and any saved name is confirmed. A uuid never admits or
/// excludes a candidate. When several candidates match, a shared uuid and then
/// an identical placement each narrow the set, but only when they leave
/// exactly one candidate.
///
/// Without a known operator display nothing is recognized, because the
/// lone display may be the operator. The operator display is never recognized
/// as the congregation display.
pub fn recognize(evidence: &DisplayEvidence, topology: &Topology) -> Recognition {
    if topology.operator().is_none() {
        return Recognition::Missing;
    }
    let mut ids = matching(topology, |seen| evidence.identifies(seen));
    if ids.len() > 1 {
        ids = narrow(topology, ids, |seen| {
            evidence.uuid.is_some() && seen.uuid == evidence.uuid
        });
    }
    if ids.len() > 1 {
        ids = narrow(topology, ids, |seen| seen.bounds == evidence.bounds);
    }
    decide(ids)
}

fn matching(topology: &Topology, matches: impl Fn(&DisplayEvidence) -> bool) -> Vec<DisplayId> {
    topology
        .candidates()
        .filter(|display| matches(&display.evidence))
        .map(|display| display.id)
        .collect()
}

/// Keeps the candidates that satisfy `keep`, but only when exactly one remains.
fn narrow(
    topology: &Topology,
    ids: Vec<DisplayId>,
    keep: impl Fn(&DisplayEvidence) -> bool,
) -> Vec<DisplayId> {
    let kept = ids
        .iter()
        .copied()
        .filter(|id| {
            topology
                .display(*id)
                .is_some_and(|display| keep(&display.evidence))
        })
        .collect::<Vec<_>>();
    if kept.len() == 1 { kept } else { ids }
}

fn decide(ids: Vec<DisplayId>) -> Recognition {
    match ids.as_slice() {
        [] => Recognition::Missing,
        [id] => Recognition::Recognized(*id),
        _ => Recognition::Ambiguous(ids),
    }
}

/// Discovery source for display topology snapshots.
pub trait DisplayAdapter {
    fn snapshot(&self) -> Topology;
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A display whose placement is the exact integer bounds.
    pub fn display(id: u64, x: i32, y: i32, width: u32, height: u32) -> DetectedDisplay {
        let bounds = DisplayBounds::new(x, y, width, height);
        DetectedDisplay {
            id: DisplayId::new(id),
            evidence: DisplayEvidence {
                name: None,
                uuid: None,
                bounds,
            },
            placement: PlacementBounds::from(bounds),
        }
    }

    pub fn named(mut display: DetectedDisplay, name: &str, uuid: Option<&str>) -> DetectedDisplay {
        display.evidence.name = Some(name.to_owned());
        display.evidence.uuid = uuid.map(str::to_owned);
        display
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{display, named};
    use super::*;
    use RelativeDirection::*;

    fn direction(from: DisplayBounds, to: DisplayBounds) -> RelativeDirection {
        RelativeDirection::between(&from, &to)
    }

    #[test]
    fn cardinal_directions_from_origin_operator() {
        let operator = DisplayBounds::new(0, 0, 1920, 1080);
        assert_eq!(
            direction(operator, DisplayBounds::new(1920, 0, 1920, 1080)),
            Right
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(-1280, 0, 1280, 720)),
            Left
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(0, -1080, 1920, 1080)),
            Above
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(0, 1080, 1920, 1080)),
            Below
        );
    }

    #[test]
    fn stacked_display_offset_sideways_stays_cardinal() {
        let operator = DisplayBounds::new(0, 0, 1920, 1080);
        assert_eq!(
            direction(operator, DisplayBounds::new(400, -768, 1366, 768)),
            Above
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(-300, 1080, 1280, 720)),
            Below
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(1920, 500, 1280, 1024)),
            Right
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(-1024, -200, 1024, 768)),
            Left
        );
    }

    #[test]
    fn separated_corners_and_identical_centres() {
        let operator = DisplayBounds::new(0, 0, 1920, 1080);
        assert_eq!(
            direction(operator, DisplayBounds::new(1920, -1080, 1920, 1080)),
            AboveRight
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(-1920, -1080, 1920, 1080)),
            AboveLeft
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(1920, 1080, 1920, 1080)),
            BelowRight
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(-800, 1080, 800, 600)),
            BelowLeft
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(0, 0, 1920, 1080)),
            Overlapping
        );
        // A different size with the same centre (960, 540) also overlaps.
        assert_eq!(
            direction(operator, DisplayBounds::new(460, 290, 1000, 500)),
            Overlapping
        );
    }

    #[test]
    fn mixed_dpi_overlap_is_classified_by_centres() {
        // Windows divides each monitor's physical origin by its own scale, so a
        // 1280-wide projector beside a 1920-wide laptop can report logical
        // bounds that overlap the laptop's.
        let operator = DisplayBounds::new(0, 0, 1920, 1080);
        assert_eq!(
            direction(operator, DisplayBounds::new(1280, 0, 1280, 720)),
            Right
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(-900, 0, 1280, 720)),
            Left
        );
        assert_eq!(
            direction(operator, DisplayBounds::new(600, 300, 1000, 800)),
            BelowRight
        );
    }

    #[test]
    fn operator_away_from_origin_uses_actual_arrangement() {
        // The primary projector sits at the origin; the operator laptop is to
        // its right and lower, so the projector is up-left of the operator.
        let topology = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                display(2, 1920, 1080, 1366, 768),
            ],
            Some(DisplayId::new(2)),
        );
        assert_eq!(
            topology.direction_from_operator(DisplayId::new(1)),
            Some(AboveLeft)
        );
        assert_eq!(
            topology.direction(DisplayId::new(1), DisplayId::new(2)),
            Some(BelowRight)
        );

        let topology = Topology::new(
            vec![
                display(7, 0, 0, 1920, 1080),
                display(8, -1920, 0, 1920, 1080),
            ],
            Some(DisplayId::new(7)),
        );
        assert_eq!(
            topology.direction_from_operator(DisplayId::new(8)),
            Some(Left)
        );

        let topology = Topology::new(
            vec![
                display(3, 2560, -1440, 2560, 1440),
                display(4, 2560, 0, 1920, 1080),
            ],
            Some(DisplayId::new(4)),
        );
        assert_eq!(
            topology.direction_from_operator(DisplayId::new(3)),
            Some(Above)
        );
    }

    #[test]
    fn unknown_ids_and_operators_have_no_direction() {
        let topology = Topology::new(vec![display(1, 0, 0, 1920, 1080)], Some(DisplayId::new(9)));
        assert_eq!(topology.operator(), None);
        assert_eq!(topology.direction_from_operator(DisplayId::new(1)), None);
        assert_eq!(
            topology.direction(DisplayId::new(1), DisplayId::new(2)),
            None
        );
    }

    #[test]
    fn aspect_ratio_reports_width_over_height() {
        let evidence = display(1, 0, 0, 1366, 768).evidence;
        let ratio = evidence.aspect_ratio().unwrap();
        assert!((ratio - 1366.0 / 768.0).abs() < f64::EPSILON);
        assert_eq!(DisplayBounds::new(0, 0, 0, 768).aspect_ratio(), None);
    }

    #[test]
    fn placement_rounds_for_evidence_and_keeps_its_exact_value() {
        let placement = PlacementBounds::new(1706.6666, -0.4, 1365.3334, -1.0);
        assert_eq!(placement.rounded(), DisplayBounds::new(1707, 0, 1365, 0));
        assert_eq!(
            PlacementBounds::from(DisplayBounds::new(-1280, 0, 1280, 720)),
            PlacementBounds::new(-1280.0, 0.0, 1280.0, 720.0)
        );
    }

    #[test]
    fn recognition_tolerates_a_new_os_id() {
        let saved = named(display(2, 1920, 0, 1920, 1080), "Projector", Some("u-2")).evidence;
        let topology = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(40, 1920, 0, 1920, 1080), "Projector", Some("u-2")),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(
            recognize(&saved, &topology),
            Recognition::Recognized(DisplayId::new(40))
        );
    }

    #[test]
    fn same_uuid_with_a_different_resolution_is_not_recognized() {
        // Windows derives the uuid from the output port, so a reused port can
        // carry a different display with the same uuid.
        let saved = named(display(2, 1920, 0, 1280, 720), "Projector", Some("u-2")).evidence;
        let topology = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(5, 1920, 0, 1920, 1080), "Projector", Some("u-2")),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(recognize(&saved, &topology), Recognition::Missing);
    }

    #[test]
    fn recognition_falls_back_to_name_and_resolution_then_position() {
        let saved = named(display(2, 1920, 0, 1280, 720), "Projector", None).evidence;
        let moved = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(5, -1280, 0, 1280, 720), "Projector", None),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(
            recognize(&saved, &moved),
            Recognition::Recognized(DisplayId::new(5))
        );

        let twins = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(5, -1280, 0, 1280, 720), "Projector", None),
                named(display(6, 1920, 0, 1280, 720), "Projector", None),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(
            recognize(&saved, &twins),
            Recognition::Recognized(DisplayId::new(6))
        );
    }

    #[test]
    fn uuid_narrows_several_matches_before_position() {
        let saved = named(display(2, 1920, 0, 1920, 1080), "Projector", Some("u-b")).evidence;
        let topology = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(5, -1920, 0, 1920, 1080), "Projector", Some("u-a")),
                named(display(6, 0, -1080, 1920, 1080), "Projector", Some("u-b")),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(
            recognize(&saved, &topology),
            Recognition::Recognized(DisplayId::new(6))
        );
    }

    #[test]
    fn ambiguous_evidence_lists_candidates() {
        let saved = display(2, 1920, 0, 1920, 1080).evidence;
        let topology = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                display(5, -1920, 0, 1920, 1080),
                display(6, 0, -1080, 1920, 1080),
            ],
            Some(DisplayId::new(1)),
        );
        assert_eq!(
            recognize(&saved, &topology),
            Recognition::Ambiguous(vec![DisplayId::new(5), DisplayId::new(6)])
        );
    }

    #[test]
    fn recognition_needs_a_known_operator_display() {
        let saved = named(display(2, 1920, 0, 1920, 1080), "Projector", Some("u-2")).evidence;
        let topology = Topology::new(
            vec![
                display(1, 0, 0, 1920, 1080),
                named(display(5, 1920, 0, 1920, 1080), "Projector", Some("u-2")),
            ],
            None,
        );
        assert_eq!(recognize(&saved, &topology), Recognition::Missing);
    }

    #[test]
    fn operator_display_is_never_recognized() {
        let saved = named(display(1, 0, 0, 1920, 1080), "Laptop", Some("u-1")).evidence;
        let topology = Topology::new(
            vec![named(display(1, 0, 0, 1920, 1080), "Laptop", Some("u-1"))],
            Some(DisplayId::new(1)),
        );
        assert_eq!(recognize(&saved, &topology), Recognition::Missing);
        assert_eq!(
            recognize(&saved, &Topology::default()),
            Recognition::Missing
        );
    }
}
