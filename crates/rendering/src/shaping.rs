//! Deterministic shaping, wrapping, and measurement for the canonical path.
//!
//! T0.4 PR 4: shape resolved text over the bundled faces with the qualified
//! shaper, wrap to the safe-area width from measured advances, and report the
//! pagination inputs the later review flow consumes. Everything here is
//! integer arithmetic over the bundled bytes, so identical inputs yield
//! identical output in one process, across processes, and on both packaged
//! targets.
//!
//! Scope notes:
//! - Sizes are candidate at T0.4.10 at display 90px / passage 70px / meta 50px
//!   (PR 5 provisional 72/56/40, each +25%), plus the Celebration exception
//!   at candidate title 135px/800 with optional subtitle 68px/300, plus a
//!   Response-only pending candidate at 113px/400 (`90 * 1.25 = 112.5`,
//!   rounded; advance `130px`) used only for `Response`, pending
//!   human approval per `docs/qualification/T0.4-MANUAL-GATE.md`: the 90px
//!   display baseline stays the gallery baseline for the other emphasized
//!   bodies, and the template
//!   corpus chooses per-zone sizes from it. Line advance stays
//!   `ceil(size * 1.15)` at every size. The measured-not-counted rule the
//!   corpus refines is the pinned part.
//! - Placement is neutral by default: lines start at the zone's left edge
//!   unless the caller passes another [`Alignment`]. Alignment shifts come
//!   from measured widths only, so identical inputs stay identical everywhere.
//! - Blank authored lines contribute no rendered lines. In the body zone they
//!   additionally mark a hard split that overrides any automatic proposal
//!   derived from fit; header and footer zones ignore the split flag.
//! - Combining marks shape in their base's face, and breaks never separate a
//!   mark from its base, so decomposed characters survive fallback intact.
//! - Overlong words break between characters and overlong zones keep every
//!   line: this path never shrinks and never clips. Overflow is reported, not
//!   hidden.

use crate::fonts::{self, CanonicalFont};

/// Provisional body size: the 90px shared display baseline (candidate at T0.4.10
/// from the 72px PR 5 baseline, +25%, pending approval), still used by Lyrics, Psalm, and Song identification; the projected-response token is now the Response-only 113px. The template corpus carries the full
/// hierarchy later.
pub const BODY_FONT_SIZE: u32 = 90;
/// Regular weight: the variable-font default instance. Layout and raster
/// apply no `wght` variation here, so every non-celebration zone keeps its
/// pinned advances and masks byte-for-byte.
pub const REGULAR_WEIGHT: u16 = 400;
/// Candidate celebration-title weight (T0.4.10, pending human approval):
/// ExtraBold 800, the top of the bundled Atkinson `wght` axis.
pub const CELEBRATION_TITLE_WEIGHT: u16 = 800;
/// Candidate celebration-subtitle weight (T0.4.10, pending human approval):
/// Light 300 on the bundled `wght` axis.
pub const CELEBRATION_SUBTITLE_WEIGHT: u16 = 300;
/// Line advance for a size: `ceil(size * 1.15)` logical units.
pub const fn line_height_for(size: u32) -> u32 {
    (size * 115).div_ceil(100)
}
/// Line advance for the provisional size: ceil(90 * 1.15) = 104 logical units.
pub const BODY_LINE_HEIGHT: u32 = line_height_for(BODY_FONT_SIZE);

/// How body content relates to its zone: it fits, or it overflows with every
/// line still present and a warning riding beside the frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fit {
    Fits,
    Overflows,
}

/// Horizontal alignment inside a zone, applied from measured widths with
/// truncating integer arithmetic, so identical inputs stay identical on both
/// packaged targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alignment {
    /// Lines start at the zone's left edge.
    Left,
    /// Each line centers in the zone: `x = zone + (zone_width - line_width) / 2`.
    Center,
    /// Lines stay left-aligned inside one block, and the block centers: every
    /// line shifts by `(zone_width - widest_line_width) / 2`.
    CenterBlock,
}

/// Vertical placement of a zone's shaped block inside its rect, applied from
/// measured content height. Rects never move; only lines do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VAlign {
    /// Lines start at the zone's top edge.
    Top,
    /// The block centers: lines shift down by half the leftover height,
    /// truncating, and an overflowing block stays at the top edge beside
    /// its warning.
    Center,
}

/// Measured pagination inputs for one composition: geometry, line counts, and
/// the fit result rather than a boolean.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginationInputs {
    /// Authored body lines, split on `'\n'` and counting blanks.
    pub authored_body_lines: u32,
    pub header_lines: u32,
    pub body_lines: u32,
    pub footer_lines: u32,
    /// Measured body content height in logical units.
    pub body_content_height: u32,
    /// Body zone height available in logical units.
    pub body_available_height: u32,
    pub fit: Fit,
    /// A blank authored line divides the slide and overrides any automatic
    /// proposal derived from fit.
    pub hard_split: bool,
    /// Authored index of the first blank body line, when present.
    pub first_hard_split: Option<u32>,
}

/// One shaped glyph with its logical position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedGlyph {
    pub id: u32,
    pub font: CanonicalFont,
    pub x: i32,
    pub y: i32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    /// Byte offset of the source character within its wrapped word.
    pub cluster: u32,
}

/// One wrapped line with its logical bounds and glyphs in visual order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapedLine {
    /// Wrapped line text with single-space joins; never contains a newline.
    pub text: String,
    /// Index of the authored line (split on `'\n'`) this line came from.
    pub authored_index: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: u32,
    /// Logical font size the line was shaped at, for the raster pass to
    /// scale. It never enters [`crate::Frame::serialize`], so goldens are
    /// unaffected; the raster pass needs the size that produced the advances.
    pub size: u32,
    /// Variable-font `wght` the line was shaped at, for the raster pass to
    /// match. `REGULAR_WEIGHT` (400) means the default instance with no
    /// variation applied; celebration title/subtitle carry 800/300. It never
    /// enters [`crate::Frame::serialize`]: the measured advances already pin
    /// the composition, and the raster pass reads this to pick the same axis
    /// the shaper used.
    pub weight: u16,
    pub glyphs: Vec<PlacedGlyph>,
}

/// Body text laid out at a provisional origin: the caller shifts it into its
/// measured zone afterwards.
pub struct LaidOutBody {
    pub lines: Vec<ShapedLine>,
    pub authored_line_count: u32,
    pub hard_split: bool,
    pub first_hard_split: Option<u32>,
}

/// Parsed faces plus the integer scales derived from them. Advances depend
/// on the requested size, so ascent and inter-word gap are computed per size
/// rather than stored.
pub struct Faces<'a> {
    primary: rustybuzz::Face<'a>,
    fallback: rustybuzz::Face<'a>,
    upem_primary: i32,
    upem_fallback: i32,
    ascender_units: i32,
}

impl Faces<'_> {
    /// Baseline offset within a line box, from the primary ascender.
    fn ascent_px(&self, size: u32) -> i32 {
        scale_units(self.ascender_units, self.upem_primary, size)
    }

    /// Advance of one inter-word gap, from the primary face.
    fn space_px(&self, size: u32) -> i32 {
        shape_run(" ", CanonicalFont::Primary, self, size)
            .iter()
            .map(|glyph| glyph.x_advance)
            .sum()
    }
}

/// Parse both bundled faces once per composition. `None` means the bundle
/// itself is damaged; the caller reports it as a structured failure.
pub fn load_faces() -> Option<Faces<'static>> {
    load_faces_with_weight(REGULAR_WEIGHT)
}

/// Parse both bundled faces at one variable-font `wght`. `REGULAR_WEIGHT`
/// keeps the default instance with no variation so existing zones stay
/// byte-identical; any other weight sets `wght` on both faces so shaping
/// advances match the raster masks. Vertical metrics stay on the default
/// ascender: weight changes thickness and advances, not the line box.
pub fn load_faces_with_weight(weight: u16) -> Option<Faces<'static>> {
    let mut primary = rustybuzz::Face::from_slice(fonts::ATKINSON_HYPERLEGIBLE_NEXT, 0)?;
    let mut fallback = rustybuzz::Face::from_slice(fonts::NOTO_SANS, 0)?;
    if weight != REGULAR_WEIGHT {
        let variation = rustybuzz::Variation {
            tag: ttf_parser::Tag::from_bytes(b"wght"),
            value: f32::from(weight),
        };
        primary.set_variations(&[variation]);
        fallback.set_variations(&[variation]);
    }
    let parsed = ttf_parser::Face::parse(fonts::ATKINSON_HYPERLEGIBLE_NEXT, 0).ok()?;
    let fallback_parsed = ttf_parser::Face::parse(fonts::NOTO_SANS, 0).ok()?;
    let upem_primary = i32::from(parsed.units_per_em());
    let upem_fallback = i32::from(fallback_parsed.units_per_em());
    if upem_primary <= 0 || upem_fallback <= 0 {
        return None;
    }
    Some(Faces {
        primary,
        fallback,
        upem_primary,
        upem_fallback,
        ascender_units: i32::from(parsed.ascender()),
    })
}

/// Scale font units to logical units at the requested size. Truncating
/// integer division is deterministic on every target for identical inputs.
fn scale_units(units: i32, upem: i32, size: u32) -> i32 {
    ((i64::from(units) * i64::from(size)) / i64::from(upem)) as i32
}

/// The shaper script for a run: Han when any CJK ideograph is present, Greek
/// when any Greek letter is present, otherwise Latin. Digits, punctuation,
/// and spacing ride with the surrounding run.
fn script_for(text: &str) -> rustybuzz::Script {
    let mut seen_greek = false;
    for c in text.chars() {
        match script_class(c) {
            ScriptClass::Han => return rustybuzz::script::HAN,
            ScriptClass::Greek => seen_greek = true,
            ScriptClass::Latin | ScriptClass::Other => {}
        }
    }
    if seen_greek {
        rustybuzz::script::GREEK
    } else {
        rustybuzz::script::LATIN
    }
}

/// One glyph before placement: scaled advances with an unpositioned origin.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RelativeGlyph {
    id: u32,
    font: CanonicalFont,
    x_advance: i32,
    y_advance: i32,
    x_offset: i32,
    y_offset: i32,
    cluster: u32,
}

/// Shape one single-face run. Direction is always left-to-right: the
/// congregation corpus for this milestone reads that way.
fn shape_run(run: &str, font: CanonicalFont, faces: &Faces, size: u32) -> Vec<RelativeGlyph> {
    let (face, upem) = match font {
        CanonicalFont::Primary => (&faces.primary, faces.upem_primary),
        CanonicalFont::Fallback => (&faces.fallback, faces.upem_fallback),
    };
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(run);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);
    buffer.set_script(script_for(run));
    let shaped = rustybuzz::shape(face, &[], buffer);
    shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions().iter())
        .map(|(info, position)| RelativeGlyph {
            id: info.glyph_id,
            font,
            x_advance: scale_units(position.x_advance, upem, size),
            y_advance: scale_units(position.y_advance, upem, size),
            x_offset: scale_units(position.x_offset, upem, size),
            y_offset: scale_units(position.y_offset, upem, size),
            cluster: info.cluster,
        })
        .collect()
}

/// A combining mark extends its base character: it must shape in the base's
/// face so the shaper can attach it, even when the mark alone would resolve
/// to the other bundled face. Covers the combining-mark blocks (diacritical
/// marks, extensions, supplement, symbols, half marks); a dangling mark with
/// no base keeps its own resolution.
fn is_combining_mark(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF
        | 0xFE20..=0xFE2F
    )
}

/// The scripts this path itemizes. Latin, Greek, and Han are strong: a
/// change between them starts a new shaping run even when the face does
/// not. Everything else (digits, punctuation, spacing, marks) is common:
/// it rides with its neighbors instead of forcing a run boundary. Latin
/// letters run through Extended Additional; rarer Latin extensions ride as
/// common and still shape under the Latin script by default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScriptClass {
    Han,
    Greek,
    Latin,
    Other,
}

fn script_class(c: char) -> ScriptClass {
    match c as u32 {
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2A6DF => ScriptClass::Han,
        0x0370..=0x03FF | 0x1F00..=0x1FFF => ScriptClass::Greek,
        0x41..=0x5A
        | 0x61..=0x7A
        | 0xC0..=0xD6
        | 0xD8..=0xF6
        | 0xF8..=0xFF
        | 0x100..=0x24F
        | 0x1E00..=0x1EFF => ScriptClass::Latin,
        _ => ScriptClass::Other,
    }
}

/// One word shaped once, with clusters rebased to word byte offsets.
struct ShapedWord {
    text: String,
    glyphs: Vec<RelativeGlyph>,
    width: i32,
}

fn shape_word(word: &str, faces: &Faces, size: u32) -> ShapedWord {
    // Cluster base characters with their combining marks, then choose one
    // face per cluster: primary only when it carries every member,
    // otherwise the whole cluster shapes in fallback (which stays the
    // catch-all, even for marks neither face carries). The class is the
    // base's; marks inherit it.
    let mut clusters: Vec<(usize, usize, bool, ScriptClass)> = Vec::new();
    for (byte, c) in word.char_indices() {
        let end = byte + c.len_utf8();
        match clusters.last_mut() {
            Some((_, cluster_end, covers, _)) if is_combining_mark(c) => {
                *cluster_end = end;
                *covers &= fonts::resolve_font_for(c) == CanonicalFont::Primary;
            }
            _ => clusters.push((
                byte,
                end,
                fonts::resolve_font_for(c) == CanonicalFont::Primary,
                script_class(c),
            )),
        }
    }
    // Merge adjacent clusters into single-face, single-script runs. A
    // strong-script change starts a new run even when the face does not; a
    // common cluster joins its neighbors, and a still-neutral run adopts
    // the first strong script that reaches it so leading commons can never
    // bridge two strong scripts into one run.
    let mut runs: Vec<(CanonicalFont, ScriptClass, usize, usize)> = Vec::new();
    for (start, end, covers, class) in clusters {
        let font = if covers {
            CanonicalFont::Primary
        } else {
            CanonicalFont::Fallback
        };
        match runs.last_mut() {
            Some((run_font, run_class, _, run_end))
                if *run_font == font
                    && (*run_class == ScriptClass::Other
                        || class == ScriptClass::Other
                        || *run_class == class) =>
            {
                *run_end = end;
                if *run_class == ScriptClass::Other {
                    *run_class = class;
                }
            }
            _ => runs.push((font, class, start, end)),
        }
    }
    let mut glyphs = Vec::new();
    for (font, _, start, end) in runs {
        for mut glyph in shape_run(&word[start..end], font, faces, size) {
            glyph.cluster += start as u32;
            glyphs.push(glyph);
        }
    }
    let width = glyphs.iter().map(|glyph| glyph.x_advance).sum();
    ShapedWord {
        text: word.to_owned(),
        glyphs,
        width,
    }
}

/// One wrapped line before placement.
struct LinePlan {
    text: String,
    width: i32,
    glyphs: Vec<RelativeGlyph>,
    /// Glyph indices after which one inter-word gap follows.
    gaps: Vec<usize>,
}

fn push_line(plans: &mut Vec<LinePlan>, words: &[ShapedWord], indices: &[usize], space_px: i32) {
    let mut text = String::new();
    let mut glyphs = Vec::new();
    let mut gaps = Vec::new();
    for (n, &i) in indices.iter().enumerate() {
        if n > 0 {
            text.push(' ');
        }
        text.push_str(&words[i].text);
        glyphs.extend_from_slice(&words[i].glyphs);
        if n + 1 < indices.len() {
            gaps.push(glyphs.len() - 1);
        }
    }
    let words_width: i32 = indices.iter().map(|&i| words[i].width).sum();
    let width = words_width + indices.len().saturating_sub(1) as i32 * space_px;
    plans.push(LinePlan {
        text,
        width,
        glyphs,
        gaps,
    });
}

/// Break one overlong word between characters so no text is ever dropped.
/// Each fragment is re-shaped so its advances stay truthful to the fragment.
/// Base characters cluster with their combining marks so a break never
/// separates a mark from its base.
fn split_word(word: &ShapedWord, max_width: i32, faces: &Faces, size: u32) -> Vec<LinePlan> {
    let mut units: Vec<(usize, usize)> = Vec::new();
    for (byte, c) in word.text.char_indices() {
        match units.last_mut() {
            Some((_, end)) if is_combining_mark(c) => *end = byte + c.len_utf8(),
            _ => units.push((byte, byte + c.len_utf8())),
        }
    }
    let mut plans = Vec::new();
    let mut start = 0;
    while start < units.len() {
        let mut end = start + 1;
        let mut shaped = shape_word(&word.text[units[start].0..units[start].1], faces, size);
        while end < units.len() {
            let candidate = &word.text[units[start].0..units[end].1];
            let candidate_shaped = shape_word(candidate, faces, size);
            if candidate_shaped.width > max_width {
                break;
            }
            shaped = candidate_shaped;
            end += 1;
        }
        let base = units[start].0 as u32;
        let mut glyphs = shaped.glyphs;
        for glyph in &mut glyphs {
            glyph.cluster += base;
        }
        plans.push(LinePlan {
            text: shaped.text,
            width: shaped.width,
            glyphs,
            gaps: Vec::new(),
        });
        start = end;
    }
    plans
}

/// Wrap one authored line (no newlines, non-blank) into line plans.
fn wrap_authored_line(line: &str, max_width: i32, faces: &Faces, size: u32) -> Vec<LinePlan> {
    let words: Vec<ShapedWord> = line
        .split_whitespace()
        .map(|word| shape_word(word, faces, size))
        .collect();
    let mut plans = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_width: i32 = 0;
    let space_px = faces.space_px(size);
    for (i, word) in words.iter().enumerate() {
        if word.width > max_width {
            if !current.is_empty() {
                push_line(&mut plans, &words, &current, space_px);
                current.clear();
                current_width = 0;
            }
            plans.extend(split_word(word, max_width, faces, size));
        } else if current.is_empty() {
            current.push(i);
            current_width = word.width;
        } else if current_width + space_px + word.width <= max_width {
            current.push(i);
            current_width += space_px + word.width;
        } else {
            push_line(&mut plans, &words, &current, space_px);
            current.clear();
            current.push(i);
            current_width = word.width;
        }
    }
    if !current.is_empty() {
        push_line(&mut plans, &words, &current, space_px);
    }
    plans
}

/// Assign logical positions to one planned line. `x_shift` moves the whole
/// line right inside its zone; it comes from the zone's [`Alignment`] and its
/// measured widths, and there is no other input it could come from.
struct LinePlacement {
    authored_index: u32,
    origin_x: i32,
    line_y: i32,
    line_height: u32,
    size: u32,
    weight: u16,
    ascent_px: i32,
    space_px: i32,
    x_shift: i32,
}

fn place(plan: LinePlan, at: LinePlacement) -> ShapedLine {
    let baseline = at.line_y + at.ascent_px;
    let mut glyphs = Vec::with_capacity(plan.glyphs.len());
    let mut cursor = at.origin_x + at.x_shift;
    for (i, glyph) in plan.glyphs.into_iter().enumerate() {
        glyphs.push(PlacedGlyph {
            x: cursor + glyph.x_offset,
            y: baseline + glyph.y_offset,
            x_advance: glyph.x_advance,
            y_advance: glyph.y_advance,
            x_offset: glyph.x_offset,
            y_offset: glyph.y_offset,
            id: glyph.id,
            font: glyph.font,
            cluster: glyph.cluster,
        });
        cursor += glyph.x_advance;
        if plan.gaps.contains(&i) {
            cursor += at.space_px;
        }
    }
    ShapedLine {
        text: plan.text,
        authored_index: at.authored_index,
        x: at.origin_x + at.x_shift,
        y: at.line_y,
        width: plan.width,
        height: at.line_height,
        size: at.size,
        weight: at.weight,
        glyphs,
    }
}

/// Horizontal shift for one line under an alignment: truncating division
/// keeps it deterministic, and an overwide line keeps a negative shift so
/// the measured geometry stays truthful beside the overflow warning.
fn shift_for(align: Alignment, zone_width: i32, line_width: i32, block_width: i32) -> i32 {
    match align {
        Alignment::Left => 0,
        Alignment::Center => (zone_width - line_width) / 2,
        Alignment::CenterBlock => (zone_width - block_width) / 2,
    }
}

/// Wrap a header/footer-style zone at a provisional origin. Blank authored
/// lines contribute nothing. `weight` is stored on each line for the raster
/// pass; the caller must pass the same weight its `faces` were loaded with.
pub fn layout_zone(
    text: &str,
    size: u32,
    max_width: i32,
    origin: (i32, i32),
    faces: &Faces,
    align: Alignment,
    weight: u16,
) -> Vec<ShapedLine> {
    let (origin_x, origin_y) = origin;
    let line_height = line_height_for(size);
    let ascent_px = faces.ascent_px(size);
    let space_px = faces.space_px(size);
    let mut planned: Vec<(LinePlan, u32)> = Vec::new();
    for (authored_index, authored) in text.split('\n').enumerate() {
        if authored.trim().is_empty() {
            continue;
        }
        for plan in wrap_authored_line(authored, max_width, faces, size) {
            planned.push((plan, authored_index as u32));
        }
    }
    let block_width = planned
        .iter()
        .map(|(plan, _)| plan.width)
        .max()
        .unwrap_or(0);
    planned
        .into_iter()
        .enumerate()
        .map(|(index, (plan, authored_index))| {
            let line_y = origin_y + index as i32 * line_height as i32;
            let x_shift = shift_for(align, max_width, plan.width, block_width);
            place(
                plan,
                LinePlacement {
                    authored_index,
                    origin_x,
                    line_y,
                    line_height,
                    size,
                    weight,
                    ascent_px,
                    space_px,
                    x_shift,
                },
            )
        })
        .collect()
}

/// Wrap the body zone, recording the hard split blank lines mark. `weight`
/// is stored on each line for the raster pass; the caller must pass the same
/// weight its `faces` were loaded with.
pub fn layout_body(
    text: &str,
    size: u32,
    max_width: i32,
    origin: (i32, i32),
    faces: &Faces,
    align: Alignment,
    weight: u16,
) -> LaidOutBody {
    let (origin_x, origin_y) = origin;
    let line_height = line_height_for(size);
    let ascent_px = faces.ascent_px(size);
    let space_px = faces.space_px(size);
    let mut planned: Vec<(LinePlan, u32)> = Vec::new();
    let mut authored_line_count = 0;
    let mut hard_split = false;
    let mut first_hard_split = None;
    for (authored_index, authored) in text.split('\n').enumerate() {
        authored_line_count += 1;
        if authored.trim().is_empty() {
            hard_split = true;
            if first_hard_split.is_none() {
                first_hard_split = Some(authored_index as u32);
            }
            continue;
        }
        for plan in wrap_authored_line(authored, max_width, faces, size) {
            planned.push((plan, authored_index as u32));
        }
    }
    let block_width = planned
        .iter()
        .map(|(plan, _)| plan.width)
        .max()
        .unwrap_or(0);
    let lines = planned
        .into_iter()
        .enumerate()
        .map(|(index, (plan, authored_index))| {
            let line_y = origin_y + index as i32 * line_height as i32;
            let x_shift = shift_for(align, max_width, plan.width, block_width);
            place(
                plan,
                LinePlacement {
                    authored_index,
                    origin_x,
                    line_y,
                    line_height,
                    size,
                    weight,
                    ascent_px,
                    space_px,
                    x_shift,
                },
            )
        })
        .collect();
    LaidOutBody {
        lines,
        authored_line_count,
        hard_split,
        first_hard_split,
    }
}
