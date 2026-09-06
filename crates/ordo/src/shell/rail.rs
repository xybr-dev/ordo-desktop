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
    pub const ANCHORED: [Self; 3] = [Self::Settings, Self::Help, Self::Diagnostics];

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
            Self::Themes => icon::SWATCH_BOOK,
            Self::TransferAndBackup => icon::DATABASE_BACKUP,
            Self::Settings => icon::SETTINGS,
            Self::Help => icon::CIRCLE_QUESTION_MARK,
            Self::Diagnostics => icon::ACTIVITY,
        }
    }

    /// The horizontal local views inside this destination, empty where it has
    /// none. `MVP-PLAN.md` names both sets.
    pub fn local_views(self) -> &'static [&'static str] {
        match self {
            Self::Masses => &["Upcoming", "History", "Archive", "Templates"],
            Self::Library => &["Songs", "Responses & Prayers", "General Presentations"],
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

/// The expanded rail's width: an icon, the adopted gap, and enough room for
/// the longest destination label at the operator label size.
const EXPANDED_WIDTH: f32 = 200.0;
/// `DESIGN.md`'s minimum logical list and editor widths. The rail gives up its
/// labels before the workspace gives up a pane, so these are the threshold.
const MINIMUM_WORKSPACE: f32 = 240.0 + 520.0;

pub fn collapsed_width(scale: InterfaceScale) -> f32 {
    scale.apply(tokens::density::DEFAULT_CONTROL + 2.0 * tokens::spacing::RELATED)
}

pub fn width(collapsed: bool, scale: InterfaceScale) -> f32 {
    if collapsed {
        collapsed_width(scale)
    } else {
        scale.apply(EXPANDED_WIDTH)
    }
}

/// `DESIGN.md` decides this from the effective workspace remaining after
/// Interface scale, window size, and navigation chrome — never from a
/// resolution breakpoint — so both terms are scaled and the window is measured.
pub fn collapsed(viewport_width: f32, scale: InterfaceScale) -> bool {
    viewport_width - scale.apply(EXPANDED_WIDTH)
        < scale.apply(MINIMUM_WORKSPACE + 2.0 * tokens::density::WORKSPACE_GUTTER_COMPACT)
}

/// One rail row. Collapsed it is an icon whose tooltip is its accessible name,
/// which is the Button's own contract for an icon-only control.
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
    div()
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        // The non-color half of the active treatment: a marker on the leading
        // edge, so the current destination is legible without the tint.
        .child(
            div()
                .flex_none()
                .w(px(scale.apply(tokens::spacing::UNIT)))
                .h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
                .when(active, |marker| {
                    marker
                        .debug_selector(move || format!("rail-active-{label}"))
                        .bg(rgb(tokens::color::PRIMARY))
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .rounded(px(scale.apply(tokens::radius::CONTROL)))
                .when(active, |row| row.bg(rgb(palette.selection)))
                .child(
                    Button::new(SharedString::from(label), label, environment)
                        .icon(destination.icon())
                        .quiet()
                        .truncate()
                        .when(collapsed, Button::icon_only)
                        .on_surface(if active {
                            palette.selection
                        } else {
                            palette.surface
                        })
                        .on_activate(open),
                ),
        )
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
        .w(px(width(collapsed, scale)))
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
            collapsed_width(Percent100) < width(false, Percent100),
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
