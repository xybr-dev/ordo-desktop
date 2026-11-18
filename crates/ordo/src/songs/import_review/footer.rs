//! The header actions and the sticky footer: Cancel, the summary, Import
//! approved.

use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, Stateful, Styled,
    div, px, rgb,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        Availability,
        button::{Button, Tone},
    },
    tokens,
};

use super::{evidence, review::ImportReview};

/// What blocks **Import approved**, in words; `None` when it can be taken.
fn blocked(review: &ImportReview) -> Option<&'static str> {
    let tally = review.tally();
    if tally.open > 0 {
        Some("Make every decision first.")
    } else if tally.new_arrangements + tally.updates == 0 {
        Some("Everything is already in the Library.")
    } else {
        None
    }
}

/// The header: **Cancel import**, the trail and title, and the reassurance.
pub(super) fn header(
    review: &ImportReview,
    region: Stateful<Div>,
    environment: UiEnvironment,
    cx: &mut Context<ImportReview>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let weak = cx.weak_entity();
    region
        .flex_none()
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .py(px(scale.apply(tokens::spacing::RELATED)))
        .bg(rgb(palette.surface))
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            Button::new("import-cancel", "Cancel import", environment)
                .icon(icon::X)
                .icon_only()
                .quiet()
                .tooltip("Cancel import")
                .on_surface(palette.surface)
                .track_focus(&review.cancel_focus)
                .availability(review.held().map_or(Availability::Available, Availability::unavailable))
                .on_activate(move |window, cx| {
                    // Close the comparison first, outside the review's own
                    // update: closing it calls back into the review.
                    ImportReview::close_surfaces(&weak, window, cx);
                    let _ = weak.update(cx, |review, cx| review.cancel(cx));
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("Library / Songs"),
                )
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child("Review Song import"),
                ),
        )
        .child(
            div()
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child("Nothing has changed in the Library yet."),
        )
        .into_any_element()
}

/// The sticky footer.
pub(super) fn footer(
    review: &ImportReview,
    region: Stateful<Div>,
    environment: UiEnvironment,
    cx: &mut Context<ImportReview>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let summary = evidence::summary(&review.tally());
    // Unavailable, not Disabled: the button keeps its label and its place in
    // the tab order, so focus is not lost while the import runs.
    let availability = review
        .held()
        .or_else(|| blocked(review))
        .map_or(Availability::Available, Availability::unavailable);
    let commit = Button::new("import-approved", "Import approved", environment)
    .icon(icon::CHECK)
    .tone(Tone::Primary)
    .on_surface(palette.surface)
    .availability(availability)
    .on_activate(ImportReview::act(cx, |review, window, cx| {
        review.commit(window, cx);
    }));
    region
        .flex_none()
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .py(px(scale.apply(tokens::spacing::RELATED)))
        .bg(rgb(palette.surface))
        .border_t(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child("One Library update"),
                )
                .child(
                    div()
                        .text_size(px(scale.apply(tokens::typography::META.size)))
                        .text_color(rgb(palette.text_muted))
                        .child("If any approved change fails, none of them is saved."),
                )
                .children(review.refusal.clone().map(|refusal| {
                    div()
                        .debug_selector(|| "import-refusal".into())
                        .text_color(rgb(palette.danger))
                        .child(refusal)
                })),
        )
        .child(
            div()
                .debug_selector(|| "import-summary".into())
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .text_color(rgb(palette.text_muted))
                .child(summary),
        )
        .child(commit)
        .into_any_element()
}
