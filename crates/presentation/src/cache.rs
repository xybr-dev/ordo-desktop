//! Bounded caches for Program frames and their lower-resolution thumbnails.
//!
//! Both caches are pure in-memory structures. They hold complete
//! [`ProgramFrame`] values and account for their bytes exactly, so memory use
//! is a function of the configured budget, not of how long a deck is open.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::ops::Range;

use crate::ProgramFrame;

/// A frame that cannot fit the cache even when it is empty. Nothing changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameTooLarge {
    pub bytes: usize,
    pub budget: usize,
}

/// Byte-bounded least-recently-used cache of full-resolution frames.
///
/// Invariant: `held_bytes() <= budget()` after every call. Inserting past the
/// budget evicts the least recently used entries first; a frame larger than
/// the whole budget is rejected and leaves the cache untouched.
pub struct FrameCache<K> {
    budget: usize,
    held: usize,
    tick: u64,
    entries: HashMap<K, Entry>,
    /// Ordered by last use: the first key is the eviction candidate.
    recency: BTreeMap<u64, K>,
}

struct Entry {
    frame: ProgramFrame,
    tick: u64,
}

impl<K: Eq + Hash + Clone> FrameCache<K> {
    /// An empty cache that never holds more than `bytes` of pixel data.
    pub fn with_budget(bytes: usize) -> Self {
        Self {
            budget: bytes,
            held: 0,
            tick: 0,
            entries: HashMap::new(),
            recency: BTreeMap::new(),
        }
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Total RGBA bytes currently held. Never exceeds [`FrameCache::budget`].
    pub fn held_bytes(&self) -> usize {
        self.held
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Store `frame` under `key`, replacing any previous frame for that key
    /// and evicting least recently used entries until it fits.
    pub fn insert(&mut self, key: K, frame: ProgramFrame) -> Result<(), FrameTooLarge> {
        let bytes = frame.rgba().len();
        if bytes > self.budget {
            return Err(FrameTooLarge {
                bytes,
                budget: self.budget,
            });
        }
        self.remove(&key);
        while self.held + bytes > self.budget {
            let Some((_, oldest)) = self.recency.pop_first() else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&oldest) {
                self.held -= evicted.frame.rgba().len();
            }
        }
        self.tick += 1;
        self.recency.insert(self.tick, key.clone());
        self.entries.insert(
            key,
            Entry {
                frame,
                tick: self.tick,
            },
        );
        self.held += bytes;
        Ok(())
    }

    /// Look up a frame and mark it most recently used.
    pub fn get(&mut self, key: &K) -> Option<&ProgramFrame> {
        let entry = self.entries.get_mut(key)?;
        self.recency.remove(&entry.tick);
        self.tick += 1;
        entry.tick = self.tick;
        self.recency.insert(self.tick, key.clone());
        self.entries.get(key).map(|entry| &entry.frame)
    }

    /// Drop the frame under `key`, if any, and release its bytes.
    pub fn remove(&mut self, key: &K) {
        if let Some(entry) = self.entries.remove(key) {
            self.recency.remove(&entry.tick);
            self.held -= entry.frame.rgba().len();
        }
    }
}

/// Lower-resolution thumbnails keyed by Cue, released when they fall outside
/// the window the operator can currently see.
///
/// Thumbnails are produced by [`downscale`] at insert time, so the cache holds
/// only the small copy; the full-resolution source is never retained here.
pub struct ThumbnailCache<K> {
    entries: BTreeMap<K, ProgramFrame>,
    held: usize,
}

impl<K: Ord> ThumbnailCache<K> {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            held: 0,
        }
    }

    /// Downscale `source` to fit `max_w` by `max_h` and store it under `key`,
    /// replacing any previous thumbnail for that key.
    pub fn insert(&mut self, key: K, source: &ProgramFrame, max_w: u32, max_h: u32) {
        let thumbnail = downscale(source, max_w, max_h);
        let bytes = thumbnail.rgba().len();
        if let Some(previous) = self.entries.insert(key, thumbnail) {
            self.held -= previous.rgba().len();
        }
        self.held += bytes;
    }

    pub fn get(&self, key: &K) -> Option<&ProgramFrame> {
        self.entries.get(key)
    }

    /// Release every thumbnail whose key lies outside `visible`.
    pub fn retain_visible(&mut self, visible: Range<K>) {
        let mut released = 0;
        self.entries.retain(|key, thumbnail| {
            let keep = visible.contains(key);
            if !keep {
                released += thumbnail.rgba().len();
            }
            keep
        });
        self.held -= released;
    }

    /// Total RGBA bytes of the thumbnails currently held.
    pub fn held_bytes(&self) -> usize {
        self.held
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: Ord> Default for ThumbnailCache<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Nearest-neighbour downscale of `source` to fit inside `max_w` by `max_h`,
/// preserving aspect ratio.
///
/// A source that already fits is returned unchanged, so a thumbnail is never
/// larger than its source. Each output pixel samples the source pixel under
/// the centre of its own cell, which keeps the mapping symmetric and free of
/// interpolation, so the result is a pure function of the input bytes.
pub fn downscale(source: &ProgramFrame, max_w: u32, max_h: u32) -> ProgramFrame {
    let (w, h) = (source.width(), source.height());
    let max_w = max_w.max(1);
    let max_h = max_h.max(1);
    if w <= max_w && h <= max_h {
        return source.clone();
    }

    // Fit the tighter of the two scale factors. The comparison is exact in
    // u64: w * max_h > h * max_w means the width limit binds first.
    let (out_w, out_h) = if u64::from(w) * u64::from(max_h) > u64::from(h) * u64::from(max_w) {
        let out_h = (u64::from(h) * u64::from(max_w) / u64::from(w)).max(1);
        (max_w, out_h as u32)
    } else {
        let out_w = (u64::from(w) * u64::from(max_h) / u64::from(h)).max(1);
        (out_w as u32, max_h)
    };

    let src = source.rgba();
    let mut out = Vec::with_capacity(out_w as usize * out_h as usize * 4);
    for y in 0..out_h {
        let sy = sample(y, out_h, h) as usize;
        for x in 0..out_w {
            let sx = sample(x, out_w, w) as usize;
            let index = (sy * w as usize + sx) * 4;
            out.extend_from_slice(&src[index..index + 4]);
        }
    }
    // The buffer length matches the dimensions by construction, so this
    // cannot fail; falling back to the source keeps the function total.
    ProgramFrame::new(source.revision(), out_w, out_h, out).unwrap_or_else(|_| source.clone())
}

/// Map output index `index` of `out_extent` cells to the source cell under its
/// centre, out of `src_extent` cells.
fn sample(index: u32, out_extent: u32, src_extent: u32) -> u32 {
    let centre = (2 * u64::from(index) + 1) * u64::from(src_extent) / (2 * u64::from(out_extent));
    centre.min(u64::from(src_extent) - 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(revision: u64, width: u32, height: u32, value: u8) -> ProgramFrame {
        let rgba = vec![value; width as usize * height as usize * 4];
        ProgramFrame::new(revision, width, height, rgba).unwrap()
    }

    #[test]
    fn frame_cache_never_exceeds_its_budget() {
        let mut cache = FrameCache::with_budget(100);
        for key in 0..20u32 {
            cache.insert(key, solid(1, 5, 2, key as u8)).unwrap();
            assert!(cache.held_bytes() <= cache.budget());
        }
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.held_bytes(), 80);
    }

    #[test]
    fn frame_cache_evicts_least_recently_used_first() {
        let mut cache = FrameCache::with_budget(100);
        cache.insert("a", solid(1, 5, 2, 1)).unwrap();
        cache.insert("b", solid(1, 5, 2, 2)).unwrap();
        assert!(cache.get(&"a").is_some());
        cache.insert("c", solid(1, 5, 2, 3)).unwrap();

        assert!(cache.get(&"b").is_none(), "b was least recently used");
        assert!(cache.get(&"a").is_some());
        assert!(cache.get(&"c").is_some());
    }

    #[test]
    fn frame_cache_rejects_a_frame_larger_than_the_budget_without_change() {
        let mut cache = FrameCache::with_budget(40);
        cache.insert("a", solid(1, 5, 2, 1)).unwrap();
        let too_big = solid(2, 5, 3, 9);

        assert_eq!(
            cache.insert("b", too_big),
            Err(FrameTooLarge {
                bytes: 60,
                budget: 40
            })
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.held_bytes(), 40);
        assert!(cache.get(&"a").is_some());
    }

    #[test]
    fn reinserting_a_key_replaces_its_bytes_without_double_counting() {
        let mut cache = FrameCache::with_budget(100);
        cache.insert("a", solid(1, 5, 2, 1)).unwrap();
        cache.insert("a", solid(2, 5, 2, 2)).unwrap();
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.held_bytes(), 40);
        assert_eq!(cache.get(&"a").unwrap().revision(), 2);
    }

    #[test]
    fn downscale_fits_the_box_and_keeps_aspect() {
        let source = solid(7, 1920, 1080, 128);
        let thumb = downscale(&source, 160, 160);
        assert_eq!((thumb.width(), thumb.height()), (160, 90));
        assert_eq!(thumb.revision(), 7);
        assert!(thumb.rgba().len() < source.rgba().len());
    }

    #[test]
    fn downscale_samples_nearest_pixels() {
        // A 4x1 strip of four distinct pixels downscaled to 2x1 keeps the
        // pixels whose cell centres fall on them: indices 1 and 3.
        let rgba = vec![10, 0, 0, 255, 20, 0, 0, 255, 30, 0, 0, 255, 40, 0, 0, 255];
        let source = ProgramFrame::new(1, 4, 1, rgba).unwrap();
        let thumb = downscale(&source, 2, 1);
        assert_eq!((thumb.width(), thumb.height()), (2, 1));
        assert_eq!(thumb.rgba(), &[20, 0, 0, 255, 40, 0, 0, 255]);
    }

    #[test]
    fn downscale_returns_small_sources_unchanged() {
        let source = solid(1, 4, 3, 5);
        assert_eq!(downscale(&source, 160, 90), source);
    }

    #[test]
    fn thumbnail_cache_releases_entries_outside_the_visible_window() {
        let source = solid(1, 320, 180, 200);
        let mut cache = ThumbnailCache::new();
        for cue in 0..10usize {
            cache.insert(cue, &source, 32, 18);
        }
        let per_thumb = cache.get(&0).unwrap().rgba().len();
        assert_eq!(cache.held_bytes(), per_thumb * 10);

        cache.retain_visible(3..6);

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.held_bytes(), per_thumb * 3);
        assert!(cache.get(&2).is_none());
        assert!(cache.get(&4).is_some());
    }
}
