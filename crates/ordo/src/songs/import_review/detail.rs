//! The selected document on the right: its two cards, the evidence, and one
//! block per Arrangement; or, for an Invalid document, why and what to do.

use domain::{
    identity::SongId,
    pagination::slides,
    song_library::{
        SongLibrary,
        import::{Classification, Outcome, Review, SongChoice, SongIdentity, Staged},
    },
};
use gpui::{
    AnyElement, Context, Div, IntoElement, ParentElement, SharedString, Stateful, Styled, Window,
    div, prelude::*, px, rgb,
};
use ui::{
    assets::icon,
    environment::UiEnvironment,
    primitives::{
        Availability,
        button::Button,
        disclosure::Disclosure,
        radio::{RadioGroup, RadioOption},
        selection::{Choice, Select},
    },
    tokens,
};

use super::{
    evidence,
    list::{outcome_label, title_of},
    review::{ImportReview, default_outcome},
};

/// What the Library says about the Song a document will join.
struct Card {
    title: String,
    meta: String,
}

/// Everything one Arrangement block shows, composed before any control is
/// made because the Library borrow and the control states cannot overlap.
struct Block {
    label: String,
    counts: String,
    evidence: String,
    notes: Vec<String>,
    update: String,
    review: domain::song_library::import::ArrangementReview,
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn song_card(library: &SongLibrary, song: SongId) -> Option<Card> {
    let work = library.song(song).ok()?;
    let count = library
        .arrangements_for_song(song)
        .map_or(0, |list| list.len());
    let count = plural(count, "Arrangement", "Arrangements");
    Some(Card {
        title: work.title().to_owned(),
        meta: match work.original_attribution() {
            Some(by) => format!("{by} \u{b7} {count}"),
            None => count,
        },
    })
}

fn block_label(imported: &domain::song_library::import::ImportArrangement) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = &imported.title {
        parts.push(title.clone());
    }
    if !imported.languages.is_empty() {
        parts.push(imported.languages.join(", "));
    }
    if !imported.choirs.is_empty() {
        parts.push(imported.choirs.join(", "));
    }
    if parts.is_empty() {
        "Arrangement".to_owned()
    } else {
        parts.join(" \u{b7} ")
    }
}

fn block_counts(imported: &domain::song_library::import::ImportArrangement) -> String {
    let slides_in = |section: usize| {
        imported
            .sections
            .get(section)
            .map_or(0, |section| slides(&section.lyrics).len())
    };
    let total: usize = imported
        .sequence
        .iter()
        .map(|entry| match entry {
            domain::song_library::import::SequenceEntry::Section(at) => slides_in(*at),
            domain::song_library::import::SequenceEntry::Unresolved { .. } => 0,
        })
        .sum();
    format!(
        "{} \u{b7} {}",
        plural(imported.sections.len(), "section", "sections"),
        plural(total, "slide", "slides")
    )
}

/// The Update description: who will see the new words.
fn update_text(library: &SongLibrary, nearest: Option<domain::identity::ArrangementId>) -> String {
    let names: Vec<String> = nearest
        .and_then(|id| library.arrangement(id).ok())
        .map(|arrangement| {
            arrangement
                .choirs()
                .iter()
                .filter_map(|choir| library.choir(*choir).ok())
                .map(|choir| choir.name().to_owned())
                .collect()
        })
        .unwrap_or_default();
    let who = if names.is_empty() {
        "No Choir".to_owned()
    } else {
        evidence::join_and(&names)
    };
    format!(
        "Replaces the existing Arrangement as a new revision. {who} will see the new words; Mass-local copies don\u{2019}t change."
    )
}

fn label_text(text: &str, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .text_color(rgb(palette.text_muted))
        .child(SharedString::from(text.to_owned()))
}

fn meta(text: impl Into<SharedString>, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .text_color(rgb(palette.text_muted))
        .child(text.into())
}

fn bold(text: impl Into<SharedString>, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    div()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgb(palette.text))
        .child(text.into())
}

fn card(
    selector: &'static str,
    heading: &str,
    title: String,
    meta_line: String,
    environment: UiEnvironment,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(move || selector.into())
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .p(px(scale.apply(tokens::spacing::RELATED)))
        .border(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .rounded(px(tokens::radius::CONTROL))
        .child(label_text(heading, environment))
        .child(bold(title, environment))
        .child(meta(meta_line, environment))
}

/// The detail pane, inside its F6 region.
pub(super) fn render(
    review: &mut ImportReview,
    region: Stateful<Div>,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut Context<ImportReview>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let index = review.selected;
    let body: AnyElement = if review.staging.documents.is_empty() {
        div()
            .debug_selector(|| "import-empty".into())
            .p(px(scale.apply(tokens::spacing::GROUP)))
            .child(meta("No documents to review.", environment))
            .into_any_element()
    } else if review.is_staged_valid(index) {
        valid(review, index, environment, cx)
    } else {
        invalid(review, index, environment, window, cx)
    };
    let heading = review
        .staging
        .documents
        .get(index)
        .map(|document| (document.source.clone(), title_of(review, index)));
    let scroll = review.detail_scroll.clone();
    region
        .size_full()
        .flex()
        .flex_col()
        .children(heading.map(|(file, title)| {
            div()
                .flex_none()
                .flex()
                .flex_col()
                .px(px(scale.apply(tokens::spacing::GROUP)))
                .py(px(scale.apply(tokens::spacing::RELATED)))
                .border_b(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .child(meta(file, environment))
                .child(
                    div()
                        .debug_selector(|| "import-detail-title".into())
                        .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(title),
                )
        }))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    scroll
                        .bounded(div().id("import-detail"))
                        .size_full()
                        .flex()
                        .flex_col()
                        .gap(px(scale.apply(tokens::spacing::GROUP)))
                        .p(px(scale.apply(tokens::spacing::GROUP)))
                        .child(body),
                )
                .children(scroll.indicator("import-detail", 0.0, window, cx, environment)),
        )
        .into_any_element()
}

fn invalid(
    review: &mut ImportReview,
    index: usize,
    environment: UiEnvironment,
    _window: &mut Window,
    cx: &mut Context<ImportReview>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let document = &review.staging.documents[index];
    let reasons = match &document.content {
        Staged::Invalid(reasons) => reasons.clone(),
        Staged::Valid(_) => Vec::new(),
    };
    let reason = evidence::invalid_sentence(&reasons, &document.source);
    let lines: Vec<String> = reasons.iter().map(evidence::technical_line).collect();
    let technical = review.technical.clone();
    let details = div()
        .debug_selector(|| "import-technical".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .children(lines.into_iter().map(|line| meta(line, environment)));
    let remove = Button::new("import-remove", "Remove from review", environment)
        .on_surface(palette.background)
        .on_activate(ImportReview::act(cx, move |review, window, cx| {
            review.remove(index, window, cx);
        }));
    let another = Button::new("import-choose-another", "Choose another file", environment)
        .on_surface(palette.background)
        .on_activate(ImportReview::act(cx, move |review, window, cx| {
            review.choose_another(index, window, cx);
        }));
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .child(
            div()
                .debug_selector(|| "import-invalid".into())
                .w_full()
                .flex()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .p(px(scale.apply(tokens::spacing::RELATED)))
                .border(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.danger))
                .rounded(px(tokens::radius::CONTROL))
                .child(
                    gpui::svg()
                        .path(icon::CIRCLE_X)
                        .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                        .flex_none()
                        .text_color(rgb(palette.danger)),
                )
                .child(
                    div()
                        .text_color(rgb(palette.text))
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("This document can\u{2019}t be imported."),
                        )
                        .child(reason),
                ),
        )
        .child(meta(
            "Invalid documents stay here so you can see why, but they never change the Library. Fix the file and import it again.",
            environment,
        ))
        .child(Disclosure::new(
            "import-technical-details",
            "View technical details",
            &technical,
            environment,
            details,
        ))
        .child(
            div()
                .flex()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(remove)
                .child(another),
        )
        .into_any_element()
}

#[allow(clippy::too_many_lines)] // one pane: the cards, the evidence and the blocks read top to bottom
fn valid(
    review: &mut ImportReview,
    index: usize,
    environment: UiEnvironment,
    cx: &mut Context<ImportReview>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let (Some(Staged::Valid(imported)), Some(Review::Valid(document))) = (
        review
            .staging
            .documents
            .get(index)
            .map(|d| d.content.clone()),
        review.staging.reviews.get(index).cloned(),
    ) else {
        return div().into_any_element();
    };
    let chosen = review.choices.songs.get(&index).copied();
    let store = review.store.clone();
    let library = store.read(cx).library();
    let target: Option<SongId> = match (chosen, &document.identity) {
        (Some(SongChoice::Existing(song)), _) => Some(song),
        (Some(_), _) => None,
        (None, SongIdentity::Existing { song, .. }) => Some(*song),
        (None, _) => None,
    };
    let awaiting =
        chosen.is_none() && matches!(document.identity, SongIdentity::NeedsDecision { .. });
    let skipped = chosen == Some(SongChoice::Skip);
    let target_card = target.and_then(|song| song_card(library, song));
    let library_card = match (&target_card, &document.identity) {
        (Some(card), _) => (card.title.clone(), card.meta.clone()),
        (None, _) if skipped => (
            "Skip document".to_owned(),
            "Leaves the Library as it is.".to_owned(),
        ),
        (None, _) if awaiting => (
            "Which Song is this?".to_owned(),
            "Choose the Song first.".to_owned(),
        ),
        (None, SongIdentity::StagedBy { document: earlier }) => (
            "New Song".to_owned(),
            evidence::staged_by_sentence(
                review
                    .staging
                    .documents
                    .get(*earlier)
                    .map_or("an earlier document", |d| d.source.as_str()),
            ),
        ),
        (None, _) => ("New Song".to_owned(), "No credible match".to_owned()),
    };
    let lead = if skipped || awaiting {
        None
    } else {
        let identity = match (chosen, &document.identity) {
            (Some(SongChoice::Existing(song)), _) => SongIdentity::Existing {
                song,
                evidence: Vec::new(),
            },
            (Some(_), _) => SongIdentity::New,
            (None, identity) => identity.clone(),
        };
        Some(evidence::song_sentence(&identity, &imported, library))
    };
    let blocks: Vec<Block> = document
        .arrangements
        .iter()
        .zip(&imported.arrangements)
        .map(|(arrangement, source)| Block {
            label: block_label(source),
            counts: block_counts(source),
            evidence: evidence::arrangement_sentence(arrangement, library),
            notes: source.warnings.clone(),
            update: update_text(library, arrangement.nearest),
            review: arrangement.clone(),
        })
        .collect();
    let candidates: Vec<(SongId, Card)> = match &document.identity {
        SongIdentity::NeedsDecision {
            candidates,
            archived,
        } => candidates
            .iter()
            .filter(|(song, _)| Some(*song) != *archived)
            .filter_map(|(song, _)| song_card(library, *song).map(|card| (*song, card)))
            .collect(),
        _ => Vec::new(),
    };
    let archived_note = match &document.identity {
        SongIdentity::NeedsDecision {
            archived: Some(song),
            ..
        } => library
            .song(*song)
            .ok()
            .map(|work| format!("{} is archived. Restore it to attach to it.", work.title())),
        _ => None,
    };
    let preview_only = target.is_none();

    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .child(
            div()
                .flex()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(card(
                    "import-card-imported",
                    "Imported document",
                    imported.song.title.clone(),
                    {
                        let count =
                            plural(imported.arrangements.len(), "Arrangement", "Arrangements");
                        match &imported.song.original_attribution {
                            Some(by) => format!("{by} \u{b7} {count}"),
                            None => count,
                        }
                    },
                    environment,
                ))
                .child(card(
                    "import-card-library",
                    "Song in the Library",
                    library_card.0,
                    library_card.1,
                    environment,
                )),
        );
    if let Some((lead, why)) = lead {
        column = column.child(
            div()
                .debug_selector(|| "import-song-evidence".into())
                .flex()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .items_start()
                .child(
                    gpui::svg()
                        .path(icon::SEARCH_CHECK)
                        .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                        .flex_none()
                        .text_color(rgb(palette.text_muted)),
                )
                .child(
                    div()
                        .text_color(rgb(palette.text))
                        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(lead))
                        .child(why),
                ),
        );
    }
    if matches!(document.identity, SongIdentity::NeedsDecision { .. }) {
        column = column.child(identity_block(
            review,
            index,
            &candidates,
            archived_note,
            environment,
            cx,
        ));
    }
    for (at, block) in blocks.into_iter().enumerate() {
        column = column.child(arrangement_block(
            review,
            index,
            at,
            block,
            awaiting,
            skipped,
            preview_only,
            environment,
            cx,
        ));
    }
    let notes = imported.warnings.clone();
    if !notes.is_empty() {
        column = column.child(notes_block(&notes, environment));
    }
    column.into_any_element()
}

fn notes_block(notes: &[String], environment: UiEnvironment) -> Div {
    let scale = environment.preferences().scale;
    div()
        .debug_selector(|| "import-notes".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .child(label_text("Notes from the file", environment))
        .children(notes.iter().map(|note| meta(note.clone(), environment)))
}

/// **Which Song is this?**: the document's Song, decided once.
fn identity_block(
    review: &mut ImportReview,
    index: usize,
    candidates: &[(SongId, Card)],
    archived_note: Option<String>,
    environment: UiEnvironment,
    cx: &mut Context<ImportReview>,
) -> Div {
    let scale = environment.preferences().scale;
    let ids: Vec<SongId> = candidates.iter().map(|(song, _)| *song).collect();
    let mut options: Vec<RadioOption> = candidates
        .iter()
        .map(|(_, card)| RadioOption {
            label: card.title.clone().into(),
            description: Some(card.meta.clone().into()),
            badge: None,
        })
        .collect();
    for label in ["Add as a new Song", "Skip document"] {
        options.push(RadioOption {
            label: label.into(),
            description: None,
            badge: None,
        });
    }
    let selected = match review.choices.songs.get(&index) {
        Some(SongChoice::Existing(song)) => ids.iter().position(|id| id == song),
        Some(SongChoice::New) => Some(ids.len()),
        Some(SongChoice::Skip) => Some(ids.len() + 1),
        None => None,
    };
    let labels: Vec<Choice> = options
        .iter()
        .map(|option| Choice::new(option.label.clone()))
        .collect();
    let state = review.control((index, None), || (labels, selected), cx);
    let weak = cx.weak_entity();
    let group = RadioGroup::new(
        ("import-song-choice", index),
        "Which Song is this?",
        options,
        &state,
        environment,
    )
    .on_select(move |at, window, cx| {
        let _ = weak.update(cx, |review, cx| {
            if let Some(choice) = review.song_choice_at(index, &ids, at) {
                review.choose_song(index, choice, window, cx);
            }
        });
    });
    div()
        .debug_selector(|| "import-identity".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(bold("Which Song is this?", environment))
        .children(archived_note.map(|note| meta(note, environment)))
        .child(group)
}

#[allow(clippy::too_many_arguments)] // one block of the pane, built from the pane's facts
fn arrangement_block(
    review: &mut ImportReview,
    index: usize,
    at: usize,
    block: Block,
    awaiting: bool,
    skipped: bool,
    preview_only: bool,
    environment: UiEnvironment,
    cx: &mut Context<ImportReview>,
) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let classification = block.review.classification;
    let offered = block.review.offered.clone();
    let recommended = block.review.recommended;
    let current: Option<Outcome> = review
        .choices
        .arrangements
        .get(&(index, at))
        .copied()
        .or_else(|| default_outcome(classification));
    let position = |outcome: Option<Outcome>| {
        outcome.and_then(|outcome| offered.iter().position(|offered| *offered == outcome))
    };
    let control: Option<AnyElement> = if awaiting {
        Some(meta("Choose the Song first.", environment).into_any_element())
    } else if skipped || offered.is_empty() {
        None
    } else if classification == Classification::NeedsDecision {
        let options: Vec<RadioOption> = offered
            .iter()
            .map(|outcome| RadioOption {
                label: outcome_label(*outcome, classification).into(),
                description: Some(
                    match outcome {
                        Outcome::AddAsNew => "Keeps both renditions side by side.".to_owned(),
                        Outcome::UpdateExisting => block.update.clone(),
                        _ => "Leaves the Library as it is.".to_owned(),
                    }
                    .into(),
                ),
                badge: (recommended == Some(*outcome)).then(|| "Recommended".into()),
            })
            .collect();
        let labels: Vec<Choice> = options
            .iter()
            .map(|option| Choice::new(option.label.clone()))
            .collect();
        let made = position(review_choice(review, index, at));
        let state = review.control((index, Some(at)), || (labels, made), cx);
        let weak = cx.weak_entity();
        Some(
            RadioGroup::new(
                ("import-outcome", index * 1000 + at),
                "Outcome",
                options,
                &state,
                environment,
            )
            .on_select(move |choice, _, cx| {
                let _ = weak.update(cx, |review, cx| {
                    review.choose_outcome(index, at, choice, cx);
                });
            })
            .into_any_element(),
        )
    } else {
        let labels: Vec<Choice> = offered
            .iter()
            .map(|outcome| Choice::new(outcome_label(*outcome, classification)))
            .collect();
        let state = review.control((index, Some(at)), || (labels, position(current)), cx);
        let weak = cx.weak_entity();
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .child(label_text("Staged action", environment))
                .child(
                    Select::new(
                        ("import-staged-action", index * 1000 + at),
                        "Staged action",
                        &state,
                        environment,
                    )
                    .on_select(move |choice, _, cx| {
                        if let Some(choice) = choice {
                            let _ = weak.update(cx, |review, cx| {
                                review.choose_outcome(index, at, choice, cx);
                            });
                        }
                    }),
                )
                .into_any_element(),
        )
    };
    let compare = Button::new(
        ("import-compare", index * 1000 + at),
        if preview_only {
            "Preview document"
        } else {
            "Compare contents"
        },
        environment,
    )
    .icon(icon::COLUMNS_2)
    .on_surface(palette.background)
    .availability(if awaiting {
        Availability::Disabled
    } else {
        Availability::Available
    });
    div()
        .debug_selector(move || format!("import-arrangement-{at}"))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .child(
            div()
                .w_full()
                .flex()
                .items_baseline()
                .justify_between()
                .child(bold(block.label, environment))
                .child(meta(block.counts, environment)),
        )
        .child(meta(block.evidence, environment))
        .children(control)
        .child(compare)
        .children((!block.notes.is_empty()).then(|| notes_block(&block.notes, environment)))
}

/// The Outcome the operator chose for an Arrangement, if any.
fn review_choice(review: &ImportReview, document: usize, at: usize) -> Option<Outcome> {
    review.choices.arrangements.get(&(document, at)).copied()
}
