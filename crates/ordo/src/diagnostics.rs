//! The Diagnostics destination's Performance panel (T0.6 WP4).
//!
//! The panel is a screen of the Diagnostics destination, so the shell owns its
//! state and asks this module to draw it. It shows renderer acceleration,
//! startup, current memory, the latency table from the last bench run, and a
//! button that runs the Presenter bench on a background executor.
//!
//! Nothing here blocks the GPUI thread: memory sampling and the bench run as
//! background tasks, and the shell stores their results when they land.

use std::time::Duration;

use gpui::{
    App, GpuSpecs, IntoElement, ParentElement, SharedString, Styled, Task, Window, div, prelude::*,
    px, rgb,
};
use ui::{environment::UiEnvironment, primitives::button::Button, tokens};

use crate::performance::{
    BenchOutcome, StartupClock,
    latency::{CommandKind, Surface},
};

/// The bench's state as the panel shows it.
#[derive(Debug, Default)]
pub enum BenchState {
    #[default]
    Idle,
    Running,
    Done(Box<BenchOutcome>),
}

/// Everything the Performance panel shows that the shell must keep between
/// renders.
#[derive(Debug, Default)]
pub struct PerformanceState {
    /// Process start to the first operator render.
    pub startup: StartupClock,
    /// Resident set size of this process at the last memory sample.
    pub rss_bytes: Option<u64>,
    pub bench: BenchState,
    /// The window's GPU adapter, read once the first time Diagnostics is shown.
    pub gpu_specs: Option<GpuSpecs>,
    /// Whether `gpu_specs` has been read. An unknown adapter is `None` and
    /// must not be re-read on every render.
    pub gpu_specs_read: bool,
    /// The memory sample in flight, if any. Dropping this cancels the sample.
    pub memory_task: Option<Task<()>>,
    /// The bench run in flight or finished, if any. Dropping this cancels a
    /// run that has not finished.
    pub bench_task: Option<Task<()>>,
}

/// The renderer line: whether the window's GPU adapter is hardware or a
/// software fallback, or unknown when GPUI reports nothing.
pub fn renderer_status(specs: Option<&GpuSpecs>) -> String {
    match specs {
        None => "unknown".to_owned(),
        Some(specs) if specs.is_software_emulated => {
            format!(
                "SOFTWARE RENDERING \u{2014} fallback detected ({})",
                specs.device_name
            )
        }
        Some(specs) => format!("Hardware acceleration: active ({})", specs.device_name),
    }
}

/// Whether the renderer line reports a software fallback.
pub fn is_software_fallback(specs: Option<&GpuSpecs>) -> bool {
    specs.is_some_and(|specs| specs.is_software_emulated)
}

fn format_ms(value: Option<Duration>) -> String {
    match value {
        Some(value) => format!("{:.2}", value.as_secs_f64() * 1000.0),
        None => "\u{2014}".to_owned(),
    }
}

fn format_mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// The Performance panel. `on_run` starts a bench run; the shell decides
/// whether one is already in flight.
pub fn panel(
    state: &PerformanceState,
    environment: UiEnvironment,
    on_run: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let scale = environment.preferences().scale;
    let palette = tokens::palette(environment.appearance());
    let gap = scale.apply(tokens::spacing::SECTION);
    let related = scale.apply(tokens::spacing::RELATED);
    let label_size = scale.apply(tokens::typography::LABEL.size);
    let body_size = scale.apply(tokens::typography::BODY.size);
    let renderer = state.gpu_specs.as_ref();

    let heading = |text: &'static str| {
        div()
            .text_size(px(scale.apply(tokens::typography::TITLE.size)))
            .child(text)
    };
    let line = |text: SharedString, color: u32| {
        div()
            .text_size(px(body_size))
            .text_color(rgb(color))
            .child(text)
    };

    let renderer_color = if is_software_fallback(renderer) {
        palette.warning
    } else {
        palette.text
    };
    let rss = match state.rss_bytes {
        Some(bytes) => format_mib(bytes),
        None => "not sampled yet".to_owned(),
    };
    let startup = match state.startup.time_to_first_paint() {
        Some(elapsed) => format!(
            "{} ms from process start to first render",
            format_ms(Some(elapsed))
        ),
        None => "not yet rendered".to_owned(),
    };

    let outcome = match &state.bench {
        BenchState::Done(outcome) => Some(outcome.as_ref()),
        BenchState::Idle | BenchState::Running => None,
    };
    let latency = outcome.map(|outcome| &outcome.report.latency);

    let header_cells = [
        "Operator action",
        "Preview p50",
        "p95",
        "p99",
        "Program p50",
        "p95",
        "p99",
    ];
    let header = div()
        .flex()
        .gap(px(related))
        .text_size(px(label_size))
        .text_color(rgb(palette.text_muted))
        .children(header_cells.iter().enumerate().map(|(index, cell)| {
            column(index, cell.to_string(), rgb(palette.text_muted), label_size)
        }));
    let rows = CommandKind::ALL.map(|command| {
        let mut cells = vec![command.as_str().to_owned()];
        for surface in Surface::ALL {
            let histogram = latency.and_then(|book| book.histogram(command, surface));
            // One sort per histogram: p50, p95, p99 in that order.
            let values = histogram
                .map(|h| h.percentiles(&[50.0, 95.0, 99.0]))
                .unwrap_or_default();
            for index in 0..3 {
                cells.push(format_ms(values.get(index).copied().flatten()));
            }
        }
        div()
            .flex()
            .gap(px(related))
            .text_size(px(body_size))
            .children(
                cells
                    .into_iter()
                    .enumerate()
                    .map(|(index, cell)| column(index, cell, rgb(palette.text), body_size)),
            )
    });

    let bench_status = match &state.bench {
        BenchState::Idle => "No bench run yet. The bench drives the Presenter engine with 2,000 mixed commands over a 500-Cue deck.".to_owned(),
        BenchState::Running => "Bench running on a background thread\u{2026}".to_owned(),
        BenchState::Done(_) => String::new(),
    };

    let mut result = div()
        .debug_selector(|| "bench-result".into())
        .flex()
        .flex_col()
        .gap(px(related));
    if let Some(outcome) = outcome {
        for summary in outcome.detail_lines() {
            result = result.child(line(summary.into(), palette.text));
        }
        let verdict_color = if outcome.verdict.is_pass() {
            palette.text
        } else {
            palette.warning
        };
        result = result.child(div().debug_selector(|| "bench-verdict".into()).child(line(
            crate::performance::bench::verdict_line(&outcome.verdict).into(),
            verdict_color,
        )));
    }

    div()
        .debug_selector(|| "diagnostics-panel".into())
        .flex()
        .flex_col()
        .gap(px(gap))
        .child(heading("Performance"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(related))
                .child(
                    div()
                        .debug_selector(|| "renderer-status".into())
                        .child(line(
                            format!("Renderer: {}", renderer_status(renderer)).into(),
                            renderer_color,
                        )),
                )
                .child(line(format!("Startup: {startup}").into(), palette.text))
                .child(
                    div()
                        .debug_selector(|| "rss".into())
                        .child(line(format!("Current RSS: {rss}").into(), palette.text)),
                ),
        )
        .child(
            div()
                .debug_selector(|| "latency-table".into())
                .flex()
                .flex_col()
                .gap(px(related))
                .child(header)
                .children(rows),
        )
        .child(line(
            "Display-path comparison: run by the display-path test harness (cargo test -p ordo display_path). It is not measured in this window.".into(),
            palette.text_muted,
        ))
        .child(
            Button::new("run-presenter-bench", "Run Presenter bench", environment)
                .on_surface(palette.background)
                .on_activate(on_run),
        )
        .child(line(bench_status.into(), palette.text_muted))
        .child(result)
}

/// One fixed-width-ish table column, sized to its content so the header and
/// every row line up without a shared grid.
fn column(index: usize, text: String, color: gpui::Rgba, size: f32) -> impl IntoElement {
    let width = if index == 0 { 160.0 } else { 88.0 };
    div()
        .flex_none()
        .w(px(width))
        .text_size(px(size))
        .text_color(color)
        .child(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::bench::{BenchConfig, run_presenter_bench};
    use gpui::GpuSpecs;

    fn specs(software: bool) -> GpuSpecs {
        GpuSpecs {
            is_software_emulated: software,
            device_name: "Test Adapter".to_owned(),
            driver_name: String::new(),
            driver_info: String::new(),
        }
    }

    #[test]
    fn the_renderer_line_names_hardware_software_and_unknown() {
        assert_eq!(renderer_status(None), "unknown");
        assert_eq!(
            renderer_status(Some(&specs(false))),
            "Hardware acceleration: active (Test Adapter)"
        );
        let software = renderer_status(Some(&specs(true)));
        assert!(software.starts_with("SOFTWARE RENDERING"), "{software}");
        assert!(software.contains("fallback detected"));
        assert!(!is_software_fallback(None));
        assert!(!is_software_fallback(Some(&specs(false))));
        assert!(is_software_fallback(Some(&specs(true))));
    }

    #[test]
    fn a_done_bench_outcome_has_latency_and_verdict_lines() {
        let outcome = run_presenter_bench(BenchConfig {
            cues: 20,
            commands: 200,
            seed: 3,
        });
        let lines = outcome.summary_lines();
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Oracle match: yes"))
        );
        assert!(lines.iter().any(|line| line.starts_with("Budget:")));
    }
}
