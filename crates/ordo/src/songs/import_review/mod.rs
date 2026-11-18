//! Review Song import: the full-workspace frame that shows every picked
//! document, what importing it would do and why, and takes the operator's
//! decisions before anything is written.
//!
//! The shell hosts [`ImportReview::render_frame`] while a review is open and
//! closes it on [`ImportReviewEvent`]. Nothing here writes: staging,
//! re-classification and the commit all go through the `LibraryStore`.

mod compare;
mod detail;
mod evidence;
mod footer;
mod list;
pub mod markdown;
mod picker;
mod review;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px, rgb};
use ui::{environment::UiEnvironment, tokens};

pub use compare::{ImportCompare, imported_side, open_compare};
pub use evidence::{arrangement_sentence, song_sentence, summary};
pub use markdown::{MarkdownDraftEvent, MarkdownDraftView};
pub use picker::{FilePicker, Picked, SystemPicker, route};
pub use review::{ImportReview, ImportReviewEvent};

/// The F6 regions, in reading order.
const HEADER_ACTIONS: &str = "Workspace actions";
const DOCUMENT_LIST: &str = "Primary list";
const DOCUMENT_DETAIL: &str = "Main editor";
const REVIEW_FOOTER: &str = "Review summary";

/// How wide the document list is, before the Interface scale.
const LIST_WIDTH: f32 = 340.0;

impl ImportReview {
    /// The whole review frame: header, document list, detail and footer, as
    /// four F6 regions in that order.
    pub fn render_frame(
        &mut self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let header = self.regions.region(HEADER_ACTIONS, window, cx);
        let header = footer::header(self, header, environment, cx);
        let list = self.regions.region(DOCUMENT_LIST, window, cx);
        let list = list::render(self, list, environment, window, cx);
        let detail = self.regions.region(DOCUMENT_DETAIL, window, cx);
        let detail = detail::render(self, detail, environment, window, cx);
        let summary = self.regions.region(REVIEW_FOOTER, window, cx);
        let summary = footer::footer(self, summary, environment, cx);
        if let Some(handle) = self.take_refocus() {
            // Focus lands after this frame is drawn, when the row exists.
            window
                .spawn(cx, async move |cx| {
                    cx.update(|window, cx| handle.focus(window, cx)).ok();
                })
                .detach();
        }
        self.regions
            .root(window, cx)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.background))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .flex_none()
                            .w(px(scale.apply(LIST_WIDTH)))
                            .h_full()
                            .border_r(px(tokens::border::SEPARATOR))
                            .border_color(rgb(palette.border))
                            .child(list),
                    )
                    .child(div().flex_1().min_w_0().h_full().child(detail)),
            )
            .child(summary)
            .into_any_element()
    }

    /// The region that currently holds focus, for tests.
    pub fn focused_region(&self, window: &Window, cx: &gpui::App) -> Option<gpui::SharedString> {
        self.regions.focused_region(window, cx)
    }
}

#[cfg(test)]
pub(crate) mod tests;

/// Reads of the review's private surfaces for the end-to-end tests, so they
/// ask what the operator is told without widening any visibility.
#[cfg(test)]
pub(crate) mod probe {
    use domain::song_library::{
        SongLibrary,
        import::{Outcome, Staged},
    };
    use ui::primitives::Availability;

    use super::{ImportReview, detail, evidence, footer, list};

    /// The classification chip of a document row.
    pub(crate) fn chip(review: &ImportReview, document: usize) -> &'static str {
        list::chip(review, document)
    }

    /// The accessible name of a document row.
    pub(crate) fn row_name(review: &ImportReview, document: usize) -> String {
        list::row_name(review, document)
    }

    /// Whether **Import approved** is available, and why not.
    pub(crate) fn gate(review: &ImportReview) -> Availability {
        footer::import_availability(review)
    }

    /// The footer's summary sentence.
    pub(crate) fn summary(review: &ImportReview) -> String {
        evidence::summary(&review.tally())
    }

    /// The position of `outcome` among those offered for one Arrangement.
    pub(crate) fn offered_at(
        review: &ImportReview,
        document: usize,
        arrangement: usize,
        outcome: Outcome,
    ) -> Option<usize> {
        review
            .review_of(document)?
            .arrangements
            .get(arrangement)?
            .offered
            .iter()
            .position(|offered| *offered == outcome)
    }

    /// The evidence sentence under one Arrangement.
    pub(crate) fn evidence_of(
        review: &ImportReview,
        document: usize,
        arrangement: usize,
        library: &SongLibrary,
    ) -> Option<String> {
        let reviewed = review.review_of(document)?.arrangements.get(arrangement)?;
        Some(evidence::arrangement_sentence(reviewed, library))
    }

    /// The reason an Invalid document gives, in the operator's words.
    pub(crate) fn invalid_reason(review: &ImportReview, document: usize) -> Option<String> {
        let staged = review.documents().get(document)?;
        let Staged::Invalid(reasons) = &staged.content else {
            return None;
        };
        Some(evidence::invalid_sentence(reasons, &staged.source))
    }

    /// The inline disclosure an Update states for one Arrangement's nearest
    /// local match.
    pub(crate) fn update_disclosure(
        review: &ImportReview,
        document: usize,
        arrangement: usize,
        library: &SongLibrary,
    ) -> Option<String> {
        let nearest = review
            .review_of(document)?
            .arrangements
            .get(arrangement)?
            .nearest;
        Some(detail::update_text(library, nearest))
    }
}
