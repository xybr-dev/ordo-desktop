//! Ordo's motion policies, on top of the adopted `gpui_base::motion`.
//!
//! `DESIGN.md` gives every movement one of a few durations and the `easing`
//! token. This module is where those become `Transition`s, so a surface asks
//! for "standard" motion rather than restating a duration and a curve.
//!
//! GPUI has no element transform, so nothing here scales or translates paint.
//! Movement is a relative inset on an element that keeps its place in the
//! layout, and appearance is opacity.

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{App, Bounds, ElementId, Pixels, Window};
use gpui_base::motion::{Easing, Transition, transition};

use crate::tokens;

/// The `easing` token, `cubic-bezier(0.2, 0, 0, 1)`: fast start, slow finish.
pub fn easing() -> Easing {
    Easing::cubic_bezier(0.2, 0.0, 0.0, 1.0).expect("the easing token is a valid bezier")
}

/// `160ms` on the `easing` token: navigation markers, disclosures, and the
/// page beneath a navigation change (Decision 301).
pub fn standard() -> Transition {
    Transition::new(Duration::from_millis(
        tokens::motion::STANDARD_DURATION_MS.into(),
    ))
    .easing(easing())
}

/// A marker's position travelling to `target`, keyed by `id`.
///
/// It travels on [`standard`] motion only when `selection` changes — the
/// marker moving to another destination or tab. Any other change of target is
/// the layout moving under it (a resize, a new Interface scale, a label that
/// wrapped), and the marker is simply placed there. The first target is
/// adopted on the spot, so a marker is there at startup, and a second change
/// mid-flight turns it around from where it is. Reduced motion makes it jump.
pub fn travel(
    id: impl Into<ElementId>,
    selection: usize,
    target: f32,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let id = id.into();
    let now = cx.background_executor().now();
    // The selection last travelled to, and until when that travel runs. The
    // policy has to stay the same for the whole flight: `transition` samples
    // its curve on every frame, and a zero duration means "be there now".
    let state = window.use_keyed_state((id.clone(), "selection"), cx, |_, _| (selection, now));
    let (last, until) = *state.read(cx);
    let moved = last != selection;
    if moved {
        let until = now + Duration::from_millis(tokens::motion::STANDARD_DURATION_MS.into());
        state.update(cx, |state, _| *state = (selection, until));
    }
    let policy = if moved || now < until {
        standard()
    } else {
        Transition::new(Duration::ZERO)
    };
    transition((id, "travel"), target, policy, window, cx)
}

/// How a view arriving after a navigation change is drawn on this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entrance {
    /// `0.0` when it starts, `1.0` once it has arrived.
    pub opacity: f32,
    /// How far from its resting place it still is, in the direction the
    /// caller gave, in logical pixels.
    pub offset: f32,
}

impl Entrance {
    pub const ARRIVED: Self = Self {
        opacity: 1.0,
        offset: 0.0,
    };
}

#[derive(Clone, Copy)]
struct EntranceState {
    generation: u64,
    started_at: std::time::Instant,
    distance: f32,
}

/// The entrance of a view that replaced another, keyed by `id` and restarted
/// whenever `generation` changes. The first view under an `id` is already
/// there: nothing moves at startup.
///
/// The view travels `distance` over [`standard`] motion while it fades in.
/// Under reduced motion it does not travel at all and fades in over at most
/// `REDUCED_MOTION_MAX_DURATION_MS`, as `DESIGN.md` requires of a page change.
pub fn entrance(
    id: impl Into<ElementId>,
    generation: u64,
    distance: f32,
    window: &mut Window,
    cx: &mut App,
) -> Entrance {
    let now = cx.background_executor().now();
    let reduced = cx.reduce_motion();
    let duration = Duration::from_millis(
        if reduced {
            tokens::motion::REDUCED_MOTION_MAX_DURATION_MS
        } else {
            tokens::motion::STANDARD_DURATION_MS
        }
        .into(),
    );
    let state = window.use_keyed_state(id.into(), cx, |_, _| EntranceState {
        generation,
        // Seeded as already finished, so the first view does not enter.
        started_at: now.checked_sub(duration).unwrap_or(now),
        distance,
    });
    let mut snapshot = *state.read(cx);
    if snapshot.generation != generation {
        snapshot = EntranceState {
            generation,
            started_at: now,
            distance,
        };
        state.update(cx, |state, _| *state = snapshot);
    }
    let elapsed = now.saturating_duration_since(snapshot.started_at);
    if elapsed >= duration {
        return Entrance::ARRIVED;
    }
    window.request_animation_frame();
    let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
    if reduced {
        return Entrance {
            opacity: progress,
            offset: 0.0,
        };
    }
    let eased = easing().sample(progress);
    Entrance {
        opacity: eased,
        offset: snapshot.distance * (1.0 - eased),
    }
}

/// A listener for `Div::on_children_prepainted`.
pub type RecordBounds = Box<dyn Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App)>;

/// A disclosure's openness, keyed by `id`: `1.0` open, `0.0` shut, and in
/// between while it grows or folds over [`standard`] motion. Under reduced
/// motion it is already there.
pub fn fold(id: impl Into<ElementId>, open: bool, window: &mut Window, cx: &mut App) -> f32 {
    transition(
        (id.into(), "fold"),
        if open { 1.0 } else { 0.0 },
        standard(),
        window,
        cx,
    )
}

/// The height a container's only child was last laid out at, for content
/// that grows from nothing to its natural height. Pair the listener with
/// `Div::on_children_prepainted` on the container, and keep the child
/// `flex_none` so the container's clipped height never squeezes it.
pub fn natural_height(
    id: impl Into<ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> (Option<f32>, RecordBounds) {
    let state = window.use_keyed_state((id.into(), "natural-height"), cx, |_, _| None::<f32>);
    let height = *state.read(cx);
    let record = move |bounds: Vec<Bounds<Pixels>>, window: &mut Window, cx: &mut App| {
        let Some(child) = bounds.first() else {
            return;
        };
        let measured = Some(f32::from(child.size.height));
        if *state.read(cx) != measured {
            state.update(cx, |height, _| *height = measured);
            window.refresh();
        }
    };
    (height, Box::new(record))
}

/// The bounds a container's children were last laid out at, for a marker
/// that travels between them. GPUI reports them after layout, so a marker is
/// positioned from the previous frame's layout and the window is redrawn
/// whenever that layout moved — a resize, a new Interface scale, a label that
/// wrapped — so the marker never stays where a child used to be.
#[derive(Clone, Default)]
pub struct Track(Rc<RefCell<Vec<Bounds<Pixels>>>>);

impl Track {
    /// The listener for `Div::on_children_prepainted`.
    pub fn record(&self) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static {
        let track = self.0.clone();
        move |bounds, window, _| {
            if *track.borrow() != bounds {
                *track.borrow_mut() = bounds;
                window.refresh();
            }
        }
    }

    /// The child at `index`, once it has been laid out.
    pub fn get(&self, index: usize) -> Option<Bounds<Pixels>> {
        self.0.borrow().get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_curve_starts_fast_and_finishes_slow() {
        let curve = easing();
        assert_eq!(curve.sample(0.0), 0.0);
        assert!((curve.sample(1.0) - 1.0).abs() < 1e-4);
        // Fast start: well past halfway by a quarter of the time.
        assert!(curve.sample(0.25) > 0.5, "{}", curve.sample(0.25));
        // No overshoot anywhere: `DESIGN.md` forbids it.
        for step in 0..=100 {
            let value = curve.sample(step as f32 / 100.0);
            assert!((0.0..=1.0 + 1e-4).contains(&value), "{value} at {step}%");
        }
    }
}
