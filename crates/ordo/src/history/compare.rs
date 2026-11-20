//! The Compare revision surface: two read-only columns, **Current** and
//! **Revision to restore**, with what differs marked. It holds no field and no
//! edit control; the footer (built by the opener) owns the one restore action.

use std::collections::HashMap;

use domain::{
    identity::ChoirId,
    song_library::{
        ArrangementComparison, ArrangementDetails, ArrangementLyrics, ArrangementState, LineMark,
        SongDraft, SongField, compare_arrangements, position_label, song_changed_fields,
    },
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
    rgb,
};
use ui::{
    environment::UiEnvironment,
    overlay::mix,
    primitives::selection::{Choice, SelectionState, Tabs},
    tokens,
};

use super::slides::{SlideFrames, SungSlide, sung_slides};
use crate::songs::lyrics::navigator::pagination_label;

/// How much of the danger or live colour tints a column's heading.
const COLUMN_TINT: f32 = 0.14;

/// The tile width of the Slides tab, before the Interface scale.
const TILE_WIDTH: f32 = 150.0;

/// One side of an Arrangement comparison.
#[derive(Clone)]
pub struct ArrangementSide {
    /// Its details.
    pub details: ArrangementDetails,
    /// Its sections and Sequence.
    pub lyrics: ArrangementLyrics,
    /// Whether its section states were recorded (false for a legacy revision).
    pub recorded: bool,
}

impl ArrangementSide {
    fn state(&self) -> ArrangementState<'_> {
        ArrangementState {
            details: &self.details,
            lyrics: &self.lyrics,
            states_recorded: self.recorded,
        }
    }
}

/// What is compared.
#[derive(Clone)]
pub enum Sides {
    /// Two states of one Arrangement.
    Arrangement {
        /// The Arrangement in memory.
        current: ArrangementSide,
        /// The revision to restore.
        revision: ArrangementSide,
    },
    /// Two states of one Song.
    Song {
        /// The Song in memory.
        current: SongDraft,
        /// The revision to restore.
        revision: SongDraft,
    },
}

/// The tabs of an Arrangement comparison.
const TABS: [&str; 4] = ["Overview", "Lyrics", "Sequence", "Slides"];

/// The headings of the two columns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnTitles {
    /// The left column: what is in hand (the History head, or the import).
    pub left: String,
    /// The right column: what it is compared with.
    pub right: String,
}

impl ColumnTitles {
    /// History's headings: **Current** and **Revision to restore**.
    pub fn history() -> Self {
        Self {
            left: "Current".to_owned(),
            right: "Revision to restore".to_owned(),
        }
    }
}

/// What the right column says when nothing is there to compare with.
const NO_MATCH: &str = "No match.";

/// The comparison body.
pub struct CompareView {
    sides: Sides,
    choirs: HashMap<ChoirId, String>,
    titles: ColumnTitles,
    /// Set when the right column has nothing to show (an import with no
    /// local candidate); the left column is then not marked.
    no_match: bool,
    current_time: String,
    revision_time: String,
    tabs: SelectionState,
    frames: Entity<SlideFrames>,
    /// What each side's sections, Sequence and Overview differ in, read once
    /// when the comparison opens (the stored revision and the head never
    /// change under it), `(current, revision)`.
    marks: Option<(ArrangementComparison, ArrangementComparison)>,
    /// Each side's sung slides, `(current, revision)`.
    slides: Option<(Vec<SungSlide>, Vec<SungSlide>)>,
    /// A Song's differing fields.
    song_fields: Vec<SongField>,
}

impl CompareView {
    /// Builds the view and asks for the Slides frames of both sides.
    pub fn new(
        sides: Sides,
        choirs: HashMap<ChoirId, String>,
        titles: ColumnTitles,
        times: (String, String),
        frames: Entity<SlideFrames>,
        cx: &mut App,
    ) -> Entity<Self> {
        let choices = match sides {
            Sides::Arrangement { .. } => TABS.iter().map(|tab| Choice::new(*tab)).collect(),
            Sides::Song { .. } => vec![Choice::new("Overview")],
        };
        // An Arrangement's comparison opens on Lyrics, the tab the operator
        // reads a revision by; a Song has only its Overview.
        let first = match sides {
            Sides::Arrangement { .. } => 1,
            Sides::Song { .. } => 0,
        };
        let tabs = SelectionState::new(choices, Some(first), cx);
        let (mut marks, mut slides, mut song_fields) = (None, None, Vec::new());
        match &sides {
            Sides::Arrangement { current, revision } => {
                let (mine, theirs) = (sung_slides(&current.lyrics), sung_slides(&revision.lyrics));
                let wanted: Vec<_> = mine
                    .iter()
                    .chain(&theirs)
                    .map(|slide| slide.key.clone())
                    .collect();
                frames.update(cx, |frames, cx| frames.request(wanted, cx));
                marks = Some((
                    compare_arrangements(&current.state(), &revision.state()),
                    compare_arrangements(&revision.state(), &current.state()),
                ));
                slides = Some((mine, theirs));
            }
            Sides::Song { current, revision } => {
                song_fields = song_changed_fields(revision, current);
            }
        }
        cx.new(|_| Self {
            marks,
            slides,
            song_fields,
            sides,
            choirs,
            titles,
            no_match: false,
            current_time: times.0,
            revision_time: times.1,
            tabs,
            frames,
        })
    }

    /// Swaps the right column for another Arrangement (or, with `None`, for
    /// "No match."), and the left column for `left` (an import's section ids
    /// are aligned to the right column's), re-marks both against each other
    /// and asks for their frames. The tab and the frames cache stay.
    pub fn replace_sides(
        &mut self,
        left: ArrangementSide,
        right: Option<ArrangementSide>,
        title: String,
        cx: &mut Context<Self>,
    ) {
        let Sides::Arrangement { current, revision } = &mut self.sides else {
            return;
        };
        *current = left;
        self.no_match = right.is_none();
        *revision = right.unwrap_or_else(|| ArrangementSide {
            details: ArrangementDetails::default(),
            lyrics: ArrangementLyrics {
                sections: Vec::new(),
                sequence: Vec::new(),
            },
            recorded: true,
        });
        self.titles.right = title;
        let (mine, theirs) = (sung_slides(&current.lyrics), sung_slides(&revision.lyrics));
        let wanted: Vec<_> = mine
            .iter()
            .chain(&theirs)
            .map(|slide| slide.key.clone())
            .collect();
        self.frames
            .update(cx, |frames, cx| frames.request(wanted, cx));
        self.marks = Some(if self.no_match {
            (
                compare_arrangements(&current.state(), &current.state()),
                compare_arrangements(&revision.state(), &revision.state()),
            )
        } else {
            (
                compare_arrangements(&current.state(), &revision.state()),
                compare_arrangements(&revision.state(), &current.state()),
            )
        });
        self.slides = Some((mine, theirs));
        cx.notify();
    }

    /// Replaces the Choir names the columns resolve ids against.
    pub fn set_choirs(&mut self, choirs: HashMap<ChoirId, String>) {
        self.choirs = choirs;
    }

    /// The two column headings as they stand.
    pub fn titles(&self) -> &ColumnTitles {
        &self.titles
    }

    /// Whether the right column is showing "No match.".
    pub fn no_match(&self) -> bool {
        self.no_match
    }

    /// The Slides frames this comparison asked for.
    pub fn frames(&self) -> &Entity<SlideFrames> {
        &self.frames
    }

    /// The tab showing, by name.
    pub fn tab(&self, cx: &App) -> &'static str {
        match self.sides {
            Sides::Song { .. } => TABS[0],
            Sides::Arrangement { .. } => TABS[self.tabs.selected(cx).unwrap_or(0).min(3)],
        }
    }

    /// The Lyrics headings of both columns, `(current, revision)`; empty for a
    /// Song.
    pub fn lyrics_heads(&self) -> (Vec<String>, Vec<String>) {
        let (Sides::Arrangement { current, revision }, Some((mine, theirs))) =
            (&self.sides, &self.marks)
        else {
            return (Vec::new(), Vec::new());
        };
        let heads = |data: &ArrangementSide, marks: &ArrangementComparison| {
            data.lyrics
                .sections
                .iter()
                .zip(&marks.sections)
                .map(|(section, mark)| {
                    section_head(
                        section
                            .label
                            .clone()
                            .unwrap_or_else(|| "Unnamed section".to_owned()),
                        mark.state,
                    )
                })
                .collect()
        };
        (heads(current, mine), heads(revision, theirs))
    }

    /// Draws the tabs (Arrangement only) and the two columns.
    pub fn render_body(
        &mut self,
        environment: UiEnvironment,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let gap = px(scale.apply(tokens::spacing::RELATED));
        let tab = self.tab(cx);
        let columns = match &self.sides {
            Sides::Song { current, revision } => {
                let fields = &self.song_fields;
                vec![
                    self.column(
                        "current",
                        &self.titles.left,
                        &self.current_time,
                        song_rows("current", current, fields, environment),
                        environment,
                    ),
                    self.column(
                        "revision",
                        &self.titles.right,
                        &self.revision_time,
                        song_rows("revision", revision, fields, environment),
                        environment,
                    ),
                ]
            }
            Sides::Arrangement { current, revision } => {
                let (Some((mine, theirs)), Some((mine_slides, their_slides))) =
                    (&self.marks, &self.slides)
                else {
                    return div().into_any_element();
                };
                vec![
                    self.column(
                        "current",
                        &self.titles.left,
                        &self.current_time,
                        self.tab_rows(tab, "current", current, mine, mine_slides, environment, cx),
                        environment,
                    ),
                    self.column(
                        "revision",
                        &self.titles.right,
                        &self.revision_time,
                        if self.no_match {
                            vec![
                                div()
                                    .text_size(px(scale.apply(tokens::typography::META.size)))
                                    .text_color(rgb(palette.text_muted))
                                    .child(NO_MATCH)
                                    .into_any_element(),
                            ]
                        } else {
                            self.tab_rows(
                                tab,
                                "revision",
                                revision,
                                theirs,
                                their_slides,
                                environment,
                                cx,
                            )
                        },
                        environment,
                    ),
                ]
            }
        };
        let tabs = matches!(self.sides, Sides::Arrangement { .. }).then(|| {
            Tabs::new(
                "compare-tabs",
                "Compare revision views",
                &self.tabs,
                environment,
            )
            .on_surface(palette.surface)
        });
        div()
            .debug_selector(|| "compare-body".into())
            .flex()
            .flex_col()
            .gap(gap)
            .children(tabs)
            .child(div().flex().items_start().gap(gap).children(columns))
            .into_any_element()
    }

    fn column(
        &self,
        side: &'static str,
        label: &str,
        time: &str,
        body: Vec<AnyElement>,
        environment: UiEnvironment,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        // The mockup tints the Current column toward danger (what a restore
        // would replace) and the revision toward live (what it would bring).
        let tint = mix(
            if side == "current" {
                palette.danger
            } else {
                palette.live
            },
            palette.surface,
            COLUMN_TINT,
        );
        div()
            .debug_selector(move || format!("compare-{side}"))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .p(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border_1()
            .border_color(rgb(palette.border))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                    .rounded(px(scale.apply(tokens::radius::CONTROL)))
                    .bg(rgb(tint))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(label.to_owned()),
                    )
                    .children((!time.is_empty()).then(|| {
                        div()
                            .text_size(px(scale.apply(tokens::typography::META.size)))
                            .text_color(rgb(palette.text_muted))
                            .child(time.to_owned())
                    })),
            )
            .children(body)
            .into_any_element()
    }

    fn choir_names(&self, details: &ArrangementDetails) -> String {
        let names: Vec<&str> = details
            .choirs
            .iter()
            .filter_map(|id| self.choirs.get(id).map(String::as_str))
            .collect();
        if names.is_empty() {
            "\u{2014}".to_owned()
        } else {
            names.join(", ")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tab_rows(
        &self,
        tab: &str,
        side: &'static str,
        data: &ArrangementSide,
        marks: &ArrangementComparison,
        slides: &[SungSlide],
        environment: UiEnvironment,
        cx: &App,
    ) -> Vec<AnyElement> {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        match tab {
            "Lyrics" => data
                .lyrics
                .sections
                .iter()
                .zip(&marks.sections)
                .enumerate()
                .map(|(at, (section, mark))| {
                    let name = section
                        .label
                        .clone()
                        .unwrap_or_else(|| "Unnamed section".to_owned());
                    let head = section_head(name, mark.state);
                    let lines = section.lyrics.lines().zip(&mark.lines).enumerate().map(
                        |(n, (line, mark))| {
                            if line.trim().is_empty() {
                                let marked = *mark == LineMark::SlideBreak;
                                marked_line(
                                    format!(
                                        "compare-{side}-break-{at}-{n}{}",
                                        if marked { "-changed" } else { "" }
                                    ),
                                    "slide break".to_owned(),
                                    marked,
                                    true,
                                    environment,
                                )
                            } else {
                                let marked = *mark == LineMark::Changed;
                                marked_line(
                                    format!(
                                        "compare-{side}-line-{at}-{n}{}",
                                        if marked { "-changed" } else { "" }
                                    ),
                                    line.to_owned(),
                                    marked,
                                    false,
                                    environment,
                                )
                            }
                        },
                    );
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                        .child(
                            div()
                                .text_size(px(scale.apply(tokens::typography::META.size)))
                                .text_color(rgb(palette.text_muted))
                                .child(head),
                        )
                        .children(lines)
                        .into_any_element()
                })
                .collect(),
            "Sequence" => data
                .lyrics
                .sequence
                .iter()
                .enumerate()
                .map(|(at, occurrence)| {
                    let name = data
                        .lyrics
                        .sections
                        .iter()
                        .find(|section| section.id == occurrence.section)
                        .and_then(|section| section.label.clone())
                        .unwrap_or_else(|| "Unnamed section".to_owned());
                    let marked = marks.sequence.get(at).copied().unwrap_or(false);
                    marked_line(
                        format!(
                            "compare-{side}-seq-{at}{}",
                            if marked { "-changed" } else { "" }
                        ),
                        format!("{} {name}", position_label(at)),
                        marked,
                        false,
                        environment,
                    )
                })
                .collect(),
            "Slides" => {
                let frames = self.frames.read(cx);
                let tiles = slides.iter().enumerate().map(|(at, slide)| {
                    div()
                        .debug_selector(move || format!("compare-{side}-slide-{at}"))
                        .flex()
                        .flex_col()
                        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                        .child(frames.picture(&slide.key, scale.apply(TILE_WIDTH), environment))
                        .child(
                            div()
                                .text_size(px(scale.apply(tokens::typography::META.size)))
                                .text_color(rgb(palette.text_muted))
                                .child(format!(
                                    "{} {}",
                                    position_label(slide.position),
                                    slide.name
                                )),
                        )
                });
                vec![
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(scale.apply(tokens::spacing::RELATED)))
                        .children(tiles)
                        .into_any_element(),
                ]
            }
            _ => {
                let slide_count = slides.len();
                let mut rows = vec![
                    (
                        "Languages",
                        data.details.languages.join(", "),
                        marks.overview.languages,
                    ),
                    (
                        "Choirs",
                        self.choir_names(&data.details),
                        marks.overview.choirs,
                    ),
                    (
                        "Sections",
                        data.lyrics.sections.len().to_string(),
                        marks.overview.sections,
                    ),
                    (
                        "Sequence",
                        format!(
                            "{} occurrences \u{b7} {slide_count} slides",
                            data.lyrics.sequence.len()
                        ),
                        marks.overview.sequence,
                    ),
                ];
                let dash =
                    |text: Option<&String>| text.cloned().unwrap_or_else(|| "\u{2014}".to_owned());
                rows.push((
                    "Title",
                    dash(data.details.title.as_ref()),
                    marks.overview.title,
                ));
                rows.push((
                    "Rendition attribution",
                    dash(data.details.rendition_attribution.as_ref()),
                    marks.overview.attribution,
                ));
                rows.push((
                    "Occasions",
                    if data.details.occasions.is_empty() {
                        "\u{2014}".to_owned()
                    } else {
                        data.details.occasions.join(", ")
                    },
                    marks.overview.occasions,
                ));
                rows.push((
                    "Notes",
                    dash(data.details.notes.as_ref()),
                    marks.overview.notes,
                ));
                rows.into_iter()
                    .map(|(label, value, marked)| {
                        overview_row(side, label, value, marked, environment)
                    })
                    .collect()
            }
        }
    }
}

/// A section's heading: its name, then its pagination label when its state
/// is known.
fn section_head(name: String, state: Option<domain::song_library::PaginationState>) -> String {
    match state {
        Some(state) => format!("{name} \u{b7} {}", pagination_label(state)),
        None => name,
    }
}

/// The Song Overview's rows for one side; a field that differs is marked.
fn song_rows(
    side: &'static str,
    draft: &SongDraft,
    fields: &[SongField],
    environment: UiEnvironment,
) -> Vec<AnyElement> {
    let dash = |text: Option<&String>| text.cloned().unwrap_or_else(|| "\u{2014}".to_owned());
    let list = |items: &[String]| {
        if items.is_empty() {
            "\u{2014}".to_owned()
        } else {
            items.join(", ")
        }
    };
    let rows = [
        ("Title", draft.title.clone(), SongField::Title),
        ("Also known as", list(&draft.aliases), SongField::Aliases),
        (
            "Attribution",
            dash(draft.original_attribution.as_ref()),
            SongField::Attribution,
        ),
        (
            "Provenance",
            dash(draft.provenance.as_ref()),
            SongField::Provenance,
        ),
        (
            "Rights notes",
            dash(draft.rights_notes.as_ref()),
            SongField::RightsNotes,
        ),
        (
            "Liturgical moments",
            list(&draft.liturgical_moments),
            SongField::Moments,
        ),
        ("Occasions", list(&draft.occasions), SongField::Occasions),
    ];
    rows.into_iter()
        .map(|(label, value, field)| {
            overview_row(side, label, value, fields.contains(&field), environment)
        })
        .collect()
}

fn overview_row(
    side: &'static str,
    label: &'static str,
    value: String,
    marked: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let selector = format!(
        "compare-{side}-{label}{}",
        if marked { "-changed" } else { "" }
    );
    mark_frame(
        selector.clone(),
        format!("{label}: {value}"),
        marked,
        environment,
    )
    .flex()
    .flex_col()
    .child(
        div()
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.text_muted))
            .child(label),
    )
    .child(div().child(value))
    .into_any_element()
}

/// One line or gap, marked when it differs.
fn marked_line(
    selector: String,
    text: String,
    marked: bool,
    gap: bool,
    environment: UiEnvironment,
) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let frame = mark_frame(selector, text.clone(), marked, environment);
    if gap {
        frame
            .text_size(px(scale.apply(tokens::typography::META.size)))
            .text_color(rgb(palette.text_muted))
            .child(text)
            .into_any_element()
    } else {
        frame.child(text).into_any_element()
    }
}

/// The frame of a comparison cell: a changed one has a left bar and a tint,
/// and is named "Changed: ..." so colour is never the only sign.
fn mark_frame(
    selector: String,
    name: String,
    marked: bool,
    environment: UiEnvironment,
) -> gpui::Stateful<gpui::Div> {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let label = if marked {
        format!("Changed: {name}")
    } else {
        name
    };
    let id = SharedString::from(selector.clone());
    div()
        .id(id)
        .debug_selector(move || selector.clone())
        .aria_label(label)
        .w_full()
        .min_w_0()
        .px(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .border_l(px(if marked { 3.0 } else { 0.0 }))
        .border_color(rgb(palette.warning))
        .when(marked, |cell| cell.bg(rgb(palette.surface_secondary)))
}
