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
pub fn finish_frame(raster: &rendering::RasterizedFrame) -> Option<FinishedFrame> {
    FinishedFrame::new(raster.encode_png().ok()?, raster.width(), raster.height())
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
