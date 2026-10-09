//! Display-path comparison for T0.6 WP3 (Decision 261).
//!
//! Two ways to put one completed canonical [`Frame`] on screen, measured for
//! the same frame:
//!
//! - **CPU raster path:** `rendering::rasterize` to RGBA, PNG encode, decode.
//!   This is the path `canonical_views` finishes today. Its held bytes are the
//!   RGBA buffer plus the PNG bytes.
//! - **Retained path:** walk the logical `Frame` into the GPUI scene with
//!   `paint_quad` and `paint_glyph`, letting the GPUI sprite atlas cache glyph
//!   masks. Its held bytes are an estimate, documented on [`PaintTally`].
//!
//! [`verify_bundled_faces`] checks that GPUI resolves the two bundled faces to
//! the glyph ids the canonical shaper uses. Shaped glyph ids are face-intrinsic,
//! so a same-named system font would paint wrong without failing anything else.
//!
//! Bundled faces are registered by [`ui::init`] (`ui::assets`), which must run
//! before [`verify_bundled_faces`] or the retained paint; this module does not
//! register them a second time.
//!
//! The drivers that host a retained frame in a `TestAppContext` window
//! (`retained_path`, `compare_display_paths`) exist only under `cfg(test)`,
//! because `TestAppContext` is only available with GPUI's test support.

use std::{cell::Cell, collections::HashMap, fmt, rc::Rc, time::Duration, time::Instant};

use gpui::{
    App, Bounds, Font, FontId, FontWeight, GlyphId, IntoElement, Pixels, Styled, TextRun, Window,
    black, canvas, fill, point, px, white,
};
use rendering::{
    BODY_FONT_SIZE, Frame, REGULAR_WEIGHT, RasterError, TargetDensity, fonts::CanonicalFont,
};

use crate::canonical_views::finish_frame;

/// The probe string both bundled faces must resolve identically for.
pub const PROBE: &str = "Pagbasa gikan sa \u{d1} \u{101} 123";

/// Wall-clock cost and retained memory of one display path for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathMeasurement {
    /// CPU path: rasterize, encode, and decode of one frame. Retained path:
    /// median wall-clock time of one notify-to-paint cycle of the host window.
    pub latency: Duration,
    /// CPU path: RGBA bytes plus PNG bytes. Retained path: estimated glyph-mask
    /// bytes (see [`PaintTally`]).
    pub held_bytes: usize,
}

/// Structured failures of the display-path measurements.
#[derive(Debug)]
pub enum DisplayPathError {
    Rasterize(RasterError),
    Finish,
    Decode,
    Paint(String),
}

impl fmt::Display for DisplayPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rasterize(error) => write!(f, "CPU rasterize failed: {error}"),
            Self::Finish => write!(f, "CPU raster did not finish into PNG bytes"),
            Self::Decode => write!(f, "finished PNG bytes did not decode"),
            Self::Paint(message) => write!(f, "retained paint failed: {message}"),
        }
    }
}

impl std::error::Error for DisplayPathError {}

/// CPU raster path for one frame at one density.
///
/// The timed span covers rasterize, the viewport finish (row padding and PNG
/// encode, shared with `canonical_views`), and the decode back to pixels that
/// an image upload would consume. Frame composition is not timed.
pub fn cpu_path(
    frame: &Frame,
    density: TargetDensity,
) -> Result<PathMeasurement, DisplayPathError> {
    let started = Instant::now();
    let raster = rendering::rasterize(frame, density, &HashMap::new())
        .map_err(DisplayPathError::Rasterize)?;
    let rgba_bytes = raster.rgba().len();
    let finished = finish_frame(&raster).ok_or(DisplayPathError::Finish)?;
    let png = finished.image().bytes();
    let decoded = tiny_skia::Pixmap::decode_png(png).map_err(|_| DisplayPathError::Decode)?;
    let latency = started.elapsed();
    std::hint::black_box(&decoded);
    Ok(PathMeasurement {
        latency,
        held_bytes: rgba_bytes + png.len(),
    })
}

/// Counters written by the most recent retained paint.
///
/// Each paint resets the counters, so they describe the last draw only.
///
/// `mask_bytes` is an estimate. For every placed glyph it adds one em square
/// of device pixels at one byte per pixel (`ceil(font_px)^2`), which is the
/// size of the monochrome alpha mask GPUI's sprite atlas would hold for it.
/// Ink boxes are usually smaller than the em square, and the atlas stores one
/// mask per distinct glyph, face, size, and subpixel variant, so this is an
/// upper bound on atlas bytes per frame. It is not a measured atlas size.
#[derive(Debug, Default)]
pub struct PaintTally {
    draws: Cell<usize>,
    quads: Cell<usize>,
    glyphs: Cell<usize>,
    failed: Cell<usize>,
    mask_bytes: Cell<usize>,
}

impl PaintTally {
    fn begin_draw(&self) {
        self.draws.set(self.draws.get() + 1);
        self.quads.set(0);
        self.glyphs.set(0);
        self.failed.set(0);
        self.mask_bytes.set(0);
    }

    /// Number of draws that have started since this tally was created.
    pub fn draws(&self) -> usize {
        self.draws.get()
    }

    /// Glyphs the last draw placed without error.
    pub fn glyphs(&self) -> usize {
        self.glyphs.get()
    }

    /// Glyphs the last draw could not paint.
    pub fn failed(&self) -> usize {
        self.failed.get()
    }

    /// Estimated mask bytes for the glyphs the last draw placed.
    pub fn mask_bytes(&self) -> usize {
        self.mask_bytes.get()
    }
}

/// The GPUI family name the bundled face is registered under by `ui::init`.
pub fn family_name(font: CanonicalFont) -> &'static str {
    match font {
        CanonicalFont::Primary => "Atkinson Hyperlegible Next",
        CanonicalFont::Fallback => "Noto Sans",
    }
}

/// The GPUI font for a bundled face at one weight.
pub fn bundled_font(font: CanonicalFont, weight: u16) -> Font {
    let mut resolved = gpui::font(family_name(font));
    resolved.weight = FontWeight(f32::from(weight));
    resolved
}

/// Total glyph placements in a frame's header, body, and footer lines.
pub fn glyph_count(frame: &Frame) -> usize {
    frame
        .header_lines()
        .iter()
        .chain(frame.body_lines())
        .chain(frame.footer_lines())
        .map(|line| line.glyphs.len())
        .sum()
}

/// A canvas element that paints one frame into its bounds. Glyph origins are
/// the canonical logical baselines scaled to the bounds, so a 1920x1080
/// canvas paints at the logical positions and a smaller one scales with it.
pub fn retained_frame(frame: Rc<Frame>, tally: Rc<PaintTally>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| paint_frame(&frame, bounds, &tally, window, cx),
    )
    .size_full()
}

fn paint_frame(
    frame: &Frame,
    bounds: Bounds<Pixels>,
    tally: &PaintTally,
    window: &mut Window,
    cx: &mut App,
) {
    tally.begin_draw();
    window.paint_quad(fill(bounds, black()));
    tally.quads.set(tally.quads.get() + 1);

    let (logical_width, logical_height) = frame.logical_size();
    let scale_x = f32::from(bounds.size.width) / logical_width as f32;
    let scale_y = f32::from(bounds.size.height) / logical_height as f32;
    let device_scale = window.scale_factor();
    let mut font_ids: HashMap<(&'static str, u16), FontId> = HashMap::new();

    for line in frame
        .header_lines()
        .iter()
        .chain(frame.body_lines())
        .chain(frame.footer_lines())
    {
        let font_size = px(line.size as f32 * scale_x);
        let device_px = (f32::from(font_size) * device_scale).ceil() as usize;
        for glyph in &line.glyphs {
            let family = family_name(glyph.font);
            let font_id = *font_ids.entry((family, line.weight)).or_insert_with(|| {
                cx.text_system()
                    .resolve_font(&bundled_font(glyph.font, line.weight))
            });
            let origin = point(
                bounds.origin.x + px(glyph.x as f32 * scale_x),
                bounds.origin.y + px(glyph.y as f32 * scale_y),
            );
            match window.paint_glyph(origin, font_id, GlyphId(glyph.id), font_size, white()) {
                Ok(()) => tally.glyphs.set(tally.glyphs.get() + 1),
                Err(_) => tally.failed.set(tally.failed.get() + 1),
            }
            tally
                .mask_bytes
                .set(tally.mask_bytes.get() + device_px * device_px);
        }
    }
}

/// One face that GPUI resolved to the canonical glyph ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceEvidence {
    pub family: &'static str,
    /// Probe characters checked against this face.
    pub characters: usize,
    /// GPUI font id the family resolved to.
    pub font_id: usize,
}

/// Both bundled faces, verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceReport {
    pub faces: Vec<FaceEvidence>,
}

/// The first probe character where GPUI and the canonical shaper disagree,
/// or where the bundled bytes do not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceMismatch {
    pub family: &'static str,
    pub character: char,
    /// Glyph id the canonical shaper assigns in the bundled face (0 when the
    /// bytes do not parse).
    pub expected_glyph: u32,
    /// Glyph id GPUI assigned, or `None` when GPUI produced no glyph.
    pub actual_glyph: Option<u32>,
    pub reason: String,
}

impl fmt::Display for FaceMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bundled face {} did not resolve through GPUI: {} (character {:?}: canonical glyph {}, GPUI glyph {}); a same-named system font would mispaint",
            self.family,
            self.reason,
            self.character,
            self.expected_glyph,
            self.actual_glyph
                .map_or_else(|| "none".to_owned(), |glyph| glyph.to_string()),
        )
    }
}

impl std::error::Error for FaceMismatch {}

/// Check that GPUI resolves the bundled Atkinson Hyperlegible Next and Noto
/// Sans faces to the glyph ids the canonical shaper uses for [`PROBE`].
///
/// The canonical ids come from `rustybuzz` over the bundled bytes, the same
/// shaper `rendering` uses. The GPUI ids come from `shape_line` per character,
/// because `glyph_for_char` is not public on the pinned `TextSystem`. Per-run
/// font ids are also compared against the resolved face, so a run that GPUI
/// shapes with a fallback font is reported even when its glyph id happens to
/// match.
///
/// Requires [`ui::init`] to have registered the bundled faces. Under the GPUI
/// test platform the text system is fake and this returns a mismatch.
///
/// Takes a `Window` because GPUI only exposes glyph-level shaping on the
/// window's text system; `cx` supplies the font resolution.
pub fn verify_bundled_faces(window: &Window, cx: &App) -> Result<FaceReport, FaceMismatch> {
    let faces = [CanonicalFont::Primary, CanonicalFont::Fallback]
        .into_iter()
        .map(|font| verify_face(window, cx, font))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FaceReport { faces })
}

fn verify_face(
    window: &Window,
    cx: &App,
    font: CanonicalFont,
) -> Result<FaceEvidence, FaceMismatch> {
    let family = family_name(font);
    let mismatch =
        |character: char, expected_glyph: u32, actual_glyph: Option<u32>, reason: String| {
            FaceMismatch {
                family,
                character,
                expected_glyph,
                actual_glyph,
                reason,
            }
        };
    let canonical = rustybuzz::Face::from_slice(font.bytes(), 0).ok_or_else(|| {
        mismatch(
            '\0',
            0,
            None,
            "the bundled bytes do not parse as a font".to_owned(),
        )
    })?;

    let gpui_font = bundled_font(font, REGULAR_WEIGHT);
    let resolved = cx.text_system().resolve_font(&gpui_font);
    let shaper = window.text_system();
    let mut characters = 0;

    for character in PROBE.chars() {
        let expected = canonical_glyph(&canonical, character).ok_or_else(|| {
            mismatch(
                character,
                0,
                None,
                "the canonical shaper did not produce one glyph".to_owned(),
            )
        })?;
        let text: String = character.to_string();
        let shaped = shaper.shape_line(
            text.clone().into(),
            px(BODY_FONT_SIZE as f32),
            &[TextRun {
                len: text.len(),
                font: gpui_font.clone(),
                color: white(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let runs = &shaped.runs;
        if let Some(run) = runs.iter().find(|run| run.font_id != resolved) {
            return Err(mismatch(
                character,
                expected,
                run.glyphs.first().map(|glyph| glyph.id.0),
                format!(
                    "GPUI shaped the character with font {} instead of the resolved {}",
                    run.font_id.0, resolved.0
                ),
            ));
        }
        let actual = runs
            .iter()
            .flat_map(|run| run.glyphs.iter())
            .map(|glyph| glyph.id.0)
            .next();
        if actual != Some(expected) {
            return Err(mismatch(
                character,
                expected,
                actual,
                "glyph ids differ".to_owned(),
            ));
        }
        characters += 1;
    }

    Ok(FaceEvidence {
        family,
        characters,
        font_id: resolved.0,
    })
}

/// The glyph id the canonical shaper assigns to one character, or `None`
/// when shaping does not yield exactly one glyph.
fn canonical_glyph(face: &rustybuzz::Face<'_>, character: char) -> Option<u32> {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(&character.to_string());
    let shaped = rustybuzz::shape(face, &[], buffer);
    let infos = shaped.glyph_infos();
    (infos.len() == 1).then(|| infos[0].glyph_id)
}

/// Both display paths for one frame plus the face check, ready to print.
#[derive(Debug)]
pub struct DisplayPathComparison {
    pub cpu: PathMeasurement,
    pub retained: PathMeasurement,
    pub faces: Result<FaceReport, FaceMismatch>,
}

impl DisplayPathComparison {
    /// One line per measurement, suitable for logs and the Diagnostics panel.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "CPU raster path: {:.2} ms, {} KiB held (RGBA + PNG)",
                millis(self.cpu.latency),
                kib(self.cpu.held_bytes)
            ),
            format!(
                "Retained scene path: {:.2} ms per paint, ~{} KiB held (glyph-mask estimate)",
                millis(self.retained.latency),
                kib(self.retained.held_bytes)
            ),
        ];
        lines.push(match &self.faces {
            Ok(report) => format!(
                "Bundled faces: {} resolve to the canonical glyph ids",
                report
                    .faces
                    .iter()
                    .map(|face| face.family)
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            Err(mismatch) => format!("Bundled faces: MISMATCH - {mismatch}"),
        });
        lines
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn kib(bytes: usize) -> usize {
    bytes.div_ceil(1024)
}

/// Test-window drivers. `TestAppContext` exists only with GPUI's test support,
/// so these are compiled for tests alone; app code reaches the retained paint
/// through [`retained_frame`] inside its own window.
#[cfg(test)]
mod harness {
    use std::{rc::Rc, time::Instant};

    use gpui::{
        AppContext as _, Context, ParentElement, Pixels, Render, Size, Styled, TestAppContext,
        VisualContext as _, Window, div, px,
    };
    use rendering::{Frame, LOGICAL_HEIGHT, LOGICAL_WIDTH, TargetDensity};

    use super::{
        DisplayPathComparison, DisplayPathError, FaceMismatch, FaceReport, PaintTally,
        PathMeasurement, cpu_path, glyph_count, retained_frame, verify_bundled_faces,
    };

    /// Repeated retained paints; the first pass warms caches and is discarded.
    const PASSES: usize = 5;

    struct RetainedHost {
        frame: Rc<Frame>,
        tally: Rc<PaintTally>,
        viewport: Size<Pixels>,
    }

    impl Render for RetainedHost {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div()
                .w(self.viewport.width)
                .h(self.viewport.height)
                .child(retained_frame(self.frame.clone(), self.tally.clone()))
        }
    }

    /// Retained path for one frame in a test window of the given logical viewport.
    ///
    /// Latency is the median of [`PASSES`] notify-to-paint cycles of a host
    /// window that holds only the frame element, after one discarded warm-up
    /// pass. Each pass is checked to have drawn the frame, so a skipped paint is
    /// an error, not a fast result.
    pub fn retained_path(
        cx: &mut TestAppContext,
        frame: &Frame,
        viewport: Size<Pixels>,
    ) -> Result<PathMeasurement, DisplayPathError> {
        let tally = Rc::new(PaintTally::default());
        let host = RetainedHost {
            frame: Rc::new(frame.clone()),
            tally: tally.clone(),
            viewport,
        };
        let (view, window_cx) = cx.add_window_view(move |_, _| host);
        let expected = glyph_count(frame);
        let mut samples = Vec::with_capacity(PASSES);

        for pass in 0..=PASSES {
            let draws_before = tally.draws();
            let started = Instant::now();
            view.update(window_cx, |_, cx| cx.notify());
            window_cx.run_until_parked();
            let elapsed = started.elapsed();

            if tally.draws() == draws_before {
                return Err(DisplayPathError::Paint(
                    "the host window did not repaint after notify".to_owned(),
                ));
            }
            if tally.failed() > 0 {
                return Err(DisplayPathError::Paint(format!(
                    "{} glyphs could not be painted",
                    tally.failed()
                )));
            }
            if tally.glyphs() != expected {
                return Err(DisplayPathError::Paint(format!(
                    "painted {} glyphs, the frame places {expected}",
                    tally.glyphs()
                )));
            }
            if pass > 0 {
                samples.push(elapsed);
            }
        }

        samples.sort_unstable();
        Ok(PathMeasurement {
            latency: samples[samples.len() / 2],
            held_bytes: tally.mask_bytes(),
        })
    }

    /// Run [`verify_bundled_faces`] inside a fresh test window.
    pub fn verify_in_window(cx: &mut TestAppContext) -> Result<FaceReport, FaceMismatch> {
        let window_cx = cx.add_empty_window();
        let handle = window_cx.window_handle();
        window_cx
            .update_window(handle, |_, window, app| verify_bundled_faces(window, app))
            .expect("the empty test window must stay live")
    }

    /// Measure both paths for one frame and check the bundled faces.
    ///
    /// The retained path uses the logical 1920x1080 canvas as its viewport, so
    /// its glyph origins match the canonical logical positions.
    pub fn compare_display_paths(
        cx: &mut TestAppContext,
        frame: &Frame,
        density: TargetDensity,
    ) -> Result<DisplayPathComparison, DisplayPathError> {
        let cpu = cpu_path(frame, density)?;
        let viewport = Size {
            width: px(LOGICAL_WIDTH as f32),
            height: px(LOGICAL_HEIGHT as f32),
        };
        let retained = retained_path(cx, frame, viewport)?;
        let faces = verify_in_window(cx);
        Ok(DisplayPathComparison {
            cpu,
            retained,
            faces,
        })
    }

    /// The 1920x1080 logical canvas as a viewport.
    pub fn logical_viewport() -> Size<Pixels> {
        Size {
            width: px(LOGICAL_WIDTH as f32),
            height: px(LOGICAL_HEIGHT as f32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::harness::{
        compare_display_paths, logical_viewport, retained_path, verify_in_window,
    };
    use super::*;
    use gpui::TestAppContext;
    use rendering::{LOGICAL_HEIGHT, LOGICAL_WIDTH, TemplateContent, TemplateKind};

    /// The canonical composition of the mixed-language fixture used across
    /// the T0.4 viewport tests, plus the probe characters.
    fn fixture_frame() -> Frame {
        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some("Bersikulo".to_owned()),
            body: format!(
                "Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo \u{2014} awit 95\n{PROBE}"
            ),
            footer: None,
            image: None,
        };
        rendering::render_template(
            &content,
            &rendering::Theme::default(),
            TargetDensity::SCALE_100,
        )
        .expect("fixture must compose")
        .0
    }

    const RGBA_1080P: usize = (LOGICAL_WIDTH as usize) * (LOGICAL_HEIGHT as usize) * 4;

    #[test]
    fn cpu_path_measures_latency_and_holds_rgba_and_png() {
        let frame = fixture_frame();
        let measured = cpu_path(&frame, TargetDensity::SCALE_100).expect("CPU path must run");
        assert!(!measured.latency.is_zero(), "latency must be measured");
        assert!(
            measured.held_bytes > RGBA_1080P,
            "held bytes must include the full RGBA frame plus PNG bytes: {}",
            measured.held_bytes
        );
    }

    #[gpui::test]
    fn retained_path_paints_every_glyph_and_holds_less_than_rgba(cx: &mut TestAppContext) {
        cx.update(ui::init).unwrap();
        let frame = fixture_frame();
        let measured =
            retained_path(cx, &frame, logical_viewport()).expect("retained path must paint");
        assert!(measured.held_bytes > 0, "mask estimate must be non-zero");
        assert!(
            measured.held_bytes < RGBA_1080P,
            "retained glyph masks ({} bytes) must undercut the RGBA frame ({RGBA_1080P})",
            measured.held_bytes
        );
    }

    #[gpui::test]
    fn comparison_reports_both_paths_for_one_frame(cx: &mut TestAppContext) {
        cx.update(ui::init).unwrap();
        let comparison = compare_display_paths(cx, &fixture_frame(), TargetDensity::SCALE_100)
            .expect("both paths must run");
        assert!(comparison.retained.held_bytes < RGBA_1080P);
        assert_eq!(comparison.summary_lines().len(), 3);
    }

    #[gpui::test]
    fn test_text_system_is_reported_as_a_face_mismatch(cx: &mut TestAppContext) {
        // The GPUI test platform answers every character with a synthetic
        // glyph id, so the check must refuse it rather than pass vacuously.
        cx.update(ui::init).unwrap();
        let mismatch = verify_in_window(cx).expect_err("the fake text system must not verify");
        assert_eq!(mismatch.family, "Atkinson Hyperlegible Next");
        assert!(mismatch.to_string().contains("Atkinson Hyperlegible Next"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn bundled_faces_resolve_to_canonical_glyph_ids_on_native_text_system() {
        // Needs the real DirectWrite text system: the GPUI test platform's
        // glyph ids are synthetic, so only the native platform can prove the
        // bundled faces are what GPUI resolves. The platform must not be
        // headless: a headless DirectWrite enumerates no system fonts and
        // shapes every family to one placeholder glyph, so the check could
        // never pass there. Constructing the platform opens no window.
        let text_system = gpui_platform::current_platform(false).text_system();
        let mut cx =
            TestAppContext::build_with_text_system(gpui::TestDispatcher::new(0), None, text_system);
        cx.update(ui::init).unwrap();
        let report = verify_in_window(&mut cx).unwrap_or_else(|mismatch| panic!("{mismatch}"));
        assert_eq!(report.faces.len(), 2);
        for face in &report.faces {
            assert_eq!(face.characters, PROBE.chars().count());
        }
    }

    #[test]
    fn summary_names_a_face_mismatch() {
        let measurement = PathMeasurement {
            latency: Duration::from_millis(2),
            held_bytes: 4096,
        };
        let comparison = DisplayPathComparison {
            cpu: measurement,
            retained: measurement,
            faces: Err(FaceMismatch {
                family: "Noto Sans",
                character: 'P',
                expected_glyph: 36,
                actual_glyph: Some(1),
                reason: "glyph ids differ".to_owned(),
            }),
        };
        let lines = comparison.summary_lines();
        assert_eq!(lines.len(), 3);
        assert!(lines[2].contains("MISMATCH") && lines[2].contains("Noto Sans"));
    }
}
