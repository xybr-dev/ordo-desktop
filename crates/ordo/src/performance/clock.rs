//! Startup timing and worker-queue latency.

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use super::latency::LatencyHistogram;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

/// Measures time from process start to the first operator paint.
///
/// The process start instant is captured once, by the first call to
/// [`StartupClock::capture_process_start`] or [`StartupClock::process_start`].
/// `main` must call `capture_process_start` as its first statement. A later
/// capture would understate startup time, because the clock would start
/// after the work it should measure.
#[derive(Clone, Debug, Default)]
pub struct StartupClock {
    first_paint: Option<Duration>,
}

impl StartupClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Captures the process start instant if it is not already set, and
    /// returns it. Intended to be called first thing in `main`.
    pub fn capture_process_start() -> Instant {
        Self::process_start()
    }

    /// The process start instant, set on first use.
    pub fn process_start() -> Instant {
        *PROCESS_START.get_or_init(Instant::now)
    }

    /// Records the first operator paint at `at`. Later calls are ignored, so
    /// the value always reflects the first paint. Returns the recorded
    /// elapsed time.
    pub fn record_first_operator_paint(&mut self, at: Instant) -> Duration {
        *self
            .first_paint
            .get_or_insert_with(|| at.saturating_duration_since(Self::process_start()))
    }

    /// Elapsed time from process start to the first operator paint, if any.
    pub fn time_to_first_paint(&self) -> Option<Duration> {
        self.first_paint
    }
}

/// A message stamped with the instant it was enqueued.
#[derive(Clone, Debug)]
pub struct Enqueued<T> {
    message: T,
    enqueued_at: Instant,
}

impl<T> Enqueued<T> {
    /// Wraps `message`, stamping it with the current instant.
    pub fn new(message: T) -> Self {
        Self {
            message,
            enqueued_at: Instant::now(),
        }
    }

    /// When the message was enqueued.
    pub fn enqueued_at(&self) -> Instant {
        self.enqueued_at
    }

    /// Borrows the message.
    pub fn message(&self) -> &T {
        &self.message
    }

    /// Splits into the message and its enqueue instant.
    pub fn into_parts(self) -> (T, Instant) {
        (self.message, self.enqueued_at)
    }
}

/// Histogram of enqueue-to-dequeue latency for one worker queue.
#[derive(Clone, Debug, Default)]
pub struct QueueLatency {
    histogram: LatencyHistogram,
}

impl QueueLatency {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the wait between enqueue and `dequeued_at` and returns it.
    pub fn observe<T>(&mut self, item: &Enqueued<T>, dequeued_at: Instant) -> Duration {
        let wait = dequeued_at.saturating_duration_since(item.enqueued_at);
        self.histogram.record(wait);
        wait
    }

    pub fn histogram(&self) -> &LatencyHistogram {
        &self.histogram
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_start_is_stable_once_captured() {
        let first = StartupClock::capture_process_start();
        let second = StartupClock::process_start();
        assert_eq!(first, second);
    }

    #[test]
    fn first_paint_is_recorded_once() {
        let mut clock = StartupClock::new();
        assert_eq!(clock.time_to_first_paint(), None);
        let start = StartupClock::process_start();
        let first = clock.record_first_operator_paint(start + Duration::from_millis(700));
        assert_eq!(first, Duration::from_millis(700));
        clock.record_first_operator_paint(start + Duration::from_millis(900));
        assert_eq!(
            clock.time_to_first_paint(),
            Some(Duration::from_millis(700))
        );
    }

    #[test]
    fn queue_latency_records_dequeue_wait() {
        let item = Enqueued::new("render");
        let enqueued_at = item.enqueued_at();
        let mut queue = QueueLatency::new();
        let wait = queue.observe(&item, enqueued_at + Duration::from_millis(3));
        assert_eq!(wait, Duration::from_millis(3));
        assert_eq!(queue.histogram().count(), 1);
        assert_eq!(queue.histogram().p95(), Some(Duration::from_millis(3)));
        let (message, stamp) = item.into_parts();
        assert_eq!(message, "render");
        assert_eq!(stamp, enqueued_at);
    }
}
