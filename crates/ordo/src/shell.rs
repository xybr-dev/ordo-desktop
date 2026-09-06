//! Ordo's production Preparation shell.
//!
//! Application workflow lives here rather than in `ui`: the crate below owns
//! reusable primitives, the region registry, and the one overlay layer, and
//! this view is the first thing to compose them into a workspace an operator
//! navigates. `ui-lab` composes the same primitives its own way.

pub mod rail;

use std::{mem, rc::Rc};

use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, actions, div, prelude::*, px, rgb,
};
use rail::Destination;
use ui::{
    DecreaseInterfaceScale, IncreaseInterfaceScale, ResetInterfaceScale,
    accessibility::RegionRegistry,
    environment::{
        EnvironmentChange, InterfaceScale, PreparationAppearance, UiEnvironment, UiPreferences,
    },
    overlay::{Overlay, OverlayHost},
    primitives::{
        button::Button,
        selection::{Choice, SelectionState, Tabs},
    },
    resolved_appearance, tokens,
};

actions!(
    ordo_shell,
    [
        OpenSongs,
        UseSystemAppearance,
        UseLightAppearance,
        UseDarkAppearance,
        EnterPresenter,
        LeavePresenter
    ]
);

/// The regions `DESIGN.md` names for a Preparation workspace, in reading order.
/// The Primary list is absent for a single-surface destination and while a
/// focused editor is open, which is exactly what the `F6` cycle skips.
const GLOBAL_NAVIGATION: &str = "Global navigation";
const WORKSPACE_ACTIONS: &str = "Workspace actions";
const PRIMARY_LIST: &str = "Primary list";
const MAIN_EDITOR: &str = "Main editor";

/// Fixture rows until T1 gives the shell real records. One is long and
/// multilingual on purpose: a workspace that cannot hold it would report the
/// horizontal scroll `DESIGN.md` forbids.
const LONG_TITLE: &str =
    "Pasidungog sa Mahal nga Birhen Maria sa Kaputli ug sa Kalinaw sa Katilingban";

fn rows(view: &str) -> Vec<SharedString> {
    vec![
        format!("{view} · 6:00 AM").into(),
        format!("{view} · 9:30 AM").into(),
        LONG_TITLE.into(),
    ]
}

/// The Preparation shell: the rail, the current destination's local views, its
/// list and editor, and the one overlay layer.
pub struct ShellView {
    build_identity: &'static str,
    destination: Destination,
    /// Local view state per destination that has tabs, owned by the view so a
    /// local selection survives leaving the destination and coming back —
    /// window-keyed state would not, because it dies with the unmounted tabs.
    masses_views: SelectionState,
    library_views: SelectionState,
    selected_row: Option<usize>,
    /// The entity a focused editor is open on. The rail stays; the workspace
    /// belongs to this one entity until the operator returns.
    editing: Option<SharedString>,
    registry: RegionRegistry,
    overlays: OverlayHost,
    environment: UiEnvironment,
    preferences_restore_pending: bool,
    appearance_changed_since_startup: bool,
    scale_changed_since_startup: bool,
    save_preferences: Option<Rc<dyn Fn(UiPreferences)>>,
    focus_handle: FocusHandle,
    _appearance_subscription: Subscription,
}

impl ShellView {
    pub const CONNECTION_STATUS: &str = "Offline";

    pub fn new(build_identity: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_preferences(build_identity, UiPreferences::default(), None, window, cx)
    }

    /// Supplying a save sink starts a pending restore; the caller must finish it with
    /// [`Self::finish_preferences_restore`] before subsequent changes can be persisted.
    pub fn with_preferences(
        build_identity: &'static str,
        preferences: UiPreferences,
        save_preferences: Option<Rc<dyn Fn(UiPreferences)>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let environment = UiEnvironment::new(preferences, resolved_appearance(window.appearance()));
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            this.change_environment(
                EnvironmentChange::SystemAppearanceChanged(resolved_appearance(
                    window.appearance(),
                )),
                false,
                cx,
            );
        });
        let views = |destination: Destination, cx: &mut Context<Self>| {
            SelectionState::new(
                destination
                    .local_views()
                    .iter()
                    .map(|view| Choice::new(*view))
                    .collect(),
                Some(0),
                cx,
            )
        };
        Self {
            build_identity,
            destination: Destination::default(),
            masses_views: views(Destination::Masses, cx),
            library_views: views(Destination::Library, cx),
            selected_row: None,
            editing: None,
            registry: RegionRegistry::default(),
            overlays: OverlayHost::new(cx),
            environment,
            preferences_restore_pending: save_preferences.is_some(),
            appearance_changed_since_startup: false,
            scale_changed_since_startup: false,
            save_preferences,
            focus_handle: cx.focus_handle(),
            _appearance_subscription: appearance_subscription,
        }
    }

    pub fn destination(&self) -> Destination {
        self.destination
    }

    /// The active local view of the current destination, or `None` where the
    /// destination has no local views at all.
    pub fn local_view(&self, cx: &App) -> Option<&'static str> {
        let views = self.destination.local_views();
        self.local_views()
            .and_then(|state| state.selected(cx))
            .and_then(|index| views.get(index).copied())
    }

    /// What the current destination's list and editor are showing.
    fn context(&self, cx: &App) -> &'static str {
        self.local_view(cx).unwrap_or(self.destination.label())
    }

    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    pub fn editing(&self) -> Option<&SharedString> {
        self.editing.as_ref()
    }

    pub fn environment(&self) -> UiEnvironment {
        self.environment
    }

    fn local_views(&self) -> Option<&SelectionState> {
        match self.destination {
            Destination::Masses => Some(&self.masses_views),
            Destination::Library => Some(&self.library_views),
            _ => None,
        }
    }

    pub fn open(&mut self, destination: Destination, cx: &mut Context<Self>) {
        if self.destination == destination {
            return;
        }
        self.destination = destination;
        // A destination keeps its own local view, but not another's selection:
        // row 2 of Upcoming means nothing in Songs.
        self.selected_row = None;
        self.editing = None;
        cx.notify();
    }

    pub fn finish_preferences_restore(
        &mut self,
        restored: Option<UiPreferences>,
        cx: &mut Context<Self>,
    ) {
        if !self.preferences_restore_pending {
            return;
        }
        let current = self.environment.preferences();
        let restored = restored.unwrap_or(current);
        let merged = UiPreferences {
            appearance: if self.appearance_changed_since_startup {
                current.appearance
            } else {
                restored.appearance
            },
            scale: if self.scale_changed_since_startup {
                current.scale
            } else {
                restored.scale
            },
        };
        let changed_since_startup = mem::take(&mut self.appearance_changed_since_startup)
            | mem::take(&mut self.scale_changed_since_startup);
        self.preferences_restore_pending = false;
        self.change_environment(EnvironmentChange::SetPreferences(merged), false, cx);
        if changed_since_startup {
            self.save(merged);
        }
    }

    pub fn change_from_settings(&mut self, change: EnvironmentChange, cx: &mut Context<Self>) {
        self.change_environment(change, true, cx);
    }

    fn change_environment(
        &mut self,
        change: EnvironmentChange,
        persist: bool,
        cx: &mut Context<Self>,
    ) {
        let before = self.environment.preferences();
        if !self.environment.apply(change) {
            return;
        }
        let after = self.environment.preferences();
        if persist && after != before {
            self.appearance_changed_since_startup |= after.appearance != before.appearance;
            self.scale_changed_since_startup |= after.scale != before.scale;
            if !self.preferences_restore_pending {
                self.save(after);
            }
        }
        cx.notify();
    }

    fn save(&self, preferences: UiPreferences) {
        if let Some(save) = &self.save_preferences {
            save(preferences);
        }
    }

    /// The action seam for opening the Song Library, which a later menu and
    /// the acceptance path both use. It opens Library on the local view the
    /// operator left it on — Songs, until they change it.
    fn open_songs(&mut self, _: &OpenSongs, _: &mut Window, cx: &mut Context<Self>) {
        self.open(Destination::Library, cx);
    }
}

impl Focusable for ShellView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ShellView {
    fn header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let view = cx.entity().downgrade();
        let overlays = self.overlays.clone();
        let tabs = self.local_views().map(|state| {
            Tabs::new(
                "local-views",
                format!("{} views", self.destination.label()),
                state,
                environment,
            )
            .on_surface(palette.surface)
            // The state is authoritative and notifies on its own; this is the
            // redraw seam for what the local view *shows*.
            .on_select(move |_, _, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.selected_row = None;
                    cx.notify();
                });
            })
        });
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(scale.apply(tokens::spacing::GROUP)))
                    .child(
                        div()
                            .debug_selector(|| "workspace-title".into())
                            .min_w_0()
                            .truncate()
                            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                            .child(self.destination.label()),
                    )
                    .child(
                        // The Inspector `DESIGN.md` puts in a labeled right
                        // slideover when the workspace is constrained, and the
                        // shell's one use of the overlay layer it owns.
                        Button::new("inspector", "Inspector", environment)
                            .on_surface(palette.surface)
                            .on_activate(move |window, cx| {
                                overlays.clone().open(
                                    Overlay::slideover("Inspector")
                                        .description("Details and readiness")
                                        .body(|_, _, _| {
                                            div().child("Nothing is selected.").into_any_element()
                                        }),
                                    window,
                                    cx,
                                );
                            }),
                    ),
            )
            .children(tabs)
            .into_any_element()
    }

    fn primary_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let view_name = self.context(cx);
        let selected = self.selected_row;
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .children(rows(view_name).into_iter().enumerate().map(|(index, row)| {
                let view = cx.entity().downgrade();
                let active = selected == Some(index);
                div()
                    .rounded(px(scale.apply(tokens::radius::CONTROL)))
                    .when(active, |wrapper| wrapper.bg(rgb(palette.selection)))
                    .child(
                        Button::new(("row", index), row, environment)
                            .quiet()
                            .truncate()
                            .on_surface(if active {
                                palette.selection
                            } else {
                                palette.surface
                            })
                            // Selection refreshes the editor beside it and
                            // leaves focus on the row, which is what
                            // `DESIGN.md` asks a primary list for.
                            .on_activate(move |_, cx| {
                                let _ = view.update(cx, |view, cx| {
                                    view.selected_row = Some(index);
                                    cx.notify();
                                });
                            }),
                    )
            }))
            .into_any_element()
    }

    /// The main editor: the selected entity's detail, the focused editor it
    /// opens into, or the destination's own single surface.
    fn editor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let pane = || {
            div()
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::GROUP)))
                .items_start()
        };
        if self.destination == Destination::Settings {
            return self.settings(cx);
        }
        if !self.destination.has_primary_list() {
            return pane()
                .child(format!(
                    "{} has no parish records.",
                    self.destination.label()
                ))
                .into_any_element();
        }
        let view = cx.entity().downgrade();
        if let Some(entity) = self.editing.clone() {
            return pane()
                .debug_selector(|| "focused-editor".into())
                .child(
                    Button::new("back", format!("Back to {}", self.context(cx)), environment)
                        .icon(ui::assets::icon::CHEVRON_RIGHT)
                        .on_surface(palette.background)
                        .on_activate(move |_, cx| {
                            let _ = view.update(cx, |view, cx| {
                                view.editing = None;
                                // The invoker is unmounted with the editor, so
                                // the region gives back its remembered target
                                // or its first tab stop — the Open control the
                                // operator left from.
                                view.registry.focus_region(MAIN_EDITOR);
                                cx.notify();
                            });
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                        .child(entity),
                )
                .into_any_element();
        }
        let Some(row) = self
            .selected_row
            .and_then(|index| rows(self.context(cx)).get(index).cloned())
        else {
            return pane()
                .child("Select an item to see its details.")
                .into_any_element();
        };
        pane()
            .child(
                div()
                    .debug_selector(|| "editor-title".into())
                    .min_w_0()
                    .truncate()
                    .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                    .child(row.clone()),
            )
            .child(
                Button::new("open-editor", "Open", environment)
                    .on_surface(palette.background)
                    .on_activate(move |_, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.editing = Some(row.clone());
                            view.registry.focus_region(MAIN_EDITOR);
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }

    /// Settings owns the appearance and Interface scale preferences. Each is a
    /// row of choices with the current one marked the way the rail marks the
    /// current destination, so neither depends on color alone.
    fn settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let preferences = environment.preferences();
        let choice = |id: SharedString,
                      label: SharedString,
                      active: bool,
                      change: EnvironmentChange,
                      cx: &mut Context<Self>| {
            let view = cx.entity().downgrade();
            div()
                .rounded(px(scale.apply(tokens::radius::CONTROL)))
                .when(active, |row| {
                    row.debug_selector({
                        let label = label.clone();
                        move || format!("setting-active-{label}")
                    })
                    .bg(rgb(palette.selection))
                })
                .child(
                    Button::new(id, label, environment)
                        .on_surface(if active {
                            palette.selection
                        } else {
                            palette.background
                        })
                        .on_activate(move |_, cx| {
                            let _ = view.update(cx, |view, cx| {
                                view.change_from_settings(change, cx);
                            });
                        }),
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
                cx,
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
                cx,
            )
        })
        .collect();
        div()
            .debug_selector(|| "settings-panel".into())
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::SECTION)))
            .child(group("Preparation appearance", appearances))
            .child(group("Interface scale", scales))
            .into_any_element()
    }
}

impl Render for ShellView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let collapsed = rail::collapsed(window.viewport_size().width.into(), scale);
        let gutter = scale.apply(if collapsed {
            tokens::density::WORKSPACE_GUTTER_COMPACT
        } else {
            tokens::density::WORKSPACE_GUTTER_WIDE
        });

        let view = cx.entity().downgrade();
        let rail = rail::render(
            self.destination,
            collapsed,
            environment,
            move |destination| {
                let view = view.clone();
                Box::new(move |_, cx| {
                    let _ = view.update(cx, |view, cx| view.open(destination, cx));
                })
            },
        );
        let header = self.header(cx);
        let show_list = self.destination.has_primary_list() && self.editing.is_none();
        let list = show_list.then(|| self.primary_list(cx));
        let editor = self.editor(cx);

        // Reading order: rail, header, list, editor. Layout nests them; the
        // registry only needs to be their ancestor.
        let rail = self
            .registry
            .region(GLOBAL_NAVIGATION, window, cx)
            .child(rail);
        let header = self
            .registry
            .region(WORKSPACE_ACTIONS, window, cx)
            .flex_none()
            .child(header);
        let list = list.map(|list| {
            self.registry
                .region(PRIMARY_LIST, window, cx)
                .debug_selector(|| "shell-primary-list".into())
                .flex_none()
                .w(px(scale.apply(240.0)))
                .min_w_0()
                .h_full()
                .overflow_hidden()
                .child(list)
        });
        let editor = self
            .registry
            .region(MAIN_EDITOR, window, cx)
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .child(editor);

        div()
            .id("ordo-shell")
            .key_context("OrdoShell")
            .on_action(cx.listener(Self::open_songs))
            .on_action(cx.listener(|this, _: &UseSystemAppearance, _, cx| {
                this.change_from_settings(
                    EnvironmentChange::SetAppearance(PreparationAppearance::System),
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &UseLightAppearance, _, cx| {
                this.change_from_settings(
                    EnvironmentChange::SetAppearance(PreparationAppearance::Light),
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &UseDarkAppearance, _, cx| {
                this.change_from_settings(
                    EnvironmentChange::SetAppearance(PreparationAppearance::Dark),
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &IncreaseInterfaceScale, _, cx| {
                this.change_from_settings(EnvironmentChange::IncreaseScale, cx);
            }))
            .on_action(cx.listener(|this, _: &DecreaseInterfaceScale, _, cx| {
                this.change_from_settings(EnvironmentChange::DecreaseScale, cx);
            }))
            .on_action(cx.listener(|this, _: &ResetInterfaceScale, _, cx| {
                this.change_from_settings(EnvironmentChange::ResetScale, cx);
            }))
            .on_action(cx.listener(|this, _: &EnterPresenter, _, cx| {
                this.change_environment(EnvironmentChange::EnterPresenter, false, cx);
            }))
            .on_action(cx.listener(|this, _: &LeavePresenter, _, cx| {
                this.change_environment(EnvironmentChange::LeavePresenter, false, cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .font_family(tokens::typography::OPERATOR_FAMILY)
            .text_size(px(scale.apply(tokens::typography::BODY.size)))
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .child(
                self.registry
                    .root(window, cx)
                    .track_focus(&self.focus_handle)
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(rail)
                    .child(
                        div()
                            .debug_selector(|| "shell-workspace".into())
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(gutter))
                            .p(px(gutter))
                            .child(header)
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .gap(px(gutter))
                                    .children(list)
                                    .child(editor),
                            ),
                    ),
            )
            // Not a region: nothing here is interactive, and T0.1's build
            // identity and offline status still have to be on screen.
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .px(px(gutter))
                    .py(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                    .border_t(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.surface))
                    .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(
                        div()
                            .debug_selector(|| "build-identity".into())
                            .child(self.build_identity),
                    )
                    .child(
                        div()
                            .debug_selector(|| "connection-status".into())
                            .child(Self::CONNECTION_STATUS),
                    ),
            )
            // The one overlay layer, last and once, as a sibling of the region
            // root so a modal's focus cannot reach the F6 cycle.
            .child(self.overlays.clone().layer(environment, window, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BUILD_IDENTITY;
    use gpui::{Modifiers, TestAppContext, VisualTestContext, size};
    use std::cell::Cell;

    /// Real key down/up pairs, because a primitive's own raw handlers are half
    /// of what the shell is composing.
    fn keys(cx: &mut VisualTestContext, keys: &str) {
        for key in keys.split_whitespace() {
            let keystroke = gpui::Keystroke::parse(key).unwrap();
            cx.simulate_event(gpui::KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            cx.simulate_event(gpui::KeyUpEvent { keystroke });
            cx.run_until_parked();
        }
    }

    /// `debug_bounds` takes a `&'static str` and every selector here is built
    /// from a label, so a test leaks the handful of names it looks up.
    fn sel(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} should be on screen"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }

    fn open(cx: &mut TestAppContext) -> (gpui::Entity<ShellView>, &mut VisualTestContext) {
        cx.update(ui::init).unwrap();
        let (shell, cx) = cx.add_window_view(|window, cx| {
            let shell = ShellView::new(BUILD_IDENTITY, window, cx);
            window.activate_window();
            window.focus(&shell.focus_handle, cx);
            shell
        });
        cx.simulate_resize(size(px(1920.0), px(1080.0)));
        cx.run_until_parked();
        (shell, cx)
    }

    /// The rail owns every global destination and the tabs own every local
    /// view — the hybrid hierarchy `MVP-PLAN.md` requires, walked end to end.
    #[gpui::test]
    fn the_rail_reaches_every_destination_and_tabs_reach_every_local_view(cx: &mut TestAppContext) {
        let (shell, cx) = open(cx);
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.destination(), Destination::Masses);
            assert_eq!(shell.local_view(cx), Some("Upcoming"));
        });
        assert!(cx.debug_bounds("build-identity").is_some());
        assert!(cx.debug_bounds("connection-status").is_some());
        assert_eq!(ShellView::CONNECTION_STATUS, "Offline");

        for destination in Destination::PRIMARY
            .into_iter()
            .chain(Destination::ANCHORED)
        {
            click(cx, sel(format!("button-{}", destination.label())));
            shell.read_with(cx, |shell, _| assert_eq!(shell.destination(), destination));
            // The active treatment is a marker, not only a tint, and it is on
            // exactly one destination.
            for other in Destination::PRIMARY
                .into_iter()
                .chain(Destination::ANCHORED)
            {
                assert_eq!(
                    cx.debug_bounds(sel(format!("rail-active-{}", other.label())))
                        .is_some(),
                    other == destination,
                    "{} marked while {} is active",
                    other.label(),
                    destination.label()
                );
            }
            assert_eq!(
                cx.debug_bounds("shell-primary-list").is_some(),
                destination.has_primary_list(),
                "{} primary list",
                destination.label()
            );
            for (index, view) in destination.local_views().iter().enumerate() {
                click(cx, sel(format!("tab-{index}")));
                shell.read_with(cx, |shell, cx| {
                    assert_eq!(shell.local_view(cx), Some(*view));
                });
            }
        }

        // Settings is the other half of the Interface scale AC: the same
        // preference the Ctrl keys change.
        click(cx, "button-Settings");
        click(cx, "button-150%");
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.environment().preferences().scale,
                InterfaceScale::Percent150
            );
        });
        assert!(cx.debug_bounds("setting-active-150%").is_some());
        keys(cx, "ctrl-0");
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.environment().preferences().scale,
                InterfaceScale::Percent100
            );
        });

        // A destination keeps the local view it was left on.
        click(cx, "button-Library");
        click(cx, "tab-2");
        click(cx, "button-Masses");
        click(cx, "button-Library");
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.local_view(cx), Some("General Presentations"));
        });
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }

    /// `DESIGN.md` decides the rail's labels from effective width and Interface
    /// scale, and forbids whole-workspace horizontal scrolling at either
    /// qualified viewport — so the two are asserted together, on screen.
    #[gpui::test]
    fn the_rail_collapses_and_the_workspace_never_runs_past_the_window(cx: &mut TestAppContext) {
        let (shell, cx) = open(cx);
        let mut seen_collapsed = false;
        let mut seen_expanded = false;
        for (width, height) in [(1366.0, 768.0), (1920.0, 1080.0)] {
            for scale in [
                InterfaceScale::Percent100,
                InterfaceScale::Percent125,
                InterfaceScale::Percent150,
                InterfaceScale::Percent175,
                InterfaceScale::Percent200,
            ] {
                cx.simulate_resize(size(px(width), px(height)));
                shell.update(cx, |shell, cx| {
                    shell.change_from_settings(EnvironmentChange::SetScale(scale), cx);
                });
                cx.run_until_parked();
                let collapsed = rail::collapsed(width, scale);
                seen_collapsed |= collapsed;
                seen_expanded |= !collapsed;
                let rail = cx.debug_bounds("shell-rail").unwrap();
                assert_eq!(
                    rail.size.width,
                    px(rail::width(collapsed, scale)),
                    "rail at {width} and {}%",
                    scale.percent()
                );
                for selector in ["shell-rail", "shell-workspace", "workspace-title"] {
                    let bounds = cx.debug_bounds(selector).unwrap();
                    assert!(
                        bounds.right() <= px(width),
                        "{selector} runs {}px past the window at {width} and {}%",
                        f32::from(bounds.right() - px(width)),
                        scale.percent()
                    );
                }
            }
        }
        assert!(
            seen_collapsed && seen_expanded,
            "the matrix has to exercise both rail states"
        );
    }

    /// Entering and leaving a focused editor is the one transition that takes
    /// the workspace away and gives it back, so everything the operator was in
    /// the middle of has to survive it — including where the keyboard was.
    #[gpui::test]
    fn a_focused_editor_returns_the_destination_local_view_selection_and_invoker(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = open(cx);
        click(cx, "tab-1"); // Masses · History
        click(cx, "button-History · 9:30 AM");
        shell.read_with(cx, |shell, _| assert_eq!(shell.selected_row(), Some(1)));
        assert!(cx.debug_bounds("editor-title").is_some());

        click(cx, "button-Open");
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert_eq!(
                shell.editing().map(SharedString::as_ref),
                Some("History · 9:30 AM")
            );
            assert_eq!(shell.local_view(cx), Some("History"));
        });
        assert!(cx.debug_bounds("focused-editor").is_some());
        assert!(
            cx.debug_bounds("shell-primary-list").is_none(),
            "a focused editor owns the workspace, so the list region is absent"
        );
        assert!(
            cx.debug_bounds("rail-active-Masses").is_some(),
            "the rail keeps the active destination through a focused editor"
        );

        click(cx, "button-Back to History");
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.destination(), Destination::Masses);
            assert_eq!(shell.local_view(cx), Some("History"));
            assert_eq!(shell.selected_row(), Some(1));
            assert!(shell.editing().is_none());
        });
        assert!(cx.debug_bounds("shell-primary-list").is_some());

        // The invoker got the keyboard back: Enter alone reopens the editor.
        keys(cx, "enter");
        shell.read_with(cx, |shell, _| {
            assert!(
                shell.editing().is_some(),
                "returning left focus on the control the editor was opened from"
            );
        });
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn settings_actions_shortcuts_and_presenter_use_the_environment_transition(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = open(cx);

        cx.dispatch_action(UseDarkAppearance);
        cx.dispatch_action(EnterPresenter);
        cx.dispatch_action(LeavePresenter);
        cx.dispatch_action(OpenSongs);
        keys(cx, "ctrl-= ctrl-= ctrl-- ctrl-0");

        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.destination(), Destination::Library);
            assert_eq!(shell.local_view(cx), Some("Songs"));
            let environment = shell.environment();
            assert_eq!(
                environment.preferences().appearance,
                PreparationAppearance::Dark
            );
            assert_eq!(environment.preferences().scale, InterfaceScale::Percent100);
            assert_eq!(
                environment.mode(),
                ui::environment::OperatorMode::Preparation
            );
            assert_eq!(
                environment.appearance(),
                ui::environment::ResolvedAppearance::Dark
            );
        });
    }

    #[gpui::test]
    fn startup_restore_merges_with_changes_without_saving_defaults(cx: &mut TestAppContext) {
        cx.update(ui::init).unwrap();
        let saved = Rc::new(Cell::new(None));
        let (shell, cx) = cx.add_window_view({
            let saved = saved.clone();
            move |window, cx| {
                let shell = ShellView::with_preferences(
                    BUILD_IDENTITY,
                    UiPreferences::default(),
                    Some(Rc::new(move |preferences| saved.set(Some(preferences)))),
                    window,
                    cx,
                );
                window.focus(&shell.focus_handle, cx);
                shell
            }
        });

        cx.dispatch_action(IncreaseInterfaceScale);
        assert_eq!(saved.get(), None);

        shell.update(cx, |shell, cx| {
            shell.finish_preferences_restore(
                Some(UiPreferences {
                    appearance: PreparationAppearance::Dark,
                    scale: InterfaceScale::Percent175,
                }),
                cx,
            );
        });

        let expected = UiPreferences {
            appearance: PreparationAppearance::Dark,
            scale: InterfaceScale::Percent125,
        };
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.environment().preferences(), expected);
            assert!(!shell.preferences_restore_pending);
        });
        assert_eq!(saved.get(), Some(expected));
    }
}
