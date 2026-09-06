//! Operator-only region navigation and expected status transitions.
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{FocusNext, FocusPrevious, NextRegion, PreviousRegion};
use gpui::{
    AnyElement, App, FocusHandle, SharedString, Subscription, WeakFocusHandle, Window, div,
    prelude::*,
};

struct Region {
    focus: FocusHandle,
    remembered: Rc<RefCell<Option<WeakFocusHandle>>>,
    _subscription: Subscription,
}

/// One registry per operator workspace. Supply only visible regions, in reading
/// order, on each render. Names are stable identities and accessible labels.
#[derive(Default)]
pub struct RegionRegistry {
    regions: HashMap<SharedString, Region>,
}

impl RegionRegistry {
    pub fn render(
        &mut self,
        regions: impl IntoIterator<Item = (SharedString, AnyElement)>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let mut visible = Vec::new();
        let mut root = div().id("workspace-regions");
        for (name, content) in regions {
            let region = self.regions.entry(name.clone()).or_insert_with(|| {
                let focus = cx.focus_handle();
                let remembered = Rc::new(RefCell::new(None));
                let saved = remembered.clone();
                let subscription = window.on_focus_out(&focus, cx, move |event, _, _| {
                    *saved.borrow_mut() = Some(event.blurred);
                });
                Region {
                    focus,
                    remembered,
                    _subscription: subscription,
                }
            });
            visible.push((region.focus.clone(), region.remembered.clone()));
            root = root.child(region_container(name, &region.focus).child(content));
        }
        root.on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_action({
                let visible = visible.clone();
                move |_: &NextRegion, window, cx| cycle(&visible, false, window, cx)
            })
            .on_action(move |_: &PreviousRegion, window, cx| cycle(&visible, true, window, cx))
    }
}

fn region_container(name: SharedString, focus: &FocusHandle) -> gpui::Stateful<gpui::Div> {
    div()
        .id(name.clone())
        .track_focus(focus)
        .role(gpui::Role::Region)
        .aria_label(name)
        .aria_keyshortcuts("F6 Shift+F6")
}

type Target = (FocusHandle, Rc<RefCell<Option<WeakFocusHandle>>>);

fn cycle(regions: &[Target], backwards: bool, window: &mut Window, cx: &mut App) {
    if regions.is_empty() {
        return;
    }
    let current = regions
        .iter()
        .position(|(focus, _)| focus.contains_focused(window, cx));
    if let Some(index) = current {
        *regions[index].1.borrow_mut() = window.focused(cx).map(|focus| focus.downgrade());
    }
    let next = match (current, backwards) {
        (Some(index), false) => (index + 1) % regions.len(),
        (Some(index), true) => (index + regions.len() - 1) % regions.len(),
        (None, false) => 0,
        (None, true) => regions.len() - 1,
    };
    let (region, remembered) = &regions[next];
    let target = remembered
        .borrow()
        .as_ref()
        .and_then(WeakFocusHandle::upgrade)
        .filter(|target| region.contains(target, window));
    if let Some(target) = target {
        target.focus(window, cx);
    } else {
        region.focus(window, cx);
        window.focus_next(cx);
        if !region.contains_focused(window, cx) {
            region.focus(window, cx);
        }
    }
}

/// The workspace owns one channel. Ordinary focus/selection never calls it.
/// Repeated identical transitions leave the accessible node unchanged.
#[derive(Default)]
pub struct StatusAnnouncement(Option<SharedString>);

impl StatusAnnouncement {
    pub fn announce(&mut self, message: impl Into<SharedString>) -> bool {
        let message = message.into();
        if self.0.as_ref() == Some(&message) {
            return false;
        }
        self.0 = Some(message);
        true
    }

    pub fn render(&self) -> impl IntoElement + use<> {
        LiveElement::new(
            div()
                .id("workspace-status")
                .absolute()
                .size_0()
                .role(gpui::Role::Status)
                .when_some(self.0.clone(), |node, message| node.aria_label(message)),
            gpui::accesskit::Live::Polite,
        )
    }
}

/// The pin has no live-region setter. Delegate the real element's layout and
/// input behavior, adding only AccessKit's live priority to its existing node.
/// Remove this adapter when GPUI exposes that property on Stateful<Div>.
pub(crate) struct LiveElement {
    element: gpui::Stateful<gpui::Div>,
    priority: gpui::accesskit::Live,
}

impl LiveElement {
    pub(crate) fn new(element: gpui::Stateful<gpui::Div>, priority: gpui::accesskit::Live) -> Self {
        Self { element, priority }
    }
}

impl IntoElement for LiveElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for LiveElement {
    type RequestLayoutState = <gpui::Stateful<gpui::Div> as gpui::Element>::RequestLayoutState;
    type PrepaintState = <gpui::Stateful<gpui::Div> as gpui::Element>::PrepaintState;
    fn id(&self) -> Option<gpui::ElementId> {
        gpui::Element::id(&self.element)
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.element.source_location()
    }
    fn request_layout(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        self.element.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.element
            .prepaint(id, inspector, bounds, layout, window, cx)
    }
    fn paint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element
            .paint(id, inspector, bounds, layout, prepaint, window, cx);
    }
    fn a11y_role(&self) -> Option<gpui::Role> {
        self.element.a11y_role()
    }
    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.element.write_a11y_info(node);
        node.set_live(self.priority);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        Element,
        accesskit::{Live, Node, Role},
    };

    #[test]
    fn status_is_polite_and_deduplicates_only_consecutive_transitions() {
        let mut status = StatusAnnouncement::default();
        let node = |status: &StatusAnnouncement| {
            let element = status.render().into_element();
            let mut node = Node::new(element.a11y_role().unwrap());
            element.write_a11y_info(&mut node);
            node
        };
        assert_eq!(node(&status).label(), None);
        assert!(status.announce("Inspector hidden"));
        assert!(!status.announce("Inspector hidden"));
        assert_eq!(node(&status).role(), Role::Status);
        assert_eq!(node(&status).live(), Some(Live::Polite));
        assert_eq!(node(&status).label(), Some("Inspector hidden"));
        assert!(status.announce("Inspector shown"));
        assert!(status.announce("Inspector hidden"));
        let alert = LiveElement::new(
            div()
                .id("failure")
                .role(Role::Alert)
                .aria_label("Export failed"),
            Live::Assertive,
        );
        let mut node = Node::new(alert.a11y_role().unwrap());
        alert.write_a11y_info(&mut node);
        assert_eq!(node.live(), Some(Live::Assertive));
        assert_eq!(node.label(), Some("Export failed"));
    }

    #[gpui::test]
    fn a_removed_remembered_control_falls_back_to_the_surviving_region(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Workspace {
            registry: RegionRegistry,
            first: FocusHandle,
            removed: FocusHandle,
            second: FocusHandle,
            show_control: bool,
        }
        impl gpui::Render for Workspace {
            fn render(
                &mut self,
                window: &mut Window,
                cx: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                self.registry.render(
                    [
                        (
                            "First".into(),
                            div()
                                .child(div().track_focus(&self.first))
                                .when(self.show_control, |d| {
                                    d.child(div().track_focus(&self.removed))
                                })
                                .into_any_element(),
                        ),
                        (
                            "Second".into(),
                            div().track_focus(&self.second).into_any_element(),
                        ),
                    ],
                    window,
                    cx,
                )
            }
        }
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(|_, cx| Workspace {
            registry: RegionRegistry::default(),
            first: cx.focus_handle().tab_stop(true),
            removed: cx.focus_handle().tab_stop(true),
            second: cx.focus_handle().tab_stop(true),
            show_control: true,
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let removed = view.read(cx).removed.clone();
            removed.focus(window, cx);
        });
        cx.simulate_keystrokes("f6");
        cx.update(|window, cx| assert!(view.read(cx).second.is_focused(window)));
        view.update(cx, |view, cx| {
            view.show_control = false;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("shift-f6");
        cx.update(|window, cx| assert!(view.read(cx).first.is_focused(window)));
    }

    #[gpui::test]
    fn region_node_uses_visible_name_and_declares_traversal(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let element = region_container("Main editor".into(), &cx.focus_handle());
            let mut node = Node::new(element.a11y_role().unwrap());
            element.write_a11y_info(&mut node);
            assert_eq!(node.role(), Role::Region);
            assert_eq!(node.label(), Some("Main editor"));
            assert_eq!(node.keyboard_shortcut(), Some("F6 Shift+F6"));
        });
    }
}
