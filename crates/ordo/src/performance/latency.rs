//! Command-latency histograms keyed by command kind and painted surface.
//!
//! The latency we measure is keydown to the first paint of the affected
//! surface. [`PendingKeydown`] takes the keydown stamp; the caller passes the
//! stamp back when the surface paints and the elapsed time lands in a
//! [`LatencyBook`].

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// Samples retained per histogram. Older samples are overwritten, so memory
/// stays bounded across long sessions.
pub const DEFAULT_SAMPLE_CAPACITY: usize = 100_000;

/// The operator commands whose latency the spike measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandKind {
    Navigation,
    Freeze,
    Blackout,
    Space,
}

impl CommandKind {
    /// Every command kind, in report order.
    pub const ALL: [CommandKind; 4] = [
        CommandKind::Navigation,
        CommandKind::Freeze,
        CommandKind::Blackout,
        CommandKind::Space,
    ];

    /// Stable lowercase name used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            CommandKind::Navigation => "navigation",
            CommandKind::Freeze => "freeze",
            CommandKind::Blackout => "blackout",
            CommandKind::Space => "space",
        }
    }
}

/// The two painted surfaces whose latency is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Surface {
    Preview,
    Program,
}

impl Surface {
    /// Every surface, in report order.
    pub const ALL: [Surface; 2] = [Surface::Preview, Surface::Program];

    /// Stable lowercase name used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Surface::Preview => "preview",
            Surface::Program => "program",
        }
    }
}

/// A bounded set of duration samples with nearest-rank percentiles.
///
/// Once `capacity` samples are held, each new sample overwrites the oldest one.
/// [`count`](Self::count) reports retained samples; [`total_recorded`](Self::total_recorded)
/// reports every sample ever recorded.
#[derive(Clone, Debug)]
pub struct LatencyHistogram {
    samples: Vec<Duration>,
    next: usize,
    capacity: usize,
    total: u64,
}

impl LatencyHistogram {
    /// Creates a histogram with [`DEFAULT_SAMPLE_CAPACITY`].
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_SAMPLE_CAPACITY)
    }

    /// Creates a histogram retaining at most `capacity` samples (minimum 1).
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            samples: Vec::new(),
            next: 0,
            capacity,
            total: 0,
        }
    }

    /// Records one sample.
    pub fn record(&mut self, sample: Duration) {
        self.total = self.total.saturating_add(1);
        if self.samples.len() < self.capacity {
            self.samples.push(sample);
        } else {
            self.samples[self.next] = sample;
            self.next = (self.next + 1) % self.capacity;
        }
    }

    /// Number of samples currently retained.
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Number of samples ever recorded, including those since overwritten.
    pub fn total_recorded(&self) -> u64 {
        self.total
    }

    /// Nearest-rank percentile of the retained samples.
    ///
    /// `p` is in percent and is clamped to `0.0..=100.0`. Returns `None` when
    /// there are no samples or `p` is NaN.
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.samples.is_empty() || p.is_nan() {
            return None;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let n = sorted.len();
        let p = p.clamp(0.0, 100.0);
        let rank = ((p / 100.0) * n as f64).ceil() as usize;
        let rank = rank.clamp(1, n);
        Some(sorted[rank - 1])
    }

    /// Median (nearest-rank 50th percentile).
    pub fn p50(&self) -> Option<Duration> {
        self.percentile(50.0)
    }

    /// Nearest-rank 95th percentile.
    pub fn p95(&self) -> Option<Duration> {
        self.percentile(95.0)
    }

    /// Nearest-rank 99th percentile.
    pub fn p99(&self) -> Option<Duration> {
        self.percentile(99.0)
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

/// One histogram per (command, surface) pair.
#[derive(Clone, Debug, Default)]
pub struct LatencyBook {
    histograms: BTreeMap<(CommandKind, Surface), LatencyHistogram>,
}

impl LatencyBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a latency for `command` painted on `surface`.
    pub fn record(&mut self, command: CommandKind, surface: Surface, latency: Duration) {
        self.histograms
            .entry((command, surface))
            .or_default()
            .record(latency);
    }

    /// The histogram for a pair, if any sample has been recorded for it.
    pub fn histogram(&self, command: CommandKind, surface: Surface) -> Option<&LatencyHistogram> {
        self.histograms.get(&(command, surface))
    }

    /// Every recorded pair in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (CommandKind, Surface, &LatencyHistogram)> {
        self.histograms
            .iter()
            .map(|((command, surface), histogram)| (*command, *surface, histogram))
    }
}

/// Keydown timestamp for one in-flight command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeydownStamp {
    kind: CommandKind,
    at: Instant,
}

impl KeydownStamp {
    /// The command this keydown started.
    pub fn kind(&self) -> CommandKind {
        self.kind
    }

    /// When the keydown was observed.
    pub fn at(&self) -> Instant {
        self.at
    }
}

/// Starts and finishes keydown-to-paint measurements.
///
/// Each keydown gets its own [`KeydownStamp`], so rapid sequences of commands
/// are measured independently and nothing is shared between them.
#[derive(Debug)]
pub struct PendingKeydown;

impl PendingKeydown {
    /// Stamps a keydown for `kind` at the current instant.
    pub fn start(kind: CommandKind) -> KeydownStamp {
        KeydownStamp {
            kind,
            at: Instant::now(),
        }
    }

    /// Records the elapsed time from `stamp` to now on `surface` and returns it.
    pub fn finish(stamp: KeydownStamp, surface: Surface, book: &mut LatencyBook) -> Duration {
        let elapsed = stamp.at.elapsed();
        book.record(stamp.kind, surface, elapsed);
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn nearest_rank_percentiles_over_one_to_hundred() {
        let mut histogram = LatencyHistogram::new();
        for n in 1..=100 {
            histogram.record(ms(n));
        }
        assert_eq!(histogram.count(), 100);
        assert_eq!(histogram.p50(), Some(ms(50)));
        assert_eq!(histogram.p95(), Some(ms(95)));
        assert_eq!(histogram.p99(), Some(ms(99)));
        assert_eq!(histogram.percentile(0.0), Some(ms(1)));
        assert_eq!(histogram.percentile(100.0), Some(ms(100)));
    }

    #[test]
    fn percentile_of_empty_histogram_is_none() {
        let histogram = LatencyHistogram::new();
        assert_eq!(histogram.p50(), None);
        assert_eq!(histogram.percentile(f64::NAN), None);
    }

    #[test]
    fn nearest_rank_on_small_sample_uses_ceiling() {
        let mut histogram = LatencyHistogram::new();
        for n in [10, 20, 30] {
            histogram.record(ms(n));
        }
        // ceil(0.5 * 3) = 2 -> second-smallest sample.
        assert_eq!(histogram.p50(), Some(ms(20)));
        // ceil(0.95 * 3) = 3 -> largest sample.
        assert_eq!(histogram.p95(), Some(ms(30)));
    }

    #[test]
    fn histogram_is_bounded_and_keeps_most_recent_samples() {
        let mut histogram = LatencyHistogram::with_capacity(3);
        for n in 1..=5 {
            histogram.record(ms(n));
        }
        assert_eq!(histogram.count(), 3);
        assert_eq!(histogram.total_recorded(), 5);
        // Retained samples are {3, 4, 5}.
        assert_eq!(histogram.percentile(0.0), Some(ms(3)));
        assert_eq!(histogram.percentile(100.0), Some(ms(5)));
    }

    #[test]
    fn book_keys_histograms_by_command_and_surface() {
        let mut book = LatencyBook::new();
        book.record(CommandKind::Navigation, Surface::Preview, ms(4));
        book.record(CommandKind::Navigation, Surface::Preview, ms(8));
        book.record(CommandKind::Freeze, Surface::Program, ms(12));

        let preview = book
            .histogram(CommandKind::Navigation, Surface::Preview)
            .expect("recorded pair");
        assert_eq!(preview.count(), 2);
        assert!(
            book.histogram(CommandKind::Navigation, Surface::Program)
                .is_none()
        );
        assert_eq!(book.iter().count(), 2);
    }

    #[test]
    fn pending_keydown_records_elapsed_time_for_its_pair() {
        let mut book = LatencyBook::new();
        let stamp = PendingKeydown::start(CommandKind::Space);
        assert_eq!(stamp.kind(), CommandKind::Space);
        let elapsed = PendingKeydown::finish(stamp, Surface::Program, &mut book);
        let histogram = book
            .histogram(CommandKind::Space, Surface::Program)
            .expect("finish records the pair");
        assert_eq!(histogram.count(), 1);
        assert_eq!(histogram.p50(), Some(elapsed));
    }
}
