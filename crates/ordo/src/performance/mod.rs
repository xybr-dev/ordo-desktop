//! Performance instrumentation primitives for the T0.6 spike (WP2).
//!
//! These types are plain synchronous data. They do not touch GPUI, storage,
//! or the window system, so they can be exercised from tests and from any
//! thread. Production callers keep sampling and aggregation off the GPUI
//! thread (see [`process::ProcessSampler`]). The exception is
//! [`display_path`], which compares the CPU raster and retained GPUI paths.

pub mod bench;
pub mod clock;
pub mod display_path;
pub mod frames;
pub mod hot_path;
pub mod latency;
pub mod live;
pub mod process;
pub mod report;

pub use bench::{BaselineRun, BenchConfig, BenchOutcome, run_presenter_bench, three_run_baselines};
pub use clock::{Enqueued, QueueLatency, StartupClock};
pub use frames::FrameTimer;
pub use hot_path::HotPathKind;
pub use latency::{
    CommandKind, KeydownStamp, LatencyBook, LatencyHistogram, PendingKeydown, Surface,
};
pub use live::{DriveError, PresenterProbe, ProbeOutcome, drive, open as open_probe};
pub use process::{GpuMemory, ProcessSample, ProcessSampler, ProcessSamplerError};
pub use report::{BudgetLimits, BudgetVerdict, BudgetViolation, PerformanceReport, RendererInfo};
