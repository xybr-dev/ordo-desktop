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
    DecreaseInterfaceScale, FocusNext, FocusPrevious, IncreaseInterfaceScale, ResetInterfaceScale,
    assets,
    environment::{
        EnvironmentChange, PreparationAppearance, ResolvedAppearance, UiEnvironment, UiPreferences,
    },
    overlay::{Overlay, OverlayHost, Toast},
    primitives::{
        Availability,
        button::Button,
        disclosure::{Disclosure, DisclosureState},
        field::{Field, FieldState},
        list::{Badge, ListRow},
        selection::{Choice, Combobox, Menu, Select, SelectionState, Tabs},
    },
    resolved_appearance, tokens,
};

mod regions;

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
        name: "regions",
        render: regions::render,
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
        name: "dialog",
        render: render_dialog,
    },
    Scenario {
        name: "slideover",
        render: render_slideover,
    },
    Scenario {
        name: "toasts",
        render: render_toasts,
    },
    Scenario {
        name: "tabs",
        render: render_tabs,
    },
    Scenario {
        name: "disclosure",
        render: render_disclosure,
    },
    Scenario {
        name: "list",
        render: render_list,
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
/// The primary-list shape: a ruled column of rows, each carrying the detail
/// and the classifiers that tell one entity from another. The liturgical
/// colors appear here and nowhere else in the lab, because a season is the
/// domain metadata `DESIGN.md` reserves them for.
fn render_list(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let selected = window.use_keyed_state("lab-list", cx, |_, _| None::<usize>);
    let current = *selected.read(cx);
    let stage = tokens::palette(environment.appearance()).surface;
    type Season = Option<(&'static str, u32)>;
    let entries: [(&str, &str, Season); 4] = [
        (
            "Second Sunday of Lent",
            "6:00 AM \u{b7} Cebuano \u{b7} San Nicolas Parish Choir",
            Some(("Lent", tokens::color::LITURGICAL_PURPLE)),
        ),
        (
            "Easter Sunday of the Resurrection",
            "9:30 AM \u{b7} English",
            Some(("Easter", tokens::color::LITURGICAL_WHITE)),
        ),
        (
            fixtures::LONG_CEBUANO_TITLE,
            fixtures::LONG_MULTILINGUAL_BODY,
            Some(("Ordinary Time", tokens::color::LITURGICAL_GREEN)),
        ),
        ("Draft template", "No scheduled start", None),
    ];
    row("scenario-list", environment)
        .child(
            div()
                .debug_selector(|| "list-column".into())
                .w(px(environment
                    .preferences()
                    .scale
                    .apply(tokens::density::WORKSPACE_GUTTER_COMPACT * 20.0)))
                .flex()
                .flex_col()
                .children(entries.into_iter().enumerate().map(
                    |(index, (title, detail, season))| {
                        let selected = selected.clone();
                        ListRow::new(("row", index), title, environment)
                            .detail(detail)
                            .badges(
                                season
                                    .map(|(name, tone)| Badge::new(name, environment).tone(tone))
                                    .into_iter()
                                    .chain(
                                        (index == 3).then(|| Badge::new("Incomplete", environment)),
                                    ),
                            )
                            .selected(current == Some(index))
                            .on_surface(stage)
                            .on_activate(move |_, cx| {
                                selected.update(cx, |state, cx| {
                                    *state = Some(index);
                                    cx.notify();
                                });
                            })
                    },
                )),
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

/// The one host every scenario and the lab frame share. Keyed state, so the
/// content that opens a surface and the view that paints the layer reach the
/// same host without either being handed the other.
pub(crate) fn overlays(window: &mut Window, cx: &mut App) -> OverlayHost {
    OverlayHost::keyed("ordo-overlays", window, cx)
}

/// Enough body to exceed a bounded overlay at every scale, so the scroll
/// indicator and the fixed header/footer have something to prove.
fn long_body(environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "lab-long-body".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .children((0..8).map(|_| div().child(fixtures::LONG_MULTILINGUAL_BODY)))
        .into_any_element()
}

fn render_dialog(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let host = overlays(window, cx);
    let stage = tokens::palette(environment.appearance()).surface;
    row("scenario-dialog", environment)
        .child(
            Button::new("open-dialog", "Delete Mass", environment)
                .on_surface(stage)
                .on_activate({
                    let host = host.clone();
                    move |window, cx| {
                        let host = host.clone();
                        host.clone().open(
                            Overlay::dialog("Delete this Mass?")
                                .description(fixtures::LONG_CEBUANO_TITLE)
                                .body(|environment, _, _| long_body(environment))
                                .secondary("Keep", |_, _| {})
                                .primary("Delete", move |_, cx| {
                                    host.notify(Toast::confirmation("Mass deleted."), cx)
                                }),
                            window,
                            cx,
                        )
                    }
                }),
        )
        .into_any_element()
}

fn render_slideover(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let host = overlays(window, cx);
    let stage = tokens::palette(environment.appearance()).surface;
    // The nested request, built where the parent's own body can reach it.
    fn child() -> Overlay {
        Overlay::slideover("Song details").body(move |environment, window, cx| {
            let stage = tokens::palette(environment.appearance()).surface;
            let state =
                SelectionState::keyed("lab-slideover-select", choices(), Some(1), window, cx);
            div()
                .flex()
                .flex_col()
                .gap(px(environment
                    .preferences()
                    .scale
                    .apply(tokens::spacing::GROUP)))
                // A popup opened inside an overlay body has to paint over the
                // surface that owns it, which is what the layer's priorities
                // are for.
                .child(
                    Select::new("nested-select", "Language", &state, environment).on_surface(stage),
                )
                .child(long_body(environment))
                .into_any_element()
        })
    }
    row("scenario-slideover", environment)
        .child(
            Button::new("open-slideover", "Inspector", environment)
                .on_surface(stage)
                .on_activate({
                    let host = host.clone();
                    move |window, cx| {
                        let host = host.clone();
                        host.clone().open(
                            Overlay::slideover("Inspector")
                                .description("Mass details and readiness")
                                .body(move |environment, _, _| {
                                    let host = host.clone();
                                    let stage = tokens::palette(environment.appearance()).surface;
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(environment
                                            .preferences()
                                            .scale
                                            .apply(tokens::spacing::GROUP)))
                                        .child(
                                            Button::new("open-nested", "Open Song", environment)
                                                .on_surface(stage)
                                                .on_activate(move |window, cx| {
                                                    host.clone().open(child(), window, cx)
                                                }),
                                        )
                                        .child(long_body(environment))
                                        .into_any_element()
                                }),
                            window,
                            cx,
                        )
                    }
                }),
        )
        .into_any_element()
}

fn render_toasts(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let host = overlays(window, cx);
    let stage = tokens::palette(environment.appearance()).surface;
    row("scenario-toasts", environment)
        .child(
            Button::new("post-confirmation", "Remove Cue", environment)
                .on_surface(stage)
                .on_activate({
                    let host = host.clone();
                    move |_, cx| {
                        host.notify(
                            Toast::confirmation("Cue removed.").undo("Undo", |_, _| {}),
                            cx,
                        )
                    }
                }),
        )
        .child(
            Button::new("post-failure", "Lose output", environment)
                .on_surface(stage)
                .on_activate({
                    let host = host.clone();
                    move |_, cx| {
                        host.notify(
                            Toast::failure("Congregation display disconnected. Program is frozen."),
                            cx,
                        )
                    }
                }),
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

    fn next_scenario(&mut self, _: &NextScenario, window: &mut Window, cx: &mut Context<Self>) {
        self.scenario = (self.scenario + 1) % SCENARIOS.len();
        // The switch unmounts whatever held focus, leaving a stale stop that
        // Tab cannot step from and later keys cannot dispatch from. Home
        // focus so the next key starts live.
        window.focus(&self.focus_handle, cx);
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
            // The plain window tab order, so a scenario is keyboard-reachable
            // after F2. The pin stops action dispatch at the innermost
            // handler, so the regions scenario's registry and an open
            // overlay's trap both keep precedence over this root.
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
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
            // The one overlay layer, last and once. Every temporary surface in
            // the lab comes through it, exactly as in the shell.
            .child(overlays(window, cx).layer(environment, window, cx))
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
                        "list",
                        "regions",
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
                            "regions" => "scenario-regions",
                            "list" => "scenario-list",
                            _ => "scenario-disclosure",
                        };
                        assert!(cx.debug_bounds(selector).is_some());
                        if scenario == "list" {
                            // The row holding a long multilingual title and
                            // body is the one that would paint past its column
                            // if a row stopped truncating, and its season
                            // badge is the last thing on its detail line.
                            let column = cx.debug_bounds("list-column").unwrap();
                            let badge = cx.debug_bounds("badge-Ordinary Time").unwrap();
                            assert!(
                                badge.right() <= column.right() && column.right() <= px(width),
                                "a row's inline content stays inside its column \
                                 ({appearance:?} {scale:?})"
                            );
                        }
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
                            // Taken the way an operator reaches for it.
                            cx.simulate_click(create.center(), gpui::Modifiers::default());
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
                            assert!(
                                cx.debug_bounds("scroll-indicator-selection-list").is_none(),
                                "one match fits, so the indicator goes away \
                                 ({appearance:?} {scale:?})"
                            );
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
                            // Six choices, one of them a paragraph, exceed the
                            // popup's bounded height at every scale.
                            let corner = px(scale.apply(tokens::radius::OVERLAY));
                            let top = cx
                                .debug_bounds("scroll-indicator-selection-list")
                                .expect("a popup taller than its viewport shows the indicator");
                            // At rest the indicator has travelled nothing, so
                            // it starts exactly where its track does. Asserting
                            // the equality rather than a bound is the only
                            // check that the track really is the surface inset
                            // by its own corner, which the length and the drag
                            // gain are both computed from.
                            assert!(
                                (top.origin.y - (popup.origin.y + corner)).abs() <= px(1.0)
                                    && top.bottom() <= popup.bottom()
                                    && top.right() <= popup.right()
                                    && top.size.height < popup.size.height,
                                "the indicator is a thin overlay inside the popup, starting \
                                 where the corner its own edge curves away at leaves off \
                                 ({scenario} {appearance:?} {scale:?})"
                            );
                            cx.simulate_keystrokes("end");
                            let last = cx.debug_bounds("choice-5").unwrap();
                            assert!(
                                last.origin.y >= popup.origin.y && last.bottom() <= popup.bottom(),
                                "arrow navigation scrolls the long last value into view"
                            );
                            let bottom =
                                cx.debug_bounds("scroll-indicator-selection-list").unwrap();
                            // A bound, not an equality: `End` scrolls the last
                            // choice into view, which stops at that choice
                            // rather than at the surface's own maximum offset,
                            // so the list's bottom padding is travel the
                            // indicator still has left. Pinning the far end of
                            // the track needs an offset driven the whole way,
                            // which the drag test below does.
                            assert!(
                                bottom.origin.y > top.origin.y
                                    && bottom.bottom() <= popup.bottom() - corner,
                                "the indicator reports the position scrolling reached, and \
                                 stops clear of the far corner too \
                                 ({scenario} {appearance:?} {scale:?})"
                            );
                            cx.simulate_keystrokes("escape");
                            assert!(cx.debug_bounds("selection-list").is_none());
                        }
                    }
                }
            }
        }
    }

    /// Lets a surface finish entering or leaving. The transition asks for a
    /// platform animation frame, and the test platform has none.
    fn settle(cx: &mut gpui::VisualTestContext, duration: u16) {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(duration.into()));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }

    /// The constrained nested-overlay matrix: every temporary surface stays
    /// inside the window with its exterior gutters, keeps its header and its
    /// actions reachable while only the body scrolls, and a nested slideover
    /// leaves its parent where it was.
    #[gpui::test]
    fn overlay_scenarios_cover_the_appearance_scale_viewport_matrix(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        let enter = tokens::motion::OVERLAY_OPEN_DURATION_MS;
        let exit = tokens::motion::OVERLAY_CLOSE_DURATION_MS;
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
                    let contained = |bounds: gpui::Bounds<gpui::Pixels>, what: &str| {
                        assert!(
                            bounds.origin.x > px(0.0)
                                && bounds.origin.y > px(0.0)
                                && bounds.right() < px(width)
                                && bounds.bottom() < px(height),
                            "{what} stays inside the viewport with its exterior gutters \
                             ({appearance:?} {scale:?} {viewport:?})"
                        );
                    };

                    for scenario in ["dialog", "slideover"] {
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
                        cx.run_until_parked();

                        let (invoker, title, bar) = if scenario == "dialog" {
                            (
                                "button-Delete Mass",
                                "overlay-Delete this Mass?",
                                "scroll-indicator-overlay-body-Delete this Mass?",
                            )
                        } else {
                            (
                                "button-Inspector",
                                "overlay-Inspector",
                                "scroll-indicator-overlay-body-Inspector",
                            )
                        };
                        let trigger = cx.debug_bounds(invoker).unwrap();
                        cx.simulate_click(trigger.center(), gpui::Modifiers::default());
                        settle(cx, enter);

                        let surface = cx.debug_bounds(title).expect("the surface is open");
                        contained(surface, title);
                        // The header's Close control and the body's indicator
                        // are both inside it: the body is what scrolls, and the
                        // header stays put while it does.
                        let close = cx.debug_bounds("button-Close").unwrap();
                        assert!(
                            close.origin.y >= surface.origin.y
                                && close.bottom() <= surface.bottom(),
                            "the Close control stays in the header ({title} {scale:?})"
                        );
                        let indicator = cx
                            .debug_bounds(bar)
                            .expect("a body taller than its surface shows the indicator");
                        assert!(
                            indicator.origin.y > close.bottom()
                                && indicator.bottom() <= surface.bottom()
                                && indicator.size.width < surface.size.width,
                            "the indicator is a thin overlay on the body alone \
                             ({title} {scale:?} {viewport:?})"
                        );

                        if scenario == "dialog" {
                            // The footer keeps both actions reachable while the
                            // body scrolls between them and the header.
                            let primary = cx.debug_bounds("button-Delete").unwrap();
                            let secondary = cx.debug_bounds("button-Keep").unwrap();
                            assert!(
                                primary.bottom() <= surface.bottom()
                                    && secondary.bottom() <= surface.bottom()
                                    && primary.origin.y >= indicator.origin.y
                                    && secondary.origin.x < primary.origin.x,
                                "the primary action ends the footer ({scale:?} {viewport:?})"
                            );
                        } else {
                            // Nested: the parent is kept, not replaced.
                            let nest = cx.debug_bounds("button-Open Song").unwrap();
                            cx.simulate_click(nest.center(), gpui::Modifiers::default());
                            settle(cx, enter);
                            let child = cx
                                .debug_bounds("overlay-Song details")
                                .expect("the nested slideover is open");
                            contained(child, "the nested slideover");
                            assert!(
                                cx.debug_bounds("overlay-Inspector").is_some(),
                                "its parent stays where it was ({scale:?} {viewport:?})"
                            );
                            cx.simulate_keystrokes("escape");
                            settle(cx, exit);
                            assert!(cx.debug_bounds("overlay-Song details").is_none());
                            assert!(cx.debug_bounds("overlay-Inspector").is_some());
                        }

                        cx.simulate_keystrokes("escape");
                        settle(cx, exit);
                        assert!(
                            cx.debug_bounds(title).is_none(),
                            "{title} closes ({appearance:?} {scale:?} {viewport:?})"
                        );
                    }
                }
            }
        }
    }

    /// A bounded scrolling surface contains its own wheel. GPUI's scroll
    /// listener never stops propagating, so without containment one wheel over
    /// the popup moved the popup, the slideover body it sits in, and the lab
    /// stage behind that, all by the same delta.
    #[gpui::test]
    fn a_popup_inside_a_slideover_scrolls_only_itself(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        // A window smaller than the preset, so the stage behind everything
        // scrolls too and has something to be kept still.
        cx.simulate_resize(size(px(1366.0), px(768.0)));
        lab.update(cx, |lab, cx| {
            lab.set_viewport(ViewportPreset::Wide, cx);
            assert!(lab.select_scenario("slideover", cx));
        });
        cx.run_until_parked();

        let trigger = cx.debug_bounds("button-Inspector").unwrap();
        cx.simulate_click(trigger.center(), gpui::Modifiers::default());
        settle(cx, tokens::motion::OVERLAY_OPEN_DURATION_MS);
        let nest = cx.debug_bounds("button-Open Song").unwrap();
        cx.simulate_click(nest.center(), gpui::Modifiers::default());
        settle(cx, tokens::motion::OVERLAY_OPEN_DURATION_MS);

        let select = cx.debug_bounds("button-Language: English").unwrap();
        cx.simulate_click(select.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        let popup = cx.debug_bounds("selection-list").expect("the popup opens");
        let before = (
            cx.debug_bounds("scroll-indicator-selection-list").unwrap(),
            cx.debug_bounds("scroll-indicator-overlay-body-Song details")
                .unwrap(),
            cx.debug_bounds("lab-stage").unwrap(),
        );

        cx.simulate_mouse_move(popup.center(), None, gpui::Modifiers::default());
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: popup.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-120.0))),
            modifiers: gpui::Modifiers::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        cx.run_until_parked();

        let after = (
            cx.debug_bounds("scroll-indicator-selection-list").unwrap(),
            cx.debug_bounds("scroll-indicator-overlay-body-Song details")
                .unwrap(),
            cx.debug_bounds("lab-stage").unwrap(),
        );
        assert!(
            after.0.origin.y > before.0.origin.y,
            "the collection under the pointer scrolls"
        );
        assert_eq!(
            after.1.origin.y, before.1.origin.y,
            "the slideover body it sits in does not"
        );
        assert_eq!(
            after.2.origin.y, before.2.origin.y,
            "and neither does the stage behind that"
        );
    }

    /// Toasts sit bottom-center and outlive nothing but their own rules: an
    /// expiring confirmation carries its Undo, an owned failure waits.
    #[gpui::test]
    fn toasts_sit_bottom_center_and_keep_their_two_kinds_apart(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        let (width, height) = ViewportPreset::Compact.size();
        cx.simulate_resize(size(px(width), px(height)));
        lab.update(cx, |lab, cx| assert!(lab.select_scenario("toasts", cx)));
        cx.run_until_parked();

        for label in ["button-Remove Cue", "button-Lose output"] {
            let trigger = cx.debug_bounds(label).unwrap();
            cx.simulate_click(trigger.center(), gpui::Modifiers::default());
            cx.run_until_parked();
        }
        // Both have arrived from below and come to rest; the entrance is what
        // the overlay tests cover, and this one is about where they rest.
        settle(cx, crate::tokens::motion::OVERLAY_OPEN_DURATION_MS);
        let confirmation = cx.debug_bounds("toast-Cue removed.").unwrap();
        let failure = cx
            .debug_bounds("toast-Congregation display disconnected. Program is frozen.")
            .unwrap();
        for (toast, what) in [(confirmation, "confirmation"), (failure, "failure")] {
            assert!(
                toast.bottom() < px(height)
                    && toast.origin.x > px(0.0)
                    && (toast.center().x - px(width / 2.0)).abs() <= px(1.0),
                "the {what} sits bottom-center"
            );
        }
        assert!(cx.debug_bounds("button-Undo").is_some());
        assert!(cx.debug_bounds("button-Dismiss").is_some());

        cx.executor()
            .advance_clock(crate::overlay::TOAST_TIMEOUT + std::time::Duration::from_secs(1));
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("toast-Cue removed.").is_none(),
            "the confirmation expires on its own"
        );
        assert!(
            cx.debug_bounds("toast-Congregation display disconnected. Program is frozen.")
                .is_some(),
            "and the owned failure waits to be dismissed"
        );
        let dismiss = cx.debug_bounds("button-Dismiss").unwrap();
        cx.simulate_click(dismiss.center(), gpui::Modifiers::default());
        settle(cx, crate::tokens::motion::OVERLAY_CLOSE_DURATION_MS);
        assert!(
            cx.debug_bounds("toast-Congregation display disconnected. Program is frozen.")
                .is_none()
        );
    }

    /// The indicator is also a control. `DESIGN.md` only forbids it being the
    /// *only* way to scroll, so it stays out of the way until the pointer comes
    /// for it and then takes the surface with it.
    #[gpui::test]
    fn the_scroll_indicator_widens_for_the_pointer_and_drags_the_surface(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        lab.update(cx, |lab, cx| assert!(lab.select_scenario("select", cx)));
        cx.run_until_parked();
        let trigger = cx.debug_bounds("button-Language: Not assigned").unwrap();
        cx.simulate_click(trigger.center(), gpui::Modifiers::default());
        let popup = cx.debug_bounds("selection-list").unwrap();
        let resting = cx.debug_bounds("scroll-indicator-selection-list").unwrap();
        let last = cx.debug_bounds("choice-5").unwrap();
        assert!(
            last.origin.y > popup.bottom(),
            "the collection starts taller than the popup, or there is nothing to drag"
        );

        // Reaching for the edge is enough: the indicator is too thin to aim at
        // while it is only reporting a position.
        cx.simulate_mouse_move(resting.center(), None, gpui::Modifiers::default());
        let reached = cx.debug_bounds("scroll-indicator-selection-list").unwrap();
        assert!(
            reached.size.width > resting.size.width,
            "the indicator widens to something grabbable"
        );

        // Pulled the whole way down, by a pointer that leaves the indicator's
        // own few pixels immediately.
        cx.simulate_event(gpui::MouseDownEvent {
            position: reached.center(),
            modifiers: gpui::Modifiers::default(),
            button: gpui::MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });
        // The first move past the threshold is what takes hold; GPUI reports a
        // drag to the moves after it.
        cx.simulate_mouse_move(
            gpui::point(reached.center().x, reached.center().y + px(24.0)),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.simulate_mouse_move(
            gpui::point(reached.center().x, popup.bottom() + px(200.0)),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.simulate_event(gpui::MouseUpEvent {
            position: gpui::point(reached.center().x, popup.bottom() + px(200.0)),
            modifiers: gpui::Modifiers::default(),
            button: gpui::MouseButton::Left,
            click_count: 1,
        });

        let dragged = cx.debug_bounds("scroll-indicator-selection-list").unwrap();
        assert!(
            dragged.origin.y > reached.origin.y && dragged.bottom() <= popup.bottom(),
            "the indicator reports where the drag took the surface"
        );
        // Dragged past the end, so the offset is clamped to the surface's
        // maximum and the indicator is at the far end of its travel. Asserting
        // where it lands, rather than only that it stays inside, is what checks
        // that the track really is the surface inset by its own corner — the
        // derivation the bar's length and the drag's own gain both rest on.
        let corner = px(tokens::radius::OVERLAY);
        assert!(
            (dragged.bottom() - (popup.bottom() - corner)).abs() <= px(1.0),
            "at the end of its travel the indicator ends where its track does"
        );
        let last = cx.debug_bounds("choice-5").unwrap();
        assert!(
            last.bottom() <= popup.bottom() && last.origin.y >= popup.origin.y,
            "the end of the collection is what the drag arrived at"
        );
        assert!(
            cx.debug_bounds("selection-list").is_some(),
            "dragging the indicator is not a press outside the popup"
        );
    }

    /// The T0.3 milestone path through the lab, driven by keyboard only.
    ///
    /// `MILESTONES.md` T0.3 E2E, first half: open `ui-lab`, exercise the
    /// field, select, combobox, dialog, and nested-slideover scenarios using
    /// only the keyboard, change from 1920x1080/100% to 1366x768/200%, and
    /// verify state, focus return, and viewport containment.
    ///
    /// Every operator action below is a key event or text input; this test
    /// issues no pointer event. Scenario selection rides the lab's own F2
    /// cycle. Focus stepping is harness setup (see below); the F2 cycle
    /// itself is additionally covered by
    /// `lab_keys_cycle_the_viewport_and_drive_appearance_and_scale`.
    /// Role semantics beyond presence are covered by the node-capture unit
    /// tests named inline; this path proves the composed behavior.
    #[gpui::test]
    fn the_milestone_keyboard_path_holds_state_focus_and_containment_across_viewport_switch(
        cx: &mut TestAppContext,
    ) {
        let enter_ms = tokens::motion::OVERLAY_OPEN_DURATION_MS;
        let exit_ms = tokens::motion::OVERLAY_CLOSE_DURATION_MS;

        let (lab, cx) = open(cx);
        cx.update(|window, _| window.activate_window());

        // The milestone starts at 1920x1080 and 100%.
        cx.simulate_resize(size(px(1920.0), px(1080.0)));
        lab.update(cx, |lab, cx| {
            lab.set_viewport(ViewportPreset::Wide, cx);
            lab.change(
                EnvironmentChange::SetPreferences(UiPreferences {
                    appearance: PreparationAppearance::Light,
                    scale: crate::environment::InterfaceScale::Percent100,
                }),
                cx,
            );
        });
        cx.run_until_parked();

        // Raw key events, one per frame: the shared `keys` shape from the
        // region and shell harnesses. Keystrokes that transfer focus need the
        // frame in between, or the next key reaches the stop it just left.
        let press = |cx: &mut gpui::VisualTestContext, key: &str| {
            let keystroke = gpui::Keystroke::parse(key).unwrap();
            cx.simulate_event(gpui::KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            cx.simulate_event(gpui::KeyUpEvent { keystroke });
            cx.run_until_parked();
        };
        // Harness setup: reach the scenario with the lab's own F2 cycle, then
        // Tab into its first control with the same key an operator presses.
        // The lab root owns the plain `FocusNext` pair; the region registry
        // and overlay trap keep precedence inside.
        let tab_into_scenario = |scenario: &'static str, cx: &mut gpui::VisualTestContext| {
            let root = lab.read_with(cx, |lab, _| lab.focus_handle.clone());
            cx.update(|window, cx| window.focus(&root, cx));
            cx.run_until_parked();
            for _ in 0..SCENARIOS.len() {
                if lab.read_with(cx, |lab, _| lab.scenario().name) == scenario {
                    break;
                }
                press(cx, "f2");
            }
            assert_eq!(
                lab.read_with(cx, |lab, _| lab.scenario().name),
                scenario,
                "F2 reaches {scenario}"
            );
            press(cx, "tab");
            cx.update(|window, cx| window.focused(cx).expect("the first Tab stop takes focus"))
        };
        let contained =
            |selector: &'static str, width: f32, height: f32, cx: &mut gpui::VisualTestContext| {
                let bounds = cx
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("{selector} is on screen"));
                assert!(
                    bounds.origin.x >= px(0.0)
                        && bounds.origin.y >= px(0.0)
                        && bounds.right() <= px(width)
                        && bounds.bottom() <= px(height),
                    "{selector} escapes {width}x{height}: {bounds:?}"
                );
            };

        // Field: Tab reaches every field in reading order and typing delivers
        // into the focused one without disturbing its neighbours. Value
        // reporting is pinned by
        // `primitives::field::tests::typing_edits_the_value_and_the_field_reports_it`
        // and the TextInput role by
        // `primitives::field::tests::field_node_exposes_name_value_placeholder_and_description`;
        // filtering in the combobox below proves Field typing observably.
        let mut last = tab_into_scenario("fields", cx);
        cx.simulate_input("Kyrie");
        cx.run_until_parked();
        for _ in 0..3 {
            press(cx, "tab");
            let now =
                cx.update(|window, cx| window.focused(cx).expect("Tab keeps focus on a field"));
            assert_ne!(now, last, "Tab advances through the fields in order");
            last = now;
        }
        contained("field-control", 1920.0, 1080.0, cx);
        assert!(
            cx.debug_bounds("field-read-only").is_some(),
            "the Read-only reason stays stated"
        );
        assert!(
            cx.debug_bounds("control-unavailable").is_some(),
            "the Unavailable cue stays stated"
        );

        // Select: Enter opens, arrows move, Enter commits, Escape cancels, and
        // focus returns to the trigger either way. Option roles are pinned by
        // `primitives::selection::tests::final_accesskit_nodes_report_roles_selection_and_blocked_actions`.
        let invoker = tab_into_scenario("select", cx);
        press(cx, "enter");
        contained("selection-list", 1920.0, 1080.0, cx);
        press(cx, "escape");
        assert!(
            cx.debug_bounds("selection-list").is_none(),
            "Escape cancels the popup"
        );
        cx.update(|window, _| {
            assert!(
                invoker.is_focused(window),
                "cancelling returns focus to the trigger"
            )
        });
        press(cx, "enter");
        // One key per frame: Enter hands focus to the popup, and Down must
        // reach the popup rather than the trigger it left.
        press(cx, "down");
        press(cx, "enter");
        assert!(
            cx.debug_bounds("selection-list").is_none(),
            "Enter commits and closes"
        );
        assert!(
            cx.debug_bounds("button-Language: English").is_some(),
            "committing English renames the trigger"
        );
        cx.update(|window, _| {
            assert!(
                invoker.is_focused(window),
                "committing returns focus to the trigger"
            )
        });

        // Combobox: typing filters without leaving the Field, Enter commits.
        // The caret/active-choice split is pinned by
        // `primitives::selection::tests::combobox_typing_filters_while_the_caret_stays_in_the_field`.
        let field_focus = tab_into_scenario("combobox", cx);
        cx.simulate_input("kyrie");
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("choice-4").is_some(),
            "filtering leaves the long match"
        );
        assert!(
            cx.debug_bounds("choice-0").is_none(),
            "and filters the rest away"
        );
        assert!(
            cx.debug_bounds("button-Clear").is_some(),
            "a typed value may be cleared"
        );
        assert!(
            cx.debug_bounds("scroll-indicator-selection-list").is_none(),
            "one match fits, so no indicator"
        );
        contained("selection-list", 1920.0, 1080.0, cx);
        // Raw Enter: from inside the Field, a keystroke Enter never reaches
        // the popup root's confirm seam — the unit harness drives the same
        // key the same raw way.
        press(cx, "enter");
        assert!(
            cx.debug_bounds("selection-list").is_none(),
            "Enter commits and closes"
        );
        cx.update(|window, _| {
            assert!(
                field_focus.is_focused(window),
                "the caret never left the Field"
            )
        });

        // Dialog: Enter opens, Tab stays inside, Escape closes with focus back
        // on the invoker. Trap cycling is pinned by
        // `overlay::tests::tab_cycles_inside_the_surface_instead_of_escaping_it`;
        // naming by
        // `overlay::tests::the_surface_names_and_describes_itself_as_a_dialog`.
        let invoker = tab_into_scenario("dialog", cx);
        press(cx, "enter");
        settle(cx, enter_ms);
        contained("overlay-Delete this Mass?", 1920.0, 1080.0, cx);
        assert!(
            cx.debug_bounds("overlay-scrim").is_some(),
            "the scrim captures outside input"
        );
        press(cx, "tab");
        assert!(
            cx.debug_bounds("overlay-Delete this Mass?").is_some(),
            "Tab does not escape the dialog"
        );
        cx.update(|window, _| {
            assert!(
                !invoker.is_focused(window),
                "Tab moved on inside the dialog"
            )
        });
        press(cx, "escape");
        settle(cx, exit_ms);
        assert!(
            cx.debug_bounds("overlay-Delete this Mass?").is_none(),
            "Escape closes the dialog"
        );
        cx.update(|window, _| {
            assert!(
                invoker.is_focused(window),
                "closing returns focus to the invoker"
            )
        });

        // Nested slideover: Enter opens the parent with focus on its first
        // stop, the nested invoker, so Enter opens the child too. A stop
        // that dismisses the parent instead reopens it from the invoker
        // holding focus, converging back on the nested invoker. Retention is
        // pinned by
        // `overlay::tests::a_nested_slideover_keeps_its_parent_and_returns_focus_to_it`.
        let invoker = tab_into_scenario("slideover", cx);
        press(cx, "enter");
        settle(cx, enter_ms);
        contained("overlay-Inspector", 1920.0, 1080.0, cx);
        // Opening leaves focus on no live stop; two Tabs reach the nested
        // invoker through the header. Each step must land somewhere new, so a
        // composed reading-order regression fails here rather than at the
        // Enter below; the child opening proves the second stop was its
        // invoker.
        let unmoved = cx.update(|window, cx| window.focused(cx));
        press(cx, "tab");
        let first = cx.update(|window, cx| window.focused(cx));
        assert_ne!(first, unmoved, "the first Tab leaves the stale stop");
        press(cx, "tab");
        let nested_invoker = cx
            .update(|window, cx| window.focused(cx))
            .expect("the second Tab reaches a live stop");
        assert_ne!(
            Some(nested_invoker.clone()),
            first,
            "the second Tab reaches the nested invoker"
        );
        press(cx, "enter");
        settle(cx, enter_ms);
        contained("overlay-Song details", 1920.0, 1080.0, cx);
        assert!(
            cx.debug_bounds("overlay-Inspector").is_some(),
            "the nested child keeps its parent"
        );
        // Tab stays inside the child; Escape unwinds child then parent, with
        // focus back on the nested invoker and then the scenario invoker.
        press(cx, "tab");
        assert!(
            cx.debug_bounds("overlay-Song details").is_some(),
            "Tab does not escape the nested child"
        );
        press(cx, "escape");
        settle(cx, exit_ms);
        assert!(
            cx.debug_bounds("overlay-Song details").is_none(),
            "Escape closes the child"
        );
        assert!(
            cx.debug_bounds("overlay-Inspector").is_some(),
            "and the parent stays"
        );
        cx.update(|window, _| {
            assert!(
                nested_invoker.is_focused(window),
                "closing the child returns focus to its invoker"
            )
        });
        press(cx, "escape");
        settle(cx, exit_ms);
        assert!(
            cx.debug_bounds("overlay-Inspector").is_none(),
            "Escape closes the parent"
        );
        cx.update(|window, _| {
            assert!(
                invoker.is_focused(window),
                "closing returns focus to the invoker"
            )
        });

        // The milestone's viewport switch: 1366x768 at 200%. Earlier state
        // survives it, and the same Select stays contained with its overflow
        // reported.
        cx.simulate_resize(size(px(1366.0), px(768.0)));
        lab.update(cx, |lab, cx| {
            lab.set_viewport(ViewportPreset::Compact, cx);
            lab.change(
                EnvironmentChange::SetPreferences(UiPreferences {
                    appearance: PreparationAppearance::Light,
                    scale: crate::environment::InterfaceScale::Percent200,
                }),
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("lab-stage").map(|bounds| bounds.size),
            Some(size(px(1366.0), px(768.0))),
            "the stage keeps its exact geometry after the switch"
        );
        let invoker = tab_into_scenario("select", cx);
        // Scenario state is fresh on return — persistence belongs to the
        // shell, proven by the second half below — so commit again at the new
        // scale and prove the same containment, indicator, and focus return.
        assert!(
            cx.debug_bounds("button-Language: Not assigned").is_some(),
            "the scenario restarts from its default"
        );
        press(cx, "enter");
        contained("selection-list", 1366.0, 768.0, cx);
        assert!(
            cx.debug_bounds("scroll-indicator-selection-list").is_some(),
            "the taller popup reports its overflow at 200%"
        );
        press(cx, "down");
        press(cx, "enter");
        assert!(cx.debug_bounds("selection-list").is_none());
        assert!(
            cx.debug_bounds("button-Language: English").is_some(),
            "committing English renames the trigger at 200%"
        );
        cx.update(|window, _| {
            assert!(
                invoker.is_focused(window),
                "focus still returns after the switch"
            )
        });
    }

    /// Dialog body copy follows Interface scale: the card owns the operator
    /// text size, so bare body content measures taller at 200% than at 100%
    /// inside the same viewport. Without it the copy keeps the pin's fixed
    /// default wherever the host root sets none, as `ui-lab` does not.
    #[gpui::test]
    fn the_dialog_body_grows_with_interface_scale(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        cx.simulate_resize(size(px(1920.0), px(1080.0)));
        lab.update(cx, |lab, cx| {
            lab.set_viewport(ViewportPreset::Wide, cx);
            assert!(lab.select_scenario("dialog", cx));
        });
        cx.run_until_parked();

        // Opens the dialog, measures its content height, and closes it, at
        // one scale. Pointer setup only; the assertion is about text sizing.
        let content_height = |scale: crate::environment::InterfaceScale,
                              cx: &mut gpui::VisualTestContext| {
            lab.update(cx, |lab, cx| {
                lab.change(
                    EnvironmentChange::SetPreferences(UiPreferences {
                        appearance: PreparationAppearance::Light,
                        scale,
                    }),
                    cx,
                );
            });
            cx.run_until_parked();
            let trigger = cx.debug_bounds("button-Delete Mass").unwrap();
            cx.simulate_click(trigger.center(), gpui::Modifiers::default());
            cx.run_until_parked();
            settle(cx, tokens::motion::OVERLAY_OPEN_DURATION_MS);
            let height = cx
                .debug_bounds("lab-long-body")
                .expect("the dialog body renders")
                .size
                .height;
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            settle(cx, tokens::motion::OVERLAY_CLOSE_DURATION_MS);
            height
        };

        let at_100 = content_height(crate::environment::InterfaceScale::Percent100, cx);
        let at_200 = content_height(crate::environment::InterfaceScale::Percent200, cx);
        assert!(
            at_200 > at_100,
            "body copy grows with scale ({at_100:?} at 100%, {at_200:?} at 200%)"
        );
    }

    /// F2 keeps cycling scenarios after focus moves inside one: the lab-root
    /// handler answers from any live stop, and only a modal trap (which owns
    /// its keys while open) takes precedence. Covers every scenario, since
    /// each hosts different controls.
    #[gpui::test]
    fn f2_cycles_scenarios_from_inside_a_scenario(cx: &mut TestAppContext) {
        let (lab, cx) = open(cx);
        cx.update(|window, _| window.activate_window());
        assert_eq!(
            lab.read_with(cx, |lab, _| lab.scenario().name),
            "placeholder"
        );

        for next in super::SCENARIOS.iter().skip(1).map(|s| s.name) {
            cx.simulate_keystrokes("f2");
            cx.run_until_parked();
            lab.read_with(cx, |lab, _| assert_eq!(lab.scenario().name, next));
            // Into the scenario with the operator key, then onward: F2 must
            // still cycle from wherever Tab landed.
            cx.simulate_keystrokes("tab");
            cx.run_until_parked();
            assert!(
                cx.update(|window, cx| window.focused(cx)).is_some(),
                "Tab keeps focus somewhere live in {next}"
            );
        }
        cx.simulate_keystrokes("f2");
        cx.run_until_parked();
        lab.read_with(cx, |lab, _| assert_eq!(lab.scenario().name, "placeholder"));
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
