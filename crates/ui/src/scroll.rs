//! The one scroll indicator every bounded scrolling surface uses.
//!
//! The pinned GPUI scrolls from wheel and keyboard but paints nothing and ships
//! no scrollbar element, so a list taller than its owner gives no sign that
//! more exists below the fold; see `docs/qualification/GPUI-BASE-T0.3.md`.
//! `DESIGN.md` asks for one thin overlay drawn in the strong border token *over*
//! the scrolled content rather than in a reserved gutter, present only while the
//! content exceeds its viewport. It indicates position and nothing else: wheel,
//! keyboard, and `scroll_to_item` behavior are untouched, and it is not a
//! control.

use std::{cell::Cell, rc::Rc};

use gpui::{IntoElement, ScrollHandle, Styled, Window, div, prelude::*, px, relative, rgb};

use crate::{environment::UiEnvironment, tokens};

/// The narrowest useful indicator, and its inset from the surface's inner edge.
/// Both are the tight spacing step rather than a new local vocabulary.
const THICKNESS: f32 = tokens::spacing::RELATED_TIGHT;

/// A very long collection would otherwise shrink the indicator to a dot with no
/// readable position.
const MINIMUM_LENGTH: f32 = tokens::spacing::SECTION;

/// A bounded scrolling surface: GPUI's scroll handle plus what the last frame
/// drew from it.
///
/// Ordo owns this rather than a bare [`ScrollHandle`] because the geometry only
/// exists once the surface has been measured, so the frame that opens a popup
/// has nothing to draw yet. Remembering what was drawn lets the surface ask for
/// one more frame when the measurement moves under it, which also covers the
/// reverse case: content that shrinks to fit must lose its indicator on an
/// otherwise idle surface.
#[derive(Clone, Debug, Default)]
pub struct Scroll {
    handle: ScrollHandle,
    drawn: Rc<Cell<Option<Measurement>>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Measurement {
    viewport: f32,
    max_offset: f32,
    offset: f32,
}

/// Where the indicator sits in its track, as fractions of the track.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Proportions {
    top: f32,
    length: f32,
}

/// `None` whenever the content fits, because `DESIGN.md` forbids the indicator
/// on a surface that does not scroll.
fn proportions(measured: Measurement) -> Option<Proportions> {
    if measured.viewport <= 0.0 || measured.max_offset <= 0.0 {
        return None;
    }
    let length = measured.viewport / (measured.viewport + measured.max_offset);
    let travelled = (measured.offset.abs() / measured.max_offset).clamp(0.0, 1.0);
    Some(Proportions {
        top: travelled * (1.0 - length),
        length,
    })
}

impl Scroll {
    pub fn new() -> Self {
        Self::default()
    }

    fn measure(&self) -> Measurement {
        Measurement {
            viewport: f32::from(self.handle.bounds().size.height),
            max_offset: f32::from(self.handle.max_offset().y),
            offset: f32::from(self.handle.offset().y),
        }
    }

    /// For `track_scroll` and `scroll_to_item`: this type replaces the handle as
    /// the thing a surface owns, not as the thing GPUI is given.
    pub fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    /// The indicator for the surface's *current* measurement, to be placed in a
    /// `relative` wrapper around the scrolling element — a sibling, because a
    /// child of the scrolling element would scroll away with the content.
    ///
    pub fn indicator(
        &self,
        window: &mut Window,
        cx: &mut gpui::App,
        environment: UiEnvironment,
    ) -> Option<impl IntoElement + use<>> {
        let measured = self.measure();
        self.drawn.set(Some(measured));
        // What this frame draws from is the *previous* frame's measurement:
        // GPUI writes the surface's bounds and clamps its offset during
        // prepaint, after every render has run, and it refuses to dirty a
        // window mid-draw. So the frame is checked once it is over and asked
        // for again only when the surface moved under it, which converges in
        // one extra frame and leaves an idle surface idle.
        let scroll = self.clone();
        window
            .spawn(cx, async move |cx| {
                if scroll.drawn.get() != Some(scroll.measure()) {
                    cx.update(|window, _| window.refresh()).ok();
                }
            })
            .detach();
        let proportions = proportions(measured)?;
        let scale = environment.preferences().scale;
        Some(
            div()
                .debug_selector(|| "scroll-indicator".into())
                .absolute()
                .top(relative(proportions.top))
                .right(px(scale.apply(THICKNESS)))
                .w(px(scale.apply(THICKNESS)))
                .h(relative(proportions.length))
                .min_h(px(scale.apply(MINIMUM_LENGTH)))
                .rounded(px(scale.apply(tokens::radius::CONTROL)))
                .bg(rgb(tokens::palette(environment.appearance()).border_strong)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured(viewport: f32, max_offset: f32, offset: f32) -> Measurement {
        Measurement {
            viewport,
            max_offset,
            offset,
        }
    }

    #[test]
    fn content_that_fits_has_no_indicator() {
        assert_eq!(proportions(measured(240.0, 0.0, 0.0)), None);
        // Nothing has been measured yet, which is not the same as fitting, but
        // there is equally nothing to draw from.
        assert_eq!(proportions(measured(0.0, 0.0, 0.0)), None);
    }

    #[test]
    fn the_indicator_reports_how_much_is_visible_and_where_it_is() {
        // Three viewports of content: a third is visible.
        let top = proportions(measured(100.0, 200.0, 0.0)).unwrap();
        assert!((top.length - 1.0 / 3.0).abs() < f32::EPSILON);
        assert_eq!(top.top, 0.0);

        // GPUI counts downward scroll as a negative offset; halfway down puts
        // the indicator halfway through the track it has left to travel.
        let middle = proportions(measured(100.0, 200.0, -100.0)).unwrap();
        assert!((middle.top - (1.0 / 3.0)).abs() < 1e-6);

        let bottom = proportions(measured(100.0, 200.0, -200.0)).unwrap();
        assert!((bottom.top + bottom.length - 1.0).abs() < 1e-6);

        // An overscrolled offset stays inside the track rather than sliding
        // out of the surface.
        let overscrolled = proportions(measured(100.0, 200.0, -400.0)).unwrap();
        assert_eq!(overscrolled, bottom);
    }
}
