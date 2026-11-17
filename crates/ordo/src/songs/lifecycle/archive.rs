//! The archive dialog and Restore.

use domain::song_library::LifecycleAction;
use gpui::{App, Entity, Window};
use ui::overlay::{Overlay, OverlayHost};

use super::{AfterLifecycle, LifecycleTarget, Request, perform};
use crate::songs::store::LibraryStore;

/// What archiving `target` does, said before the operator commits.
pub fn archive_impact(store: &Entity<LibraryStore>, target: LifecycleTarget, cx: &App) -> String {
    let library = store.read(cx).library();
    match target {
        LifecycleTarget::Song(id) => {
            let count = library.arrangements_for_song(id).map_or(0, |all| all.len());
            let kept = "Existing Mass plans and past Presentation snapshots don\u{2019}t change.";
            match count {
                0 => format!("It has no Arrangements. {kept}"),
                1 => format!(
                    "Its Arrangement leaves planning with it and keeps its own archive state. {kept}"
                ),
                n => format!(
                    "Its {n} Arrangements leave planning with it and keep their own archive state. {kept}"
                ),
            }
        }
        LifecycleTarget::Arrangement(id) => {
            let names: Vec<&str> = library
                .arrangement(id)
                .map(|arrangement| {
                    arrangement
                        .choirs()
                        .iter()
                        .filter_map(|choir| library.choir(*choir).ok().map(|choir| choir.name()))
                        .collect()
                })
                .unwrap_or_default();
            let used = if names.is_empty() {
                "no choir".to_owned()
            } else {
                names.join(", ")
            };
            format!(
                "Used by {used}. It leaves planning; existing Mass plans and past Presentation snapshots don\u{2019}t change."
            )
        }
    }
}

/// Opens the archive dialog: it names what archiving does, and the only
/// answers are Cancel and Archive. Archive settles pending edits through
/// `after`, writes, and toasts with Undo.
pub fn confirm_archive(
    target: LifecycleTarget,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after: AfterLifecycle,
    window: &mut Window,
    cx: &mut App,
) {
    let request = Request::new(target, store.clone(), host.clone(), after, cx);
    let overlay = Overlay::dialog(format!("Archive {}?", request.name))
        .description(archive_impact(&store, target, cx))
        .secondary("Cancel", |_, _| {})
        .primary("Archive", move |window, cx| {
            perform(LifecycleAction::Archive, request.clone(), window, cx);
        });
    host.open(overlay, window, cx);
}

/// Restores `target` at once: restoring needs no confirmation. Toasts
/// "Restored {name}." and calls `after`.
pub fn restore(
    target: LifecycleTarget,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after: AfterLifecycle,
    window: &mut Window,
    cx: &mut App,
) {
    let request = Request::new(target, store, host, after, cx);
    perform(LifecycleAction::Restore, request, window, cx);
}
