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
    primitives::{button::Button, record::Marker},
    tokens,
};

use super::{layout::HEADER_HEIGHT, rail::OnPress};

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
pub fn header(
    model: HeaderModel,
    on_cue_view: OnCueView,
    on_reference: OnPress,
) -> impl IntoElement {
    let environment = model.environment;
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let health_color = match model.health {
        DisplayHealth::Connected => palette.live,
        DisplayHealth::NotEnabled => palette.text_muted,
        DisplayHealth::BackNotResumed => palette.warning,
        DisplayHealth::Lost => palette.danger,
    };
    // Shape as well as color: a dot is healthy, a diamond is waiting on the
    // operator, a square is off or lost.
    let shape = match model.health {
        DisplayHealth::Connected => Marker::Dot,
        DisplayHealth::BackNotResumed => Marker::Diamond,
        DisplayHealth::NotEnabled | DisplayHealth::Lost => Marker::Square,
    };
    let gap = px(scale.apply(tokens::spacing::CONTROL_INSET));

    div()
        .id("presenter-header")
        .debug_selector(|| "presenter-header".into())
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
                .child(super::dock::marker(shape, health_color, 10.0, environment))
                .child(model.health.label()),
        )
        .child(shortcuts_control(
            model.icon_only,
            environment,
            on_reference,
        ))
        .child(cue_view_control(
            model.cue_view,
            model.icon_only,
            environment,
            on_cue_view,
        ))
}

/// The labeled Keyboard shortcuts entry (`MVP-PLAN.md` section 8.3, "shortcut
/// help"). It opens the same panel `Ctrl+/` toggles and never touches Program.
fn shortcuts_control(
    icon_only: bool,
    environment: UiEnvironment,
    on_reference: OnPress,
) -> impl IntoElement {
    let palette = tokens::palette(environment.appearance());
    let mut button = Button::new(
        "header-keyboard-shortcuts",
        crate::shortcut_reference::TITLE,
        environment,
    )
    .icon(ui::assets::icon::CIRCLE_QUESTION_MARK)
    .quiet()
    .on_surface(palette.surface)
    .on_activate(move |window, cx| on_reference(window, cx));
    if icon_only {
        button = button.icon_only();
    }
    div()
        .debug_selector(|| "header-shortcuts".into())
        .flex_none()
        .child(button)
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
        .child(segment(
            CueView::Grid,
            "Grid",
            ui::assets::icon::LAYOUT_GRID,
        ))
        .child(segment(CueView::List, "List", ui::assets::icon::LIST))
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
