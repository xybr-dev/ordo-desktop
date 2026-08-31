//! Development-only scenario host.
//!
//! `ui-lab` is only a native launcher; the scenarios, viewport presets, and
//! fixtures live here so GPUI tests can open the same scenario the operator
//! opens by hand, without `ui-lab` growing a workspace dependency of its own.

use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, IntoElement, KeyBinding, Render,
    Subscription, Window, actions, div, prelude::*, px, rgb,
};

use crate::{
    DecreaseInterfaceScale, IncreaseInterfaceScale, ResetInterfaceScale,
    environment::{
        EnvironmentChange, PreparationAppearance, ResolvedAppearance, UiEnvironment, UiPreferences,
    },
    resolved_appearance, tokens,
};

actions!(ordo_lab, [NextScenario, NextViewport, NextAppearance]);

/// Initializes the UI environment plus the lab-only keys. Production shells call
/// [`crate::init`] instead, so these bindings never reach an operator build.
pub fn init(cx: &mut App) -> Result<(), String> {
    crate::init(cx)?;
    cx.bind_keys(key_bindings());
    Ok(())
}

/// Lab-only keys. Every one cycles, because a lab is driven one-handed while
/// watching the stage; `crate::key_bindings` covers the shared scale keys.
pub(crate) fn key_bindings() -> [KeyBinding; 3] {
    [
        KeyBinding::new("f2", NextScenario, None),
        KeyBinding::new("f3", NextViewport, None),
        KeyBinding::new("f4", NextAppearance, None),
    ]
}

/// The two operator screen sizes T0.3 must hold up at.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewportPreset {
    #[default]
    Compact,
    Wide,
}

impl ViewportPreset {
    pub const ALL: [Self; 2] = [Self::Compact, Self::Wide];

    pub const fn size(self) -> (f32, f32) {
        match self {
            Self::Compact => (1366.0, 768.0),
            Self::Wide => (1920.0, 1080.0),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "1366x768",
            Self::Wide => "1920x1080",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Compact => Self::Wide,
            Self::Wide => Self::Compact,
        }
    }
}

/// Long multilingual content every scenario can reuse, in tests and by hand.
pub mod fixtures {
    pub const LONG_CEBUANO_TITLE: &str =
        "Pasidungog sa Mahal nga Birhen Maria sa Kaputli ug sa Kalinaw sa Katilingban";

    pub const LONG_MULTILINGUAL_BODY: &str = concat!(
        "Ang Ginoo maanaa kaninyo, ug maanaa usab sa imong espiritu. ",
        "Panginoon, maawa ka sa amin at sa buong sambayanan. ",
        "Lord, have mercy on us and on the whole assembly. ",
        "Kyrie eleison, Christe eleison, Kyrie eleison.",
    );
}

/// One curated lab scenario: a stable name plus the element it renders.
pub struct Scenario {
    pub name: &'static str,
    render: fn(UiEnvironment) -> AnyElement,
}

static SCENARIOS: &[Scenario] = &[Scenario {
    name: "placeholder",
    render: render_placeholder,
}];

pub fn scenarios() -> &'static [Scenario] {
    SCENARIOS
}

fn render_placeholder(environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "scenario-placeholder".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .p(px(scale.apply(tokens::spacing::SECTION)))
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                .child(fixtures::LONG_CEBUANO_TITLE),
        )
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::BODY.size)))
                .child(fixtures::LONG_MULTILINGUAL_BODY),
        )
        .into_any_element()
}

/// The one on-screen readout of what the scenario is being shown under. The
/// preference alone would hide which appearance System resolved to, including
/// after a live system change repaints the whole stage.
fn status_line(scenario: &str, viewport: ViewportPreset, environment: UiEnvironment) -> String {
    let preferences = environment.preferences();
    let appearance = match (preferences.appearance, environment.appearance()) {
        (PreparationAppearance::System, ResolvedAppearance::Light) => "System (Light)",
        (PreparationAppearance::System, ResolvedAppearance::Dark) => "System (Dark)",
        (PreparationAppearance::Light, _) => "Light",
        (PreparationAppearance::Dark, _) => "Dark",
    };
    format!(
        "{scenario} · {} · {appearance} · {}%",
        viewport.label(),
        preferences.scale.percent()
    )
}

/// Hosts one scenario inside a fixed viewport preset, with appearance and scale
/// driven by the same environment transition the production shell uses.
pub struct LabView {
    environment: UiEnvironment,
    viewport: ViewportPreset,
    scenario: usize,
    focus_handle: FocusHandle,
    _appearance_subscription: Subscription,
}

impl LabView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let environment = UiEnvironment::new(
            UiPreferences::default(),
            resolved_appearance(window.appearance()),
        );
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            this.change(
                EnvironmentChange::SystemAppearanceChanged(resolved_appearance(
                    window.appearance(),
                )),
                cx,
            );
        });
        Self {
            environment,
            viewport: ViewportPreset::default(),
            scenario: 0,
            focus_handle: cx.focus_handle(),
            _appearance_subscription: appearance_subscription,
        }
    }

    pub fn environment(&self) -> UiEnvironment {
        self.environment
    }

    pub fn viewport(&self) -> ViewportPreset {
        self.viewport
    }

    pub fn scenario(&self) -> &'static Scenario {
        &SCENARIOS[self.scenario]
    }

    /// Opens a scenario by name; reports whether the registry knows it.
    pub fn select_scenario(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        let Some(index) = SCENARIOS.iter().position(|scenario| scenario.name == name) else {
            return false;
        };
        self.scenario = index;
        cx.notify();
        true
    }

    pub fn set_viewport(&mut self, viewport: ViewportPreset, cx: &mut Context<Self>) {
        self.viewport = viewport;
        cx.notify();
    }

    pub fn change(&mut self, change: EnvironmentChange, cx: &mut Context<Self>) {
        if self.environment.apply(change) {
            cx.notify();
        }
    }

    fn next_scenario(&mut self, _: &NextScenario, _: &mut Window, cx: &mut Context<Self>) {
        self.scenario = (self.scenario + 1) % SCENARIOS.len();
        cx.notify();
    }

    fn next_viewport(&mut self, _: &NextViewport, _: &mut Window, cx: &mut Context<Self>) {
        self.set_viewport(self.viewport.next(), cx);
    }

    fn next_appearance(&mut self, _: &NextAppearance, _: &mut Window, cx: &mut Context<Self>) {
        let next = match self.environment.preferences().appearance {
            PreparationAppearance::System => PreparationAppearance::Light,
            PreparationAppearance::Light => PreparationAppearance::Dark,
            PreparationAppearance::Dark => PreparationAppearance::System,
        };
        self.change(EnvironmentChange::SetAppearance(next), cx);
    }
}

impl Focusable for LabView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LabView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment;
        let preferences = environment.preferences();
        let (background, surface, text, border) = match environment.appearance() {
            ResolvedAppearance::Light => (
                tokens::color::PREPARATION_BACKGROUND,
                tokens::color::PREPARATION_SURFACE,
                tokens::color::PREPARATION_TEXT,
                tokens::color::PREPARATION_BORDER,
            ),
            ResolvedAppearance::Dark => (
                tokens::color::PRESENTER_BACKGROUND,
                tokens::color::PRESENTER_SURFACE,
                tokens::color::PRESENTER_TEXT,
                tokens::color::PRESENTER_BORDER,
            ),
        };
        let scenario = self.scenario();
        let (width, height) = self.viewport.size();
        let status = status_line(scenario.name, self.viewport, environment);

        div()
            .id("ordo-lab")
            .key_context("OrdoLab")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::next_scenario))
            .on_action(cx.listener(Self::next_viewport))
            .on_action(cx.listener(Self::next_appearance))
            .on_action(cx.listener(|this, _: &IncreaseInterfaceScale, _, cx| {
                this.change(EnvironmentChange::IncreaseScale, cx);
            }))
            .on_action(cx.listener(|this, _: &DecreaseInterfaceScale, _, cx| {
                this.change(EnvironmentChange::DecreaseScale, cx);
            }))
            .on_action(cx.listener(|this, _: &ResetInterfaceScale, _, cx| {
                this.change(EnvironmentChange::ResetScale, cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .font_family(tokens::typography::OPERATOR_FAMILY)
            .bg(rgb(background))
            .text_color(rgb(text))
            .child(
                div()
                    .debug_selector(|| "lab-status".into())
                    .flex_none()
                    .p(px(preferences.scale.apply(tokens::spacing::CONTROL_INSET)))
                    .border_b(px(tokens::border::SEPARATOR))
                    .border_color(rgb(border))
                    .bg(rgb(surface))
                    .text_size(px(preferences.scale.apply(tokens::typography::LABEL.size)))
                    .child(status),
            )
            .child(
                // Dev-only: a preset larger than the lab window scrolls rather
                // than reflowing, so the scenario keeps its exact geometry.
                div()
                    .id("lab-stage-viewport")
                    .flex_1()
                    .overflow_scroll()
                    .child(
                        // The border is outside the stage so a scenario gets the
                        // whole preset, not the preset minus two border pixels.
                        div()
                            .flex_none()
                            .border_1()
                            .border_color(rgb(border))
                            .child(
                                div()
                                    .debug_selector(|| "lab-stage".into())
                                    .w(px(width))
                                    .h(px(height))
                                    .bg(rgb(surface))
                                    .child((scenario.render)(environment)),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, size};

    fn open(cx: &mut TestAppContext) -> (gpui::Entity<LabView>, &mut gpui::VisualTestContext) {
        cx.update(init).unwrap();
        let (lab, cx) = cx.add_window_view(|window, cx| {
            let lab = LabView::new(window, cx);
            window.focus(&lab.focus_handle, cx);
            lab
        });
        // Larger than the widest preset so nothing under test is clipped away.
        cx.simulate_resize(size(px(2400.0), px(1400.0)));
        (lab, cx)
    }

    #[gpui::test]
    fn the_placeholder_scenario_opens_at_both_viewport_presets(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);

        for preset in ViewportPreset::ALL {
            let opened = lab.update(cx, |lab, cx| {
                lab.set_viewport(preset, cx);
                lab.select_scenario("placeholder", cx)
            });
            cx.run_until_parked();

            assert!(opened, "the placeholder scenario should be registered");
            assert!(cx.debug_bounds("scenario-placeholder").is_some());
            let (width, height) = preset.size();
            assert_eq!(
                cx.debug_bounds("lab-stage").map(|bounds| bounds.size),
                Some(size(px(width), px(height))),
                "{} stage geometry",
                preset.label()
            );
        }

        let unknown = lab.update(cx, |lab, cx| lab.select_scenario("missing", cx));
        assert!(!unknown, "an unknown scenario name should not open");
    }

    #[test]
    fn the_status_line_reports_the_resolved_appearance_not_only_the_preference() {
        let mut environment =
            UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        assert_eq!(
            status_line("placeholder", ViewportPreset::Compact, environment),
            "placeholder · 1366x768 · System (Light) · 100%"
        );

        environment.apply(EnvironmentChange::SystemAppearanceChanged(
            ResolvedAppearance::Dark,
        ));
        assert_eq!(
            status_line("placeholder", ViewportPreset::Compact, environment),
            "placeholder · 1366x768 · System (Dark) · 100%"
        );

        environment.apply(EnvironmentChange::SetAppearance(
            PreparationAppearance::Light,
        ));
        assert_eq!(
            status_line("placeholder", ViewportPreset::Wide, environment),
            "placeholder · 1920x1080 · Light · 100%"
        );
    }

    #[gpui::test]
    fn lab_keys_cycle_the_viewport_and_drive_appearance_and_scale(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);

        cx.simulate_keystrokes("f3");
        lab.read_with(cx, |lab, _| {
            assert_eq!(lab.viewport(), ViewportPreset::Wide)
        });

        cx.simulate_keystrokes("f4 f4 ctrl-= ctrl-=");
        lab.read_with(cx, |lab, _| {
            let preferences = lab.environment().preferences();
            assert_eq!(preferences.appearance, PreparationAppearance::Dark);
            assert_eq!(
                preferences.scale,
                crate::environment::InterfaceScale::Percent150
            );
        });
    }
}
