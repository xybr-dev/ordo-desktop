//! The Available sections pane of the Sequence task: every section with how
//! often the Sequence uses it, and **Add** to place it.
//!
//! Rows are not controls. Choosing or focusing one changes nothing; only
//! **Add** edits the Sequence.

use domain::{pagination::slides, song_library::ArrangementLyrics};
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, Role, Styled,
    WeakEntity, div, prelude::*, px, rgb,
};
use ui::{assets::icon, environment::UiEnvironment, primitives::button::Button, tokens};

use crate::songs::lyrics::{ArrangementEditor, panes::section_name};

/// The pane's heading.
pub const HEAD: &str = "Available sections";

/// What the pane says while the Arrangement has no sections.
pub const NO_SECTIONS: &str = "Write sections in Lyrics first.";

/// The row's meta line: "{n} slide(s) · used {n}×" or "{n} slide(s) · unused".
pub fn meta(content: &ArrangementLyrics, lyrics: &str, id: domain::identity::SectionId) -> String {
    let count = slides(lyrics).len();
    let slides = if count == 1 {
        "1 slide".to_owned()
    } else {
        format!("{count} slides")
    };
    match content.occurrences_of(id) {
        0 => format!("{slides} \u{b7} unused"),
        used => format!("{slides} \u{b7} used {used}\u{d7}"),
    }
}

/// The Available sections pane. An archived Arrangement lists its sections
/// without **Add**.
pub(super) fn pane(
    weak: &WeakEntity<ArrangementEditor>,
    content: &ArrangementLyrics,
    read_only: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let rows = content.sections.iter().enumerate().map(|(at, section)| {
        let id = section.id;
        let name = section_name(&content.sections, at);
        let add = (!read_only).then(|| {
            let weak = weak.clone();
            div()
                .debug_selector(move || format!("sequence-add-{at}"))
                .flex_none()
                .child(
                    Button::new(("sequence-add", at), "Add", environment)
                        .name(format!("Add {name} to the Sequence"))
                        .icon(icon::PLUS)
                        .on_surface(palette.background)
                        .on_activate(move |_, cx| {
                            let _ =
                                weak.update(cx, |editor, cx| editor.add_occurrence(id, false, cx));
                        }),
                )
        });
        div()
            .id(("sequence-available-row", at))
            .debug_selector(move || format!("sequence-available-{at}"))
            .role(Role::ListItem)
            .aria_label(format!("{name}, {}", meta(content, &section.lyrics, id)))
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                    .child(
                        div()
                            .min_w_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.text))
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(scale.apply(tokens::typography::META.size)))
                            .text_color(rgb(palette.text_muted))
                            .child(meta(content, &section.lyrics, id)),
                    ),
            )
            .children(add)
    });
    div()
        .id("sequence-available-list")
        .debug_selector(|| "sequence-available".into())
        .role(Role::List)
        .aria_label(HEAD)
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .size_full()
        .child(crate::songs::lyrics::panes::head(HEAD, environment))
        .children(rows)
        .when(content.sections.is_empty(), |pane| {
            pane.child(
                div()
                    .debug_selector(|| "sequence-no-sections".into())
                    .text_color(rgb(palette.text_muted))
                    .child(NO_SECTIONS),
            )
        })
        .into_any_element()
}
