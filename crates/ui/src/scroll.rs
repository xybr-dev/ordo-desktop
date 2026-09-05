//! The one scroll indicator every bounded scrolling surface uses.
//!
//! The pinned GPUI scrolls from wheel and keyboard but paints nothing and ships
//! no scrollbar element, so a list taller than its owner gives no sign that
//! more exists below the fold; see `docs/qualification/GPUI-BASE-T0.3.md`.
//! `DESIGN.md` asks for one thin overlay drawn in the strong border token *over*
//! the scrolled content rather than in a reserved gutter, present only while the
//! content exceeds its viewport. Wheel, keyboard, and `scroll_to_item` behavior
//! are untouched: the indicator adds a way to scroll and replaces none, which is
//! what `DESIGN.md` means by never being the only one.

use std::{cell::Cell, rc::Rc};

use gpui::{
    CursorStyle, IntoElement, MouseButton, ScrollHandle, Styled, Window, div, point, prelude::*,
    px, relative, rgb,
};

use crate::{environment::UiEnvironment, tokens};

/// At rest the indicator is only wide enough to read. `DESIGN.md` puts it over
/// the content with no gutter, so it sits half a spacing step off the edge:
/// close enough to read as the surface's own margin rather than as something
/// floating in the content.
const THICKNESS: f32 = tokens::spacing::RELATED_TIGHT;
const INSET: f32 = tokens::spacing::UNIT / 2.0;

/// What it grows to once the pointer is near the edge, which is the width it
/// has to be to grab. It is never this wide at rest, because a control that
/// size sitting over the content would read as a gutter Ordo deliberately does
/// not reserve.
const GRAB_THICKNESS: f32 = tokens::spacing::RELATED_TIGHT * 2.5;

/// How far from the edge the pointer counts as reaching for the indicator. The
/// strip only detects the pointer; it registers no press of its own, so a row
/// underneath keeps every pixel of its own hit area except the ones the
/// indicator itself covers.
const REACH: f32 = tokens::spacing::GROUP;

/// The shortest the indicator may become, so a very long collection still
/// leaves something with a readable position rather than a dot.
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
    /// Whether the pointer is close enough to the edge to be reaching for the
    /// indicator, which is what widens it to something grabbable. The indicator
    /// blocks presses, and so also blocks the strip's hover underneath itself,
    /// which is why the two are counted separately: one flag alone would drop
    /// the moment the pointer arrived on the thing it was reaching for.
    near: Rc<Cell<bool>>,
    over: Rc<Cell<bool>>,
    /// Where a drag took hold: the pointer and the offset it started from, so
    /// the indicator keeps the point it was grabbed by instead of jumping its
    /// middle to the pointer.
    grab: Rc<Cell<Option<(f32, f32)>>>,
}

/// The payload that makes GPUI deliver moves outside the indicator's own
/// bounds, which is the whole point of a drag: the pointer leaves a 4px bar
/// immediately and must keep dragging it anyway.
struct Grab;

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
    /// `corner` is the surface's own radius. The indicator sits on the edge, so
    /// it has to stop where that edge starts curving away from it; a square
    /// surface passes `0`.
    pub fn indicator(
        &self,
        corner: f32,
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
        let dragging = self.grab.get().is_some();
        let thickness = if dragging || self.near.get() || self.over.get() {
            GRAB_THICKNESS
        } else {
            THICKNESS
        };
        let bar = div()
            .id("scroll-indicator")
            .debug_selector(|| "scroll-indicator".into())
            .absolute()
            .top(relative(proportions.top))
            .right_0()
            .w(px(scale.apply(thickness)))
            .h(relative(proportions.length))
            // A very long collection would otherwise shrink it to a dot, and a
            // very short track would let that minimum grow past the surface.
            .min_h(px(scale.apply(MINIMUM_LENGTH)))
            .max_h(relative(1.0))
            .rounded(px(scale.apply(tokens::radius::CONTROL)))
            .bg(rgb(tokens::palette(environment.appearance()).border_strong))
            // Open until it is taken hold of, which is the whole of what this
            // control does.
            .cursor(if dragging {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            // The bar is the only part of this that takes a press, so it is
            // also the only part that can shadow a row underneath it.
            .occlude()
            .on_mouse_down(MouseButton::Left, {
                let scroll = self.clone();
                move |event, _, _| {
                    scroll
                        .grab
                        .set(Some((f32::from(event.position.y), scroll.measure().offset)));
                }
            })
            .on_drag(Grab, |_, _, _, cx| cx.new(|_| gpui::EmptyView))
            .on_hover({
                let scroll = self.clone();
                move |over, window, _| {
                    if scroll.over.replace(*over) != *over {
                        window.refresh();
                    }
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let scroll = self.clone();
                move |_, _, _| scroll.grab.set(None)
            })
            .on_mouse_up_out(MouseButton::Left, {
                let scroll = self.clone();
                move |_, _, _| scroll.grab.set(None)
            });
        Some(
            div()
                .id("scroll-track")
                .absolute()
                .top(px(scale.apply(corner)))
                .bottom(px(scale.apply(corner)))
                .right(px(scale.apply(INSET)))
                .w(px(scale.apply(REACH)))
                .flex()
                .justify_end()
                .on_hover({
                    let scroll = self.clone();
                    move |near, window, _| {
                        if scroll.near.replace(*near) != *near {
                            window.refresh();
                        }
                    }
                })
                .on_drag_move({
                    let scroll = self.clone();
                    move |event: &gpui::DragMoveEvent<Grab>, window, _| {
                        let Some((from, offset)) = scroll.grab.get() else {
                            return;
                        };
                        let measured = scroll.measure();
                        let track = f32::from(event.bounds.size.height);
                        let bar = (proportions.length * track)
                            .max(scale.apply(MINIMUM_LENGTH))
                            .min(track);
                        // The pointer covers the track it has left to travel,
                        // not the whole track, so a drag to the bottom of the
                        // surface lands on the bottom of the content.
                        let travel = track - bar;
                        if travel <= 0.0 {
                            return;
                        }
                        let moved = f32::from(event.event.position.y) - from;
                        let scrolled = (offset - moved * measured.max_offset / travel)
                            .clamp(-measured.max_offset, 0.0);
                        scroll
                            .handle
                            .set_offset(point(scroll.handle.offset().x, px(scrolled)));
                        window.refresh();
                    }
                })
                .child(bar),
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
