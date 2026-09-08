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
/// an unaligned width paints black forever instead of failing loudly.
/// Densities whose widths already align (100/150/200%) encode
/// byte-identically to the unpadded raster.
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
    FinishedFrame::new(png, aligned, height)
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

    /// One mixed-language fixture through the real path: compose, rasterize,
    /// and finish, the way the runtime worker hands a frame to these views.
    fn finished_fixture() -> FinishedFrame {
        use rendering::{TargetDensity, TemplateContent, TemplateKind, Theme};

        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some("Bersikulo".to_owned()),
            body: "Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo \u{2014} awit 95".to_owned(),
            footer: None,
            image: None,
        };
        let (frame, _) =
            rendering::render_template(&content, &Theme::default(), TargetDensity::SCALE_100)
                .expect("fixture must compose");
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
        use rendering::{TargetDensity, TemplateContent, TemplateKind, Theme};

        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some("Bersikulo".to_owned()),
            body: "Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo \u{2014} awit 95".to_owned(),
            footer: None,
            image: None,
        };
        let density = TargetDensity::from_percent(percent).expect("valid density");
        let (frame, _) = rendering::render_template(&content, &Theme::default(), density)
            .expect("fixture must compose");
        rendering::rasterize(&frame, density, &HashMap::new()).expect("fixture must rasterize")
    }

    #[test]
    fn every_finished_width_survives_the_upload_alignment() {
        // The pinned GPUI revision uploads viewport images with an unpadded
        // bytes_per_row, which wgpu rejects unless width * 4 is a multiple
        // of 256: an unaligned width paints black forever under its cached
        // image id instead of failing loudly.
        for percent in [100, 125, 150, 175, 200] {
            let finished = finish_frame(&finished_at_density(percent)).expect("raster must finish");
            let (pixel_w, pixel_h) = finished.pixel_size();
            assert_eq!(
                pixel_w % 64,
                0,
                "{percent}% finished width must stay upload-aligned"
            );
            let decoded = tiny_skia::Pixmap::decode_png(finished.image().bytes())
                .expect("finished bytes must stay a decodable PNG");
            assert_eq!((decoded.width(), decoded.height()), (pixel_w, pixel_h));
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
        // 125% rasterizes 2400 wide and finishes 2432: the original columns
        // keep their ink and the pad column is opaque black, invisible
        // against the slide background.
        let raster = finished_at_density(125);
        let finished = finish_frame(&raster).expect("raster must finish");
        assert_eq!(finished.pixel_size(), (2432, 1350));
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
        // so identical internal pixels is structural, not coincidental.
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
                    "shared pixels must survive {viewport:?} at {scale:?}"
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

    /// The rendered half of the exit check: one fixture in all four viewports
    /// at both viewport presets and every Interface scale. Each outer box
    /// holds `16:9` and differs only in outer scale; the shared frame inside
    /// never moves.
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
                    "shared pixels must not move with preset or scale"
                );
            }
        }
    }

    /// No operator-only node reaches the congregation window: it mounts no
    /// canonical viewport, keeps its Program and status selectors, and takes
    /// no focus.
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
        assert!(cx.update(|window, cx| window.focused(cx).is_none()));
    }
}
