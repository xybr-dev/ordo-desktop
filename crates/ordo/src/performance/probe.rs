//! Hot-path guard for the Presenter command path (T0.6 WP4).
//!
//! The Presenter hot path is keydown → Presentation Engine → one paint. It must
//! never send to a worker, touch the filesystem, or wait on storage. Production
//! code that does those things calls a probe method at the point of the send,
//! and the bench asserts the counts stay zero across the measured path.
//!
//! The counters are thread-local: a measurement sees only work done on its own
//! thread, so concurrent tests in one binary cannot disturb each other. No
//! synchronization, allocation, or lock is involved in recording a hit.

use std::cell::Cell;

/// Counts of hot-path-forbidden operations observed on the current thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotPathCounts {
    /// Messages sent to the preferences storage worker.
    pub preference_sends: u64,
    /// Messages sent to the canonical composition worker.
    pub canonical_sends: u64,
}

impl HotPathCounts {
    /// Whether no forbidden operation was observed.
    pub const fn is_clean(self) -> bool {
        self.preference_sends == 0 && self.canonical_sends == 0
    }
}

thread_local! {
    static PREFERENCE_SENDS: Cell<u64> = const { Cell::new(0) };
    static CANONICAL_SENDS: Cell<u64> = const { Cell::new(0) };
}

/// The hot-path probe. Stateless: all state is the thread-local counters.
#[derive(Debug)]
pub struct HotPathProbe;

impl HotPathProbe {
    /// Record one send to the preferences storage worker.
    pub fn record_preference_send() {
        PREFERENCE_SENDS.with(|count| count.set(count.get().saturating_add(1)));
    }

    /// Record one send to the canonical composition worker.
    pub fn record_canonical_send() {
        CANONICAL_SENDS.with(|count| count.set(count.get().saturating_add(1)));
    }

    /// The counts observed on the current thread since the last [`Self::reset`].
    pub fn counts() -> HotPathCounts {
        HotPathCounts {
            preference_sends: PREFERENCE_SENDS.with(Cell::get),
            canonical_sends: CANONICAL_SENDS.with(Cell::get),
        }
    }

    /// Clear the current thread's counters.
    pub fn reset() {
        PREFERENCE_SENDS.with(|count| count.set(0));
        CANONICAL_SENDS.with(|count| count.set(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_clean_and_reset_to_clean() {
        HotPathProbe::reset();
        assert!(HotPathProbe::counts().is_clean());
        HotPathProbe::record_preference_send();
        HotPathProbe::record_canonical_send();
        assert!(!HotPathProbe::counts().is_clean());
        HotPathProbe::reset();
        assert_eq!(HotPathProbe::counts(), HotPathCounts::default());
    }

    #[test]
    fn each_site_counts_only_its_own_kind() {
        HotPathProbe::reset();
        HotPathProbe::record_preference_send();
        HotPathProbe::record_preference_send();
        assert_eq!(
            HotPathProbe::counts(),
            HotPathCounts {
                preference_sends: 2,
                canonical_sends: 0,
            }
        );
        HotPathProbe::reset();
    }

    #[test]
    fn counters_are_per_thread() {
        HotPathProbe::reset();
        std::thread::spawn(|| {
            HotPathProbe::record_canonical_send();
            assert_eq!(HotPathProbe::counts().canonical_sends, 1);
        })
        .join()
        .unwrap();
        assert!(HotPathProbe::counts().is_clean());
    }
}
