//! Frame-interval timing and dropped-frame counting.
//!
//! The caller reports the instant each frame was presented. The timer keeps
//! the interval between consecutive frames in a bounded histogram and counts
//! an interval as dropped when it exceeds `drop_factor` times the expected
//! refresh interval.

use std::time::{Duration, Instant};

use super::latency::LatencyHistogram;

/// 60 Hz refresh interval, 16.67 ms.
pub const DEFAULT_EXPECTED_FRAME: Duration = Duration::from_micros(16_670);

/// An interval longer than this many expected frames counts as dropped.
pub const DEFAULT_DROP_FACTOR: f64 = 2.0;

#[derive(Clone, Debug)]
pub struct FrameTimer {
    expected: Duration,
    drop_factor: f64,
    last: Option<Instant>,
    intervals: LatencyHistogram,
    frames: u64,
    dropped: u64,
}

impl FrameTimer {
    /// A timer for the default 60 Hz interval with a 2x drop threshold.
    pub fn new() -> Self {
        Self::with_expected(DEFAULT_EXPECTED_FRAME, DEFAULT_DROP_FACTOR)
    }

    /// A timer for an explicit refresh interval and drop factor.
    ///
    /// A non-finite or non-positive `drop_factor` falls back to
    /// [`DEFAULT_DROP_FACTOR`].
    pub fn with_expected(expected: Duration, drop_factor: f64) -> Self {
        let drop_factor = if drop_factor.is_finite() && drop_factor > 0.0 {
            drop_factor
        } else {
            DEFAULT_DROP_FACTOR
        };
        Self {
            expected,
            drop_factor,
            last: None,
            intervals: LatencyHistogram::new(),
            frames: 0,
            dropped: 0,
        }
    }

    /// Records a presented frame at `at`.
    ///
    /// The first frame only establishes the baseline. An instant earlier than
    /// the previous frame is ignored rather than producing a negative interval.
    pub fn record_frame(&mut self, at: Instant) {
        let Some(last) = self.last else {
            self.last = Some(at);
            self.frames = 1;
            return;
        };
        let Some(interval) = at.checked_duration_since(last) else {
            return;
        };
        self.last = Some(at);
        self.frames += 1;
        self.intervals.record(interval);
        if interval > self.drop_threshold() {
            self.dropped += 1;
        }
    }

    /// The interval beyond which a frame counts as dropped.
    pub fn drop_threshold(&self) -> Duration {
        self.expected.mul_f64(self.drop_factor)
    }

    /// The expected refresh interval this timer was configured with.
    pub fn expected_interval(&self) -> Duration {
        self.expected
    }

    /// Frames presented, including the first baseline frame.
    pub fn frame_count(&self) -> u64 {
        self.frames
    }

    /// Intervals that exceeded the drop threshold.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }

    /// Frame-to-frame intervals observed so far.
    pub fn intervals(&self) -> &LatencyHistogram {
        &self.intervals
    }

    /// Nearest-rank percentiles of the frame intervals, one per entry of `ps`,
    /// sorted once for the batch. See [`LatencyHistogram::percentiles`].
    pub fn percentiles(&self, ps: &[f64]) -> Vec<Option<Duration>> {
        self.intervals.percentiles(ps)
    }

    pub fn p50(&self) -> Option<Duration> {
        self.intervals.p50()
    }

    pub fn p95(&self) -> Option<Duration> {
        self.intervals.p95()
    }

    pub fn p99(&self) -> Option<Duration> {
        self.intervals.p99()
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_is_baseline_only() {
        let mut timer = FrameTimer::new();
        timer.record_frame(Instant::now());
        assert_eq!(timer.frame_count(), 1);
        assert_eq!(timer.intervals().count(), 0);
        assert_eq!(timer.p95(), None);
        assert_eq!(timer.dropped_frames(), 0);
    }

    #[test]
    fn default_threshold_is_twice_sixty_hertz_interval() {
        let timer = FrameTimer::new();
        assert_eq!(timer.expected_interval(), Duration::from_micros(16_670));
        assert_eq!(timer.drop_threshold(), Duration::from_micros(33_340));
    }

    #[test]
    fn counts_only_intervals_above_threshold_as_dropped() {
        let start = Instant::now();
        let mut timer = FrameTimer::new();
        let mut at = start;
        timer.record_frame(at);
        // Three on-time frames at 16 ms, then one at 33 ms (not > 33.34 ms),
        // then one at 40 ms (dropped), then one at 100 ms (dropped).
        for gap in [16, 16, 16, 33, 40, 100] {
            at += Duration::from_millis(gap);
            timer.record_frame(at);
        }
        assert_eq!(timer.frame_count(), 7);
        assert_eq!(timer.intervals().count(), 6);
        assert_eq!(timer.dropped_frames(), 2);
        assert_eq!(timer.p50(), Some(Duration::from_millis(16)));
        assert_eq!(timer.p99(), Some(Duration::from_millis(100)));
    }

    #[test]
    fn configurable_drop_factor_changes_threshold() {
        let start = Instant::now();
        let mut timer = FrameTimer::with_expected(Duration::from_millis(10), 3.0);
        timer.record_frame(start);
        timer.record_frame(start + Duration::from_millis(29));
        // 29 ms is within the 30 ms threshold; the following 31 ms is not.
        timer.record_frame(start + Duration::from_millis(60));
        assert_eq!(timer.drop_threshold(), Duration::from_millis(30));
        assert_eq!(timer.dropped_frames(), 1);
    }

    #[test]
    fn invalid_drop_factor_falls_back_to_default() {
        let timer = FrameTimer::with_expected(Duration::from_millis(10), f64::NAN);
        assert_eq!(timer.drop_threshold(), Duration::from_millis(20));
    }

    #[test]
    fn backwards_instant_is_ignored() {
        let start = Instant::now();
        let mut timer = FrameTimer::new();
        timer.record_frame(start + Duration::from_millis(50));
        timer.record_frame(start);
        assert_eq!(timer.frame_count(), 1);
        assert_eq!(timer.dropped_frames(), 0);
    }
}
