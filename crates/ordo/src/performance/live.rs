//! Live Presenter probe for T0.6 WP5.
//!
//! [`PresenterProbe`] is a GPUI view over the pure [`PresenterEngine`]. It
//! renders Preview and Program side by side and binds the operator keys to
//! Presenter actions. Each action is one keydown. The keydown is stamped,
//! a [`hot_path`] scope opens, the engine applies the command, and a
//! `on_next_frame` callback closes the scope and records keydown-to-paint for
//! Preview and Program from the frame that drew the change.
//!
//! [`drive`] is the self-driver. It dispatches a command stream through
//! `Window::dispatch_keystroke`, one key per presented frame, then idles
//! without input and samples process CPU. It delivers a [`ProbeOutcome`]
//! through a callback.
//!
//! Thumbnail work runs on one named worker thread, `ordo-probe-thumbnails`.
//! Each request is wrapped in [`Enqueued`] and its dequeue wait is observed
//! by [`QueueLatency`]. The worker owns the thread: dropping the probe sends
//! an explicit `Stop` and joins it. The synthetic Cue frames stand in for real
//! Cue content, which the probe does not have.
//!
//! Replies come back over a plain `mpsc` channel that the GPUI thread drains at
//! each paint and at outcome time. An async reply channel would wake a GPUI
//! task from the worker thread, which the test scheduler reports as
//! non-determinism.
//!
//! Every thumbnail request is sent from the frame callback after its scope
//! closes. A send made inside the scope is a hot-path violation.

use std::{
    collections::VecDeque,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyBinding, Keystroke, ParentElement as _, Render, Styled as _, Window,
    WindowHandle, WindowOptions, actions, div, rgb,
};
use presentation::presenter::{PresenterCommand, PresenterEngine, PresenterState};

use super::{
    clock::{Enqueued, QueueLatency},
    frames::FrameTimer,
    hot_path::{self, HotPathKind, HotPathScope},
    latency::{CommandKind, KeydownStamp, LatencyBook, LatencyHistogram, PendingKeydown, Surface},
    process::{GpuMemory, ProcessSample, ProcessSampler, ProcessSamplerError},
    report::{PerformanceReport, RendererInfo},
};

/// What the report's `latency_scope` says the latency measures.
pub const LATENCY_SCOPE: &str = "keydown_to_paint";

/// Key context the probe's bindings are scoped to.
const KEY_CONTEXT: &str = "PresenterProbe";

/// Name of the thumbnail worker thread, visible in debuggers and profilers.
const THUMBNAIL_WORKER_NAME: &str = "ordo-probe-thumbnails";

/// Requests the worker queue holds before a new thumbnail is refused.
const THUMBNAIL_QUEUE_CAPACITY: usize = 64;

/// Synthetic source frame size, in RGBA pixels.
const SOURCE_WIDTH: usize = 320;
const SOURCE_HEIGHT: usize = 180;

/// Thumbnail size. Each thumbnail pixel averages one `BLOCK` of source pixels.
const THUMBNAIL_WIDTH: usize = 32;
const THUMBNAIL_HEIGHT: usize = 18;
const BLOCK_WIDTH: usize = SOURCE_WIDTH / THUMBNAIL_WIDTH;
const BLOCK_HEIGHT: usize = SOURCE_HEIGHT / THUMBNAIL_HEIGHT;

actions!(
    ordo_presenter_probe,
    [
        ProbeNext,
        ProbePrevious,
        ProbeTakeLive,
        ProbeToggleFreeze,
        ProbeToggleBlackout
    ]
);

/// Registers the probe's key bindings. Call once per app before
/// [`drive`] or a key press reaches the probe.
pub fn register_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("right", ProbeNext, Some(KEY_CONTEXT)),
        KeyBinding::new("left", ProbePrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("space", ProbeTakeLive, Some(KEY_CONTEXT)),
        KeyBinding::new("f", ProbeToggleFreeze, Some(KEY_CONTEXT)),
        KeyBinding::new("b", ProbeToggleBlackout, Some(KEY_CONTEXT)),
    ]);
}

/// Registers the key bindings and opens a probe window over a `cue_count`
/// deck.
pub fn open(
    cx: &mut App,
    options: WindowOptions,
    cue_count: usize,
) -> gpui::Result<WindowHandle<PresenterProbe>> {
    register_keys(cx);
    cx.open_window(options, |window, cx| {
        cx.new(|cx| PresenterProbe::new(cue_count, window, cx))
    })
}

/// The Presenter command an operator key sends.
fn command_kind(command: PresenterCommand) -> CommandKind {
    match command {
        PresenterCommand::Next | PresenterCommand::Previous | PresenterCommand::GoTo(_) => {
            CommandKind::Navigation
        }
        PresenterCommand::TakeLive => CommandKind::Space,
        PresenterCommand::ToggleFreeze => CommandKind::Freeze,
        PresenterCommand::ToggleBlackout => CommandKind::Blackout,
    }
}

/// The keystroke that sends `command`. `GoTo` has no key of its own, so the
/// driver expands it into arrow keys instead.
fn keystroke_for(command: PresenterCommand) -> Option<&'static str> {
    match command {
        PresenterCommand::Next => Some("right"),
        PresenterCommand::Previous => Some("left"),
        PresenterCommand::TakeLive => Some("space"),
        PresenterCommand::ToggleFreeze => Some("f"),
        PresenterCommand::ToggleBlackout => Some("b"),
        PresenterCommand::GoTo(_) => None,
    }
}

/// Work order for one thumbnail, as sent to the worker.
struct ThumbnailRequest {
    cue: usize,
}

/// Messages the worker thread receives.
enum ThumbnailCommand {
    Render(Enqueued<ThumbnailRequest>),
    Stop,
}

/// What the worker sends back for one finished thumbnail.
struct ThumbnailReply {
    queue_wait: Duration,
    pixels: Vec<u8>,
}

/// Owns the named thumbnail thread. Dropping it sends `Stop` and joins.
struct ThumbnailWorker {
    commands: async_channel::Sender<ThumbnailCommand>,
    thread: Option<JoinHandle<()>>,
}

impl ThumbnailWorker {
    fn spawn(replies: mpsc::Sender<ThumbnailReply>) -> std::io::Result<Self> {
        let (commands, requests) = async_channel::bounded(THUMBNAIL_QUEUE_CAPACITY);
        let thread = thread::Builder::new()
            .name(THUMBNAIL_WORKER_NAME.to_string())
            .spawn(move || serve_thumbnails(requests, replies))?;
        Ok(Self {
            commands,
            thread: Some(thread),
        })
    }

    /// Queues a thumbnail of `cue`. Never waits: a full queue refuses the
    /// request and returns `false`.
    fn request(&self, cue: usize) -> bool {
        hot_path::note(HotPathKind::WorkerSend);
        self.commands
            .try_send(ThumbnailCommand::Render(Enqueued::new(ThumbnailRequest {
                cue,
            })))
            .is_ok()
    }
}

impl Drop for ThumbnailWorker {
    fn drop(&mut self) {
        // A dead worker has already stopped, so a failed send needs no action.
        let _ = self.commands.send_blocking(ThumbnailCommand::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Worker loop: dequeue, observe the wait, downscale, reply, until `Stop`.
fn serve_thumbnails(
    requests: async_channel::Receiver<ThumbnailCommand>,
    replies: mpsc::Sender<ThumbnailReply>,
) {
    let mut queue = QueueLatency::new();
    while let Ok(command) = requests.recv_blocking() {
        let item = match command {
            ThumbnailCommand::Render(item) => item,
            ThumbnailCommand::Stop => break,
        };
        let queue_wait = queue.observe(&item, Instant::now());
        let (request, _) = item.into_parts();
        let pixels = downscale(&synthetic_source(request.cue));
        if replies.send(ThumbnailReply { queue_wait, pixels }).is_err() {
            break;
        }
    }
}

/// A deterministic RGBA source frame for `cue`. The probe's Cues carry no
/// content, so each one gets a pattern that depends on its index.
fn synthetic_source(cue: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; SOURCE_WIDTH * SOURCE_HEIGHT * 4];
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let x = index % SOURCE_WIDTH;
        let y = index / SOURCE_WIDTH;
        pixel.copy_from_slice(&[x as u8, y as u8, cue as u8, u8::MAX]);
    }
    pixels
}

/// Box-averages the source frame down to the thumbnail size.
fn downscale(source: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; THUMBNAIL_WIDTH * THUMBNAIL_HEIGHT * 4];
    let block_area = (BLOCK_WIDTH * BLOCK_HEIGHT) as u32;
    for ty in 0..THUMBNAIL_HEIGHT {
        for tx in 0..THUMBNAIL_WIDTH {
            let mut sums = [0u32; 4];
            for by in 0..BLOCK_HEIGHT {
                let row = (ty * BLOCK_HEIGHT + by) * SOURCE_WIDTH;
                for bx in 0..BLOCK_WIDTH {
                    let at = (row + tx * BLOCK_WIDTH + bx) * 4;
                    for (channel, sum) in sums.iter_mut().enumerate() {
                        *sum += u32::from(source[at + channel]);
                    }
                }
            }
            let at = (ty * THUMBNAIL_WIDTH + tx) * 4;
            for (channel, sum) in sums.iter().enumerate() {
                out[at + channel] = (sum / block_area) as u8;
            }
        }
    }
    out
}

/// What one drive delivers: the report and the Presenter state the probe ended
/// in, for the oracle check.
#[derive(Clone, Debug)]
pub struct ProbeOutcome {
    pub report: PerformanceReport,
    pub final_state: PresenterState,
}

/// Why a drive could not start.
#[derive(Debug)]
pub enum DriveError {
    /// The current process could not be identified for CPU and memory sampling.
    Sampler(ProcessSamplerError),
    /// The probe window is no longer open.
    WindowClosed,
}

/// Receives the outcome of a drive.
type DoneCallback = Box<dyn FnOnce(ProbeOutcome, &mut App)>;

/// One drive in flight. Dropped when the drive ends.
struct Run {
    pending: VecDeque<PresenterCommand>,
    /// Arrow keys expanded from a `GoTo`, not yet dispatched.
    keys: VecDeque<&'static str>,
    idle: Duration,
    sampler: ProcessSampler,
    renderer: RendererInfo,
    on_done: DoneCallback,
}

/// GPUI view over one Presenter deck, with the latency and frame instruments.
pub struct PresenterProbe {
    engine: PresenterEngine,
    cue_count: usize,
    focus: FocusHandle,
    latency: LatencyBook,
    frames: FrameTimer,
    worker_queue: LatencyHistogram,
    thumbnails: ThumbnailWorker,
    /// Thumbnail replies from the worker, drained on the GPUI thread.
    thumbnail_replies: mpsc::Receiver<ThumbnailReply>,
    thumbnails_received: u64,
    thumbnail_bytes: u64,
    run: Option<Run>,
    /// Test hook: the next command notes a violation inside its scope.
    #[cfg(test)]
    violate_on_next_command: bool,
}

impl PresenterProbe {
    /// A probe over `cue_count` Cues with Preview on the first Cue. Takes
    /// focus so the key bindings reach it.
    pub fn new(cue_count: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let (reply_tx, reply_rx) = mpsc::channel();
        let thumbnails = ThumbnailWorker::spawn(reply_tx)
            .expect("the probe's thumbnail worker thread must start");
        Self {
            engine: PresenterEngine::new(cue_count),
            cue_count,
            focus,
            latency: LatencyBook::new(),
            frames: FrameTimer::new(),
            worker_queue: LatencyHistogram::new(),
            thumbnails,
            thumbnail_replies: reply_rx,
            thumbnails_received: 0,
            thumbnail_bytes: 0,
            run: None,
            #[cfg(test)]
            violate_on_next_command: false,
        }
    }

    /// The Presenter state now. Equal to the pure engine's state after the
    /// same commands, which is what the oracle check compares.
    pub fn final_state(&self) -> PresenterState {
        *self.engine.state()
    }

    /// Takes every thumbnail reply the worker has sent and records it. Returns
    /// the total thumbnails recorded so far. Called at each paint and at
    /// outcome time, on the GPUI thread.
    pub fn drain_thumbnails(&mut self) -> u64 {
        while let Ok(reply) = self.thumbnail_replies.try_recv() {
            self.record_thumbnail(reply);
        }
        self.thumbnails_received
    }

    /// Dequeue waits of the thumbnail worker, as recorded so far.
    pub fn worker_queue(&self) -> &LatencyHistogram {
        &self.worker_queue
    }

    /// The probe's keydown-to-paint samples so far.
    pub fn latency(&self) -> &LatencyBook {
        &self.latency
    }

    /// Applies one operator command. The stamp, the hot-path scope, and the
    /// paint callback are all opened here, so the keydown and the paint of
    /// its frame are the ends of one measurement.
    fn apply_command(
        &mut self,
        command: PresenterCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stamp = PendingKeydown::start(command_kind(command));
        let scope = hot_path::enter();
        #[cfg(test)]
        self.inject_violation();
        self.engine.apply(command);
        cx.notify();
        let entity = cx.entity();
        window.on_next_frame(move |window, cx| paint(entity, stamp, scope, window, cx));
    }

    #[cfg(test)]
    fn inject_violation(&mut self) {
        if std::mem::take(&mut self.violate_on_next_command) {
            hot_path::note(HotPathKind::FileAccess);
        }
    }

    /// Records the paint of the frame that drew one command. Preview and
    /// Program are both painted by that frame, so both get the same elapsed
    /// time.
    fn record_paint(&mut self, stamp: KeydownStamp) {
        for surface in Surface::ALL {
            PendingKeydown::finish(stamp, surface, &mut self.latency);
        }
    }

    /// Queues the thumbnail for the Preview Cue. Called only after the paint
    /// scope has closed.
    fn request_thumbnail(&mut self) {
        let preview = self.engine.state().preview();
        if self.cue_count > 0 {
            self.thumbnails.request(preview);
        }
    }

    fn record_thumbnail(&mut self, reply: ThumbnailReply) {
        self.worker_queue.record(reply.queue_wait);
        self.thumbnails_received += 1;
        self.thumbnail_bytes += reply.pixels.len() as u64;
    }

    /// Starts a drive: resets the instruments, stores the command stream, and
    /// arms the frame loop. The first key is dispatched by the caller, outside
    /// this view update.
    fn begin_run(
        &mut self,
        commands: Vec<PresenterCommand>,
        idle: Duration,
        sampler: ProcessSampler,
        on_done: DoneCallback,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.latency = LatencyBook::new();
        self.frames = FrameTimer::new();
        self.worker_queue = LatencyHistogram::new();
        self.thumbnails_received = 0;
        self.thumbnail_bytes = 0;
        self.engine = PresenterEngine::new(self.cue_count);
        hot_path::take_violations();
        self.run = Some(Run {
            pending: commands.into(),
            keys: VecDeque::new(),
            idle,
            sampler,
            renderer: window
                .gpu_specs()
                .map_or_else(RendererInfo::default, |specs| RendererInfo {
                    software_emulated: Some(specs.is_software_emulated),
                    device_name: Some(specs.device_name),
                }),
            on_done,
        });
        arm_frame_loop(cx.entity(), window);
    }

    /// The next key of the run, or `None` when the run is over or none is
    /// active. A `GoTo` expands into arrow keys from the current Preview. The
    /// queue is only expanded once the previous keys have applied, so the
    /// Preview read here is current.
    fn next_key(&mut self) -> Option<&'static str> {
        loop {
            let run = self.run.as_mut()?;
            if let Some(key) = run.keys.pop_front() {
                return Some(key);
            }
            let command = run.pending.pop_front()?;
            if let Some(key) = keystroke_for(command) {
                return Some(key);
            }
            let PresenterCommand::GoTo(target) = command else {
                continue;
            };
            let from = self.engine.state().preview();
            let to = target.min(self.cue_count.saturating_sub(1));
            let keys = if to > from {
                std::iter::repeat_n("right", to - from)
            } else {
                std::iter::repeat_n("left", from - to)
            };
            if let Some(run) = self.run.as_mut() {
                run.keys.extend(keys);
            }
        }
    }

    /// Ends the run and takes its state out of the view. The hot-path count
    /// is read here, on the GPUI thread that noted the violations.
    fn close_run(&mut self) -> Option<Run> {
        self.run.take()
    }

    /// The outcome once the idle sample lands.
    fn outcome(
        &self,
        sample: Option<ProcessSample>,
        renderer: RendererInfo,
        hot_path_violations: u64,
    ) -> ProbeOutcome {
        ProbeOutcome {
            report: PerformanceReport {
                latency: self.latency.clone(),
                latency_scope: LATENCY_SCOPE,
                frames: self.frames.clone(),
                // The probe is not the process start. The lab owns startup.
                time_to_first_paint: None,
                worker_queue: self.worker_queue.clone(),
                process: sample,
                idle_cpu_percent: sample.and_then(|s| s.cpu_percent),
                gpu_memory: GpuMemory::Unavailable,
                renderer,
                hot_path_violations,
            },
            final_state: self.final_state(),
        }
    }
}

/// Keeps a frame callback armed while a run is active, recording each
/// presented frame. The loop stops when the run closes.
fn arm_frame_loop(entity: Entity<PresenterProbe>, window: &Window) {
    window.on_next_frame(move |window, cx| {
        let active = entity.update(cx, |probe, _| {
            let active = probe.run.is_some();
            if active {
                probe.frames.record_frame(Instant::now());
            }
            active
        });
        if active {
            arm_frame_loop(entity, window);
        }
    });
}

/// The paint callback for one command. It runs after the frame that drew the
/// command. Thumbnail work starts only after the scope closes, so the send
/// is never a violation.
fn paint(
    entity: Entity<PresenterProbe>,
    stamp: KeydownStamp,
    scope: HotPathScope,
    window: &mut Window,
    cx: &mut App,
) {
    entity.update(cx, |probe, _| probe.record_paint(stamp));
    scope.close();
    entity.update(cx, |probe, _| {
        probe.drain_thumbnails();
        probe.request_thumbnail();
    });
    step(&entity, window, cx);
}

/// Dispatches the run's next key through the real key path, or finishes the
/// run when no key is left. A key no binding handles would never paint, so the
/// run ends rather than waiting for a frame that cannot come.
fn step(entity: &Entity<PresenterProbe>, window: &mut Window, cx: &mut App) {
    let next = entity.update(cx, |probe, _| probe.next_key());
    let handled = match next {
        Some(key) => {
            let keystroke = Keystroke::parse(key).expect("probe keys are valid keystrokes");
            window.dispatch_keystroke(keystroke, cx)
        }
        None => false,
    };
    if next.is_none() || !handled {
        finish_run(entity, cx);
    }
}

/// Closes the run, then samples CPU over the idle window on a background
/// task. The outcome is delivered from that task.
fn finish_run(entity: &Entity<PresenterProbe>, cx: &mut App) {
    // Without an open run there is nothing to finish, and nothing to count.
    let Some(run) = entity.update(cx, |probe, _| probe.close_run()) else {
        return;
    };
    let violations: u64 = hot_path::take_violations()
        .iter()
        .map(|(_, count)| count)
        .sum();
    let Run {
        idle,
        sampler,
        renderer,
        on_done,
        ..
    } = run;
    let probe = entity.downgrade();
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        // The first sample only sets the CPU baseline. The second sample's
        // CPU is the delta over the idle window.
        let (sampler, _) = background
            .spawn(async move {
                let mut sampler = sampler;
                let baseline = sampler.sample();
                (sampler, baseline)
            })
            .await;
        background.timer(idle).await;
        let idle_sample = background
            .spawn(async move {
                let mut sampler = sampler;
                sampler.sample()
            })
            .await;
        let Ok(outcome) = probe.update(cx, |probe, _| {
            probe.drain_thumbnails();
            probe.outcome(idle_sample, renderer, violations)
        }) else {
            return;
        };
        cx.update(move |cx| on_done(outcome, cx));
    })
    .detach();
}

impl Focusable for PresenterProbe {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PresenterProbe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = *self.engine.state();
        div()
            .track_focus(&self.focus)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &ProbeNext, window, cx| {
                this.apply_command(PresenterCommand::Next, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbePrevious, window, cx| {
                this.apply_command(PresenterCommand::Previous, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbeTakeLive, window, cx| {
                this.apply_command(PresenterCommand::TakeLive, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbeToggleFreeze, window, cx| {
                this.apply_command(PresenterCommand::ToggleFreeze, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbeToggleBlackout, window, cx| {
                this.apply_command(PresenterCommand::ToggleBlackout, window, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .bg(rgb(0x18181c))
            .text_color(rgb(0xe8e8ee))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(surface_panel(
                        "Preview",
                        &[
                            cue_label(state.preview(), self.cue_count),
                            format!("revision {}", state.revision()),
                        ],
                    ))
                    .child(surface_panel(
                        "Program",
                        &program_lines(&state, self.cue_count),
                    )),
            )
            .child(self.footer())
    }
}

impl PresenterProbe {
    /// Live keydown-to-paint percentiles per command and surface, plus frame,
    /// worker-queue, and thumbnail counters.
    fn footer(&self) -> impl IntoElement {
        let mut lines = vec!["Keydown to paint, p50 / p95 / p99".to_string()];
        for command in CommandKind::ALL {
            let cell = |surface: Surface| match self.latency.histogram(command, surface) {
                Some(h) if h.count() > 0 => {
                    let [p50, p95, p99] = [h.p50(), h.p95(), h.p99()];
                    format!(
                        "{} {} / {} / {}",
                        surface.as_str(),
                        fmt_ms(p50),
                        fmt_ms(p95),
                        fmt_ms(p99)
                    )
                }
                _ => format!("{} no samples", surface.as_str()),
            };
            lines.push(format!(
                "{}: {}   |   {}",
                command.as_str(),
                cell(Surface::Preview),
                cell(Surface::Program)
            ));
        }
        lines.push(format!(
            "Frames {} presented, {} dropped   Worker queue p95 {}   Thumbnails {} ({} KiB)",
            self.frames.frame_count(),
            self.frames.dropped_frames(),
            fmt_ms(self.worker_queue.p95()),
            self.thumbnails_received,
            self.thumbnail_bytes / 1024,
        ));
        div()
            .flex()
            .flex_col()
            .gap_1()
            .text_sm()
            .children(lines.into_iter().map(|line| div().child(line)))
    }
}

fn surface_panel(title: &str, lines: &[String]) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(rgb(0x26262e))
        .child(div().text_lg().child(title.to_string()))
        .children(lines.iter().map(|line| div().child(line.clone())))
}

fn cue_label(index: usize, cue_count: usize) -> String {
    if cue_count == 0 {
        "No Cues".to_string()
    } else {
        format!("Cue {} of {}", index + 1, cue_count)
    }
}

/// Program's lines, with the BLACK and FROZEN markers the operator must see.
fn program_lines(state: &PresenterState, cue_count: usize) -> Vec<String> {
    let mut lines = Vec::new();
    if state.blackout() {
        lines.push("BLACK".to_string());
    }
    if state.frozen() {
        lines.push("FROZEN".to_string());
    }
    lines.push(match state.program_visible() {
        Some(index) => cue_label(index, cue_count),
        None if state.blackout() => "Program is black".to_string(),
        None => "No Cue taken yet".to_string(),
    });
    lines.push(format!("revision {}", state.revision()));
    lines
}

fn fmt_ms(value: Option<Duration>) -> String {
    match value {
        Some(d) => format!("{:.2} ms", d.as_secs_f64() * 1000.0),
        None => "n/a".to_string(),
    }
}

/// Drives `commands` through the probe window, one key per presented frame,
/// then idles for `idle` and delivers the outcome to `on_done`.
///
/// Every command is sent as its operator keystroke through
/// `Window::dispatch_keystroke`, so it takes the real binding and action
/// path. The next key waits for the previous frame to paint. The window must
/// be open and the bindings registered with [`register_keys`] or [`open`].
pub fn drive(
    window: WindowHandle<PresenterProbe>,
    commands: Vec<PresenterCommand>,
    idle: Duration,
    on_done: impl FnOnce(ProbeOutcome, &mut App) + 'static,
    cx: &mut App,
) -> Result<(), DriveError> {
    let sampler = ProcessSampler::new().map_err(DriveError::Sampler)?;
    let entity = window
        .update(cx, |probe, window, cx| {
            probe.begin_run(commands, idle, sampler, Box::new(on_done), window, cx);
            cx.entity()
        })
        .map_err(|_| DriveError::WindowClosed)?;
    cx.update_window(window.into(), |_, window, cx| step(&entity, window, cx))
        .map_err(|_| DriveError::WindowClosed)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::TestAppContext;

    use super::*;

    /// Pumps frames and the idle clock until the drive delivers an outcome.
    fn run_to_outcome(
        cx: &mut TestAppContext,
        window: WindowHandle<PresenterProbe>,
        commands: Vec<PresenterCommand>,
    ) -> ProbeOutcome {
        let delivered = Rc::new(RefCell::new(None));
        let sink = delivered.clone();
        let idle = Duration::from_secs(3);
        cx.update(|cx| {
            drive(
                window,
                commands,
                idle,
                move |outcome, _| *sink.borrow_mut() = Some(outcome),
                cx,
            )
            .expect("drive starts")
        });
        for _ in 0..1_000 {
            cx.run_until_parked();
            if delivered.borrow().is_some() {
                break;
            }
            let drawn = cx
                .update_window(window.into(), |_, window, cx| {
                    window.simulate_next_frame(cx)
                })
                .expect("probe window is open");
            if drawn == 0 {
                cx.executor().advance_clock(idle);
            }
        }
        let outcome = delivered.borrow_mut().take();
        outcome.expect("drive delivers an outcome")
    }

    /// Waits for every thumbnail reply the worker sends to be drained.
    fn wait_for_thumbnails(
        cx: &mut TestAppContext,
        window: WindowHandle<PresenterProbe>,
        expected: u64,
    ) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while window
            .update(cx, |probe, _, _| probe.drain_thumbnails())
            .expect("probe window is open")
            < expected
        {
            assert!(
                Instant::now() < deadline,
                "thumbnail replies did not arrive"
            );
            std::thread::sleep(Duration::from_millis(2));
            cx.run_until_parked();
        }
    }

    fn oracle(cue_count: usize, commands: &[PresenterCommand]) -> PresenterState {
        let mut engine = PresenterEngine::new(cue_count);
        for &command in commands {
            engine.apply(command);
        }
        *engine.state()
    }

    /// Whether two states show the same thing. Revision is excluded because an
    /// expanded GoTo makes one state change per arrow key.
    fn same_position(a: &PresenterState, b: &PresenterState) -> bool {
        a.preview() == b.preview()
            && a.program() == b.program()
            && a.frozen() == b.frozen()
            && a.blackout() == b.blackout()
    }

    #[gpui::test]
    fn drive_records_keydown_to_paint_for_each_command_and_surface(cx: &mut TestAppContext) {
        cx.update(register_keys);
        let window = cx.add_window(|window, cx| PresenterProbe::new(5, window, cx));
        let commands = vec![
            PresenterCommand::Next,
            PresenterCommand::Next,
            PresenterCommand::TakeLive,
            PresenterCommand::ToggleFreeze,
            PresenterCommand::Next,
            PresenterCommand::ToggleBlackout,
            PresenterCommand::Previous,
            PresenterCommand::TakeLive,
            PresenterCommand::ToggleFreeze,
            PresenterCommand::ToggleBlackout,
        ];

        let outcome = run_to_outcome(cx, window, commands.clone());

        // Four navigation keys, two Space, two Freeze, two Blackout.
        let expected = [
            (CommandKind::Navigation, 4),
            (CommandKind::Space, 2),
            (CommandKind::Freeze, 2),
            (CommandKind::Blackout, 2),
        ];
        for (command, count) in expected {
            for surface in Surface::ALL {
                let histogram = outcome
                    .report
                    .latency
                    .histogram(command, surface)
                    .unwrap_or_else(|| panic!("{command:?} to {surface:?} recorded"));
                assert_eq!(histogram.total_recorded(), count, "{command:?} {surface:?}");
                assert!(histogram.p50().is_some());
                assert!(histogram.p99().is_some());
            }
        }
        assert_eq!(outcome.report.latency_scope, LATENCY_SCOPE);
        assert_eq!(outcome.report.hot_path_violations, 0);
        assert!(outcome.report.frames.frame_count() > 0);
        assert!(outcome.report.process.is_some());
        assert!(outcome.report.idle_cpu_percent.is_some());
        assert_eq!(outcome.final_state, oracle(5, &commands));

        wait_for_thumbnails(cx, window, commands.len() as u64);
        let queued = window
            .read_with(cx, |probe, _| probe.worker_queue().count())
            .expect("probe window is open");
        assert_eq!(queued, commands.len());
    }

    #[gpui::test]
    fn deliberate_violation_inside_the_scope_is_reported(cx: &mut TestAppContext) {
        cx.update(register_keys);
        let window = cx.add_window(|window, cx| PresenterProbe::new(3, window, cx));
        window
            .update(cx, |probe, _, _| probe.violate_on_next_command = true)
            .expect("probe window is open");

        let commands = vec![PresenterCommand::Next, PresenterCommand::TakeLive];
        let outcome = run_to_outcome(cx, window, commands.clone());

        assert_eq!(outcome.report.hot_path_violations, 1);
        assert!(
            outcome
                .report
                .summary_lines()
                .iter()
                .any(|line| line.contains("1 off-path violations"))
        );
        assert_eq!(outcome.final_state, oracle(3, &commands));
    }

    #[gpui::test]
    fn goto_expands_into_arrow_keys_and_matches_the_oracle(cx: &mut TestAppContext) {
        cx.update(register_keys);
        let window = cx.add_window(|window, cx| PresenterProbe::new(5, window, cx));
        let commands = vec![
            PresenterCommand::GoTo(4),
            PresenterCommand::Previous,
            PresenterCommand::GoTo(0),
            PresenterCommand::Next,
            PresenterCommand::GoTo(99),
            PresenterCommand::TakeLive,
            PresenterCommand::GoTo(1),
        ];

        let outcome = run_to_outcome(cx, window, commands.clone());

        assert!(same_position(&outcome.final_state, &oracle(5, &commands)));
        assert_eq!(outcome.report.hot_path_violations, 0);
        // Each arrow key is one navigation keydown. From Preview 0: GoTo(4) is
        // 4 keys, Previous 1, GoTo(0) 3, Next 1, GoTo(99) clamps to Cue 4 in
        // 3 keys, and GoTo(1) is 3 keys.
        let navigation = outcome
            .report
            .latency
            .histogram(CommandKind::Navigation, Surface::Preview)
            .expect("navigation recorded");
        assert_eq!(navigation.total_recorded(), 4 + 1 + 3 + 1 + 3 + 3);
    }
}
