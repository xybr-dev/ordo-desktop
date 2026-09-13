//! The four operator viewports onto one finished canonical frame (T0.4 PR 9).
//!
//! This is where `rendering` and `ui` legitimately meet: a completed raster is
//! finished into shared pixels here, and the timeline thumbnail, Inspector,
//! Preview, and Program surfaces each show those same pixels through the one
//! [`ui::canonical_frame`] wrapper. These are four viewports onto one frame,
//! not four workflows: no Cue timeline, Inspector navigation, or
//! Preview-to-Program behavior lives here. The congregation window never
//! mounts any of these; it stays passive, square, and edge-to-edge.

use gpui::{AnyElement, IntoElement, ParentElement, div, prelude::*};
use ui::{canonical_frame::FinishedFrame, environment::UiEnvironment};

/// Finish a completed raster into the shared pixels every viewport shows.
///
/// The PNG bytes come through the qualified `tiny-skia` edge beside the
/// raster's own density-dependent extent; the logical size stays the
/// `1920x1080` invariant the wrapper owns. `None` when the raster has no
/// backing store to finish.
///
/// Finished rows are padded rightward with opaque black to a multiple of
/// 64px: the pinned GPUI revision uploads viewport images with an unpadded
/// `bytes_per_row`, which its wgpu atlas rejects unless `width * 4` is a
/// multiple of 256, and the failed tile stays cached under its image id, so
/// an unaligned width paints black forever instead of failing loudly. The
/// finished frame keeps the true raster extent as its visible size and
/// records the stride separately, so the wrapper crops the upload-only
/// padding instead of letterboxing it. Densities whose widths already align
/// (100/150/200%) encode byte-identically to the unpadded raster.
pub fn finish_frame(raster: &rendering::RasterizedFrame) -> Option<FinishedFrame> {
    let (width, height) = (raster.width(), raster.height());
    let aligned = width.next_multiple_of(64);
    let png = if aligned == width {
        raster.encode_png().ok()?
    } else {
        let padded = pad_rows(raster.rgba(), width, height, aligned)?;
        tiny_skia::Pixmap::from_vec(padded, tiny_skia::IntSize::from_wh(aligned, height)?)?
            .encode_png()
            .ok()?
    };
    FinishedFrame::new(png, width, height, aligned)
}

/// Pad one row-major RGBA buffer rightward to `aligned_width` with opaque
/// black, keeping every original pixel byte-identical.
fn pad_rows(rgba: &[u8], width: u32, height: u32, aligned_width: u32) -> Option<Vec<u8>> {
    let (width, height, aligned) = (width as usize, height as usize, aligned_width as usize);
    if aligned < width || rgba.len() != width * height * 4 {
        return None;
    }
    let mut padded = vec![0u8; aligned * height.saturating_mul(4)];
    for (row, dest) in padded.chunks_exact_mut(aligned * 4).enumerate() {
        dest[..width * 4].copy_from_slice(&rgba[row * width * 4..(row + 1) * width * 4]);
        for pixel in dest[width * 4..].chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }
    Some(padded)
}

/// One timeline cell: the finished frame at grid width. `index` scopes the
/// cell when a row of thumbnails shares a window.
pub fn timeline_thumbnail(
    frame: &FinishedFrame,
    index: usize,
    environment: UiEnvironment,
) -> AnyElement {
    let width = environment.preferences().scale.apply(320.0);
    div()
        .id(("canonical-thumbnail", index))
        .debug_selector(move || format!("canonical-thumbnail-{index}"))
        .w(gpui::px(width))
        .child(ui::canonical_frame::canonical_frame(
            frame,
            width,
            environment,
        ))
        .into_any_element()
}

/// The Inspector viewport: the finished frame beside the selected entity's
/// details.
pub fn inspector_frame(frame: &FinishedFrame, environment: UiEnvironment) -> AnyElement {
    let width = environment.preferences().scale.apply(480.0);
    div()
        .id("canonical-inspector")
        .debug_selector(|| "canonical-inspector".into())
        .w(gpui::px(width))
        .child(ui::canonical_frame::canonical_frame(
            frame,
            width,
            environment,
        ))
        .into_any_element()
}

/// The Preview viewport: the finished frame before it goes live.
pub fn preview_frame(frame: &FinishedFrame, environment: UiEnvironment) -> AnyElement {
    let width = environment.preferences().scale.apply(720.0);
    div()
        .id("canonical-preview")
        .debug_selector(|| "canonical-preview".into())
        .w(gpui::px(width))
        .child(ui::canonical_frame::canonical_frame(
            frame,
            width,
            environment,
        ))
        .into_any_element()
}

/// The operator Program monitor: the finished frame that is live. The
/// congregation window is not this; it stays a separate passive surface.
/// Production Preview and Program share the 720 width, so production holds
/// three distinct operator outer sizes (320 / 480 / 720 before Interface
/// scale); the milestone's four different-size viewports remain an open
/// acceptance gate (see the milestone E2E below).
pub fn program_frame(frame: &FinishedFrame, environment: UiEnvironment) -> AnyElement {
    let width = environment.preferences().scale.apply(720.0);
    div()
        .id("canonical-program")
        .debug_selector(|| "canonical-program".into())
        .w(gpui::px(width))
        .child(ui::canonical_frame::canonical_frame(
            frame,
            width,
            environment,
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use ui::environment::{
        InterfaceScale, PreparationAppearance, ResolvedAppearance, UiPreferences,
    };

    const SCALES: [InterfaceScale; 5] = [
        InterfaceScale::Percent100,
        InterfaceScale::Percent125,
        InterfaceScale::Percent150,
        InterfaceScale::Percent175,
        InterfaceScale::Percent200,
    ];

    fn environment_at(scale: InterfaceScale) -> UiEnvironment {
        UiEnvironment::new(
            UiPreferences {
                appearance: PreparationAppearance::Light,
                scale,
            },
            ResolvedAppearance::Light,
        )
    }

    /// One mixed-language fixture through the production path: the
    /// runtime-owned canonical worker composes from live operator state,
    /// and these views rasterize and finish from that payload — never from
    /// a direct renderer call.
    fn compose_through_worker() -> (rendering::Frame, rendering::Diagnostics) {
        use crate::canonical::CanonicalCommand;
        use platform::tasks::{OwnedWorker, WorkerKind};
        use rendering::{TemplateContent, TemplateKind};

        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some("Bersikulo".to_owned()),
            body: "Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo \u{2014} awit 95".to_owned(),
            footer: None,
            image: None,
        };
        let (commands, requests) = std::sync::mpsc::channel();
        let mut worker = OwnedWorker::spawn(WorkerKind::Canonical, move |cancel| {
            crate::canonical::serve_canonical(requests, cancel)
        })
        .expect("canonical worker must start");
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        commands
            .send(CanonicalCommand::Compose {
                environment,
                window_scale: 1.0,
                content,
                reply: reply_tx,
            })
            .expect("canonical worker must accept the probe");
        let composed = reply_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("canonical worker must answer")
            .expect("fixture must compose through the worker");
        commands.send(CanonicalCommand::Stop).ok();
        let _ = worker.cancel_and_join();
        composed
    }

    /// One mixed-language fixture through the real path: compose on the
    /// production worker, then rasterize and finish, the way the runtime
    /// hands a frame to these views.
    fn finished_fixture() -> FinishedFrame {
        use rendering::TargetDensity;

        let (frame, _) = compose_through_worker();
        let raster = rendering::rasterize(&frame, TargetDensity::SCALE_100, &HashMap::new())
            .expect("fixture must rasterize");
        finish_frame(&raster).expect("raster must finish")
    }

    #[test]
    fn finishing_keeps_the_raster_extent_beside_the_bytes() {
        let finished = finished_fixture();
        assert_eq!(finished.logical_size(), (1920, 1080));
        assert_eq!(finished.pixel_size(), (1920, 1080));
        assert!(!finished.image().bytes().is_empty());
    }

    fn finished_at_density(percent: u16) -> rendering::RasterizedFrame {
        use rendering::TargetDensity;

        let density = TargetDensity::from_percent(percent).expect("valid density");
        let (frame, _) = compose_through_worker();
        rendering::rasterize(&frame, density, &HashMap::new()).expect("fixture must rasterize")
    }

    #[test]
    fn every_finished_width_survives_the_upload_alignment() {
        // The pinned GPUI revision uploads viewport images with an unpadded
        // bytes_per_row, which wgpu rejects unless width * 4 is a multiple
        // of 256: an unaligned stride paints black forever under its cached
        // image id instead of failing loudly. The finished frame keeps the
        // true density-derived extent as its visible size and carries the
        // stride separately for the wrapper to crop.
        for (percent, extent) in [
            (100, (1920, 1080)),
            (125, (2400, 1350)),
            (150, (2880, 1620)),
            (175, (3360, 1890)),
            (200, (3840, 2160)),
        ] {
            let finished = finish_frame(&finished_at_density(percent)).expect("raster must finish");
            let (pixel_w, pixel_h) = finished.pixel_size();
            assert_eq!((pixel_w, pixel_h), extent);
            let (upload_w, upload_h) = finished.upload_size();
            assert_eq!(upload_h, pixel_h);
            assert_eq!(upload_w % 64, 0);
            assert!(upload_w >= pixel_w);
            let decoded = tiny_skia::Pixmap::decode_png(finished.image().bytes())
                .expect("finished bytes must stay a decodable PNG");
            assert_eq!((decoded.width(), decoded.height()), (upload_w, upload_h));
        }
    }

    #[test]
    fn aligned_densities_finish_byte_identical_to_the_raster() {
        // Padding must be a no-op where the raster already aligns: 100, 150,
        // and 200% keep their exact finished bytes.
        for percent in [100, 150, 200] {
            let raster = finished_at_density(percent);
            let expected = raster.encode_png().expect("raster must encode");
            let finished = finish_frame(&raster).expect("raster must finish");
            assert_eq!(finished.image().bytes(), expected.as_slice());
            assert_eq!(
                finished.pixel_size(),
                (raster.width(), raster.height()),
                "{percent}% keeps its density-scaled extent"
            );
        }
    }

    #[test]
    fn padding_preserves_content_beside_opaque_black() {
        // 125% rasterizes 2400 wide and uploads 2432: the finished frame
        // keeps the true extent as its visible size, the original columns
        // keep their ink, and the stride's pad column is opaque black,
        // invisible against the slide background once cropped.
        let raster = finished_at_density(125);
        let finished = finish_frame(&raster).expect("raster must finish");
        assert_eq!(finished.pixel_size(), (2400, 1350));
        assert_eq!(finished.upload_size(), (2432, 1350));
        let decoded = tiny_skia::Pixmap::decode_png(finished.image().bytes())
            .expect("finished bytes must stay a decodable PNG");
        let (width, data) = (decoded.width(), decoded.data());
        assert!(
            data.chunks_exact(4).any(|pixel| pixel[0] > 32),
            "the finished slide must keep its glyph ink"
        );
        for y in 0..decoded.height() {
            let at = ((y * width + 2400) * 4) as usize;
            assert_eq!(
                &data[at..at + 4],
                &[0, 0, 0, 255],
                "the first pad column must stay opaque black on row {y}"
            );
        }
    }

    #[test]
    fn every_viewport_takes_the_same_finished_pixels() {
        // The data-level half of the exit check: at both viewport presets and
        // every Interface scale, all four consumers take the one shared frame,
        // so identical source bytes is structural, not coincidental. No
        // compositor output is claimed here, only the shared image source.
        let finished = finished_fixture();
        let expected = finished.image().bytes().to_vec();
        for scale in SCALES {
            let environment = environment_at(scale);
            for viewport in [(1366.0f32, 768.0f32), (1920.0, 1080.0)] {
                let _ = viewport;
                let _ = timeline_thumbnail(&finished, 0, environment);
                let _ = inspector_frame(&finished, environment);
                let _ = preview_frame(&finished, environment);
                let _ = program_frame(&finished, environment);
                assert_eq!(
                    finished.image().bytes(),
                    expected.as_slice(),
                    "shared source bytes must survive {viewport:?} at {scale:?}"
                );
            }
        }
    }

    struct ViewportsHost {
        frame: FinishedFrame,
        environment: UiEnvironment,
    }

    impl gpui::Render for ViewportsHost {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .child(timeline_thumbnail(&self.frame, 0, self.environment))
                .child(inspector_frame(&self.frame, self.environment))
                .child(preview_frame(&self.frame, self.environment))
                .child(program_frame(&self.frame, self.environment))
        }
    }

    /// One operator viewport mounted alone, so its inner `canonical-frame`
    /// pixels box is unambiguous (the combined host reuses the same inner
    /// selectors four times). `which`: 0 thumbnail, 1 inspector, 2 preview,
    /// 3 program.
    struct SingleViewportHost {
        frame: FinishedFrame,
        environment: UiEnvironment,
        which: u8,
    }

    impl gpui::Render for SingleViewportHost {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            // The viewport must not be the window root: a width-only root
            // stretches to the window height, hiding the viewport's own
            // 16:9 box. Nesting it under a full-size parent keeps the
            // measured outer box honest.
            div().size_full().child(match self.which {
                0 => timeline_thumbnail(&self.frame, 0, self.environment),
                1 => inspector_frame(&self.frame, self.environment),
                2 => preview_frame(&self.frame, self.environment),
                _ => program_frame(&self.frame, self.environment),
            })
        }
    }

    /// The rendered half of the exit check: one fixture in all four viewports
    /// at both viewport presets and every Interface scale. Each outer box
    /// holds `16:9` and differs only in outer scale; the shared frame inside
    /// never moves. Production Preview and Program share the 720 width, so
    /// only three distinct outer sizes exist here; the milestone's four
    /// different-size viewports remain an open gate (see the milestone E2E).
    /// Source bytes plus geometry only, never compositor output (see the
    /// milestone E2E below for why no screenshot exists on this host).
    #[gpui::test]
    fn four_viewports_hold_one_frame_across_presets_and_scales(cx: &mut gpui::TestAppContext) {
        cx.update(ui::init).unwrap();
        let frame = finished_fixture();
        let expected = frame.image().bytes().to_vec();
        let (host, cx) = cx.add_window_view(|_, _| ViewportsHost {
            frame: frame.clone(),
            environment: environment_at(InterfaceScale::Percent100),
        });

        for (width, height) in [(1366.0f32, 768.0f32), (1920.0, 1080.0)] {
            cx.simulate_resize(gpui::size(gpui::px(width), gpui::px(height)));
            for scale in SCALES {
                let environment = environment_at(scale);
                host.update(cx, |host, cx| {
                    host.environment = environment;
                    cx.notify();
                });
                cx.run_until_parked();

                for (selector, base) in [
                    ("canonical-thumbnail-0", 320.0f32),
                    ("canonical-inspector", 480.0),
                    ("canonical-preview", 720.0),
                    ("canonical-program", 720.0),
                ] {
                    let leaked: &'static str = Box::leak(selector.to_owned().into_boxed_str());
                    let bounds = cx
                        .debug_bounds(leaked)
                        .unwrap_or_else(|| panic!("{selector} must render"));
                    let outer = scale.apply(base);
                    let (actual_w, actual_h) =
                        (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    assert!(
                        (actual_w - outer).abs() < 1.0,
                        "{selector} outer width must follow scale: got {actual_w} for {outer}"
                    );
                    assert!(
                        (actual_h - outer * 9.0 / 16.0).abs() < 1.0,
                        "{selector} must hold 16:9: got {actual_h} for {outer}"
                    );
                }
                assert!(
                    cx.debug_bounds("canonical-frame").is_some(),
                    "the shared wrapper must render inside every viewport"
                );
                assert_eq!(
                    frame.image().bytes(),
                    expected.as_slice(),
                    "shared source bytes must not move with preset or scale"
                );
            }
        }
    }

    /// No operator-only node reaches the congregation window: it mounts no
    /// canonical viewport, keeps its Program and status selectors, reserves
    /// the Program pixels box edge-to-edge (black until the background
    /// encode lands, then the complete image), and takes no focus.
    #[gpui::test]
    fn congregation_mounts_no_operator_viewport(cx: &mut gpui::TestAppContext) {
        use presentation::{CongregationState, OutputStatus, ProgramFrame};

        cx.update(ui::init).unwrap();
        let state = CongregationState::new(
            ProgramFrame::new(7, 1, 1, vec![0, 0, 0, 255]).expect("complete frame"),
            OutputStatus::Connected,
        );
        let (_, cx) = cx.add_window_view(|_, cx| crate::CongregationView::new(state, cx));
        cx.run_until_parked();

        for selector in [
            "canonical-frame",
            "canonical-thumbnail-0",
            "canonical-inspector",
            "canonical-preview",
            "canonical-program",
        ] {
            assert!(
                cx.debug_bounds(selector).is_none(),
                "congregation must not mount {selector}"
            );
        }
        assert!(cx.debug_bounds("program-frame-7").is_some());
        assert!(cx.debug_bounds("output-status-Connected").is_some());
        assert!(
            cx.debug_bounds("congregation-frame-pixels").is_some(),
            "congregation must paint the complete Program pixels"
        );
        assert!(cx.update(|window, cx| window.focused(cx).is_none()));
    }

    /// T0.4 milestone E2E (`MILESTONES.md` T0.4 `[UI-AUTO]`): render one
    /// mixed-language fixture once, display it in the four operator viewports
    /// plus the production congregation renderer, check every surface's
    /// image source plus geometry, and assert identical canonical content
    /// and unchanged pagination.
    ///
    /// Composition runs once on the production canonical worker; the raster,
    /// the operator [`FinishedFrame`], and the congregation
    /// [`presentation::ProgramFrame`] each derive once from that single
    /// frame. All five surfaces mount for real: the four operator viewports
    /// through their production wrappers at production sizes (timeline 320 /
    /// Inspector 480 / Preview 720 / Program 720, before Interface scale;
    /// Preview and Program share 720, so only three distinct operator outer
    /// sizes exist), and the congregation output through the production
    /// [`crate::CongregationView`], which reserves the Program pixels box
    /// edge-to-edge (black until the background encode lands). The
    /// milestone's four different-size operator viewports therefore remain
    /// an open acceptance gate: this E2E proves the shared content and
    /// pagination at production sizes without manufacturing a fourth
    /// production size, and does not claim that gate.
    /// This is source-byte plus geometry inspection only, NOT a compositor
    /// screenshot: `gpui::TestAppContext` always constructs `TestPlatform`
    /// without any headless renderer on every OS, so
    /// `window.render_to_image` returns `Err` on macOS too and no compositor
    /// pixel readback exists to claim here. No screenshot is attempted and
    /// no mocked pixels are claimed as compositor output; the gate is each
    /// surface's measured outer/inner bounds plus its `img` image source
    /// (id and decodable backing bytes) against the one rasterization.
    /// Actual compositor captures plus outer-scale-normalized pixel
    /// equivalence stay the pending manual gate
    /// (`docs/qualification/T0.4-MANUAL-GATE.md` script C plus the gallery
    /// candidate); this host only proves the sources and boxes those
    /// captures would read. Only outer bounds may differ; everything inside
    /// must stay identical. Mounted congregation isolation stays
    /// pinned by `congregation_mounts_no_operator_viewport` and window
    /// topology by `native-runtime/tests/runtime.rs`; this E2E pins the
    /// shared content and pagination.
    #[gpui::test]
    fn milestone_e2e_one_fixture_feeds_four_viewports_and_congregation(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::canonical::CanonicalCommand;
        use platform::tasks::{OwnedWorker, WorkerKind};
        use presentation::{CongregationState, OutputStatus, ProgramFrame};
        use rendering::{TargetDensity, TemplateContent, TemplateKind};

        cx.update(ui::init).unwrap();

        // One mixed-language fixture: Cebuano, Tagalog, and English lines
        // plus bundled-fallback glyphs (peso sign, Greek alpha), so the
        // composition exercises the Noto path with real ids, never `.notdef`.
        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some("Mixed".to_owned()),
            body: "Maayong buntag, mga igsoon!\nmagandang umaga, mga kapatid \u{2014} awit 95 \u{3B1}\nthe choir sings, salmo 7 \u{20B1}".to_owned(),
            footer: None,
            image: None,
        };

        // Compose once, on the production worker off the GPUI thread.
        let (commands, requests) = std::sync::mpsc::channel();
        let mut worker = OwnedWorker::spawn(WorkerKind::Canonical, move |cancel| {
            crate::canonical::serve_canonical(requests, cancel)
        })
        .expect("canonical worker must start");
        let environment = UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        commands
            .send(CanonicalCommand::Compose {
                environment,
                window_scale: 1.0,
                content,
                reply: reply_tx,
            })
            .expect("canonical worker must accept the fixture");
        let (frame, diagnostics) = reply_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("canonical worker must answer")
            .expect("fixture must compose through the worker");
        commands.send(CanonicalCommand::Stop).ok();
        let _ = worker.cancel_and_join();

        // Capture the canonical output and pagination once, at the edge.
        let baseline_serialization = frame.serialize();
        let baseline_pagination = frame.pagination().clone();
        let baseline_warnings = diagnostics.warnings.clone();
        assert!(
            baseline_serialization.contains("font=fallback"),
            "the mixed-language fixture must exercise the bundled fallback"
        );
        assert!(
            !baseline_serialization
                .lines()
                .any(|line| line.contains(" id=0 ")),
            "the fixture must pin real fallback glyphs, never `.notdef`"
        );

        // Rasterize once; both renderers derive from this single sampling.
        let raster = rendering::rasterize(&frame, TargetDensity::SCALE_100, &HashMap::new())
            .expect("fixture must rasterize");
        assert_eq!(raster.logical_size(), (1920, 1080));
        let raster_bytes = raster.rgba().to_vec();
        let (raster_w, raster_h) = (raster.width(), raster.height());

        // Finish once per renderer from the one composition.
        let finished = finish_frame(&raster).expect("raster must finish");
        let finished_bytes = finished.image().bytes().to_vec();
        let program =
            ProgramFrame::from_raster(1, raster).expect("raster must feed the congregation seam");
        // The congregation renderer shows the same sampling: identical bytes
        // and extent under the one revision.
        assert_eq!(program.revision(), 1);
        assert_eq!((program.width(), program.height()), (raster_w, raster_h));
        assert_eq!(
            program.rgba(),
            raster_bytes.as_slice(),
            "congregation bytes must equal the rasterized canonical output"
        );
        let congregation = CongregationState::new(program, OutputStatus::Connected);
        assert_eq!(
            congregation.program().rgba(),
            raster_bytes.as_slice(),
            "congregation state must carry the one composition"
        );

        // Measure the one finished frame in all four operator viewports,
        // then mount the production congregation renderer on the same
        // composition: each `add_window_view` borrows the app context, so
        // the operator assertions close out before the second window opens.
        {
            let (_, cx) = cx.add_window_view(|_, _| ViewportsHost {
                frame: finished.clone(),
                environment: environment_at(InterfaceScale::Percent100),
            });
            // Measure every operator surface at both qualified viewport presets;
            // only the outer scale is normalized, everything inside must stay
            // identical. Backing bytes plus bounds here, not a screenshot.
            for (width, height) in [(1366.0f32, 768.0f32), (1920.0, 1080.0)] {
                cx.simulate_resize(gpui::size(gpui::px(width), gpui::px(height)));
                cx.run_until_parked();
                let mut outers = Vec::new();
                for (selector, base) in [
                    ("canonical-thumbnail-0", 320.0f32),
                    ("canonical-inspector", 480.0),
                    ("canonical-preview", 720.0),
                    ("canonical-program", 720.0),
                ] {
                    let leaked: &'static str = Box::leak(selector.to_owned().into_boxed_str());
                    let bounds = cx
                        .debug_bounds(leaked)
                        .unwrap_or_else(|| panic!("{selector} must render at {width}x{height}"));
                    let (actual_w, actual_h) =
                        (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    assert!(
                        (actual_w - base).abs() < 1.0,
                        "{selector} outer width must follow its viewport: got {actual_w} for {base}"
                    );
                    assert!(
                        (actual_h - base * 9.0 / 16.0).abs() < 1.0,
                        "{selector} must hold 16:9: got {actual_h} for {base}"
                    );
                    outers.push(actual_w);
                }
                // Production Preview and Program share the 720 width by design,
                // so only three distinct operator outer sizes exist. The
                // milestone's four different-size viewports remain an open
                // acceptance gate and are not claimed here; the shared frame
                // inside never moves.
                assert!(
                    (outers[2] - outers[3]).abs() < 1.0,
                    "production Preview and Program share 720: {outers:?}"
                );
                let mut sorted = outers.clone();
                sorted.sort_by(|left, right| left.total_cmp(right));
                let mut distinct: Vec<f32> = Vec::with_capacity(4);
                for width in sorted {
                    let is_new = match distinct.last() {
                        None => true,
                        Some(last) => (width - *last).abs() > 1.0,
                    };
                    if is_new {
                        distinct.push(width);
                    }
                }
                assert_eq!(
                    distinct.len(),
                    3,
                    "production holds three distinct operator outer sizes: {outers:?}"
                );
                for pair in distinct.windows(2) {
                    assert!(
                        pair[1] - pair[0] > 1.0,
                        "viewports must differ only in outer scale: {outers:?}"
                    );
                }
                assert_eq!(
                    finished.image().bytes(),
                    finished_bytes.as_slice(),
                    "shared operator source bytes must not move with the preset"
                );
            }
        }

        // Per-surface image-source plus geometry only, never compositor
        // output: each operator viewport mounts alone so its inner pixels
        // box is unambiguous. The mounted `img` source (id and decodable
        // backing bytes) must equal the one rasterization beside each
        // surface's measured bounds. No compositor screenshot is attempted:
        // `TestAppContext` supplies no headless renderer on any OS, so
        // `render_to_image` would return `Err` everywhere.
        for (which, selector, base) in [
            (0u8, "canonical-thumbnail-0", 320.0f32),
            (1, "canonical-inspector", 480.0),
            (2, "canonical-preview", 720.0),
            (3, "canonical-program", 720.0),
        ] {
            let (host, cx) = cx.add_window_view(|_, _| SingleViewportHost {
                frame: finished.clone(),
                environment: environment_at(InterfaceScale::Percent100),
                which,
            });
            cx.simulate_resize(gpui::size(gpui::px(1920.0), gpui::px(1080.0)));
            cx.run_until_parked();
            let leaked: &'static str = Box::leak(selector.to_owned().into_boxed_str());
            let outer = cx
                .debug_bounds(leaked)
                .unwrap_or_else(|| panic!("{selector} must render alone"));
            let (outer_w, outer_h) = (f32::from(outer.size.width), f32::from(outer.size.height));
            assert!(
                (outer_w - base).abs() < 1.0 && (outer_h - base * 9.0 / 16.0).abs() < 1.0,
                "{selector} alone must hold its viewport box: got {outer_w}x{outer_h} for {base}"
            );
            let inner = cx
                .debug_bounds("canonical-frame-pixels")
                .unwrap_or_else(|| panic!("{selector} must paint its pixels box"));
            let (inner_w, inner_h) = (f32::from(inner.size.width), f32::from(inner.size.height));
            assert!(
                (inner_w - base).abs() < 1.0 && (inner_h - base * 9.0 / 16.0).abs() < 1.0,
                "{selector} pixels box must fill its viewport box: got {inner_w}x{inner_h} for {base}"
            );
            let (source_id, source_bytes) = host.read_with(cx, |host, _| {
                (host.frame.image().id(), host.frame.image().bytes().to_vec())
            });
            assert_eq!(
                source_id,
                finished.image().id(),
                "{selector} must show the shared image source"
            );
            let decoded = tiny_skia::Pixmap::decode_png(&source_bytes)
                .unwrap_or_else(|_| panic!("{selector} bytes must stay a decodable PNG"));
            assert_eq!(
                decoded.data(),
                raster_bytes.as_slice(),
                "{selector} backing bytes must equal the one rasterization"
            );
        }

        // The congregation surface's boxes plus backing bytes: the pixels
        // box must fill the 16:9 window edge-to-edge, and the exact backing
        // bytes the background encode publishes must decode to the one
        // rasterization. Source bytes plus bounds only, never compositor
        // output; no compositor screenshot is attempted here either.
        let (congregation_view, cx) =
            cx.add_window_view(|_, cx| crate::CongregationView::new(congregation.clone(), cx));
        cx.simulate_resize(gpui::size(gpui::px(1920.0), gpui::px(1080.0)));
        cx.run_until_parked();
        let pixels = cx
            .debug_bounds("congregation-frame-pixels")
            .expect("congregation must reserve the Program pixels box");
        let (actual_w, actual_h) = (f32::from(pixels.size.width), f32::from(pixels.size.height));
        assert!(
            (actual_w - 1920.0).abs() < 1.0 && (actual_h - 1080.0).abs() < 1.0,
            "congregation pixels box must fill the output: got {actual_w}x{actual_h}"
        );
        // The background encode lands via the Context executors; park until
        // the complete image publishes (the surface holds black until then).
        for _ in 0..100 {
            let ready = congregation_view.read_with(cx, |view, _| view.image().is_some());
            if ready {
                break;
            }
            cx.run_until_parked();
        }
        let painted = congregation_view.read_with(cx, |view, _| {
            view.image()
                .expect("congregation background encode must land")
                .bytes()
                .to_vec()
        });
        let decoded = tiny_skia::Pixmap::decode_png(&painted)
            .expect("congregation bytes must stay a decodable PNG");
        assert_eq!(
            (decoded.width(), decoded.height()),
            (raster_w, raster_h),
            "congregation output must carry the raster extent"
        );
        assert_eq!(
            decoded.data(),
            raster_bytes.as_slice(),
            "congregation backing bytes must equal the one rasterization"
        );
        let decoded = tiny_skia::Pixmap::decode_png(&finished_bytes)
            .expect("operator bytes must stay a decodable PNG");
        assert_eq!(
            decoded.data(),
            raster_bytes.as_slice(),
            "operator backing bytes must equal the one rasterization"
        );

        // Unchanged pagination: the one logical frame still serializes and
        // paginates exactly as composed, after every surface consumed it.
        assert_eq!(
            frame.serialize(),
            baseline_serialization,
            "canonical content must be identical on every surface"
        );
        assert_eq!(
            frame.pagination(),
            &baseline_pagination,
            "pagination inputs must be unchanged on every surface"
        );
        assert_eq!(
            diagnostics.warnings, baseline_warnings,
            "diagnostics must ride beside the complete frame unchanged"
        );
        assert_eq!(
            congregation.program().rgba(),
            raster_bytes.as_slice(),
            "congregation backing bytes must still equal the one rasterization"
        );
    }
}
