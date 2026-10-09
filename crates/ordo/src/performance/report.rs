//! Aggregated performance report, its JSON and summary renderings, and the
//! T0.6 budget check.
//!
//! The report owns plain snapshots of the instrumentation. Callers assemble it
//! off the GPUI thread and hand it to the Diagnostics panel or to the
//! measurement harness.

use std::{fmt::Write as _, time::Duration};

use super::{
    frames::FrameTimer,
    latency::{CommandKind, LatencyBook, LatencyHistogram, Surface},
    process::{GpuMemory, ProcessSample},
};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Schema tag written into every JSON report.
pub const REPORT_SCHEMA: &str = "ordo.performance.v1";

/// Renderer identity as the platform reports it. Both fields are `None` when
/// the platform does not expose them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RendererInfo {
    /// `Some(true)` if the adapter is a software emulation (no hardware
    /// acceleration), `Some(false)` if it is hardware, `None` if unknown.
    pub software_emulated: Option<bool>,
    pub device_name: Option<String>,
}

/// The percentiles printed for every histogram: p50, p95, and p99.
const TAIL_PERCENTILES: [f64; 3] = [50.0, 95.0, 99.0];

/// Everything the spike records, in one snapshot.
#[derive(Clone, Debug)]
pub struct PerformanceReport {
    pub latency: LatencyBook,
    /// What `latency` measures. The bench records engine apply time only, not
    /// keydown-to-paint, and the JSON and summary say so.
    pub latency_scope: &'static str,
    pub frames: FrameTimer,
    /// Process start to first operator paint.
    pub time_to_first_paint: Option<Duration>,
    /// Enqueue-to-dequeue latency for the worker queue.
    pub worker_queue: LatencyHistogram,
    /// Memory and CPU sampled from a background task.
    pub process: Option<ProcessSample>,
    /// CPU usage sampled while the application is idle.
    pub idle_cpu_percent: Option<f32>,
    pub gpu_memory: GpuMemory,
    pub renderer: RendererInfo,
}

impl Default for PerformanceReport {
    fn default() -> Self {
        Self {
            latency: LatencyBook::default(),
            latency_scope: "engine_apply",
            frames: FrameTimer::default(),
            time_to_first_paint: None,
            worker_queue: LatencyHistogram::default(),
            process: None,
            idle_cpu_percent: None,
            gpu_memory: GpuMemory::default(),
            renderer: RendererInfo::default(),
        }
    }
}

/// Limits enforced by [`PerformanceReport::budget_check`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetLimits {
    /// Every command and surface p95 must be strictly below this.
    pub p95_limit: Duration,
    /// Process memory above this fails the qualification budget.
    pub qualification_bytes: u64,
    /// Process memory above this fails the absolute release ceiling.
    pub ceiling_bytes: u64,
}

impl Default for BudgetLimits {
    /// The adopted T0.6 limits: p95 below 50 ms, 512 MiB qualification,
    /// 1 GiB absolute ceiling.
    fn default() -> Self {
        Self {
            p95_limit: Duration::from_millis(50),
            qualification_bytes: 512 * MIB,
            ceiling_bytes: GIB,
        }
    }
}

/// One way a report misses the budget.
#[derive(Clone, Debug, PartialEq)]
pub enum BudgetViolation {
    /// No samples were recorded for this command and surface.
    LatencyUnmeasured {
        command: CommandKind,
        surface: Surface,
    },
    /// The p95 is at or above the limit.
    P95Exceeded {
        command: CommandKind,
        surface: Surface,
        p95: Duration,
    },
    /// No process memory sample was available.
    MemoryUnmeasured,
    /// Process memory is above the qualification budget.
    MemoryOverQualification { bytes: u64 },
    /// Process memory is above the absolute ceiling.
    MemoryOverCeiling { bytes: u64 },
}

/// Typed result of a budget check.
#[derive(Clone, Debug, PartialEq)]
pub enum BudgetVerdict {
    Pass,
    Fail(Vec<BudgetViolation>),
}

impl BudgetVerdict {
    pub fn is_pass(&self) -> bool {
        matches!(self, BudgetVerdict::Pass)
    }
}

impl PerformanceReport {
    /// An empty report for the given renderer.
    pub fn new(renderer: RendererInfo) -> Self {
        Self {
            renderer,
            ..Self::default()
        }
    }

    /// Checks the report against `limits`.
    ///
    /// Every (command, surface) pair must have samples and a p95 below
    /// `limits.p95_limit`. Memory uses private bytes when the platform reports
    /// them, and RSS otherwise. A missing memory sample fails, because an
    /// unmeasured budget is not a passing one.
    pub fn budget_check(&self, limits: BudgetLimits) -> BudgetVerdict {
        let mut violations = Vec::new();

        for command in CommandKind::ALL {
            for surface in Surface::ALL {
                match self
                    .latency
                    .histogram(command, surface)
                    .and_then(|h| if h.count() == 0 { None } else { h.p95() })
                {
                    None => {
                        violations.push(BudgetViolation::LatencyUnmeasured { command, surface })
                    }
                    Some(p95) if p95 >= limits.p95_limit => {
                        violations.push(BudgetViolation::P95Exceeded {
                            command,
                            surface,
                            p95,
                        })
                    }
                    Some(_) => {}
                }
            }
        }

        match self.process {
            None => violations.push(BudgetViolation::MemoryUnmeasured),
            Some(sample) => {
                let bytes = sample.private_bytes.unwrap_or(sample.rss_bytes);
                if bytes > limits.ceiling_bytes {
                    violations.push(BudgetViolation::MemoryOverCeiling { bytes });
                }
                if bytes > limits.qualification_bytes {
                    violations.push(BudgetViolation::MemoryOverQualification { bytes });
                }
            }
        }

        if violations.is_empty() {
            BudgetVerdict::Pass
        } else {
            BudgetVerdict::Fail(violations)
        }
    }

    /// Serializes the report as compact JSON. Written by hand so the report
    /// has no serialization dependency.
    ///
    /// Durations are milliseconds with three decimals. Unavailable values are
    /// `null`. The latency array always lists all eight command/surface pairs.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push('{');
        let _ = write!(out, "\"schema\":{}", json_string(REPORT_SCHEMA));

        out.push_str(",\"renderer\":{");
        let _ = write!(
            out,
            "\"software_emulated\":{},\"device_name\":{}",
            json_opt_bool(self.renderer.software_emulated),
            json_opt_string(self.renderer.device_name.as_deref()),
        );
        out.push('}');

        out.push_str(",\"latency\":[");
        let mut first = true;
        for command in CommandKind::ALL {
            for surface in Surface::ALL {
                if !first {
                    out.push(',');
                }
                first = false;
                let histogram = self.latency.histogram(command, surface);
                let (samples, [p50, p95, p99]) = match histogram {
                    Some(h) => (h.count(), tail(h.percentiles(&TAIL_PERCENTILES))),
                    None => (0, [None; 3]),
                };
                let _ = write!(
                    out,
                    "{{\"command\":{},\"surface\":{},\"samples\":{},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{}}}",
                    json_string(command.as_str()),
                    json_string(surface.as_str()),
                    samples,
                    json_ms(p50),
                    json_ms(p95),
                    json_ms(p99),
                );
            }
        }
        out.push(']');

        let _ = write!(
            out,
            ",\"latency_scope\":{}",
            json_string(self.latency_scope)
        );

        let [frame_p50, frame_p95, frame_p99] = tail(self.frames.percentiles(&TAIL_PERCENTILES));
        let _ = write!(
            out,
            ",\"frames\":{{\"count\":{},\"expected_ms\":{},\"drop_threshold_ms\":{},\"dropped\":{},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{}}}",
            self.frames.frame_count(),
            json_ms(Some(self.frames.expected_interval())),
            json_ms(Some(self.frames.drop_threshold())),
            self.frames.dropped_frames(),
            json_ms(frame_p50),
            json_ms(frame_p95),
            json_ms(frame_p99),
        );

        let _ = write!(
            out,
            ",\"startup\":{{\"time_to_first_paint_ms\":{}}}",
            json_ms(self.time_to_first_paint),
        );

        let [queue_p50, queue_p95, queue_p99] =
            tail(self.worker_queue.percentiles(&TAIL_PERCENTILES));
        let _ = write!(
            out,
            ",\"worker_queue\":{{\"samples\":{},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{}}}",
            self.worker_queue.count(),
            json_ms(queue_p50),
            json_ms(queue_p95),
            json_ms(queue_p99),
        );

        let (rss, virtual_bytes, private, cpu) = match self.process {
            Some(s) => (
                json_u64(Some(s.rss_bytes)),
                json_u64(Some(s.virtual_bytes)),
                json_u64(s.private_bytes),
                json_f32(s.cpu_percent),
            ),
            None => ("null".into(), "null".into(), "null".into(), "null".into()),
        };
        let _ = write!(
            out,
            ",\"memory\":{{\"rss_bytes\":{rss},\"virtual_bytes\":{virtual_bytes},\"private_bytes\":{private}}}\
             ,\"cpu\":{{\"process_percent\":{cpu},\"idle_percent\":{}}}",
            json_f32(self.idle_cpu_percent),
        );

        let _ = write!(
            out,
            ",\"gpu_memory\":{}",
            match self.gpu_memory {
                GpuMemory::Unavailable => "\"unavailable\"",
            }
        );

        out.push('}');
        out
    }

    /// Human-readable lines for the Diagnostics panel.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(format!(
            "Renderer: {}, device {}",
            match self.renderer.software_emulated {
                Some(true) => "software emulated",
                Some(false) => "hardware accelerated",
                None => "acceleration unknown",
            },
            self.renderer.device_name.as_deref().unwrap_or("unknown"),
        ));

        lines.push("Engine apply latency (not keydown→paint)".to_string());
        for command in CommandKind::ALL {
            for surface in Surface::ALL {
                let line = match self.latency.histogram(command, surface) {
                    Some(h) if h.count() > 0 => {
                        let [p50, p95, p99] = tail(h.percentiles(&TAIL_PERCENTILES));
                        format!(
                            "{} to {}: {} samples, p50 {}, p95 {}, p99 {}",
                            command.as_str(),
                            surface.as_str(),
                            h.count(),
                            fmt_ms(p50),
                            fmt_ms(p95),
                            fmt_ms(p99),
                        )
                    }
                    _ => format!("{} to {}: no samples", command.as_str(), surface.as_str()),
                };
                lines.push(line);
            }
        }

        let [frame_p50, frame_p95, frame_p99] = tail(self.frames.percentiles(&TAIL_PERCENTILES));
        lines.push(format!(
            "Frames: {} presented, p50 {}, p95 {}, p99 {}, {} dropped (over {})",
            self.frames.frame_count(),
            fmt_ms(frame_p50),
            fmt_ms(frame_p95),
            fmt_ms(frame_p99),
            self.frames.dropped_frames(),
            fmt_ms(Some(self.frames.drop_threshold())),
        ));

        lines.push(match self.time_to_first_paint {
            Some(t) => format!("Startup: first operator paint at {}", fmt_ms(Some(t))),
            None => "Startup: first operator paint not measured".to_string(),
        });

        let [queue_p50, queue_p95, queue_p99] =
            tail(self.worker_queue.percentiles(&TAIL_PERCENTILES));
        lines.push(format!(
            "Worker queue: {} samples, p50 {}, p95 {}, p99 {}",
            self.worker_queue.count(),
            fmt_ms(queue_p50),
            fmt_ms(queue_p95),
            fmt_ms(queue_p99),
        ));

        lines.push(match self.process {
            Some(s) => format!(
                "Memory: RSS {}, virtual {}, private {}",
                fmt_mib(s.rss_bytes),
                fmt_mib(s.virtual_bytes),
                s.private_bytes
                    .map(fmt_mib)
                    .unwrap_or_else(|| "unavailable".to_string()),
            ),
            None => "Memory: not sampled".to_string(),
        });

        lines.push(format!(
            "CPU: process {}, idle {}",
            match self.process {
                Some(ProcessSample {
                    cpu_percent: Some(cpu),
                    ..
                }) => format!("{cpu:.1}%"),
                Some(_) => "n/a".to_string(),
                None => "not sampled".to_string(),
            },
            self.idle_cpu_percent
                .map(|c| format!("{c:.1}%"))
                .unwrap_or_else(|| "not sampled".to_string()),
        ));

        lines.push(match self.gpu_memory {
            GpuMemory::Unavailable => {
                "GPU memory: unavailable (pinned GPUI exposes no allocation counters)".to_string()
            }
        });

        lines
    }
}

/// Unpacks the output of [`LatencyHistogram::percentiles`] called with
/// [`TAIL_PERCENTILES`], which always returns one entry per requested percentile.
fn tail(values: Vec<Option<Duration>>) -> [Option<Duration>; 3] {
    [values[0], values[1], values[2]]
}

fn fmt_ms(value: Option<Duration>) -> String {
    match value {
        Some(d) => format!("{:.2} ms", d.as_secs_f64() * 1000.0),
        None => "n/a".to_string(),
    }
}

fn fmt_mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / MIB as f64)
}

fn json_ms(value: Option<Duration>) -> String {
    match value {
        Some(d) => format!("{:.3}", d.as_secs_f64() * 1000.0),
        None => "null".to_string(),
    }
}

fn json_f32(value: Option<f32>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("{v:.2}"),
        _ => "null".to_string(),
    }
}

fn json_u64(value: Option<u64>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "null".to_string(),
    }
}

fn json_opt_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
}

fn json_opt_string(value: Option<&str>) -> String {
    match value {
        Some(s) => json_string(s),
        None => "null".to_string(),
    }
}

/// Quotes and escapes `value` as a JSON string.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn sample(rss: u64, private: Option<u64>) -> ProcessSample {
        ProcessSample {
            rss_bytes: rss,
            virtual_bytes: rss * 2,
            private_bytes: private,
            cpu_percent: Some(1.5),
        }
    }

    /// A report with every pair measured at `p95_ms` and the given memory.
    fn passing_report(latency_ms: u64, memory: ProcessSample) -> PerformanceReport {
        let mut report = PerformanceReport::new(RendererInfo {
            software_emulated: Some(false),
            device_name: Some("GPU \"A\"".to_string()),
        });
        for command in CommandKind::ALL {
            for surface in Surface::ALL {
                report.latency.record(command, surface, ms(latency_ms));
            }
        }
        report.process = Some(memory);
        report
    }

    #[test]
    fn budget_passes_when_all_pairs_and_memory_are_within_limits() {
        let report = passing_report(10, sample(300 * MIB, None));
        assert_eq!(
            report.budget_check(BudgetLimits::default()),
            BudgetVerdict::Pass
        );
    }

    #[test]
    fn budget_p95_equal_to_limit_fails() {
        let report = passing_report(50, sample(300 * MIB, None));
        let verdict = report.budget_check(BudgetLimits::default());
        let BudgetVerdict::Fail(violations) = verdict else {
            panic!("expected failure");
        };
        assert_eq!(violations.len(), 8);
        assert!(matches!(
            violations[0],
            BudgetViolation::P95Exceeded { p95, .. } if p95 == ms(50)
        ));
    }

    #[test]
    fn budget_reports_unmeasured_pairs_and_memory() {
        let report = PerformanceReport::new(RendererInfo::default());
        let BudgetVerdict::Fail(violations) = report.budget_check(BudgetLimits::default()) else {
            panic!("empty report must fail");
        };
        assert_eq!(
            violations
                .iter()
                .filter(|v| matches!(v, BudgetViolation::LatencyUnmeasured { .. }))
                .count(),
            8
        );
        assert!(violations.contains(&BudgetViolation::MemoryUnmeasured));
    }

    #[test]
    fn budget_memory_over_qualification_but_under_ceiling_fails_qualification_only() {
        let report = passing_report(10, sample(600 * MIB, None));
        let BudgetVerdict::Fail(violations) = report.budget_check(BudgetLimits::default()) else {
            panic!("expected failure");
        };
        assert_eq!(
            violations,
            vec![BudgetViolation::MemoryOverQualification { bytes: 600 * MIB }]
        );
    }

    #[test]
    fn budget_memory_over_ceiling_fails_both_gates() {
        let report = passing_report(10, sample(GIB + 1, None));
        let BudgetVerdict::Fail(violations) = report.budget_check(BudgetLimits::default()) else {
            panic!("expected failure");
        };
        assert!(violations.contains(&BudgetViolation::MemoryOverCeiling { bytes: GIB + 1 }));
        assert!(violations.contains(&BudgetViolation::MemoryOverQualification { bytes: GIB + 1 }));
    }

    #[test]
    fn budget_prefers_private_bytes_when_available() {
        let report = passing_report(10, sample(900 * MIB, Some(100 * MIB)));
        assert!(report.budget_check(BudgetLimits::default()).is_pass());
    }

    #[test]
    fn json_has_expected_shape_and_escapes_strings() {
        let mut report = passing_report(10, sample(300 * MIB, None));
        report.time_to_first_paint = Some(ms(640));
        report.idle_cpu_percent = Some(0.25);
        report.frames.record_frame(std::time::Instant::now());
        let json = report.to_json();

        assert!(json.starts_with('{') && json.ends_with('}'));
        assert_eq!(json.matches('{').count(), json.matches('}').count());
        assert_eq!(json.matches('[').count(), json.matches(']').count());
        assert!(json.contains("\"schema\":\"ordo.performance.v1\""));
        assert!(json.contains("\"device_name\":\"GPU \\\"A\\\"\""));
        assert!(json.contains("\"software_emulated\":false"));
        assert_eq!(json.matches("\"command\":").count(), 8);
        assert!(json.contains(
            "{\"command\":\"navigation\",\"surface\":\"preview\",\"samples\":1,\"p50_ms\":10.000,\"p95_ms\":10.000,\"p99_ms\":10.000}"
        ));
        assert!(json.contains("\"time_to_first_paint_ms\":640.000"));
        assert!(json.contains("\"dropped\":0"));
        assert!(json.contains("\"private_bytes\":null"));
        assert!(json.contains("\"idle_percent\":0.25"));
        assert!(json.contains("\"gpu_memory\":\"unavailable\""));
        assert!(json.contains("\"latency_scope\":\"engine_apply\""));
        assert!(json.contains("\"process_percent\":1.50"));
    }

    #[test]
    fn json_uses_null_for_unmeasured_values() {
        let json = PerformanceReport::default().to_json();
        assert!(json.contains("\"device_name\":null"));
        assert!(json.contains("\"software_emulated\":null"));
        assert!(json.contains("\"time_to_first_paint_ms\":null"));
        assert!(json.contains("\"rss_bytes\":null"));
        assert!(json.contains("\"latency_scope\":\"engine_apply\""));
        assert!(json.contains(
            "{\"command\":\"space\",\"surface\":\"program\",\"samples\":0,\"p50_ms\":null,\"p95_ms\":null,\"p99_ms\":null}"
        ));
    }

    #[test]
    fn json_escapes_control_characters() {
        assert_eq!(json_string("a\u{1}b\n"), "\"a\\u0001b\\n\"");
    }

    #[test]
    fn summary_lines_cover_every_section() {
        let mut report = passing_report(10, sample(300 * MIB, None));
        report.time_to_first_paint = Some(ms(640));
        let lines = report.summary_lines();
        assert_eq!(lines.len(), 1 + 1 + 8 + 6);
        assert!(lines[0].contains("hardware accelerated"));
        assert_eq!(lines[1], "Engine apply latency (not keydown→paint)");
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("navigation to preview: 1 samples"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("first operator paint at 640.00 ms"))
        );
        assert!(lines.iter().any(|l| l.contains("RSS 300.0 MiB")));
        assert!(lines.last().is_some_and(|l| l.contains("unavailable")));
    }

    #[test]
    fn cpu_before_a_delta_is_null_in_json_and_n_a_in_summary() {
        let mut report = passing_report(10, sample(300 * MIB, None));
        report.process = Some(ProcessSample {
            cpu_percent: None,
            ..sample(300 * MIB, None)
        });
        assert!(report.to_json().contains("\"process_percent\":null"));
        assert!(
            report
                .summary_lines()
                .iter()
                .any(|l| l.starts_with("CPU: process n/a,"))
        );
    }
}
