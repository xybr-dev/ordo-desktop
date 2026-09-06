//! The region-navigation fixture is also the automated keyboard harness.
use crate::{
    accessibility::RegionRegistry,
    environment::UiEnvironment,
    overlay::{Overlay, OverlayHost},
    primitives::{
        button::Button,
        field::{Field, FieldState},
    },
};
use gpui::{AnyElement, App, Context, FocusHandle, Render, Window, div, prelude::*};

pub(super) fn render(environment: UiEnvironment, window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state("region-scenario", cx, |_, cx| {
        RegionScenario::new(environment, cx)
    });
    view.update(cx, |view, _| view.environment = environment);
    view.into_any_element()
}

struct RegionScenario {
    registry: RegionRegistry,
    host: OverlayHost,
    environment: UiEnvironment,
    inspector: bool,
    selected: bool,
    focus: FocusHandle,
}

impl RegionScenario {
    fn new(environment: UiEnvironment, cx: &mut Context<Self>) -> Self {
        Self {
            registry: RegionRegistry::default(),
            host: OverlayHost::new(cx),
            environment,
            inspector: true,
            selected: false,
            focus: cx.focus_handle(),
        }
    }
}

impl Render for RegionScenario {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let environment = self.environment;
        let view = cx.entity().downgrade();
        let navigation = div()
            .child(Button::new("masses", "Masses", environment))
            .child(Button::new("library", "Library", environment))
            .into_any_element();
        let actions = div()
            .child(
                Button::new("inspector-toggle", "Toggle Inspector", environment).on_activate(
                    move |_, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.inspector = !view.inspector;
                            view.host.announce_status(
                                if view.inspector {
                                    "Inspector shown"
                                } else {
                                    "Inspector hidden"
                                },
                                cx,
                            );
                            cx.notify();
                        });
                    },
                ),
            )
            .into_any_element();
        let view = cx.entity().downgrade();
        let list = div()
            .child(
                Button::new("row", "Select item", environment).on_activate(move |_, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.selected = true;
                        cx.notify();
                    });
                }),
            )
            .into_any_element();
        let host = self.host.clone();
        let field = FieldState::keyed("region-editor-field", "", window, cx);
        let editor = div()
            .child(if self.selected {
                "Selected item"
            } else {
                "Select an item"
            })
            .child(
                Button::new("details", "Open details", environment).on_activate(
                    move |window, cx| {
                        host.open(
                            Overlay::dialog("Details").body(|environment, _, _| {
                                div()
                                    .child(Button::new("detail-one", "First detail", environment))
                                    .child(Button::new("detail-two", "Second detail", environment))
                                    .into_any_element()
                            }),
                            window,
                            cx,
                        );
                    },
                ),
            )
            .child(Field::new("region-title", "Title", &field, environment))
            .into_any_element();
        let mut regions = vec![
            ("Global navigation".into(), navigation),
            ("Workspace actions".into(), actions),
            ("Primary list".into(), list),
            ("Main editor".into(), editor),
        ];
        if self.inspector {
            regions.push((
                "Inspector".into(),
                div()
                    .child(Button::new("inspect", "Inspect item", environment))
                    .into_any_element(),
            ));
        }
        let regions: Vec<_> = regions
            .into_iter()
            .map(|(name, content): (gpui::SharedString, _)| {
                self.registry.region(name, window, cx).child(content)
            })
            .collect();
        div()
            .debug_selector(|| "scenario-regions".into())
            .size_full()
            .child(
                self.registry
                    .root(window, cx)
                    .track_focus(&self.focus)
                    .flex()
                    .flex_col()
                    .children(regions),
            )
            .child(self.host.layer(environment, window, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{ResolvedAppearance, UiPreferences};
    use gpui::{Modifiers, TestAppContext};

    fn keys(cx: &mut gpui::VisualTestContext, keys: &str) {
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

    #[gpui::test]
    fn visible_regions_restore_targets_without_stealing_selection_focus(cx: &mut TestAppContext) {
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(|window, cx| {
            let view = RegionScenario::new(
                UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light),
                cx,
            );
            window.activate_window();
            window.focus(&view.focus, cx);
            view
        });
        cx.run_until_parked();
        // Pointer entry and then Tab use the same remembered target as F6.
        let bounds = cx.debug_bounds("button-Masses").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        keys(cx, "tab");
        let library = cx.update(|window, cx| window.focused(cx).unwrap());
        keys(cx, "tab");
        let actions = cx.update(|window, cx| window.focused(cx).unwrap());
        assert_ne!(actions, library);
        keys(cx, "shift-tab");
        cx.update(|window, _| assert!(library.is_focused(window)));
        keys(cx, "tab");
        keys(cx, "shift-f6");
        cx.update(|window, _| assert!(library.is_focused(window)));
        keys(cx, "f6 enter"); // Hide Inspector.
        view.read_with(cx, |view, _| assert!(!view.inspector));
        keys(cx, "f6");
        let row = cx.update(|window, cx| window.focused(cx).unwrap());
        keys(cx, "enter");
        view.read_with(cx, |view, _| assert!(view.selected));
        cx.update(|window, _| assert!(row.is_focused(window)));
        keys(cx, "f6");
        let editor = cx.update(|window, cx| window.focused(cx).unwrap());
        keys(cx, "f6");
        cx.update(|window, _| {
            assert!(
                library.is_focused(window),
                "hidden Inspector is skipped and cycle wraps"
            )
        });
        keys(cx, "shift-f6");
        cx.update(|window, _| assert!(editor.is_focused(window)));
        keys(cx, "enter");
        let modal = cx.update(|window, cx| window.focused(cx).unwrap());
        keys(cx, "f6 shift-f6");
        cx.update(|window, _| assert!(modal.is_focused(window), "modal owns region traversal"));
        keys(cx, "tab shift-tab escape");
        cx.update(|window, _| assert!(editor.is_focused(window), "closing restores the invoker"));
        keys(cx, "f6");
        cx.update(|window, _| assert!(library.is_focused(window)));
        keys(cx, "shift-f6 tab");
        let field = cx.update(|window, cx| window.focused(cx).unwrap());
        assert_ne!(field, editor);
        keys(cx, "f6 shift-f6");
        cx.update(|window, _| {
            assert!(
                field.is_focused(window),
                "editable Field returns through the same registry"
            )
        });
    }
}
