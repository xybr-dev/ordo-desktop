//! Native-window display-path comparison for T0.6 WP6.
//!
//! [`super::display_path`] compares the CPU raster path and the retained path
//! on the GPUI test platform. This module runs the same two paths in a real
//! native window, so the timed span includes the frame loop, the image upload
//! or glyph paint, and the presentation of the frame.
//!
//! [`DisplayPathProbe`] shows one canonical [`Frame`] in one of two modes:
//!
//! - **CPU:** each pass rasterizes, finishes, and decodes the frame on the GPUI
//!   thread (the [`cpu_path`](super::display_path::cpu_path) span), wraps the
//!   PNG bytes as a fresh `gpui::Image`, and shows it through `img`. Each pass
//!   uses a new image id, so GPUI decodes and uploads it again every time.
//! - **Retained:** the probe shows [`retained_frame`], which paints the logical
//!   frame straight into the scene. Nothing is rasterized per pass.
//!
//! [`measure`] alternates the two modes after one warm-up pass each. Every pass
//! starts its clock, notifies the probe, and stops the clock in a
//! `Window::on_next_frame` callback that runs after the frame showing that pass
//! has been drawn. Passes are chained through `on_next_frame`; nothing sleeps
//! or blocks the GPUI thread between passes. The CPU work itself runs on the
//! GPUI thread on purpose, because the measured span includes it.

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Write as _,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{
    App, Context, Entity, Image, ImageFormat, IntoElement, ObjectFit, ParentElement, Render,
    Styled, StyledImage, Window, div, img, px,
};
use rendering::{
    Frame, TargetDensity, TemplateContent, TemplateKind, Theme, rasterize, render_template,
};

use super::{
    display_path::{DisplayPathError, PROBE, PaintTally, glyph_count, retained_frame},
    report::RendererInfo,
};
use crate::canonical_views::finish_frame;

/// Both paths fill this fixed 16:9 box. Glyph and image scaling then match,
/// and the window size does not change what is measured.
const VIEWPORT_WIDTH: f32 = 1280.0;
const VIEWPORT_HEIGHT: f32 = 720.0;

/// Frames a pass may wait for its frame to present before it fails. A CPU
/// image decodes off the GPUI thread, so the wait is real and bounded.
const MAX_PRESENT_FRAMES: usize = 600;

/// Source of image ids for CPU passes. GPUI caches decoded images by id, so a
/// fresh id per pass forces the decode and upload a new frame would pay.
static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Which display path the probe shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathMode {
    Cpu,
    Retained,
}

/// Median latency and held bytes for one display path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathTiming {
    /// Median notify-to-presented time over the measured passes.
    pub median: Duration,
    /// CPU path: RGBA bytes plus PNG bytes. Retained path: the glyph-mask
    /// estimate from the last draw (see [`PaintTally`]).
    pub held_bytes: usize,
}

/// Both paths for one canonical frame, measured in a native window.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeDisplayPathResult {
    /// Measured passes per path, excluding the warm-up pass.
    pub passes: usize,
    pub cpu: PathTiming,
    pub retained: PathTiming,
    /// Renderer identity from `Window::gpu_specs`. Fields are `None` when the
    /// platform does not report them.
    pub renderer: RendererInfo,
}

impl NativeDisplayPathResult {
    /// The `"display_path"` JSON object, as a complete JSON document.
    ///
    /// Latency is in milliseconds with three decimals. `software_emulated` and
    /// `device` are `null` when the platform did not report them.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\"display_path\":{");
        let _ = write!(
            out,
            "\"scope\":\"native_window_notify_to_presented_frame\",\"passes\":{}",
            self.passes
        );
        let _ = write!(
            out,
            ",\"cpu\":{{\"median_ms\":{:.3},\"held_bytes\":{}}}",
            millis(self.cpu.median),
            self.cpu.held_bytes
        );
        let _ = write!(
            out,
            ",\"retained\":{{\"median_ms\":{:.3},\"held_bytes\":{}}}",
            millis(self.retained.median),
            self.retained.held_bytes
        );
        let software = match self.renderer.software_emulated {
            Some(true) => "true",
            Some(false) => "false",
            None => "null",
        };
        let _ = write!(out, ",\"software_emulated\":{software}");
        out.push_str(",\"device\":");
        match &self.renderer.device_name {
            Some(name) => out.push_str(&json_string(name)),
            None => out.push_str("null"),
        }
        out.push_str("}}");
        out
    }

    /// One line per fact, suitable for logs and the Diagnostics panel.
    pub fn summary_lines(&self) -> Vec<String> {
        let renderer = match (
            self.renderer.device_name.as_deref(),
            self.renderer.software_emulated,
        ) {
            (Some(name), Some(true)) => format!("{name} (software emulated)"),
            (Some(name), Some(false)) => format!("{name} (hardware)"),
            (Some(name), None) => name.to_owned(),
            (None, _) => "unknown".to_owned(),
        };
        vec![
            format!(
                "Native display path, notify to presented frame ({} passes after warm-up)",
                self.passes
            ),
            format!(
                "CPU raster path: {:.2} ms median, {} KiB held (RGBA + PNG)",
                millis(self.cpu.median),
                kib(self.cpu.held_bytes)
            ),
            format!(
                "Retained scene path: {:.2} ms median, ~{} KiB held (glyph-mask estimate)",
                millis(self.retained.median),
                kib(self.retained.held_bytes)
            ),
            format!("Renderer: {renderer}"),
        ]
    }
}

/// A view that shows one canonical frame through one display path at a time.
pub struct DisplayPathProbe {
    frame: Rc<Frame>,
    tally: Rc<PaintTally>,
    mode: PathMode,
    /// The CPU-path image of the latest CPU pass, if any.
    image: Option<Arc<Image>>,
    /// Frames rendered since creation. A pass waits for a render newer than
    /// its own start.
    renders: usize,
    /// Whether the latest render found the CPU image decoded and ready.
    image_ready: bool,
}

impl Default for DisplayPathProbe {
    /// A probe for the fixed fixture frame, shown in CPU mode with no image.
    fn default() -> Self {
        Self {
            frame: Rc::new(fixture_frame()),
            tally: Rc::new(PaintTally::default()),
            mode: PathMode::Cpu,
            image: None,
            renders: 0,
            image_ready: false,
        }
    }
}

impl DisplayPathProbe {
    /// Switch to `mode` for the next pass and notify the probe.
    ///
    /// The CPU pass does its raster, finish, and decode work here, before the
    /// notify. Returns the bytes the path holds for this frame.
    fn begin_pass(
        &mut self,
        mode: PathMode,
        cx: &mut Context<Self>,
    ) -> Result<usize, DisplayPathError> {
        self.mode = mode;
        let held = match mode {
            PathMode::Cpu => {
                let (image, held) = cpu_pass(&self.frame)?;
                if let Some(previous) = self.image.replace(image) {
                    previous.remove_asset(cx);
                }
                held
            }
            PathMode::Retained => {
                if let Some(previous) = self.image.take() {
                    previous.remove_asset(cx);
                }
                0
            }
        };
        cx.notify();
        Ok(held)
    }
}

impl Render for DisplayPathProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let content = match (self.mode, &self.image) {
            (PathMode::Cpu, Some(image)) => {
                // `get_render_image` is the non-blocking readiness check: it is
                // `None` until GPUI has decoded the image off the GPUI thread.
                self.image_ready = image.clone().get_render_image(window, cx).is_some();
                img(image.clone())
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            }
            (PathMode::Cpu, None) => {
                self.image_ready = false;
                div().size_full().into_any_element()
            }
            (PathMode::Retained, _) => {
                self.image_ready = false;
                retained_frame(self.frame.clone(), self.tally.clone()).into_any_element()
            }
        };
        div()
            .w(px(VIEWPORT_WIDTH))
            .h(px(VIEWPORT_HEIGHT))
            .child(content)
    }
}

/// Measure both paths in the probe's window: one warm-up pass per path, then
/// `passes` measured passes per path, alternating CPU and retained.
///
/// `on_done` runs once, from a frame callback, with the medians or the first
/// failure. `passes` is raised to at least one. The first pass starts from the
/// next frame, so this returns before any work is done.
pub fn measure(
    probe: &Entity<DisplayPathProbe>,
    window: &mut Window,
    cx: &mut App,
    passes: usize,
    on_done: impl FnOnce(Result<NativeDisplayPathResult, DisplayPathError>) + 'static,
) {
    let passes = passes.max(1);
    let (tally, expected_glyphs) = {
        let probe = probe.read(cx);
        (probe.tally.clone(), glyph_count(&probe.frame))
    };
    let mut plan = vec![
        Pass {
            mode: PathMode::Cpu,
            warm_up: true,
        },
        Pass {
            mode: PathMode::Retained,
            warm_up: true,
        },
    ];
    for _ in 0..passes {
        plan.push(Pass {
            mode: PathMode::Cpu,
            warm_up: false,
        });
        plan.push(Pass {
            mode: PathMode::Retained,
            warm_up: false,
        });
    }
    let run = Rc::new(RefCell::new(Run {
        probe: probe.clone(),
        tally,
        expected_glyphs,
        plan,
        next: 0,
        clock: None,
        cpu_samples: Vec::with_capacity(passes),
        retained_samples: Vec::with_capacity(passes),
        cpu_held: 0,
        retained_held: 0,
        renderer: renderer_info(window),
        on_done: Some(Box::new(on_done)),
    }));
    window.on_next_frame(move |window, cx| start_pass(&run, window, cx));
}

/// One step of the measurement plan.
#[derive(Clone, Copy)]
struct Pass {
    mode: PathMode,
    warm_up: bool,
}

/// The clock of the pass in flight.
struct Clock {
    started: Instant,
    renders_before: usize,
    draws_before: usize,
    held: usize,
    waited_frames: usize,
}

struct Run {
    probe: Entity<DisplayPathProbe>,
    tally: Rc<PaintTally>,
    expected_glyphs: usize,
    plan: Vec<Pass>,
    next: usize,
    clock: Option<Clock>,
    cpu_samples: Vec<Duration>,
    retained_samples: Vec<Duration>,
    cpu_held: usize,
    retained_held: usize,
    renderer: RendererInfo,
    on_done: Option<OnDone>,
}

/// The caller's completion callback, taken out of the run before it is called.
type OnDone = Box<dyn FnOnce(Result<NativeDisplayPathResult, DisplayPathError>)>;

/// Start the next pass in the plan, or finish when the plan is done.
///
/// Always called from a frame callback, so the settle callback registered
/// here runs at the start of the next frame, after this pass's frame is drawn.
fn start_pass(run: &Rc<RefCell<Run>>, window: &mut Window, cx: &mut App) {
    let (probe, pass) = {
        let state = run.borrow();
        (state.probe.clone(), state.plan.get(state.next).copied())
    };
    let Some(pass) = pass else {
        let result = {
            let state = run.borrow();
            NativeDisplayPathResult {
                passes: state.cpu_samples.len(),
                cpu: timing(&state.cpu_samples, state.cpu_held),
                retained: timing(&state.retained_samples, state.retained_held),
                renderer: state.renderer.clone(),
            }
        };
        finish(run, Ok(result));
        return;
    };

    let started = Instant::now();
    let held = match probe.update(cx, |probe, cx| probe.begin_pass(pass.mode, cx)) {
        Ok(held) => held,
        Err(error) => return finish(run, Err(error)),
    };
    let (renders_before, draws_before) = (probe.read(cx).renders, run.borrow().tally.draws());
    run.borrow_mut().clock = Some(Clock {
        started,
        renders_before,
        draws_before,
        held,
        waited_frames: 0,
    });
    let run = run.clone();
    window.on_next_frame(move |window, cx| settle(&run, window, cx));
}

/// Check whether the pass in flight has presented its frame. Runs at the start
/// of the frame after the pass's notify was drawn.
fn settle(run: &Rc<RefCell<Run>>, window: &mut Window, cx: &mut App) {
    let (probe, tally, expected_glyphs, mode, warm_up, clock) = {
        let mut state = run.borrow_mut();
        let Some(clock) = state.clock.take() else {
            return;
        };
        let pass = state.plan[state.next];
        (
            state.probe.clone(),
            state.tally.clone(),
            state.expected_glyphs,
            pass.mode,
            pass.warm_up,
            clock,
        )
    };

    let (renders, image_ready) = {
        let probe = probe.read(cx);
        (probe.renders, probe.image_ready)
    };
    let presented = match mode {
        PathMode::Cpu => {
            if renders > clock.renders_before && image_ready {
                Ok(Some(clock.held))
            } else {
                Ok(None)
            }
        }
        PathMode::Retained => {
            if renders > clock.renders_before && tally.draws() > clock.draws_before {
                if tally.failed() > 0 {
                    Err(DisplayPathError::Paint(format!(
                        "{} glyphs could not be painted",
                        tally.failed()
                    )))
                } else if tally.glyphs() != expected_glyphs {
                    Err(DisplayPathError::Paint(format!(
                        "painted {} glyphs, the frame places {expected_glyphs}",
                        tally.glyphs()
                    )))
                } else {
                    Ok(Some(tally.mask_bytes()))
                }
            } else {
                Ok(None)
            }
        }
    };

    match presented {
        Err(error) => finish(run, Err(error)),
        Ok(Some(held)) => {
            let elapsed = clock.started.elapsed();
            {
                let mut state = run.borrow_mut();
                match mode {
                    PathMode::Cpu => {
                        state.cpu_held = held;
                        if !warm_up {
                            state.cpu_samples.push(elapsed);
                        }
                    }
                    PathMode::Retained => {
                        state.retained_held = held;
                        if !warm_up {
                            state.retained_samples.push(elapsed);
                        }
                    }
                }
                state.next += 1;
            }
            start_pass(run, window, cx);
        }
        Ok(None) => {
            let mut clock = clock;
            clock.waited_frames += 1;
            if clock.waited_frames > MAX_PRESENT_FRAMES {
                return finish(
                    run,
                    Err(DisplayPathError::Paint(
                        "the frame was not presented within the wait limit".to_owned(),
                    )),
                );
            }
            run.borrow_mut().clock = Some(clock);
            // Nothing else may dirty the window while the image decodes, so
            // notify it to draw the next frame, then wait for that frame.
            probe.update(cx, |_, cx| cx.notify());
            let run = run.clone();
            window.on_next_frame(move |window, cx| settle(&run, window, cx));
        }
    }
}

fn finish(run: &Rc<RefCell<Run>>, result: Result<NativeDisplayPathResult, DisplayPathError>) {
    // Take the callback out before calling it, so it may touch the run.
    let on_done = run.borrow_mut().on_done.take();
    if let Some(on_done) = on_done {
        on_done(result);
    }
}

/// CPU raster path for one frame at the canonical density. Mirrors the span of
/// [`super::display_path::cpu_path`]: rasterize, finish, and decode. Returns the
/// GPUI image of the finished PNG bytes and the bytes held (RGBA plus PNG).
fn cpu_pass(frame: &Frame) -> Result<(Arc<Image>, usize), DisplayPathError> {
    let raster = rasterize(frame, TargetDensity::SCALE_100, &HashMap::new())
        .map_err(DisplayPathError::Rasterize)?;
    let rgba_bytes = raster.rgba().len();
    let finished = finish_frame(&raster).ok_or(DisplayPathError::Finish)?;
    let png = finished.image().bytes();
    let decoded = tiny_skia::Pixmap::decode_png(png).map_err(|_| DisplayPathError::Decode)?;
    std::hint::black_box(&decoded);
    let held = rgba_bytes + png.len();
    let image = Image {
        format: ImageFormat::Png,
        bytes: png.to_vec(),
        id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
    };
    Ok((Arc::new(image), held))
}

/// The fixed fixture: the mixed-language composition the display-path tests
/// use, plus the probe characters. It is composed here rather than shared
/// because those tests are private to `display_path`.
fn fixture_frame() -> Frame {
    let content = TemplateContent {
        kind: TemplateKind::Lyrics,
        header: Some("Bersikulo".to_owned()),
        body: format!("Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo \u{2014} awit 95\n{PROBE}"),
        footer: None,
        image: None,
    };
    render_template(&content, &Theme::default(), TargetDensity::SCALE_100)
        .expect("the fixture composes")
        .0
}

fn renderer_info(window: &Window) -> RendererInfo {
    window
        .gpu_specs()
        .map_or_else(RendererInfo::default, |specs| RendererInfo {
            software_emulated: Some(specs.is_software_emulated),
            device_name: (!specs.device_name.is_empty()).then_some(specs.device_name),
        })
}

fn timing(samples: &[Duration], held_bytes: usize) -> PathTiming {
    let mut samples = samples.to_vec();
    PathTiming {
        median: median(&mut samples).unwrap_or_default(),
        held_bytes,
    }
}

/// The median of `samples`: the middle sample when the count is odd, and the
/// upper of the two middle samples when it is even. `None` when empty.
fn median(samples: &mut [Duration]) -> Option<Duration> {
    samples.sort_unstable();
    samples.get(samples.len() / 2).copied()
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn kib(bytes: usize) -> usize {
    bytes.div_ceil(1024)
}

/// Quotes and escapes `value` as a JSON string.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(renderer: RendererInfo) -> NativeDisplayPathResult {
        NativeDisplayPathResult {
            passes: 5,
            cpu: PathTiming {
                median: Duration::from_micros(2_500),
                held_bytes: 100,
            },
            retained: PathTiming {
                median: Duration::from_micros(1_250),
                held_bytes: 4096,
            },
            renderer,
        }
    }

    #[test]
    fn median_selects_the_middle_sample() {
        let ms = Duration::from_millis;
        assert_eq!(median(&mut [ms(3), ms(1), ms(2)]), Some(ms(2)));
        assert_eq!(median(&mut [ms(9)]), Some(ms(9)));
        // Even counts take the upper middle, as `display_path` does.
        assert_eq!(median(&mut [ms(4), ms(1), ms(3), ms(2)]), Some(ms(3)));
        assert_eq!(median(&mut []), None);
    }

    #[test]
    fn timing_of_no_samples_is_zero_and_keeps_held_bytes() {
        let timing = timing(&[], 512);
        assert_eq!(timing.median, Duration::ZERO);
        assert_eq!(timing.held_bytes, 512);
    }

    #[test]
    fn json_reports_both_paths_and_the_renderer() {
        let json = result(RendererInfo {
            software_emulated: Some(false),
            device_name: Some("Test \"GPU\"".to_owned()),
        })
        .to_json();
        assert_eq!(
            json,
            "{\"display_path\":{\"scope\":\"native_window_notify_to_presented_frame\",\"passes\":5,\
             \"cpu\":{\"median_ms\":2.500,\"held_bytes\":100},\
             \"retained\":{\"median_ms\":1.250,\"held_bytes\":4096},\
             \"software_emulated\":false,\"device\":\"Test \\\"GPU\\\"\"}}"
        );
    }

    #[test]
    fn json_uses_null_for_an_unreported_renderer() {
        let json = result(RendererInfo::default()).to_json();
        assert!(json.ends_with(",\"software_emulated\":null,\"device\":null}}"));
    }

    #[test]
    fn json_marks_software_emulation() {
        let json = result(RendererInfo {
            software_emulated: Some(true),
            device_name: None,
        })
        .to_json();
        assert!(json.contains("\"software_emulated\":true"));
    }

    #[test]
    fn json_string_escapes_control_characters() {
        assert_eq!(json_string("a\u{1}b\n"), "\"a\\u0001b\\n\"");
    }

    #[test]
    fn summary_lines_cover_both_paths_and_the_renderer() {
        let lines = result(RendererInfo {
            software_emulated: Some(true),
            device_name: Some("Adapter".to_owned()),
        })
        .summary_lines();
        assert_eq!(lines.len(), 4);
        assert!(lines[1].starts_with("CPU raster path: 2.50 ms median, 1 KiB held"));
        assert!(lines[2].starts_with("Retained scene path: 1.25 ms median, ~4 KiB held"));
        assert_eq!(lines[3], "Renderer: Adapter (software emulated)");
    }

    #[test]
    fn summary_names_an_unknown_renderer() {
        let lines = result(RendererInfo::default()).summary_lines();
        assert_eq!(lines[3], "Renderer: unknown");
    }

    #[test]
    fn fixture_has_glyphs_for_the_probe_to_paint() {
        assert!(glyph_count(&fixture_frame()) > PROBE.chars().count());
    }

    #[gpui::test]
    fn probe_runs_both_modes_on_the_test_platform(cx: &mut gpui::TestAppContext) {
        cx.update(ui::init).unwrap();
        let (probe, window_cx) = cx.add_window_view(|_, _| DisplayPathProbe::default());
        let outcome = Rc::new(RefCell::new(None));
        let sink = outcome.clone();
        window_cx.update(|window, cx| {
            measure(&probe, window, cx, 2, move |result| {
                *sink.borrow_mut() = Some(result);
            });
        });
        for _ in 0..MAX_PRESENT_FRAMES {
            if outcome.borrow().is_some() {
                break;
            }
            window_cx.run_until_parked();
            window_cx.update(|window, cx| {
                window.simulate_next_frame(cx);
            });
        }
        let measured = outcome
            .borrow_mut()
            .take()
            .expect("measurement must finish")
            .expect("both paths must present");
        assert_eq!(measured.passes, 2);
        assert!(measured.cpu.held_bytes > 0);
        assert!(measured.retained.held_bytes > 0);
    }
}
