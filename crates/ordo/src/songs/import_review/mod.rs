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
#[cfg(test)]
mod tests;

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
