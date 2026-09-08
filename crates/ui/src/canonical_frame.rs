//! Operator viewport wrapper over finished canonical pixels (T0.4 PR 9).
//!
//! `ui` may not depend on `rendering`, so this wrapper takes finished pixels
//! plus their logical size and nothing else: PNG bytes with their
//! density-dependent raster extent, composed in `ordo` where `rendering` and
//! `ui` legitimately meet. The wrapper scales the finished frame uniformly
//! inside the `4px` slide-viewport chrome and clips it there; the radius never
//! enters the render, saved content, or an export. Keyboard focus lives with
//! the consumer outside this wrapper: it owns no focus handle and renders no
//! focusable content, and the congregation window never mounts it at all.

use std::sync::Arc;

use gpui::{AnyElement, IntoElement, ParentElement, Styled, StyledImage, div, img, prelude::*, px};

use crate::{environment::UiEnvironment, tokens};

/// Finished canonical pixels plus their sizes.
///
/// `png` is the finished raster encoded as PNG bytes; `pixel_width` and
/// `pixel_height` are the density-dependent raster extent, while the logical
/// composition size is always the `1920x1080` invariant from
/// [`tokens::canonical_slide`]. The bytes are shared across the timeline
/// thumbnail, Inspector, Preview, and Program viewports, so every consumer
/// shows identical internal pixels.
#[derive(Clone, Debug)]
pub struct FinishedFrame {
    image: Arc<gpui::Image>,
    pixel_width: u32,
    pixel_height: u32,
}

impl FinishedFrame {
    /// Wrap finished PNG bytes with their raster extent. Rejects empty bytes
    /// and zero extents, so an incomplete frame cannot reach a viewport.
    pub fn new(png: Vec<u8>, pixel_width: u32, pixel_height: u32) -> Option<Self> {
        if png.is_empty() || pixel_width == 0 || pixel_height == 0 {
            return None;
        }
        Some(Self {
            image: Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png)),
            pixel_width,
            pixel_height,
        })
    }

    /// The finished pixels every viewport shares.
    pub fn image(&self) -> &Arc<gpui::Image> {
        &self.image
    }

    /// The density-dependent raster extent the bytes sample.
    pub fn pixel_size(&self) -> (u32, u32) {
        (self.pixel_width, self.pixel_height)
    }

    /// The logical composition size the pixels were sampled from.
    pub fn logical_size(&self) -> (u32, u32) {
        (
            tokens::canonical_slide::WIDTH,
            tokens::canonical_slide::HEIGHT,
        )
    }

    /// The uniform scale fitting the logical frame inside an outer box of
    /// `available_width` by `available_height` px: the smaller axis wins, so
    /// internal pixels never stretch. The frame itself is unchanged; only the
    /// outer viewport differs per consumer, preset, and Interface scale.
    pub fn fit_scale(available_width: f32, available_height: f32) -> f32 {
        if available_width <= 0.0 || available_height <= 0.0 {
            return 0.0;
        }
        let width_scale = available_width / f32::from(tokens::canonical_slide::WIDTH as u16);
        let height_scale = available_height / f32::from(tokens::canonical_slide::HEIGHT as u16);
        width_scale.min(height_scale).max(0.0)
    }
}

/// One viewport onto a finished frame: the finished pixels scaled uniformly,
/// clipped inside the `4px` operator-only slide-viewport chrome.
///
/// `width` is the outer viewport width in px; the height follows at exactly
/// `16:9`, so the contained image fills the box without letterboxing and
/// Interface scale, OS scale, and viewport presets can only resize the box,
/// never reflow what is inside it. The caller scopes the viewport with its
/// own id and debug selector when several share a window; the inner frame
/// keeps the one shared `canonical-frame` selector.
pub fn canonical_frame(
    frame: &FinishedFrame,
    width: f32,
    environment: UiEnvironment,
) -> AnyElement {
    let scale = environment.preferences().scale;
    let height =
        width * tokens::canonical_slide::HEIGHT as f32 / tokens::canonical_slide::WIDTH as f32;
    div()
        .debug_selector(|| "canonical-frame".into())
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .rounded(px(scale.apply(tokens::radius::SLIDE_VIEWPORT)))
        .bg(gpui::rgb(tokens::color::PROJECTION_BACKGROUND))
        .child(
            img(frame.image.clone())
                .w_full()
                .h_full()
                .object_fit(gpui::ObjectFit::Contain),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{InterfaceScale, PreparationAppearance, UiPreferences};

    fn environment_at(scale: InterfaceScale) -> UiEnvironment {
        UiEnvironment::new(
            UiPreferences {
                appearance: PreparationAppearance::Light,
                scale,
            },
            crate::environment::ResolvedAppearance::Light,
        )
    }

    fn finished() -> FinishedFrame {
        FinishedFrame::new(vec![0x89, b'P', b'N', b'G'], 1920, 1080).expect("complete pixels")
    }

    #[test]
    fn incomplete_pixels_cannot_reach_a_viewport() {
        assert!(FinishedFrame::new(Vec::new(), 1920, 1080).is_none());
        assert!(FinishedFrame::new(vec![1], 0, 1080).is_none());
        assert!(FinishedFrame::new(vec![1], 1920, 0).is_none());
    }

    #[test]
    fn logical_size_is_the_canonical_invariant() {
        assert_eq!(finished().logical_size(), (1920, 1080));
        assert_eq!(finished().pixel_size(), (1920, 1080));
    }

    #[test]
    fn fit_scale_is_uniform_and_never_stretches() {
        // The Compact preset is a hair wider than 16:9, so its height binds;
        // the Wide preset is exactly 16:9.
        assert_eq!(FinishedFrame::fit_scale(1366.0, 768.0), 768.0 / 1080.0);
        assert_eq!(FinishedFrame::fit_scale(1920.0, 1080.0), 1.0);
        // A narrow box binds on height instead; neither axis ever stretches.
        assert_eq!(FinishedFrame::fit_scale(320.0, 100.0), 100.0 / 1080.0);
        assert_eq!(FinishedFrame::fit_scale(0.0, 768.0), 0.0);
        assert_eq!(FinishedFrame::fit_scale(1366.0, -1.0), 0.0);
    }

    #[test]
    fn interface_scale_reaches_only_the_chrome() {
        // The scale selects the operator-only radius; the shared bytes it
        // wraps are untouched, so the frame inside cannot move with scale.
        let frame = finished();
        let before = frame.image().bytes().to_vec();
        for scale in [
            InterfaceScale::Percent100,
            InterfaceScale::Percent125,
            InterfaceScale::Percent150,
            InterfaceScale::Percent175,
            InterfaceScale::Percent200,
        ] {
            let _ = canonical_frame(&frame, scale.apply(640.0), environment_at(scale));
            assert_eq!(frame.image().bytes(), before.as_slice());
        }
        assert_eq!(finished().image().bytes(), before.as_slice());
    }

    #[gpui::test]
    fn the_wrapper_holds_sixteen_by_nine_at_every_scale(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init).expect("ui must initialize");
        let frame = finished();
        let (host, cx) = cx.add_window_view(|_, _| WrapperHost {
            frame: frame.clone(),
            width: 640.0,
            environment: environment_at(InterfaceScale::Percent100),
        });
        cx.simulate_resize(gpui::size(gpui::px(2400.0), gpui::px(1400.0)));

        for scale in [
            InterfaceScale::Percent100,
            InterfaceScale::Percent125,
            InterfaceScale::Percent150,
            InterfaceScale::Percent175,
            InterfaceScale::Percent200,
        ] {
            let environment = environment_at(scale);
            let width = scale.apply(640.0);
            host.update(cx, |host, cx| {
                host.width = width;
                host.environment = environment;
                cx.notify();
            });
            cx.run_until_parked();

            let bounds = cx
                .debug_bounds("canonical-frame")
                .expect("the wrapper must render");
            let (actual_w, actual_h) =
                (f32::from(bounds.size.width), f32::from(bounds.size.height));
            assert!(
                (actual_w - width).abs() < 0.01,
                "outer width must follow the viewport, got {actual_w} for {width}"
            );
            assert!(
                (actual_h - width * 9.0 / 16.0).abs() < 0.05,
                "outer height must hold 16:9, got {actual_h} for {width}"
            );
        }
    }

    struct WrapperHost {
        frame: FinishedFrame,
        width: f32,
        environment: UiEnvironment,
    }

    impl gpui::Render for WrapperHost {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            canonical_frame(&self.frame, self.width, self.environment)
        }
    }
}
