//! Packaged canonical composition (T0.4 PR 8).
//!
//! The worker-owned production boundary from live runtime state to the
//! canonical renderer. Composition runs on the `ordo-canonical` worker
//! thread — rendering never blocks the GPUI thread — behind a request
//! channel the runtime owns: callers snapshot operator state, send
//! [`CanonicalCommand::Compose`], and receive the serialized logical frame.
//! Neither the operator Interface scale nor the window display scale may
//! enter composition; both cross this boundary on every request, so any
//! future use of either value fails the packaged invariance gate naming
//! its axis. PR 9's viewport consumers compose real slide content through
//! this same worker.

use std::sync::mpsc::{Receiver, Sender};

use platform::tasks::{CancellationToken, WorkerExit};
use ui::environment::UiEnvironment;

/// One canonical composition request, or the stop that ends the worker loop
/// (mirroring the preferences worker's shutdown message, so `recv` unblocks
/// deterministically at shutdown rather than hanging the join).
pub enum CanonicalCommand {
    Compose {
        environment: UiEnvironment,
        window_scale: f32,
        content: rendering::TemplateContent,
        reply: Sender<Result<String, rendering::RenderError>>,
    },
    Stop,
}

/// Canonical renderer inputs for one composition.
///
/// Operator scale resizes viewports *around* the frame; window display scale
/// selects the sampling the raster pass applies later. Neither enters the
/// logical composition, so both map to fixed inputs here.
pub fn canonical_inputs_for(
    environment: UiEnvironment,
    window_scale: f32,
) -> (rendering::Theme, rendering::TargetDensity) {
    let _ = (environment.preferences().scale, window_scale);
    (
        rendering::Theme::default(),
        rendering::TargetDensity::SCALE_100,
    )
}

/// Serialize one completed logical frame: glyph positions, wrapping,
/// pagination inputs, safe area, and element bounds, plus the diagnostics
/// riding beside the frame.
pub fn compose_serialization(
    environment: UiEnvironment,
    window_scale: f32,
    content: &rendering::TemplateContent,
) -> Result<String, rendering::RenderError> {
    let (theme, density) = canonical_inputs_for(environment, window_scale);
    let (frame, diagnostics) = rendering::render_template(content, &theme, density)?;
    let mut out = frame.serialize();
    out.push_str(&format!("warnings {}\n", diagnostics.warnings.len()));
    for warning in &diagnostics.warnings {
        out.push_str(&format!("warning {warning}\n"));
    }
    Ok(out)
}

/// Serve composition requests until `Stop` (or sender drop) ends the loop.
/// Every render runs on this worker thread, never on the GPUI thread.
pub fn serve_canonical(
    requests: Receiver<CanonicalCommand>,
    cancel: CancellationToken,
) -> Result<WorkerExit<()>, String> {
    while let Ok(command) = requests.recv() {
        match command {
            CanonicalCommand::Stop => break,
            CanonicalCommand::Compose {
                environment,
                window_scale,
                content,
                reply,
            } => {
                let _ = reply.send(compose_serialization(environment, window_scale, &content));
            }
        }
    }
    Ok(if cancel.is_cancelled() {
        WorkerExit::Cancelled
    } else {
        WorkerExit::Completed(())
    })
}
