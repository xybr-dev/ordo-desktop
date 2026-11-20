//! The reference-safety deletion dialog.
//!
//! It opens at once with a loading line while the store reads what still uses
//! the record off the GPUI thread, then becomes either the blocked list (no
//! way to delete) or the safe summary (Cancel and Delete permanently).

use std::{cell::Cell, rc::Rc};

use domain::{
    identity::{ArrangementId, EntityId},
    song_library::{BlockerGroup, DeletionVerdict, LifecycleAction, RecordRef},
};
use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::{Overlay, OverlayHost},
    primitives::button::{Button, Tone},
    tokens,
};

use super::{AfterLifecycle, LifecycleTarget, Request, perform};
use crate::songs::store::{COULDNT_CHECK_USES, LibraryStore, NOT_ARCHIVED};

/// Shown in the dialog while the store reads what still uses the record.
const CHECKING: &str = "Checking what still uses it\u{2026}";

/// The line under the title of the blocked dialog.
pub fn blocked_description(name: &str) -> String {
    format!(
        "{name} can\u{2019}t be deleted while these still use it. Archived, it stays out of planning."
    )
}

/// What a deletion erases, said before the operator commits. An Arrangement
/// names its sections and Sequence; a Song has neither of its own.
pub fn erase_summary(
    target: LifecycleTarget,
    name: &str,
    sections: usize,
    revisions: usize,
) -> String {
    let history = match revisions {
        1 => "its 1 revision of its History".to_owned(),
        n => format!("all {n} revisions of its History"),
    };
    match target {
        LifecycleTarget::Arrangement(_) => {
            let sections = match sections {
                1 => "its 1 section".to_owned(),
                n => format!("its {n} sections"),
            };
            format!(
                "This erases {name}, {sections}, its Sequence and {history}. This can\u{2019}t be undone."
            )
        }
        LifecycleTarget::Song(_) => {
            format!("This erases {name} and {history}. This can\u{2019}t be undone.")
        }
    }
}

/// The heading of a blocker group.
fn heading(group: &BlockerGroup) -> &'static str {
    match group {
        BlockerGroup::NotArchived => "Not archived",
        BlockerGroup::Arrangements(_) => "Arrangements",
        BlockerGroup::MassPlans(_) => "Mass plans",
        BlockerGroup::PresentationSnapshots(_) => "Presentation snapshots",
        BlockerGroup::CelebrationPackages(_) => "Celebration packages",
        BlockerGroup::Other(_) => "Other records",
    }
}

/// The kind a reference group names, in the singular.
fn reference_kind(group: &BlockerGroup) -> &'static str {
    match group {
        BlockerGroup::MassPlans(_) => "Mass plan",
        BlockerGroup::PresentationSnapshots(_) => "Presentation snapshot",
        BlockerGroup::CelebrationPackages(_) => "Celebration package",
        _ => "Record",
    }
}

/// One reference row: its kind and the first eight characters of its id.
pub(super) fn reference_row(kind: &str, id: &EntityId) -> String {
    let id = id.to_string();
    format!("{kind} {}", id.get(..8).unwrap_or(&id))
}

/// Opens the deletion dialog for `target`: a loading line first, then the
/// store's verdict.
pub fn review_deletion(
    target: LifecycleTarget,
    store: Entity<LibraryStore>,
    host: OverlayHost,
    after: AfterLifecycle,
    window: &mut Window,
    cx: &mut App,
) {
    let request = Request::new(target, store, host, after, cx);
    review(request, window, cx);
}

/// Opens the loading dialog and starts the review. Also the way a deletion
/// that the store refused at commit shows what now blocks it.
pub(super) fn review(request: Request, window: &mut Window, cx: &mut App) {
    // Set once the answer was used or the operator walked away. The review
    // is a one-shot read, so its task is detached and `finished` makes a
    // late answer a no-op.
    let finished = Rc::new(Cell::new(false));
    let dismissed = finished.clone();
    let loading = request.host.open(
        Overlay::dialog(format!("Delete {} permanently?", request.name))
            .body(|environment, _, _| checking(environment))
            .secondary("Cancel", |_, _| {})
            .on_close(move |_, _| dismissed.set(true)),
        window,
        cx,
    );
    let record = match request.target {
        LifecycleTarget::Song(id) => RecordRef::Song(id),
        LifecycleTarget::Arrangement(id) => RecordRef::Arrangement(id),
    };
    let review = request
        .store
        .update(cx, |store, cx| store.deletion_review(record, cx));
    window
        .spawn(cx, async move |cx| {
            let result = review.await;
            cx.update(|window, cx| {
                if finished.replace(true) {
                    return;
                }
                request.host.close(loading, window, cx);
                match result {
                    Ok(review) => match review.verdict {
                        DeletionVerdict::Allowed => {
                            confirm(
                                request,
                                review.facts.sections,
                                review.facts.revisions,
                                window,
                                cx,
                            );
                        }
                        DeletionVerdict::Blocked(_) => blocked(request, review.groups, window, cx),
                    },
                    Err(message) => failed(request, message, window, cx),
                }
            })
            .ok();
        })
        .detach();
}

/// The loading line, with the selector tests wait on.
fn checking(environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "lifecycle-checking".into())
        .text_size(px(scale.apply(tokens::typography::BODY.size)))
        .text_color(rgb(palette.text_muted))
        .child(CHECKING)
        .into_any_element()
}

/// The safe dialog: what is erased, then Cancel / Delete permanently.
fn confirm(request: Request, sections: usize, revisions: usize, window: &mut Window, cx: &mut App) {
    let summary = erase_summary(request.target, &request.name, sections, revisions);
    let host = request.host.clone();
    let overlay = Overlay::dialog(format!("Delete {} permanently?", request.name))
        .description(summary)
        .secondary("Cancel", |_, _| {})
        .primary("Delete permanently", move |window, cx| {
            perform(
                LifecycleAction::DeletePermanently,
                request.clone(),
                window,
                cx,
            );
        })
        .primary_tone(Tone::Danger);
    host.open(overlay, window, cx);
}

/// The blocked dialog: every blocker, named, and no way to delete. **Close**
/// is the only action.
fn blocked(request: Request, groups: Vec<BlockerGroup>, window: &mut Window, cx: &mut App) {
    let Request {
        name,
        store,
        host,
        after,
        ..
    } = request;
    // The dialog's own id, known once it is open, so Inspect can close it by
    // id rather than as "the top surface".
    let id = Rc::new(Cell::new(None::<usize>));
    let own = id.clone();
    let body_host = host.clone();
    let overlay = Overlay::dialog(format!("Delete {name} permanently?"))
        .description(blocked_description(&name))
        .body(move |environment, _, cx| {
            blocker_list(&groups, &store, &body_host, &own, &after, environment, cx)
        })
        .secondary("Close", |_, _| {});
    id.set(Some(host.open(overlay, window, cx)));
}

/// The headed blocker groups; each Arrangement row has an Inspect button.
fn blocker_list(
    groups: &[BlockerGroup],
    store: &Entity<LibraryStore>,
    host: &OverlayHost,
    id: &Rc<Cell<Option<usize>>>,
    after: &AfterLifecycle,
    environment: UiEnvironment,
    cx: &App,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let library = store.read(cx).library();
    let line = |text: String| {
        div()
            .text_size(px(scale.apply(tokens::typography::BODY.size)))
            .text_color(rgb(palette.text))
            .child(text)
    };
    let mut inspect_at = 0_usize;
    let mut sections = Vec::new();
    for group in groups {
        let rows: Vec<AnyElement> = match group {
            BlockerGroup::NotArchived => vec![line(NOT_ARCHIVED.to_owned()).into_any_element()],
            BlockerGroup::Arrangements(all) => all
                .iter()
                .map(|arrangement| {
                    let at: usize = inspect_at;
                    inspect_at += 1;
                    let arrangement: ArrangementId = *arrangement;
                    let label = library.arrangement_label(arrangement).unwrap_or_default();
                    let (host, id, after) = (host.clone(), id.clone(), after.clone());
                    div()
                        .debug_selector(move || format!("lifecycle-blocker-{at}"))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .child(line(label.clone()))
                        .child(
                            div()
                                .flex_none()
                                .debug_selector(move || format!("lifecycle-inspect-{at}"))
                                .child(
                                    Button::new(("lifecycle-inspect", at), "Inspect", environment)
                                        .name(format!("Inspect {label}"))
                                        .on_surface(palette.surface)
                                        .on_activate(move |window, cx| {
                                            if let Some(open) = id.get() {
                                                host.close(open, window, cx);
                                            }
                                            (after.inspect)(arrangement, window, cx);
                                        }),
                                ),
                        )
                        .into_any_element()
                })
                .collect(),
            BlockerGroup::MassPlans(all)
            | BlockerGroup::PresentationSnapshots(all)
            | BlockerGroup::CelebrationPackages(all) => all
                .iter()
                .enumerate()
                .map(|(at, record)| {
                    div()
                        .debug_selector(move || format!("lifecycle-reference-{at}"))
                        .child(line(reference_row(reference_kind(group), record)))
                        .into_any_element()
                })
                .collect(),
            BlockerGroup::Other(all) => all
                .iter()
                .enumerate()
                .map(|(at, (kind, record))| {
                    div()
                        .debug_selector(move || format!("lifecycle-reference-{at}"))
                        .child(line(reference_row(kind, record)))
                        .into_any_element()
                })
                .collect(),
        };
        sections.push(
            div()
                .debug_selector(|| format!("lifecycle-group-{}", heading(group)))
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::BODY.size)))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(heading(group)),
                )
                .children(rows),
        );
    }
    div()
        .debug_selector(|| "lifecycle-blockers".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .children(sections)
        .into_any_element()
}

/// The review could not be read: the message, and **Close**.
fn failed(request: Request, message: SharedString, window: &mut Window, cx: &mut App) {
    let text = if message.is_empty() {
        SharedString::from(COULDNT_CHECK_USES)
    } else {
        message
    };
    let overlay = Overlay::dialog(format!("Delete {} permanently?", request.name))
        .description(text)
        .secondary("Close", |_, _| {});
    request.host.open(overlay, window, cx);
}
