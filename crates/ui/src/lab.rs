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
    DecreaseInterfaceScale, IncreaseInterfaceScale, ResetInterfaceScale, assets,
    environment::{
        EnvironmentChange, PreparationAppearance, ResolvedAppearance, UiEnvironment, UiPreferences,
    },
    primitives::{
        Availability,
        button::Button,
        disclosure::{Disclosure, DisclosureState},
        field::{Field, FieldState},
        selection::{Choice, Combobox, Menu, Select, SelectionState, Tabs},
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

    /// A visiting bishop: the value `MVP-PLAN.md` needs a Presider combobox to
    /// accept without the parish first creating a record for him.
    pub const LONG_PRESIDER_NAME: &str =
        "Most Rev. Jose Alan Villanueva Dela Cruz, Obispo sa Diyosesis sa Dumaguete";

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
    /// A scenario renders inside the host's frame, so it gets the same context
    /// a view would: window-keyed control state has to survive between frames.
    render: fn(UiEnvironment, &mut Window, &mut App) -> AnyElement,
}

static SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "placeholder",
        render: render_placeholder,
    },
    Scenario {
        name: "buttons",
        render: render_buttons,
    },
    Scenario {
        name: "fields",
        render: render_fields,
    },
    Scenario {
        name: "menu",
        render: render_menu,
    },
    Scenario {
        name: "select",
        render: render_select,
    },
    Scenario {
        name: "combobox",
        render: render_combobox,
    },
    Scenario {
        name: "combobox-new",
        render: render_combobox_new,
    },
    Scenario {
        name: "tabs",
        render: render_tabs,
    },
    Scenario {
        name: "disclosure",
        render: render_disclosure,
    },
];

fn choices() -> Vec<Choice> {
    vec![
        Choice::new("Not assigned"),
        Choice::new("English"),
        Choice::new("Cebuano").availability(Availability::Disabled),
        Choice::new("Filipino").availability(Availability::unavailable("The resource is missing")),
        Choice::new(fixtures::LONG_CEBUANO_TITLE),
        Choice::new(fixtures::LONG_MULTILINGUAL_BODY),
    ]
}

fn render_menu(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = SelectionState::keyed("lab-menu", choices(), None, window, cx);
    row("scenario-menu", environment)
        .child(
            Menu::new("menu", "More actions", &state, environment)
                .on_surface(tokens::palette(environment.appearance()).surface),
        )
        .into_any_element()
}
fn render_select(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = SelectionState::keyed("lab-select", choices(), Some(0), window, cx);
    row("scenario-select", environment)
        .child(
            Select::new("select", "Language", &state, environment)
                .on_surface(tokens::palette(environment.appearance()).surface),
        )
        .into_any_element()
}
/// The Combobox opens unassigned, so what the operator types is a filter from
/// the first keystroke rather than an edit of a value already in the Field.
///
/// Its collection drops the `Not assigned` choice the Select carries: an empty
/// Combobox already *is* that value, and offering it as a choice would put the
/// words in the Field as if they were a Language.
fn render_combobox(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let assignable = choices().split_off(1);
    let state = SelectionState::combobox_keyed("lab-combobox", assignable, None, window, cx);
    row("scenario-combobox", environment)
        .child(
            Combobox::new("combobox", "Language", &state, environment)
                .on_surface(tokens::palette(environment.appearance()).surface),
        )
        .into_any_element()
}
/// The other half of the Combobox: a collection that does not hold every valid
/// answer. `MVP-PLAN.md` needs a visiting Presider assignable to one Mass
/// without becoming a parish record, so an unmatched name is offered rather
/// than refused.
fn render_combobox_new(
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = SelectionState::combobox_keyed(
        "lab-combobox-new",
        vec![
            Choice::new("Fr. Antonio Reyes"),
            Choice::new("Fr. Jose Maria Villanueva"),
            Choice::new(fixtures::LONG_PRESIDER_NAME),
        ],
        None,
        window,
        cx,
    )
    .accepting_new_values(cx);
    row("scenario-combobox-new", environment)
        .child(
            Combobox::new("presider", "Presider", &state, environment)
                .on_surface(tokens::palette(environment.appearance()).surface),
        )
        .into_any_element()
}
fn render_tabs(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = SelectionState::keyed(
        "lab-tabs",
        vec![
            Choice::new("Upcoming"),
            Choice::new("History"),
            Choice::new("Archive"),
            Choice::new("Templates"),
        ],
        Some(0),
        window,
        cx,
    );
    let label = ["Upcoming", "History", "Archive", "Templates"][state.selected(cx).unwrap_or(0)];
    row("scenario-tabs", environment)
        .flex_col()
        .child(
            Tabs::new("tabs", "Mass views", &state, environment)
                .on_surface(tokens::palette(environment.appearance()).surface),
        )
        .child(
            div()
                .id("local-view")
                .role(gpui::Role::TabPanel)
                .aria_label(label)
                .child(label),
        )
        .into_any_element()
}
fn render_disclosure(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let scale = environment.preferences().scale;
    let stage = tokens::palette(environment.appearance()).surface;
    let details = DisclosureState::keyed("lab-disclosure", window, cx);
    let overflow = DisclosureState::keyed("lab-disclosure-overflow", window, cx);
    let detail = |label: &'static str, value: &'static str| {
        div()
            .flex()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(
                div()
                    .text_color(gpui::rgb(
                        tokens::palette(environment.appearance()).text_muted,
                    ))
                    .child(label),
            )
            .child(value)
    };
    row("scenario-disclosure", environment)
        .flex_col()
        // What the primitive is for: structured secondary detail kept on
        // screen and reachable without a card or an overlay.
        .child(
            Disclosure::new(
                "disclosure",
                "Details",
                &details,
                environment,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .child(detail("Presider", "Fr. Antonio Reyes"))
                    .child(detail("Choir", "San Nicolas Parish Choir"))
                    .child(detail("Language", "Cebuano"))
                    .child(Button::new("inspect", "Inspect", environment).on_surface(stage)),
            )
            .on_surface(stage),
        )
        // The same primitive holding prose that has to wrap inside its region
        // rather than run off the window.
        .child(
            Disclosure::new(
                "disclosure-overflow",
                "Notes",
                &overflow,
                environment,
                div().child(fixtures::LONG_MULTILINGUAL_BODY),
            )
            .on_surface(stage),
        )
        .into_any_element()
}

pub fn scenarios() -> &'static [Scenario] {
    SCENARIOS
}

fn render_placeholder(environment: UiEnvironment, _: &mut Window, _: &mut App) -> AnyElement {
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

/// Every meaningful Button state on one stage, so hover, press, and keyboard
/// focus can be walked by hand in one pass at either appearance.
fn render_buttons(environment: UiEnvironment, _: &mut Window, _: &mut App) -> AnyElement {
    let stage = tokens::palette(environment.appearance()).surface;
    row("scenario-buttons", environment)
        .child(
            Button::new("save", "Save", environment)
                .icon(assets::icon::CHECK)
                .on_surface(stage),
        )
        .child(
            Button::new("more", "More actions", environment)
                .icon(assets::icon::ELLIPSIS)
                .icon_only()
                .on_surface(stage),
        )
        .child(
            Button::new("archive", "Archive", environment)
                .availability(Availability::Disabled)
                .on_surface(stage),
        )
        .child(
            Button::new("go-live", "Go Live", environment)
                .availability(Availability::unavailable("No output is connected"))
                .on_surface(stage),
        )
        // ponytail: this only widens the control — Button applies none of
        // DESIGN.md's truncation rules yet, and the row's wrapping hides that
        // at the row rather than at the control. Truncation is per-context, so
        // it lands with the typography work; this scenario is what it fixes.
        .child(Button::new("long", fixtures::LONG_CEBUANO_TITLE, environment).on_surface(stage))
        .into_any_element()
}

/// Every meaningful Field state, including the empty value that is a choice
/// rather than a fault.
fn render_fields(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let mut field =
        |id: &'static str, value: &'static str| FieldState::keyed(id, value, window, cx);
    // Every state but Not assigned carries a value, so the states that refuse
    // editing can be checked against the one that accepts it.
    let (title, celebration, recorded, song) = (
        field("lab-title", "Entrance Hymn"),
        field("lab-celebration", ""),
        field("lab-recorded", "Fr. Antonio Reyes"),
        field("lab-song", "gloria-mass-of-creation.mp3"),
    );
    let stage = tokens::palette(environment.appearance()).surface;
    row("scenario-fields", environment)
        .child(
            Field::new("title", "Title", &title, environment)
                .on_clear({
                    let title = title.clone();
                    move |window, cx| title.set_value("", window, cx)
                })
                .on_surface(stage),
        )
        .child(
            Field::new("celebration", "Celebration", &celebration, environment)
                .not_assigned()
                .on_surface(stage),
        )
        .child(
            Field::new("recorded", "Recorded by", &recorded, environment)
                .read_only("The Mass has already been celebrated")
                .on_surface(stage),
        )
        .child(
            Field::new("song", "Song file", &song, environment)
                .availability(Availability::unavailable("The Song file is missing"))
                .on_surface(stage),
        )
        .into_any_element()
}

/// The shared stage frame: controls sit on the surface their focus separation
/// ring assumes, wrap at a narrow viewport, and never crowd each other.
fn row(selector: &'static str, environment: UiEnvironment) -> gpui::Div {
    let scale = environment.preferences().scale;
    div()
        .debug_selector(move || selector.into())
        .flex()
        .flex_wrap()
        .items_start()
        .gap(px(scale.apply(tokens::spacing::SECTION)))
        .p(px(scale.apply(tokens::spacing::SECTION)))
        .text_size(px(scale.apply(tokens::typography::BODY.size)))
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment;
        let preferences = environment.preferences();
        let palette = tokens::palette(environment.appearance());
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
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .debug_selector(|| "lab-status".into())
                    .flex_none()
                    .p(px(preferences.scale.apply(tokens::spacing::CONTROL_INSET)))
                    .border_b(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.surface))
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
                            .border_color(rgb(palette.border))
                            .child(
                                div()
                                    .debug_selector(|| "lab-stage".into())
                                    .w(px(width))
                                    .h(px(height))
                                    .bg(rgb(palette.surface))
                                    .child((scenario.render)(environment, window, cx)),
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

    /// Both control scenarios at both appearances: the curated corpus the lab
    /// exists to make reviewable by hand is also opened once by the harness, so
    /// a scenario cannot rot into one that no longer renders.
    #[gpui::test]
    fn the_control_scenarios_open_at_both_appearances(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);

        for appearance in [PreparationAppearance::Light, PreparationAppearance::Dark] {
            for (scenario, selector) in [
                ("buttons", "scenario-buttons"),
                ("fields", "scenario-fields"),
            ] {
                let opened = lab.update(cx, |lab, cx| {
                    lab.change(EnvironmentChange::SetAppearance(appearance), cx);
                    lab.select_scenario(scenario, cx)
                });
                cx.run_until_parked();

                assert!(opened, "{scenario} should be registered");
                assert!(
                    cx.debug_bounds(selector).is_some(),
                    "{scenario} renders under {appearance:?}"
                );
                assert!(
                    cx.debug_bounds("control-unavailable").is_some(),
                    "{scenario} shows its Unavailable Cue under {appearance:?}"
                );
            }
            assert!(
                cx.debug_bounds("field-read-only").is_some(),
                "the Read-only reason stays visible under {appearance:?}"
            );
        }
    }

    #[gpui::test]
    fn composite_scenarios_cover_the_appearance_scale_viewport_matrix(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        for appearance in [PreparationAppearance::Light, PreparationAppearance::Dark] {
            for scale in [
                crate::environment::InterfaceScale::Percent100,
                crate::environment::InterfaceScale::Percent125,
                crate::environment::InterfaceScale::Percent150,
                crate::environment::InterfaceScale::Percent175,
                crate::environment::InterfaceScale::Percent200,
            ] {
                for viewport in ViewportPreset::ALL {
                    let (width, height) = viewport.size();
                    cx.simulate_resize(size(px(width), px(height)));
                    for scenario in [
                        "menu",
                        "select",
                        "combobox",
                        "combobox-new",
                        "tabs",
                        "disclosure",
                    ] {
                        lab.update(cx, |lab, cx| {
                            lab.change(
                                EnvironmentChange::SetPreferences(UiPreferences {
                                    appearance,
                                    scale,
                                }),
                                cx,
                            );
                            lab.set_viewport(viewport, cx);
                            assert!(lab.select_scenario(scenario, cx));
                        });
                        let selector = match scenario {
                            "menu" => "scenario-menu",
                            "select" => "scenario-select",
                            "combobox" => "scenario-combobox",
                            "combobox-new" => "scenario-combobox-new",
                            "tabs" => "scenario-tabs",
                            _ => "scenario-disclosure",
                        };
                        assert!(cx.debug_bounds(selector).is_some());
                        if scenario == "combobox-new" {
                            let field = cx.debug_bounds("field-control").unwrap();
                            cx.simulate_click(field.center(), gpui::Modifiers::default());
                            // One long multilingual match, plus the offer to
                            // keep what was typed.
                            cx.simulate_input("Most");
                            let popup = cx.debug_bounds("selection-list").unwrap();
                            let long = cx.debug_bounds("choice-2").unwrap();
                            let create = cx.debug_bounds("choice-create").unwrap();
                            assert!(
                                long.origin.y >= popup.origin.y
                                    && create.bottom() <= popup.bottom()
                                    && popup.right() <= px(width)
                                    && popup.bottom() <= px(height),
                                "both rows stay inside a contained popup \
                                 ({appearance:?} {scale:?})"
                            );
                            // Nothing matches, and the collection still offers
                            // the name rather than refusing it.
                            cx.simulate_input(" Rev. Juan");
                            assert!(cx.debug_bounds("choice-2").is_none());
                            assert!(cx.debug_bounds("choice-none").is_none());
                            let create = cx.debug_bounds("choice-create").unwrap();
                            let popup = cx.debug_bounds("selection-list").unwrap();
                            assert!(
                                create.right() <= popup.right() && popup.right() <= px(width),
                                "the offered name is truncated inside the popup \
                                 ({appearance:?} {scale:?})"
                            );
                            cx.simulate_keystrokes("escape");
                            assert!(cx.debug_bounds("selection-list").is_none());
                        }
                        if scenario == "combobox" {
                            let field = cx.debug_bounds("field-control").unwrap();
                            cx.simulate_click(field.center(), gpui::Modifiers::default());
                            let popup = cx.debug_bounds("selection-list").unwrap();
                            assert!(
                                popup.origin.x >= px(0.0)
                                    && popup.origin.y >= field.bottom()
                                    && popup.right() <= px(width)
                                    && popup.bottom() <= px(height),
                                "the popup opens below the Field and inside the window \
                                 ({appearance:?} {scale:?})"
                            );
                            // One match, and it is the long multilingual value,
                            // so the surviving row is also the widest one.
                            // A Combobox holding a value offers to give it up,
                            // and an empty one has nothing to offer.
                            assert!(cx.debug_bounds("button-Clear").is_none());
                            cx.simulate_input("kyrie");
                            assert!(cx.debug_bounds("button-Clear").is_some());
                            let only = cx.debug_bounds("choice-4").unwrap();
                            assert!(cx.debug_bounds("choice-0").is_none());
                            let popup = cx.debug_bounds("selection-list").unwrap();
                            assert!(
                                only.origin.y >= popup.origin.y
                                    && only.bottom() <= popup.bottom()
                                    && popup.right() <= px(width),
                                "the one match is visible inside a contained popup \
                                 ({appearance:?} {scale:?})"
                            );
                            // No match: an empty popup would read as a fault.
                            cx.simulate_input("x");
                            assert!(cx.debug_bounds("choice-4").is_none());
                            assert!(cx.debug_bounds("choice-none").is_some());
                            cx.simulate_keystrokes("escape");
                            assert!(cx.debug_bounds("selection-list").is_none());
                        }
                        if matches!(scenario, "menu" | "select") {
                            let label = if scenario == "menu" {
                                "button-More actions"
                            } else {
                                "button-Language: Not assigned"
                            };
                            let trigger = cx.debug_bounds(label).unwrap();
                            cx.simulate_click(trigger.center(), gpui::Modifiers::default());
                            let popup = cx.debug_bounds("selection-list").unwrap();
                            assert!(popup.origin.x >= px(0.0) && popup.origin.y >= px(0.0));
                            assert!(
                                popup.right() <= px(width) && popup.bottom() <= px(height),
                                "{scenario} {appearance:?} {scale:?}"
                            );
                            // Containment alone passes for a popup rendered
                            // anywhere that fits, which is how a whole
                            // control-height of dead space survived this matrix.
                            assert!(
                                popup.origin.y >= trigger.bottom()
                                    && popup.origin.y - trigger.bottom()
                                        <= px(scale.apply(tokens::spacing::RELATED_TIGHT)),
                                "the popup opens directly under its trigger \
                                 ({scenario} {appearance:?} {scale:?})"
                            );
                            assert!(
                                popup.origin.x == trigger.origin.x
                                    || popup.right() >= px(width) - px(16.0),
                                "the popup aligns to the trigger's leading edge \
                                 unless the window snapped it \
                                 ({scenario} {appearance:?} {scale:?})"
                            );
                            cx.simulate_keystrokes("end");
                            let last = cx.debug_bounds("choice-5").unwrap();
                            assert!(
                                last.origin.y >= popup.origin.y && last.bottom() <= popup.bottom(),
                                "arrow navigation scrolls the long last value into view"
                            );
                            cx.simulate_keystrokes("escape");
                            assert!(cx.debug_bounds("selection-list").is_none());
                        }
                    }
                }
            }
        }
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
