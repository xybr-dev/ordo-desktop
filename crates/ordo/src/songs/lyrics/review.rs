//! What the Lyrics task says and does about a section's place in the
//! Sequence and its pagination review: Add to Sequence, the review notice
//! with Approve pagination, and the section row's status.
//!
//! Both actions are one structural edit of the shared session. The domain
//! decides every rule (where an occurrence goes, whether a section can be
//! approved); the view never builds a pagination state itself.

use domain::{
    identity::SectionId,
    song_library::{ArrangementLyrics, PaginationState},
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled,
    WeakEntity, Window, div, prelude::FluentBuilder as _, px, rgb, svg,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{button::Button, record::Marker},
    tokens,
};

use super::{editor::ArrangementEditor, panes::section_name};

/// The quiet action that puts an unused section into the Sequence.
pub const ADD_TO_SEQUENCE: &str = "Add to Sequence";

/// The status of a section whose pagination has not been approved.
pub const NEEDS_REVIEW: &str = "Needs review";

/// The status of a section with no lyrics.
pub const EMPTY: &str = "Empty";

/// The notice's action.
pub const APPROVE_PAGINATION: &str = "Approve pagination";

/// The notice's lead for slides that changed after they were approved.
pub const REVIEW_LEAD: &str = "Pagination needs review.";

/// What follows the lead for slides that changed after they were approved.
pub const REVIEW_BODY: &str =
    "The lyrics changed since the slides were approved. Check each slide in the Inspector.";

/// The notice's lead for a section nobody has approved yet.
pub const DRAFT_LEAD: &str = "Pagination not approved yet.";

/// What follows the lead for a section nobody has approved yet.
pub const DRAFT_BODY: &str = "Check each slide in the Inspector, then approve.";

/// The status a section row shows under its meta line: "Empty" for a section
/// with no lyrics, "Needs review" for one that is not Approved, nothing for an
/// Approved one.
pub fn row_status(lyrics: &str, state: PaginationState) -> Option<&'static str> {
    if lyrics.trim().is_empty() {
        Some(EMPTY)
    } else if state == PaginationState::Approved {
        None
    } else {
        Some(NEEDS_REVIEW)
    }
}

/// The toast after Approve.
pub fn approved_message(name: &str) -> String {
    format!("Approved {name} pagination.")
}

impl ArrangementEditor {
    /// Adds an occurrence of an unused section with the shared insertion rule
    /// (after the selected occurrence, else at the end), selects it, and opens
    /// the Sequence tab. Does nothing for a section already in the Sequence
    /// or when the editor is read-only.
    pub fn add_to_sequence(&mut self, section: SectionId, cx: &mut Context<Self>) {
        let Some(before) = self.content(cx) else {
            return;
        };
        if self.is_read_only() || before.occurrences_of(section) != 0 {
            return;
        }
        self.add_occurrence(section, false, cx);
        let grew = self
            .content(cx)
            .is_some_and(|now| now.sequence.len() > before.sequence.len());
        if grew {
            self.show_sequence_tab(cx);
        }
    }

    /// Approves a section's pagination: one structural edit, undoable, with a
    /// toast. A refused save is shown like every other structural edit.
    pub fn approve_pagination(&mut self, section: SectionId, cx: &mut Context<Self>) {
        let Some(content) = self.content(cx) else {
            return;
        };
        let Some(at) = content.sections.iter().position(|row| row.id == section) else {
            return;
        };
        let message = approved_message(&section_name(&content.sections, at));
        let result = self.structural_lyrics(
            move |content| match content.approve_section(section) {
                Ok(changed) => changed,
                Err(error) => {
                    eprintln!("approve refused by the domain: {error:?}");
                    false
                }
            },
            message,
            None,
            cx,
        );
        self.notify_refusal(result, cx);
    }
}

/// The quiet Add to Sequence beside the section's name.
pub(super) fn add_to_sequence_button(
    weak: &WeakEntity<ArrangementEditor>,
    section: SectionId,
    name: &str,
    surface: u32,
    environment: UiEnvironment,
) -> AnyElement {
    let weak = weak.clone();
    div()
        .debug_selector(|| "arrangement-add-to-sequence".into())
        .flex_none()
        .child(
            Button::new("arrangement-add-to-sequence", ADD_TO_SEQUENCE, environment)
                .name(format!("Add {name} to the Sequence"))
                .icon(icon::PLUS)
                .quiet()
                .on_surface(surface)
                .on_activate(move |_: &mut Window, cx: &mut App| {
                    let _ = weak.update(cx, |editor, cx| editor.add_to_sequence(section, cx));
                }),
        )
        .into_any_element()
}

/// The review notice above the lyric editor, with Approve pagination.
pub(super) fn review_notice(
    weak: &WeakEntity<ArrangementEditor>,
    section: SectionId,
    state: PaginationState,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let (lead, body) = match state {
        PaginationState::NeedsReview => (REVIEW_LEAD, REVIEW_BODY),
        _ => (DRAFT_LEAD, DRAFT_BODY),
    };
    let weak = weak.clone();
    div()
        .debug_selector(|| "arrangement-review-notice".into())
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .w_full()
        .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .border(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .bg(rgb(palette.surface_secondary))
        .text_color(rgb(palette.text))
        .child(
            svg()
                .flex_none()
                .size(px(scale.apply(10.0)))
                .text_color(rgb(palette.primary_foreground))
                .data(Marker::Square.svg().as_bytes()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(scale.apply(160.0)))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(lead))
                .child(body),
        )
        .child(
            Button::new(
                "arrangement-approve-pagination",
                APPROVE_PAGINATION,
                environment,
            )
            .icon(icon::CHECK)
            .on_surface(palette.surface_secondary)
            .on_activate(move |_: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |editor, cx| editor.approve_pagination(section, cx));
            }),
        )
        .into_any_element()
}

/// A section row's status line: plain meta for Empty, a square marker and
/// "Needs review" otherwise.
pub(super) fn status_line(
    status: &'static str,
    at: usize,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let info = status == NEEDS_REVIEW;
    div()
        .debug_selector(move || format!("arrangement-section-status-{at}"))
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .text_color(rgb(if info {
            palette.primary_foreground
        } else {
            palette.text_muted
        }))
        .when(info, |line| {
            line.child(
                svg()
                    .flex_none()
                    .size(px(scale.apply(10.0)))
                    .text_color(rgb(palette.primary_foreground))
                    .data(Marker::Square.svg().as_bytes()),
            )
        })
        .child(status)
        .into_any_element()
}

/// Whether the lyric editor shows the review notice for `content`'s section.
pub(super) fn needs_notice(
    content: &ArrangementLyrics,
    section: SectionId,
    has_slides: bool,
) -> bool {
    has_slides
        && content
            .sections
            .iter()
            .find(|row| row.id == section)
            .is_some_and(|row| row.pagination != PaginationState::Approved)
}
