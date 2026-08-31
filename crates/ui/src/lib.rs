//! Ordo-owned GPUI primitives and application-shell views.

pub mod assets;
pub mod environment;
/// Development-only scenario host; `ui-lab` enables it, operator builds do not.
#[cfg(any(test, feature = "lab"))]
pub mod lab;
pub mod tokens;

use std::{borrow::Cow, mem, rc::Rc};

use environment::{
    EnvironmentChange, PreparationAppearance, ResolvedAppearance, UiEnvironment, UiPreferences,
};
use gpui::{
    App, Context, FocusHandle, Focusable, KeyBinding, Render, Subscription, Window,
    WindowAppearance, actions, div, prelude::*, px, rgb,
};

actions!(
    ordo_shell,
    [
        OpenSongs,
        UseSystemAppearance,
        UseLightAppearance,
        UseDarkAppearance,
        IncreaseInterfaceScale,
        DecreaseInterfaceScale,
        ResetInterfaceScale,
        EnterPresenter,
        LeavePresenter
    ]
);

pub fn init(cx: &mut App) -> Result<(), String> {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(assets::ATKINSON_HYPERLEGIBLE_NEXT),
            Cow::Borrowed(assets::NOTO_SANS),
        ])
        .map_err(|error| error.to_string())?;
    cx.bind_keys(key_bindings());
    Ok(())
}

pub(crate) fn key_bindings() -> [KeyBinding; 4] {
    [
        KeyBinding::new("ctrl-=", IncreaseInterfaceScale, None),
        KeyBinding::new("ctrl-+", IncreaseInterfaceScale, None),
        KeyBinding::new("ctrl--", DecreaseInterfaceScale, None),
        KeyBinding::new("ctrl-0", ResetInterfaceScale, None),
    ]
}

/// The minimal shell destinations needed to prove action-driven navigation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShellDestination {
    #[default]
    Masses,
    Songs,
}

/// T0.1 root view used by the native harness and the production shell.
pub struct RootView {
    build_identity: &'static str,
    active_destination: ShellDestination,
    environment: UiEnvironment,
    preferences_restore_pending: bool,
    appearance_changed_since_startup: bool,
    scale_changed_since_startup: bool,
    save_preferences: Option<Rc<dyn Fn(UiPreferences)>>,
    focus_handle: FocusHandle,
    _appearance_subscription: Subscription,
}

impl RootView {
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
        Self {
            build_identity,
            active_destination: ShellDestination::Masses,
            environment,
            preferences_restore_pending: save_preferences.is_some(),
            appearance_changed_since_startup: false,
            scale_changed_since_startup: false,
            save_preferences,
            focus_handle: cx.focus_handle(),
            _appearance_subscription: appearance_subscription,
        }
    }

    pub fn active_destination(&self) -> ShellDestination {
        self.active_destination
    }

    pub fn environment(&self) -> UiEnvironment {
        self.environment
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

    fn open_songs(&mut self, _: &OpenSongs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_destination = ShellDestination::Songs;
        cx.notify();
    }

    fn use_system_appearance(
        &mut self,
        _: &UseSystemAppearance,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_from_settings(
            EnvironmentChange::SetAppearance(PreparationAppearance::System),
            cx,
        );
    }

    fn use_light_appearance(
        &mut self,
        _: &UseLightAppearance,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_from_settings(
            EnvironmentChange::SetAppearance(PreparationAppearance::Light),
            cx,
        );
    }

    fn use_dark_appearance(
        &mut self,
        _: &UseDarkAppearance,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_from_settings(
            EnvironmentChange::SetAppearance(PreparationAppearance::Dark),
            cx,
        );
    }

    fn increase_interface_scale(
        &mut self,
        _: &IncreaseInterfaceScale,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_from_settings(EnvironmentChange::IncreaseScale, cx);
    }

    fn decrease_interface_scale(
        &mut self,
        _: &DecreaseInterfaceScale,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_from_settings(EnvironmentChange::DecreaseScale, cx);
    }

    fn reset_interface_scale(
        &mut self,
        _: &ResetInterfaceScale,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_from_settings(EnvironmentChange::ResetScale, cx);
    }

    fn enter_presenter(&mut self, _: &EnterPresenter, _: &mut Window, cx: &mut Context<Self>) {
        self.change_environment(EnvironmentChange::EnterPresenter, false, cx);
    }

    fn leave_presenter(&mut self, _: &LeavePresenter, _: &mut Window, cx: &mut Context<Self>) {
        self.change_environment(EnvironmentChange::LeavePresenter, false, cx);
    }
}

pub(crate) fn resolved_appearance(appearance: WindowAppearance) -> ResolvedAppearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => ResolvedAppearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => ResolvedAppearance::Dark,
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RootView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let destination = match self.active_destination {
            ShellDestination::Masses => "Masses",
            ShellDestination::Songs => "Songs",
        };
        let (background, text) = match self.environment.appearance() {
            ResolvedAppearance::Light => (
                tokens::color::PREPARATION_BACKGROUND,
                tokens::color::PREPARATION_TEXT,
            ),
            ResolvedAppearance::Dark => (
                tokens::color::PRESENTER_BACKGROUND,
                tokens::color::PRESENTER_TEXT,
            ),
        };
        let preferences = self.environment.preferences();

        div()
            .id("ordo-root")
            .key_context("OrdoShell")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_songs))
            .on_action(cx.listener(Self::use_system_appearance))
            .on_action(cx.listener(Self::use_light_appearance))
            .on_action(cx.listener(Self::use_dark_appearance))
            .on_action(cx.listener(Self::increase_interface_scale))
            .on_action(cx.listener(Self::decrease_interface_scale))
            .on_action(cx.listener(Self::reset_interface_scale))
            .on_action(cx.listener(Self::enter_presenter))
            .on_action(cx.listener(Self::leave_presenter))
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .p(px(preferences.scale.apply(tokens::spacing::GROUP)))
            .font_family(tokens::typography::OPERATOR_FAMILY)
            .text_size(px(preferences.scale.apply(tokens::typography::BODY.size)))
            .bg(rgb(background))
            .text_color(rgb(text))
            .child(
                div()
                    .debug_selector(|| "build-identity".into())
                    .child(self.build_identity),
            )
            .child(
                div()
                    .debug_selector(|| "connection-status".into())
                    .child(Self::CONNECTION_STATUS),
            )
            .child(
                div()
                    .debug_selector(move || format!("destination-{destination}"))
                    .child(destination),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::cell::Cell;

    #[gpui::test]
    fn root_view_opens_dispatches_navigation_and_stays_offline(cx: &mut TestAppContext) {
        let (root, cx) = cx.add_window_view(|window, cx| {
            let root = RootView::new("test-build", window, cx);
            window.focus(&root.focus_handle, cx);
            root
        });

        root.read_with(cx, |root, _| {
            assert_eq!(RootView::CONNECTION_STATUS, "Offline");
            assert_eq!(root.active_destination(), ShellDestination::Masses);
        });
        assert!(cx.debug_bounds("build-identity").is_some());
        assert!(cx.debug_bounds("connection-status").is_some());
        assert!(cx.debug_bounds("destination-Masses").is_some());

        cx.dispatch_action(OpenSongs);

        root.read_with(cx, |root, _| {
            assert_eq!(root.active_destination(), ShellDestination::Songs);
        });
        assert!(cx.debug_bounds("destination-Songs").is_some());
        cx.run_until_parked();
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn settings_actions_shortcuts_and_presenter_use_the_environment_transition(
        cx: &mut TestAppContext,
    ) {
        cx.update(init).unwrap();
        let (root, cx) = cx.add_window_view(|window, cx| {
            let root = RootView::new("test-build", window, cx);
            window.focus(&root.focus_handle, cx);
            root
        });

        cx.dispatch_action(UseDarkAppearance);
        cx.dispatch_action(EnterPresenter);
        cx.dispatch_action(LeavePresenter);
        cx.simulate_keystrokes("ctrl-= ctrl-= ctrl-- ctrl-0");

        root.read_with(cx, |root, _| {
            assert_eq!(
                root.environment().preferences().appearance,
                PreparationAppearance::Dark
            );
            assert_eq!(
                root.environment().preferences().scale,
                environment::InterfaceScale::Percent100
            );
            assert_eq!(
                root.environment().mode(),
                environment::OperatorMode::Preparation
            );
            assert_eq!(root.environment().appearance(), ResolvedAppearance::Dark);
        });
    }

    /// `ctrl-shift-1` never fired on Linux: xkb reports the shifted keysym, so
    /// the key arrives as `!` and GPUI then drops `shift` for a non-letter, and
    /// no binding written with `shift` plus a non-letter can ever match.
    #[test]
    fn no_binding_asks_for_shift_with_a_key_shift_already_rewrites() {
        for binding in key_bindings().iter().chain(lab::key_bindings().iter()) {
            for keystroke in binding.keystrokes() {
                let keystroke = keystroke.inner();
                let unreachable = keystroke.modifiers.shift
                    && keystroke.key.chars().count() == 1
                    && keystroke.key.to_lowercase() == keystroke.key.to_uppercase();
                assert!(
                    !unreachable,
                    "{keystroke:?} can never be typed: the platform reports the shifted key instead"
                );
            }
        }
    }

    /// The half of live OS-theme following that the pinned GPUI revision lets us
    /// automate; `docs/qualification/GPUI-BASE-T0.3.md` records why the observer
    /// itself cannot be driven from a test.
    #[test]
    fn every_window_appearance_resolves_to_an_operator_appearance() {
        assert_eq!(
            resolved_appearance(WindowAppearance::Light),
            ResolvedAppearance::Light
        );
        assert_eq!(
            resolved_appearance(WindowAppearance::VibrantLight),
            ResolvedAppearance::Light
        );
        assert_eq!(
            resolved_appearance(WindowAppearance::Dark),
            ResolvedAppearance::Dark
        );
        assert_eq!(
            resolved_appearance(WindowAppearance::VibrantDark),
            ResolvedAppearance::Dark
        );
    }

    #[gpui::test]
    fn startup_restore_merges_with_changes_without_saving_defaults(cx: &mut TestAppContext) {
        cx.update(init).unwrap();
        let saved = Rc::new(Cell::new(None));
        let (root, cx) = cx.add_window_view({
            let saved = saved.clone();
            move |window, cx| {
                let root = RootView::with_preferences(
                    "test-build",
                    UiPreferences::default(),
                    Some(Rc::new(move |preferences| saved.set(Some(preferences)))),
                    window,
                    cx,
                );
                window.focus(&root.focus_handle, cx);
                root
            }
        });

        cx.dispatch_action(IncreaseInterfaceScale);
        assert_eq!(saved.get(), None);

        root.update(cx, |root, cx| {
            root.finish_preferences_restore(
                Some(UiPreferences {
                    appearance: PreparationAppearance::Dark,
                    scale: environment::InterfaceScale::Percent175,
                }),
                cx,
            );
        });

        let expected = UiPreferences {
            appearance: PreparationAppearance::Dark,
            scale: environment::InterfaceScale::Percent125,
        };
        root.read_with(cx, |root, _| {
            assert_eq!(root.environment().preferences(), expected);
            assert!(!root.preferences_restore_pending);
            assert!(!root.appearance_changed_since_startup);
            assert!(!root.scale_changed_since_startup);
        });
        assert_eq!(saved.get(), Some(expected));
    }
}
