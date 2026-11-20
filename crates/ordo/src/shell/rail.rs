//! The vertical application rail: Ordo's global destinations.
//!
//! `MVP-PLAN.md` makes the rail the only owner of global navigation, and
//! horizontal tabs the only owner of local views inside one destination. The
//! two never trade places, so the rail is a list of destinations and nothing
//! else.

use gpui::{App, Div, ParentElement, SharedString, Styled, Window, div, prelude::*, px, rgb};
use ui::{
    assets::icon,
    environment::{InterfaceScale, UiEnvironment},
    motion::{self, Track},
    primitives::nav::NavEntry,
    tokens,
};

/// The Library local view that hosts the persistent Songs workspace. The
/// shell compares against this rather than a literal label.
pub const SONGS_VIEW: &str = "Songs";

/// A global destination. The first four are the parish's work; the last three
/// are the installation's, and `DESIGN.md` anchors them at the bottom of the
/// same rail rather than hiding them behind a menu.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Destination {
    #[default]
    Masses,
    Library,
    Themes,
    TransferAndBackup,
    Settings,
    Help,
    Diagnostics,
}

impl Destination {
    pub const PRIMARY: [Self; 4] = [
        Self::Masses,
        Self::Library,
        Self::Themes,
        Self::TransferAndBackup,
    ];
    pub const ANCHORED: [Self; 3] = [Self::Diagnostics, Self::Help, Self::Settings];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Masses => "Masses",
            Self::Library => "Library",
            Self::Themes => "Themes",
            Self::TransferAndBackup => "Transfer & Backup",
            Self::Settings => "Settings",
            Self::Help => "Help",
            Self::Diagnostics => "Diagnostics",
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::Masses => icon::CALENDAR_DAYS,
            Self::Library => icon::LIBRARY,
            Self::Themes => icon::PALETTE,
            Self::TransferAndBackup => icon::HARD_DRIVE_DOWNLOAD,
            Self::Settings => icon::SETTINGS,
            Self::Help => icon::CIRCLE_QUESTION_MARK,
            Self::Diagnostics => icon::ACTIVITY,
        }
    }

    /// The filled form shown while this is the current destination
    /// (Decision 300).
    const fn filled_icon(self) -> &'static str {
        match self {
            Self::Masses => icon::filled::CALENDAR_DAYS,
            Self::Library => icon::filled::LIBRARY,
            Self::Themes => icon::filled::PALETTE,
            Self::TransferAndBackup => icon::filled::HARD_DRIVE_DOWNLOAD,
            Self::Settings => icon::filled::SETTINGS,
            Self::Help => icon::filled::CIRCLE_QUESTION_MARK,
            Self::Diagnostics => icon::filled::ACTIVITY,
        }
    }

    /// The horizontal local views inside this destination, empty where it has
    /// none. `MVP-PLAN.md` names both sets; Decision 278 adds Choirs.
    pub fn local_views(self) -> &'static [&'static str] {
        match self {
            Self::Masses => &["Upcoming", "History", "Archive", "Templates"],
            Self::Library => &[
                SONGS_VIEW,
                "Responses & Prayers",
                "General Presentations",
                "Choirs",
            ],
            _ => &[],
        }
    }

    /// Whether this destination shows a primary list beside its editor.
    /// Settings, Help, and Diagnostics are single surfaces, so their Primary
    /// list region is absent rather than empty — which is also what the region
    /// cycle has to skip.
    pub fn has_primary_list(self) -> bool {
        Self::PRIMARY.contains(&self)
    }
}

/// The rail's width, fixed rather than sized by its labels: each entry stacks
/// its icon over its label, and a label too long for one line ("Transfer &
/// Backup") wraps under its icon instead of widening the rail (Decision 300).
pub const EXPANDED_WIDTH: f32 = 76.0;
/// `DESIGN.md`'s minimum logical list and editor widths. The rail gives up its
/// labels before the workspace gives up a pane, so these are the threshold.
const MINIMUM_WORKSPACE: f32 = 240.0 + 520.0;
/// The pill that marks the current destination: 3px wide and 16px tall, with
/// rounded ends (Decision 300).
const PILL_WIDTH: f32 = 3.0;
const PILL_HEIGHT: f32 = 16.0;
const PILL_RADIUS: f32 = 2.0;

/// A collapsed rail is exactly one square icon control plus the rail's inset
/// and its separator. The pill is out of flow, so it adds nothing.
pub fn collapsed_width(scale: InterfaceScale) -> f32 {
    scale.apply(tokens::density::DEFAULT_CONTROL)
        + 2.0 * scale.apply(tokens::spacing::RELATED)
        + tokens::border::SEPARATOR
}

/// `DESIGN.md` decides this from the effective workspace remaining after
/// Interface scale, window size, and navigation chrome — never from a
/// resolution breakpoint — so both terms are scaled and the window is measured.
/// The gutter reserved is the wide one, because that is what an expanded rail
/// renders: measuring the branch it is deciding against the other branch's
/// gutter would leave the workspace narrower than the minimum this protects.
pub fn collapsed(viewport_width: f32, scale: InterfaceScale) -> bool {
    viewport_width - (scale.apply(EXPANDED_WIDTH) + tokens::border::SEPARATOR)
        < scale.apply(MINIMUM_WORKSPACE + 2.0 * tokens::density::WORKSPACE_GUTTER_WIDE)
}

/// Where each destination sits among the rail's children: the origin probe
/// first, then the parish's four, the spacer, and the installation's three.
fn child_index(destination: Destination) -> usize {
    if let Some(at) = Destination::PRIMARY.iter().position(|d| *d == destination) {
        1 + at
    } else {
        let at = Destination::ANCHORED
            .iter()
            .position(|d| *d == destination)
            .expect("every destination is on the rail");
        2 + Destination::PRIMARY.len() + at
    }
}

/// The rail: the parish's destinations at the top and the installation's
/// anchored at the bottom, with one pill marking the current one.
///
/// The pill belongs to the rail rather than to an entry, so it can travel
/// between them (Decision 301). It is placed from where the rail last laid its
/// entries out, which `track` records. `MILESTONES.md` asks the current
/// destination to be non-color-only: the pill, the filled icon, and the entry
/// showing its icon alone are that cue, and `aria-selected` is its assistive
/// half.
pub fn render(
    active: Destination,
    collapsed: bool,
    track: &Track,
    environment: UiEnvironment,
    open: impl Fn(Destination) -> Box<dyn Fn(&mut Window, &mut App)>,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let entry = |destination: Destination| {
        let label = destination.label();
        let open = open(destination);
        NavEntry::new(
            SharedString::from(label),
            label,
            destination.icon(),
            destination.filled_icon(),
            environment,
        )
        .current(destination == active)
        .compact(collapsed)
        .on_activate(move |window, cx| open(window, cx))
    };

    // The pill's place relative to the rail, from the last layout. Until the
    // rail has been laid out once there is nowhere to put it; the first layout
    // redraws the window, so that lasts a single frame.
    let height = scale.apply(PILL_HEIGHT);
    let place = track
        .get(0)
        .zip(track.get(child_index(active)))
        .map(|(rail, entry)| {
            (
                f32::from(entry.origin.x - rail.origin.x),
                f32::from(entry.origin.y - rail.origin.y)
                    + (f32::from(entry.size.height) - height) / 2.0,
            )
        });
    let pill = place.map(|(left, top)| {
        // `transition` adopts its first target on the spot, so the pill is
        // simply there at startup and only travels on a change. Under reduced
        // motion it jumps, as Decision 301 asks.
        let top = motion::travel("rail-pill", child_index(active), top, window, cx);
        let label = active.label();
        div()
            .debug_selector(move || format!("rail-active-{label}"))
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(PILL_WIDTH))
            .h(px(height))
            .rounded(px(PILL_RADIUS))
            .bg(rgb(palette.text))
    });

    div()
        .debug_selector(|| "shell-rail".into())
        .relative()
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::UNIT)))
        .when(collapsed, |rail| {
            rail.p(px(scale.apply(tokens::spacing::RELATED)))
        })
        .when(!collapsed, |rail| {
            rail.w(px(scale.apply(EXPANDED_WIDTH) + tokens::border::SEPARATOR))
                .py(px(scale.apply(tokens::spacing::RELATED)))
                .px(px(scale.apply(tokens::spacing::UNIT)))
        })
        .border_r(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .bg(rgb(palette.surface))
        .on_children_prepainted(track.record())
        // The origin probe: an empty box over the whole rail, so the recorded
        // bounds carry the rail's own origin as their first entry.
        .child(div().absolute().inset_0())
        .children(Destination::PRIMARY.map(&entry))
        .child(div().flex_1())
        .children(Destination::ANCHORED.map(&entry))
        .children(pill)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule the collapse is written from: labels survive while the
    /// workspace still has `DESIGN.md`'s list-plus-editor minimum, and go when
    /// it does not — at either qualified viewport, depending only on scale.
    #[test]
    fn the_rail_gives_up_its_labels_before_the_workspace_gives_up_a_pane() {
        use InterfaceScale::*;
        let expected = [
            // 1366x768
            ((1366.0, Percent100), false),
            ((1366.0, Percent125), false),
            ((1366.0, Percent150), false),
            ((1366.0, Percent175), true),
            ((1366.0, Percent200), true),
            // 1920x1080: the stacked rail is narrow enough to keep its labels
            // at every Interface scale.
            ((1920.0, Percent100), false),
            ((1920.0, Percent150), false),
            ((1920.0, Percent200), false),
            // A narrow window collapses it at any scale.
            ((800.0, Percent100), true),
        ];
        for ((width, scale), collapse) in expected {
            assert_eq!(
                collapsed(width, scale),
                collapse,
                "{width} at {}%",
                scale.percent()
            );
        }
        for scale in [Percent100, Percent200] {
            assert!(
                collapsed_width(scale) < scale.apply(EXPANDED_WIDTH),
                "collapsing has to give the workspace width back"
            );
        }
    }

    #[test]
    fn every_destination_is_reachable_and_local_views_belong_only_to_a_destination() {
        let all: Vec<_> = Destination::PRIMARY
            .into_iter()
            .chain(Destination::ANCHORED)
            .collect();
        assert_eq!(all.len(), 7);
        assert_eq!(
            Destination::Masses.local_views(),
            ["Upcoming", "History", "Archive", "Templates"]
        );
        for destination in all {
            assert!(!destination.label().is_empty());
            assert!(!destination.icon().is_empty());
            assert_ne!(destination.icon(), destination.filled_icon());
            // A destination is navigation; only its local views are tabs.
            assert_eq!(
                destination.has_primary_list(),
                Destination::PRIMARY.contains(&destination)
            );
        }
    }
}
