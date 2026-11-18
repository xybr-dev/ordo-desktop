//! The draft's three read-only panes: the parsed Song and Arrangement details
//! with the possible-existing-Song evidence (left), and the live preview of
//! sections, slide boundaries and the Sequence (right).

use domain::{
    identity::SongId,
    pagination::slides,
    song_library::{
        SongLibrary,
        import::{ImportArrangement, SequenceEntry},
        position_label,
    },
};
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px, rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::Overlay,
    primitives::{
        button::Button,
        disclosure::Disclosure,
        list::{Badge, ListRow},
    },
    tokens,
};

use super::MarkdownDraftView;
use domain::song_library::import::SongChoice;

/// Section headings (Decision 24).
pub(super) const SONG: &str = "Song";
pub(super) const ARRANGEMENT: &str = "Arrangement";
pub(super) const PREVIEW: &str = "Preview";

/// The line under a nested Song preview.
const EXISTING_PREVIEW: &str = "Existing Song \u{b7} preview only";
/// The empty value of a read-only row.
const NONE: &str = "\u{2014}";
/// Between two slides of one section, as the comparison words it.
const SLIDE_BREAK: &str = "slide break";

/// "1 slide" or "N slides".
pub(super) fn slide_count(count: usize) -> String {
    format!("{count} slide{}", if count == 1 { "" } else { "s" })
}

/// "1 possible existing Song" or "N possible existing Songs".
pub(super) fn count_label(count: usize) -> String {
    format!(
        "{count} possible existing Song{}",
        if count == 1 { "" } else { "s" }
    )
}

fn arrangement_count(count: usize) -> String {
    format!("{count} Arrangement{}", if count == 1 { "" } else { "s" })
}

/// A section's preview heading: "{label | Unlabeled} \u{b7} {n} slides".
pub(super) fn section_heading(label: Option<&str>, lyrics: &str) -> String {
    format!(
        "{} \u{b7} {}",
        label.unwrap_or("Unlabeled"),
        slide_count(slides(lyrics).len())
    )
}

/// One Sequence line: "{position} {label}", marked as a repeat from the
/// second time a section is sung, and an unresolved `[Repeat: label]` as
/// "{position} [Repeat: {label}] \u{b7} not found".
pub(super) fn sequence_lines(arrangement: &ImportArrangement) -> Vec<String> {
    let mut sung = vec![false; arrangement.sections.len()];
    arrangement
        .sequence
        .iter()
        .enumerate()
        .map(|(at, entry)| {
            let position = position_label(at);
            match entry {
                SequenceEntry::Section(slot) => {
                    let name = arrangement
                        .sections
                        .get(*slot)
                        .and_then(|section| section.label.as_deref())
                        .unwrap_or("Unlabeled");
                    let again = sung.get(*slot).copied().unwrap_or(false);
                    if let Some(seen) = sung.get_mut(*slot) {
                        *seen = true;
                    }
                    if again {
                        format!("{position} {name} \u{b7} repeat")
                    } else {
                        format!("{position} {name}")
                    }
                }
                SequenceEntry::Unresolved {
                    label, candidates, ..
                } => {
                    if *candidates == 0 {
                        format!("{position} [Repeat: {label}] \u{b7} not found")
                    } else {
                        format!("{position} [Repeat: {label}] \u{b7} {candidates} sections match")
                    }
                }
            }
        })
        .collect()
}

fn muted(text: impl Into<SharedString>, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .text_color(rgb(palette.text_muted))
        .child(text.into())
}

fn heading(text: &'static str, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(palette.text))
        .child(text)
}

/// One read-only labelled value.
fn fact(label: &'static str, value: String, environment: UiEnvironment) -> Div {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let value = if value.trim().is_empty() {
        NONE.to_owned()
    } else {
        value
    };
    div()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .child(muted(label, environment))
        .child(div().text_color(rgb(palette.text)).child(value))
}

/// The left column: Song, Arrangement and identity evidence.
pub(super) fn details(
    view: &MarkdownDraftView,
    environment: UiEnvironment,
    cx: &mut Context<MarkdownDraftView>,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let gap = px(scale.apply(tokens::spacing::GROUP));
    let song = &view.parsed.document.song;
    let Some(arrangement) = view.parsed.document.arrangements.first() else {
        return div().into_any_element();
    };
    let moments = song.liturgical_moments.to_vec().join(", ");
    div()
        .debug_selector(|| "markdown-details".into())
        .flex()
        .flex_col()
        .gap(gap)
        .w_full()
        .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .child(heading(SONG, environment))
        .child(fact("Primary title", song.title.clone(), environment))
        .child(fact("Aliases", song.aliases.join(", "), environment))
        .child(fact(
            "Composer or original author",
            song.original_attribution.clone().unwrap_or_default(),
            environment,
        ))
        .child(fact(
            "Source",
            song.provenance.clone().unwrap_or_default(),
            environment,
        ))
        .child(fact(
            "Rights notes",
            song.rights_notes.clone().unwrap_or_default(),
            environment,
        ))
        .child(fact("Usual moments", moments, environment))
        .child(identity(view, environment, cx))
        .child(heading(ARRANGEMENT, environment))
        .child(fact(
            "Display title",
            arrangement.title.clone().unwrap_or_default(),
            environment,
        ))
        .child(fact(
            "Languages",
            arrangement.languages.join(", "),
            environment,
        ))
        .child(fact("Choirs", arrangement.choirs.join(", "), environment))
        .child(fact(
            "Translator or adapter",
            arrangement
                .rendition_attribution
                .clone()
                .unwrap_or_default(),
            environment,
        ))
        .child(fact(
            "Occasions",
            arrangement.occasions.join(", "),
            environment,
        ))
        .child(fact(
            "Source and rights notes",
            arrangement.notes.clone().unwrap_or_default(),
            environment,
        ))
        .into_any_element()
}

/// The possible-existing-Song count with its rows, and where the draft will
/// be saved. Never opened or chosen automatically.
fn identity(
    view: &MarkdownDraftView,
    environment: UiEnvironment,
    cx: &mut Context<MarkdownDraftView>,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let library = view.store.read(cx).library();
    let weak = cx.weak_entity();
    let rows = view
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(at, possible)| {
            let song = library.song(possible.song).ok()?;
            let arrangements = library
                .arrangements_for_song(possible.song)
                .map_or(0, |all| all.len());
            let detail = match song.original_attribution() {
                Some(composer) => format!("{composer} \u{b7} {}", arrangement_count(arrangements)),
                None => arrangement_count(arrangements),
            };
            let id = possible.song;
            let weak = weak.clone();
            let row = ListRow::new(
                ("markdown-candidate-row", at),
                song.title().to_owned(),
                environment,
            )
            .detail(detail)
            .badges(
                song.is_archived()
                    .then(|| Badge::new("Archived", environment)),
            )
            .on_surface(palette.background)
            .on_activate(move |window, cx| {
                let _ = weak.update(cx, |view, cx| view.preview_song(id, window, cx));
            });
            Some(
                div()
                    .debug_selector(move || format!("markdown-candidate-{at}"))
                    .w_full()
                    .child(row),
            )
        })
        .collect::<Vec<_>>();
    let possible = (!view.candidates.is_empty()).then(|| {
        let list = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .children(rows);
        div().debug_selector(|| "markdown-possible".into()).child(
            Disclosure::new(
                "markdown-possible-disclosure",
                count_label(view.candidates.len()),
                &view.disclosure,
                environment,
                list,
            )
            .on_surface(palette.background),
        )
    });
    let target = match view.target {
        SongChoice::Existing(song) => library.song(song).ok().map(|work| {
            let leave = MarkdownDraftView::act(cx, |view, _, cx| view.use_new_song(cx));
            div()
                .debug_selector(|| "markdown-target".into())
                .flex()
                .flex_col()
                .items_start()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(muted(
                    format!("Saves as an Arrangement of {}.", work.title()),
                    environment,
                ))
                .child(
                    Button::new("markdown-new-song", "Add as a new Song", environment)
                        .quiet()
                        .on_surface(palette.background)
                        .on_activate(leave),
                )
        }),
        _ => None,
    };
    div()
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .children(possible)
        .children(target)
        .into_any_element()
}

/// The nested, identity-only preview of an existing Song, with **Back** and
/// **Use this existing Song**. An archived Song cannot take an Arrangement,
/// so its preview offers Back only.
pub(super) fn overlay(
    view: &MarkdownDraftView,
    song: SongId,
    cx: &mut Context<MarkdownDraftView>,
) -> Option<Overlay> {
    let library = view.store.read(cx).library();
    let work = library.song(song).ok()?;
    let title = work.title().to_owned();
    let archived = work.is_archived();
    let facts = Facts::of(library, song)?;
    let weak = cx.weak_entity();
    let closed = cx.weak_entity();
    let description = if archived {
        format!("{title} is archived. Restore it to attach to it.")
    } else {
        EXISTING_PREVIEW.to_owned()
    };
    let overlay = Overlay::slideover(title)
        .description(description)
        .body(move |environment, _, _| render_facts(&facts, environment))
        .secondary("Back", |_, _| {})
        .on_close(move |_, cx| {
            let _ = closed.update(cx, |view, cx| view.preview_closed(cx));
        });
    Some(if archived {
        overlay
    } else {
        overlay.primary("Use this existing Song", move |_, cx| {
            let _ = weak.update(cx, |view, cx| view.use_existing_song(song, cx));
        })
    })
}

/// The Song text the nested preview shows.
struct Facts {
    composer: String,
    aliases: String,
    moments: String,
    arrangements: Vec<(String, String)>,
}

impl Facts {
    fn of(library: &SongLibrary, song: SongId) -> Option<Self> {
        let work = library.song(song).ok()?;
        let arrangements = library
            .arrangements_for_song(song)
            .unwrap_or_default()
            .into_iter()
            .map(|arrangement| {
                let choirs = arrangement
                    .choirs()
                    .iter()
                    .filter_map(|id| library.choir(*id).ok())
                    .map(|choir| choir.name().to_owned())
                    .collect::<Vec<_>>();
                let choirs = if choirs.is_empty() {
                    "Choir unspecified".to_owned()
                } else {
                    choirs.join(", ")
                };
                let languages = arrangement.languages().join(", ");
                let summary = if languages.is_empty() {
                    choirs
                } else {
                    format!("{languages} \u{b7} {choirs}")
                };
                (
                    library
                        .arrangement_label(arrangement.id())
                        .unwrap_or_default(),
                    summary,
                )
            })
            .collect();
        Some(Self {
            composer: work.original_attribution().unwrap_or_default().to_owned(),
            aliases: work.aliases().join(", "),
            moments: work
                .liturgical_moments()
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            arrangements,
        })
    }
}

fn render_facts(facts: &Facts, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let arrangements = facts
        .arrangements
        .iter()
        .map(|(title, summary)| {
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(title.clone()),
                )
                .child(muted(summary.clone(), environment))
        })
        .collect::<Vec<_>>();
    div()
        .debug_selector(|| "markdown-song-preview".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .w_full()
        .child(fact(
            "Composer / original author",
            facts.composer.clone(),
            environment,
        ))
        .child(fact("Aliases", facts.aliases.clone(), environment))
        .child(fact("Usual moments", facts.moments.clone(), environment))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(muted("Arrangements", environment))
                .children(arrangements),
        )
        .into_any_element()
}

/// The right column: sections with their slides, then the Sequence.
pub(super) fn render(view: &MarkdownDraftView, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let Some(arrangement) = view.parsed.document.arrangements.first() else {
        return div().into_any_element();
    };
    let sections = arrangement
        .sections
        .iter()
        .enumerate()
        .map(|(at, section)| {
            let slide_list = slides(&section.lyrics);
            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)));
            for (index, slide) in slide_list.iter().enumerate() {
                if index > 0 {
                    body = body.child(muted(SLIDE_BREAK, environment));
                }
                body = body.child(
                    div().text_color(rgb(palette.text)).child(
                        section
                            .lyrics
                            .get(slide.range.clone())
                            .unwrap_or_default()
                            .to_owned(),
                    ),
                );
            }
            div()
                .debug_selector(move || format!("markdown-section-{at}"))
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.text))
                        .child(section_heading(section.label.as_deref(), &section.lyrics)),
                )
                .child(body)
        })
        .collect::<Vec<_>>();
    let sequence = sequence_lines(arrangement)
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            let unresolved = matches!(
                arrangement.sequence.get(at),
                Some(SequenceEntry::Unresolved { .. })
            );
            div()
                .debug_selector(move || format!("markdown-sequence-{at}"))
                .text_color(rgb(if unresolved {
                    palette.danger
                } else {
                    palette.text
                }))
                .child(line)
        })
        .collect::<Vec<_>>();
    div()
        .debug_selector(|| "markdown-preview".into())
        .flex()
        .flex_col()
        .gap(px(scale.apply(tokens::spacing::GROUP)))
        .w_full()
        .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
        .child(heading(PREVIEW, environment))
        .when(sections.is_empty(), |pane| {
            pane.child(muted("No sections yet.", environment))
        })
        .children(sections)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .child(muted("Sequence", environment))
                .children(sequence),
        )
        .into_any_element()
}
