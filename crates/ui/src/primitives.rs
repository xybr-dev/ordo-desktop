//! Ordo-owned primitive Interfaces.
//!
//! Every primitive wraps the `gpui-base` behavior qualified in
//! `docs/qualification/GPUI-BASE-T0.3.md` and owns all of its own presentation.
//! No `gpui-base` type appears in a signature here, so replacing an adopted
//! behavior stays a change to one wrapper.

pub mod button;
pub mod disclosure;
pub mod field;
pub mod list;
pub mod selection;

use gpui::{
    BoxShadow, IntoElement, ParentElement, SharedString, Styled, div, point, prelude::*, px, rgb,
    svg,
};

use crate::{environment::UiEnvironment, tokens};

/// Whether a control can be used, and why not when it cannot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Availability {
    #[default]
    Available,
    /// Obviously inapplicable here: natively disabled, out of the tab order,
    /// neutral subdued styling, and no explanation owed.
    Disabled,
    /// Consequentially blocked: still focusable and named, with activation
    /// suppressed and the reason rendered beside the control. `DESIGN.md`
    /// forbids a hover-only tooltip as the sole explanation, so the reason is
    /// never carried by the tooltip alone.
    Unavailable(SharedString),
}

impl Availability {
    /// The word `DESIGN.md` requires an Unavailable Cue to pair with its icon
    /// and reason, so no caller spells it differently.
    pub const UNAVAILABLE: &str = "Unavailable";

    pub fn unavailable(reason: impl Into<SharedString>) -> Self {
        Self::Unavailable(reason.into())
    }

    pub fn is_available(&self) -> bool {
        *self == Self::Available
    }

    pub fn is_disabled(&self) -> bool {
        *self == Self::Disabled
    }

    pub fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Unavailable(reason) => Some(reason),
            _ => None,
        }
    }
}

/// The two rings `DESIGN.md` requires, as box shadows so neither layer takes
/// part in layout and focus cannot reflow the control it lands on.
///
/// `surface` is the color immediately outside the control: the separation ring
/// matches it so the focus ring stays distinct on primary, semantic-state,
/// destructive, and informational controls alike. The rings are ordered
/// outermost first, because GPUI paints box shadows in list order and the
/// separation ring has to cover the focus ring's inner half.
///
/// Ordo paints this in every platform mode. `DESIGN.md` allows suppressing
/// operating-system focus indication precisely when the complete Ordo treatment
/// replaces it, and MILESTONES.md's `[MANUAL]` gate asks for the treatment to be
/// *preserved* under Windows High Contrast — a GPU-composited window gets no
/// system repaint to defer to, so standing down would leave no indication at all.
pub fn focus_rings(environment: UiEnvironment, surface: u32) -> Vec<BoxShadow> {
    let scale = environment.preferences().scale;
    let separation = scale.apply(tokens::border::FOCUS_SEPARATION);
    let outer = separation + scale.apply(tokens::border::FOCUS_OUTER);
    let ring = |color: u32, spread: f32| BoxShadow {
        color: rgb(color).into(),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(spread),
        inset: false,
    };
    vec![
        ring(tokens::palette(environment.appearance()).focus, outer),
        ring(surface, separation),
    ]
}

/// Applies the treatment through GPUI's own focus/input modality, so it appears
/// for keyboard navigation and never after pointer activation.
pub fn focus_visible<E: gpui::InteractiveElement>(
    element: E,
    environment: UiEnvironment,
    surface: u32,
) -> E {
    let rings = focus_rings(environment, surface);
    element.focus_visible(move |style| style.shadow(rings))
}

/// Reveals a truncated value's full text on pointer hover, which `DESIGN.md`
/// requires of every visually truncated value. It is attached unconditionally:
/// measuring whether a given string actually clipped costs a frame and buys
/// nothing, since the tooltip only ever repeats what is already there.
pub fn truncated<E: gpui::StatefulInteractiveElement>(
    element: E,
    text: impl Into<SharedString>,
    environment: UiEnvironment,
) -> E {
    let text = text.into();
    element
        .tooltip_show_delay(button::TOOLTIP_SHOW_DELAY)
        .tooltip(move |_, cx| button::tooltip_view(text.clone(), environment, cx))
}

/// A persistent line of explanation beside a control: an icon, then the state
/// word and a short reason. `DESIGN.md` requires the explanation to stay on
/// screen, so no primitive is allowed to reduce one of these to a tooltip.
pub(crate) fn note(
    icon: &'static str,
    color: u32,
    text: String,
    selector: &'static str,
    environment: UiEnvironment,
) -> impl IntoElement {
    let scale = environment.preferences().scale;
    div()
        .debug_selector(move || selector.into())
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .text_color(rgb(color))
        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
        .child(
            svg()
                .flex_none()
                .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                .text_color(rgb(color))
                .data(icon.as_bytes()),
        )
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{InterfaceScale, ResolvedAppearance, UiPreferences};

    fn environment(scale: InterfaceScale) -> UiEnvironment {
        UiEnvironment::new(
            UiPreferences {
                scale,
                ..Default::default()
            },
            ResolvedAppearance::Light,
        )
    }

    /// The half of the focus treatment that is observable without a screen:
    /// GPUI exposes no computed style to a test, so the rings themselves are
    /// asserted here and their keyboard-only gating is left to `focus_visible`.
    #[test]
    fn focus_is_two_rings_that_grow_with_interface_scale() {
        let surface = tokens::color::PREPARATION_SURFACE;
        let rings = focus_rings(environment(InterfaceScale::Percent100), surface);

        assert_eq!(rings.len(), 2);
        assert_eq!(rings[0].color, rgb(tokens::color::FOCUS_PREPARATION).into());
        assert_eq!(rings[0].spread_radius, px(4.0));
        assert_eq!(rings[1].color, rgb(surface).into());
        assert_eq!(rings[1].spread_radius, px(2.0));
        for ring in &rings {
            assert_eq!(
                ring.blur_radius,
                px(0.0),
                "a focus ring is a ring, not a glow"
            );
        }

        let enlarged = focus_rings(environment(InterfaceScale::Percent200), surface);
        assert_eq!(enlarged[0].spread_radius, px(8.0));
        assert_eq!(enlarged[1].spread_radius, px(4.0));
    }

    #[test]
    fn focus_uses_the_appearance_that_is_resolved_not_the_preference() {
        let mut dark = environment(InterfaceScale::Percent100);
        dark.apply(crate::environment::EnvironmentChange::EnterPresenter);
        assert_eq!(
            focus_rings(dark, tokens::color::PRESENTER_SURFACE)[0].color,
            rgb(tokens::color::FOCUS_PRESENTER).into()
        );
    }
}
