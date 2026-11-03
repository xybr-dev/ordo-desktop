//! The latest-wins frame scheduler (T1.3).
//!
//! A pure state machine: it decides *what* the executor must compose and
//! which finished jobs still matter. It owns no thread and reads no clock, so
//! tests drive slow, failed and superseded jobs deterministically and the
//! platform runs the work on its own task worker.
//!
//! Each of the Program and Preview slots wants at most one key at a time. A
//! new request supersedes the slot's previous one. Wanted-ness is a property of
//! the *key*: a key is wanted while Program, Preview or the preload set names
//! it, and there is at most one job in flight per key (a second slot asking for
//! a key already being composed shares that job). A finished job whose key is
//! no longer wanted completes as [`Completion::Ignored`]: its frame is dropped
//! and can never replace what a newer request produced (Preview always reads
//! the cache entry of its current key). A failure is a fact about the frame
//! identity, so it is stored even when nobody wants the key any more. The
//! preload set is replaced wholesale. Pinning the Program and Preview keys is
//! the cache owner's job ([`FrameCache::pin`]); the scheduler only reads the
//! cache.

use std::collections::BTreeSet;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use crate::frame_cache::FrameCache;
use crate::{Frame, RenderError, TargetDensity, TemplateContent, Theme, render_template};

/// Cues preloaded on each side of Preview. The caller chooses which keys those
/// are; the scheduler only bounds the set to `2 * PRELOAD_RADIUS` keys.
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
    /// A frame's foreground asset could not be used: its bytes are not a
    /// decodable image, the asset is missing from the snapshot, or the
    /// frame's image id does not parse. Produced by the Presenter's frame
    /// store (T1.4). A General Image frame with no image at all is reported
    /// by [`compose`] as `Render(RenderError::MissingForegroundImage)`.
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
    /// The result is in the cache: the key was still wanted, or the job failed
    /// (a failure is stored whether or not the key is still wanted).
    Accepted,
    /// The key is no longer wanted (or the ticket is unknown or was
    /// abandoned); the frame was dropped.
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
    /// The request always supersedes the slot's previous one. Returns a ticket
    /// when the executor must compose the key, and `None` when the frame is
    /// already in the cache (ready or Unavailable) or a job for the same key
    /// is already in flight, whichever slot issued it: there is one job per
    /// key, and that job answers this request too.
    pub fn request(&mut self, slot: FrameSlot, key: K, cache: &FrameCache<K>) -> Option<Ticket<K>> {
        let want = match slot {
            FrameSlot::Program => &mut self.program,
            FrameSlot::Preview => &mut self.preview,
            FrameSlot::Preload => return None,
        };
        want.generation += 1;
        want.key = Some(key);
        let generation = want.generation;
        if cache.peek(&key).is_some() || self.has_job(&key) {
            return None;
        }
        let ticket = Ticket {
            slot,
            generation,
            key,
        };
        self.in_flight.insert(ticket);
        Some(ticket)
    }

    /// Replaces the whole preload set and returns tickets for the keys not yet
    /// composed and without a job in flight (from any slot). At most
    /// `2 * PRELOAD_RADIUS` distinct keys are taken, in order; the rest are
    /// ignored. Jobs for keys that left every slot become obsolete.
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
        for index in 0..self.preload.len() {
            let key = self.preload[index];
            if cache.peek(&key).is_some() || self.has_job(&key) {
                continue;
            }
            let ticket = Ticket {
                slot: FrameSlot::Preload,
                generation: self.preload_generation,
                key,
            };
            self.in_flight.insert(ticket);
            tickets.push(ticket);
        }
        tickets
    }

    fn has_job(&self, key: &K) -> bool {
        self.in_flight.iter().any(|ticket| ticket.key == *key)
    }

    /// A key is wanted while Program, Preview or the preload set names it. A
    /// ticket's slot and generation are only its identity.
    fn wanted(&self, key: &K) -> bool {
        self.program.key.as_ref() == Some(key)
            || self.preview.key.as_ref() == Some(key)
            || self.preload.contains(key)
    }

    /// Tickets the executor may abandon: in flight but whose key no slot wants
    /// any more.
    ///
    /// Executor contract: run `obsolete()` and [`FrameScheduler::abandon`]
    /// after every request pass, which keeps [`FrameScheduler::in_flight`]
    /// within `2 + 2 * PRELOAD_RADIUS` jobs.
    pub fn obsolete(&self) -> Vec<Ticket<K>> {
        self.in_flight
            .iter()
            .filter(|ticket| !self.wanted(&ticket.key))
            .copied()
            .collect()
    }

    /// Forgets `ticket` because the executor dropped the job. Returns `false`
    /// and keeps the ticket when its key is wanted again (the job still
    /// answers that request) or the ticket is not in flight. Abandon only
    /// tickets listed by [`FrameScheduler::obsolete`] that are still obsolete.
    /// A later completion of an abandoned ticket is [`Completion::Ignored`].
    pub fn abandon(&mut self, ticket: &Ticket<K>) -> bool {
        if self.wanted(&ticket.key) {
            return false;
        }
        self.in_flight.remove(ticket)
    }

    /// Reports a finished job.
    ///
    /// A frame is stored as ready when its key is still wanted
    /// ([`Completion::Accepted`]) and dropped otherwise. A failure is stored as
    /// Unavailable whether or not the key is still wanted, so the failing
    /// frame is not recomposed in a loop. An abandoned or unknown ticket is
    /// [`Completion::Ignored`].
    pub fn complete(
        &mut self,
        ticket: Ticket<K>,
        result: Result<Frame, UnavailableReason>,
        cache: &mut FrameCache<K>,
    ) -> Completion {
        if !self.in_flight.remove(&ticket) {
            return Completion::Ignored;
        }
        if result.is_ok() && !self.wanted(&ticket.key) {
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
            scheduler.complete(ticket, Err(UnavailableReason::DecodeFailed), &mut cache),
            Completion::Accepted
        );
        assert_eq!(
            cache.peek(&3),
            Some(FrameEntry::Unavailable(&UnavailableReason::DecodeFailed))
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
            UnavailableReason::DecodeFailed
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

    #[test]
    fn linked_program_and_preview_on_one_key_share_one_job() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let program = scheduler.request(FrameSlot::Program, 5u32, &cache).unwrap();
        assert!(scheduler.request(FrameSlot::Preview, 5, &cache).is_none());
        assert_eq!(scheduler.in_flight(), 1);
        assert_eq!(
            scheduler.complete(program, Ok(frame("shared")), &mut cache),
            Completion::Accepted
        );
        assert!(matches!(cache.peek(&5), Some(FrameEntry::Ready(_))));
    }

    #[test]
    fn navigating_onto_a_preloading_key_keeps_that_job() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let tickets = scheduler.request_preload([1u32, 2], &cache);
        let preloading = tickets[0];
        // Preview lands on the key being preloaded: no duplicate job.
        assert!(scheduler.request(FrameSlot::Preview, 1, &cache).is_none());
        assert_eq!(scheduler.in_flight(), 2);
        // The preload set moves on, but Preview still wants key 1.
        scheduler.request_preload([3u32], &cache);
        assert!(!scheduler.obsolete().iter().any(|t| t.key() == 1));
        assert_eq!(
            scheduler.complete(preloading, Ok(frame("one")), &mut cache),
            Completion::Accepted
        );
        assert!(matches!(cache.peek(&1), Some(FrameEntry::Ready(_))));
    }

    #[test]
    fn returning_to_a_key_keeps_its_first_job_wanted() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let a = scheduler.request(FrameSlot::Preview, 1u32, &cache).unwrap();
        let b = scheduler.request(FrameSlot::Preview, 2u32, &cache).unwrap();
        assert!(scheduler.request(FrameSlot::Preview, 1, &cache).is_none());
        assert_eq!(scheduler.obsolete(), vec![b]);
        assert_eq!(
            scheduler.complete(a, Ok(frame("alpha")), &mut cache),
            Completion::Accepted
        );
    }

    #[test]
    fn abandon_refuses_a_ticket_that_is_wanted_again() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let a = scheduler.request(FrameSlot::Preview, 1u32, &cache).unwrap();
        scheduler.request(FrameSlot::Preview, 2u32, &cache).unwrap();
        assert_eq!(scheduler.obsolete(), vec![a]);
        // Preview comes back before the executor drops the job.
        assert!(scheduler.request(FrameSlot::Preview, 1, &cache).is_none());
        assert!(!scheduler.abandon(&a));
        assert_eq!(scheduler.in_flight(), 2);
        assert_eq!(
            scheduler.complete(a, Ok(frame("alpha")), &mut cache),
            Completion::Accepted
        );
    }

    #[test]
    fn a_failure_is_stored_even_when_the_key_is_no_longer_wanted() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        let mut scheduler = FrameScheduler::new();
        let a = scheduler.request(FrameSlot::Preview, 1u32, &cache).unwrap();
        scheduler.request(FrameSlot::Preview, 2u32, &cache).unwrap();
        assert_eq!(
            scheduler.complete(a, Err(UnavailableReason::DecodeFailed), &mut cache),
            Completion::Accepted
        );
        assert_eq!(
            cache.peek(&1),
            Some(FrameEntry::Unavailable(&UnavailableReason::DecodeFailed))
        );
    }

    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }
    }

    #[test]
    fn random_interleavings_keep_the_scheduler_consistent() {
        const KEYS: u64 = 12;
        let bound = 2 + 2 * PRELOAD_RADIUS;
        let proto = frame("proto");
        for seed in 1..=40u64 {
            let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            // Large enough that eviction never hides a wanted key.
            let mut cache = FrameCache::new(FRAME_CAPACITY);
            let mut scheduler = FrameScheduler::<u32>::new();
            let mut pending: Vec<Ticket<u32>> = Vec::new();
            let mut program = None;
            let mut preview = None;
            let mut preload: Vec<u32> = Vec::new();
            for _ in 0..400 {
                let key = u32::try_from(rng.below(KEYS)).unwrap();
                match rng.below(6) {
                    0 | 1 => {
                        let slot = if rng.below(2) == 0 {
                            program = Some(key);
                            FrameSlot::Program
                        } else {
                            preview = Some(key);
                            FrameSlot::Preview
                        };
                        pending.extend(scheduler.request(slot, key, &cache));
                    }
                    2 => {
                        let count = usize::try_from(rng.below(7)).unwrap();
                        let keys: Vec<u32> = (0..count)
                            .map(|_| u32::try_from(rng.below(KEYS)).unwrap())
                            .collect();
                        preload.clear();
                        for k in &keys {
                            if preload.len() < 2 * PRELOAD_RADIUS && !preload.contains(k) {
                                preload.push(*k);
                            }
                        }
                        pending.extend(scheduler.request_preload(keys, &cache));
                    }
                    3 if !pending.is_empty() => {
                        let at = usize::try_from(rng.below(pending.len() as u64)).unwrap();
                        let ticket = pending.swap_remove(at);
                        let wanted = program == Some(ticket.key())
                            || preview == Some(ticket.key())
                            || preload.contains(&ticket.key());
                        let (result, is_err) = if rng.below(4) == 0 {
                            (Err(UnavailableReason::DecodeFailed), true)
                        } else {
                            (Ok(proto.clone()), false)
                        };
                        let outcome = scheduler.complete(ticket, result, &mut cache);
                        let expected = if wanted || is_err {
                            Completion::Accepted
                        } else {
                            Completion::Ignored
                        };
                        assert_eq!(outcome, expected);
                    }
                    4 => {
                        for ticket in scheduler.obsolete() {
                            if rng.below(2) == 0 && scheduler.abandon(&ticket) {
                                pending.retain(|t| *t != ticket);
                            }
                        }
                    }
                    _ => {}
                }
                // The executor contract: after a full pass nothing obsolete
                // remains in flight.
                if rng.below(3) == 0 {
                    for ticket in scheduler.obsolete() {
                        assert!(scheduler.abandon(&ticket));
                        pending.retain(|t| *t != ticket);
                    }
                    assert!(scheduler.in_flight() <= bound);
                    assert!(scheduler.obsolete().is_empty());
                }
                // One job per key; every wanted uncached key has its job.
                let mut seen = BTreeSet::new();
                for ticket in &pending {
                    assert!(seen.insert(ticket.key()), "two jobs for one key");
                }
                assert_eq!(pending.len(), scheduler.in_flight());
                let wanted_keys: BTreeSet<u32> = program
                    .into_iter()
                    .chain(preview)
                    .chain(preload.iter().copied())
                    .collect();
                for key in wanted_keys {
                    if cache.peek(&key).is_none() {
                        assert!(seen.contains(&key), "wanted key {key} has no job");
                    }
                }
            }
        }
    }
}
