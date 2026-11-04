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
//! The owner is the Presenter view: it holds one `PresenterFrames` per
//! [`LoadedSnapshot`] (a new snapshot gets a new value) and calls
//! [`PresenterFrames::sync`] when the Engine changes, never from `render`.
//! Every running job is a [`Task`] held by the store: compose jobs in
//! `in_flight`, full-size image upgrades in `upgrades`, and the one-shot
//! atlas release in `flush`. Dropping the store (with its owning view) drops
//! every task. Obsolete jobs are dropped the same way as soon as the
//! scheduler abandons their tickets or no wanted frame needs their asset. A
//! background job that is already running cannot be interrupted: it finishes
//! and its result is discarded.
//!
//! # Images: eager, bounded, two sizes
//!
//! Foreground images are decoded eagerly inside the job (not lazily at first
//! paint), so a frame is only ever Ready with its image, and a corrupt asset
//! becomes [`UnavailableReason::DecodeFailed`] instead of a silent gap.
//!
//! Memory is bounded like frames. A decoded image is kept only while a Ready
//! frame in the cache places its asset, so the decoded images never outnumber
//! the cache's [`FRAME_CAPACITY`]. After every completion, `sync` and
//! `request_visible` the store releases the rest. A released image is dropped
//! from GPUI's sprite atlas only once nothing else holds it (a monitor that is
//! still painting it keeps its `Arc`); until then it waits in `retiring` and
//! the atlas copy is dropped from a one-shot hop (never from inside an event
//! handler, where the window may be leased and the drop would be skipped).
//!
//! Frames wanted only as thumbnails (`FrameSlot::Visible`) decode at
//! `THUMBNAIL_IMAGE_MAX_PX`; Program, Preview and Preload decode in full. A
//! frame promoted from thumbnail to one of those slots keeps its thumbnail
//! while a background upgrade job decodes the full image, and the monitors
//! read through [`PresenterFrames::get_full`] / [`PresenterFrames::peek_full`],
//! which keep an image frame `Pending` until the full image exists, so
//! nothing partial ever reaches the congregation. [`PresenterFrames::get`] and
//! [`PresenterFrames::peek`] (the timeline thumbnails) show whichever image
//! exists.
//!
//! The store does not own the Engine. A composition that fails is queued and
//! handed out by [`PresenterFrames::take_failures`] so the view can call
//! `engine.mark_frame_unavailable` for each.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use domain::presentation::{AssetId, FrameId};
use gpui::{App, AppContext, Context, ImageId, RenderImage, SvgRenderer, Task, Window};
use presentation::snapshot_reader::LoadedSnapshot;
use rendering::{
    Completion, FRAME_CAPACITY, Frame, FrameCache, FrameEntry, FrameScheduler, FrameSlot,
    PRELOAD_RADIUS, Ticket, UnavailableReason, compose,
};

use super::images::{decode_full, decode_thumbnail};

/// What a view should draw for one frame identity right now.
#[derive(Clone, Debug)]
pub enum FrameView {
    /// A complete composed frame, plus the decoded foreground image when the
    /// frame places one. Which size the image is depends on the accessor:
    /// [`PresenterFrames::get`] and [`PresenterFrames::peek`] may return a
    /// thumbnail-sized one; the `_full` accessors only ever return the full
    /// image (and `Pending` until it exists).
    Ready {
        frame: Arc<Frame>,
        image: Option<Arc<RenderImage>>,
    },
    /// Not composed yet. The caller keeps drawing the last frame it drew
    /// (the consumer rule in `rendering::frame_cache`).
    Pending,
    /// Composition failed for a structured reason.
    Unavailable(UnavailableReason),
}

/// Which decode a job makes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Quality {
    /// At most `THUMBNAIL_IMAGE_MAX_PX` on the longest side.
    Thumb,
    /// The asset's own size.
    Full,
}

/// The outcome of one background job: the composed frame (or why not) and
/// the image it decoded, if one was needed.
struct Prepared {
    result: Result<Frame, UnavailableReason>,
    /// True when `compose` actually ran.
    composed: bool,
    image: Option<(AssetId, Quality, Arc<RenderImage>)>,
}

/// Owner of composed frames, decoded images and in-flight jobs for one
/// snapshot. See the module docs for ownership and shutdown.
pub struct PresenterFrames {
    snapshot: Arc<LoadedSnapshot>,
    cache: FrameCache<FrameId>,
    scheduler: FrameScheduler<FrameId>,
    /// Full-size decoded images, keyed by `AssetId` rather than by asset hash
    /// (Decision 310): within one snapshot an `AssetId` names exactly one
    /// verified byte string, so the two keys select the same entries, and a
    /// new snapshot gets a new store. Only assets placed by a Ready cached
    /// frame are kept.
    full: HashMap<AssetId, Arc<RenderImage>>,
    /// Thumbnail-sized decoded images, under the same rule. An asset has a
    /// thumbnail only until its full image arrives.
    thumb: HashMap<AssetId, Arc<RenderImage>>,
    /// Released images some monitor may still be painting, keyed by the
    /// image's own id so one image is never listed twice; their atlas copy is
    /// dropped once this is the last holder. An image is never in here and in
    /// `full`/`thumb` at once: every insert into those maps removes it from
    /// here first, so a re-inserted image cannot keep a stale extra `Arc`.
    retiring: HashMap<ImageId, Arc<RenderImage>>,
    /// The frames Program, Preview and Preload want now, which need the full
    /// image.
    full_wanted: Vec<FrameId>,
    /// Dropping a task cancels its job. The quality says which decode the
    /// job makes, so an upgrade is not started over a full compose job.
    in_flight: HashMap<FrameId, (Quality, Task<()>)>,
    /// Full-size decodes that promote a thumbnail-only asset, one per asset.
    upgrades: HashMap<AssetId, Task<()>>,
    /// Assets whose full decode failed; their wanted frames become
    /// Unavailable instead of retrying in a loop. Never cleared: a snapshot
    /// is immutable and a decode of the same bytes is deterministic, so a
    /// retry could only fail again, and a new snapshot gets a new store.
    failed_assets: HashSet<AssetId>,
    /// The one-shot hop that drops released images from the atlas.
    flush: Option<Task<()>>,
    composed_count: usize,
    failures: Vec<(FrameId, UnavailableReason)>,
    /// Test seam: atlas drops issued, so a test can see each image dropped
    /// exactly once.
    #[cfg(test)]
    dropped: usize,
    /// Test seam: a verified snapshot never lacks or corrupts an asset, so
    /// this is the only way to reach those paths.
    #[cfg(test)]
    fault: AssetFault,
    /// Test seam: how long a finished job waits (on the executor clock)
    /// before its result is handed back, so a test can hold a frame in
    /// `Pending` and release it by advancing the clock.
    #[cfg(test)]
    hold: std::time::Duration,
}

/// Where a test injects an asset fault. Production is always `None`.
#[derive(Clone, Copy)]
enum AssetFault {
    None,
    #[cfg(test)]
    Withhold,
    #[cfg(test)]
    Corrupt,
}

/// Frames wanted only as thumbnails decode small; every other slot decodes
/// in full.
fn quality_of(ticket: &Ticket<FrameId>) -> Quality {
    if ticket.slot() == FrameSlot::Visible {
        Quality::Thumb
    } else {
        Quality::Full
    }
}

/// The asset a frame places, if any.
fn asset_of(snapshot: &LoadedSnapshot, id: FrameId) -> Option<AssetId> {
    let content = snapshot.frame(id)?;
    AssetId::parse(&content.image.as_ref()?.id).ok()
}

impl PresenterFrames {
    pub fn new(snapshot: Arc<LoadedSnapshot>) -> Self {
        Self {
            snapshot,
            cache: FrameCache::new(FRAME_CAPACITY),
            scheduler: FrameScheduler::new(),
            full: HashMap::new(),
            thumb: HashMap::new(),
            retiring: HashMap::new(),
            full_wanted: Vec::new(),
            in_flight: HashMap::new(),
            upgrades: HashMap::new(),
            failed_assets: HashSet::new(),
            flush: None,
            composed_count: 0,
            failures: Vec::new(),
            #[cfg(test)]
            dropped: 0,
            #[cfg(test)]
            fault: AssetFault::None,
            #[cfg(test)]
            hold: std::time::Duration::ZERO,
        }
    }

    /// A store whose cache holds `capacity` entries (clamped up to the
    /// cache's minimum), so a test can force evictions with a small fixture.
    #[cfg(test)]
    fn with_capacity(snapshot: Arc<LoadedSnapshot>, capacity: usize) -> Self {
        let mut frames = Self::new(snapshot);
        frames.cache = FrameCache::new(capacity);
        frames
    }

    /// Bring the store in line with what the Engine now shows.
    ///
    /// Pins Program and Preview, requests both and the preload set (latest
    /// wins per slot), drops the jobs the scheduler reports obsolete, and
    /// starts one background job per new ticket. Each job composes with
    /// [`compose`] off the GPUI thread and hops back through `access`, which
    /// must return this store from the owning view `V`. A frame that is
    /// already cached with only a thumbnail image gets a background upgrade to
    /// the full image. Releases images no cached frame uses. Finishes with
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

        // The same set the scheduler takes: distinct keys, in order, capped.
        let mut preload_set: Vec<FrameId> = Vec::new();
        for id in preload {
            if preload_set.len() == 2 * PRELOAD_RADIUS {
                break;
            }
            if !preload_set.contains(&id) {
                preload_set.push(id);
            }
        }
        let preload = preload_set;
        self.full_wanted = program
            .into_iter()
            .chain(preview)
            .chain(preload.iter().copied())
            .collect();

        let mut tickets = Vec::new();
        if let Some(id) = program {
            tickets.extend(self.scheduler.request(FrameSlot::Program, id, &self.cache));
        }
        if let Some(id) = preview {
            tickets.extend(self.scheduler.request(FrameSlot::Preview, id, &self.cache));
        }
        tickets.extend(self.scheduler.request_preload(preload, &self.cache));

        let abandoned = self.drop_obsolete();
        let started = !tickets.is_empty();
        for ticket in tickets {
            let task = self.start_job(ticket, access, cx);
            self.in_flight
                .insert(ticket.key(), (quality_of(&ticket), task));
        }
        let changed = self.after_change(access, cx);
        if started || abandoned || changed {
            cx.notify();
        }
    }

    /// Replace the set of frames the timeline's thumbnails want (the lowest
    /// priority; see [`rendering::FrameScheduler::request_visible`]), nearest
    /// first, and start a job for each that is neither composed nor in flight.
    /// Like [`PresenterFrames::sync`] it runs after state changed, never from
    /// `render`. At most `MAX_VISIBLE` keys are taken, so the cache stays
    /// within `FRAME_CAPACITY`; they are never pinned. A frame requested only
    /// here decodes its image at thumbnail size.
    pub fn request_visible<V: 'static>(
        &mut self,
        ids: impl IntoIterator<Item = FrameId>,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) {
        let tickets = self.scheduler.request_visible(ids, &self.cache);
        let abandoned = self.drop_obsolete();
        let started = !tickets.is_empty();
        for ticket in tickets {
            let task = self.start_job(ticket, access, cx);
            self.in_flight
                .insert(ticket.key(), (quality_of(&ticket), task));
        }
        let changed = self.after_change(access, cx);
        if started || abandoned || changed {
            cx.notify();
        }
    }

    /// Drop the tasks of jobs the scheduler no longer wants; true if any.
    fn drop_obsolete(&mut self) -> bool {
        let mut abandoned = false;
        for ticket in self.scheduler.obsolete() {
            if self.scheduler.abandon(&ticket) {
                self.in_flight.remove(&ticket.key());
                abandoned = true;
            }
        }
        abandoned
    }

    /// The bytes of `asset`, honoring the test fault seam.
    fn asset_bytes(
        snapshot: &LoadedSnapshot,
        asset: AssetId,
        fault: AssetFault,
    ) -> Option<Cow<'_, [u8]>> {
        match fault {
            AssetFault::None => snapshot.asset(asset).map(Cow::Borrowed),
            #[cfg(test)]
            AssetFault::Withhold => None,
            #[cfg(test)]
            AssetFault::Corrupt => Some(Cow::Borrowed(b"not a png")),
        }
    }

    fn start_job<V: 'static>(
        &self,
        ticket: Ticket<FrameId>,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) -> Task<()> {
        let key = ticket.key();
        let snapshot = self.snapshot.clone();
        let quality = quality_of(&ticket);
        let known = self.known_image(key, quality);
        #[cfg(test)]
        let fault = self.fault;
        #[cfg(not(test))]
        let fault = AssetFault::None;
        #[cfg(test)]
        let hold = self.hold;
        let renderer = cx.svg_renderer();
        let job = cx.background_spawn(async move {
            prepare(&snapshot, key, quality, known, &renderer, fault)
        });
        cx.spawn(async move |this, cx| {
            let prepared = job.await;
            #[cfg(test)]
            if !hold.is_zero() {
                cx.background_executor().timer(hold).await;
            }
            this.update(cx, |view, cx| {
                access(view).finish(ticket, prepared, access, cx);
                cx.notify();
            })
            .ok();
        })
    }

    /// The image already decoded for the asset `key`'s frame uses and good
    /// enough for `quality`, so a job does not decode a second copy. A full
    /// image serves both qualities; a thumbnail serves only `Thumb`. The job
    /// hands it back with its result, so the image is still held if the frame
    /// that owned it was evicted while the job ran.
    fn known_image(&self, key: FrameId, quality: Quality) -> Option<(Quality, Arc<RenderImage>)> {
        let asset = asset_of(&self.snapshot, key)?;
        if let Some(full) = self.full.get(&asset) {
            return Some((Quality::Full, Arc::clone(full)));
        }
        match quality {
            Quality::Full => None,
            Quality::Thumb => self
                .thumb
                .get(&asset)
                .map(|thumb| (Quality::Thumb, Arc::clone(thumb))),
        }
    }

    fn finish<V: 'static>(
        &mut self,
        ticket: Ticket<FrameId>,
        prepared: Prepared,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) {
        // Dropping the finished task here is safe: the job has nothing left
        // to await.
        self.in_flight.remove(&ticket.key());
        if prepared.composed {
            self.composed_count += 1;
        }
        let failure = prepared.result.as_ref().err().copied();
        let completion = self
            .scheduler
            .complete(ticket, prepared.result, &mut self.cache);
        // Defensive: `sync` drops obsolete tasks, so a stale result should
        // not arrive; the scheduler still decides if one does.
        if completion == Completion::Accepted {
            match (failure, prepared.image) {
                (Some(reason), _) => self.failures.push((ticket.key(), reason)),
                (None, Some((asset, quality, image))) => self.store_image(asset, quality, image),
                (None, None) => {}
            }
        }
        // The insert may have evicted a frame, and a thumbnail-only frame may
        // already be wanted in full.
        self.after_change(access, cx);
    }

    /// Keep a freshly decoded image. A thumbnail never replaces or outlives a
    /// full image of the same asset; a full image retires the thumbnail.
    fn store_image(&mut self, asset: AssetId, quality: Quality, image: Arc<RenderImage>) {
        match quality {
            Quality::Full => {
                self.retiring.remove(&image.id);
                self.full.insert(asset, image);
                if let Some(thumb) = self.thumb.remove(&asset) {
                    self.retire(thumb);
                }
            }
            Quality::Thumb => {
                if !self.full.contains_key(&asset) {
                    self.retiring.remove(&image.id);
                    self.thumb.insert(asset, image);
                }
            }
        }
    }

    /// List `image` for its atlas drop. Listing is by image id, so an image
    /// that is retired twice is still one entry.
    fn retire(&mut self, image: Arc<RenderImage>) {
        self.retiring.insert(image.id, image);
    }

    /// The assets placed by Ready cached frames.
    fn referenced_assets(&self) -> HashSet<AssetId> {
        self.cache
            .keys()
            .filter(|id| matches!(self.cache.peek(id), Some(FrameEntry::Ready(_))))
            .filter_map(|id| asset_of(&self.snapshot, *id))
            .collect()
    }

    /// The one pass after state changed: start (or cancel) the full-size
    /// upgrades the wanted frames need, then release images no Ready cached
    /// frame uses. Returns true when it changed what a view would draw.
    fn after_change<V: 'static>(
        &mut self,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) -> bool {
        let changed = self.reconcile(access, cx);
        self.prune(access, cx);
        changed
    }

    /// Move every decoded image no Ready cached frame places into `retiring`
    /// and schedule the atlas drop.
    fn prune<V: 'static>(&mut self, access: fn(&mut V) -> &mut Self, cx: &mut Context<V>) {
        let referenced = self.referenced_assets();
        for map in [&mut self.full, &mut self.thumb] {
            let stale: Vec<AssetId> = map
                .keys()
                .filter(|asset| !referenced.contains(asset))
                .copied()
                .collect();
            for asset in stale {
                if let Some(image) = map.remove(&asset) {
                    self.retiring.insert(image.id, image);
                }
            }
        }
        if !self.retiring.is_empty() && self.flush.is_none() {
            self.flush = Some(cx.spawn(async move |this, cx| {
                this.update(cx, |view, cx| access(view).flush_retiring(cx))
                    .ok();
            }));
        }
    }

    /// Drop the atlas copy of every retired image nothing else holds. An image
    /// a monitor still paints stays for the next pass: dropping its atlas tile
    /// now would only make the monitor upload it again, unowned.
    fn flush_retiring<V: 'static>(&mut self, cx: &mut Context<V>) {
        // The running task ends as soon as this returns, so it is detached
        // rather than dropped from inside itself.
        if let Some(done) = self.flush.take() {
            done.detach();
        }
        // An image that came back into use is not retired any more.
        let live: HashSet<ImageId> = self
            .full
            .values()
            .chain(self.thumb.values())
            .map(|image| image.id)
            .collect();
        self.retiring.retain(|id, _| !live.contains(id));
        let idle: Vec<ImageId> = self
            .retiring
            .iter()
            .filter(|(_, image)| Arc::strong_count(image) == 1)
            .map(|(id, _)| *id)
            .collect();
        for id in idle {
            if let Some(image) = self.retiring.remove(&id) {
                #[cfg(test)]
                {
                    self.dropped += 1;
                }
                cx.drop_image(image, None);
            }
        }
    }

    /// Start a full-size decode for every wanted Ready frame whose asset has
    /// none (the promotion of a thumbnail), cancel upgrades nobody wants, and
    /// turn the frames of an asset that failed to decode into Unavailable.
    /// Returns true when a frame changed state.
    fn reconcile<V: 'static>(
        &mut self,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) -> bool {
        let mut changed = false;
        let mut needed = HashSet::new();
        for id in self.full_wanted.clone() {
            if !matches!(self.cache.peek(&id), Some(FrameEntry::Ready(_))) {
                continue;
            }
            let Some(asset) = asset_of(&self.snapshot, id) else {
                continue;
            };
            if self.full.contains_key(&asset) {
                continue;
            }
            if self.failed_assets.contains(&asset) {
                let reason = UnavailableReason::DecodeFailed;
                self.cache.insert(id, Err(reason));
                self.failures.push((id, reason));
                changed = true;
                continue;
            }
            needed.insert(asset);
            // A full compose job for a frame placing this asset decodes it
            // anyway; finishing it re-runs this pass, so no second decode
            // overlaps it.
            let decoding = self.in_flight.iter().any(|(job, (quality, _))| {
                *quality == Quality::Full && asset_of(&self.snapshot, *job) == Some(asset)
            });
            if !decoding && !self.upgrades.contains_key(&asset) {
                let task = self.start_upgrade(asset, access, cx);
                self.upgrades.insert(asset, task);
            }
        }
        self.upgrades.retain(|asset, _| needed.contains(asset));
        changed
    }

    fn start_upgrade<V: 'static>(
        &self,
        asset: AssetId,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) -> Task<()> {
        let snapshot = self.snapshot.clone();
        #[cfg(test)]
        let fault = self.fault;
        #[cfg(not(test))]
        let fault = AssetFault::None;
        #[cfg(test)]
        let hold = self.hold;
        let renderer = cx.svg_renderer();
        let job = cx.background_spawn(async move {
            let bytes = Self::asset_bytes(&snapshot, asset, fault)?;
            decode_full(&bytes, &renderer).ok()
        });
        cx.spawn(async move |this, cx| {
            let decoded = job.await;
            #[cfg(test)]
            if !hold.is_zero() {
                cx.background_executor().timer(hold).await;
            }
            this.update(cx, |view, cx| {
                access(view).finish_upgrade(asset, decoded, access, cx);
                cx.notify();
            })
            .ok();
        })
    }

    fn finish_upgrade<V: 'static>(
        &mut self,
        asset: AssetId,
        decoded: Option<Arc<RenderImage>>,
        access: fn(&mut V) -> &mut Self,
        cx: &mut Context<V>,
    ) {
        self.upgrades.remove(&asset);
        match decoded {
            // Only keep it while a cached frame still places the asset.
            Some(image) if self.referenced_assets().contains(&asset) => {
                self.store_image(asset, Quality::Full, image);
            }
            Some(_) => {}
            None => {
                self.failed_assets.insert(asset);
            }
        }
        self.after_change(access, cx);
    }

    /// What to draw for `id`, a thumbnail-sized image allowed. Marks a ready
    /// frame recently used.
    pub fn get(&mut self, id: FrameId) -> FrameView {
        let view = self.entry(id);
        self.view_of(view, false)
    }

    /// What to draw for `id` without touching its recency, for the timeline's
    /// thumbnails: painting must never change what the cache evicts. A
    /// thumbnail-sized image is allowed.
    pub fn peek(&self, id: FrameId) -> FrameView {
        let view = self.entry_peek(id);
        self.view_of(view, false)
    }

    /// Like [`PresenterFrames::get`], for the Program and Preview monitors and
    /// the output: a frame that places an image is `Pending` until its full
    /// image is decoded, so the viewer keeps its last complete frame and no
    /// low-resolution image reaches the congregation.
    pub fn get_full(&mut self, id: FrameId) -> FrameView {
        let view = self.entry(id);
        self.view_of(view, true)
    }

    /// [`PresenterFrames::get_full`] without touching recency.
    pub fn peek_full(&self, id: FrameId) -> FrameView {
        let view = self.entry_peek(id);
        self.view_of(view, true)
    }

    fn entry(&mut self, id: FrameId) -> Result<Arc<Frame>, Option<UnavailableReason>> {
        match self.cache.get(&id) {
            Some(FrameEntry::Ready(frame)) => Ok(Arc::clone(frame)),
            Some(FrameEntry::Unavailable(reason)) => Err(Some(*reason)),
            None => Err(None),
        }
    }

    fn entry_peek(&self, id: FrameId) -> Result<Arc<Frame>, Option<UnavailableReason>> {
        match self.cache.peek(&id) {
            Some(FrameEntry::Ready(frame)) => Ok(Arc::clone(frame)),
            Some(FrameEntry::Unavailable(reason)) => Err(Some(*reason)),
            None => Err(None),
        }
    }

    fn view_of(
        &self,
        entry: Result<Arc<Frame>, Option<UnavailableReason>>,
        full_only: bool,
    ) -> FrameView {
        let frame = match entry {
            Ok(frame) => frame,
            Err(Some(reason)) => return FrameView::Unavailable(reason),
            Err(None) => return FrameView::Pending,
        };
        let asset = frame
            .image()
            .and_then(|placed| AssetId::parse(&placed.id).ok());
        let image = match asset {
            None => None,
            Some(asset) => {
                let full = self.full.get(&asset);
                let found = if full_only {
                    full
                } else {
                    full.or_else(|| self.thumb.get(&asset))
                };
                match found {
                    Some(image) => Some(Arc::clone(image)),
                    // The frame is composed but its full image is not there
                    // yet: not ready for the monitors.
                    None if full_only => return FrameView::Pending,
                    None => None,
                }
            }
        };
        FrameView::Ready { frame, image }
    }

    /// Frames whose composition failed since the last call, for the view to
    /// report to the Engine. Each failure is handed out once.
    pub fn take_failures(&mut self) -> Vec<(FrameId, UnavailableReason)> {
        std::mem::take(&mut self.failures)
    }

    /// Jobs in which `compose` actually ran (a job that failed before
    /// composing, on a missing or undecodable asset, is not counted).
    /// Resizing never changes it.
    pub fn composed_count(&self) -> usize {
        self.composed_count
    }

    /// Entries (frames and Unavailable results) held by the cache.
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Decoded images held, as `(full, thumbnail)`. Released images waiting
    /// for a monitor to let go are not counted.
    pub fn image_counts(&self) -> (usize, usize) {
        (self.full.len(), self.thumb.len())
    }

    /// Full-size upgrade decodes running now (one per asset at most).
    pub fn upgrading(&self) -> usize {
        self.upgrades.len()
    }

    /// Distinct assets placed by Ready cached frames: the bound on both
    /// [`PresenterFrames::image_counts`] entries.
    pub fn referenced_image_count(&self) -> usize {
        self.referenced_assets().len()
    }

    /// Drop every decoded image and release its GPU atlas tile. The owning
    /// `PresenterView` calls it when it is released, which is also the end of
    /// this snapshot's store (a new snapshot gets a new view and store). Frames
    /// stay cached, but a ready image frame drawn afterwards has no image, so
    /// only call it when idle.
    pub fn release(&mut self, window: Option<&mut Window>, cx: &mut App) {
        let mut window = window;
        let images = self
            .full
            .drain()
            .chain(self.thumb.drain())
            .map(|(_, image)| image)
            .chain(self.retiring.drain().map(|(_, image)| image));
        for image in images {
            #[cfg(test)]
            {
                self.dropped += 1;
            }
            cx.drop_image(image, window.as_deref_mut());
        }
        self.upgrades.clear();
    }

    #[cfg(test)]
    fn scheduler_in_flight(&self) -> usize {
        self.scheduler.in_flight()
    }

    /// Hold every job started from now on for `hold` of executor time before
    /// its result lands. Test seam for the slow-frame indicator.
    #[cfg(test)]
    pub(super) fn set_hold(&mut self, hold: std::time::Duration) {
        self.hold = hold;
    }

    /// Jobs started and not yet finished or dropped.
    pub fn jobs_in_flight(&self) -> usize {
        self.in_flight.len()
    }
}

/// The background job: resolve and decode the foreground asset at `quality`,
/// then compose. Runs off the GPUI thread. The asset is decoded eagerly into
/// the `RenderImage` GPUI paints, so a bad PNG fails here as `DecodeFailed`.
fn prepare(
    snapshot: &LoadedSnapshot,
    key: FrameId,
    quality: Quality,
    known: Option<(Quality, Arc<RenderImage>)>,
    renderer: &SvgRenderer,
    fault: AssetFault,
) -> Prepared {
    // An id from another snapshot is a programming error in the caller (the
    // Engine only names this snapshot's frames); there is no fitting render
    // error, so it is reported as the same structured failure.
    let Some(content) = snapshot.frame(key) else {
        return failed(UnavailableReason::DecodeFailed);
    };
    let mut image = None;
    if let Some(foreground) = &content.image {
        let Ok(asset) = AssetId::parse(&foreground.id) else {
            return failed(UnavailableReason::DecodeFailed);
        };
        if let Some((held, decoded)) = known {
            image = Some((asset, held, decoded));
        } else {
            let Some(bytes) = PresenterFrames::asset_bytes(snapshot, asset, fault) else {
                return failed(UnavailableReason::DecodeFailed);
            };
            let decoded = match quality {
                Quality::Full => decode_full(&bytes, renderer),
                Quality::Thumb => decode_thumbnail(&bytes),
            };
            let Ok(decoded) = decoded else {
                return failed(UnavailableReason::DecodeFailed);
            };
            image = Some((asset, quality, decoded));
        }
    }
    Prepared {
        result: compose(content),
        composed: true,
        image,
    }
}

fn failed(reason: UnavailableReason) -> Prepared {
    Prepared {
        result: Err(reason),
        composed: false,
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
    use std::time::Duration;
    use ui::environment::{ResolvedAppearance, UiEnvironment, UiPreferences};

    struct Host {
        frames: PresenterFrames,
        shown: Option<FrameId>,
        paints: usize,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.paints += 1;
            let (frame, image) = match self.shown.map(|id| self.frames.get(id)) {
                Some(FrameView::Ready { frame, image }) => (Some(frame), image),
                _ => (None, None),
            };
            let environment =
                UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);
            div()
                .size_full()
                .child(crate::canonical_views::live_viewport(
                    frame,
                    image,
                    320.0,
                    None,
                    environment,
                ))
        }
    }

    fn access(host: &mut Host) -> &mut PresenterFrames {
        &mut host.frames
    }

    fn open(
        cx: &mut TestAppContext,
        snapshot: Arc<LoadedSnapshot>,
        fault: AssetFault,
    ) -> (gpui::Entity<Host>, &mut gpui::VisualTestContext) {
        open_sized(cx, snapshot, fault, FRAME_CAPACITY)
    }

    fn open_sized(
        cx: &mut TestAppContext,
        snapshot: Arc<LoadedSnapshot>,
        fault: AssetFault,
        capacity: usize,
    ) -> (gpui::Entity<Host>, &mut gpui::VisualTestContext) {
        cx.update(ui::init).unwrap();
        let (host, cx) = cx.add_window_view(|_, _| {
            let mut frames = PresenterFrames::with_capacity(snapshot, capacity);
            frames.fault = fault;
            Host {
                frames,
                shown: None,
                paints: 0,
            }
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
        let (host, cx) = open(cx, snapshot, AssetFault::None);

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
        let (host, cx) = open(cx, snapshot, AssetFault::None);

        sync(&host, cx, Some(a), Some(b));
        sync(&host, cx, Some(a), Some(c));
        // B was still pending and nothing wants it: its task is gone.
        assert_eq!(host.read_with(cx, |h, _| h.frames.jobs_in_flight()), 2);
        assert_eq!(host.read_with(cx, |h, _| h.frames.scheduler_in_flight()), 2);

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
        let (host, cx) = open(cx, snapshot, AssetFault::None);
        sync(&host, cx, Some(a), Some(b));
        cx.run_until_parked();
        let before = counts(&host, cx);

        sync(&host, cx, Some(a), Some(b));
        cx.run_until_parked();
        assert_eq!(counts(&host, cx), before);

        host.update(cx, |host, cx| {
            host.shown = Some(a);
            cx.notify();
        });
        cx.run_until_parked();
        let paints = host.read_with(cx, |h, _| h.paints);
        cx.simulate_resize(size(px(1280.0), px(720.0)));
        cx.run_until_parked();
        assert!(
            host.read_with(cx, |h, _| h.paints) > paints,
            "the resize must repaint the viewport"
        );
        assert_eq!(counts(&host, cx), before);
    }

    #[gpui::test]
    fn an_image_frame_is_ready_with_its_decoded_image(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::None);

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
    fn an_image_frame_with_corrupt_asset_bytes_is_unavailable_and_reported(
        cx: &mut TestAppContext,
    ) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::Corrupt);

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
    }

    #[gpui::test]
    fn an_image_frame_without_its_asset_is_unavailable_and_reported(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::Withhold);

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

    fn visible(host: &gpui::Entity<Host>, cx: &mut gpui::VisualTestContext, ids: Vec<FrameId>) {
        host.update(cx, |host, cx| host.frames.request_visible(ids, access, cx));
    }

    fn image_counts(host: &gpui::Entity<Host>, cx: &mut gpui::VisualTestContext) -> (usize, usize) {
        host.read_with(cx, |host, _| host.frames.image_counts())
    }

    fn ready_image(view: FrameView) -> Option<Arc<RenderImage>> {
        match view {
            FrameView::Ready { image, .. } => image,
            other => panic!("expected a ready frame, found {other:?}"),
        }
    }

    #[gpui::test]
    fn a_visible_only_image_frame_holds_a_thumbnail_and_no_full_image(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::None);

        visible(&host, cx, vec![image_frame]);
        cx.run_until_parked();
        assert_eq!(image_counts(&host, cx), (0, 1));
        // The timeline draws the thumbnail; the monitors would not.
        let thumb = host.read_with(cx, |h, _| h.frames.peek(image_frame));
        assert!(ready_image(thumb).is_some());
        let for_monitor = host.read_with(cx, |h, _| h.frames.peek_full(image_frame));
        assert!(matches!(for_monitor, FrameView::Pending));
    }

    #[gpui::test]
    fn promoting_a_thumbnail_decodes_the_full_image_and_program_waits_for_it(
        cx: &mut TestAppContext,
    ) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::None);
        visible(&host, cx, vec![image_frame]);
        cx.run_until_parked();
        let thumb = ready_image(host.read_with(cx, |h, _| h.frames.peek(image_frame))).unwrap();

        // Hold the upgrade so the thumbnail-only state can be observed.
        host.update(cx, |h, _| h.frames.set_hold(Duration::from_millis(50)));
        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        assert_eq!(image_counts(&host, cx), (0, 1), "still only the thumbnail");
        assert_eq!(host.read_with(cx, |h, _| h.frames.upgrading()), 1);
        assert!(
            matches!(view(&host, cx, image_frame), FrameView::Ready { .. }),
            "the timeline keeps its thumbnail meanwhile"
        );
        assert!(
            matches!(
                host.update(cx, |h, _| h.frames.get_full(image_frame)),
                FrameView::Pending
            ),
            "Program must not show a thumbnail-only frame"
        );

        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        assert_eq!(image_counts(&host, cx), (1, 0), "the thumbnail is retired");
        let full = ready_image(host.update(cx, |h, _| h.frames.get_full(image_frame))).unwrap();
        assert_ne!(full.id, thumb.id, "the full image is a separate decode");
        assert!(host.read_with(cx, |h, _| h.frames.upgrading() == 0));
    }

    #[gpui::test]
    fn a_failed_upgrade_makes_the_frame_unavailable_and_is_reported_once(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::None);
        visible(&host, cx, vec![image_frame]);
        cx.run_until_parked();

        host.update(cx, |h, _| h.frames.fault = AssetFault::Corrupt);
        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        assert!(matches!(
            view(&host, cx, image_frame),
            FrameView::Unavailable(UnavailableReason::DecodeFailed)
        ));
        assert_eq!(
            host.update(cx, |h, _| h.frames.take_failures()),
            vec![(image_frame, UnavailableReason::DecodeFailed)]
        );
        // No retry loop: nothing is decoding any more.
        assert!(host.read_with(cx, |h, _| h.frames.upgrading() == 0));
    }

    #[gpui::test]
    fn a_released_image_is_gone_from_both_maps_after_its_frame_is_evicted(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let plain = frame_ids(&snapshot, false);
        let (host, cx) = open_sized(cx, snapshot, AssetFault::None, 0);

        // A thumbnail first, then the full image, then enough other frames to
        // push the image frame out of the small cache.
        visible(&host, cx, vec![image_frame]);
        cx.run_until_parked();
        assert_eq!(image_counts(&host, cx), (0, 1));
        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        assert_eq!(image_counts(&host, cx), (1, 0));
        let held = ready_image(host.update(cx, |h, _| h.frames.get_full(image_frame))).unwrap();
        let weak = Arc::downgrade(&held);
        drop(held);

        for id in &plain {
            sync(&host, cx, Some(*id), None);
            cx.run_until_parked();
        }
        let evicted = host.read_with(cx, |h, _| h.frames.cache.peek(&image_frame).is_none());
        assert!(evicted, "the fixture must overflow the small cache");
        assert_eq!(image_counts(&host, cx), (0, 0));
        assert!(
            weak.upgrade().is_none(),
            "nothing keeps the evicted image alive once the atlas copy is dropped"
        );
    }

    #[gpui::test]
    fn an_image_a_monitor_still_holds_is_not_dropped_from_the_atlas_early(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let plain = frame_ids(&snapshot, false);
        let (host, cx) = open_sized(cx, snapshot, AssetFault::None, 0);
        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        let held = ready_image(host.update(cx, |h, _| h.frames.get_full(image_frame))).unwrap();

        for id in &plain {
            sync(&host, cx, Some(*id), None);
            cx.run_until_parked();
        }
        assert_eq!(image_counts(&host, cx), (0, 0));
        assert_eq!(
            host.read_with(cx, |h, _| h.frames.retiring.len()),
            1,
            "a held image waits instead of losing its atlas tile"
        );

        drop(held);
        sync(&host, cx, Some(plain[0]), None);
        cx.run_until_parked();
        assert!(host.read_with(cx, |h, _| h.frames.retiring.is_empty()));
    }

    #[gpui::test]
    fn an_image_retired_twice_is_one_entry_and_dropped_once(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let asset = asset_of(&snapshot, image_frame).unwrap();
        let plain = frame_ids(&snapshot, false);
        let (host, cx) = open_sized(cx, snapshot, AssetFault::None, 0);
        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        let held = ready_image(host.update(cx, |h, _| h.frames.get_full(image_frame))).unwrap();

        // Evict while the image is held: it waits in `retiring`.
        for id in &plain {
            sync(&host, cx, Some(*id), None);
            cx.run_until_parked();
        }
        assert_eq!(host.read_with(cx, |h, _| h.frames.retiring.len()), 1);

        // A job that carried the image finishes and stores the same `Arc`:
        // it leaves `retiring`, and evicting again lists it once.
        host.update(cx, |h, cx| {
            h.frames.store_image(asset, Quality::Full, held.clone());
            assert!(h.frames.retiring.is_empty(), "a live image is not retired");
            h.frames.after_change(access, cx);
            assert_eq!(h.frames.retiring.len(), 1, "one entry, not two");
        });
        cx.run_until_parked();
        assert_eq!(host.read_with(cx, |h, _| h.frames.dropped), 0, "still held");

        drop(held);
        sync(&host, cx, Some(plain[0]), None);
        cx.run_until_parked();
        assert!(host.read_with(cx, |h, _| h.frames.retiring.is_empty()));
        assert_eq!(host.read_with(cx, |h, _| h.frames.dropped), 1);
    }

    #[gpui::test]
    fn release_drops_every_image_once(cx: &mut TestAppContext) {
        let snapshot = fixture_snapshot();
        let image_frame = frame_ids(&snapshot, true)[0];
        let (host, cx) = open(cx, snapshot, AssetFault::None);
        sync(&host, cx, Some(image_frame), None);
        cx.run_until_parked();
        assert_eq!(image_counts(&host, cx), (1, 0));
        host.update(cx, |h, cx| h.frames.release(None, cx));
        assert_eq!(image_counts(&host, cx), (0, 0));
        assert_eq!(host.read_with(cx, |h, _| h.frames.dropped), 1);
        assert!(host.read_with(cx, |h, _| h.frames.retiring.is_empty()));
    }
}
