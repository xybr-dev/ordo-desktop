//! Packaged canonical composition (T0.4 PR 8).
//!
//! The worker-owned production boundary from live runtime state to the
//! canonical renderer. Composition runs on the `ordo-canonical` worker
//! thread — rendering never blocks the GPUI thread — behind a request
//! channel the runtime owns: callers snapshot operator state, send
//! [`CanonicalCommand::Compose`], and receive the completed frame plus its
//! diagnostics. Neither the operator Interface scale nor the window display
//! scale may enter composition; both cross this boundary on every request,
//! so any future use of either value fails the packaged invariance gate
//! naming its axis. PR 9's viewport consumers take the frame itself (and
//! rasterize from it) through this same worker; pins and handoff serialize
//! the payload at the edge.

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
        reply: Sender<Result<(rendering::Frame, rendering::Diagnostics), rendering::RenderError>>,
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

/// Compose one completed logical frame plus its diagnostics: glyph
/// positions, wrapping, pagination inputs, safe area, and element bounds.
/// Callers serialize the payload for pins and handoff; PR 9's viewport
/// consumers take the frame itself (and rasterize from it) through this
/// same worker.
pub fn compose_frame(
    environment: UiEnvironment,
    window_scale: f32,
    content: &rendering::TemplateContent,
) -> Result<(rendering::Frame, rendering::Diagnostics), rendering::RenderError> {
    // Counted only if composition runs on a thread with an open hot-path scope.
    // The worker thread never holds one, so a worker render is not a violation.
    crate::performance::hot_path::note(
        crate::performance::hot_path::HotPathKind::CanonicalRecompose,
    );
    let (theme, density) = canonical_inputs_for(environment, window_scale);
    rendering::render_template(content, &theme, density)
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
                let _ = reply.send(compose_frame(environment, window_scale, &content));
            }
        }
    }
    Ok(if cancel.is_cancelled() {
        WorkerExit::Cancelled
    } else {
        WorkerExit::Completed(())
    })
}
