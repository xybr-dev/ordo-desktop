//! The retained-path frame store (T1.3, Decision 310).
//!
//! Live output and the monitors draw the logical [`Frame`], so the store keeps
//! composed `Frame` values (or the structured reason one could not be
//! composed), never pixels. It is bounded by entry *count*, not bytes: a
//! logical frame is small and a parish Mass is a few hundred Cues, so a count
//! bound is both simple and honest.
//!
//! The Program and Preview keys are pinned. A pinned key is never evicted, so
//! the frame the congregation sees cannot be dropped by preloading pressure.
//! The store is a plain single-owner value: no threads, no clock. Recency is a
//! counter, so behavior is deterministic. Keep one cache per Engine/snapshot:
//! keys are frame identities of that snapshot, so a new snapshot gets a new
//! cache (or [`FrameCache::clear`]).
//!
//! A slot only ever exposes a complete [`Frame`] or a structured
//! [`UnavailableReason`]; there is no partial frame.
//!
//! T1.4 consumer rule: while Program's key is still in flight (not yet in the
//! store) the Program monitor keeps drawing the last frame it drew, which is
//! complete, and the output window does likewise. Black reaches the output
//! only through Engine state (`ProgramContent::Black`), never because a frame
//! has not arrived yet.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::Frame;
use crate::frame_scheduler::{FrameSlot, PRELOAD_RADIUS, UnavailableReason};

/// Composed logical frames kept by default (count bound, not bytes;
/// Decision 310).
pub const FRAME_CAPACITY: usize = 32;

/// The smallest capacity [`FrameCache::new`] accepts: Program and Preview plus
/// a full preload set.
pub const MIN_CAPACITY: usize = 2 + 2 * PRELOAD_RADIUS;

/// What the store holds for one key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameEntry<'a> {
    /// One complete composed frame.
    Ready(&'a Arc<Frame>),
    /// Composition failed; the structured reason every consumer shares.
    Unavailable(&'a UnavailableReason),
}

#[derive(Debug)]
struct Stored {
    entry: Result<Arc<Frame>, UnavailableReason>,
    /// Recency stamp; larger is more recent.
    stamp: u64,
}

impl Stored {
    fn view(&self) -> FrameEntry<'_> {
        match &self.entry {
            Ok(frame) => FrameEntry::Ready(frame),
            Err(reason) => FrameEntry::Unavailable(reason),
        }
    }
}

/// A count-bounded, least-recently-used store of composed frames with
/// Program and Preview pins.
///
/// Unavailable results count against the capacity like frames, so a failing
/// Cue is remembered (and not recomposed in a loop) without growing the store
/// unboundedly. The store holds at most `capacity` entries (pinned ones
/// included). The capacity is clamped to at least [`MIN_CAPACITY`] (the two
/// pins plus the preload set), so a pinned frame can never be crowded out by
/// preloading.
#[derive(Debug)]
pub struct FrameCache<K> {
    capacity: usize,
    entries: HashMap<K, Stored>,
    clock: u64,
    program: Option<K>,
    preview: Option<K>,
}

impl<K: Clone + Eq + Hash> FrameCache<K> {
    /// An empty store keeping at most `capacity` entries, clamped up to
    /// [`MIN_CAPACITY`].
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(MIN_CAPACITY),
            entries: HashMap::new(),
            clock: 0,
            program: None,
            preview: None,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// The entry for `key`, marking it most recently used.
    pub fn get(&mut self, key: &K) -> Option<FrameEntry<'_>> {
        let stamp = self.tick();
        let stored = self.entries.get_mut(key)?;
        stored.stamp = stamp;
        Some(stored.view())
    }

    /// The entry for `key` without changing recency.
    pub fn peek(&self, key: &K) -> Option<FrameEntry<'_>> {
        self.entries.get(key).map(Stored::view)
    }

    /// Stores `entry` for `key` as the most recently used entry, replacing any
    /// earlier one, then evicts the least recently used unpinned entries while
    /// the store is over capacity.
    pub fn insert(&mut self, key: K, entry: Result<Arc<Frame>, UnavailableReason>) {
        let stamp = self.tick();
        self.entries.insert(key, Stored { entry, stamp });
        self.evict();
    }

    /// Pins `key` for the Program or Preview slot, unpinning that slot's
    /// previous key (`None` clears the pin). Pinning [`FrameSlot::Preload`] is
    /// ignored: preloaded frames are exactly the evictable ones.
    ///
    /// A key pinned by either slot is never evicted.
    pub fn pin(&mut self, slot: FrameSlot, key: Option<K>) {
        match slot {
            FrameSlot::Program => self.program = key,
            FrameSlot::Preview => self.preview = key,
            FrameSlot::Preload => return,
        }
        // The old pin may have been the only thing keeping the store over its
        // capacity.
        self.evict();
    }

    /// Entries held, pinned or not.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Drops every entry and both pins (for example when the snapshot the keys
    /// belong to is replaced). Recency restarts from the same counter.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.program = None;
        self.preview = None;
    }

    /// True when nothing is held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn is_pinned(&self, key: &K) -> bool {
        self.program.as_ref() == Some(key) || self.preview.as_ref() == Some(key)
    }

    fn evict(&mut self) {
        while self.entries.len() > self.capacity {
            let victim = self
                .entries
                .iter()
                .filter(|(key, _)| !self.is_pinned(key))
                .min_by_key(|(_, stored)| stored.stamp)
                .map(|(key, _)| key.clone());
            match victim {
                Some(key) => {
                    self.entries.remove(&key);
                }
                // Only pinned entries remain.
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unavailable() -> Result<Arc<Frame>, UnavailableReason> {
        Err(UnavailableReason::DecodeFailed)
    }

    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    const CAP: usize = MIN_CAPACITY;

    #[test]
    fn evicts_least_recently_used_first() {
        let mut cache = FrameCache::new(CAP);
        for key in 0..CAP as u32 {
            cache.insert(key, unavailable());
        }
        // Touch 0 so 1 becomes the oldest.
        assert!(cache.get(&0).is_some());
        cache.insert(100, unavailable());
        assert_eq!(cache.len(), CAP);
        assert!(cache.peek(&1).is_none());
        assert!(cache.peek(&0).is_some());
        assert!(cache.peek(&2).is_some());
        assert!(cache.peek(&100).is_some());
    }

    #[test]
    fn peek_does_not_touch_recency() {
        let mut cache = FrameCache::new(CAP);
        for key in 0..CAP as u32 {
            cache.insert(key, unavailable());
        }
        assert!(cache.peek(&0).is_some());
        cache.insert(100, unavailable());
        assert!(cache.peek(&0).is_none());
        assert!(cache.peek(&1).is_some());
    }

    #[test]
    fn capacity_is_clamped_to_pins_plus_preload() {
        let mut cache = FrameCache::new(1);
        for key in 0..50u32 {
            cache.insert(key, unavailable());
        }
        assert_eq!(cache.len(), MIN_CAPACITY);
    }

    #[test]
    fn pinned_keys_survive_and_repinning_releases_the_old_key() {
        let mut cache = FrameCache::new(CAP);
        cache.insert(0u32, unavailable());
        cache.pin(FrameSlot::Program, Some(0));
        for key in 1..=20 {
            cache.insert(key, unavailable());
        }
        assert_eq!(cache.len(), CAP);
        assert!(cache.peek(&0).is_some());
        // Re-pinning releases 0, which is now the oldest.
        cache.pin(FrameSlot::Program, Some(20));
        cache.insert(21, unavailable());
        assert!(cache.peek(&0).is_none());
        assert!(cache.peek(&20).is_some());
        // Clearing a pin and pinning Preload both leave nothing protected.
        cache.pin(FrameSlot::Program, None);
        cache.pin(FrameSlot::Preload, Some(20));
        for key in 30..40 {
            cache.insert(key, unavailable());
        }
        assert!(cache.peek(&20).is_none());
    }

    #[test]
    fn clear_drops_entries_and_pins() {
        let mut cache = FrameCache::new(CAP);
        cache.insert(0u32, unavailable());
        cache.pin(FrameSlot::Program, Some(0));
        cache.clear();
        assert!(cache.is_empty());
        for key in 1..=20 {
            cache.insert(key, unavailable());
        }
        cache.insert(0, unavailable());
        for key in 30..40 {
            cache.insert(key, unavailable());
        }
        assert!(cache.peek(&0).is_none(), "the old pin is gone");
    }

    #[test]
    fn pinned_program_and_preview_survive_random_inserts() {
        let mut cache = FrameCache::new(FRAME_CAPACITY);
        cache.insert(1_000_000u32, unavailable());
        cache.insert(1_000_001, unavailable());
        cache.pin(FrameSlot::Program, Some(1_000_000));
        cache.pin(FrameSlot::Preview, Some(1_000_001));
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..10_000 {
            let key = u32::try_from(rng.next() % 500).unwrap();
            cache.insert(key, unavailable());
            assert!(cache.len() <= FRAME_CAPACITY);
            assert!(cache.peek(&1_000_000).is_some());
            assert!(cache.peek(&1_000_001).is_some());
        }
    }

    /// A Vec-based LRU (oldest first) with the same pin rules.
    struct Model {
        order: Vec<u32>,
        program: Option<u32>,
        preview: Option<u32>,
    }

    impl Model {
        fn evict(&mut self) {
            while self.order.len() > CAP {
                let pos = self
                    .order
                    .iter()
                    .position(|k| Some(*k) != self.program && Some(*k) != self.preview);
                match pos {
                    Some(at) => {
                        self.order.remove(at);
                    }
                    None => break,
                }
            }
        }

        fn touch(&mut self, key: u32) {
            if let Some(at) = self.order.iter().position(|k| *k == key) {
                self.order.remove(at);
                self.order.push(key);
            }
        }
    }

    #[test]
    fn random_operations_match_a_simple_lru_model() {
        for seed in 1..=20u64 {
            let mut rng = XorShift(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
            let mut cache = FrameCache::new(CAP);
            let mut model = Model {
                order: Vec::new(),
                program: None,
                preview: None,
            };
            for _ in 0..2_000 {
                let key = u32::try_from(rng.next() % 20).unwrap();
                match rng.next() % 6 {
                    0..=2 => {
                        cache.insert(key, unavailable());
                        model.touch(key);
                        if !model.order.contains(&key) {
                            model.order.push(key);
                        }
                        model.evict();
                    }
                    3 => {
                        assert_eq!(cache.get(&key).is_some(), model.order.contains(&key));
                        model.touch(key);
                    }
                    4 => {
                        let pin = (!rng.next().is_multiple_of(4)).then_some(key);
                        if rng.next().is_multiple_of(2) {
                            cache.pin(FrameSlot::Program, pin);
                            model.program = pin;
                        } else {
                            cache.pin(FrameSlot::Preview, pin);
                            model.preview = pin;
                        }
                        model.evict();
                    }
                    _ => {
                        assert_eq!(cache.peek(&key).is_some(), model.order.contains(&key));
                    }
                }
                assert_eq!(cache.len(), model.order.len());
                for k in 0..20u32 {
                    assert_eq!(
                        cache.peek(&k).is_some(),
                        model.order.contains(&k),
                        "key {k}"
                    );
                }
            }
        }
    }

    #[test]
    fn unavailable_entries_count_toward_capacity() {
        let mut cache = FrameCache::new(CAP);
        for key in 0..20u32 {
            cache.insert(key, unavailable());
        }
        assert_eq!(cache.len(), CAP);
        assert!(!cache.is_empty());
        assert_eq!(
            cache.peek(&19),
            Some(FrameEntry::Unavailable(&UnavailableReason::DecodeFailed))
        );
    }
}
