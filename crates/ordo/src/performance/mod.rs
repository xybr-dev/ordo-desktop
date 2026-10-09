//! Performance instrumentation primitives for the T0.6 spike (WP2).
//!
//! These types are plain synchronous data. They do not touch GPUI, storage,
//! or the window system, so they can be exercised from tests and from any
//! thread. Production callers keep sampling and aggregation off the GPUI
//! thread (see [`process::ProcessSampler`]).

pub mod clock;
pub mod frames;
pub mod latency;
pub mod process;
pub mod report;

pub use clock::{Enqueued, QueueLatency, StartupClock};
pub use frames::FrameTimer;
pub use latency::{
    CommandKind, KeydownStamp, LatencyBook, LatencyHistogram, PendingKeydown, Surface,
};
pub use process::{GpuMemory, ProcessSample, ProcessSampler, ProcessSamplerError};
pub use report::{BudgetLimits, BudgetVerdict, BudgetViolation, PerformanceReport, RendererInfo};
