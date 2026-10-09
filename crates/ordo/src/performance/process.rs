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
    /// Resident set size in bytes (working set on Windows).
    pub rss_bytes: u64,
    /// Virtual memory size in bytes.
    pub virtual_bytes: u64,
    /// Private committed bytes. sysinfo 0.31 does not expose this counter on
    /// Windows, so it is `None` on the pinned stack.
    pub private_bytes: Option<u64>,
    /// Process CPU usage as reported by sysinfo, a delta between refreshes.
    /// `None` on the first sample from a sampler, which has nothing to diff.
    pub cpu_percent: Option<f32>,
}

/// GPU allocation counters.
///
/// The pinned GPUI revision (`gpui-pre` 0.3.8) exposes no allocation or
/// memory counters for its renderer, and sysinfo does not report GPU memory.
/// The report records this as unavailable rather than inventing a number.
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
