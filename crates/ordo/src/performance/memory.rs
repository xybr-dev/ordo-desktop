//! Process-memory measurement helpers shared by the native labs.
//!
//! [`MemoryWatch`] keeps a peak over a run, and [`sample_process`] takes one
//! reading. Both run the sampler on the background executor, because each
//! refresh enumerates OS counters and must never block the GPUI thread.
//! `presenter-lab` and `live-control-lab` use the same code, so their memory
//! documents mean the same thing.

use std::{cell::Cell, cell::RefCell, fmt::Write as _, rc::Rc, time::Duration};

use gpui::App;

use super::process::{PeakMemory, ProcessSample, ProcessSampler};

/// Interval of the background memory sampler that feeds the peak.
pub const MEMORY_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

/// A background memory sampler. It samples every [`MEMORY_SAMPLE_INTERVAL`] on
/// the background executor, and folds each result into a peak on the GPUI
/// thread. It keeps sampling until `finish`.
pub struct MemoryWatch {
    peak: Rc<RefCell<PeakMemory>>,
    stop: Rc<Cell<bool>>,
}

impl MemoryWatch {
    /// Starts the sampling task. It ends when [`MemoryWatch::finish`] runs.
    pub fn start(cx: &mut App) -> Result<Self, String> {
        let mut sampler = ProcessSampler::new().map_err(|error| error.to_string())?;
        let watch = Self {
            peak: Rc::default(),
            stop: Rc::default(),
        };
        let (peak, stop) = (watch.peak.clone(), watch.stop.clone());
        let background = cx.background_executor().clone();
        cx.spawn(async move |_| {
            while !stop.get() {
                // The sampler is not shared: it moves to the background task
                // for one sample and comes back with the result.
                let (returned, sample) = background
                    .spawn(async move {
                        let sample = sampler.sample();
                        (sampler, sample)
                    })
                    .await;
                sampler = returned;
                if let Some(sample) = sample {
                    peak.borrow_mut().observe(sample);
                }
                background.timer(MEMORY_SAMPLE_INTERVAL).await;
            }
        })
        .detach();
        Ok(watch)
    }

    /// The peak so far, folding in `last` if the caller took a sample. Does
    /// not stop the watch.
    pub fn peak_with(&self, last: Option<ProcessSample>) -> PeakMemory {
        if let Some(sample) = last {
            self.peak.borrow_mut().observe(sample);
        }
        *self.peak.borrow()
    }

    /// Stops sampling and returns the peak, folding in `last` if the caller
    /// already took a sample.
    pub fn finish(&self, last: Option<ProcessSample>) -> PeakMemory {
        self.stop.set(true);
        self.peak_with(last)
    }
}

/// One process memory sample, taken on the calling background task. The
/// sampler is created there, because it is not thread-safe to share.
pub fn sample_process() -> Result<ProcessSample, String> {
    let mut sampler =
        ProcessSampler::new().map_err(|error| format!("cannot sample process memory: {error}"))?;
    sampler
        .sample()
        .ok_or_else(|| "process memory was not sampled".to_owned())
}

/// A quoted JSON string. Control characters are escaped, so any message is
/// a valid document.
pub fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            control if control.is_control() => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
