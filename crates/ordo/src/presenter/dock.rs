//! The Output Dock: Program and Preview monitors, output status, Freeze and
//! Blackout, and the primary action (T1.4 WP4, Decisions 61, 65, 272).
//!
//! The dock is drawn in the same order in the Wide and Compact side column and
//! in the Constrained band: Program, Preview, status, Freeze/Blackout,
//! primary. It never scrolls and its controls never shrink; the layout model
//! gives the monitors whatever width is left.
//!
//! Nothing here reads the Engine. The owning view turns Engine state into the
//! small models below, and a control reports the [`Command`] the operator
//! chose through one callback, which the view sends to `commands::dispatch`.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderImage, SharedString, Styled, Window, div,
    prelude::*, px, rgb, svg,
};
use presentation::{commands::Command, state::LiveState};
use rendering::Frame;
use std::sync::Arc;
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        self, Availability,
        button::{Button, Tone},
        record::Marker,
    },
    tokens,
};

use super::layout::{
    DOCK_FREEZE_BLACKOUT_HEIGHT, DOCK_PRIMARY_HEIGHT, DOCK_STATUS_HEIGHT, MONITOR_LABEL_GAP,
    MONITOR_LABEL_HEIGHT,
};
use crate::canonical_views::live_viewport_fading;

/// The badge text.
pub const PREPARING_TEXT: &str = "Preparing\u{2026}";

/// Called with the command a control stands for.
pub type OnCommand = Rc<dyn Fn(Command, &mut Window, &mut App)>;

/// The primary area's content for one live state (Decision 61, as amended by
/// Decision 272).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Primary {
    /// A button that sends `command`.
    Action {
        label: &'static str,
        command: Command,
        icon: &'static str,
    },
    /// A statement, not a control: there is nothing for the operator to press.
    Info(&'static str),
}

/// The one thing the operator should do next, by live state.
pub fn primary_for(live: LiveState) -> Primary {
    match live {
        LiveState::OutputDisabled => Primary::Action {
            label: "Enable Output",
            command: Command::EnableOutput,
            icon: icon::MONITOR_UP,
        },
        LiveState::EnabledBlack | LiveState::Frozen | LiveState::Blackout => Primary::Action {
            label: "Take Preview Live",
            command: Command::Take,
            icon: icon::PLAY,
        },
        LiveState::Live => Primary::Info("LIVE — navigation updates Program"),
        LiveState::Disconnected { display_back: true } => Primary::Action {
            label: "Resume Output",
            command: Command::Take,
            icon: icon::PLAY,
        },
        LiveState::Disconnected {
            display_back: false,
        } => Primary::Info("Waiting for the congregation display"),
    }
}

/// How serious a Program state word is, which picks its color and the shape
/// of the dot beside it (color is never the only signal).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateTone {
    Live,
    Held,
    Dark,
    Off,
    Danger,
}

/// The word above the Program monitor.
pub fn program_label(live: LiveState, unavailable_black: bool) -> (&'static str, StateTone) {
    match live {
        LiveState::OutputDisabled => ("Output disabled", StateTone::Off),
        LiveState::EnabledBlack => ("BLACK · not live", StateTone::Dark),
        LiveState::Live if unavailable_black => ("LIVE · black · Cue unavailable", StateTone::Dark),
        LiveState::Live => ("LIVE", StateTone::Live),
        LiveState::Frozen => ("FROZEN", StateTone::Held),
        LiveState::Blackout => ("BLACKOUT", StateTone::Dark),
        LiveState::Disconnected {
            display_back: false,
        } => ("Display lost", StateTone::Danger),
        LiveState::Disconnected { display_back: true } => {
            ("Display lost · display back", StateTone::Danger)
        }
    }
}

/// What a monitor draws: the frame (when one is ready), its decoded image and
/// the fade opacity from the Engine's view of that monitor.
#[derive(Clone, Default)]
pub struct Shown {
    pub frame: Option<Arc<Frame>>,
    pub image: Option<Arc<RenderImage>>,
    pub opacity: f32,
}

/// Everything the Program monitor needs.
pub struct ProgramModel {
    pub live: LiveState,
    pub unavailable_black: bool,
    pub shown: Shown,
    /// The wanted frame has been pending past the slow-frame threshold.
    /// Operator-only: it adds the "Preparing…" badge to this monitor.
    pub preparing: bool,
}

/// Everything the Preview monitor needs.
pub struct PreviewModel {
    pub item_title: SharedString,
    pub private_context: Option<SharedString>,
    /// Why the Preview Cue cannot be taken live, when it cannot.
    pub unavailable: Option<SharedString>,
    pub shown: Shown,
    /// As [`ProgramModel::preparing`], for the Preview monitor.
    pub preparing: bool,
}

/// Everything the control rows need.
pub struct ControlsModel {
    pub live: LiveState,
    pub status: SharedString,
    pub can_freeze: bool,
    pub can_blackout: bool,
    pub can_primary: bool,
}

struct Colors {
    live: u32,
    held: u32,
    danger: u32,
    text: u32,
    muted: u32,
    surface: u32,
    border: u32,
}

fn colors(environment: UiEnvironment) -> Colors {
    let palette = tokens::palette(environment.appearance());
    Colors {
        live: palette.live,
        held: palette.warning,
        danger: palette.danger,
        text: palette.text,
        muted: palette.text_muted,
        surface: palette.surface,
        border: palette.border,
    }
}

impl StateTone {
    /// The shape that tells this tone without its color (`DESIGN.md`): a dot
    /// for Live, a diamond while held (Freeze), a square otherwise.
    pub fn marker(self) -> Marker {
        match self {
            Self::Live => Marker::Dot,
            Self::Held => Marker::Diamond,
            Self::Dark | Self::Off | Self::Danger => Marker::Square,
        }
    }
}

/// A filled state marker, `size` px before the Interface scale, in `color`.
pub fn marker(marker: Marker, color: u32, size: f32, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    svg()
        .flex_none()
        .size(px(scale.apply(size)))
        .text_color(rgb(color))
        .data(marker.svg().as_bytes())
        .into_any_element()
}

fn state_word(text: &'static str, tone: StateTone, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    let color = match tone {
        StateTone::Live => c.live,
        StateTone::Held => c.held,
        StateTone::Danger => c.danger,
        StateTone::Dark => c.text,
        StateTone::Off => c.muted,
    };
    div()
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .min_w_0()
        .text_color(rgb(color))
        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
        .child(marker(tone.marker(), color, 10.0, environment))
        .child(div().min_w_0().truncate().child(text))
        .into_any_element()
}

/// The label row above a monitor: `name` on the left, `detail` on the right.
fn monitor_head(name: &'static str, detail: AnyElement, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    div()
        .flex_none()
        .h(px(scale.apply(MONITOR_LABEL_HEIGHT)))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(
            div()
                .flex_none()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                .child(name),
        )
        .child(div().min_w_0().flex_1().flex().justify_end().child(detail))
        .into_any_element()
}

fn viewport_box(
    selector: &'static str,
    shown: &Shown,
    width: f32,
    environment: UiEnvironment,
) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(width * 9.0 / 16.0))
        .debug_selector(move || selector.into())
        .child(live_viewport_fading(
            shown.frame.clone(),
            shown.image.clone(),
            width,
            None,
            shown.opacity,
            environment,
        ))
}

/// The operator-only "Preparing…" badge for a slow frame.
///
/// It is a child of the operator's monitor box, beside the live viewport and
/// never inside it, so it can never reach anything the congregation output
/// draws (that output reuses the viewport alone).
fn preparing_badge(selector: &'static str, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    div()
        .debug_selector(move || selector.into())
        .absolute()
        .top(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .right(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .px(px(scale.apply(tokens::spacing::RELATED)))
        .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .bg(gpui::black().opacity(0.72))
        .text_color(rgb(c.held))
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .child(PREPARING_TEXT)
        .into_any_element()
}

/// A centered statement drawn over a monitor's black.
fn monitor_overlay(
    selector: &'static str,
    text: SharedString,
    color: u32,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    div()
        .debug_selector(move || selector.into())
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(scale.apply(tokens::spacing::RELATED)))
        .text_align(gpui::TextAlign::Center)
        .text_color(rgb(color))
        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
        .bg(gpui::black().opacity(0.6))
        .child(text)
        .into_any_element()
}

/// The Program monitor: the live frame, or the state when there is no output.
pub fn program_monitor(model: &ProgramModel, width: f32, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    let (word, tone) = program_label(model.live, model.unavailable_black);
    let state_text: Option<(&'static str, u32)> = match model.live {
        LiveState::OutputDisabled => Some(("No congregation window · Output is disabled", c.text)),
        LiveState::Disconnected {
            display_back: false,
        } => Some((
            "Display lost · Ordo can't tell what the congregation sees",
            c.danger,
        )),
        LiveState::Disconnected { display_back: true } => Some((
            "Display back · black · nothing shows until you resume",
            c.held,
        )),
        _ => None,
    };
    let viewport =
        viewport_box("monitor-program", &model.shown, width, environment)
            .id("program-monitor")
            .children(state_text.map(|(text, color)| {
                monitor_overlay("program-state", text.into(), color, environment)
            }))
            .children(
                model
                    .preparing
                    .then(|| preparing_badge("monitor-program-preparing", environment)),
            );
    div()
        .id("program")
        .flex_none()
        .w(px(width))
        .flex()
        .flex_col()
        .gap(px(scale.apply(MONITOR_LABEL_GAP)))
        .child(monitor_head(
            "Program",
            state_word(word, tone, environment),
            environment,
        ))
        .child(viewport)
        .into_any_element()
}

/// The Preview monitor, with the item title, the operator-only "Output
/// unavailable" cover and the Cue's private context.
///
/// The private context is a strip over the foot of the monitor rather than a
/// row below it: the layout model reserves exactly two label rows and the
/// controls, and the strip adds no height. A Cue that has context but no
/// projection (trigger, Hold, Wait) draws black there, so nothing the
/// congregation would see is covered.
pub fn preview_monitor(model: &PreviewModel, width: f32, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    let mut viewport =
        viewport_box("monitor-preview", &model.shown, width, environment).id("preview-monitor");
    if let Some(reason) = &model.unavailable {
        viewport = viewport.child(monitor_overlay(
            "preview-unavailable",
            format!("Output unavailable · {reason}").into(),
            c.held,
            environment,
        ));
    }
    if model.preparing {
        viewport = viewport.child(preparing_badge("monitor-preview-preparing", environment));
    }
    if let Some(context) = &model.private_context {
        viewport = viewport.child(primitives::truncated(
            div()
                .id("preview-context")
                .debug_selector(|| "preview-context".into())
                .absolute()
                .left_0()
                .bottom_0()
                .w_full()
                .px(px(scale.apply(tokens::spacing::RELATED)))
                .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .bg(gpui::black().opacity(0.72))
                .text_color(rgb(tokens::color::PROJECTION_TEXT))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                // The minister's words wrap rather than ellipsize
                // (DESIGN.md, Truncation). Two lines fit even the 175 %
                // band; a longer phrase is whole on hover.
                .line_clamp(2)
                .child(context.clone()),
            context.clone(),
            environment,
        ));
    }
    div()
        .id("preview")
        .flex_none()
        .w(px(width))
        .flex()
        .flex_col()
        .gap(px(scale.apply(MONITOR_LABEL_GAP)))
        .child(monitor_head(
            "Preview",
            div()
                .min_w_0()
                .truncate()
                .text_color(rgb(c.muted))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .child(model.item_title.clone())
                .into_any_element(),
            environment,
        ))
        .child(viewport)
        .into_any_element()
}

/// The output's health, which the status line's marker shows whatever its
/// words are (a refusal, or the standing state).
pub fn output_tone(live: LiveState) -> StateTone {
    match live {
        LiveState::OutputDisabled => StateTone::Off,
        LiveState::EnabledBlack | LiveState::Live | LiveState::Frozen | LiveState::Blackout => {
            StateTone::Live
        }
        LiveState::Disconnected { display_back: true } => StateTone::Held,
        LiveState::Disconnected {
            display_back: false,
        } => StateTone::Danger,
    }
}

/// The status line: a marker for the output's health, then the last refusal
/// in words or the standing output state.
pub fn output_status(
    text: SharedString,
    live: LiveState,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    let tone = output_tone(live);
    let marker_color = match tone {
        StateTone::Live => c.live,
        StateTone::Held => c.held,
        StateTone::Danger => c.danger,
        StateTone::Dark | StateTone::Off => c.muted,
    };
    div()
        .id("output-status")
        .debug_selector(|| "output-status".into())
        .flex_none()
        .h(px(scale.apply(DOCK_STATUS_HEIGHT)))
        .w_full()
        .flex()
        .items_center()
        .min_w_0()
        .text_color(rgb(c.text))
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .child(marker(tone.marker(), marker_color, 8.0, environment))
        .child(div().min_w_0().truncate().child(text))
        .into_any_element()
}

/// One of the two output toggles, with its label and icon. `pressed` is the toggle's state; it is
/// shown by an edge in the state color as well as by the accessible state.
fn toggle(
    (label, glyph): (&'static str, &'static str),
    command: Command,
    pressed: bool,
    enabled: bool,
    edge: u32,
    on_command: &OnCommand,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let on_command = on_command.clone();
    // The Button primitive is DEFAULT_CONTROL tall. The slot is the Presenter
    // control height, and the edge always takes its space, so pressing a
    // toggle never moves anything.
    let edge_width = scale.apply(2.0);
    div()
        .flex_1()
        .min_w_0()
        .h(px(scale.apply(DOCK_FREEZE_BLACKOUT_HEIGHT)))
        .flex()
        .items_center()
        .rounded(px(scale.apply(tokens::radius::CONTROL) + edge_width))
        .border(px(edge_width))
        .border_color(if pressed {
            rgb(edge).into()
        } else {
            gpui::transparent_black()
        })
        .child(
            div().flex_1().min_w_0().flex().flex_col().child(
                Button::new(format!("toggle-{label}"), label, environment)
                    .icon(glyph)
                    .control_height(tokens::density::PRESENTER_CONTROL)
                    .current(pressed)
                    .availability(if enabled {
                        Availability::Available
                    } else {
                        Availability::Disabled
                    })
                    .on_activate(move |window, cx| on_command(command, window, cx)),
            ),
        )
        .into_any_element()
}

/// Freeze and Blackout side by side, in the Presenter control height.
pub fn freeze_blackout(
    model: &ControlsModel,
    on_command: &OnCommand,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    div()
        .id("freeze-blackout")
        .flex_none()
        .w_full()
        .flex()
        .flex_row()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(toggle(
            ("Freeze", icon::SNOWFLAKE),
            Command::ToggleFreeze,
            model.live == LiveState::Frozen,
            model.can_freeze,
            c.held,
            on_command,
            environment,
        ))
        .child(toggle(
            ("Blackout", icon::SQUARE),
            Command::ToggleBlackout,
            model.live == LiveState::Blackout,
            model.can_blackout,
            c.text,
            on_command,
            environment,
        ))
        .into_any_element()
}

/// The primary action, or the non-action statement that replaces it.
pub fn primary_action(
    model: &ControlsModel,
    on_command: &OnCommand,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let c = colors(environment);
    let slot = div()
        .debug_selector(|| "primary-action".into())
        .flex_none()
        .w_full()
        .h(px(scale.apply(DOCK_PRIMARY_HEIGHT)))
        .flex()
        .items_center();
    match primary_for(model.live) {
        Primary::Action {
            label,
            command,
            icon: glyph,
        } => {
            let on_command = on_command.clone();
            slot.child(
                div().flex_1().min_w_0().flex().flex_col().child(
                    Button::new("primary-action-button", label, environment)
                        .icon(glyph)
                        .control_height(tokens::density::PRESENTER_CONTROL)
                        .tone(Tone::Primary)
                        .availability(if model.can_primary {
                            Availability::Available
                        } else {
                            Availability::Disabled
                        })
                        .on_activate(move |window, cx| on_command(command, window, cx)),
                ),
            )
            .into_any_element()
        }
        Primary::Info(text) => slot
            .child(
                div()
                    .debug_selector(|| "primary-info".into())
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                    .h_full()
                    .rounded(px(scale.apply(tokens::radius::CONTROL)))
                    .border(px(tokens::border::SEPARATOR))
                    .border_color(rgb(c.border))
                    .bg(rgb(c.surface))
                    .text_color(rgb(if model.live == LiveState::Live {
                        c.live
                    } else {
                        c.muted
                    }))
                    .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                    .child(div().min_w_0().truncate().child(text)),
            )
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_primary_follows_the_live_state() {
        use LiveState::*;
        let action = |live| match primary_for(live) {
            Primary::Action { label, command, .. } => Some((label, command)),
            Primary::Info(_) => None,
        };
        assert_eq!(
            action(OutputDisabled),
            Some(("Enable Output", Command::EnableOutput))
        );
        for live in [EnabledBlack, Frozen, Blackout] {
            assert_eq!(action(live), Some(("Take Preview Live", Command::Take)));
        }
        assert_eq!(
            action(Disconnected { display_back: true }),
            Some(("Resume Output", Command::Take))
        );
        assert_eq!(action(Live), None);
        assert_eq!(
            primary_for(Live),
            Primary::Info("LIVE — navigation updates Program")
        );
        assert_eq!(
            primary_for(Disconnected {
                display_back: false
            }),
            Primary::Info("Waiting for the congregation display")
        );
    }

    #[test]
    fn every_live_state_has_a_program_word() {
        use LiveState::*;
        let states = [
            OutputDisabled,
            EnabledBlack,
            Live,
            Frozen,
            Blackout,
            Disconnected {
                display_back: false,
            },
            Disconnected { display_back: true },
        ];
        for live in states {
            let (word, _) = program_label(live, false);
            assert!(!word.is_empty(), "{live:?}");
        }
        assert_eq!(program_label(Live, false).0, "LIVE");
        assert_eq!(
            program_label(Live, true).0,
            "LIVE · black · Cue unavailable"
        );
        assert_eq!(program_label(Frozen, false).0, "FROZEN");
        assert_eq!(program_label(OutputDisabled, false).0, "Output disabled");
    }

    #[test]
    fn frozen_and_blackout_differ_by_shape_as_well_as_color() {
        use LiveState::*;
        let shape = |live| program_label(live, false).1.marker();
        assert_eq!(shape(Live), Marker::Dot);
        assert_eq!(shape(Frozen), Marker::Diamond);
        assert_eq!(shape(Blackout), Marker::Square);
        assert_ne!(shape(Frozen), shape(Blackout));
        assert_eq!(output_tone(OutputDisabled).marker(), Marker::Square);
        assert_eq!(output_tone(Frozen).marker(), Marker::Dot, "output still on");
    }
}
