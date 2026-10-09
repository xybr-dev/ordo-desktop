//! Frame and thumbnail cache bounds, exercised through the public interface.

use presentation::ProgramFrame;
use presentation::cache::{FrameCache, ThumbnailCache, downscale};

struct XorShift(u64);

impl XorShift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// A full-resolution frame with a gradient so downscaling has real content.
fn gradient(revision: u64, width: u32, height: u32) -> ProgramFrame {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
        }
    }
    ProgramFrame::new(revision, width, height, rgba).unwrap()
}

#[test]
fn frame_cache_stays_within_budget_under_random_churn() {
    let budget = 64 * 1024;
    let mut cache: FrameCache<u32> = FrameCache::with_budget(budget);
    let mut rng = XorShift(0xA076_1D64_78BD_642F);
    for step in 0..5_000u32 {
        let key = (rng.next_u64() % 40) as u32;
        if rng.next_u64().is_multiple_of(3) {
            let _ = cache.get(&key);
        } else {
            let side = 4 + (rng.next_u64() % 40) as u32;
            let frame = gradient(u64::from(step), side, side);
            let _ = cache.insert(key, frame);
        }
        assert!(
            cache.held_bytes() <= budget,
            "step {step}: held {} exceeds budget {budget}",
            cache.held_bytes()
        );
    }
}

#[test]
fn frame_cache_rejects_oversized_frames_and_keeps_its_contents() {
    let mut cache = FrameCache::with_budget(1024);
    cache.insert(1u32, gradient(1, 8, 8)).unwrap();
    let before = cache.held_bytes();

    assert!(cache.insert(2u32, gradient(2, 64, 64)).is_err());
    assert_eq!(cache.held_bytes(), before);
    assert!(cache.get(&1).is_some());
    assert!(cache.get(&2).is_none());
}

#[test]
fn thumbnails_are_smaller_than_their_source_and_released_when_obsolete() {
    let source = gradient(1, 1920, 1080);
    let mut thumbnails = ThumbnailCache::new();
    for cue in 0..500usize {
        thumbnails.insert(cue, &source, 192, 108);
    }

    let thumb = thumbnails.get(&0).unwrap();
    assert!(thumb.width() < source.width());
    assert!(thumb.height() < source.height());
    assert!(thumb.rgba().len() * 50 < source.rgba().len());
    let per_thumb = thumb.rgba().len();
    assert_eq!(thumbnails.len(), 500);
    assert_eq!(thumbnails.held_bytes(), per_thumb * 500);

    // Scrolling the operator's window releases everything outside it.
    thumbnails.retain_visible(240..260);
    assert_eq!(thumbnails.len(), 20);
    assert!(thumbnails.get(&239).is_none());
    assert!(thumbnails.get(&240).is_some());
    assert_eq!(thumbnails.held_bytes(), per_thumb * 20);

    thumbnails.retain_visible(0..0);
    assert!(thumbnails.is_empty());
    assert_eq!(thumbnails.held_bytes(), 0);
}

#[test]
fn downscale_is_pure_and_bounded_by_the_box() {
    let source = gradient(3, 1280, 720);
    let first = downscale(&source, 200, 120);
    let second = downscale(&source, 200, 120);
    assert_eq!(first, second);
    assert!(first.width() <= 200 && first.height() <= 120);
    assert_eq!(first.revision(), 3);
}
