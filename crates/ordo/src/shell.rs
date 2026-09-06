//! Ordo's production Preparation shell.
//!
//! Application workflow lives here rather than in `ui`: the crate below owns
//! reusable primitives, the region registry, and the one overlay layer, and
//! this view is the first thing to compose them into a workspace an operator
//! navigates. `ui-lab` composes the same primitives its own way.
//!
//! **What belongs here is chrome, not content.** The shell owns the rail, the
//! workspace header, the region registration, the one overlay layer, the
//! Interface preferences, and the frame a focused editor is hosted in. It does
//! not own screens. A destination's screens are its own top-level module —
//! `masses/`, `songs/`, `library/`, `themes/`, `transfer/`, `settings/`,
//! `diagnostics/`, `help/`, and `presenter/`, each split by screen the way
//! `MILESTONES.md`'s **Changes:** paths name them — and a focused editor
//! belongs to the entity it edits (`masses/planner/`, `songs/lyrics/`,
//! `themes/editor/`), not to the frame that hosts it. Presenter Mode is a
//! different window with a different region cycle and is not a shell screen at
//! all. Without that rule this file grows once per feature; with it, it stops
//! at the chrome.

pub mod fixtures;
pub mod rail;

use std::{mem, rc::Rc};

use fixtures::rows;
use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, actions, div, prelude::*, px, rgb,
};
use rail::Destination;

use crate::settings;
use ui::{
    DecreaseInterfaceScale, IncreaseInterfaceScale, ResetInterfaceScale,
    accessibility::RegionRegistry,
    environment::{EnvironmentChange, PreparationAppearance, UiEnvironment, UiPreferences},
    overlay::{Overlay, OverlayHost},
    primitives::{
        button::Button,
        list::{self, Badge, ListRow},
        selection::{Choice, SelectionState, Tabs},
    },
    resolved_appearance,
    scroll::Scroll,
    tokens,
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
/// focused editor owns the screen, which is exactly what the `F6` cycle skips.
const GLOBAL_NAVIGATION: &str = "Global navigation";
const WORKSPACE_ACTIONS: &str = "Workspace actions";
const PRIMARY_LIST: &str = "Primary list";
const MAIN_EDITOR: &str = "Main editor";

/// The list column's own logical width, `DESIGN.md`'s minimum for a primary
/// list plus the inset its rows carry.
const LIST_WIDTH: f32 = 280.0;

/// The Preparation shell: the rail, the current destination's local views, its
/// list and viewer, and the one overlay layer.
pub struct ShellView {
    build_identity: &'static str,
    destination: Destination,
    /// Local view state per destination that has tabs, owned by the view so a
    /// local selection survives leaving the destination and coming back —
    /// window-keyed state would not, because it dies with the unmounted tabs.
    masses_views: SelectionState,
    library_views: SelectionState,
    selected_row: Option<usize>,
    /// The entity a focused editor is open on. It takes the whole screen, rail
    /// included: the operator is working on one thing, and Back returns them to
    /// the screen they left rather than to a pane beside it.
    editing: Option<SharedString>,
    list_scroll: Scroll,
    viewer_scroll: Scroll,
    /// The focused editor's body keeps its own scroll rather than sharing the
    /// viewer's: the two are never on screen together, so one handle would hand
    /// the operator back a viewer scrolled to a position they never chose.
    editor_scroll: Scroll,
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
            list_scroll: Scroll::new(),
            viewer_scroll: Scroll::new(),
            editor_scroll: Scroll::new(),
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

    /// What the current destination's list and viewer are showing.
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

    /// The action seam for opening the Song Library, which a later menu and the
    /// acceptance path both use. It opens Library on the local view the
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
    fn gutter(&self, window: &Window) -> f32 {
        let scale = self.environment.preferences().scale;
        let collapsed = rail::collapsed(window.viewport_size().width.into(), scale);
        scale.apply(if collapsed {
            tokens::density::WORKSPACE_GUTTER_COMPACT
        } else {
            tokens::density::WORKSPACE_GUTTER_WIDE
        })
    }

    /// The workspace header: the destination's title and actions above its
    /// local views.
    ///
    /// The seam between what the workspace *is* and what it is *showing* is the
    /// header's own bottom edge, so a destination with no local views is ruled
    /// like every other one. The tab strip, where there is one, draws only the
    /// edge above itself — the seam between the title and the views — and the
    /// header's edge finishes it below. That keeps the pair of rules the tabs
    /// sit between without either surface doubling the other's border.
    fn header(&mut self, gutter: f32, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let view = cx.entity().downgrade();
        let overlays = self.overlays.clone();
        let tabs = self.local_views().map(|state| {
            div()
                .debug_selector(|| "workspace-tabs".into())
                .flex_none()
                .w_full()
                // No gutter: the strip rules the workspace edge to edge, and
                // the first local view starts on that edge rather than floating
                // in from it.
                .border_t(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .child(
                    Tabs::new(
                        "local-views",
                        format!("{} views", self.destination.label()),
                        state,
                        environment,
                    )
                    .on_surface(palette.background)
                    // The state is authoritative and notifies on its own; this
                    // is the redraw seam for what the local view *shows*.
                    .on_select(move |_, _, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.selected_row = None;
                            cx.notify();
                        });
                    }),
                )
        });
        // A short window scrolls its panes; it does not compress the header.
        // `DESIGN.md` puts the current entity and the primary workspace action
        // among the things that are never truncated, and a flex child gives up
        // its own height unless it is told not to.
        div()
            .debug_selector(|| "workspace-header".into())
            .flex_none()
            .flex()
            .flex_col()
            .border_b(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(scale.apply(tokens::spacing::GROUP)))
                    .px(px(gutter))
                    .py(px(scale.apply(tokens::spacing::GROUP)))
                    .child(ui::primitives::truncated(
                        div()
                            .id("workspace-title")
                            .debug_selector(|| "workspace-title".into())
                            .min_w_0()
                            .truncate()
                            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                            .child(self.destination.label()),
                        self.destination.label(),
                        environment,
                    ))
                    .child(
                        // The Inspector `DESIGN.md` puts in a labeled right
                        // slideover when the workspace is constrained, and the
                        // shell's one use of the overlay layer it owns.
                        Button::new("inspector", "Inspector", environment)
                            .on_surface(palette.background)
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

    /// The primary list: one ruled column of rows. It is a bounded scrolling
    /// surface like every other, so it goes through `ui::scroll` rather than
    /// growing its own scroller and its own indicator.
    fn primary_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let palette = tokens::palette(environment.appearance());
        let selected = self.selected_row;
        let scroll = self.list_scroll.clone();
        let rows = rows(self.context(cx))
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let view = cx.entity().downgrade();
                ListRow::new(("row", index), entry.title, environment)
                    .detail(entry.detail)
                    .badges(
                        entry
                            .season
                            .map(|(name, tone)| Badge::new(name, environment).tone(tone)),
                    )
                    .selected(selected == Some(index))
                    .on_surface(palette.background)
                    // Selection refreshes the viewer beside it and leaves focus
                    // on the row, which is what `DESIGN.md` asks a primary list
                    // for. Opening the entity is the separate deliberate act.
                    .on_activate(move |_, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.selected_row = Some(index);
                            cx.notify();
                        });
                    })
            })
            .collect::<Vec<_>>();
        div()
            .relative()
            .size_full()
            .child(
                list::arrows(scroll.bounded(div().id("primary-list")))
                    .size_full()
                    .flex()
                    .flex_col()
                    .children(rows),
            )
            // A square pane, so the indicator's track runs its whole height.
            .children(scroll.indicator("primary-list", 0.0, window, cx, environment))
            .into_any_element()
    }

    /// The content viewer beside the list: what the selected entity is, and the
    /// way into its own screen.
    fn viewer(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
            let view = cx.entity().downgrade();
            return settings::installation::panel(environment, move |change| {
                let view = view.clone();
                Box::new(move |_, cx| {
                    let _ = view.update(cx, |view, cx| view.change_from_settings(change, cx));
                })
            })
            .into_any_element();
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
        let Some(entry) = self
            .selected_row
            .and_then(|index| rows(self.context(cx)).into_iter().nth(index))
        else {
            return pane()
                .child("Select an item to see its details.")
                .into_any_element();
        };
        let title = entry.title.clone();
        pane()
            .w_full()
            .child(ui::primitives::truncated(
                div()
                    .id("viewer-title")
                    .debug_selector(|| "viewer-title".into())
                    .w_full()
                    .min_w_0()
                    .truncate()
                    .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                    .child(entry.title.clone()),
                entry.title,
                environment,
            ))
            .child(
                div()
                    .text_color(rgb(palette.text_muted))
                    .child(entry.detail),
            )
            .child(
                Button::new("open-editor", "Open", environment)
                    .on_surface(palette.background)
                    .on_activate(move |_, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.editing = Some(title.clone());
                            // The screen the operator is arriving on: its Back
                            // control is the first thing in Workspace actions.
                            view.registry.focus_region(WORKSPACE_ACTIONS);
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }

    /// The workspace: the rail beside the destination's header, list, and
    /// viewer. The right border belongs to the list column rather than to the
    /// gap between two padded panes, so it runs the whole height of the pair.
    fn workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let collapsed = rail::collapsed(window.viewport_size().width.into(), scale);
        let gutter = self.gutter(window);

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
        let header = self.header(gutter, cx);
        let list = self
            .destination
            .has_primary_list()
            .then(|| self.primary_list(window, cx));
        let viewer = self.viewer(cx);

        // Reading order: rail, header, list, viewer. Layout nests them; the
        // registry only needs to be their ancestor.
        let rail = self
            .registry
            .region(GLOBAL_NAVIGATION, window, cx)
            .flex_none()
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
                .w(px(scale.apply(LIST_WIDTH)))
                .min_w_0()
                .h_full()
                .overflow_hidden()
                .border_r(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .child(list)
        });
        // The viewer scrolls rather than clipping: a pane that hides the rest
        // of what it holds, with nothing saying so, is the one thing worse
        // than a scrollbar.
        let scroll = self.viewer_scroll.clone();
        let viewer = self
            .registry
            .region(MAIN_EDITOR, window, cx)
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .overflow_hidden()
            .child(
                scroll
                    .bounded(div().id("viewer"))
                    .size_full()
                    .p(px(gutter))
                    .child(viewer),
            )
            .children(scroll.indicator("viewer", 0.0, window, cx, environment));

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
                    .child(header)
                    .child(div().flex_1().min_h_0().flex().children(list).child(viewer)),
            )
            .into_any_element()
    }

    /// A focused editor takes the whole screen, rail included: the operator is
    /// working on one entity, and Back returns them to the screen they left
    /// rather than to a pane beside it.
    fn focused_editor(
        &mut self,
        entity: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let gutter = self.gutter(window);
        let view = cx.entity().downgrade();
        let back = self.context(cx);
        let header = div()
            .flex_none()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(scale.apply(tokens::spacing::GROUP)))
                    .px(px(gutter))
                    .py(px(scale.apply(tokens::spacing::GROUP)))
                    .child(
                        Button::new("back", format!("Back to {back}"), environment)
                            // An icon that describes its action leads the
                            // label, so it has to point the way the control
                            // goes.
                            .icon(ui::assets::icon::CHEVRON_LEFT)
                            .on_surface(palette.background)
                            .on_activate(move |_, cx| {
                                let _ = view.update(cx, |view, cx| {
                                    view.editing = None;
                                    // The invoker unmounts with this screen, so
                                    // the region hands back its remembered
                                    // target or its first tab stop — the Open
                                    // control the operator left from.
                                    view.registry.focus_region(MAIN_EDITOR);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(ui::primitives::truncated(
                        div()
                            .id("editor-title")
                            .debug_selector(|| "editor-title".into())
                            .min_w_0()
                            .truncate()
                            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                            .child(entity.clone()),
                        entity,
                        environment,
                    )),
            )
            .child(
                div()
                    .w_full()
                    .border_t(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border)),
            );

        let header = self
            .registry
            .region(WORKSPACE_ACTIONS, window, cx)
            .flex_none()
            .child(header);
        let scroll = self.editor_scroll.clone();
        let body = self
            .registry
            .region(MAIN_EDITOR, window, cx)
            .flex_1()
            .min_h_0()
            .min_w_0()
            .relative()
            .overflow_hidden()
            .child(
                scroll
                    .bounded(div().id("editor-body"))
                    .size_full()
                    .p(px(gutter))
                    .child("This entity has no editor yet."),
            )
            .children(scroll.indicator("editor-body", 0.0, window, cx, environment));

        self.registry
            .root(window, cx)
            .track_focus(&self.focus_handle)
            .debug_selector(|| "focused-editor".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(body)
            .into_any_element()
    }
}

impl Render for ShellView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let gutter = self.gutter(window);
        let screen = match self.editing.clone() {
            Some(entity) => self.focused_editor(entity, window, cx),
            None => self.workspace(window, cx),
        };

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
            .child(screen)
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
    use ui::environment::InterfaceScale;

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
            // The seam under the header is the header's own, so a destination
            // with no local views is still ruled off from what it shows. Only
            // the strip is conditional.
            assert!(
                cx.debug_bounds("workspace-header").is_some(),
                "{} has no header to rule it",
                destination.label()
            );
            assert_eq!(
                cx.debug_bounds("workspace-tabs").is_some(),
                !destination.local_views().is_empty(),
                "{} tab strip",
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
        // Every choice reserves the mark's slot, so taking one changes the mark
        // and not the line: the appearance choices wrap, and a choice that grew
        // when it was taken would shove its neighbours along and drop one to the
        // next row. Appearance is the half that can be picked without moving
        // every other measurement with it.
        fn appearances(cx: &mut VisualTestContext) -> [gpui::Point<gpui::Pixels>; 3] {
            ["System", "Light", "Dark"].map(|label| {
                cx.debug_bounds(sel(format!("button-{label}")))
                    .unwrap()
                    .origin
            })
        }
        let before = appearances(cx);
        click(cx, "button-Dark");
        assert!(
            cx.debug_bounds("setting-active-Dark").is_some(),
            "the picked appearance is marked"
        );
        assert_eq!(
            before,
            appearances(cx),
            "taking a choice moved the ones beside it"
        );
        click(cx, "button-System");
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
                let reserved = px(if collapsed {
                    rail::collapsed_width(scale)
                } else {
                    scale.apply(rail::EXPANDED_RESERVE)
                });
                assert!(
                    rail.size.width <= reserved,
                    "the rail measures {}px against a reserve of {reserved:?} at {width} and {}%",
                    f32::from(rail.size.width),
                    scale.percent()
                );
                // ... and every row is the width of the rail, rather than of
                // its own label: seven fills at seven widths staggered down
                // the rail and took the icons off their shared edge with them.
                let mut widths = Vec::new();
                // The bug this pins: a rail narrower than its own labels
                // painted them straight past its edge, and only the hover fill
                // showed how far. Every destination has to end inside it.
                for destination in Destination::PRIMARY
                    .into_iter()
                    .chain(Destination::ANCHORED)
                {
                    let item = cx
                        .debug_bounds(sel(format!("button-{}", destination.label())))
                        .unwrap();
                    assert!(
                        item.right() <= rail.right(),
                        "{} runs {}px past the rail at {width} and {}%",
                        destination.label(),
                        f32::from(item.right() - rail.right()),
                        scale.percent()
                    );
                    widths.push(item.size.width);
                    // Collapsed there is no label to line the icons up with,
                    // so they sit in the middle of the rail rather than
                    // off-centre beside a marker.
                    if collapsed {
                        let offset = f32::from(item.center().x - rail.center().x).abs();
                        assert!(
                            offset <= 1.0,
                            "{} sits {offset}px off the centre of a collapsed rail at \
                             {width} and {}%",
                            destination.label(),
                            scale.percent()
                        );
                    }
                }
                assert!(
                    widths.windows(2).all(|pair| pair[0] == pair[1]),
                    "the rail rows measure {widths:?} at {width} and {}%",
                    scale.percent()
                );
                // The marker is the row's own leading edge, inside its fill and
                // on the rail's inset, in both widths — `MILESTONES.md` asks
                // for a cue that is not the fill alone, and a collapsed rail is
                // no exception. Collapsed it is out of flow, which is what
                // leaves the icon centred above.
                let marker = cx
                    .debug_bounds(sel(format!(
                        "rail-active-{}",
                        Destination::default().label()
                    )))
                    .unwrap_or_else(|| {
                        panic!(
                            "the rail marks where it is at {width} and {}%",
                            scale.percent()
                        )
                    });
                assert_eq!(
                    marker.left(),
                    rail.left() + px(scale.apply(tokens::spacing::RELATED)),
                    "the marker is off the rail's inset at {width} and {}%",
                    scale.percent()
                );
                // The list is a bounded scrolling surface like every other, so
                // it reports its position when it overflows and says nothing
                // when it does not — the shared indicator, not one of its own.
                let indicator = cx.debug_bounds("scroll-indicator-primary-list").is_some();
                if (width, scale) == (1366.0, InterfaceScale::Percent200) {
                    assert!(indicator, "a list past the end of its column reports so");
                } else if (width, scale) == (1920.0, InterfaceScale::Percent100) {
                    assert!(!indicator, "a list that fits its column reports nothing");
                }
                // The strip is a seam across the workspace: the first local
                // view starts on its edge, the rest meet it end to end, and
                // they stay on one line however narrow the workspace becomes —
                // a wrapped strip moves every tab under it.
                let strip = cx.debug_bounds("workspace-tabs").unwrap();
                let tabs: Vec<_> = (0..Destination::Masses.local_views().len())
                    .map(|i| cx.debug_bounds(sel(format!("tab-{i}"))).unwrap())
                    .collect();
                assert_eq!(
                    tabs[0].left(),
                    strip.left(),
                    "the first local view is inset from the workspace edge at \
                     {width} and {}%",
                    scale.percent()
                );
                for pair in tabs.windows(2) {
                    assert_eq!(
                        pair[0].top(),
                        pair[1].top(),
                        "the local views wrapped at {width} and {}%",
                        scale.percent()
                    );
                    assert_eq!(
                        pair[0].right(),
                        pair[1].left(),
                        "the local views are spaced apart at {width} and {}%",
                        scale.percent()
                    );
                }
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

    /// A column of records is one collection the operator reads through, so the
    /// arrows move between its rows rather than leaving `Tab` as the only way
    /// down a list. Focus is compared against where clicking the same row puts
    /// it, which is the only thing that says *which* row the arrows reached.
    #[gpui::test]
    fn the_arrows_move_between_list_rows(cx: &mut TestAppContext) {
        let (shell, cx) = open(cx);
        let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
        click(cx, "row-Upcoming \u{b7} entry 3");
        let third = focused(cx);
        click(cx, "row-Upcoming \u{b7} entry 1");
        let first = focused(cx);
        assert!(
            first.is_some() && first != third,
            "clicking a row focuses it"
        );
        shell.read_with(cx, |shell, _| assert_eq!(shell.selected_row(), Some(0)));

        cx.simulate_keystrokes("down down");
        cx.run_until_parked();
        assert_eq!(
            focused(cx),
            third,
            "two rows down from the first is the third"
        );

        cx.simulate_keystrokes("up up");
        cx.run_until_parked();
        assert_eq!(focused(cx), first, "and two back up is where it started");
    }

    /// `DESIGN.md` owes every visually truncated value its full text on pointer
    /// hover as well as to AccessKit. The row's title and detail both truncate,
    /// and so do the workspace, viewer, and editor titles, none of which is a
    /// control that would carry a tooltip on its own.
    #[gpui::test]
    fn a_truncated_value_reveals_its_full_text_to_the_pointer(cx: &mut TestAppContext) {
        /// Rests the pointer on one element and reports where its tooltip
        /// landed. Reporting the bounds rather than only their presence is what
        /// makes the loop below bite: a tooltip that outlived the move to the
        /// next element would otherwise satisfy every iteration after the first.
        fn rest_on(
            cx: &mut VisualTestContext,
            selector: &'static str,
        ) -> Option<gpui::Bounds<gpui::Pixels>> {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} renders"));
            cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
            cx.executor()
                .advance_clock(ui::primitives::button::TOOLTIP_SHOW_DELAY);
            cx.run_until_parked();
            cx.debug_bounds("control-tooltip")
        }

        let (_, cx) = open(cx);
        click(cx, "row-Upcoming \u{b7} entry 1");
        // A value that does not truncate says nothing, which is also what says
        // the reveals below are `truncated`'s doing rather than something the
        // shell shows everywhere.
        let mut previous = rest_on(cx, "connection-status");
        assert!(previous.is_none(), "an untruncated value offers no tooltip");
        for selector in [
            "row-Upcoming \u{b7} entry 1",
            "workspace-title",
            "viewer-title",
        ] {
            let tooltip = rest_on(cx, selector);
            assert!(
                tooltip.is_some() && tooltip != previous,
                "{selector} truncates without offering its own full text"
            );
            previous = tooltip;
        }
        click(cx, "button-Open");
        let tooltip = rest_on(cx, "editor-title");
        assert!(
            tooltip.is_some() && tooltip != previous,
            "the editor title truncates without offering its full text"
        );
    }

    /// More local views than the strip is wide is the case a wrapped strip got
    /// wrong: the overflow dropped onto a second line and moved every tab under
    /// it. They stay on one line and the strip scrolls across instead, which
    /// makes it a bounded scrolling surface like any other and so gives it the
    /// shared indicator.
    #[gpui::test]
    fn a_strip_too_narrow_for_its_local_views_scrolls_across(cx: &mut TestAppContext) {
        let (shell, cx) = open(cx);
        click(cx, "button-Library");
        // Narrower than either qualified viewport on purpose: the strip has to
        // hold up wherever the operator drags the window to, not only where the
        // matrix measures it.
        cx.simulate_resize(size(px(800.0), px(600.0)));
        shell.update(cx, |shell, cx| {
            shell.change_from_settings(EnvironmentChange::SetScale(InterfaceScale::Percent200), cx);
        });
        cx.run_until_parked();

        let strip = cx.debug_bounds("workspace-tabs").unwrap();
        let tabs: Vec<_> = (0..Destination::Library.local_views().len())
            .map(|i| cx.debug_bounds(sel(format!("tab-{i}"))).unwrap())
            .collect();
        assert!(
            tabs.last().unwrap().right() > strip.right(),
            "this is only the overflow case while the views outrun the strip"
        );
        for pair in tabs.windows(2) {
            assert_eq!(pair[0].top(), pair[1].top(), "the local views wrapped");
        }
        assert!(
            cx.debug_bounds("scroll-indicator-local-views").is_some(),
            "a strip with more views than it can show reports its position"
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
        click(cx, "row-History · entry 2");
        shell.read_with(cx, |shell, _| assert_eq!(shell.selected_row(), Some(1)));
        assert!(cx.debug_bounds("viewer-title").is_some());
        // A row carries its classifiers inline, which is what the shape is for.
        assert!(cx.debug_bounds("badge-Easter").is_some());

        // A short window makes the list too long for its column; it does not
        // make the rows shorter. Squeezing them closed is what left the
        // indicator with nothing to report.
        let tall = cx.debug_bounds("row-History \u{b7} entry 1").unwrap();
        assert!(cx.debug_bounds("scroll-indicator-primary-list").is_none());
        cx.simulate_resize(size(px(1366.0), px(420.0)));
        cx.run_until_parked();
        let short = cx.debug_bounds("row-History \u{b7} entry 1").unwrap();
        assert_eq!(
            short.size.height, tall.size.height,
            "a row keeps its own height when the window shortens"
        );
        assert!(
            cx.debug_bounds("scroll-indicator-primary-list").is_some(),
            "the list overflows instead, and says so"
        );
        cx.simulate_resize(size(px(1920.0), px(1080.0)));
        cx.run_until_parked();

        click(cx, "button-Open");
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert_eq!(
                shell.editing().map(SharedString::as_ref),
                Some("History · entry 2")
            );
            assert_eq!(shell.local_view(cx), Some("History"));
        });
        assert!(cx.debug_bounds("focused-editor").is_some());
        // The whole screen, not a pane: the list, the tabs, and the rail all
        // give way to the entity, and come back with it.
        for absent in [
            "shell-primary-list",
            "workspace-tabs",
            "shell-rail",
            "rail-active-Masses",
        ] {
            assert!(
                cx.debug_bounds(absent).is_none(),
                "{absent} survived into a focused editor"
            );
        }

        click(cx, "button-Back to History");
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert_eq!(shell.destination(), Destination::Masses);
            assert_eq!(shell.local_view(cx), Some("History"));
            assert_eq!(shell.selected_row(), Some(1));
            assert!(shell.editing().is_none());
        });
        assert!(cx.debug_bounds("shell-primary-list").is_some());
        assert!(cx.debug_bounds("workspace-tabs").is_some());
        assert!(
            cx.debug_bounds("rail-active-Masses").is_some(),
            "Back returns the whole screen the operator left"
        );

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
