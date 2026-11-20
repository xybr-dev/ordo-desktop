//! The read-only Arrangement overview that "Open" leads to.
//!
//! T2.6 builds the Arrangement editor; until then this is the real focused
//! view the shell hosts in its focused-editor frame, so opening an Arrangement
//! and returning to the Songs list is a complete round trip. It shows what an
//! Arrangement is: title, Song, languages, Choirs, pagination and archive
//! state, its sections and its Sequence. It changes nothing.

use domain::{
    identity::ArrangementId,
    song_library::{Arrangement, PaginationState, SongLibrary},
};
use gpui::{
    AnyElement, Div, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px, rgb,
};
use ui::{environment::UiEnvironment, tokens};

use super::list;

/// The overview of one Arrangement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArrangementOverview {
    arrangement: ArrangementId,
}

impl ArrangementOverview {
    /// An overview of `arrangement`.
    pub fn new(arrangement: ArrangementId) -> Self {
        Self { arrangement }
    }

    /// The Arrangement shown.
    pub fn arrangement(&self) -> ArrangementId {
        self.arrangement
    }

    /// The title the focused-editor frame shows in its header: the
    /// Arrangement's own title, or a plain fallback when it is gone.
    pub fn title(&self, library: &SongLibrary) -> String {
        library
            .arrangement_label(self.arrangement)
            .unwrap_or_else(|_| "Arrangement".to_owned())
    }

    /// The body: the content of the Arrangement, to sit in the frame's scroll
    /// region.
    pub fn render(&self, library: &SongLibrary, environment: UiEnvironment) -> AnyElement {
        match library.arrangement(self.arrangement) {
            Ok(arrangement) => body(library, arrangement, environment).into_any_element(),
            Err(_) => list::notice(
                "arrangement-missing",
                "This Arrangement is no longer in the library",
                vec!["Go back to the Songs list.".to_owned()],
                Vec::new(),
                environment,
            )
            .into_any_element(),
        }
    }
}

/// How a pagination state reads.
fn pagination_label(state: PaginationState) -> &'static str {
    match state {
        PaginationState::Draft => "Draft",
        PaginationState::Approved => "Approved",
        PaginationState::NeedsReview => "Needs review",
    }
}

fn body(library: &SongLibrary, arrangement: &Arrangement, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let gap = px(scale.apply(tokens::spacing::GROUP));
    let song = library.song(arrangement.song()).ok();
    let song_archived = song.is_some_and(|song| song.is_archived());

    let languages = if arrangement.languages().is_empty() {
        "No language".to_owned()
    } else {
        arrangement.languages().join(", ")
    };
    let choirs: Vec<&str> = arrangement
        .choirs()
        .iter()
        .filter_map(|id| library.choir(*id).ok().map(|choir| choir.name()))
        .collect();
    let choirs = if choirs.is_empty() {
        "No choir".to_owned()
    } else {
        choirs.join(", ")
    };
    let state = if arrangement.is_archived() {
        "Archived"
    } else if song_archived {
        "Available when Song is restored"
    } else {
        "Active"
    };
    let mut facts = vec![
        (
            "Song",
            song.map_or_else(String::new, |song| song.title().to_owned()),
        ),
        ("Languages", languages),
        ("Choirs", choirs),
        (
            "Pagination",
            pagination_label(arrangement.pagination_state()).to_owned(),
        ),
        ("State", state.to_owned()),
    ];
    if let Some(text) = arrangement.rendition_attribution() {
        facts.push(("Attribution", text.to_owned()));
    }
    if !arrangement.occasions().is_empty() {
        let occasions: Vec<&str> = arrangement.occasions().iter().map(String::as_str).collect();
        facts.push(("Occasions", occasions.join(", ")));
    }

    let label_of = |index: usize| {
        arrangement
            .sections()
            .get(index)
            .and_then(|section| section.label().map(str::to_owned))
            .unwrap_or_else(|| format!("Section {}", index + 1))
    };
    let sections = arrangement
        .sections()
        .iter()
        .enumerate()
        .map(|(at, section)| {
            div()
                .debug_selector(move || format!("arrangement-section-{at}"))
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(label_of(at)),
                )
                .children(
                    section
                        .lyrics()
                        .lines()
                        .map(|line| div().text_color(rgb(palette.text)).child(line.to_owned())),
                )
        })
        .collect::<Vec<_>>();
    let sequence = arrangement
        .sequence()
        .iter()
        .enumerate()
        .map(|(at, occurrence)| {
            let index = arrangement
                .sections()
                .iter()
                .position(|section| section.id() == occurrence.section())
                .unwrap_or(0);
            div()
                .debug_selector(move || format!("arrangement-sequence-{at}"))
                .text_color(rgb(palette.text))
                .child(format!("{}. {}", at + 1, label_of(index)))
        })
        .collect::<Vec<_>>();

    let heading = |text: &'static str| {
        div()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(palette.text))
            .child(text)
    };
    div()
        .debug_selector(|| "arrangement-overview".into())
        .flex()
        .flex_col()
        .gap(gap)
        .w_full()
        .p(gap)
        .child(
            div()
                .debug_selector(|| "arrangement-title".into())
                .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text))
                .child(
                    library
                        .arrangement_label(arrangement.id())
                        .unwrap_or_default(),
                ),
        )
        .when(arrangement.is_archived() || song_archived, |view| {
            view.child(
                div()
                    .debug_selector(|| "arrangement-archived-notice".into())
                    .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                    .rounded(px(scale.apply(tokens::radius::CONTROL)))
                    .border(px(tokens::border::SEPARATOR))
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.surface_secondary))
                    .text_color(rgb(palette.text))
                    .child("Archived \u{2014} inspect only until it is restored"),
            )
        })
        .child(
            div()
                .debug_selector(|| "arrangement-facts".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .children(facts.into_iter().map(|(label, value)| {
                    div()
                        .flex()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .child(
                            div()
                                .flex_none()
                                .w(px(scale.apply(140.0)))
                                .text_size(px(scale.apply(tokens::typography::META.size)))
                                .text_color(rgb(palette.text_muted))
                                .child(label),
                        )
                        .child(div().min_w_0().text_color(rgb(palette.text)).child(value))
                })),
        )
        .child(heading("Sections"))
        .child(
            div()
                .debug_selector(|| "arrangement-sections".into())
                .flex()
                .flex_col()
                .gap(gap)
                .children(sections),
        )
        .child(heading("Sequence"))
        .child(
            div()
                .debug_selector(|| "arrangement-sequence".into())
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .children(sequence),
        )
}
