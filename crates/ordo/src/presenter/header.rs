//! The Presenter header: Mass title, display health, and the Grid/List
//! control (T1.4 WP4).
//!
//! There is no clock yet: a local-time clock arrives with the T1.6 platform
//! module (the owner chose no clock over a UTC one).
//!
//! The header only draws what the view hands it. The Grid/List control is the
//! one thing here that is not Engine state: it changes the operator's
//! preference, never an output, so it reports through a callback and the
//! owning view decides what to save.

use std::rc::Rc;

use gpui::{
    App, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px, rgb,
};
use presentation::state::LiveState;
use ui::{
    environment::{CueView, UiEnvironment},
    primitives::button::Button,
    tokens,
};

use super::layout::HEADER_HEIGHT;

/// Lucide `layout-grid`, kept here because the bundled icon set has no
/// layout icons yet. Same format and licence as the files in
/// `crates/ui/assets/icons/lucide`.
const GRID_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="7" height="7" x="3" y="3" rx="1" /><rect width="7" height="7" x="14" y="3" rx="1" /><rect width="7" height="7" x="14" y="14" rx="1" /><rect width="7" height="7" x="3" y="14" rx="1" /></svg>"#;

/// Lucide `list`.
const LIST_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h.01" /><path d="M3 18h.01" /><path d="M3 6h.01" /><path d="M8 12h13" /><path d="M8 18h13" /><path d="M8 6h13" /></svg>"#;

/// What the header says about the congregation display.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayHealth {
    /// Output is not enabled yet; nothing is wrong.
    NotEnabled,
    /// Output is enabled and the display is there.
    Connected,
    /// The display is gone.
    Lost,
    /// The recognized display is back but output is not resumed.
    BackNotResumed,
}

impl DisplayHealth {
    /// Health as a function of the Engine live state alone.
    pub fn of(live: LiveState) -> Self {
        match live {
            LiveState::OutputDisabled => Self::NotEnabled,
            LiveState::EnabledBlack | LiveState::Live | LiveState::Frozen | LiveState::Blackout => {
                Self::Connected
            }
            LiveState::Disconnected {
                display_back: false,
            } => Self::Lost,
            LiveState::Disconnected { display_back: true } => Self::BackNotResumed,
        }
    }

    /// The word the operator reads. The state is also carried by the dot's
    /// shape, so color is never the only signal.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotEnabled => "Display ready · not enabled",
            Self::Connected => "Display connected",
            Self::Lost => "Display lost",
            Self::BackNotResumed => "Display back · not resumed",
        }
    }
}

/// Everything the header draws.
pub struct HeaderModel {
    pub title: SharedString,
    pub health: DisplayHealth,
    pub cue_view: CueView,
    /// Constrained layout: the Grid/List buttons show their icon alone.
    pub icon_only: bool,
    pub environment: UiEnvironment,
}

/// Called with the view the operator chose.
pub type OnCueView = Rc<dyn Fn(CueView, &mut Window, &mut App)>;

/// The header row. Its height is [`HEADER_HEIGHT`] scaled, and it never
/// wraps: the title ellipsizes before anything else gives way.
pub fn header(model: HeaderModel, on_cue_view: OnCueView) -> impl IntoElement {
    let environment = model.environment;
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let health_color = match model.health {
        DisplayHealth::Connected => palette.live,
        DisplayHealth::NotEnabled => palette.text_muted,
        DisplayHealth::BackNotResumed => palette.warning,
        DisplayHealth::Lost => palette.danger,
    };
    // Shape as well as color: a round dot is healthy, a square one is not.
    let round = matches!(model.health, DisplayHealth::Connected);
    let gap = px(scale.apply(tokens::spacing::CONTROL_INSET));

    div()
        .id("presenter-header")
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(gap)
        .h(px(scale.apply(HEADER_HEIGHT)))
        .px(px(scale.apply(tokens::spacing::GROUP)))
        .bg(rgb(palette.surface))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .id("presenter-title")
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                .child(model.title),
        )
        .child(
            div()
                .id("display-health")
                .flex_none()
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .text_color(rgb(health_color))
                .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                .child(
                    div()
                        .size(px(scale.apply(10.0)))
                        .bg(rgb(health_color))
                        .when(round, |dot| dot.rounded_full()),
                )
                .child(model.health.label()),
        )
        .child(cue_view_control(
            model.cue_view,
            model.icon_only,
            environment,
            on_cue_view,
        ))
}

/// The Grid/List segmented control. Each segment is a Button named for its
/// view whether or not it shows a word, and the current one is marked for
/// assistive technology and by a fill.
fn cue_view_control(
    current: CueView,
    icon_only: bool,
    environment: UiEnvironment,
    on_cue_view: OnCueView,
) -> impl IntoElement {
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let segment = move |view: CueView, label: &'static str, icon: &'static str| {
        let chosen = current == view;
        let on_cue_view = on_cue_view.clone();
        let fill = if chosen {
            palette.selection
        } else {
            palette.surface
        };
        let mut button = Button::new(format!("cue-view-{label}"), label, environment)
            .icon(icon)
            .quiet()
            .current(chosen)
            .on_surface(fill)
            .on_activate(move |window, cx| on_cue_view(view, window, cx));
        if icon_only {
            button = button.icon_only();
        }
        div()
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .bg(rgb(fill))
            .child(button)
    };
    div()
        .id("cue-view-control")
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .border(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(segment(CueView::Grid, "Grid", GRID_ICON))
        .child(segment(CueView::List, "List", LIST_ICON))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_words_follow_the_live_state() {
        let cases = [
            (LiveState::OutputDisabled, "Display ready · not enabled"),
            (LiveState::EnabledBlack, "Display connected"),
            (LiveState::Live, "Display connected"),
            (LiveState::Frozen, "Display connected"),
            (LiveState::Blackout, "Display connected"),
            (
                LiveState::Disconnected {
                    display_back: false,
                },
                "Display lost",
            ),
            (
                LiveState::Disconnected { display_back: true },
                "Display back · not resumed",
            ),
        ];
        for (live, word) in cases {
            assert_eq!(DisplayHealth::of(live).label(), word, "{live:?}");
        }
    }
}
