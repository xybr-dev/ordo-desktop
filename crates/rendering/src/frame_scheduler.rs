//! The latest-wins frame scheduler (T1.3).
//!
//! A pure state machine: it decides *what* the executor must compose and
//! which finished jobs still matter. It owns no thread and reads no clock, so
//! tests drive slow, failed and superseded jobs deterministically and the
//! platform runs the work on its own task worker.
//!
//! Each of the Program and Preview slots wants at most one key at a time. A
//! new request supersedes the slot's previous one, and a job whose request was
//! superseded completes as [`Completion::Ignored`]: its result is dropped and
//! can never replace what a newer request produced. The preload set is
//! replaced wholesale. A slot only ever exposes a complete frame or a
//! structured [`UnavailableReason`] (no partial frame). Pinning the Program
//! and Preview keys is the cache owner's job ([`FrameCache::pin`]); the
//! scheduler only reads the cache.

use std::collections::BTreeSet;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use crate::frame_cache::FrameCache;
use crate::{Frame, RenderError, TargetDensity, TemplateContent, Theme, render_template};

/// Cues preloaded on each side of Preview.
pub const PRELOAD_RADIUS: usize = 2;

/// Which consumer a request serves.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FrameSlot {
    /// The congregation output.
    Program,
    /// The operator's Preview monitor.
    Preview,
    /// Frames composed ahead of navigation.
    Preload,
}

/// Why a frame could not be produced. One type for every producer, so the
/// Engine and Preflight read the same structured result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailableReason {
    /// Composition failed.
    Render(RenderError),
    /// A required asset (for example the foreground image) is missing.
    MissingAsset,
    /// An asset exists but could not be decoded. Reserved for image decode.
    DecodeFailed,
}

impl From<RenderError> for UnavailableReason {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}

impl fmt::Display for UnavailableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(error) => write!(f, "frame could not be composed: {error}"),
            Self::MissingAsset => f.write_str("a required asset is missing"),
            Self::DecodeFailed => f.write_str("an asset could not be decoded"),
        }
    }
}

impl std::error::Error for UnavailableReason {}

/// Composes `content` at the default [`Theme`] and 100% density, dropping the
/// diagnostics. A missing foreground image is reported as the render error
/// [`RenderError::MissingForegroundImage`].
pub fn compose(content: &TemplateContent) -> Result<Frame, UnavailableReason> {
    render_template(content, &Theme::default(), TargetDensity::SCALE_100)
        .map(|(frame, _diagnostics)| frame)
        .map_err(UnavailableReason::from)
}

/// Proof that the executor was asked to compose `key` for `slot`. Hand it
/// back to [`FrameScheduler::complete`] or [`FrameScheduler::abandon`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ticket<K> {
    slot: FrameSlot,
    generation: u64,
    key: K,
}

impl<K: Copy> Ticket<K> {
    /// The slot the job serves.
    pub fn slot(&self) -> FrameSlot {
        self.slot
    }

    /// The slot's request generation when the ticket was issued.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The key to compose.
    pub fn key(&self) -> K {
        self.key
    }
}

/// What [`FrameScheduler::complete`] did with a finished job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    /// The request was still wanted; the result is in the cache.
    Accepted,
    /// The request was superseded (or unknown); the result was dropped.
    Ignored,
}

#[derive(Debug)]
struct SlotWant<K> {
    generation: u64,
    key: Option<K>,
}

/// Latest-wins request bookkeeping for the Program, Preview and Preload slots.
#[derive(Debug)]
pub struct FrameScheduler<K> {
    program: SlotWant<K>,
    preview: SlotWant<K>,
    preload_generation: u64,
    preload: Vec<K>,
    in_flight: BTreeSet<Ticket<K>>,
}

impl<K: Copy + Eq + Hash + Ord> Default for FrameScheduler<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Copy + Eq + Hash + Ord> FrameScheduler<K> {
    /// A scheduler with nothing wanted and nothing in flight.
    pub fn new() -> Self {
        Self {
            program: SlotWant {
                generation: 0,
                key: None,
            },
            preview: SlotWant {
                generation: 0,
                key: None,
            },
            preload_generation: 0,
            preload: Vec::new(),
            in_flight: BTreeSet::new(),
        }
    }

    /// Ask for `key` in `slot` (Program or Preview; a Preload slot is ignored
    /// and returns `None`; use [`FrameScheduler::request_preload`]).
    ///
    /// Returns a ticket when the executor must compose it, and `None` when the
    /// frame is already in the cache (ready or Unavailable) or the slot's
    /// current request for the same key is still in flight. A request for a
    /// different key, or one answered by the cache, supersedes the slot's
    /// previous request (generation += 1), which makes that request's job
    /// obsolete. Repeating the identical in-flight request does not bump the
    /// generation: that would orphan the only job that can answer it.
    pub fn request(&mut self, slot: FrameSlot, key: K, cache: &FrameCache<K>) -> Option<Ticket<K>> {
        let want = match slot {
            FrameSlot::Program => &mut self.program,
            FrameSlot::Preview => &mut self.preview,
            FrameSlot::Preload => return None,
        };
        if want.key == Some(key)
            && self.in_flight.contains(&Ticket {
                slot,
                generation: want.generation,
                key,
            })
        {
            return None;
        }
        want.generation += 1;
        want.key = Some(key);
        if cache.peek(&key).is_some() {
            return None;
        }
        let ticket = Ticket {
            slot,
            generation: want.generation,
            key,
        };
        self.in_flight.insert(ticket);
        Some(ticket)
    }

    /// Replaces the whole preload set and returns tickets for the keys not yet
    /// composed or already in flight as preload. At most
    /// `2 * PRELOAD_RADIUS` distinct keys are taken, in order; the rest are
    /// ignored. Jobs for keys that left the set become obsolete.
    pub fn request_preload(
        &mut self,
        keys: impl IntoIterator<Item = K>,
        cache: &FrameCache<K>,
    ) -> Vec<Ticket<K>> {
        let mut set: Vec<K> = Vec::new();
        for key in keys {
            if set.len() == 2 * PRELOAD_RADIUS {
                break;
            }
            if !set.contains(&key) {
                set.push(key);
            }
        }
        self.preload_generation += 1;
        self.preload = set;
        let mut tickets = Vec::new();
        for &key in &self.preload {
            if cache.peek(&key).is_some() {
                continue;
            }
            let pending = self
                .in_flight
                .iter()
                .any(|ticket| ticket.slot == FrameSlot::Preload && ticket.key == key);
            if pending {
                continue;
            }
            tickets.push(Ticket {
                slot: FrameSlot::Preload,
                generation: self.preload_generation,
                key,
            });
        }
        self.in_flight.extend(tickets.iter().copied());
        tickets
    }

    fn wanted(&self, ticket: &Ticket<K>) -> bool {
        match ticket.slot {
            FrameSlot::Program => {
                self.program.key == Some(ticket.key) && self.program.generation == ticket.generation
            }
            FrameSlot::Preview => {
                self.preview.key == Some(ticket.key) && self.preview.generation == ticket.generation
            }
            // The preload set is replaced wholesale; a job survives while its
            // key is still in the set, whatever generation issued it.
            FrameSlot::Preload => self.preload.contains(&ticket.key),
        }
    }

    /// Tickets the executor may abandon: in flight but no longer wanted by any
    /// slot.
    pub fn obsolete(&self) -> Vec<Ticket<K>> {
        self.in_flight
            .iter()
            .filter(|ticket| !self.wanted(ticket))
            .copied()
            .collect()
    }

    /// Forgets `ticket` because the executor dropped the job. A later
    /// completion of it is [`Completion::Ignored`].
    pub fn abandon(&mut self, ticket: &Ticket<K>) {
        self.in_flight.remove(ticket);
    }

    /// Reports a finished job.
    ///
    /// Accepted when the ticket is still wanted: a frame is stored as ready, a
    /// failure as Unavailable (so the failing Cue is not recomposed in a
    /// loop). Ignored when the ticket is obsolete, was abandoned, or is
    /// unknown: the result is dropped and never replaces a newer Preview.
    pub fn complete(
        &mut self,
        ticket: Ticket<K>,
        result: Result<Frame, UnavailableReason>,
        cache: &mut FrameCache<K>,
    ) -> Completion {
        if !self.in_flight.remove(&ticket) || !self.wanted(&ticket) {
            return Completion::Ignored;
        }
        cache.insert(ticket.key, result.map(Arc::new));
        Completion::Accepted
    }

    /// Jobs issued and not yet completed or abandoned.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TemplateKind;
    use crate::frame_cache::{FRAME_CAPACITY, FrameEntry};

    fn content(body: &str) -> TemplateContent {
        TemplateContent {
            kind: TemplateKind::GeneralText,
            header: None,
            body: body.to_owned(),
            footer: None,
            image: None,
        }
    }

    fn frame(body: &str) -> Frame {
        compose(&content(body)).expect("plain content composes")
    }

    #[test]
    fn latest_wins_in_preview() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let a = scheduler.request(FrameSlot::Preview, 1u32, &cache).unwrap();
        let b = scheduler.request(FrameSlot::Preview, 2u32, &cache).unwrap();
        assert_eq!(scheduler.obsolete(), vec![a]);
        assert_eq!(
            scheduler.complete(a, Ok(frame("alpha")), &mut cache),
            Completion::Ignored
        );
        assert!(cache.peek(&1).is_none());
        assert_eq!(
            scheduler.complete(b, Ok(frame("beta")), &mut cache),
            Completion::Accepted
        );
        assert!(matches!(cache.peek(&2), Some(FrameEntry::Ready(_))));
        assert_eq!(scheduler.in_flight(), 0);
    }

    #[test]
    fn abandon_removes_an_obsolete_ticket() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let a = scheduler.request(FrameSlot::Preview, 1u32, &cache).unwrap();
        scheduler.request(FrameSlot::Preview, 2u32, &cache).unwrap();
        assert_eq!(scheduler.in_flight(), 2);
        scheduler.abandon(&a);
        assert!(scheduler.obsolete().is_empty());
        assert_eq!(scheduler.in_flight(), 1);
        assert_eq!(
            scheduler.complete(a, Ok(frame("alpha")), &mut cache),
            Completion::Ignored
        );
    }

    #[test]
    fn cache_hit_and_identical_in_flight_request_return_none() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let a = scheduler.request(FrameSlot::Program, 7u32, &cache).unwrap();
        assert!(scheduler.request(FrameSlot::Program, 7, &cache).is_none());
        assert!(scheduler.obsolete().is_empty());
        assert_eq!(
            scheduler.complete(a, Ok(frame("alpha")), &mut cache),
            Completion::Accepted
        );
        assert!(scheduler.request(FrameSlot::Program, 7, &cache).is_none());
        assert_eq!(scheduler.in_flight(), 0);
    }

    #[test]
    fn a_cache_hit_still_supersedes_the_slots_in_flight_request() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        cache.insert(9u32, Ok(Arc::new(frame("cached"))));
        let mut scheduler = FrameScheduler::new();
        let slow = scheduler.request(FrameSlot::Preview, 1u32, &cache).unwrap();
        assert!(scheduler.request(FrameSlot::Preview, 9, &cache).is_none());
        assert_eq!(scheduler.obsolete(), vec![slow]);
        assert_eq!(
            scheduler.complete(slow, Ok(frame("late")), &mut cache),
            Completion::Ignored
        );
        assert!(cache.peek(&1).is_none());
    }

    #[test]
    fn a_failed_job_is_cached_as_unavailable_and_accepted() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let ticket = scheduler.request(FrameSlot::Program, 3u32, &cache).unwrap();
        assert_eq!(
            scheduler.complete(ticket, Err(UnavailableReason::MissingAsset), &mut cache),
            Completion::Accepted
        );
        assert_eq!(
            cache.peek(&3),
            Some(FrameEntry::Unavailable(&UnavailableReason::MissingAsset))
        );
        // The failure is remembered: no job is issued again.
        assert!(scheduler.request(FrameSlot::Program, 3, &cache).is_none());
    }

    #[test]
    fn preload_is_bounded_and_replaced() {
        let cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let first = scheduler.request_preload(0u32..10, &cache);
        assert_eq!(first.len(), 2 * PRELOAD_RADIUS);
        assert_eq!(scheduler.in_flight(), 2 * PRELOAD_RADIUS);
        // Same set again: nothing new, nothing obsolete.
        assert!(scheduler.request_preload(0u32..10, &cache).is_empty());
        assert!(scheduler.obsolete().is_empty());
        // Replace with an overlapping set: 0 and 1 leave, 4 and 5 join.
        let second = scheduler.request_preload([2u32, 3, 4, 5, 5], &cache);
        let keys: Vec<u32> = second.iter().map(Ticket::key).collect();
        assert_eq!(keys, vec![4, 5]);
        let obsolete: Vec<u32> = scheduler.obsolete().iter().map(Ticket::key).collect();
        assert_eq!(obsolete, vec![0, 1]);
    }

    #[test]
    fn preload_skips_cached_keys() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        cache.insert(1u32, Ok(Arc::new(frame("cached"))));
        let mut scheduler = FrameScheduler::new();
        let tickets = scheduler.request_preload([1u32, 2], &cache);
        assert_eq!(tickets.len(), 1);
        assert_eq!(tickets[0].key(), 2);
        assert_eq!(tickets[0].slot(), FrameSlot::Preload);
        assert!(tickets[0].generation() > 0);
    }

    #[test]
    fn program_and_preview_are_independent() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let program = scheduler.request(FrameSlot::Program, 1u32, &cache).unwrap();
        let preview = scheduler.request(FrameSlot::Preview, 2u32, &cache).unwrap();
        assert!(scheduler.obsolete().is_empty());
        // A new Preview request leaves Program's job alone.
        let preview2 = scheduler.request(FrameSlot::Preview, 3u32, &cache).unwrap();
        assert_eq!(scheduler.obsolete(), vec![preview]);
        assert_eq!(
            scheduler.complete(program, Ok(frame("p")), &mut cache),
            Completion::Accepted
        );
        assert_eq!(
            scheduler.complete(preview2, Ok(frame("q")), &mut cache),
            Completion::Accepted
        );
    }

    #[test]
    fn a_ticket_completes_only_once() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let ticket = scheduler.request(FrameSlot::Program, 1u32, &cache).unwrap();
        assert_eq!(
            scheduler.complete(ticket, Ok(frame("a")), &mut cache),
            Completion::Accepted
        );
        assert_eq!(
            scheduler.complete(ticket, Ok(frame("a")), &mut cache),
            Completion::Ignored
        );
    }

    #[test]
    fn compose_reports_a_missing_image_as_unavailable() {
        let content = TemplateContent {
            kind: TemplateKind::GeneralImage,
            header: None,
            body: String::new(),
            footer: None,
            image: None,
        };
        assert_eq!(
            compose(&content).unwrap_err(),
            UnavailableReason::Render(RenderError::MissingForegroundImage)
        );
    }

    #[test]
    fn reasons_have_readable_messages() {
        assert!(
            UnavailableReason::MissingAsset
                .to_string()
                .contains("asset")
        );
        assert!(
            UnavailableReason::DecodeFailed
                .to_string()
                .contains("decoded")
        );
        assert!(
            UnavailableReason::Render(RenderError::FontUnavailable)
                .to_string()
                .contains("font")
        );
    }
}
