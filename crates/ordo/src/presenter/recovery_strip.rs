//! The recovery strip (T1.7d WP7): the persistent, non-blocking row under the
//! session header that says a presentation was recovered.
//!
//! The strip is exactly as long-lived as the Engine's private recovered
//! context: the view shows it while `engine.state().recovered()` is `Some`, so
//! Space, Resume Output, release and Disable Output clear it by clearing the
//! context, and nothing here can dismiss it (no close button, no click, no
//! timer).
//!
//! The prior Program block is private operator context. It draws the frame
//! through the timeline's thumbnail path, never the Program monitor path, so
//! it can neither move `program_drawn()` nor reach the congregation window,
//! and its text says the frame is not on screen now.

use std::sync::Arc;

use gpui::{
    AnyElement, FontWeight, IntoElement, ParentElement, RenderImage, Role, SharedString, Styled,
    div, prelude::*, px, rgb, svg,
};
use presentation::{journal::LiveIntent, state::LiveState};
use rendering::Frame;
use ui::{
    assets::icon,
    environment::{ResolvedAppearance, UiEnvironment},
    tokens,
};

use crate::canonical_views::live_viewport;

/// Unscaled height the layout reserves for the strip: the prior block's mini
/// frame (36 px) with its padding and border, plus the strip's own padding.
pub const STRIP_HEIGHT: f32 = 56.0;

/// Unscaled width of the prior block's mini frame (the mockup's 64 px).
const MINI_WIDTH: f32 = 64.0;

const HEADLINE: &str = "Presentation recovered · Congregation output is black";

/// Fallback when the journal recorded no display name.
const UNNAMED_DISPLAY: &str = "The congregation display";

/// The private context of Program before the interruption.
pub struct PriorModel {
    /// The Cue's frame once the thumbnail store has composed it; `None` draws
    /// the same black placeholder a timeline tile does.
    pub frame: Option<Arc<Frame>>,
    pub image: Option<Arc<RenderImage>>,
    /// `LIVE`, `FROZEN`, ... as [`state_word`] spells the interrupted state.
    pub state: &'static str,
    pub item_title: SharedString,
    /// The timeline's `n.m` number for the Cue.
    pub number: SharedString,
}

impl PriorModel {
    /// The second line of the block.
    pub fn meta(&self) -> String {
        format!(
            "{} on {} · {} · not on screen now",
            self.state, self.item_title, self.number
        )
    }
}

/// Everything the strip draws.
pub struct StripModel {
    pub detail: SharedString,
    /// `None` when nothing was live before the interruption, or when the
    /// Constrained layout leaves no room for the block.
    pub prior: Option<PriorModel>,
    pub environment: UiEnvironment,
}

/// The upper-case word for the state the session was in when it stopped.
pub fn state_word(interrupted: LiveIntent) -> &'static str {
    match interrupted {
        LiveIntent::Live => "LIVE",
        LiveIntent::Frozen => "FROZEN",
        LiveIntent::Blackout => "BLACKOUT",
        LiveIntent::EnabledBlack => "BLACK",
        LiveIntent::Disconnected => "DISCONNECTED",
        LiveIntent::OutputDisabled => "OUTPUT OFF",
    }
}

/// What the operator needs to do next, from the Engine's live state now (not
/// from the journal, which only says what the interrupted session was doing).
pub fn detail(closed_at: &str, live: LiveState, display_name: Option<&str>) -> String {
    let lead = format!("Ordo closed unexpectedly at {closed_at}. Your place is back.");
    match live {
        LiveState::Disconnected { .. } => format!(
            "{lead} {} wasn’t recognized, so output is disconnected.",
            display_name.unwrap_or(UNNAMED_DISPLAY)
        ),
        LiveState::OutputDisabled => {
            format!("{lead} Enable Output, then Space takes Preview live.")
        }
        _ => format!("{lead} Space takes Preview live."),
    }
}

/// The text a screen reader announces: headline then detail.
pub fn announcement(detail: &str) -> String {
    format!("{HEADLINE}. {detail}")
}

/// The information color for the appearance (Presenter lightens it).
fn info_color(environment: UiEnvironment) -> u32 {
    match environment.appearance() {
        ResolvedAppearance::Light => tokens::color::INFORMATION_PREPARATION,
        ResolvedAppearance::Dark => tokens::color::INFORMATION_PRESENTER,
    }
}

/// The strip row. Full width, a status node for assistive technology, and no
/// interactive child.
pub fn strip(model: StripModel) -> impl IntoElement {
    let environment = model.environment;
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let info = info_color(environment);
    let label = announcement(&model.detail);

    div()
        .id("recovery-strip")
        .debug_selector(|| "recovery-strip".into())
        .role(Role::Status)
        .aria_label(label)
        .flex_none()
        .w_full()
        .min_h(px(scale.apply(STRIP_HEIGHT)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(scale.apply(tokens::spacing::GROUP)))
        .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .bg(rgb(palette.surface))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(info))
        .text_size(px(scale.apply(14.0)))
        .child(
            svg()
                .flex_none()
                .size(px(scale.apply(16.0)))
                .text_color(rgb(info))
                .data(icon::INFO.as_bytes()),
        )
        .child(
            div()
                .id("recovery-text")
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_x(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .text_color(rgb(palette.text))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .whitespace_nowrap()
                        .child(HEADLINE),
                )
                .child(
                    div()
                        .text_color(rgb(palette.text_muted))
                        .child(model.detail),
                ),
        )
        .children(model.prior.map(|prior| prior_block(prior, environment)))
}

/// The dashed "Before the interruption" block: a mini frame and two lines.
fn prior_block(prior: PriorModel, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let meta = prior.meta();
    div()
        .id("recovery-prior")
        .debug_selector(|| "recovery-prior".into())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(scale.apply(tokens::spacing::RELATED)))
        .py(px(2.0))
        .border(px(tokens::border::SEPARATOR))
        .border_dashed()
        .border_color(rgb(palette.border_strong))
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .text_size(px(scale.apply(13.0)))
        .child(
            div()
                .debug_selector(|| "recovery-prior-mini".into())
                .flex_none()
                .child(live_viewport(
                    prior.frame,
                    prior.image,
                    scale.apply(MINI_WIDTH),
                    None,
                    environment,
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .text_color(rgb(palette.text))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child("Before the interruption"),
                )
                .child(
                    div()
                        .debug_selector(|| "recovery-prior-meta".into())
                        .text_color(rgb(palette.text_muted))
                        .child(meta),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISCONNECTED: LiveState = LiveState::Disconnected {
        display_back: false,
    };

    #[test]
    fn detail_follows_the_live_state() {
        assert_eq!(
            detail("9:31", LiveState::EnabledBlack, None),
            "Ordo closed unexpectedly at 9:31. Your place is back. Space takes Preview live."
        );
        assert_eq!(
            detail("9:31", LiveState::OutputDisabled, None),
            "Ordo closed unexpectedly at 9:31. Your place is back. Enable Output, then Space \
             takes Preview live."
        );
        assert_eq!(
            detail("9:31", DISCONNECTED, Some("Display 2")),
            "Ordo closed unexpectedly at 9:31. Your place is back. Display 2 wasn’t \
             recognized, so output is disconnected."
        );
        assert_eq!(
            detail("9:31", DISCONNECTED, None),
            "Ordo closed unexpectedly at 9:31. Your place is back. The congregation display \
             wasn’t recognized, so output is disconnected."
        );
    }

    #[test]
    fn every_interrupted_state_has_its_word() {
        let words = [
            LiveIntent::Live,
            LiveIntent::Frozen,
            LiveIntent::Blackout,
            LiveIntent::EnabledBlack,
            LiveIntent::Disconnected,
            LiveIntent::OutputDisabled,
        ]
        .map(state_word);
        assert_eq!(
            words,
            [
                "LIVE",
                "FROZEN",
                "BLACKOUT",
                "BLACK",
                "DISCONNECTED",
                "OUTPUT OFF"
            ]
        );
    }

    #[test]
    fn the_announcement_is_headline_then_detail() {
        assert_eq!(
            announcement("Space takes Preview live."),
            "Presentation recovered · Congregation output is black. Space takes Preview live."
        );
    }
}
