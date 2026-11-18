//! The document list: one row per picked file, with its chip.

use domain::song_library::import::{Classification, Outcome, Review, SongChoice, SongIdentity};
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, Role, Stateful,
    StatefulInteractiveElement, Styled, Window, div, prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    primitives::{
        focus_visible,
        list::{self, Badge},
    },
    tokens,
};

use super::review::{ImportReview, default_outcome};

/// The chip of an Arrangement's staged action.
pub(super) fn outcome_chip(outcome: Outcome, classification: Classification) -> &'static str {
    match outcome {
        Outcome::AddNewSong => "Add new Song",
        Outcome::Attach | Outcome::AttachAsAnother => "Attach Arrangement",
        Outcome::AddAsNew => "Add as new Arrangement",
        Outcome::UpdateExisting => "Update existing",
        Outcome::Skip if classification == Classification::SkipUnchanged => "Skip unchanged",
        Outcome::Skip => "Skip document",
    }
}

/// The label of an outcome in the Staged action select and the Outcome group.
pub(super) fn outcome_label(outcome: Outcome, classification: Classification) -> &'static str {
    match outcome {
        Outcome::AddNewSong => "Add new Song",
        Outcome::Attach => "Attach Arrangement",
        Outcome::AttachAsAnother => "Attach as another Arrangement",
        Outcome::AddAsNew => "Add as new Arrangement",
        Outcome::UpdateExisting => "Update existing Arrangement",
        Outcome::Skip if classification == Classification::SkipUnchanged => "Skip unchanged",
        Outcome::Skip => "Skip document",
    }
}

/// The chip of document `index`: **Invalid**, **Needs decision** while a
/// decision is open, else the staged action (the first open decision of a
/// multi-Arrangement document, else its first Arrangement's action).
pub(super) fn chip(review: &ImportReview, index: usize) -> &'static str {
    let Some(Review::Valid(document)) = review.staging.reviews.get(index) else {
        return "Invalid";
    };
    match review.choices.songs.get(&index) {
        Some(SongChoice::Skip) => return "Skip document",
        None if matches!(document.identity, SongIdentity::NeedsDecision { .. }) => {
            return "Needs decision";
        }
        _ => {}
    }
    let staged = |at: usize| {
        let arrangement = &document.arrangements[at];
        review
            .choices
            .arrangements
            .get(&(index, at))
            .copied()
            .or_else(|| default_outcome(arrangement.classification))
    };
    if (0..document.arrangements.len()).any(|at| staged(at).is_none()) {
        return "Needs decision";
    }
    match document.arrangements.first() {
        Some(first) => staged(0).map_or("Needs decision", |outcome| {
            outcome_chip(outcome, first.classification)
        }),
        None => "Skip document",
    }
}

/// The title a row shows: the Song's, or the file name when it is invalid.
pub(super) fn title_of(review: &ImportReview, index: usize) -> String {
    let document = &review.staging.documents[index];
    match &document.content {
        domain::song_library::import::Staged::Valid(valid) => valid.song.title.clone(),
        domain::song_library::import::Staged::Invalid(_) => document.source.clone(),
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The list pane, inside its F6 region.
pub(super) fn render(
    review: &mut ImportReview,
    region: Stateful<Div>,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ImportReview>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let count = review.staging.documents.len();
    let row_focus = review.row_focus.clone();
    let rows = (0..count)
        .map(|index| {
            let title = title_of(review, index);
            let chip = chip(review, index);
            let valid = review.is_staged_valid(index);
            let file = review.staging.documents[index].source.clone();
            let selected = review.selected == index;
            let name = if valid {
                format!("{title}, {file}, {chip}")
            } else {
                format!("{title}, {chip}")
            };
            let select = ImportReview::act(cx, move |review, _, cx| review.select(index, cx));
            let key = ImportReview::act(cx, move |review, _, cx| review.select(index, cx));
            let row = div().id(("import-document", index));
            // The selected row carries the review's handle, so focus can land
            // on it after a removal; the others stay ordinary Tab stops.
            let row = if selected {
                row.track_focus(&row_focus)
            } else {
                row.focusable().tab_stop(true)
            };
            let row = row
                .debug_selector(move || format!("import-doc-{index}"))
                .role(Role::ListItem)
                .aria_label(name)
                .aria_selected(selected)
                .flex_none()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .w_full()
                .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                .py(px(scale.apply(tokens::spacing::RELATED)))
                .border_b(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .text_color(rgb(palette.text))
                .cursor_pointer()
                .when(selected, |row| row.bg(rgb(palette.selection)))
                .on_click(move |_, window, cx| select(window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        key(window, cx);
                    }
                })
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .when(valid, |row| {
                    row.child(
                        div()
                            .w_full()
                            .min_w_0()
                            .truncate()
                            .text_size(px(scale.apply(tokens::typography::META.size)))
                            .text_color(rgb(palette.text_muted))
                            .child(file),
                    )
                })
                .child(Badge::new(chip, environment));
            focus_visible(row, environment, palette.background)
        })
        .collect::<Vec<_>>();
    let scroll = review.list_scroll.clone();
    region
        .size_full()
        .flex()
        .flex_col()
        .child(
            div()
                .debug_selector(|| "import-count".into())
                .flex_none()
                .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                .py(px(scale.apply(tokens::spacing::RELATED)))
                .border_b(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child(plural(count, "document", "documents")),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    list::arrows(scroll.bounded(div().id("import-documents")))
                        .size_full()
                        .flex()
                        .flex_col()
                        .children(rows),
                )
                .children(scroll.indicator("import-documents", 0.0, window, cx, environment)),
        )
        .into_any_element()
}