//! Process memory and CPU sampling.
//!
//! [`ProcessSampler`] is plain synchronous code. It refreshes sysinfo's view of
//! this process and reads it back. Each refresh enumerates the process's OS
//! counters, so production callers must run [`ProcessSampler::sample`] on a
//! background executor, never on the GPUI thread. The sampler holds no
//! thread of its own and needs no shutdown.
//!
//! CPU percentage is a delta between two refreshes. The first sample after
//! construction has no previous refresh to diff against, so it reports no CPU
//! value (`None`). A useful later value needs a refresh at least
//! [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`] after the previous one.

use std::fmt;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Memory and CPU for this process at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessSample {
    /// Resident set size in bytes. On Windows this is the current working set
    /// (sysinfo 0.31.4 `src/windows/process.rs:299`, `WorkingSetSize`), not
    /// the peak working set.
    pub rss_bytes: u64,
    /// Virtual memory size in bytes. On Windows this is the address-space size
    /// (`src/windows/process.rs:300`, `SYSTEM_PROCESS_INFORMATION.VirtualSize`),
    /// which is not the private commit charge.
    pub virtual_bytes: u64,
    /// Private committed bytes (`PrivateUsage`). Always `None` on the pinned
    /// stack. sysinfo 0.31.4 maps Windows `virtual_memory()` to
    /// `VirtualSize`, not `PrivateUsage`, so it cannot stand in for this
    /// counter. Reading `PROCESS_MEMORY_COUNTERS_EX` directly needs unsafe
    /// FFI, which the workspace forbids, so no other source is used.
    pub private_bytes: Option<u64>,
    /// Process CPU usage as reported by sysinfo, a delta between refreshes.
    /// `None` on the first sample from a sampler, which has nothing to diff.
    pub cpu_percent: Option<f32>,
}

/// GPU allocation counters.
///
/// `Unavailable` is the only state on the MVP stack, for two reasons. The
/// pinned GPUI revision (`gpui-pre` 0.3.8) exposes no allocation or memory
/// counters for its renderer, and sysinfo does not report GPU memory. The
/// remaining source, DXGI `QueryVideoMemoryInfo`, needs unsafe FFI, which the
/// workspace forbids. The report records this as unavailable rather than
/// inventing a number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuMemory {
    /// No allocation counters are exposed by the pinned GPUI stack.
    #[default]
    Unavailable,
}

/// The current process could not be identified for sampling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessSamplerError {
    reason: String,
}

impl fmt::Display for ProcessSamplerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot identify current process: {}",
            self.reason
        )
    }
}

impl std::error::Error for ProcessSamplerError {}

/// Peak memory over a run of [`ProcessSample`]s. Plain data: the caller
/// decides the sampling cadence and feeds each sample to [`observe`].
///
/// [`observe`]: PeakMemory::observe
///
/// The peak is only as fine as the sampling. sysinfo 0.31.4 exposes no peak
/// working-set accessor (its Windows backend reads only `WorkingSetSize` and
/// `VirtualSize`), and the OS peak counter needs unsafe FFI. Between two
/// samples a short spike can go unseen, so these values are lower bounds on
/// the true peak.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeakMemory {
    /// Largest resident set size seen, in bytes. Zero before any sample.
    pub peak_rss_bytes: u64,
    /// Largest private committed size seen. `None` while no sample has
    /// reported it, which is every sample on the pinned stack.
    pub peak_private_bytes: Option<u64>,
}

impl PeakMemory {
    /// Raises each peak to cover `sample`.
    pub fn observe(&mut self, sample: ProcessSample) {
        self.peak_rss_bytes = self.peak_rss_bytes.max(sample.rss_bytes);
        if let Some(private_bytes) = sample.private_bytes {
            self.peak_private_bytes = Some(
                self.peak_private_bytes
                    .map_or(private_bytes, |peak| peak.max(private_bytes)),
            );
        }
    }
}

/// Samples the current process. Not thread-safe by design; own it on one
/// background task.
pub struct ProcessSampler {
    system: System,
    pid: Pid,
    /// Whether an earlier sample already refreshed the process, so the CPU
    /// counter has a previous reading to diff against.
    refreshed: bool,
}

impl ProcessSampler {
    /// Identifies the current process. Does not sample yet.
    pub fn new() -> Result<Self, ProcessSamplerError> {
        let pid = sysinfo::get_current_pid().map_err(|reason| ProcessSamplerError {
            reason: reason.to_string(),
        })?;
        Ok(Self {
            system: System::new(),
            pid,
            refreshed: false,
        })
    }

    /// Refreshes this process and returns its current counters.
    ///
    /// Returns `None` if the OS no longer reports the process.
    pub fn sample(&mut self) -> Option<ProcessSample> {
        let had_previous_refresh = std::mem::replace(&mut self.refreshed, true);
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            ProcessRefreshKind::new().with_memory().with_cpu(),
        );
        let process = self.system.process(self.pid)?;
        Some(ProcessSample {
            rss_bytes: process.memory(),
            virtual_bytes: process.virtual_memory(),
            // Not available on the pinned stack; see `ProcessSample::private_bytes`.
            private_bytes: None,
            cpu_percent: had_previous_refresh.then(|| process.cpu_usage()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_reports_nonzero_resident_memory_for_this_process() {
        let mut sampler = ProcessSampler::new().expect("current process is identifiable");
        let sample = sampler.sample().expect("current process is reported");
        assert!(sample.rss_bytes > 0, "rss was {}", sample.rss_bytes);
        assert!(sample.virtual_bytes >= sample.rss_bytes);
        assert_eq!(sample.private_bytes, None);
        assert_eq!(sample.cpu_percent, None, "first sample has no CPU delta");
    }

    #[test]
    fn peak_tracks_the_largest_rss_and_keeps_private_unset() {
        let sample = |rss_bytes| ProcessSample {
            rss_bytes,
            virtual_bytes: rss_bytes,
            private_bytes: None,
            cpu_percent: None,
        };
        let mut peak = PeakMemory::default();
        assert_eq!(peak.peak_rss_bytes, 0);
        peak.observe(sample(300));
        peak.observe(sample(900));
        peak.observe(sample(500));
        assert_eq!(peak.peak_rss_bytes, 900);
        assert_eq!(peak.peak_private_bytes, None);
    }

    #[test]
    fn peak_tracks_the_largest_private_bytes_once_reported() {
        let sample = |rss_bytes, private_bytes| ProcessSample {
            rss_bytes,
            virtual_bytes: rss_bytes,
            private_bytes,
            cpu_percent: None,
        };
        let mut peak = PeakMemory::default();
        peak.observe(sample(100, None));
        assert_eq!(peak.peak_private_bytes, None);
        peak.observe(sample(80, Some(400)));
        peak.observe(sample(120, Some(250)));
        peak.observe(sample(90, None));
        assert_eq!(peak.peak_private_bytes, Some(400));
        assert_eq!(peak.peak_rss_bytes, 120);
    }

    #[test]
    fn later_samples_report_a_cpu_value() {
        let mut sampler = ProcessSampler::new().expect("current process is identifiable");
        sampler.sample().expect("current process is reported");
        let sample = sampler.sample().expect("current process is reported");
        assert!(
            sample.cpu_percent.is_some_and(f32::is_finite),
            "cpu was {:?}",
            sample.cpu_percent
        );
    }
}
