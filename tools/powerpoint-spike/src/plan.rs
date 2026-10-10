//! Turns a [`DeckPlan`] into the ordered slide list both the writer and the
//! validator agree on.

use crate::{Cue, DeckPlan};

pub(crate) const SAFETY: &str = "Safety";
pub(crate) const LEAD_IN_NOTES: &str = "Safety lead-in: black slide before the first Mass item.";
pub(crate) const CLOSING_NOTES: &str = "Safety closing: black slide after the final Mass item.";

/// What a slide shows.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Kind<'a> {
    /// Full-bleed PNG bytes.
    Image(&'a [u8]),
    /// Black background, no shapes; the notes text.
    Black(&'a str),
}

#[derive(Debug, Clone)]
pub(crate) struct Planned<'a> {
    /// Human-readable location used in validation messages.
    pub label: String,
    pub kind: Kind<'a>,
}

/// A section name and the zero-based indices of its slides.
pub(crate) type Section = (String, Vec<usize>);

/// Lists the slides in deck order and their sections.
///
/// Slide 1 is the Safety lead-in and the last slide is the Safety closing;
/// both sit in sections named `Safety`. Each Mass item gets one section named
/// after its title.
pub(crate) fn plan_slides(plan: &DeckPlan) -> (Vec<Planned<'_>>, Vec<Section>) {
    let mut slides = vec![Planned {
        label: "safety lead-in".to_owned(),
        kind: Kind::Black(LEAD_IN_NOTES),
    }];
    let mut sections = vec![(SAFETY.to_owned(), vec![0])];
    for (item_index, item) in plan.items.iter().enumerate() {
        let mut indices = Vec::with_capacity(item.cues.len());
        for (cue_index, cue) in item.cues.iter().enumerate() {
            let kind = match cue {
                Cue::Image { png } => Kind::Image(png.as_slice()),
                Cue::OperatorOnly { notes } => Kind::Black(notes.as_str()),
            };
            indices.push(slides.len());
            slides.push(Planned {
                label: format!("item {item_index} '{}' cue {cue_index}", item.title),
                kind,
            });
        }
        sections.push((item.title.clone(), indices));
    }
    let closing = slides.len();
    slides.push(Planned {
        label: "safety closing".to_owned(),
        kind: Kind::Black(CLOSING_NOTES),
    });
    sections.push((SAFETY.to_owned(), vec![closing]));
    (slides, sections)
}
