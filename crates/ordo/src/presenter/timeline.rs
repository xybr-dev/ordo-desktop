//! The grouped Cue timeline (T1.4 WP6, Decisions 295 and 303).
//!
//! One group per Mass item, in snapshot order, inside the one scrolling
//! surface of the Presenter (Decision 237: only the timeline scrolls). A
//! group's header folds its Cues away without touching the Engine; folding is
//! view state and a folded Cue stays reachable from the rail and the keyboard.
//!
//! # Grid and List
//!
//! In Grid, a Project (or Unavailable) Cue is a fixed-width tile and every
//! other Cue is a full-width compact row; in List every Cue is a row
//! (`timeline-modes-v2`). The tile width is the layout model's `cell_width`
//! and is never stretched: a group whose last row is not full, or that has one
//! Cue, keeps the width of a full row, so the 270 px floor of
//! `presenter_layout` holds for every tile.
//!
//! A row leads with what the Cue is: a small canonical thumbnail for a slide
//! (List only), or the operator-only marker of a trigger, Wait, Hold or
//! Unavailable Cue, so the kind is a shape as well as a word.
//!
//! # What a tile shows
//!
//! The thumbnail is the same retained element the monitors draw
//! ([`live_viewport`], Decision 310), laid out at `cell_width` (or
//! [`MINI_WIDTH`] in a row); nothing is rasterized and a resize never
//! recomposes. The frame store composes the groups on screen and their
//! neighbours at thumbnail quality (`request_visible`), so a tile is black
//! only until its frame is ready. An Unavailable Cue is black with the
//! operator-only "Output unavailable" placeholder and the caption
//! "Unavailable · reason".
//!
//! # Marks (Decision 295) and one-line captions (Decision 303)
//!
//! The Cue on Program carries a ring (a leading bar on a row), green when Live
//! and amber when Frozen, and the caption word "Program" (Live) or "Held"
//! (Frozen). The Cue on Preview carries a neutral bar below the frame and the
//! word "Preview". The bar's space is always reserved and every caption is one
//! line whose label ellipsizes before the number or an indicator shrinks, so a
//! tile's height never depends on which indicators it shows.
//!
//! Nothing here reads the Engine: the owning view builds the models below and
//! a click reports the Cue's flat index through one callback, which the view
//! sends as `Command::GoTo`. When Preview moves by key, the timeline glides to
//! put its Cue on the middle of the timeline, together with Program's while
//! Freeze holds a different Cue and both fit ([`follow`]); it never scrolls
//! otherwise.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Bounds, FocusHandle, FontWeight, IntoElement, ParentElement, Pixels,
    RenderImage, Role, ScrollHandle, SharedString, Styled, Window, canvas, div, point, prelude::*,
    px, rgb, svg,
};
use presentation::state::{EngineState, LiveState, ProgramContent};
use rendering::Frame;
use ui::{
    assets::icon,
    environment::{CueView, UiEnvironment},
    motion,
    primitives::{self, button::Button, record::Marker},
    scroll::Scroll,
    tokens,
};

use super::{
    dock,
    layout::{CELL_GAP, CELL_PADDING},
};
use crate::canonical_views::live_viewport;

/// Called with a Cue's flat index.
pub type OnCue = Rc<dyn Fn(usize, &mut Window, &mut App)>;
/// Called with a group's index.
pub type OnGroup = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Height of a tile caption (one line of META text and its inset).
pub const CAPTION_HEIGHT: f32 = 24.0;
/// Height of the Preview bar (Decision 295), and of the space reserved for it
/// in every tile.
pub const PREVIEW_BAR: f32 = 3.0;
/// Thickness of the Program ring and of a row's leading bar.
pub const PROGRAM_RING: f32 = 2.0;
/// Indent of a group's Cues under its header (kit `.pr-cues`).
pub const CUE_INDENT: f32 = 16.0;
/// Height of a group header.
pub const GROUP_HEAD_HEIGHT: f32 = 40.0;
/// Width of a row's small canonical thumbnail (16:9, so 27 px tall inside the
/// 32 px compact row).
pub const MINI_WIDTH: f32 = 48.0;
/// Size of a row's lead marker glyph.
const LEAD_ICON: f32 = 16.0;
/// Width of a row's Cue number column.
const NUMBER_WIDTH: f32 = 32.0;

/// Whether a Cue is on Program, and what Program is doing with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Air {
    /// Live: the congregation sees this Cue's frame.
    Program,
    /// Frozen: the congregation still sees this Cue's frame, held.
    Held,
}

/// The Cue (if any) the congregation is looking at, by flat index.
///
/// Only a Cue that put a frame on Program is on air. While Program is black
/// (output off, Blackout, a lost display, or a Hold or trigger Cue taken
/// live) no Cue carries the Program mark, because nothing it owns is shown.
pub fn air(state: &EngineState, flat: usize) -> Option<Air> {
    match (state.program(), state.live()) {
        (ProgramContent::Frame { cue, .. }, LiveState::Live) if cue == flat => Some(Air::Program),
        (ProgramContent::Frame { cue, .. }, LiveState::Frozen) if cue == flat => Some(Air::Held),
        _ => None,
    }
}

/// What kind of row a Cue is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueKind {
    /// Shows one frame.
    Slide,
    /// The minister's phrase the operator waits for.
    Trigger,
    /// Leaves Program unchanged.
    Wait,
    /// An operator-only checkpoint with black output.
    Hold,
    /// Cannot be taken live.
    Unavailable,
}

impl CueKind {
    /// The word a compact row carries for this kind.
    pub fn word(self) -> &'static str {
        match self {
            Self::Slide => "Slide",
            Self::Trigger => "Trigger",
            Self::Wait => "Wait",
            Self::Hold => "Hold",
            Self::Unavailable => "Unavailable",
        }
    }

    /// Whether Grid draws this kind as a thumbnail tile. Trigger, Wait and
    /// Hold Cues have nothing to show and stay compact rows in both modes.
    pub fn is_tile(self) -> bool {
        matches!(self, Self::Slide | Self::Unavailable)
    }

    /// The operator-only marker a row leads with; a slide leads with its
    /// thumbnail instead.
    pub fn lead_icon(self) -> Option<&'static str> {
        match self {
            Self::Slide => None,
            Self::Trigger => Some(icon::MIC),
            Self::Wait => Some(icon::HOURGLASS),
            Self::Hold => Some(icon::SQUARE),
            Self::Unavailable => Some(icon::CIRCLE_OFF),
        }
    }
}

/// Everything one Cue needs.
pub struct CueModel {
    /// Flat index, the argument of `Command::GoTo`.
    pub flat: usize,
    /// "item.cue" within the Mass, 1-based.
    pub number: SharedString,
    pub label: SharedString,
    pub kind: CueKind,
    /// The composed frame, when it is ready; `None` draws black.
    pub frame: Option<Arc<Frame>>,
    pub image: Option<Arc<RenderImage>>,
    /// Why the Cue cannot be taken live, when it cannot.
    pub unavailable: Option<SharedString>,
    pub air: Option<Air>,
    pub preview: bool,
}

impl CueModel {
    /// The caption text: the label, or the reason when unavailable.
    fn caption(&self) -> SharedString {
        match &self.unavailable {
            Some(reason) => format!("Unavailable \u{b7} {reason}").into(),
            None => self.label.clone(),
        }
    }

    /// The full accessible name: number, caption and every indicator.
    fn name(&self) -> SharedString {
        let mut name = format!("{} {}", self.number, self.caption());
        match self.air {
            Some(Air::Program) => name.push_str(", Program"),
            Some(Air::Held) => name.push_str(", Held"),
            None => {}
        }
        if self.preview {
            name.push_str(", Preview");
        }
        name.into()
    }
}

/// One Mass item and its Cues.
pub struct GroupModel {
    pub index: usize,
    pub number: usize,
    pub section: SharedString,
    pub title: SharedString,
    pub collapsed: bool,
    pub cues: Vec<CueModel>,
}

/// The whole timeline.
pub struct TimelineModel {
    pub groups: Vec<GroupModel>,
    pub mode: CueView,
    /// Tiles per row (`PresenterLayout::grid_columns`).
    pub columns: usize,
    /// Frame width of a tile (`PresenterLayout::cell_width`).
    pub cell_width: f32,
    /// Preview moved since the last frame: bring its Cue into view.
    pub follow_preview: bool,
    /// The follow in flight, owned by the view so it outlives each frame.
    pub glide: Glide,
}

/// The callbacks the timeline reports to.
#[derive(Clone)]
pub struct Handlers {
    pub go_to: OnCue,
    pub toggle_group: OnGroup,
}

/// The timeline: one bounded scroll surface with its indicator.
///
/// `scroll` is owned by the view, which is how the rail asks a group to come
/// to the top. The groups are the scroll element's only children, so a
/// group's index is also its child index for `scroll_to_top_of_item`. `focus`
/// is where focus returns when the rail overlay closes.
#[cfg_attr(test, allow(clippy::too_many_arguments))]
pub fn timeline(
    model: TimelineModel,
    scroll: &Scroll,
    focus: &FocusHandle,
    handlers: &Handlers,
    environment: UiEnvironment,
    #[cfg(test)] text_probe: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let model_glide = model.glide.clone();
    let geometry = Geometry {
        mode: model.mode,
        columns: model.columns.max(1),
        cell_width: model.cell_width,
        environment,
        follow: model.follow_preview.then(|| Follow {
            handle: scroll.handle().clone(),
            seen: Rc::default(),
        }),
    };
    let groups: Vec<AnyElement> = model
        .groups
        .into_iter()
        .map(|group| self::group(group, &geometry, handlers, window, cx))
        .collect();
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .child(
            scroll
                .bounded(div().id("cue-timeline"))
                .debug_selector(|| "cue-timeline".into())
                .track_focus(focus)
                .size_full()
                .flex()
                .flex_col()
                .pl(px(scale.apply(tokens::spacing::GROUP)))
                // The reserved scrollbar width (`SCROLLBAR_WIDTH`, unscaled)
                // belongs to the layout model's arithmetic even though the
                // indicator itself overlays the content.
                .pr(px(
                    scale.apply(tokens::spacing::GROUP) + super::layout::SCROLLBAR_WIDTH
                ))
                .children(groups)
                .child(tail(
                    scroll,
                    geometry.follow.as_ref(),
                    &model_glide,
                    environment,
                    #[cfg(test)]
                    text_probe,
                )),
        )
        .children(scroll.indicator("cue-timeline", 0.0, window, cx, environment))
        .into_any_element()
}

/// Empty space after the last group, as tall as the viewport lacks of the last
/// group, so that every group (the last included) can be scrolled to the top
/// of the timeline. It is the scroll element's last child, after the groups,
/// so a group's index is still its child index. It is sized from the previous
/// layout; the surface asks for one more frame when that moved.
///
/// When Preview has just moved, the tail also carries the step that brings it
/// into view ([`follow`]): being the last child, it is laid out after every
/// Cue has said where it is. It also carries the [`glide`] that moves the
/// timeline there.
fn tail(
    scroll: &Scroll,
    follow: Option<&Follow>,
    glide: &Glide,
    environment: UiEnvironment,
    #[cfg(test)] text_probe: &FocusHandle,
) -> AnyElement {
    let handle = scroll.handle();
    let viewport = f32::from(handle.bounds().size.height);
    let last = handle.children_count().saturating_sub(2);
    let last_height = handle
        .bounds_for_item(last)
        .map_or(0.0, |bounds| f32::from(bounds.size.height));
    let tail = div()
        .debug_selector(|| "timeline-tail".into())
        .flex_none()
        .w_full()
        .h(px((viewport - last_height).max(0.0)))
        .relative()
        .children(follow.map(|follow| self::follow(follow, glide, environment)))
        .child(self::glide(glide, handle.clone()));
    // Test only, inside the Cues so that only the binding predicate (and the
    // context check) can keep a key from the Engine. WP6 replaces this probe
    // with a real text field.
    #[cfg(test)]
    let tail = tail.child(
        div()
            .id("text-probe")
            .key_context("TextEntry")
            .track_focus(text_probe),
    );
    tail.into_any_element()
}

struct Geometry {
    mode: CueView,
    columns: usize,
    cell_width: f32,
    environment: UiEnvironment,
    /// Set when Preview's Cue (and Program's, if it fits) must come into view.
    follow: Option<Follow>,
}

fn group(
    group: GroupModel,
    g: &Geometry,
    handlers: &Handlers,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let environment = g.environment;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let index = group.index;
    let title = group.title.clone();
    // Folding reverses the growth and the turn of the chevron; under reduced
    // motion `fold` is already there.
    let fold_id = SharedString::from(format!("group-fold-{index}"));
    let open = motion::fold(fold_id.clone(), !group.collapsed, window, cx);
    let count = group.cues.len();
    let unavailable = group
        .cues
        .iter()
        .filter(|cue| cue.kind == CueKind::Unavailable)
        .count();

    let toggle = {
        let toggle_group = handlers.toggle_group.clone();
        // The item's number leads its title, as in the rail and the Cue
        // numbers below it ("4" here, "4.2" on its Cues).
        Button::new(
            SharedString::from(format!("group-toggle-{index}")),
            title.clone(),
            environment,
        )
        .leading_text(group.number.to_string())
        .name(format!(
            "{} {title}",
            if group.collapsed {
                "Expand"
            } else {
                "Collapse"
            }
        ))
        .icon(icon::CHEVRON_RIGHT)
        .icon_turn(open)
        .quiet()
        .stretch()
        .truncate()
        .on_surface(palette.background)
        .on_activate(move |window, cx| toggle_group(index, window, cx))
    };
    let head = div()
        .debug_selector(move || format!("group-{index}-head"))
        .flex_none()
        .h(px(scale.apply(GROUP_HEAD_HEIGHT)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().flex_1().min_w_0().child(toggle))
        // A folded group still says it holds a Cue that cannot go live.
        .children((unavailable > 0).then(|| {
            div()
                .debug_selector(move || format!("group-{index}-unavailable"))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                .whitespace_nowrap()
                .text_color(rgb(palette.warning))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .child(glyph(icon::CIRCLE_OFF, palette.warning, environment))
                .child(format!("{unavailable} unavailable"))
        }))
        .child(
            div()
                .flex_none()
                .max_w(px(scale.apply(160.0)))
                .truncate()
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(palette.text_muted))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .child(group.section.clone()),
        )
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(palette.text_muted))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .child(format!(
                    "{count} {}",
                    if count == 1 { "Cue" } else { "Cues" }
                )),
        );

    let body = (open > 0.0).then(|| {
        let blocks = blocks(group.cues, g, handlers);
        let (natural, record) = motion::natural_height(fold_id, window, cx);
        let content = div()
            .flex_none()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .pl(px(scale.apply(CUE_INDENT)))
            .pt(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .pb(px(scale.apply(tokens::spacing::GROUP)))
            .children(blocks);
        div()
            .flex()
            .flex_col()
            .w_full()
            .on_children_prepainted(record)
            // Clipped only while it moves, so a settled group never cuts off
            // a ring or a bar.
            .when(open < 1.0, |container| {
                container
                    .overflow_hidden()
                    .opacity(open)
                    .h(px(natural.unwrap_or(0.0) * open))
            })
            .child(content)
    });

    div()
        .id(("group", index))
        .debug_selector(move || format!("group-{index}"))
        .flex_none()
        .w_full()
        .flex()
        .flex_col()
        .border_b(px(tokens::border::SEPARATOR))
        .border_color(rgb(palette.border))
        .child(head)
        .children(body)
        .into_any_element()
}

/// A group's Cues as rows of tiles and compact rows, in order.
fn blocks(cues: Vec<CueModel>, g: &Geometry, handlers: &Handlers) -> Vec<AnyElement> {
    let mut blocks = Vec::new();
    let mut run: Vec<CueModel> = Vec::new();
    let flush = |run: &mut Vec<CueModel>, blocks: &mut Vec<AnyElement>| {
        while !run.is_empty() {
            let rest = run.split_off(g.columns.min(run.len()));
            let row = std::mem::replace(run, rest);
            blocks.push(tile_row(row, g, handlers));
        }
    };
    for cue in cues {
        if g.mode == CueView::Grid && cue.kind.is_tile() {
            run.push(cue);
        } else {
            flush(&mut run, &mut blocks);
            blocks.push(row(cue, g, handlers));
        }
    }
    flush(&mut run, &mut blocks);
    blocks
}

fn tile_row(cues: Vec<CueModel>, g: &Geometry, handlers: &Handlers) -> AnyElement {
    let scale = g.environment.preferences().scale;
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(scale.apply(CELL_GAP)))
        .children(cues.into_iter().map(|cue| tile(cue, g, handlers)))
        .into_any_element()
}

/// The number and the indicators of a caption or row. They never shrink or
/// wrap (Decision 303).
fn indicators(cue: &CueModel, environment: UiEnvironment) -> Vec<AnyElement> {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    // Shape as well as color: a dot for Program, a diamond while held, a
    // square for Preview (the monitors and the header use the same shapes).
    let word = |text: &'static str, color: u32, shape: Marker| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
            .whitespace_nowrap()
            .text_color(rgb(color))
            .child(dock::marker(shape, color, 8.0, environment))
            .child(text)
            .into_any_element()
    };
    let mut all = Vec::new();
    match cue.air {
        Some(Air::Program) => all.push(word("Program", palette.live, Marker::Dot)),
        Some(Air::Held) => all.push(word("Held", palette.warning, Marker::Diamond)),
        None => {}
    }
    if cue.preview {
        all.push(word("Preview", palette.text, Marker::Square));
    }
    all
}

/// An outlined Lucide glyph at the row-marker size.
fn glyph(svg_source: &'static str, color: u32, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    svg()
        .flex_none()
        .size(px(scale.apply(LEAD_ICON)))
        .text_color(rgb(color))
        .data(svg_source.as_bytes())
        .into_any_element()
}

/// The operator-only placeholder over an Unavailable tile's black frame. It
/// sits on the timeline's own thumbnail, never on anything the congregation
/// output draws.
fn unavailable_placeholder(flat: usize, environment: UiEnvironment) -> AnyElement {
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    div()
        .debug_selector(move || format!("cue-{flat}-unavailable"))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
        .text_color(rgb(palette.warning))
        .text_size(px(scale.apply(tokens::typography::LABEL.size)))
        .child(glyph(icon::CIRCLE_OFF, palette.warning, environment))
        .child("Output unavailable")
        .into_any_element()
}

fn tile(cue: CueModel, g: &Geometry, handlers: &Handlers) -> AnyElement {
    let environment = g.environment;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let flat = cue.flat;
    let pad = scale.apply(CELL_PADDING / 2.0);
    let width = g.cell_width + scale.apply(CELL_PADDING);

    let ring = cue.air.map(|air| {
        div()
            .debug_selector(move || format!("cue-{flat}-program-mark"))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(scale.apply(PROGRAM_RING)))
            .border_color(rgb(match air {
                Air::Program => palette.live,
                Air::Held => palette.warning,
            }))
    });
    let frame = div()
        .debug_selector(move || format!("cue-{flat}-frame"))
        .relative()
        .flex_none()
        .child(live_viewport(
            cue.frame.clone(),
            cue.image.clone(),
            g.cell_width,
            None,
            environment,
        ))
        .children(
            (cue.kind == CueKind::Unavailable).then(|| unavailable_placeholder(flat, environment)),
        );
    let wrap = div()
        .relative()
        .flex_none()
        .p(px(pad))
        .child(frame)
        .children(ring);
    // The Preview bar's place exists whether or not the bar does, so a tile
    // never changes height as Preview moves.
    let bar = div()
        .flex_none()
        .h(px(scale.apply(PREVIEW_BAR)))
        .mx(px(pad))
        .children(cue.preview.then(|| {
            div()
                .debug_selector(move || format!("cue-{flat}-preview-mark"))
                .size_full()
                .bg(rgb(palette.text))
        }));
    let caption = div()
        .flex_none()
        .h(px(scale.apply(CAPTION_HEIGHT)))
        .px(px(pad))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .text_color(rgb(if cue.preview {
            palette.text
        } else {
            palette.text_muted
        }))
        .when(cue.preview, |caption| {
            caption.font_weight(FontWeight::SEMIBOLD)
        })
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .child(cue.number.clone()),
        )
        .child(div().flex_1().min_w_0().truncate().child(cue.caption()))
        .children(indicators(&cue, environment));

    let name = cue.name();
    let go_to = handlers.go_to.clone();
    let tile = div()
        .id(("cue", flat))
        .debug_selector(move || format!("cue-{flat}"))
        .relative()
        .children(mark(&cue, g))
        .flex_none()
        .w(px(width))
        .flex()
        .flex_col()
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(name.clone())
        .on_click(move |_, window, cx| go_to(flat, window, cx))
        .child(wrap)
        .child(bar)
        .child(caption);
    // The full label is on hover and in the accessible name (Decision 303).
    primitives::truncated(tile, name, environment).into_any_element()
}

fn row(cue: CueModel, g: &Geometry, handlers: &Handlers) -> AnyElement {
    let environment = g.environment;
    let palette = tokens::palette(environment.appearance());
    let scale = environment.preferences().scale;
    let flat = cue.flat;

    let program = cue.air.map(|air| {
        div()
            .debug_selector(move || format!("cue-{flat}-program-mark"))
            .absolute()
            .top_0()
            .left_0()
            .h_full()
            .w(px(scale.apply(PROGRAM_RING * 2.0)))
            .bg(rgb(match air {
                Air::Program => palette.live,
                Air::Held => palette.warning,
            }))
    });
    let preview = cue.preview.then(|| {
        div()
            .debug_selector(move || format!("cue-{flat}-preview-mark"))
            .absolute()
            .left_0()
            .bottom_0()
            .w_full()
            .h(px(scale.apply(PREVIEW_BAR)))
            .bg(rgb(palette.text))
    });
    // A slide leads with its thumbnail; anything else with its operator-only
    // marker, in the warning color when it cannot go live.
    let lead = match cue.kind.lead_icon() {
        None => div()
            .debug_selector(move || format!("cue-{flat}-mini"))
            .flex_none()
            .child(live_viewport(
                cue.frame.clone(),
                cue.image.clone(),
                scale.apply(MINI_WIDTH),
                None,
                environment,
            ))
            .into_any_element(),
        // The marker sits in a frame of the thumbnail's size and outline, so
        // every row's lead is the same 16:9 box whatever the Cue is.
        Some(svg_source) => div()
            .debug_selector(move || format!("cue-{flat}-marker"))
            .flex_none()
            .w(px(scale.apply(MINI_WIDTH)))
            .h(px(scale.apply(MINI_WIDTH * 9.0 / 16.0)))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .border(px(tokens::border::SEPARATOR))
            .border_color(rgb(if cue.kind == CueKind::Unavailable {
                palette.warning
            } else {
                palette.border_strong
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(glyph(
                svg_source,
                if cue.kind == CueKind::Unavailable {
                    palette.warning
                } else {
                    palette.text_muted
                },
                environment,
            ))
            .into_any_element(),
    };
    let name = cue.name();
    let go_to = handlers.go_to.clone();
    let row = div()
        .id(("cue", flat))
        .debug_selector(move || format!("cue-{flat}"))
        .relative()
        .flex_none()
        .w_full()
        .h(px(scale.apply(tokens::density::COMPACT_ROW)))
        .px(px(scale.apply(tokens::spacing::RELATED)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(scale.apply(tokens::spacing::RELATED)))
        .rounded(px(scale.apply(tokens::radius::CONTROL)))
        .text_size(px(scale.apply(tokens::typography::META.size)))
        .when(cue.preview, |row| {
            row.bg(rgb(palette.selection))
                .font_weight(FontWeight::SEMIBOLD)
        })
        .hover(|style| style.bg(rgb(palette.surface_secondary)))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(name.clone())
        .on_click(move |_, window, cx| go_to(flat, window, cx))
        .child(
            // A fixed column, so the thumbnails and markers line up whether
            // the number is "3.1" or "12.10".
            div()
                .flex_none()
                .min_w(px(scale.apply(NUMBER_WIDTH)))
                .whitespace_nowrap()
                .text_color(rgb(palette.text_muted))
                .child(cue.number.clone()),
        )
        .child(lead)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                .child(cue.label.clone()),
        )
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_color(rgb(palette.text_muted))
                .child(match &cue.unavailable {
                    Some(reason) => format!("Unavailable \u{b7} {reason}"),
                    None => cue.kind.word().to_owned(),
                }),
        )
        .children(indicators(&cue, environment))
        .children(program)
        .children(preview)
        .children(mark(&cue, g));
    primitives::truncated(row, name, environment).into_any_element()
}

/// Room kept between the Preview Cue and the timeline's edges when it is
/// brought into view: below, a little air; above, its group header too.
const FOLLOW_MARGIN: f32 = 16.0;

/// Where the Preview and Program Cues were laid out this frame.
#[derive(Default)]
struct Seen {
    preview: Option<Bounds<Pixels>>,
    program: Option<Bounds<Pixels>>,
}

/// One frame's follow: the timeline's scroll handle, and where the Cues that
/// must be in view turned out to be.
struct Follow {
    handle: ScrollHandle,
    seen: Rc<RefCell<Seen>>,
}

/// When Preview has just moved, an invisible layer over the Preview Cue, and
/// over the Program Cue when Freeze holds a different one, that
/// records where it was laid out. [`follow`] then uses those bounds.
fn mark(cue: &CueModel, g: &Geometry) -> Option<AnyElement> {
    let follow = g.follow.as_ref()?;
    let preview = cue.preview;
    if !preview && cue.air.is_none() {
        return None;
    }
    let seen = follow.seen.clone();
    Some(
        canvas(
            move |bounds, _, _| {
                let mut seen = seen.borrow_mut();
                if preview {
                    seen.preview = Some(bounds);
                } else {
                    seen.program = Some(bounds);
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// A follow in flight: the timeline glides from one offset to another.
#[derive(Clone, Copy, Debug)]
struct Flight {
    from: f32,
    to: f32,
    started: Instant,
    /// The offset the glide set last, so an operator's own scroll, which
    /// moves it elsewhere, ends the glide instead of being fought.
    last: f32,
}

/// The timeline's follow in flight, if any. The Presenter view owns one, so
/// it lasts from the frame Preview moved until the glide lands.
#[derive(Clone, Debug, Default)]
pub struct Glide(Rc<Cell<Option<Flight>>>);

/// The step that scrolls the timeline, during layout, so Preview's Cue sits
/// on the middle of the timeline: what comes next is always in sight below
/// it, stepping down or back up. Near either end the timeline stops at its
/// edge rather than leaving empty space. When Freeze holds Program on a
/// different Cue, the two are centred together if they fit; otherwise Preview
/// wins. A Cue taller than the timeline shows its top, under its group
/// header.
///
/// It only runs on the frame after Preview moved, so the operator's own
/// scrolling is never fought, and hands the move to [`glide`].
fn follow(follow: &Follow, glide: &Glide, environment: UiEnvironment) -> AnyElement {
    let scale = environment.preferences().scale;
    let below = scale.apply(FOLLOW_MARGIN);
    let above = scale.apply(GROUP_HEAD_HEIGHT + FOLLOW_MARGIN);
    let handle = follow.handle.clone();
    let seen = follow.seen.clone();
    let glide = glide.clone();
    canvas(
        move |_, window, cx| {
            let seen = seen.borrow();
            // A folded group draws no Cue: nothing to follow.
            let Some(preview) = seen.preview else {
                return;
            };
            let viewport = handle.bounds();
            let (view_top, view_bottom) = (f32::from(viewport.top()), f32::from(viewport.bottom()));
            let room = view_bottom - view_top;
            let span = |b: Bounds<Pixels>| (f32::from(b.top()), f32::from(b.bottom()));
            let (mut top, mut bottom) = span(preview);
            if let Some(program) = seen.program {
                let (program_top, program_bottom) = span(program);
                let (both_top, both_bottom) = (top.min(program_top), bottom.max(program_bottom));
                if both_bottom - both_top + above + below <= room {
                    (top, bottom) = (both_top, both_bottom);
                }
            }
            let offset = handle.offset();
            let lowest = -f32::from(handle.max_offset().y);
            let y = f32::from(offset.y);
            let wanted = if bottom - top + above + below > room {
                y + (view_top + above - top)
            } else {
                y + ((view_top + view_bottom) - (top + bottom)) / 2.0
            };
            let pixel = 1.0 / window.scale_factor();
            let wanted = settled_offset(wanted, lowest, pixel);
            // Already there, or already gliding there (a step along the same
            // row mid-glide): restarting the curve would lurch, so leave it.
            let heading = glide.0.get().map(|flight| flight.to);
            if (wanted - heading.unwrap_or(y)).abs() <= pixel + f32::EPSILON {
                return;
            }
            if cx.reduce_motion() {
                // No travel under reduced motion: the timeline is simply there.
                glide.0.set(None);
                handle.set_offset(point(offset.x, px(wanted)));
                window.refresh();
                return;
            }
            // From wherever it is, so a move during a glide turns it around.
            glide.0.set(Some(Flight {
                from: y,
                to: wanted,
                started: cx.background_executor().now(),
                last: y,
            }));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_0()
    .into_any_element()
}

/// Where a follow lands: within the timeline's travel (`lowest..=0`) and on a
/// whole device pixel (`pixel` logical px). GPUI draws elements on whole
/// device pixels, so a centre that ends in half a pixel is measured back
/// rounded on the next step; an unrounded target then differed by half a
/// pixel each time and the timeline hopped a pixel to and fro along a row.
fn settled_offset(wanted: f32, lowest: f32, pixel: f32) -> f32 {
    ((wanted / pixel).round() * pixel).clamp(lowest, 0.0)
}

/// Moves the timeline along a follow in flight, over `160ms` on the easing
/// token, as navigation moves (Decision 301). It never holds input: the next
/// key press moves Preview at once and retargets the glide from where it is.
fn glide(glide: &Glide, handle: ScrollHandle) -> AnyElement {
    let glide = glide.clone();
    canvas(
        move |_, window, cx| {
            let Some(flight) = glide.0.get() else {
                return;
            };
            let offset = handle.offset();
            if (f32::from(offset.y) - flight.last).abs() >= 0.5 {
                // The operator scrolled: theirs wins.
                glide.0.set(None);
                return;
            }
            let duration = Duration::from_millis(tokens::motion::STANDARD_DURATION_MS.into());
            let elapsed = cx
                .background_executor()
                .now()
                .saturating_duration_since(flight.started);
            let progress = (elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0);
            let y = flight.from + (flight.to - flight.from) * motion::easing().sample(progress);
            handle.set_offset(point(offset.x, px(y)));
            if progress >= 1.0 {
                glide.0.set(None);
                window.refresh();
            } else {
                glide.0.set(Some(Flight { last: y, ..flight }));
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_0()
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::frames::test_support::fixture_snapshot;
    use presentation::{commands::Command, engine::Engine, state::Millis};

    fn engine() -> Engine {
        Engine::new(Arc::new(fixture_snapshot().snapshot().clone()))
    }

    /// Re-centring a Cue measured on whole device pixels, step after step
    /// along its row, lands on the same offset every time.
    #[test]
    fn centring_settles_on_one_whole_pixel_offset() {
        for scale_factor in [1.0_f32, 1.25, 1.5] {
            let pixel = 1.0 / scale_factor;
            // GPUI places the Cue on whole device pixels.
            let snap = |v: f32| (v * scale_factor).round() / scale_factor;
            let (content_top, height, view) = (1400.3_f32, 301.0_f32, 1000.0_f32);
            let centre = |y: f32| {
                let top = snap(content_top + y);
                let wanted = y + (view - (top + top + height)) / 2.0;
                settled_offset(wanted, -5000.0, pixel)
            };
            let first = centre(0.0);
            let mut y = first;
            for step in 0..6 {
                let next = centre(y);
                assert!(
                    (next - y).abs() <= pixel + f32::EPSILON,
                    "@{scale_factor}: step {step} moved {y} -> {next}"
                );
                y = if (next - y).abs() <= pixel + f32::EPSILON {
                    y
                } else {
                    next
                };
            }
            assert!(((first * scale_factor).round() - first * scale_factor).abs() < 1e-3);
        }
    }

    #[test]
    fn only_a_cue_that_put_a_frame_on_program_is_on_air() {
        let mut engine = engine();
        let at = Millis(0);
        engine.apply(Command::EnableOutput, at).unwrap();
        assert_eq!(air(engine.state(), 0), None, "enabled but black");
        engine.apply(Command::GoTo(2), at).unwrap();
        engine.apply(Command::Take, at).unwrap();
        assert_eq!(air(engine.state(), 2), Some(Air::Program));
        assert_eq!(air(engine.state(), 1), None, "no other Cue is on air");
        engine.apply(Command::ToggleFreeze, Millis(1000)).unwrap();
        assert_eq!(air(engine.state(), 2), Some(Air::Held));
        engine.apply(Command::ToggleFreeze, Millis(1000)).unwrap();
        engine.apply(Command::ToggleBlackout, Millis(1000)).unwrap();
        assert_eq!(air(engine.state(), 2), None, "Blackout shows nothing");
    }

    #[test]
    fn only_slides_and_unavailable_cues_are_tiles() {
        for kind in [CueKind::Trigger, CueKind::Wait, CueKind::Hold] {
            assert!(!kind.is_tile(), "{kind:?}");
        }
        for kind in [CueKind::Slide, CueKind::Unavailable] {
            assert!(kind.is_tile(), "{kind:?}");
        }
    }

    #[test]
    fn a_caption_names_the_reason_and_every_indicator() {
        let cue = CueModel {
            flat: 3,
            number: "2.1".into(),
            label: "Verse".into(),
            kind: CueKind::Unavailable,
            frame: None,
            image: None,
            unavailable: Some("the frame could not be prepared".into()),
            air: Some(Air::Held),
            preview: true,
        };
        assert_eq!(
            cue.caption(),
            "Unavailable \u{b7} the frame could not be prepared"
        );
        assert_eq!(
            cue.name(),
            "2.1 Unavailable \u{b7} the frame could not be prepared, Held, Preview"
        );
    }
}
