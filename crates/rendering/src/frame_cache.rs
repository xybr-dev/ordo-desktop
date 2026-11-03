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
//! counter, so behavior is deterministic.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::Frame;
use crate::frame_scheduler::{FrameSlot, UnavailableReason};

/// Composed logical frames kept by default (count bound, not bytes;
/// Decision 310).
pub const FRAME_CAPACITY: usize = 32;

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
/// included), except that pinned entries are never evicted, so a capacity
/// below the number of pinned keys (two) is exceeded by the pins alone.
#[derive(Debug)]
pub struct FrameCache<K> {
    capacity: usize,
    entries: HashMap<K, Stored>,
    clock: u64,
    program: Option<K>,
    preview: Option<K>,
}

impl<K: Clone + Eq + Hash> FrameCache<K> {
    /// An empty store keeping at most `capacity` entries (see the type docs
    /// for the pinned exception).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
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
        Err(UnavailableReason::MissingAsset)
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

    #[test]
    fn evicts_least_recently_used_first() {
        let mut cache = FrameCache::new(3);
        for key in 0..3u32 {
            cache.insert(key, unavailable());
        }
        // Touch 0 so 1 becomes the oldest.
        assert!(cache.get(&0).is_some());
        cache.insert(3, unavailable());
        assert_eq!(cache.len(), 3);
        assert!(cache.peek(&1).is_none());
        assert!(cache.peek(&0).is_some());
        assert!(cache.peek(&2).is_some());
        assert!(cache.peek(&3).is_some());
    }

    #[test]
    fn peek_does_not_touch_recency() {
        let mut cache = FrameCache::new(2);
        cache.insert(0u32, unavailable());
        cache.insert(1, unavailable());
        assert!(cache.peek(&0).is_some());
        cache.insert(2, unavailable());
        assert!(cache.peek(&0).is_none());
        assert!(cache.peek(&1).is_some());
    }

    #[test]
    fn pinned_keys_survive_and_repinning_releases_the_old_key() {
        let mut cache = FrameCache::new(2);
        cache.insert(0u32, unavailable());
        cache.pin(FrameSlot::Program, Some(0));
        cache.insert(1, unavailable());
        cache.insert(2, unavailable());
        cache.insert(3, unavailable());
        assert_eq!(cache.len(), 2);
        assert!(cache.peek(&0).is_some());
        assert!(cache.peek(&3).is_some());
        // Re-pinning releases 0, which is now the oldest.
        cache.pin(FrameSlot::Program, Some(3));
        cache.insert(4, unavailable());
        assert!(cache.peek(&0).is_none());
        assert!(cache.peek(&3).is_some());
        // Clearing a pin and pinning Preload both leave nothing protected.
        cache.pin(FrameSlot::Program, None);
        cache.pin(FrameSlot::Preload, Some(3));
        cache.insert(5, unavailable());
        cache.insert(6, unavailable());
        assert!(cache.peek(&3).is_none());
    }

    #[test]
    fn pins_alone_may_exceed_a_tiny_capacity() {
        let mut cache = FrameCache::new(1);
        cache.pin(FrameSlot::Program, Some(0u32));
        cache.pin(FrameSlot::Preview, Some(1));
        cache.insert(0, unavailable());
        cache.insert(1, unavailable());
        cache.insert(2, unavailable());
        assert_eq!(cache.len(), 2);
        assert!(cache.peek(&0).is_some() && cache.peek(&1).is_some());
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
            assert!(cache.len() <= FRAME_CAPACITY + 2);
            assert!(cache.peek(&1_000_000).is_some());
            assert!(cache.peek(&1_000_001).is_some());
        }
    }

    #[test]
    fn unavailable_entries_count_toward_capacity() {
        let mut cache = FrameCache::new(2);
        for key in 0..10u32 {
            cache.insert(key, unavailable());
        }
        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());
        assert_eq!(
            cache.peek(&9),
            Some(FrameEntry::Unavailable(&UnavailableReason::MissingAsset))
        );
    }
}
