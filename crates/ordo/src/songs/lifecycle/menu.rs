//! The overflow menu: an icon-only Menu whose rows are exactly the actions the
//! domain offers the record.

use domain::song_library::LifecycleAction;
use gpui::{AnyElement, App, IntoElement as _, Window};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::selection::{Choice, Menu, SelectionState},
    tokens,
};

use super::LifecycleTarget;

/// The row label for `action` on `target`.
pub fn action_label(target: LifecycleTarget, action: LifecycleAction) -> &'static str {
    match (action, target) {
        (LifecycleAction::Archive, LifecycleTarget::Song(_)) => "Archive song",
        (LifecycleAction::Archive, LifecycleTarget::Arrangement(_)) => "Archive arrangement",
        (LifecycleAction::Restore, _) => "Restore",
        (LifecycleAction::DeletePermanently, _) => "Delete permanently\u{2026}",
    }
}

/// The accessible name and tooltip of the trigger.
pub fn trigger_label(target: LifecycleTarget) -> &'static str {
    match target {
        LifecycleTarget::Song(_) => "More Song actions",
        LifecycleTarget::Arrangement(_) => "More Arrangement actions",
    }
}

/// The overflow Menu for `target`. `actions` is what the domain offers it;
/// choosing a row calls `on_action` with that action. The selection state is
/// keyed by `id` and its rows are updated in place, so the trigger keeps focus
/// when the record's state changes under it.
pub fn overflow(
    id: &'static str,
    target: LifecycleTarget,
    actions: &'static [LifecycleAction],
    on_action: impl Fn(LifecycleAction, &mut Window, &mut App) + 'static,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let choices: Vec<Choice> = actions
        .iter()
        .map(|action| Choice::new(action_label(target, *action)))
        .collect();
    let state = SelectionState::keyed(id, choices.clone(), None, window, cx);
    state.set_choices(choices, window, cx);
    Menu::new(id, trigger_label(target), &state, environment)
        .icon(icon::ELLIPSIS)
        .on_surface(tokens::palette(environment.appearance()).background)
        .on_select(move |chosen, window, cx| {
            if let Some(action) = chosen.and_then(|at| actions.get(at).copied()) {
                on_action(action, window, cx);
            }
        })
        .into_any_element()
}
