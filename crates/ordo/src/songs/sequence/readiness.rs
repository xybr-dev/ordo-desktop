//! The header's readiness label and its reasons (the Sequence tab's marker
//! uses [`EMPTY_SEQUENCE`]).
//!
//! The rules are [`domain::song_library::readiness`]'s; this module only
//! words them (Decision 14) and draws them. The label comes before the save
//! state; its tooltip and accessible description give the reason. There is no
//! click routing from it.

use domain::{
    identity::SectionId,
    song_library::{ArrangementLyrics, ArrangementReadiness, FindingKind, ReadinessState},
};
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px, rgb, svg,
};
use ui::{
    environment::{ResolvedAppearance, UiEnvironment},
    primitives::{record::Marker, tooltip},
    tokens,
};

use crate::songs::lyrics::panes::section_name;

/// The label of an Arrangement with no sections.
pub const NO_SECTIONS: &str = "No sections yet";
/// The label of an Arrangement that cannot be sung yet.
pub const INCOMPLETE: &str = "Incomplete";
/// The label of an Arrangement whose pagination needs a look.
pub const NEEDS_REVIEW: &str = "Needs review";
/// The label of an Arrangement that is ready.
pub const READY: &str = "Ready";
/// The reason for an empty Sequence (also the Sequence tab marker's text).
pub const EMPTY_SEQUENCE: &str = "No performance sequence yet";

/// The information colour of the appearance (DESIGN.md's `info`): what a
/// status that asks for a look, not a fix, is drawn in.
pub fn information(appearance: ResolvedAppearance) -> u32 {
    match appearance {
        ResolvedAppearance::Light => tokens::color::INFORMATION_PREPARATION,
        ResolvedAppearance::Dark => tokens::color::INFORMATION_PRESENTER,
    }
}

/// The header label of a readiness state.
pub fn state_label(state: ReadinessState) -> &'static str {
    match state {
        ReadinessState::NoSections => NO_SECTIONS,
        ReadinessState::Incomplete => INCOMPLETE,
        ReadinessState::NeedsReview => NEEDS_REVIEW,
        ReadinessState::Ready => READY,
    }
}

/// A section's name by identity.
fn name_of(lyrics: &ArrangementLyrics, id: Option<SectionId>) -> String {
    id.and_then(|id| lyrics.sections.iter().position(|section| section.id == id))
        .map(|at| section_name(&lyrics.sections, at))
        .unwrap_or_else(|| "Unnamed section".to_owned())
}

/// The reason a readiness is not Ready, in the operator's words; `None` when
/// there is nothing to say. Findings that share a kind and need the same fix
/// are grouped: every section awaiting review is one sentence.
pub fn reason(lyrics: &ArrangementLyrics, readiness: &ArrangementReadiness) -> Option<String> {
    let mut sentences = Vec::new();
    let mut review: Vec<String> = Vec::new();
    let mut review_at = None;
    for finding in &readiness.findings {
        let name = name_of(lyrics, finding.section);
        match finding.kind {
            FindingKind::EmptySequence => sentences.push(EMPTY_SEQUENCE.to_owned()),
            FindingKind::NoLyrics => sentences.push(format!(
                "{name} has no lyrics and is sung {}",
                if finding.occurrences == 1 {
                    "once".to_owned()
                } else {
                    format!("{} times", finding.occurrences)
                }
            )),
            FindingKind::Unrenderable => {
                sentences.push(format!("{name} can\u{2019}t be shown on the projector"));
            }
            FindingKind::NotApproved(_) => {
                review_at.get_or_insert(sentences.len());
                review.push(name);
            }
        }
    }
    if let Some(at) = review_at {
        let verb = if review.len() == 1 { "needs" } else { "need" };
        sentences.insert(
            at,
            format!("{} {verb} pagination review", review.join(", ")),
        );
    }
    (!sentences.is_empty()).then(|| sentences.join("; "))
}

/// The header's readiness label: plain text for **No sections yet**, otherwise
/// a shaped marker and the state's words, with the reason as tooltip and
/// accessible description.
pub fn header_label(
    lyrics: &ArrangementLyrics,
    readiness: &ArrangementReadiness,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let (color, marker) = match readiness.state {
        ReadinessState::NoSections => (palette.text_muted, None),
        ReadinessState::Incomplete => (palette.warning, Some(Marker::Diamond)),
        ReadinessState::NeedsReview => (information(environment.appearance()), Some(Marker::Dot)),
        ReadinessState::Ready => (palette.live, Some(Marker::Dot)),
    };
    let label = state_label(readiness.state);
    let why = reason(lyrics, readiness);
    let element = div()
        .id("editor-readiness")
        .debug_selector(|| "editor-readiness".into())
        .flex_none()
        .flex()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT + 2.0)))
        .whitespace_nowrap()
        .text_color(rgb(color))
        .text_size(px(scale.apply(if marker.is_some() {
            tokens::typography::LABEL.size
        } else {
            tokens::typography::META.size
        })))
        .children(marker.map(|marker| {
            svg()
                .flex_none()
                .size(px(scale.apply(10.0)))
                .text_color(rgb(color))
                .data(marker.svg().as_bytes())
        }))
        .child(label);
    match why {
        Some(why) => {
            tooltip(element.aria_description(why.clone()), why, environment).into_any_element()
        }
        None => element.into_any_element(),
    }
}
