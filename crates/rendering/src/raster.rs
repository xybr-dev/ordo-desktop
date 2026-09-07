//! Rasterize a completed logical frame to RGBA at a target density.
//!
//! T0.4 PR 7: the logical frame is already complete when this runs. Density
//! changes pixel dimensions and sampling sharpness only; it never alters the
//! composition the frame serializes. The entry takes the finished frame plus
//! a raster-time image source that maps each recorded foreground identity to
//! its encoded PNG bytes, and paints that image into the frame's contained
//! bounds. The logical frame carries geometry only, so no decode path enters
//! composition.
//!
//! The canvas stays square-cornered at the full logical extent. The `4px`
//! operator chrome radius belongs to the later operator wrapper; it never
//! enters the render, saved content, or an export.
//!
//! Output is a plain width, height, and RGBA byte vector sized exactly for
//! `presentation::ProgramFrame`: non-empty with length a multiple of four,
//! so the `ordo` composition layer can wrap it without `rendering` naming
//! that type. There is no frame cache, thumbnail path, or export flow here.

use std::collections::HashMap;

use crate::fonts::CanonicalFont;
use crate::interface::{Frame, LOGICAL_HEIGHT, LOGICAL_WIDTH, TargetDensity};

/// One rasterized canonical frame: finished RGBA pixels plus their size.
///
/// Pixels are row-major, top-left origin, four bytes per pixel (R, G, B, A),
/// fully opaque. `width` and `height` are the density-scaled extent;
/// [`RasterizedFrame::logical_size`] is always the `1920x1080` composition
/// the pixels were sampled from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RasterizedFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl RasterizedFrame {
    /// Raster width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Raster height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Finished RGBA bytes, row-major from the top-left.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// The logical composition size the pixels were sampled from.
    pub fn logical_size(&self) -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
    }

    /// Encode the pixels as PNG bytes through the qualified `tiny-skia`
    /// edge, for golden fixtures and operator handoff. The logical
    /// composition stays the source of truth; these bytes are a sampling.
    pub fn encode_png(&self) -> Result<Vec<u8>, String> {
        let size = tiny_skia::IntSize::from_wh(self.width, self.height)
            .ok_or_else(|| format!("bad raster size {}x{}", self.width, self.height))?;
        tiny_skia::Pixmap::from_vec(self.rgba.clone(), size)
            .ok_or_else(|| "raster bytes do not match their size".to_owned())?
            .encode_png()
            .map_err(|error| format!("png encode failed: {error}"))
    }
}

/// The structured ways rasterization can fail.
///
/// A missing or undecodable foreground asset fails here rather than
/// painting an empty or partial frame: the logical frame stays complete and
/// the caller sees which identity could not resolve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RasterError {
    /// The frame records a foreground identity with no entry in the image source.
    UnknownImage { id: String },
    /// The image source entry for this identity is not decodable PNG bytes.
    InvalidImageBytes { id: String },
    /// The bundled canonical font bytes do not parse.
    FontUnavailable,
    /// The density-scaled canvas has no backing store.
    CanvasTooLarge { width: u32, height: u32 },
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownImage { id } => {
                write!(
                    f,
                    "foreground image {id:?} has no entry in the image source"
                )
            }
            Self::InvalidImageBytes { id } => {
                write!(f, "foreground image {id:?} is not decodable PNG bytes")
            }
            Self::FontUnavailable => write!(f, "bundled canonical font is unavailable"),
            Self::CanvasTooLarge { width, height } => {
                write!(f, "raster canvas {width}x{height} has no backing store")
            }
        }
    }
}

impl std::error::Error for RasterError {}

/// Rasterize one completed frame at the target density.
///
/// `images` maps each foreground identity the frame records to its encoded
/// PNG bytes. Frames without a foreground element ignore the map; frames
/// with one fail when the identity is absent or undecodable. The same frame
/// rasterized at two densities yields different `width`/`height` and the
/// same [`Frame::serialize`] output.
pub fn rasterize(
    frame: &Frame,
    density: TargetDensity,
    images: &HashMap<String, Vec<u8>>,
) -> Result<RasterizedFrame, RasterError> {
    let percent = u64::from(density.scale_percent());
    let width = (u64::from(LOGICAL_WIDTH) * percent / 100) as u32;
    let height = (u64::from(LOGICAL_HEIGHT) * percent / 100) as u32;
    let mut canvas = tiny_skia::Pixmap::new(width, height)
        .ok_or(RasterError::CanvasTooLarge { width, height })?;
    canvas.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));

    if let Some(placed) = frame.image() {
        let bytes = images
            .get(&placed.id)
            .ok_or_else(|| RasterError::UnknownImage {
                id: placed.id.clone(),
            })?;
        let source =
            tiny_skia::Pixmap::decode_png(bytes).map_err(|_| RasterError::InvalidImageBytes {
                id: placed.id.clone(),
            })?;
        blit_contained(&mut canvas, &source, &placed.bounds, percent);
    }

    let primary = swash::FontRef::from_index(CanonicalFont::Primary.bytes(), 0)
        .ok_or(RasterError::FontUnavailable)?;
    let fallback = swash::FontRef::from_index(CanonicalFont::Fallback.bytes(), 0)
        .ok_or(RasterError::FontUnavailable)?;
    let mut context = swash::scale::ScaleContext::new();
    for line in frame
        .header_lines()
        .iter()
        .chain(frame.body_lines())
        .chain(frame.footer_lines())
    {
        draw_line(
            &mut canvas,
            &mut context,
            &primary,
            &fallback,
            line,
            percent,
        )?;
    }

    let (width, height) = (canvas.width(), canvas.height());
    Ok(RasterizedFrame {
        width,
        height,
        rgba: canvas.take(),
    })
}

/// Paint the decoded foreground into the scaled contained bounds.
///
/// Nearest-neighbor sampling with source-over compositing in premultiplied
/// space. Deterministic integer indexing keeps both packaged targets in
/// agreement; higher-quality filtering belongs to a later measured change,
/// not to this gate.
// ponytail: nearest-neighbor upscale, bicubic via tiny-skia if images look soft
fn blit_contained(
    canvas: &mut tiny_skia::Pixmap,
    source: &tiny_skia::Pixmap,
    bounds: &crate::interface::Rect,
    percent: u64,
) {
    let (source_w, source_h) = (source.width(), source.height());
    if source_w == 0 || source_h == 0 {
        return;
    }
    let dest_x = (u64::from(bounds.x) * percent / 100) as i32;
    let dest_y = (u64::from(bounds.y) * percent / 100) as i32;
    let dest_w = (u64::from(bounds.width) * percent / 100) as u32;
    let dest_h = (u64::from(bounds.height) * percent / 100) as u32;
    if dest_w == 0 || dest_h == 0 {
        return;
    }
    let (canvas_w, canvas_h) = (canvas.width() as i32, canvas.height() as i32);
    let canvas_stride = canvas.width();
    let data = canvas.data_mut();
    let src = source.data();
    for y in 0..dest_h {
        let src_y = (u64::from(y) * u64::from(source_h) / u64::from(dest_h)) as u32;
        for x in 0..dest_w {
            let src_x = (u64::from(x) * u64::from(source_w) / u64::from(dest_w)) as u32;
            let src_at = ((src_y * source_w + src_x) * 4) as usize;
            let (sr, sg, sb, sa) = (
                src[src_at],
                src[src_at + 1],
                src[src_at + 2],
                src[src_at + 3],
            );
            if sa == 0 {
                continue;
            }
            let (px, py) = (dest_x + x as i32, dest_y + y as i32);
            if px < 0 || py < 0 || px >= canvas_w || py >= canvas_h {
                continue;
            }
            let dst_at = ((py as u32 * canvas_stride + px as u32) * 4) as usize;
            let inv = 255 - u16::from(sa);
            // Opaque canvas: out = src + dst * (1 - src alpha).
            data[dst_at] = sr.saturating_add((u16::from(data[dst_at]) * inv / 255) as u8);
            data[dst_at + 1] = sg.saturating_add((u16::from(data[dst_at + 1]) * inv / 255) as u8);
            data[dst_at + 2] = sb.saturating_add((u16::from(data[dst_at + 2]) * inv / 255) as u8);
            data[dst_at + 3] = 255;
        }
    }
}

/// Draw one shaped line in white over the canvas.
///
/// Each glyph renders at its line size scaled by the density factor, with
/// hinting off so both packaged targets agree. Origins scale from the
/// logical baseline positions; the fractional remainder rides into the
/// shaper offset the way the shaping path's own probe does, and the
/// returned placement decides the final integer blit.
fn draw_line(
    canvas: &mut tiny_skia::Pixmap,
    context: &mut swash::scale::ScaleContext,
    primary: &swash::FontRef<'_>,
    fallback: &swash::FontRef<'_>,
    line: &crate::shaping::ShapedLine,
    percent: u64,
) -> Result<(), RasterError> {
    let scaled_size = (u64::from(line.size) * percent / 100).max(1) as f32;
    let scale = percent as f32 / 100.0;
    for glyph in &line.glyphs {
        let face = match glyph.font {
            CanonicalFont::Primary => primary,
            CanonicalFont::Fallback => fallback,
        };
        let origin_x = glyph.x as f32 * scale;
        let origin_y = glyph.y as f32 * scale;
        let floor_x = origin_x.floor() as i32;
        let floor_y = origin_y.floor() as i32;
        let offset = swash::zeno::Vector::new(origin_x - floor_x as f32, origin_y - floor_y as f32);
        let mut scaler = context.builder(*face).size(scaled_size).hint(false).build();
        let id = u16::try_from(glyph.id).unwrap_or(0);
        let Some(rendered) = swash::scale::Render::new(&[swash::scale::Source::Outline])
            .format(swash::zeno::Format::Alpha)
            .offset(offset)
            .render(&mut scaler, id)
        else {
            // No ink for this glyph (for example a space): advance only.
            continue;
        };
        if rendered.content != swash::scale::image::Content::Mask {
            continue;
        }
        // Baseline origin: left adds, top subtracts (parley blit rule).
        let dest_x = floor_x + rendered.placement.left;
        let dest_y = floor_y - rendered.placement.top;
        blend_mask(
            canvas,
            &rendered.data,
            rendered.placement.width,
            rendered.placement.height,
            dest_x,
            dest_y,
        );
    }
    Ok(())
}

/// Blend one alpha mask as white over the opaque canvas.
fn blend_mask(
    canvas: &mut tiny_skia::Pixmap,
    mask: &[u8],
    mask_w: u32,
    mask_h: u32,
    dest_x: i32,
    dest_y: i32,
) {
    if mask_w == 0 || mask_h == 0 {
        return;
    }
    let (canvas_w, canvas_h) = (canvas.width() as i32, canvas.height() as i32);
    let canvas_stride = canvas.width();
    let data = canvas.data_mut();
    for y in 0..mask_h {
        for x in 0..mask_w {
            let alpha = mask[(y * mask_w + x) as usize];
            if alpha == 0 {
                continue;
            }
            let (px, py) = (dest_x + x as i32, dest_y + y as i32);
            if px < 0 || py < 0 || px >= canvas_w || py >= canvas_h {
                continue;
            }
            let at = ((py as u32 * canvas_stride + px as u32) * 4) as usize;
            let inv = 255 - u16::from(alpha);
            // White source over opaque black: out = alpha + dst * (1 - alpha).
            let out = u16::from(alpha) + (u16::from(data[at]) * inv / 255);
            let out = out.min(255) as u8;
            data[at] = out;
            data[at + 1] = out;
            data[at + 2] = out;
            data[at + 3] = 255;
        }
    }
}
