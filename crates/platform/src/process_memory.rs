//! OS process-memory counters that sysinfo does not expose.
//!
//! This is the workspace's single `unsafe` exception (Decision 269): one
//! `GetProcessMemoryInfo` call on Windows, reading
//! `PROCESS_MEMORY_COUNTERS_EX` for the private commit charge and the
//! OS-recorded peaks. `cargo xtask verify` rejects `allow(unsafe_code)`
//! anywhere else. The call is synchronous but cheap; callers still run it on
//! a background executor alongside the rest of process sampling.

/// Counters for the current process at one instant, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessMemory {
    /// Private committed bytes (`PrivateUsage`).
    pub private_bytes: u64,
    /// Highest private commit the OS recorded since process start
    /// (`PeakPagefileUsage`, the same charge as `PrivateUsage`).
    pub peak_private_bytes: u64,
    /// Highest working set the OS recorded since process start
    /// (`PeakWorkingSetSize`).
    pub peak_working_set_bytes: u64,
}

/// Reads the current process's counters. `None` when the OS call fails or
/// the platform has no implementation (every non-Windows target).
pub fn current() -> Option<ProcessMemory> {
    imp::current()
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::ProcessMemory;
    use windows_sys::Win32::System::{
        ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
        },
        Threading::GetCurrentProcess,
    };

    pub(super) fn current() -> Option<ProcessMemory> {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).ok()?;
        counters.cb = size;
        // SAFETY: `GetCurrentProcess` returns a pseudo-handle that is always
        // valid and needs no close. `counters` is a live, initialized
        // `PROCESS_MEMORY_COUNTERS_EX` whose size is passed as `cb`; the API
        // accepts the EX layout through the base pointer type when `cb`
        // matches it, and writes no more than `cb` bytes.
        let ok = unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&raw mut counters).cast::<PROCESS_MEMORY_COUNTERS>(),
                size,
            )
        };
        (ok != 0).then_some(ProcessMemory {
            private_bytes: counters.PrivateUsage as u64,
            peak_private_bytes: counters.PeakPagefileUsage as u64,
            peak_working_set_bytes: counters.PeakWorkingSetSize as u64,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::ProcessMemory;

    pub(super) fn current() -> Option<ProcessMemory> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn reports_private_memory_with_peaks_at_or_above_current() {
        let memory = current().expect("GetProcessMemoryInfo succeeds for this process");
        assert!(memory.private_bytes > 0);
        assert!(memory.peak_private_bytes >= memory.private_bytes);
        assert!(memory.peak_working_set_bytes > 0);
    }

    #[cfg(windows)]
    #[test]
    fn peak_private_survives_a_released_allocation() {
        let before = current().expect("counters").peak_private_bytes;
        let spike = vec![1_u8; 64 * 1024 * 1024];
        std::hint::black_box(&spike);
        drop(spike);
        let after = current().expect("counters");
        assert!(
            after.peak_private_bytes >= before + 60 * 1024 * 1024,
            "peak {} did not cover a 64 MiB spike over {before}",
            after.peak_private_bytes
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn is_unavailable_off_windows() {
        assert_eq!(current(), None);
    }
}
