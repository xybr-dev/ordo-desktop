//! The vertical application rail: Ordo's global destinations.
//!
//! `MVP-PLAN.md` makes the rail the only owner of global navigation, and
//! horizontal tabs the only owner of local views inside one destination. The
//! two never trade places, so the rail is a list of destinations and nothing
//! else.

use gpui::{
    App, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px, rgb,
};
use ui::{
    assets::icon,
    environment::{InterfaceScale, UiEnvironment},
    primitives::button::Button,
    tokens,
};

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

    /// The horizontal local views inside this destination, empty where it has
    /// none. `MVP-PLAN.md` names both sets; Decision 278 adds Choirs.
    pub fn local_views(self) -> &'static [&'static str] {
        match self {
            Self::Masses => &["Upcoming", "History", "Archive", "Templates"],
            Self::Library => &[
                "Songs",
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

/// What the collapse decision reserves for an expanded rail. The rail itself
/// is sized by its own content, because Atkinson is wide and a fixed width is
/// a guess that only ever fails one way: a destination whose label is painted
/// past the edge it was told to fit inside. This is the reserve the workspace
/// is measured against, and a test pins it to what the rail actually measures.
pub const EXPANDED_RESERVE: f32 = 240.0;
/// `DESIGN.md`'s minimum logical list and editor widths. The rail gives up its
/// labels before the workspace gives up a pane, so these are the threshold.
const MINIMUM_WORKSPACE: f32 = 240.0 + 520.0;

/// A collapsed rail is exactly one square icon control plus the rail's inset
/// and its separator. The active marker is out of flow, so it adds nothing.
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
    viewport_width - scale.apply(EXPANDED_RESERVE)
        < scale.apply(MINIMUM_WORKSPACE + 2.0 * tokens::density::WORKSPACE_GUTTER_WIDE)
}

/// One rail row. Collapsed it is an icon whose tooltip is its accessible name,
/// which is the Button's own contract for an icon-only control.
///
/// The active marker is the row's own leading edge in both widths:
/// `MILESTONES.md` asks the active destination to be non-color-only, and
/// `Button::current` is the assistive half of that, not the visible one. It is
/// drawn as the left border of a quad the size of the row, so it takes the
/// row's corner rather than approximating it, and it is out of flow, so the
/// only difference between the two widths is what the row reserves beside it:
/// expanded, every row reserves the bar's width and the icons line up down the
/// rail; collapsed, nothing is reserved and the single icon stays centred.
fn item(
    destination: Destination,
    active: bool,
    collapsed: bool,
    environment: UiEnvironment,
    open: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let label = destination.label();
    let corner = px(scale.apply(tokens::radius::CONTROL));
    let bar = px(scale.apply(tokens::spacing::UNIT));
    // What a focused control's rings need beside it. They are outset box
    // shadows, so they are painted outside the Button's own bounds and the row
    // has to leave them somewhere to land — see the reserve below and the
    // absent `overflow_hidden` above it.
    let rings = px(scale.apply(tokens::border::FOCUS_SEPARATION + tokens::border::FOCUS_OUTER));
    // The marker is the leading edge of a quad the size of the whole row, drawn
    // as that quad's left border, so its curve is the row's own corner and
    // matches the control beside it exactly. A quad only the bar's own width
    // cannot do that: GPUI clamps a corner radius to half the quad's smaller
    // side, so a four-pixel bar asking for the four-pixel control radius was
    // given two, and came out visibly tighter than everything around it. Out of
    // flow it neither moves the icon nor takes the pointer — a div with nothing
    // but a border inserts no hitbox — and being the row's last child it is
    // drawn over the control's hover fill rather than under it.
    let marker = || {
        div()
            .absolute()
            .inset_0()
            .rounded(corner)
            .border_l(bar)
            .border_color(rgb(tokens::color::PRIMARY))
            .debug_selector(move || format!("rail-active-{label}"))
    };
    div()
        .flex()
        .items_center()
        .relative()
        .rounded(corner)
        // No `overflow_hidden`. GPUI masks a child's paint to this row's
        // rectangle, box shadows included, and Ordo's focus treatment is two
        // outset shadows — so clipping here erased the ring from every rail
        // destination, in the first region `F6` reaches. Nothing needs the clip
        // any more: the marker carries its own corner and the Button rounds its
        // own fill to the same radius.
        .when(active, |row| row.bg(rgb(palette.selection)))
        // Expanded, every row reserves the marker's width so the icons share one
        // edge down the rail whether or not their row is the active one, plus
        // the room a focused control's rings need — without it the ring lands in
        // the marker's band and the two cannot both be seen, which `DESIGN.md`
        // requires of focus and selection alike. Collapsed there is no label to
        // line the icons up against, so nothing is reserved, the single icon
        // stays centred, and the rings fall outside the row entirely.
        .when(!collapsed, |row| {
            row.child(div().flex_none().w(bar + rings))
        })
        // The control takes what the marker leaves, rather than only what its
        // own label needs: a row sized to its label leaves seven fills at seven
        // widths staggered down the rail.
        .child(
            div().flex_1().min_w_0().child(
                Button::new(SharedString::from(label), label, environment)
                    .icon(destination.icon())
                    .quiet()
                    .current(active)
                    // Expanded, a destination is read down the rail with the other
                    // six, so its icon and label start on a shared edge rather than
                    // floating in the middle.
                    .when(!collapsed, Button::stretch)
                    .when(collapsed, Button::icon_only)
                    .on_surface(if active {
                        palette.selection
                    } else {
                        palette.surface
                    })
                    .on_activate(open),
            ),
        )
        .when(active, |row| row.child(marker()))
}

pub fn render(
    active: Destination,
    collapsed: bool,
    environment: UiEnvironment,
    open: impl Fn(Destination) -> Box<dyn Fn(&mut Window, &mut App)>,
) -> impl IntoElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let group = |destinations: &'static [Destination]| {
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .children(destinations.iter().map(|&destination| {
                item(
                    destination,
                    destination == active,
                    collapsed,
                    environment,
                    open(destination),
                )
            }))
    };
    div()
        .debug_selector(|| "shell-rail".into())
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .justify_between()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .p(px(scale.apply(tokens::spacing::RELATED)))
        .border_r(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .bg(rgb(palette.surface))
        .child(group(&Destination::PRIMARY))
        .child(group(&Destination::ANCHORED))
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
            ((1366.0, Percent150), true),
            ((1366.0, Percent200), true),
            // 1920x1080
            ((1920.0, Percent100), false),
            ((1920.0, Percent150), false),
            ((1920.0, Percent175), false),
            ((1920.0, Percent200), true),
        ];
        for ((width, scale), collapse) in expected {
            assert_eq!(
                collapsed(width, scale),
                collapse,
                "{width} at {}%",
                scale.percent()
            );
        }
        assert!(
            collapsed_width(Percent100) < EXPANDED_RESERVE,
            "collapsing has to give the workspace width back"
        );
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
            // A destination is navigation; only its local views are tabs.
            assert_eq!(
                destination.has_primary_list(),
                Destination::PRIMARY.contains(&destination)
            );
        }
    }
}
