//! Installation preferences: Preparation appearance and Interface scale.
//!
//! These are the installation's, not the Parish's — `DESIGN.md` keeps operator
//! appearance and Interface scale independent of Parish content and of any
//! Slide Theme — so they are the Settings destination's own screen rather than
//! anything the shell holds on their behalf. The shell supplies the
//! environment and one seam for applying a change; everything else is here.

use gpui::{
    App, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px, rgb, svg,
};
use ui::{
    assets::icon,
    environment::{EnvironmentChange, InterfaceScale, PreparationAppearance, UiEnvironment},
    primitives::button::Button,
    tokens,
};

const SCALE_HELP: &str = "Makes Ordo's own text and controls larger on this computer. Slides are never affected: they're always composed at 1920 × 1080.";
const SCALE_KEYS: &str = "Ctrl++ larger · Ctrl+- smaller · Ctrl+0 100%";

/// Each preference is a row of choices with the current one marked the way the
/// rail marks the current destination, so neither depends on color alone.
///
/// `apply` is the caller's seam for one change, the same shape
/// [`crate::shell::rail::render`] takes: the panel knows which change a choice
/// means, and the view that owns the environment knows what to do with it.
pub fn panel(
    environment: UiEnvironment,
    apply: impl Fn(EnvironmentChange) -> Box<dyn Fn(&mut Window, &mut App)>,
) -> impl IntoElement {
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let preferences = environment.preferences();
    let mark = scale.apply(tokens::icon::CONTROL_SIZE);
    let choice =
        |id: SharedString, label: SharedString, active: bool, change: EnvironmentChange| {
            div()
                .flex()
                .items_center()
                .pl(px(scale.apply(tokens::spacing::RELATED)))
                .rounded(px(scale.apply(tokens::radius::CONTROL)))
                .when(active, |row| {
                    row.debug_selector({
                        let label = label.clone();
                        move || format!("setting-active-{label}")
                    })
                    .bg(rgb(palette.selection))
                })
                // The fill is never the only cue: a check marks the current
                // choice for a sighted operator, and `current` below marks it
                // for assistive technology. Every choice reserves the mark's
                // slot the way every rail row reserves its marker bar — these
                // sit on a wrapping flex line, so a choice that grew when it
                // was taken would shove its neighbours along it and drop one to
                // the next row.
                .child(div().flex_none().size(px(mark)).when(active, |slot| {
                    slot.child(
                        svg()
                            .size(px(mark))
                            .text_color(rgb(palette.text))
                            .data(icon::CHECK.as_bytes()),
                    )
                }))
                .child(
                    Button::new(id, label, environment)
                        .current(active)
                        .on_surface(if active {
                            palette.selection
                        } else {
                            palette.background
                        })
                        .on_activate(apply(change)),
                )
        };
    let group = |label: &'static str, choices: Vec<gpui::Div>| {
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(
                div()
                    .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .children(choices),
            )
    };
    let appearances = [
        (PreparationAppearance::System, "System"),
        (PreparationAppearance::Light, "Light"),
        (PreparationAppearance::Dark, "Dark"),
    ]
    .into_iter()
    .map(|(appearance, label)| {
        choice(
            format!("appearance-{label}").into(),
            label.into(),
            preferences.appearance == appearance,
            EnvironmentChange::SetAppearance(appearance),
        )
    })
    .collect();
    let scales = [
        InterfaceScale::Percent100,
        InterfaceScale::Percent125,
        InterfaceScale::Percent150,
        InterfaceScale::Percent175,
        InterfaceScale::Percent200,
    ]
    .into_iter()
    .map(|step| {
        let label = format!("{}%", step.percent());
        choice(
            format!("scale-{label}").into(),
            label.into(),
            preferences.scale == step,
            EnvironmentChange::SetScale(step),
        )
    })
    .collect();
    div()
        .debug_selector(|| "settings-panel".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::SECTION)))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                .child("Interface"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(group("Interface scale", scales))
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(SCALE_HELP),
                )
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                        .text_color(rgb(palette.text_muted))
                        .child(SCALE_KEYS),
                ),
        )
        .child(group("Preparation appearance", appearances))
}
