//! The Keyboard shortcuts reference (`MVP-PLAN.md` section 6.33, Decisions 232
//! and 67).
//!
//! # One source
//!
//! Every row is a [`registry::ENTRIES`] row and every entry is a row: the
//! model below is built from that table alone and the panel draws the model,
//! so a visible binding can never drift from a registered one. Keys that `ui`
//! binds (`F6`, `Tab`, the Interface-scale keys) are registered there as
//! Surface entries for the same reason. The model has no search and no
//! customization (both are out of the MVP) and knows nothing of the Engine.
//!
//! # Order
//!
//! The current workspace's group comes first, then the others in the fixed
//! [`Group::ALL`] order. The Presenter is the workspace group; Editing lists
//! the keys of an open editor's edit session.
//!
//! # The panel
//!
//! [`panel`] is a viewport-bounded floating panel anchored at the right edge
//! below the header (not a centred modal, so the operational context stays
//! recognizable). It is passive: it holds no command, so opening it never
//! changes Program, and `F`, `B` and `Space` keep their meanings while it has
//! focus. The owner (`PresenterView`) decides when it is open, remembers the
//! focus it came from and restores it on close.

use std::rc::Rc;

use gpui::{
    AnyElement, App, FocusHandle, FontWeight, IntoElement, ParentElement, Role, SharedString,
    Styled, Window, deferred, div, prelude::*, px, rgb,
};
use ui::{
    assets::icon, environment::UiEnvironment, primitives::button::Button, scroll::Scroll, tokens,
};

use crate::commands::registry::{self, CommandId, Group};

/// The panel's title, and the accessible name of the entry points.
pub const TITLE: &str = "Keyboard shortcuts";
/// The panel's close button.
pub const CLOSE: &str = "Close Keyboard shortcuts";
/// Said under the groups, so a suspended key never leaves the operator
/// guessing (Decision 67). The keys named come from the registry.
pub fn notes() -> [String; 2] {
    let key = |id| registry::entry_for(id).keycaps;
    [
        format!(
            "{}, {} and {} keep working while this panel is open.",
            key(CommandId::ToggleFreeze),
            key(CommandId::ToggleBlackout),
            key(CommandId::TakeLive),
        ),
        "Single-key commands pause while you type in a text field or a dialog.".to_owned(),
    ]
}

/// The closing hint under the title, from the registry's two closing keys.
pub fn close_hint() -> String {
    format!(
        "{} or {} to close",
        registry::entry_for(CommandId::ToggleReference).keycaps,
        registry::entry_for(CommandId::CloseSurface).keycaps,
    )
}

/// Width of the panel before the Interface scale (`.sh-keys` in the mockups).
const WIDTH: f32 = 400.0;
/// Above the rail overlay and its scrim, below a popup.
const PRIORITY: usize = 22;

/// One listed key: exactly a [`registry::Entry`]'s keycaps and label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceRow {
    pub id: CommandId,
    pub keycaps: &'static str,
    pub label: &'static str,
}

/// A titled group of rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceGroup {
    pub group: Group,
    pub title: &'static str,
    /// The current workspace's group, which comes first.
    pub current: bool,
    pub rows: Vec<ReferenceRow>,
}

/// What the panel lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceModel {
    pub groups: Vec<ReferenceGroup>,
}

/// The reference for a window whose current workspace is `current`: its group
/// first, then the rest in their fixed order. A group with no entry is omitted.
pub fn model(current: Group) -> ReferenceModel {
    let order = std::iter::once(current).chain(Group::ALL.into_iter().filter(|g| *g != current));
    let groups = order
        .filter_map(|group| {
            let rows: Vec<ReferenceRow> = registry::ENTRIES
                .iter()
                .filter(|entry| entry.group == group)
                .map(|entry| ReferenceRow {
                    id: entry.id,
                    keycaps: entry.keycaps,
                    label: entry.label,
                })
                .collect();
            (!rows.is_empty()).then_some(ReferenceGroup {
                group,
                title: group.title(),
                current: group == current,
                rows,
            })
        })
        .collect();
    ReferenceModel { groups }
}

/// A keycap text split into alternatives and then into the keys of each
/// chord: `"Ctrl+= / Ctrl++"` is `[["Ctrl", "="], ["Ctrl", "+"]]`.
pub fn keycap_chips(keycaps: &str) -> Vec<Vec<String>> {
    keycaps
        .split(" / ")
        .map(|alternative| match alternative.strip_suffix("++") {
            Some(modifiers) => modifiers
                .split('+')
                .map(str::to_owned)
                .chain(std::iter::once("+".to_owned()))
                .collect(),
            None => alternative.split('+').map(str::to_owned).collect(),
        })
        .collect()
}

/// Called by the close button.
pub type OnClose = Rc<dyn Fn(&mut Window, &mut App)>;

/// Where and how the panel is drawn.
pub struct Placement<'a> {
    /// The handle the owner focuses when the panel opens, so the panel is
    /// where keyboard focus sits.
    pub focus: &'a FocusHandle,
    /// The close button's handle: initial focus goes there (`DESIGN.md`,
    /// slide-overs), inside `focus`'s panel so the focus context holds.
    pub close_focus: &'a FocusHandle,
    pub scroll: &'a Scroll,
    /// The panel hangs below this (the header's scaled height).
    pub top: f32,
}

/// The floating panel over the Presenter.
pub fn panel(
    model: &ReferenceModel,
    placement: Placement<'_>,
    on_close: OnClose,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Placement {
        focus,
        close_focus,
        scroll,
        top,
    } = placement;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let viewport = f32::from(window.viewport_size().width);
    // `DESIGN.md` keeps the exterior gutters on a bounded surface: the panel
    // hangs below the header with the workspace gutter at its right, bottom and
    // (in a narrow window) left. `DESIGN.md` does not allow an edge-anchored
    // slide-over here.
    let gutter = scale.apply(tokens::density::WORKSPACE_GUTTER_WIDE);
    let width = scale.apply(WIDTH).min((viewport - 2.0 * gutter).max(0.0));

    let close = Button::new(CLOSE, CLOSE, environment)
        .icon(icon::X)
        .icon_only()
        .quiet()
        .on_surface(palette.surface)
        .track_focus(close_focus)
        .on_activate(move |window, cx| on_close(window, cx));
    // The cross leads the header, before the title (Decision 299).
    let header = div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(scale.apply(tokens::spacing::RELATED)))
        .h(px(scale.apply(tokens::density::DEFAULT_CONTROL + 16.0)))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .debug_selector(|| "shortcut-reference-close".into())
                .child(close),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                        .child(TITLE),
                )
                .child(
                    div()
                        .truncate()
                        .text_color(rgb(palette.text_muted))
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .child(close_hint()),
                ),
        );

    let mut body: Vec<AnyElement> = Vec::new();
    for group in &model.groups {
        let title: SharedString = if group.current {
            format!("{} \u{b7} this workspace", group.title).into()
        } else {
            group.title.into()
        };
        body.push(
            div()
                .flex_none()
                .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                .pt(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                .pb(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .font_weight(FontWeight::MEDIUM)
                .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                .child(title)
                .into_any_element(),
        );
        for row in &group.rows {
            body.push(row_element(row, environment));
        }
    }
    body.extend(notes().into_iter().map(|note| {
        div()
            .flex_none()
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .pt(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .text_color(rgb(palette.text_muted))
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .child(note)
            .into_any_element()
    }));

    let panel = div()
        .id("shortcut-reference-panel")
        .debug_selector(|| "shortcut-reference".into())
        .absolute()
        .top(px(top))
        .bottom(px(gutter))
        .right(px(gutter))
        .w(px(width))
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(palette.surface))
        .text_color(rgb(palette.text))
        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
        .rounded(px(scale.apply(tokens::radius::OVERLAY)))
        .border(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border_strong))
        .shadow(vec![tokens::elevation::overlay(
            environment.appearance(),
            scale,
        )])
        // The panel takes its own presses and its own wheel.
        .occlude()
        .role(Role::Complementary)
        .aria_label(TITLE)
        .track_focus(focus)
        .child(header)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    scroll
                        .bounded(div().id("shortcut-reference-scroll"))
                        .size_full()
                        .flex()
                        .flex_col()
                        .pb(px(scale.apply(tokens::spacing::GROUP)))
                        .children(body),
                )
                .children(scroll.indicator("shortcut-reference", 0.0, window, cx, environment)),
        );
    deferred(
        div()
            .id("shortcut-reference-layer")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(panel),
    )
    .with_priority(PRIORITY)
    .into_any_element()
}

/// One row: the action on the left, its keycaps on the right (`.sh-krow`).
fn row_element(row: &ReferenceRow, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let alternatives = keycap_chips(row.keycaps);
    div()
        .flex_none()
        .mx(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .py(px(scale.apply(6.0)))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(div().min_w_0().child(row.label))
        .child(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .children(alternatives.into_iter().enumerate().map(|(at, chord)| {
                    let mut group = div().flex().flex_row().items_center().gap(px(1.0));
                    if at > 0 {
                        group = group.ml(px(scale.apply(tokens::spacing::RELATED_TIGHT)));
                    }
                    let last = chord.len().saturating_sub(1);
                    group.children(chord.into_iter().enumerate().flat_map(move |(i, key)| {
                        let cap = div()
                            .px(px(scale.apply(5.0)))
                            .py(px(scale.apply(2.0)))
                            .border(px(tokens::border::SEPARATOR))
                            .border_color(rgb(palette.border_strong))
                            .rounded(px(scale.apply(tokens::radius::CONTROL)))
                            .text_color(rgb(palette.text_muted))
                            .text_size(px(scale.apply(tokens::typography::CAPTION.size)))
                            .whitespace_nowrap()
                            .child(key)
                            .into_any_element();
                        let plus = (i < last).then(|| {
                            div()
                                .text_color(rgb(palette.text_muted))
                                .child("+")
                                .into_any_element()
                        });
                        std::iter::once(cap).chain(plus)
                    }))
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::registry::ENTRIES;

    #[test]
    fn every_row_is_a_registry_row_with_its_exact_keycaps_and_label() {
        for current in Group::ALL {
            for group in model(current).groups {
                for row in group.rows {
                    let entry = registry::entry_for(row.id);
                    assert_eq!(entry.group, group.group, "{:?}", row.id);
                    assert_eq!(entry.keycaps, row.keycaps, "{:?}", row.id);
                    assert_eq!(entry.label, row.label, "{:?}", row.id);
                }
            }
        }
    }

    #[test]
    fn every_registry_entry_is_listed_exactly_once() {
        for current in Group::ALL {
            let listed: Vec<CommandId> = model(current)
                .groups
                .iter()
                .flat_map(|g| g.rows.iter().map(|r| r.id))
                .collect();
            assert_eq!(listed.len(), ENTRIES.len(), "{current:?}");
            for entry in ENTRIES {
                assert_eq!(
                    listed.iter().filter(|id| **id == entry.id).count(),
                    1,
                    "{:?}",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn the_current_workspace_comes_first_and_the_rest_keep_their_order() {
        let presenter = model(Group::Presenter);
        let titles: Vec<_> = presenter.groups.iter().map(|g| g.title).collect();
        assert_eq!(titles, ["Presenter", "Global", "Editing"]);
        assert!(presenter.groups[0].current);
        assert!(!presenter.groups[1].current);

        let global = model(Group::Global);
        let titles: Vec<_> = global.groups.iter().map(|g| g.title).collect();
        assert_eq!(titles, ["Global", "Presenter", "Editing"]);
    }

    #[test]
    fn the_surface_keys_are_listed_under_global_with_the_presenter_keys_apart() {
        let presenter = model(Group::Presenter);
        let global = &presenter.groups[1];
        for id in [
            CommandId::ToggleReference,
            CommandId::CloseSurface,
            CommandId::CycleRegion,
            CommandId::FocusNext,
            CommandId::ScaleUp,
        ] {
            assert!(global.rows.iter().any(|r| r.id == id), "{id:?}");
        }
        for key in ["Space", "F", "B", "O", "A"] {
            assert!(
                presenter.groups[0].rows.iter().any(|r| r.keycaps == key),
                "{key}"
            );
        }
    }

    #[test]
    fn keycaps_split_into_alternatives_and_chords() {
        assert_eq!(keycap_chips("Space"), [["Space"]]);
        assert_eq!(keycap_chips("Left / Up"), [["Left"], ["Up"]]);
        assert_eq!(keycap_chips("Ctrl+/"), [["Ctrl", "/"]]);
        assert_eq!(keycap_chips("Ctrl+-"), [["Ctrl", "-"]]);
        assert_eq!(
            keycap_chips("Ctrl+= / Ctrl++"),
            [vec!["Ctrl", "="], vec!["Ctrl", "+"]]
        );
        assert_eq!(
            keycap_chips("Tab / Shift+Tab"),
            [vec!["Tab"], vec!["Shift", "Tab"]]
        );
    }
}
