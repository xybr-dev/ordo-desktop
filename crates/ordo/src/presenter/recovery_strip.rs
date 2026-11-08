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

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, RenderImage, Role, SharedString,
    Styled, Window, div, prelude::*, px, rgb, svg,
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

/// The prior block's accessible name: what a click does.
const SHOW_IN_PREVIEW: &str = "Show in Preview";

const HEADLINE: &str = "Presentation recovered · Congregation output is black";

/// Fallback when the journal recorded no display name.
const UNNAMED_DISPLAY: &str = "The congregation display";

/// The private context of Program before the interruption.
pub struct PriorModel {
    /// The flat index of the prior Program Cue, which a click shows in Preview.
    pub flat: usize,
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
///
/// `display_recognized` is whether startup recognized the journaled display.
/// A `Disconnected` Engine then has three different causes: the display was
/// not recognized, it was recognized and later lost (or its window failed to
/// open), or it has come back since.
pub fn detail(
    closed_at: &str,
    live: LiveState,
    display_name: Option<&str>,
    display_recognized: bool,
) -> String {
    let lead = format!("Ordo closed unexpectedly at {closed_at}. Your place is back.");
    let display = display_name.unwrap_or(UNNAMED_DISPLAY);
    match live {
        LiveState::Disconnected { display_back: true } => {
            format!("{lead} {display} is back. Space resumes output and takes Preview live.")
        }
        LiveState::Disconnected {
            display_back: false,
        } if !display_recognized => {
            format!("{lead} {display} wasn’t recognized, so output is disconnected.")
        }
        LiveState::Disconnected {
            display_back: false,
        } => {
            format!("{lead} {display} is disconnected.")
        }
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

/// Shows a flat Cue in Preview. Navigation only: nothing it does goes live.
pub type OnShowPrior = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The strip row. Full width and a status node for assistive technology. Its
/// only interactive child is the prior block, which navigates Preview; nothing
/// in the strip dismisses it.
pub fn strip(model: StripModel, show_prior: OnShowPrior) -> impl IntoElement {
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
        .h(px(scale.apply(STRIP_HEIGHT)))
        .overflow_hidden()
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
                .data(icon::ROTATE_CCW_CLOCK.as_bytes()),
        )
        .child(
            div()
                .id("recovery-text")
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .overflow_hidden()
                .text_color(rgb(palette.text))
                .child(
                    div()
                        .debug_selector(|| "recovery-headline".into())
                        .font_weight(FontWeight::BOLD)
                        .truncate()
                        .child(HEADLINE),
                )
                .child(
                    div()
                        .debug_selector(|| "recovery-detail".into())
                        .text_color(rgb(palette.text_muted))
                        .truncate()
                        .child(model.detail),
                ),
        )
        .children(
            model
                .prior
                .map(|prior| prior_block(prior, environment, show_prior)),
        )
}

/// The dashed "Before the interruption" block: a mini frame and two lines.
fn prior_block(
    prior: PriorModel,
    environment: UiEnvironment,
    show_prior: OnShowPrior,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let meta = prior.meta();
    let flat = prior.flat;
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
        .hover(|style| style.bg(rgb(palette.surface_secondary)))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(SHOW_IN_PREVIEW)
        .aria_description(meta.clone())
        .on_click(move |_, window, cx| show_prior(flat, window, cx))
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

    const LEAD: &str = "Ordo closed unexpectedly at 9:31. Your place is back.";

    #[test]
    fn detail_follows_the_live_state() {
        assert_eq!(
            detail("9:31", LiveState::EnabledBlack, None, true),
            format!("{LEAD} Space takes Preview live.")
        );
        assert_eq!(
            detail("9:31", LiveState::OutputDisabled, None, true),
            format!("{LEAD} Enable Output, then Space takes Preview live.")
        );
    }

    #[test]
    fn disconnected_detail_names_the_real_situation() {
        let back = LiveState::Disconnected { display_back: true };
        // Not recognized at startup.
        assert_eq!(
            detail("9:31", DISCONNECTED, Some("Display 2"), false),
            format!("{LEAD} Display 2 wasn’t recognized, so output is disconnected.")
        );
        assert_eq!(
            detail("9:31", DISCONNECTED, None, false),
            format!(
                "{LEAD} The congregation display wasn’t recognized, so output is disconnected."
            )
        );
        // Recognized, then lost or the window failed to open.
        assert_eq!(
            detail("9:31", DISCONNECTED, Some("Display 2"), true),
            format!("{LEAD} Display 2 is disconnected.")
        );
        assert_eq!(
            detail("9:31", DISCONNECTED, None, true),
            format!("{LEAD} The congregation display is disconnected.")
        );
        // Back, whichever way it was lost.
        for recognized in [false, true] {
            assert_eq!(
                detail("9:31", back, Some("Display 2"), recognized),
                format!("{LEAD} Display 2 is back. Space resumes output and takes Preview live.")
            );
        }
        assert_eq!(
            detail("9:31", back, None, true),
            format!(
                "{LEAD} The congregation display is back. Space resumes output and takes Preview live."
            )
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
