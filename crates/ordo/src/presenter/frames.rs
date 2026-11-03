//! The Presenter's frame store: the GPUI-side owner that turns Engine state
//! into displayable frames (T1.4 WP3, Decision 310).
//!
//! The pure parts live in `rendering` ([`FrameCache`], [`FrameScheduler`],
//! [`compose`]); this module only owns them and runs the compose jobs on the
//! background executor. Composition never happens on the GPUI thread, and a
//! resize or scale change never recomposes: the key is the [`FrameId`] alone,
//! and the viewport rescales the retained logical frame.
//!
//! # Ownership and shutdown
//!
//! One `PresenterFrames` belongs to one view (the Presenter view) and one
//! [`LoadedSnapshot`]; a new snapshot gets a new value. Every running job is
//! a [`Task`] held in `in_flight`, so dropping the store (with its owning
//! view) drops every task and cancels the work. Obsolete jobs are dropped
//! the same way as soon as the scheduler abandons their tickets.
//!
//! The store does not own the Engine. A composition that fails is queued and
//! handed out by [`PresenterFrames::take_failures`] so the view can call
//! `engine.mark_frame_unavailable` for each.

use std::collections::HashMap;
use std::sync::Arc;

use domain::presentation::{AssetId, FrameId};
use gpui::{AppContext, Context, Image, ImageFormat, Task};
use presentation::snapshot_reader::LoadedSnapshot;
use rendering::{
    Completion, FRAME_CAPACITY, Frame, FrameCache, FrameEntry, FrameScheduler, FrameSlot, Ticket,
    UnavailableReason, compose,
};

/// What a view should draw for one frame identity right now.
#[derive(Clone, Debug)]
pub enum FrameView {
    /// A complete composed frame, plus the decoded foreground image when the
    /// frame places one.
    Ready {
        frame: Arc<Frame>,
        image: Option<Arc<Image>>,
    },
    /// Not composed yet. The caller keeps drawing the last frame it drew
    /// (the consumer rule in `rendering::frame_cache`).
    Pending,
    /// Composition failed for a structured reason.
    Unavailable(UnavailableReason),
}

/// The outcome of one background job: the composed frame (or why not) and
/// the image it wrapped, if the asset was not already cached.
struct Prepared {
    result: Result<Frame, UnavailableReason>,
    image: Option<(AssetId, Arc<Image>)>,
}

/// Owner of composed frames, decoded images and in-flight compose jobs for
/// one snapshot. See the module docs for ownership and shutdown.
pub struct PresenterFrames {
    snapshot: Arc<LoadedSnapshot>,
    cache: FrameCache<FrameId>,
    scheduler: FrameScheduler<FrameId>,
    /// One image per asset of this snapshot, so the number of entries is
    /// bounded by the snapshot's asset count.
    images: HashMap<AssetId, Arc<Image>>,
    /// Dropping a task cancels its job.
    in_flight: HashMap<FrameId, Task<()>>,
    composed_count: usize,
    failures: Vec<(FrameId, UnavailableReason)>,
    /// Test seam: pretend the snapshot carries no asset bytes, the only way
    /// to reach the missing-asset path through a verified snapshot.
    #[cfg(test)]
    withhold_assets: bool,
}

impl PresenterFrames {
    pub fn new(snapshot: Arc<LoadedSnapshot>) -> Self {
        Self {
            snapshot,
            cache: FrameCache::new(FRAME_CAPACITY),
            scheduler: FrameScheduler::new(),
            images: HashMap::new(),
            in_flight: HashMap::new(),
            composed_count: 0,
            failures: Vec::new(),
            #[cfg(test)]
            withhold_assets: false,
        }
    }

    /// Bring the store in line with what the Engine now shows.
    ///
    /// Pins Program and Preview, requests both and the preload set (latest
    /// wins per slot), drops the jobs the scheduler reports obsolete, and
    /// starts one background job per new ticket. Each job composes with
    /// [`compose`] off the GPUI thread and hops back through `access`, which
    /// must return this store from the owning view `V`. Finishes with
    /// `cx.notify()` so the view repaints.
    pub fn sync<V: 'static>(
        &mut self,
        program: Option<FrameId>,
        preview: Option<FrameId>,
        preload: impl IntoIterator<Item = FrameId>,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) {
        self.cache.pin(FrameSlot::Program, program);
        self.cache.pin(FrameSlot::Preview, preview);

        let mut tickets = Vec::new();
        if let Some(id) = program {
            tickets.extend(self.scheduler.request(FrameSlot::Program, id, &self.cache));
        }
        if let Some(id) = preview {
            tickets.extend(self.scheduler.request(FrameSlot::Preview, id, &self.cache));
        }
        tickets.extend(self.scheduler.request_preload(preload, &self.cache));

        for ticket in self.scheduler.obsolete() {
            if self.scheduler.abandon(&ticket) {
                self.in_flight.remove(&ticket.key());
            }
        }

        for ticket in tickets {
            let task = self.start_job(ticket, access, cx);
            self.in_flight.insert(ticket.key(), task);
        }
        cx.notify();
    }

    fn start_job<V: 'static>(
        &self,
        ticket: Ticket<FrameId>,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) -> Task<()> {
        let key = ticket.key();
        let snapshot = self.snapshot.clone();
        let known = self.known_image(key);
        #[cfg(test)]
        let withhold = self.withhold_assets;
        #[cfg(not(test))]
        let withhold = false;
        let job = cx.background_spawn(async move { prepare(&snapshot, key, known, withhold) });
        cx.spawn(async move |this, cx| {
            let prepared = job.await;
            this.update(cx, |view, cx| {
                access(view).finish(ticket, prepared);
                cx.notify();
            })
            .ok();
        })
    }

    /// The image already wrapped for the asset `key`'s frame uses, so a job
    /// does not build a second copy.
    fn known_image(&self, key: FrameId) -> Option<Arc<Image>> {
        let content = self.snapshot.frame(key)?;
        let asset = AssetId::parse(&content.image.as_ref()?.id).ok()?;
        self.images.get(&asset).cloned()
    }

    fn finish(&mut self, ticket: Ticket<FrameId>, prepared: Prepared) {
        // Dropping the finished task here is safe: the job has nothing left
        // to await.
        self.in_flight.remove(&ticket.key());
        self.composed_count += 1;
        let failure = prepared.result.as_ref().err().copied();
        let completion = self
            .scheduler
            .complete(ticket, prepared.result, &mut self.cache);
        if completion != Completion::Accepted {
            return;
        }
        match (failure, prepared.image) {
            (Some(reason), _) => self.failures.push((ticket.key(), reason)),
            (None, Some((asset, image))) => {
                self.images.insert(asset, image);
            }
            (None, None) => {}
        }
    }

    /// What to draw for `id`. Marks a ready frame recently used.
    pub fn get(&mut self, id: FrameId) -> FrameView {
        let frame = match self.cache.get(&id) {
            Some(FrameEntry::Ready(frame)) => Arc::clone(frame),
            Some(FrameEntry::Unavailable(reason)) => return FrameView::Unavailable(*reason),
            None => return FrameView::Pending,
        };
        let image = frame
            .image()
            .and_then(|placed| AssetId::parse(&placed.id).ok())
            .and_then(|asset| self.images.get(&asset).cloned());
        FrameView::Ready { frame, image }
    }

    /// Frames whose composition failed since the last call, for the view to
    /// report to the Engine. Each failure is handed out once.
    pub fn take_failures(&mut self) -> Vec<(FrameId, UnavailableReason)> {
        std::mem::take(&mut self.failures)
    }

    /// Compose jobs that have finished (accepted or not). Resizing never
    /// changes it.
    pub fn composed_count(&self) -> usize {
        self.composed_count
    }

    /// Entries (frames and Unavailable results) held by the cache.
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Jobs started and not yet finished or dropped.
    pub fn jobs_in_flight(&self) -> usize {
        self.in_flight.len()
    }
}

/// The background job: resolve and wrap the foreground asset, then compose.
/// Runs off the GPUI thread. `Image::from_bytes` copies the asset bytes here,
/// not on the UI thread; GPUI decodes the PNG lazily when the image is first
/// painted.
fn prepare(
    snapshot: &LoadedSnapshot,
    key: FrameId,
    known: Option<Arc<Image>>,
    withhold_assets: bool,
) -> Prepared {
    let Some(content) = snapshot.frame(key) else {
        return failed(UnavailableReason::DecodeFailed);
    };
    let mut image = None;
    if let Some(foreground) = &content.image {
        let Ok(asset) = AssetId::parse(&foreground.id) else {
            return failed(UnavailableReason::DecodeFailed);
        };
        if known.is_none() {
            let bytes = if withhold_assets {
                None
            } else {
                snapshot.asset(asset)
            };
            let Some(bytes) = bytes else {
                return failed(UnavailableReason::DecodeFailed);
            };
            image = Some((
                asset,
                Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec())),
            ));
        }
    }
    Prepared {
        result: compose(content),
        image,
    }
}

fn failed(reason: UnavailableReason) -> Prepared {
    Prepared {
        result: Err(reason),
        image: None,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;
    use std::sync::Arc;

    use domain::presentation::FrameId;
    use presentation::snapshot_reader::{DirectorySource, LoadedSnapshot, read_snapshot};

    /// The committed, sanitized `normal-mass` fixture, read from the source
    /// tree (read-only).
    pub fn fixture_snapshot() -> Arc<LoadedSnapshot> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/presentations/normal-mass");
        Arc::new(read_snapshot(&DirectorySource::new(dir)).expect("fixture snapshot loads"))
    }

    /// Frame identities in stable order; `with_image` picks those that do
    /// (or do not) place a foreground image.
    pub fn frame_ids(snapshot: &LoadedSnapshot, with_image: bool) -> Vec<FrameId> {
        snapshot
            .snapshot()
            .frame_ids()
            .into_iter()
            .filter(|id| {
                snapshot
                    .frame(*id)
                    .is_some_and(|content| content.image.is_some() == with_image)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{fixture_snapshot, frame_ids};
    use super::*;
    use gpui::{Render, TestAppContext, Window, div, prelude::*, px, size};

    struct Host {
        frames: PresenterFrames,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    fn access(host: &mut Host) -> &mut PresenterFrames {
        &mut host.frames
    }

    fn open(
        cx: &mut TestAppContext,
        snapshot: Arc<LoadedSnapshot>,
        withhold_assets: bool,
    ) -> (gpui::Entity<Host>, &mut gpui::VisualTestContext) {
        cx.update(ui::init).unwrap();
        let (host, cx) = cx.add_window_view(|_, _| {
            let mut frames = PresenterFrames::new(snapshot);
            frames.withhold_assets = withhold_assets;
            Host { frames }
        });
        cx.simulate_resize(size(px(800.0), px(600.0)));
        cx.run_until_parked();
        (host, cx)
    }

    fn sync(
        host: &gpui::Entity<Host>,
        cx: &mut gpui::VisualTestContext,
        program: Option<FrameId>,
        preview: Option<FrameId>,
    ) {
        host.update(cx, |host, cx| {
            host.frames
                .sync(program, preview, std::iter::empty(), access, cx);
        });
    }

    fn view(host: &gpui::Entity<Host>, cx: &mut gpui::VisualTestContext, id: FrameId) -> FrameView {
        host.update(cx, |host, _| host.frames.get(id))
    }

    fn counts(host: &gpui::Entity<Host>, cx: &mut gpui::VisualTestContext) -> (usize, usize) {
        host.read_with(cx, |host, _| {
            (host.frames.composed_count(), host.frames.cache_len())
        })
    }

    #[gpui::test]
    fn program_and_preview_compose_in_the_background_and_become_ready(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let plain = frame_ids(&snapshot, false);
        let (a, b) = (plain[0], plain[1]);
        let (host, cx) = open(cx, snapshot, false);

        sync(&host, cx, Some(a), Some(b));
        // Nothing is composed on the calling thread: the jobs are queued.
        assert!(matches!(view(&host, cx, a), FrameView::Pending));
        assert_eq!(host.read_with(cx, |h, _| h.frames.jobs_in_flight()), 2);

        cx.run_until_parked();
        assert!(matches!(view(&host, cx, a), FrameView::Ready { .. }));
        assert!(matches!(view(&host, cx, b), FrameView::Ready { .. }));
        assert_eq!(counts(&host, cx), (2, 2));
        assert_eq!(host.read_with(cx, |h, _| h.frames.jobs_in_flight()), 0);
    }

    #[gpui::test]
    fn a_superseded_preview_job_is_abandoned_and_the_new_one_composes(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let plain = frame_ids(&snapshot, false);
        let (a, b, c) = (plain[0], plain[1], plain[2]);
        let (host, cx) = open(cx, snapshot, false);

        sync(&host, cx, Some(a), Some(b));
        sync(&host, cx, Some(a), Some(c));
        // B was still pending and nothing wants it: its task is gone.
        assert_eq!(host.read_with(cx, |h, _| h.frames.jobs_in_flight()), 2);

        cx.run_until_parked();
        assert!(matches!(view(&host, cx, a), FrameView::Ready { .. }));
        assert!(matches!(view(&host, cx, c), FrameView::Ready { .. }));
        assert!(matches!(view(&host, cx, b), FrameView::Pending));
        assert_eq!(counts(&host, cx), (2, 2));
    }

    #[gpui::test]
    fn the_same_keys_and_a_resize_never_recompose(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let plain = frame_ids(&snapshot, false);
        let (a, b) = (plain[0], plain[1]);
        let (host, cx) = open(cx, snapshot, false);
        sync(&host, cx, Some(a), Some(b));
        cx.run_until_parked();
        let before = counts(&host, cx);

        sync(&host, cx, Some(a), Some(b));
        cx.run_until_parked();
        assert_eq!(counts(&host, cx), before);

        cx.simulate_resize(size(px(1280.0), px(720.0)));
        cx.run_until_parked();
        assert_eq!(counts(&host, cx), before);
    }

    #[gpui::test]
    fn an_image_frame_is_ready_with_its_decoded_image(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, false);

        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        match view(&host, cx, image_frame) {
            FrameView::Ready { frame, image } => {
                assert!(frame.image().is_some());
                assert!(image.is_some(), "the placed image must carry its asset");
            }
            other => panic!("expected a ready image frame, found {other:?}"),
        }
        assert!(host.update(cx, |h, _| h.frames.take_failures()).is_empty());
    }

    #[gpui::test]
    fn an_image_frame_without_its_asset_is_unavailable_and_reported(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, true);

        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        assert!(matches!(
            view(&host, cx, image_frame),
            FrameView::Unavailable(UnavailableReason::DecodeFailed)
        ));
        let failures = host.update(cx, |h, _| h.frames.take_failures());
        assert_eq!(
            failures,
            vec![(image_frame, UnavailableReason::DecodeFailed)]
        );
        assert!(host.update(cx, |h, _| h.frames.take_failures()).is_empty());
    }
}
