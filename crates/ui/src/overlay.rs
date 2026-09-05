//! The one application overlay layer: dialogs, slideovers, nested slideovers,
//! and toasts.
//!
//! `DESIGN.md` gives every temporary surface one owner, so there is no second
//! stacking system: one [`OverlayHost`] holds the stack, paints the one scrim,
//! and hands each surface its own deferred priority. Focus trapping, Escape,
//! and the input-capturing outside press come from the qualified `gpui-base`
//! `Sheet`; see `docs/qualification/GPUI-BASE-T0.3.md`. Everything visible —
//! names, descriptions, scrim, viewport bounds, layout, and motion — is Ordo's,
//! and so is the Tab policy, because the pin registers a focus trap but leaves
//! enforcing it to the styled façade this crate rejects.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, Context, ElementId, Entity, FocusHandle, IntoElement, ParentElement, Role,
    SharedString, Styled, Window, deferred, div, prelude::*, px, rgb,
};
use gpui_base::{
    ToastManager, ToastOptions, ToastTransitionStatus,
    animation::{ease_in_cubic, ease_out_cubic},
    motion::{Transition, transition},
};

use crate::{FocusNext, FocusPrevious, assets::icon, environment::UiEnvironment, scroll::Scroll};
use crate::{primitives::button::Button, tokens};

/// Below `gpui_base::POPUP_PRIORITY`, because a Select opened *inside* an
/// overlay body has to paint over the surface that owns it, and the operator
/// can only reach a popup on the topmost surface anyway — the scrim captures
/// everything below it.
const OVERLAY_PRIORITY: usize = 20;
/// Above every overlay: a toast confirming what a dialog just did is worth
/// nothing behind that dialog. Still below a popup, which is a live control.
const TOAST_PRIORITY: usize = 50;

/// ponytail: `DESIGN.md` sizes panes but names no dialog or slideover width.
/// These are the first ones; give the caller a builder when one needs another.
const DIALOG_WIDTH: f32 = 480.0;
const SLIDEOVER_WIDTH: f32 = 360.0;

/// How far a dialog sits below its resting place before it settles into the
/// fade, and falls back through on the way out.
///
/// A dialog would rather grow into its fade the way a web one does, and the pin
/// cannot: GPUI's `Style` carries opacity but no transform, and the base's own
/// effect vocabulary is slide, fade, width, and height for the same reason. A
/// width and height animation would be a real scale, and would also rewrap every
/// line of the body on every frame of it, which is a worse artifact than the
/// flourish is worth. One spacing step of rise is centred, costs the layout
/// nothing — a relative inset is a correction applied after layout — and lands in
/// the same `400ms` on the same easing.
const DIALOG_RISE: f32 = tokens::spacing::RELATED;

/// How far a parent slideover slides back under its child. `DESIGN.md` asks for
/// the parent to recede slightly and for reduced motion to remove it entirely.
const RECESSION: f32 = tokens::spacing::GROUP;

/// How far below its resting place a toast arrives from, and falls back to.
///
/// One control height, which is a toast's own minimum, so it reads as coming
/// from off the bottom edge the way the shadcn toaster's does. Same limit as
/// [`DIALOG_RISE`]: the pin has no transform, so this is a relative inset.
const TOAST_RISE: f32 = tokens::density::DEFAULT_CONTROL;

/// How long an expiring confirmation stays. Long enough to read a sentence and
/// reach its Undo, short enough not to sit over the timeline.
pub const TOAST_TIMEOUT: Duration = Duration::from_secs(6);

/// Cycling Tab inside a trap walks the whole window in the worst case; this
/// only has to be larger than any real focus order.
const CONTAINMENT_LIMIT: usize = 100;

/// Which temporary surface a request is. Both are modal and both are rendered
/// by this one host: `DESIGN.md` forbids a standalone slideover owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Surface {
    Dialog,
    Slideover,
}

type Body = Rc<dyn Fn(UiEnvironment, &mut Window, &mut App) -> AnyElement>;
type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

/// One typed request for a temporary surface.
#[derive(Clone)]
pub struct Overlay {
    kind: Surface,
    title: SharedString,
    description: Option<SharedString>,
    body: Body,
    primary: Option<(SharedString, Activate)>,
    secondary: Option<(SharedString, Activate)>,
    on_close: Option<Activate>,
}

impl Overlay {
    pub fn dialog(title: impl Into<SharedString>) -> Self {
        Self::new(Surface::Dialog, title)
    }

    /// A slideover; opening one while another is open nests it, and the parent
    /// keeps its state because nothing about it is torn down.
    pub fn slideover(title: impl Into<SharedString>) -> Self {
        Self::new(Surface::Slideover, title)
    }

    fn new(kind: Surface, title: impl Into<SharedString>) -> Self {
        Self {
            kind,
            title: title.into(),
            description: None,
            body: Rc::new(|_, _, _| div().into_any_element()),
            primary: None,
            secondary: None,
            on_close: None,
        }
    }

    /// The accessible description, and the line under the title. `DESIGN.md`
    /// keeps complete text out of a name, so this is where it goes.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Rendered fresh every frame inside the scrolling body, so the caller's
    /// own state drives it exactly as it would anywhere else.
    pub fn body(
        mut self,
        body: impl Fn(UiEnvironment, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.body = Rc::new(body);
        self
    }

    /// The one clear primary action `DESIGN.md` asks each decision surface for.
    /// Taking it closes the surface, because the decision is over.
    pub fn primary(
        mut self,
        label: impl Into<SharedString>,
        activate: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.primary = Some((label.into(), Rc::new(activate)));
        self
    }

    /// The subordinate action beside it, quiet rather than hidden.
    pub fn secondary(
        mut self,
        label: impl Into<SharedString>,
        activate: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.secondary = Some((label.into(), Rc::new(activate)));
        self
    }

    /// Runs however the surface was dismissed — Escape, the scrim, the Close
    /// control, or either action — so a caller has one place to let go.
    pub fn on_close(mut self, on_close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }
}

/// An operator-only bottom-center message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Notice {
    /// Expires on its own, and may carry Undo. Announced politely, because it
    /// reports something the operator just did.
    Confirmation,
    /// Stays until dismissed deliberately. `DESIGN.md` leaves a hazard visibly
    /// owned by the surface that has it, so this interrupts.
    Failure,
}

#[derive(Clone)]
pub struct Toast {
    notice: Notice,
    message: SharedString,
    undo: Option<(SharedString, Activate)>,
}

impl Toast {
    pub fn confirmation(message: impl Into<SharedString>) -> Self {
        Self {
            notice: Notice::Confirmation,
            message: message.into(),
            undo: None,
        }
    }

    pub fn failure(message: impl Into<SharedString>) -> Self {
        Self {
            notice: Notice::Failure,
            message: message.into(),
            undo: None,
        }
    }

    /// One reversal offered with the confirmation it belongs to. A persistent
    /// failure is not undone, so it keeps its Dismiss control instead.
    pub fn undo(
        mut self,
        label: impl Into<SharedString>,
        activate: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.undo = Some((label.into(), Rc::new(activate)));
        self
    }

    fn timeout(&self) -> Option<Duration> {
        match self.notice {
            Notice::Confirmation => Some(TOAST_TIMEOUT),
            Notice::Failure => None,
        }
    }
}

struct Entry {
    id: usize,
    overlay: Overlay,
    /// The trap's own handle, which is also what containment asks about.
    focus: FocusHandle,
    /// Whatever had focus when this opened, which is what closing gives it
    /// back to — the invoker for a first surface, the parent for a nested one.
    invoker: Option<FocusHandle>,
    scroll: Scroll,
    /// Already closed as far as a caller is concerned, and kept mounted only
    /// while it plays its exit. Its pending removal takes this entry alone, so
    /// a surface opened during that exit is untouched by it.
    closing: bool,
    /// Whether the enter transition has been seeded. `transition` adopts its
    /// first target immediately, so a surface whose first frame asks for 1.0
    /// is simply already there; the first frame has to ask for 0.0 instead.
    entered: std::cell::Cell<bool>,
}

struct Host {
    entries: Vec<Entry>,
    /// Keyed by message, which is what makes the channel deduplicated: the
    /// same operational change reported twice replaces its own toast rather
    /// than stacking a second copy of it, and a caller whose failure has been
    /// resolved can take it back by the words it posted.
    toasts: ToastManager<SharedString, Toast>,
    /// How tall the stack under each mounted message was the last time it was
    /// drawn — and, by being present at all, that its entrance has been seeded.
    ///
    /// Both exist for the reason [`Entry::entered`] does: `transition` adopts
    /// its first target on the spot, so a toast whose first frame asked for its
    /// resting value would simply be there, and a toast that gained a neighbour
    /// below it would simply be a row higher already. Rendering is the only
    /// place that knows either happened, and it holds the host by shared
    /// reference.
    seeded: RefCell<HashMap<SharedString, f32>>,
    /// What each mounted toast measured last frame, which is what makes the
    /// movement above an arriving one exactly as far as the arrival: a toast is
    /// as tall as its own words and its own controls, and no token says how
    /// tall that is. A toast with no measurement yet has never been laid out,
    /// which is what keeps it out of the flow for its first frame.
    heights: RefCell<HashMap<SharedString, f32>>,
    next: usize,
}

impl Host {
    fn empty() -> Self {
        Self {
            entries: Vec::new(),
            // Ordo draws the motion itself, so the enter duration is zero and a
            // toast is present the moment it is posted. The exit duration is
            // real, because it is also a mounting deadline: it is what keeps a
            // leaving toast on screen long enough to be seen leaving.
            toasts: ToastManager::new(gpui_base::ToastMotion {
                duration: Duration::ZERO,
                exit_duration: Duration::from_millis(
                    tokens::motion::OVERLAY_CLOSE_DURATION_MS.into(),
                ),
                ..Default::default()
            }),
            seeded: RefCell::new(HashMap::new()),
            heights: RefCell::new(HashMap::new()),
            next: 0,
        }
    }

    /// Move the toast clock, and come back once more while anything is still
    /// leaving, because an exit ends by unmounting rather than by an input.
    fn sweep(&mut self, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        let advance = self.toasts.advance(now, false);
        for (id, _) in &advance.removed {
            self.seeded.get_mut().remove(id);
            self.heights.get_mut().remove(id);
        }
        if advance.changed {
            cx.notify();
        }
        if self
            .toasts
            .iter()
            .any(|(_, _, status)| status == ToastTransitionStatus::Ending)
        {
            Self::wake_in(
                Duration::from_millis(tokens::motion::OVERLAY_CLOSE_DURATION_MS.into()),
                cx,
            );
        }
    }

    /// One wake-up per thing that is actually due, rather than a clock that
    /// ticks while nothing is waiting on it.
    fn wake_in(delay: Duration, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |host, cx| host.sweep(cx)).ok();
        })
        .detach();
    }

    fn remove(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.id == id && entry.closing)
        else {
            return;
        };
        let entry = self.entries.remove(index);
        // `DESIGN.md`: a closed overlay restores focus to its invoker or the
        // nearest surviving relevant entity, which for a nested slideover is
        // the parent that opened it.
        if let Some(invoker) = entry.invoker {
            invoker.focus(window, cx);
        }
        cx.notify();
    }
}

/// The one overlay layer. Cheap to clone, and keyed state gives a view and the
/// content inside it the same host without either passing it down.
#[derive(Clone)]
pub struct OverlayHost(Entity<Host>);

impl OverlayHost {
    pub fn new(cx: &mut App) -> Self {
        Self(cx.new(|_| Host::empty()))
    }

    /// The same host for every caller that names it, which is how a view and
    /// the content inside it reach one stack without either passing it down.
    pub fn keyed(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Self {
        Self(window.use_keyed_state(id, cx, |_, _| Host::empty()))
    }

    /// Opens a surface, nesting it above whatever is already open.
    pub fn open(&self, overlay: Overlay, window: &mut Window, cx: &mut App) {
        let invoker = window.focused(cx);
        let focus = cx.focus_handle();
        self.0.update(cx, |host, cx| {
            let id = host.next;
            host.next += 1;
            host.entries.push(Entry {
                id,
                overlay,
                focus: focus.clone(),
                invoker,
                scroll: Scroll::new(),
                closing: false,
                entered: std::cell::Cell::new(false),
            });
            cx.notify();
        });
        focus.focus(window, cx);
    }

    /// How many surfaces are open, not counting one still playing its exit.
    pub fn depth(&self, cx: &App) -> usize {
        self.0
            .read(cx)
            .entries
            .iter()
            .filter(|entry| !entry.closing)
            .count()
    }

    /// The title of the topmost open surface, which is what a caller and a
    /// test both mean by "the overlay".
    pub fn top(&self, cx: &App) -> Option<SharedString> {
        self.0
            .read(cx)
            .entries
            .iter()
            .rev()
            .find(|entry| !entry.closing)
            .map(|entry| entry.overlay.title.clone())
    }

    pub fn close_top(&self, window: &mut Window, cx: &mut App) {
        let top = self
            .0
            .read(cx)
            .entries
            .iter()
            .rev()
            .find(|entry| !entry.closing)
            .map(|entry| entry.id);
        if let Some(id) = top {
            self.close(id, window, cx);
        }
    }

    fn close(&self, id: usize, window: &mut Window, cx: &mut App) {
        let closed = self.0.update(cx, |host, cx| {
            let entry = host
                .entries
                .iter_mut()
                .find(|entry| entry.id == id && !entry.closing)?;
            entry.closing = true;
            cx.notify();
            Some(entry.overlay.on_close.clone())
        });
        let Some(on_close) = closed else {
            return;
        };
        if let Some(on_close) = on_close {
            on_close(window, cx);
        }
        // The exit is played, not skipped, so the surface stays mounted for as
        // long as it takes. Reduced motion removes the wait with the movement.
        let after = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            Duration::from_millis(tokens::motion::OVERLAY_CLOSE_DURATION_MS.into())
        };
        let host = self.0.clone();
        window
            .spawn(cx, async move |cx| {
                cx.background_executor().timer(after).await;
                cx.update(|window, cx| {
                    host.update(cx, |host, cx| host.remove(id, window, cx));
                })
                .ok();
            })
            .detach();
    }

    /// Posts a bottom-center message. A confirmation expires on its own; a
    /// failure stays until it is dismissed.
    pub fn notify(&self, toast: Toast, cx: &mut App) {
        let timeout = toast.timeout();
        self.0.update(cx, |host, cx| {
            let id = toast.message.clone();
            let now = cx.background_executor().now();
            host.toasts.push(id, toast, ToastOptions { timeout }, now);
            // Mounting and expiring are two passes of the same clock, and the
            // enter duration is zero, so the toast is present by the time its
            // own timer looks at it.
            host.sweep(cx);
            cx.notify();
            if let Some(timeout) = timeout {
                Host::wake_in(timeout, cx);
            }
        });
    }

    /// Takes a message back, for the caller whose failure has been resolved
    /// rather than dismissed. A message names its own toast.
    pub fn dismiss_toast(&self, message: impl Into<SharedString>, cx: &mut App) {
        let id = message.into();
        self.0.update(cx, |host, cx| {
            let now = cx.background_executor().now();
            if host.toasts.dismiss(&id, now) {
                // Dismissing starts the exit; `sweep` keeps it mounted for the
                // length of it and takes it away at the end.
                host.sweep(cx);
                cx.notify();
            }
        });
    }

    pub fn toasts(&self, cx: &App) -> Vec<SharedString> {
        self.0
            .read(cx)
            .toasts
            .iter()
            .map(|(_, toast, _)| toast.message.clone())
            .collect()
    }

    /// Every temporary surface this window owns, in one element. A view renders
    /// this once, last, and nothing else stacks anything.
    pub fn layer(
        &self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement + use<> {
        let entries: Vec<_> = self
            .0
            .read(cx)
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect();
        let last = entries.len().saturating_sub(1);
        let surfaces: Vec<_> = entries
            .iter()
            .enumerate()
            .filter_map(|(index, &id)| {
                self.surface(id, index, index == last, environment, window, cx)
            })
            .collect();
        div()
            .children(surfaces)
            .children(self.toast_stack(environment, window, cx))
    }

    fn surface(
        &self,
        id: usize,
        index: usize,
        top: bool,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let host = self.0.read(cx);
        let entry = host.entries.iter().find(|entry| entry.id == id)?;
        let (overlay, focus, scroll, closing) = (
            entry.overlay.clone(),
            entry.focus.clone(),
            entry.scroll.clone(),
            entry.closing,
        );
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let viewport = window.viewport_size();
        let gutter = scale.apply(tokens::density::WORKSPACE_GUTTER_WIDE);

        // Enter is fast-start/slow-finish and exit accelerates, per `DESIGN.md`.
        //
        // ponytail: the `easing` token names a CSS cubic bezier and the pin
        // ships no bezier sampler, so these two cubics stand in for it. Swap in
        // a sampler if a surface ever has to match a bezier exactly.
        let duration = Duration::from_millis(if closing {
            tokens::motion::OVERLAY_CLOSE_DURATION_MS
        } else {
            tokens::motion::OVERLAY_OPEN_DURATION_MS
        } as u64);
        let policy = Transition::new(duration).ease(if closing {
            ease_in_cubic
        } else {
            ease_out_cubic
        });
        // The first frame seeds the transition where the surface starts, because
        // `transition` adopts its first target on the spot and a surface that
        // asked for 1.0 first would simply be there already.
        if !entry.entered.replace(true) {
            let _: f32 = transition(
                (ElementId::from(("ordo-overlay", id)), "progress"),
                0.0,
                Transition::new(Duration::ZERO),
                window,
                cx,
            );
        }
        // Retargeting samples the value it is at, so a surface reversed
        // mid-flight turns around from where it is instead of jumping.
        // `transition` also collapses to the target under reduced motion, which
        // is what removes the movement rather than merely shortening it.
        let progress: f32 = transition(
            (ElementId::from(("ordo-overlay", id)), "progress"),
            if closing { 0.0 } else { 1.0 },
            policy,
            window,
            cx,
        );
        let recession: f32 = if cx.reduce_motion() {
            0.0
        } else {
            transition(
                (ElementId::from(("ordo-overlay", id)), "recession"),
                if top { 0.0 } else { scale.apply(RECESSION) },
                Transition::new(Duration::from_millis(
                    tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
                )),
                window,
                cx,
            )
        };

        let width = match overlay.kind {
            Surface::Dialog => scale.apply(DIALOG_WIDTH),
            Surface::Slideover => scale.apply(SLIDEOVER_WIDTH),
        }
        .min(f32::from(viewport.width) - 2.0 * gutter)
        .max(0.0);
        let height = (f32::from(viewport.height) - 2.0 * gutter).max(0.0);

        // The body measures from its own content and shrinks from there, so a
        // dialog is as tall as what it holds up to the viewport. `flex_1` and a
        // `size_full` scroller both measure zero instead, which left the
        // surface exactly its header plus its footer.
        let body = div()
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                scroll
                    .bounded(div().id(("overlay-body", id)).debug_selector({
                        let title = overlay.title.clone();
                        move || format!("overlay-body-{title}")
                    }))
                    .w_full()
                    .min_h_0()
                    .p(px(scale.apply(tokens::spacing::GROUP)))
                    .child((overlay.body)(environment, window, cx)),
            )
            // The body is the square middle of the surface: the rounded corners
            // belong to the header and the footer, so the track is not inset.
            .children(scroll.indicator(
                &format!("overlay-body-{}", overlay.title),
                0.0,
                window,
                cx,
                environment,
            ));

        let close = self.closer(id);
        // The card is the only occluding part: a centring wrapper that took
        // presses would be a second, invisible scrim covering the whole
        // viewport, and the real one would never see the press that dismisses.
        let card = semantics(&overlay, id)
            .flex()
            .flex_col()
            .w(px(width))
            .max_h(px(height))
            .when(overlay.kind == Surface::Slideover, |d| d.h(px(height)))
            .opacity(progress)
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border_strong))
            .rounded(px(scale.apply(tokens::radius::OVERLAY)))
            .shadow(vec![tokens::elevation::overlay(
                environment.appearance(),
                scale,
            )])
            // A dialog rises into place as it fades in and falls back through as
            // it fades out; the easing either way is `progress`'s own, so the two
            // halves cannot drift apart. See [`DIALOG_RISE`].
            .when(overlay.kind == Surface::Dialog, |d| {
                d.top(px((1.0 - progress) * scale.apply(DIALOG_RISE)))
            })
            .child(self.header(&overlay, id, environment))
            .child(body)
            .children(self.footer(&overlay, id, environment));
        let surface = div()
            .absolute()
            .map(|d| match overlay.kind {
                // Centred; the card inside carries the rise, so the box it is
                // centred in never moves.
                Surface::Dialog => d
                    .top(px(gutter))
                    .bottom(px(gutter))
                    .left_0()
                    .right_0()
                    .flex()
                    .items_center()
                    .justify_center(),
                // Advances from the right, and slides back under its child by the
                // recession that child's arrival earned.
                Surface::Slideover => d
                    .top(px(gutter))
                    .h(px(height))
                    // Right grows leftward, so entering is a negative offset that
                    // starts the surface clear of the edge it advances in from,
                    // and receding is a positive one that slides it back under its
                    // child.
                    .right(px(gutter + recession - (1.0 - progress) * (width + gutter))),
            })
            .child(card);

        // `DESIGN.md` gives the scrim the first pointer event, and that is not
        // only about the click: a control under an overlay must not light up
        // under the pointer or lend it a cursor either, or the surface reads as
        // something the operator can reach past. `occlude` stops the hit test
        // here, which settles all three at once.
        let scrim = div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(tokens::elevation::scrim(progress))
            .into_any_element();
        let sheet = gpui_base::Sheet::new(cx)
            .focus_handle(focus.clone())
            .overlay_closable(top && !closing)
            .overlay(scrim)
            .on_close({
                let close = close.clone();
                move |_, window, cx| close(window, cx)
            })
            .surface(surface);

        Some(
            div()
                .id(("ordo-overlay", id))
                // The pin registers a focus trap but leaves enforcing it to the
                // styled façade Ordo rejects, so Tab containment is Ordo's.
                .on_action({
                    let focus = focus.clone();
                    move |_: &FocusNext, window, cx| contain(&focus, false, window, cx)
                })
                .on_action(move |_: &FocusPrevious, window, cx| contain(&focus, true, window, cx))
                .child(deferred(sheet).with_priority(OVERLAY_PRIORITY + index))
                .into_any_element(),
        )
    }

    fn closer(&self, id: usize) -> Activate {
        let host = self.clone();
        Rc::new(move |window, cx| host.close(id, window, cx))
    }

    fn header(&self, overlay: &Overlay, id: usize, environment: UiEnvironment) -> impl IntoElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let close = self.closer(id);
        div()
            .flex_none()
            .flex()
            .items_start()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .p(px(scale.apply(tokens::spacing::GROUP)))
            .border_b(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(scale.apply(tokens::spacing::RELATED_TIGHT)))
                    .child(
                        div()
                            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
                            .child(overlay.title.clone()),
                    )
                    .children(overlay.description.clone().map(|text| {
                        div()
                            .text_color(rgb(palette.text_muted))
                            .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                            .child(text)
                    })),
            )
            .child(
                Button::new(("overlay-close", id), "Close", environment)
                    .icon(icon::X)
                    .icon_only()
                    .quiet()
                    .on_surface(palette.surface)
                    .on_activate(move |window, cx| close(window, cx)),
            )
    }

    /// The footer exists only where there is a decision to take; a slideover
    /// that only shows something has none.
    fn footer(
        &self,
        overlay: &Overlay,
        id: usize,
        environment: UiEnvironment,
    ) -> Option<impl IntoElement + use<>> {
        if overlay.primary.is_none() && overlay.secondary.is_none() {
            return None;
        }
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        // An action ends the decision, so taking either one also closes the
        // surface — which is what runs `on_close` exactly once, whichever way
        // the operator leaves.
        let action = |slot: Option<(SharedString, Activate)>, quiet: bool, close: Activate| {
            slot.map(|(label, activate)| {
                Button::new(
                    (
                        if quiet {
                            "overlay-secondary"
                        } else {
                            "overlay-primary"
                        },
                        id,
                    ),
                    label,
                    environment,
                )
                .when(quiet, Button::quiet)
                .on_surface(palette.surface)
                .on_activate(move |window, cx| {
                    activate(window, cx);
                    close(window, cx);
                })
            })
        };
        Some(
            div()
                .flex_none()
                .flex()
                .justify_end()
                .gap(px(scale.apply(tokens::spacing::RELATED)))
                .p(px(scale.apply(tokens::spacing::GROUP)))
                .border_t(px(tokens::border::SEPARATOR))
                .border_color(rgb(palette.border))
                .children(action(overlay.secondary.clone(), true, self.closer(id)))
                .children(action(overlay.primary.clone(), false, self.closer(id))),
        )
    }

    fn toast_stack(
        &self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let toasts: Vec<_> = self
            .0
            .read(cx)
            .toasts
            .iter()
            .map(|(_, toast, status)| (toast.clone(), status))
            .collect();
        if toasts.is_empty() {
            return None;
        }
        // A toast arrives from just below where it rests and falls back through
        // as it goes, on the same curves and in the same times as a dialog: this
        // is the same motion the shadcn toaster draws, minus the scale the pin
        // has no transform for. See [`TOAST_RISE`].
        //
        // What the arrival must not do is redraw the toasts above it a row
        // higher between two frames. A toast that has never been measured is
        // taken out of the flow for that one frame — it is at zero opacity
        // anyway — so nothing moves while it is measured; from the next frame it
        // is in the flow, and the toasts above it start exactly the distance it
        // turned out to occupy and ride up over that.
        let gap = scale.apply(tokens::spacing::RELATED);
        let heights = self.0.read(cx).heights.borrow().clone();
        let measured: Vec<Option<f32>> = toasts
            .iter()
            .map(|(toast, _)| heights.get(&toast.message).copied())
            .collect();
        let stack_below: Vec<f32> = {
            let mut below = vec![0.0; measured.len()];
            let mut running = 0.0;
            for index in (0..measured.len()).rev() {
                below[index] = running;
                if let Some(height) = measured[index] {
                    running += height + gap;
                }
            }
            below
        };
        let toasts: Vec<_> = toasts
            .into_iter()
            .enumerate()
            .map(|(index, (toast, status))| {
                let leaving = status == ToastTransitionStatus::Ending;
                let key = |property| {
                    (
                        ElementId::from(SharedString::from(format!(
                            "ordo-toast-{}",
                            toast.message
                        ))),
                        property,
                    )
                };
                let arriving = || {
                    Transition::new(Duration::from_millis(
                        tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
                    ))
                    .ease(ease_out_cubic)
                };

                // The stack below this one is what holds it off the bottom
                // edge, so a taller one is that much upward movement rather than
                // the jump the layout would draw on its own.
                let below = stack_below[index];
                let previously = self
                    .0
                    .read(cx)
                    .seeded
                    .borrow_mut()
                    .insert(toast.message.clone(), below);
                let mut shift: f32 = transition(key("shift"), 0.0, arriving(), window, cx);
                if let Some(gained) = previously
                    .map(|previously| below - previously)
                    .filter(|gained| *gained > 0.0)
                {
                    shift += gained;
                    let _: f32 = transition(
                        key("shift"),
                        shift,
                        Transition::new(Duration::ZERO),
                        window,
                        cx,
                    );
                    shift = transition(key("shift"), 0.0, arriving(), window, cx);
                }

                if previously.is_none() {
                    let _: f32 = transition(
                        key("progress"),
                        0.0,
                        Transition::new(Duration::ZERO),
                        window,
                        cx,
                    );
                }
                let progress: f32 = transition(
                    key("progress"),
                    if leaving { 0.0 } else { 1.0 },
                    Transition::new(Duration::from_millis(if leaving {
                        tokens::motion::OVERLAY_CLOSE_DURATION_MS.into()
                    } else {
                        tokens::motion::OVERLAY_OPEN_DURATION_MS.into()
                    }))
                    .ease(if leaving {
                        ease_in_cubic
                    } else {
                        ease_out_cubic
                    }),
                    window,
                    cx,
                );
                (toast, progress, shift, measured[index].is_some())
            })
            .collect();
        let viewport = window.viewport_size();
        let host = self.clone();
        let order: Vec<SharedString> = toasts
            .iter()
            .map(|(toast, ..)| toast.message.clone())
            .collect();
        let items = toasts
            .into_iter()
            .map(move |(toast, progress, shift, in_flow)| {
                let id = toast.message.clone();
                let dismiss = {
                    let host = host.clone();
                    let id = id.clone();
                    move |_: &mut Window, cx: &mut App| host.dismiss_toast(id.clone(), cx)
                };
                div()
                    .id(SharedString::from(format!("ordo-toast-{id}")))
                    .debug_selector({
                        let message = toast.message.clone();
                        move || format!("toast-{message}")
                    })
                    .occlude()
                    .relative()
                    // Out of the flow until it has been measured, so that the
                    // frame it is measured on moves nothing above it. It is at
                    // zero opacity for that frame, so where it sits is
                    // immaterial; it only has to measure the size it will keep.
                    .map(|toast| {
                        let offset = (1.0 - progress) * scale.apply(TOAST_RISE) + shift;
                        match in_flow {
                            true => toast.top(px(offset)),
                            false => toast.absolute().bottom(px(-offset)),
                        }
                    })
                    .opacity(progress)
                    .flex()
                    .items_center()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .max_w(px((f32::from(viewport.width)
                        - 2.0 * scale.apply(tokens::spacing::SECTION))
                    .max(0.0)))
                    .min_h(px(scale.apply(tokens::density::DEFAULT_CONTROL)))
                    .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
                    .py(px(scale.apply(tokens::spacing::RELATED)))
                    .rounded(px(scale.apply(tokens::radius::CONTROL)))
                    .bg(rgb(palette.surface))
                    .border_1()
                    .border_color(rgb(palette.border_strong))
                    .shadow(vec![tokens::elevation::floating(
                        environment.appearance(),
                        scale,
                    )])
                    .text_size(px(scale.apply(tokens::typography::LABEL.size)))
                    // A confirmation reports what the operator just did, so it is
                    // polite; a failure is the unexpected event `DESIGN.md` allows
                    // to interrupt.
                    .role(match toast.notice {
                        Notice::Confirmation => Role::Status,
                        Notice::Failure => Role::Alert,
                    })
                    .aria_label(toast.message.clone())
                    .child(
                        gpui::svg()
                            .flex_none()
                            .size(px(scale.apply(tokens::icon::CONTROL_SIZE)))
                            .text_color(rgb(match toast.notice {
                                Notice::Confirmation => palette.text_muted,
                                Notice::Failure => palette.warning,
                            }))
                            .data(
                                match toast.notice {
                                    Notice::Confirmation => icon::CHECK,
                                    Notice::Failure => icon::TRIANGLE_ALERT,
                                }
                                .as_bytes(),
                            ),
                    )
                    .child(div().min_w_0().child(toast.message.clone()))
                    .children(toast.undo.clone().map(|(label, activate)| {
                        let dismiss = dismiss.clone();
                        Button::new(
                            SharedString::from(format!("toast-undo-{id}")),
                            label,
                            environment,
                        )
                        .quiet()
                        .on_surface(palette.surface)
                        .on_activate(move |window, cx| {
                            activate(window, cx);
                            dismiss(window, cx);
                        })
                    }))
                    // A persistent failure is dismissed deliberately; a
                    // confirmation leaves on its own and needs no control.
                    .children((toast.notice == Notice::Failure).then(|| {
                        Button::new(
                            SharedString::from(format!("toast-dismiss-{id}")),
                            "Dismiss",
                            environment,
                        )
                        .icon(icon::X)
                        .icon_only()
                        .quiet()
                        .on_surface(palette.surface)
                        .on_activate(move |window, cx| dismiss(window, cx))
                    }))
            });
        Some(
            deferred(
                gpui::anchored()
                    .position(gpui::point(px(0.0), px(0.0)))
                    .child(
                        // Sized to the viewport, as every anchored layer here
                        // is: an anchored child is placed at its own size, so
                        // `bottom-center` needs a box to be centred in.
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w(viewport.width)
                            .h(viewport.height)
                            .flex()
                            .flex_col()
                            .justify_end()
                            .items_center()
                            .pb(px(scale.apply(tokens::spacing::SECTION)))
                            .gap(px(gap))
                            .on_children_prepainted({
                                let host = self.0.clone();
                                move |bounds, _, cx| {
                                    host.update(cx, |host, _| {
                                        let heights = host.heights.get_mut();
                                        for (message, bounds) in order.iter().zip(bounds) {
                                            heights.insert(
                                                message.clone(),
                                                f32::from(bounds.size.height),
                                            );
                                        }
                                    });
                                }
                            })
                            .children(items),
                    ),
            )
            .with_priority(TOAST_PRIORITY),
        )
    }
}

/// The node an operator's assistive technology reads. The base sheet carries no
/// role of its own, so the whole of it — role, name, and description — is
/// Ordo's, which is also what makes it probeable on its own.
fn semantics(overlay: &Overlay, id: usize) -> gpui::Stateful<gpui::Div> {
    div()
        .id(("overlay-surface", id))
        .debug_selector({
            let title = overlay.title.clone();
            move || format!("overlay-{title}")
        })
        .occlude()
        .role(Role::Dialog)
        .aria_label(overlay.title.clone())
        .when_some(overlay.description.clone(), |d, text| {
            d.aria_description(text)
        })
}

/// Keeps Tab inside `trap`: step normally, and if that left the trap, keep
/// stepping until it is back inside. This is the half the pin leaves to an
/// application — its own trap element only registers the container.
fn contain(trap: &FocusHandle, backwards: bool, window: &mut Window, cx: &mut App) {
    let before = window.focused(cx);
    let step = |window: &mut Window, cx: &mut App| {
        if backwards {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
    };
    step(window, cx);
    let mut attempts = 0;
    while !trap.contains_focused(window, cx) && attempts < CONTAINMENT_LIMIT {
        step(window, cx);
        attempts += 1;
        if window.focused(cx) == before {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        environment::{ResolvedAppearance, UiPreferences},
        primitives::button::Button,
    };
    use gpui::{Modifiers, Render, TestAppContext};
    use std::cell::Cell;

    struct Harness {
        host: OverlayHost,
        environment: UiEnvironment,
        /// A control the scrim has to keep a press away from.
        beneath: Rc<Cell<usize>>,
        closed: Rc<Cell<usize>>,
        focus: FocusHandle,
    }

    impl Harness {
        /// A surface with two focusable controls inside it, which is the least
        /// that can show Tab cycling rather than escaping.
        fn overlay(&self, kind: Surface) -> Overlay {
            let closed = self.closed.clone();
            let overlay = match kind {
                Surface::Dialog => Overlay::dialog("Delete this Mass?"),
                Surface::Slideover => Overlay::slideover("Inspector"),
            };
            overlay
                .description("This cannot be undone.")
                .body(|environment, _, _| {
                    div()
                        .child(Button::new("inside-first", "Rename", environment))
                        .child(Button::new("inside-second", "Duplicate", environment))
                        .into_any_element()
                })
                .primary("Delete", |_, _| {})
                .on_close(move |_, _| closed.set(closed.get() + 1))
        }
    }

    impl Render for Harness {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let environment = self.environment;
            let host = self.host.clone();
            let beneath = self.beneath.clone();
            div()
                .size_full()
                .track_focus(&self.focus)
                .child(Button::new("invoker", "Open", environment).on_activate({
                    let host = host.clone();
                    let overlay = self.overlay(Surface::Dialog);
                    move |window, cx| host.open(overlay.clone(), window, cx)
                }))
                .child(
                    // Icon-only, so its tooltip is an observable proxy for the
                    // hover the scrim has to withhold: the same `is_hovered`
                    // gates hover styling and the pointer cursor, neither of
                    // which a test can read back.
                    Button::new("beneath", "Beneath", environment)
                        .icon(icon::ELLIPSIS)
                        .icon_only()
                        .on_activate(move |_, _| beneath.set(beneath.get() + 1)),
                )
                .child(self.host.layer(environment, window, cx))
        }
    }

    fn open(cx: &mut TestAppContext) -> (Entity<Harness>, &mut gpui::VisualTestContext) {
        cx.update(crate::init).unwrap();
        let (view, cx) = cx.add_window_view(|window, cx| {
            let harness = Harness {
                host: OverlayHost::new(cx),
                environment: UiEnvironment::new(
                    UiPreferences::default(),
                    ResolvedAppearance::Light,
                ),
                beneath: Rc::new(Cell::new(0)),
                closed: Rc::new(Cell::new(0)),
                focus: cx.focus_handle(),
            };
            window.focus(&harness.focus, cx);
            harness
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (view, cx)
    }

    /// Plays a surface's whole exit, which is what actually unmounts it.
    fn settle(cx: &mut gpui::VisualTestContext) {
        cx.executor().advance_clock(Duration::from_millis(
            tokens::motion::OVERLAY_CLOSE_DURATION_MS.into(),
        ));
        cx.run_until_parked();
    }

    fn press_invoker(cx: &mut gpui::VisualTestContext) {
        let invoker = cx.debug_bounds("button-Open").expect("the invoker renders");
        cx.simulate_click(invoker.center(), Modifiers::default());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn escape_closes_the_surface_and_gives_focus_back_to_its_invoker(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let invoker = cx
            .debug_bounds("button-Open")
            .expect("the invoker renders")
            .center();
        cx.simulate_click(invoker, Modifiers::default());
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.depth(cx), 1);
            assert_eq!(view.host.top(cx).as_deref(), Some("Delete this Mass?"));
        });
        assert!(cx.debug_bounds("overlay-Delete this Mass?").is_some());

        cx.simulate_keystrokes("escape");
        // The exit is played rather than skipped, so the surface is still
        // mounted and already reports itself closed.
        view.read_with(cx, |view, cx| assert_eq!(view.host.depth(cx), 0));
        settle(cx);

        view.read_with(cx, |view, _| assert_eq!(view.closed.get(), 1));
        assert!(cx.debug_bounds("overlay-Delete this Mass?").is_none());
        // The invoker is a button, and the press that opened the surface is
        // where focus has to come back to.
        assert!(cx.debug_bounds("button-Open").is_some());
    }

    /// `DESIGN.md` gives the scrim the first pointer event, and a control the
    /// operator cannot reach must not answer the pointer at all: no activation,
    /// no hover, no cursor of its own.
    #[gpui::test]
    fn the_scrim_dismisses_and_the_control_it_covers_stops_answering(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let beneath = cx
            .debug_bounds("button-Beneath")
            .expect("the covered control renders")
            .center();
        // The control answers the pointer while nothing covers it, or the
        // assertion below would pass for a control that never had a tooltip.
        cx.simulate_mouse_move(beneath, None, Modifiers::default());
        cx.executor()
            .advance_clock(crate::primitives::button::TOOLTIP_SHOW_DELAY);
        cx.run_until_parked();
        assert!(cx.debug_bounds("control-tooltip").is_some());

        press_invoker(cx);
        cx.simulate_mouse_move(beneath, None, Modifiers::default());
        cx.executor()
            .advance_clock(crate::primitives::button::TOOLTIP_SHOW_DELAY);
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("control-tooltip").is_none(),
            "a covered control is not hovered, so it neither names itself nor \
             lends the pointer a cursor"
        );

        cx.simulate_click(beneath, Modifiers::default());
        cx.run_until_parked();
        settle(cx);

        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.depth(cx), 0, "the press dismissed the surface");
            assert_eq!(
                view.beneath.get(),
                0,
                "and nothing underneath it was activated"
            );
        });
    }

    /// Tab is Ordo's to contain: the pin registers a focus trap and leaves the
    /// policy to the styled façade this crate rejects.
    #[gpui::test]
    fn tab_cycles_inside_the_surface_instead_of_escaping_it(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        press_invoker(cx);
        let trap = view.read_with(cx, |view, cx| {
            view.host
                .0
                .read(cx)
                .entries
                .last()
                .expect("one surface is open")
                .focus
                .clone()
        });

        for step in 0..6 {
            cx.simulate_keystrokes("tab");
            cx.run_until_parked();
            cx.update(|window, cx| {
                assert!(
                    trap.contains_focused(window, cx),
                    "Tab {step} left the modal surface"
                );
            });
        }
        for step in 0..6 {
            cx.simulate_keystrokes("shift-tab");
            cx.run_until_parked();
            cx.update(|window, cx| {
                assert!(
                    trap.contains_focused(window, cx),
                    "Shift+Tab {step} left the modal surface"
                );
            });
        }
    }

    #[gpui::test]
    fn a_nested_slideover_keeps_its_parent_and_returns_focus_to_it(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        press_invoker(cx);
        let (parent, child) = view.read_with(cx, |view, _| {
            (
                view.overlay(Surface::Slideover),
                Overlay::slideover("Song details").body(|_, _, _| div().into_any_element()),
            )
        });
        cx.update(|window, cx| view.read(cx).host.clone().open(parent, window, cx));
        cx.run_until_parked();
        let outer = view.read_with(cx, |view, cx| {
            view.host.0.read(cx).entries.last().unwrap().focus.clone()
        });

        cx.update(|window, cx| view.read(cx).host.clone().open(child, window, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.depth(cx), 3, "the parents are kept, not replaced");
            assert_eq!(view.host.top(cx).as_deref(), Some("Song details"));
        });
        assert!(cx.debug_bounds("overlay-Inspector").is_some());

        cx.simulate_keystrokes("escape");
        settle(cx);
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.depth(cx), 2);
            assert_eq!(view.host.top(cx).as_deref(), Some("Inspector"));
        });
        cx.update(|window, cx| {
            assert!(
                outer.contains_focused(window, cx),
                "closing the child returns focus to the parent that opened it"
            );
        });
    }

    /// A slideover advances from the right rather than appearing at rest, which
    /// is the half of `DESIGN.md`'s enter timing a test can see. It is also what
    /// the seeded first frame buys: a transition handed its resting value first
    /// would simply adopt it.
    #[gpui::test]
    fn a_slideover_advances_from_the_right_and_comes_to_rest(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let slideover = view.read_with(cx, |view, _| view.overlay(Surface::Slideover));
        cx.update(|window, cx| view.read(cx).host.clone().open(slideover, window, cx));
        // Drawn without letting the clock run, so this is the surface's first
        // frame rather than some way into its entry.
        cx.update(|window, cx| window.draw(cx).clear(cx));

        let entering = cx.debug_bounds("overlay-Inspector").expect("it is mounted");
        let viewport = cx.update(|window, _| window.viewport_size());
        assert!(
            entering.origin.x >= viewport.width,
            "it starts outside the viewport it advances into"
        );

        // The animation frame the transition asks for is the platform's, and
        // the test platform has none, so the frame is asked for by hand.
        cx.executor().advance_clock(Duration::from_millis(
            tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let resting = cx.debug_bounds("overlay-Inspector").unwrap();
        assert!(
            resting.right() <= viewport.width
                && viewport.width - resting.right()
                    <= px(tokens::density::WORKSPACE_GUTTER_WIDE + 1.0),
            "and comes to rest one exterior gutter from the edge"
        );
    }

    /// A dialog rises into its fade and falls back through it, because the pin
    /// has no transform to grow one with. See [`DIALOG_RISE`].
    #[gpui::test]
    fn a_dialog_rises_into_its_fade_and_falls_back_out_of_it(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        press_invoker(cx);
        let entering = cx
            .debug_bounds("overlay-Delete this Mass?")
            .expect("it is mounted");

        cx.executor().advance_clock(Duration::from_millis(
            tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let resting = cx.debug_bounds("overlay-Delete this Mass?").unwrap();
        assert_eq!(
            entering.origin.y - resting.origin.y,
            px(DIALOG_RISE),
            "it starts one spacing step low and rises exactly that far"
        );
        assert_eq!(
            entering.size, resting.size,
            "the rise is a correction after layout, so nothing reflows with it"
        );

        cx.simulate_keystrokes("escape");
        cx.executor().advance_clock(Duration::from_millis(
            (tokens::motion::OVERLAY_CLOSE_DURATION_MS / 2).into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let leaving = cx.debug_bounds("overlay-Delete this Mass?").unwrap();
        assert!(
            leaving.origin.y > resting.origin.y,
            "and falls back through on the way out"
        );
        settle(cx);
        view.read_with(cx, |view, cx| assert_eq!(view.host.depth(cx), 0));
    }

    /// Motion may never delay the next input: a surface still playing its exit
    /// does not hold up the one that replaces it, and its pending removal takes
    /// only itself.
    #[gpui::test]
    fn a_surface_opened_during_an_exit_survives_that_exit(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        press_invoker(cx);
        cx.simulate_keystrokes("escape");

        let replacement = Overlay::slideover("Inspector");
        cx.update(|window, cx| view.read(cx).host.clone().open(replacement, window, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.top(cx).as_deref(), Some("Inspector"));
        });

        settle(cx);
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.depth(cx), 1, "only the closing surface left");
            assert_eq!(view.host.top(cx).as_deref(), Some("Inspector"));
        });
        assert!(cx.debug_bounds("overlay-Delete this Mass?").is_none());
        assert!(cx.debug_bounds("overlay-Inspector").is_some());
    }

    /// Reduced motion removes the movement, and with it the wait: the exit is
    /// not merely shortened, there is nothing left to play.
    #[gpui::test]
    fn reduced_motion_removes_the_movement_and_the_wait(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        cx.update(|_, cx| cx.set_reduce_motion(true));
        press_invoker(cx);
        let entered = cx
            .debug_bounds("overlay-Delete this Mass?")
            .expect("the surface is at its resting place on the first frame");

        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(
                view.host.depth(cx),
                0,
                "no exit is played, so nothing waits on one"
            );
        });
        assert!(cx.debug_bounds("overlay-Delete this Mass?").is_none());
        assert!(entered.size.width > px(0.0));
    }

    #[gpui::test]
    fn a_confirmation_expires_and_an_owned_failure_waits_to_be_dismissed(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        cx.update(|_, cx| {
            let host = view.read(cx).host.clone();
            host.notify(
                Toast::confirmation("Cue removed.").undo("Undo", |_, _| {}),
                cx,
            );
            host.notify(Toast::failure("Congregation display disconnected."), cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.toasts(cx).len(), 2);
        });
        assert!(cx.debug_bounds("toast-Cue removed.").is_some());
        assert!(cx.debug_bounds("button-Undo").is_some());
        // `DESIGN.md` asks for one deduplicated status channel, so the same
        // operational change reported again replaces its own toast.
        let host = view.read_with(cx, |view, _| view.host.clone());
        cx.update(|_, cx| {
            host.notify(
                Toast::confirmation("Cue removed.").undo("Undo", |_, _| {}),
                cx,
            )
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.host.toasts(cx).len(), 2, "the repeat replaced itself");
        });

        cx.executor()
            .advance_clock(TOAST_TIMEOUT + Duration::from_secs(1));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(
                view.host.toasts(cx),
                vec![SharedString::from("Congregation display disconnected.")],
                "the confirmation expired and the owned failure did not"
            );
        });

        cx.update(|_, cx| host.dismiss_toast("Congregation display disconnected.", cx));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(
                view.host.toasts(cx).len(),
                1,
                "a dismissed toast is still on screen for the length of its exit"
            );
        });
        settle(cx);
        view.read_with(cx, |view, cx| assert!(view.host.toasts(cx).is_empty()));
    }

    /// A toast is out of the flow for the frame it is measured on, so the
    /// stack's own geometry is what the frame after that draws.
    fn mounted(cx: &mut gpui::VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }

    /// A toast arrives from below and leaves the same way, rather than cutting
    /// in and out. See [`TOAST_RISE`].
    #[gpui::test]
    fn a_toast_arrives_from_below_and_falls_back_out(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let host = view.read_with(cx, |view, _| view.host.clone());
        cx.update(|_, cx| host.notify(Toast::failure("Congregation display lost."), cx));
        mounted(cx);
        let arriving = cx
            .debug_bounds("toast-Congregation display lost.")
            .expect("it is mounted");

        cx.executor().advance_clock(Duration::from_millis(
            tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let resting = cx.debug_bounds("toast-Congregation display lost.").unwrap();
        assert_eq!(
            arriving.origin.y - resting.origin.y,
            px(TOAST_RISE),
            "it arrives one control height below where it rests"
        );

        cx.update(|_, cx| host.dismiss_toast("Congregation display lost.", cx));
        cx.executor().advance_clock(Duration::from_millis(
            (tokens::motion::OVERLAY_CLOSE_DURATION_MS / 2).into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("toast-Congregation display lost.")
                .unwrap()
                .origin
                .y
                > resting.origin.y,
            "and falls back through on the way out rather than vanishing"
        );
    }

    /// The toast a newcomer pushes up rides up rather than being redrawn a row
    /// higher, which is the only movement in the stack the layout would
    /// otherwise draw as a cut.
    #[gpui::test]
    fn an_earlier_toast_rides_up_for_the_one_that_arrives_under_it(cx: &mut TestAppContext) {
        let (view, cx) = open(cx);
        let host = view.read_with(cx, |view, _| view.host.clone());
        cx.update(|_, cx| host.notify(Toast::failure("Congregation display lost."), cx));
        mounted(cx);
        cx.executor().advance_clock(Duration::from_millis(
            tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let alone = cx.debug_bounds("toast-Congregation display lost.").unwrap();

        cx.update(|_, cx| host.notify(Toast::confirmation("Cue removed."), cx));
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("toast-Congregation display lost.")
                .unwrap()
                .origin,
            alone.origin,
            "the frame the newcomer is measured on leaves the stack alone"
        );
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("toast-Congregation display lost.")
                .unwrap()
                .origin,
            alone.origin,
            "and the frame it joins the stack on starts where the earlier toast \
             already was, rather than a row above it"
        );

        cx.executor().advance_clock(Duration::from_millis(
            tokens::motion::OVERLAY_OPEN_DURATION_MS.into(),
        ));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("toast-Congregation display lost.")
                .unwrap()
                .origin
                .y
                < alone.origin.y,
            "it rides up over the arrival and comes to rest a row higher"
        );
    }

    /// The final node the operator's assistive technology reads, captured from
    /// the element because the test platform activates no tree; see
    /// `docs/qualification/GPUI-BASE-T0.3.md`.
    #[gpui::test]
    fn the_surface_names_and_describes_itself_as_a_dialog(cx: &mut TestAppContext) {
        use gpui::{Element, accesskit};
        let (view, cx) = open(cx);
        let overlay = view.read_with(cx, |view, _| view.overlay(Surface::Slideover));
        let element = semantics(&overlay, 0);
        let mut node = accesskit::Node::new(element.a11y_role().expect("a semantic element"));
        element.write_a11y_info(&mut node);

        assert_eq!(node.role(), accesskit::Role::Dialog);
        assert_eq!(node.label(), Some("Inspector"));
        assert_eq!(
            node.description(),
            Some("This cannot be undone."),
            "complete text is a description, never part of the name"
        );
    }

    /// Overlays must sit under a popup opened from inside one, and both must
    /// sit under a tooltip. Base states the same relation for its own layers.
    #[test]
    fn the_layer_order_puts_a_popup_above_every_overlay() {
        const {
            assert!(OVERLAY_PRIORITY < TOAST_PRIORITY);
            assert!(TOAST_PRIORITY < gpui_base::POPUP_PRIORITY);
        }
    }
}
