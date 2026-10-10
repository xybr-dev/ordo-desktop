//! Live Presenter probe for T0.6 WP5.
//!
//! [`PresenterProbe`] is a GPUI view over the pure [`PresenterEngine`]. It
//! renders Preview and Program side by side and binds the operator keys to
//! Presenter actions. Each command yields one keydown-to-surface-paint sample
//! per surface, measured by the following spans:
//!
//! - **Keydown stamp.** [`drive`] takes the stamp immediately before it
//!   dispatches the command: `Window::dispatch_keystroke` for keys, or
//!   `Window::dispatch_action` for `GoTo`. An interactive key with no driver
//!   stamps when its action applies.
//! - **Hot-path scope.** Opened when the command applies. It stays open through
//!   render, paint, and the settle callback, and closes in the settle callback.
//!   Notes made in that window are violations. The thumbnail request is sent
//!   after the scope closes, so it is never a violation.
//! - **Keydown to surface paint.** Preview and Program each have a `canvas`
//!   whose paint closure stamps the time that surface's paint runs. The span
//!   stops there, before GPU present. The two stamps are separate paint times,
//!   so Preview and Program can differ. This is what [`LATENCY_SCOPE`] names.
//! - **Keydown to present upper bound.** The first paint in a drawn frame
//!   registers one `on_next_frame` callback. A callback registered during a
//!   draw runs at the start of the next frame, after the drawn frame was
//!   presented. The settle callback records stamp to settle time, which is an
//!   upper bound on keydown to present, at most one frame late. Settle then
//!   dispatches the next command.
//!
//! [`drive`] is the self-driver. It dispatches a command stream one command per
//! settled frame, then idles without input and samples process CPU. It
//! delivers a `Result<ProbeOutcome, DriveError>` through a callback, and
//! delivers `Err(DriveError::WindowClosed)` if the window closes mid-run.
//!
//! Thumbnail work runs on one named worker thread, `ordo-probe-thumbnails`.
//! Each request is wrapped in [`Enqueued`] and its dequeue wait is observed
//! by [`QueueLatency`]. Dropping the probe queues `Stop` behind the pending
//! requests, at most `THUMBNAIL_QUEUE_CAPACITY` of them, or closes the channel
//! if the queue is full. The worker exits at `Stop`, or after its current item
//! once the channel is closed, and the probe joins it. The synthetic Cue frames
//! stand in for real Cue content, which the probe does not have.
//!
//! Replies come back over a plain `mpsc` channel that the GPUI thread drains at
//! each paint and at outcome time. An async reply channel would wake a GPUI
//! task from the worker thread, which the test scheduler reports as
//! non-determinism.
//!
//! Intended call sequence for the lab, beside the real runtime:
//! `open_native_runtime_at` first, then [`open`] (no runtime state is needed,
//! and key registration is idempotent), then [`drive`] on the returned handle.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyBinding, Keystroke, ParentElement as _, Render, Styled as _, Subscription,
    WeakEntity, Window, WindowHandle, WindowOptions, actions, canvas, div, rgb,
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

/// What the report's `latency_scope` says the latency measures. The span stops
/// when the surface's paint runs, before GPU present.
pub const LATENCY_SCOPE: &str = "keydown_to_surface_paint";

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

/// Moves Preview to a Cue index. The probe has no key for it, so [`drive`]
/// dispatches it with `Window::dispatch_action`.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = ordo_presenter_probe, no_json)]
pub struct ProbeGoTo(pub usize);

/// Marks that the probe's key bindings are already registered in this App.
struct ProbeKeysRegistered;

impl gpui::Global for ProbeKeysRegistered {}

/// Registers the probe's key bindings once per App. Safe to call again, so
/// [`open`] can be used beside a runtime that also registered keys.
pub fn register_keys(cx: &mut App) {
    if cx.has_global::<ProbeKeysRegistered>() {
        return;
    }
    cx.set_global(ProbeKeysRegistered);
    cx.bind_keys([
        KeyBinding::new("right", ProbeNext, Some(KEY_CONTEXT)),
        KeyBinding::new("left", ProbePrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("space", ProbeTakeLive, Some(KEY_CONTEXT)),
        KeyBinding::new("f", ProbeToggleFreeze, Some(KEY_CONTEXT)),
        KeyBinding::new("b", ProbeToggleBlackout, Some(KEY_CONTEXT)),
    ]);
}

/// Registers the key bindings and opens a probe window over a `cue_count`
/// deck. Needs only `App`, so it can run beside an opened native runtime.
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

/// The keystroke that sends `command`. `GoTo` has no key and is dispatched as
/// [`ProbeGoTo`] instead.
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

/// Index of a surface in [`OpenCommand::painted`].
fn slot(surface: Surface) -> usize {
    match surface {
        Surface::Preview => 0,
        Surface::Program => 1,
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

/// Owns the named thumbnail thread. Dropping it sends `Stop` (or closes the
/// channel if the queue is full) and joins.
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
        // A full queue refuses Stop, so the channel is closed instead. The
        // worker observes the closure between items.
        if self.commands.try_send(ThumbnailCommand::Stop).is_err() {
            self.commands.close();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Worker loop: dequeue, observe the wait, downscale, reply, until `Stop` or
/// until the channel is closed.
fn serve_thumbnails(
    requests: async_channel::Receiver<ThumbnailCommand>,
    replies: mpsc::Sender<ThumbnailReply>,
) {
    let mut queue = QueueLatency::new();
    while let Ok(command) = requests.recv_blocking() {
        if requests.is_closed() {
            break;
        }
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

/// Why a drive could not start, or ended before its outcome.
#[derive(Debug)]
pub enum DriveError {
    /// The current process could not be identified for CPU and memory sampling.
    Sampler(ProcessSamplerError),
    /// The probe window is closed, before the drive started or mid-run.
    WindowClosed,
    /// A dispatched command never applied. Its key had no binding, or its
    /// action was not handled by the focused probe.
    Unhandled,
    /// A surface did not paint the frame that drew a command before the next
    /// frame settled.
    NotPainted,
    /// A newer [`drive`] replaced this one before it finished.
    Superseded,
}

/// Receives the outcome of a drive, exactly once.
type DoneCallback = Box<dyn FnOnce(Result<ProbeOutcome, DriveError>, &mut App)>;

/// The drive's callback, shared between the run and the window-closed observer.
/// Whichever fires first takes it, so the callback runs exactly once.
type DoneSlot = Rc<RefCell<Option<DoneCallback>>>;

/// Delivers `result` through `done` if no one has delivered yet.
fn finish_with(done: &DoneSlot, result: Result<ProbeOutcome, DriveError>, cx: &mut App) {
    let callback = done.borrow_mut().take();
    if let Some(callback) = callback {
        callback(result, cx);
    }
}

/// One drive in flight. Dropped when the drive ends.
struct Run {
    pending: VecDeque<PresenterCommand>,
    /// Commands dispatched so far, counting from the start of the run. Each
    /// one must apply before its settle callback.
    dispatched: u64,
    idle: Duration,
    sampler: ProcessSampler,
    renderer: RendererInfo,
    done: DoneSlot,
    /// Unsubscribes the window-closed observer when the run ends.
    window_closed: Subscription,
}

/// One applied command, from apply until its settle callback.
struct OpenCommand {
    stamp: KeydownStamp,
    /// Open from apply until settle. Notes made in render and paint count.
    scope: HotPathScope,
    /// Paint time per surface, set by that surface's canvas.
    painted: [Option<Instant>; 2],
}

/// State shared between the probe and its surface canvases.
#[derive(Default)]
struct PaintBoard {
    /// Commands applied and not yet settled.
    open: Vec<OpenCommand>,
    /// Whether a settle callback is already registered for the next frame.
    settle_armed: bool,
    /// Test hook: note a violation during the next render.
    #[cfg(test)]
    violate_in_render: bool,
    /// Test hook: note a violation during the next surface paint.
    #[cfg(test)]
    violate_in_paint: bool,
}

/// GPUI view over one Presenter deck, with the latency and frame instruments.
pub struct PresenterProbe {
    engine: PresenterEngine,
    cue_count: usize,
    focus: FocusHandle,
    latency: LatencyBook,
    /// Keydown to settle, per command kind. See the module doc.
    present_upper_bound: BTreeMap<CommandKind, LatencyHistogram>,
    frames: FrameTimer,
    worker_queue: LatencyHistogram,
    thumbnails: ThumbnailWorker,
    /// Thumbnail replies from the worker, drained on the GPUI thread.
    thumbnail_replies: mpsc::Receiver<ThumbnailReply>,
    thumbnails_received: u64,
    thumbnail_bytes: u64,
    /// Commands applied, whichever path applied them.
    applied: u64,
    /// Stamp the driver took just before it dispatched the next command.
    pending_stamp: Option<KeydownStamp>,
    board: Rc<RefCell<PaintBoard>>,
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
            present_upper_bound: BTreeMap::new(),
            frames: FrameTimer::new(),
            worker_queue: LatencyHistogram::new(),
            thumbnails,
            thumbnail_replies: reply_rx,
            thumbnails_received: 0,
            thumbnail_bytes: 0,
            applied: 0,
            pending_stamp: None,
            board: Rc::new(RefCell::new(PaintBoard::default())),
            run: None,
            #[cfg(test)]
            violate_on_next_command: false,
        }
    }

    /// The Presenter state now. Equal to the pure engine's state after the
    /// same commands, revision included, which is what the oracle check
    /// compares.
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

    /// The probe's keydown-to-surface-paint samples so far.
    pub fn latency(&self) -> &LatencyBook {
        &self.latency
    }

    /// Applies one operator command. The keydown stamp (the driver's, or one
    /// taken now for an interactive key), the hot-path scope, and the surface
    /// paints are all tied to this command, which ends at its settle callback.
    fn apply_command(&mut self, command: PresenterCommand, cx: &mut Context<Self>) {
        let stamp = self
            .pending_stamp
            .take()
            .unwrap_or_else(|| PendingKeydown::start(command_kind(command)));
        let scope = hot_path::enter();
        #[cfg(test)]
        self.inject_violation();
        self.applied += 1;
        self.engine.apply(command);
        self.board.borrow_mut().open.push(OpenCommand {
            stamp,
            scope,
            painted: [None; 2],
        });
        cx.notify();
    }

    #[cfg(test)]
    fn inject_violation(&mut self) {
        if std::mem::take(&mut self.violate_on_next_command) {
            hot_path::note(HotPathKind::FileAccess);
        }
    }

    /// Closes every applied command at settle time. Records keydown to each
    /// surface's paint and keydown to `now`, and closes the scopes. During a
    /// run it also checks that every dispatched command applied, and that every
    /// surface painted. Returns an error only during a run, since interactive
    /// commands have no caller to report to.
    fn settle_paints(&mut self, now: Instant) -> Result<(), DriveError> {
        let open = {
            let mut board = self.board.borrow_mut();
            board.settle_armed = false;
            std::mem::take(&mut board.open)
        };
        let mut result = Ok(());
        for OpenCommand {
            stamp,
            scope,
            painted,
        } in open
        {
            let kind = stamp.kind();
            self.present_upper_bound
                .entry(kind)
                .or_default()
                .record(now.saturating_duration_since(stamp.at()));
            for surface in Surface::ALL {
                match painted[slot(surface)] {
                    Some(at) => {
                        let elapsed = at.saturating_duration_since(stamp.at());
                        self.latency.record(kind, surface, elapsed);
                    }
                    None if result.is_ok() => result = Err(DriveError::NotPainted),
                    None => {}
                }
            }
            scope.close();
        }
        match &self.run {
            Some(run) if self.applied != run.dispatched && result.is_ok() => {
                result = Err(DriveError::Unhandled);
            }
            _ => {}
        }
        if self.run.is_some() { result } else { Ok(()) }
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
    /// arms the frame loop. The first command is dispatched by the caller,
    /// outside this view update.
    #[allow(clippy::too_many_arguments)]
    fn begin_run(
        &mut self,
        commands: Vec<PresenterCommand>,
        idle: Duration,
        sampler: ProcessSampler,
        done: DoneSlot,
        window_closed: Subscription,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(superseded) = self.run.take() {
            // Deferred, so the earlier callback never runs inside this update.
            let superseded_done = superseded.done.clone();
            drop(superseded);
            cx.defer(move |cx| finish_with(&superseded_done, Err(DriveError::Superseded), cx));
        }
        self.latency = LatencyBook::new();
        self.present_upper_bound.clear();
        self.frames = FrameTimer::new();
        self.worker_queue = LatencyHistogram::new();
        self.thumbnails_received = 0;
        self.thumbnail_bytes = 0;
        self.engine = PresenterEngine::new(self.cue_count);
        self.pending_stamp = None;
        self.board.borrow_mut().open.clear();
        hot_path::take_violations();
        self.run = Some(Run {
            pending: commands.into(),
            dispatched: self.applied,
            idle,
            sampler,
            renderer: window
                .gpu_specs()
                .map_or_else(RendererInfo::default, |specs| RendererInfo {
                    software_emulated: Some(specs.is_software_emulated),
                    device_name: Some(specs.device_name),
                }),
            done,
            window_closed,
        });
        arm_frame_loop(cx.entity(), window);
    }

    /// The next command of the run, counted as dispatched. `None` when the run
    /// is over or none is active.
    fn next_command(&mut self) -> Option<PresenterCommand> {
        let run = self.run.as_mut()?;
        let command = run.pending.pop_front()?;
        run.dispatched += 1;
        Some(command)
    }

    /// Ends the run and takes its state out of the view.
    fn close_run(&mut self) -> Option<Run> {
        self.pending_stamp = None;
        self.run.take()
    }

    /// Drops the run's open commands and takes its state, for a failed run.
    fn abort_run(&mut self) -> Option<Run> {
        self.pending_stamp = None;
        self.board.borrow_mut().open.clear();
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
                present_upper_bound: self.present_upper_bound.clone(),
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

/// Stamps the paint of `surface` on every command applied so far that this
/// surface has not painted yet. The first paint in a drawn frame also arms
/// the settle callback for the next frame.
fn record_surface_paint(
    board: &RefCell<PaintBoard>,
    probe: &WeakEntity<PresenterProbe>,
    surface: Surface,
    window: &Window,
) {
    let now = Instant::now();
    let mut board = board.borrow_mut();
    #[cfg(test)]
    if hot_path::is_open() && std::mem::take(&mut board.violate_in_paint) {
        hot_path::note(HotPathKind::FileAccess);
    }
    for open in &mut board.open {
        let painted = &mut open.painted[slot(surface)];
        if painted.is_none() {
            *painted = Some(now);
        }
    }
    if !board.open.is_empty() && !board.settle_armed {
        board.settle_armed = true;
        let probe = probe.clone();
        // Registered during a draw, so it runs at the start of the next frame,
        // after this frame was presented.
        window.on_next_frame(move |window, cx| settle(&probe, window, cx));
    }
}

/// A surface's paint hook: a zero-size canvas laid over the panel. It paints
/// nothing and only stamps the surface's paint time.
fn surface_canvas(
    board: Rc<RefCell<PaintBoard>>,
    probe: WeakEntity<PresenterProbe>,
    surface: Surface,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _cx| record_surface_paint(&board, &probe, surface, window),
    )
    .absolute()
    .inset_0()
}

/// The settle callback for the frame after the one that drew the open
/// commands. It runs before that frame's draw, so the frame it settles has
/// already been presented. It closes the scopes, records the frame, and
/// dispatches the run's next command.
fn settle(probe: &WeakEntity<PresenterProbe>, window: &mut Window, cx: &mut App) {
    let Some(entity) = probe.upgrade() else {
        return;
    };
    let settled = entity.update(cx, |probe, _| probe.settle_paints(Instant::now()));
    if let Err(error) = settled {
        fail_run(&entity, error, cx);
        return;
    }
    entity.update(cx, |probe, _| {
        probe.drain_thumbnails();
        probe.request_thumbnail();
    });
    step(probe, window, cx);
}

/// Dispatches the run's next command through the real input path, or
/// finishes the run when no command is left.
fn step(probe: &WeakEntity<PresenterProbe>, window: &mut Window, cx: &mut App) {
    let command = match probe.update(cx, |probe, _| probe.next_command()) {
        Ok(Some(command)) => command,
        Ok(None) => return finish_run(probe, cx),
        Err(_) => return,
    };
    // The keydown stamp is taken immediately before the dispatch.
    let stamp = PendingKeydown::start(command_kind(command));
    if probe
        .update(cx, |probe, _| probe.pending_stamp = Some(stamp))
        .is_err()
    {
        return;
    }
    match keystroke_for(command) {
        Some(key) => {
            let keystroke = Keystroke::parse(key).expect("probe keys are valid keystrokes");
            if !window.dispatch_keystroke(keystroke, cx) {
                // A key no binding handles never applies, so the run would
                // wait for a settle that cannot come.
                fail_run_by_weak(probe, DriveError::Unhandled, cx);
            }
        }
        None => {
            if let PresenterCommand::GoTo(index) = command {
                window.dispatch_action(Box::new(ProbeGoTo(index)), cx);
            }
        }
    }
}

/// Ends a run with `error`, taking its callback through the probe entity.
fn fail_run_by_weak(probe: &WeakEntity<PresenterProbe>, error: DriveError, cx: &mut App) {
    if let Some(entity) = probe.upgrade() {
        fail_run(&entity, error, cx);
    }
}

/// Ends a run with `error` and delivers it to the drive's callback.
fn fail_run(entity: &Entity<PresenterProbe>, error: DriveError, cx: &mut App) {
    if let Some(run) = entity.update(cx, |probe, _| probe.abort_run()) {
        let done = run.done.clone();
        drop(run);
        finish_with(&done, Err(error), cx);
    }
}

/// Closes the run, then samples CPU over the idle window on a background
/// task. The outcome is delivered from that task.
fn finish_run(probe: &WeakEntity<PresenterProbe>, cx: &mut App) {
    // Without an open run there is nothing to finish, and nothing to count.
    let Ok(Some(run)) = probe.update(cx, |probe, _| probe.close_run()) else {
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
        done,
        window_closed,
        ..
    } = run;
    let probe = probe.clone();
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        // Held until the outcome is delivered, so the window-closed observer
        // stays subscribed during the idle sample.
        let _window_closed = window_closed;
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
        let result = match probe.update(cx, |probe, _| {
            probe.drain_thumbnails();
            probe.outcome(idle_sample, renderer, violations)
        }) {
            Ok(outcome) => Ok(outcome),
            Err(_) => Err(DriveError::WindowClosed),
        };
        cx.update(move |cx| finish_with(&done, result, cx));
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
        #[cfg(test)]
        if hot_path::is_open() && std::mem::take(&mut self.board.borrow_mut().violate_in_render) {
            hot_path::note(HotPathKind::FileAccess);
        }
        let state = *self.engine.state();
        let preview_lines = [
            cue_label(state.preview(), self.cue_count),
            format!("revision {}", state.revision()),
        ];
        let program_text = program_lines(&state, self.cue_count);
        let preview = surface_panel(
            "Preview",
            &preview_lines,
            surface_canvas(
                self.board.clone(),
                cx.entity().downgrade(),
                Surface::Preview,
            ),
        );
        let program = surface_panel(
            "Program",
            &program_text,
            surface_canvas(
                self.board.clone(),
                cx.entity().downgrade(),
                Surface::Program,
            ),
        );
        div()
            .track_focus(&self.focus)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &ProbeNext, _window, cx| {
                this.apply_command(PresenterCommand::Next, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbePrevious, _window, cx| {
                this.apply_command(PresenterCommand::Previous, cx)
            }))
            .on_action(cx.listener(|this, action: &ProbeGoTo, _window, cx| {
                this.apply_command(PresenterCommand::GoTo(action.0), cx)
            }))
            .on_action(cx.listener(|this, _: &ProbeTakeLive, _window, cx| {
                this.apply_command(PresenterCommand::TakeLive, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbeToggleFreeze, _window, cx| {
                this.apply_command(PresenterCommand::ToggleFreeze, cx)
            }))
            .on_action(cx.listener(|this, _: &ProbeToggleBlackout, _window, cx| {
                this.apply_command(PresenterCommand::ToggleBlackout, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .bg(rgb(0x18181c))
            .text_color(rgb(0xe8e8ee))
            .child(div().flex().gap_4().child(preview).child(program))
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

fn surface_panel(title: &str, lines: &[String], paint: impl IntoElement) -> impl IntoElement {
    div()
        .relative()
        .flex_1()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(rgb(0x26262e))
        .child(paint)
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

/// Drives `commands` through the probe window, one command per settled frame,
/// then idles for `idle` and delivers the outcome to `on_done`.
///
/// Every command goes through the real input path: keys through
/// `Window::dispatch_keystroke`, `GoTo` through `Window::dispatch_action`. The
/// next command waits for the previous frame to settle. The window must be
/// open and the bindings registered with [`register_keys`] or [`open`].
///
/// `on_done` receives `Err(DriveError::WindowClosed)` if the window closes
/// mid-run, `Err(DriveError::Unhandled)` or `Err(DriveError::NotPainted)` if a
/// command fails to apply or paint, and runs exactly once.
pub fn drive(
    window: WindowHandle<PresenterProbe>,
    commands: Vec<PresenterCommand>,
    idle: Duration,
    on_done: impl FnOnce(Result<ProbeOutcome, DriveError>, &mut App) + 'static,
    cx: &mut App,
) -> Result<(), DriveError> {
    let sampler = ProcessSampler::new().map_err(DriveError::Sampler)?;
    let done: DoneSlot = Rc::new(RefCell::new(Some(Box::new(on_done) as DoneCallback)));
    let window_id = window.window_id();
    let closed_done = done.clone();
    let window_closed = cx.on_window_closed(move |cx, closed| {
        if closed == window_id {
            finish_with(&closed_done, Err(DriveError::WindowClosed), cx);
        }
    });
    let probe = window
        .update(cx, |probe, window, cx| {
            probe.begin_run(commands, idle, sampler, done, window_closed, window, cx);
            cx.entity().downgrade()
        })
        .map_err(|_| DriveError::WindowClosed)?;
    cx.update_window(window.into(), |_, window, cx| step(&probe, window, cx))
        .map_err(|_| DriveError::WindowClosed)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::TestAppContext;

    use super::*;

    type Delivered = Rc<RefCell<Option<Result<ProbeOutcome, DriveError>>>>;

    /// One frame: the callbacks due this frame run, then the window draws.
    /// Returns how many callbacks ran.
    fn pump_frame(cx: &mut TestAppContext, window: WindowHandle<PresenterProbe>) -> usize {
        cx.update_window(window.into(), |_, window, cx| {
            let callbacks = window.simulate_next_frame(cx);
            let arena = window.draw(cx);
            arena.clear(cx);
            callbacks
        })
        .expect("probe window is open")
    }

    /// Pumps frames and the idle clock until the drive delivers a result.
    fn run_to_outcome(
        cx: &mut TestAppContext,
        window: WindowHandle<PresenterProbe>,
        commands: Vec<PresenterCommand>,
    ) -> Result<ProbeOutcome, DriveError> {
        let delivered: Delivered = Rc::new(RefCell::new(None));
        let sink = delivered.clone();
        let idle = Duration::from_secs(3);
        cx.update(|cx| {
            drive(
                window,
                commands,
                idle,
                move |result, _| *sink.borrow_mut() = Some(result),
                cx,
            )
            .expect("drive starts")
        });
        for _ in 0..1_000 {
            cx.run_until_parked();
            if delivered.borrow().is_some() {
                break;
            }
            if pump_frame(cx, window) == 0 {
                cx.executor().advance_clock(idle);
            }
        }
        let result = delivered.borrow_mut().take();
        result.expect("drive delivers a result")
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

        let outcome = run_to_outcome(cx, window, commands.clone()).expect("run completes");

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
                let bound = outcome
                    .report
                    .present_upper_bound
                    .get(&command)
                    .expect("upper bound recorded");
                assert_eq!(bound.total_recorded(), count, "{command:?} upper bound");
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
        let outcome = run_to_outcome(cx, window, commands.clone()).expect("run completes");

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
    fn notes_during_render_and_paint_are_counted_before_settle(cx: &mut TestAppContext) {
        cx.update(register_keys);
        let window = cx.add_window(|window, cx| PresenterProbe::new(3, window, cx));
        window
            .update(cx, |probe, _, _| {
                let mut board = probe.board.borrow_mut();
                board.violate_in_render = true;
                board.violate_in_paint = true;
            })
            .expect("probe window is open");

        let commands = vec![PresenterCommand::Next, PresenterCommand::TakeLive];
        let outcome = run_to_outcome(cx, window, commands.clone()).expect("run completes");

        // One note in the render and one in the paint of the first drawn frame,
        // both while the command's scope is still open.
        assert_eq!(outcome.report.hot_path_violations, 2);
        assert_eq!(outcome.final_state, oracle(3, &commands));
    }

    #[gpui::test]
    fn goto_is_one_action_with_one_navigation_sample(cx: &mut TestAppContext) {
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

        let outcome = run_to_outcome(cx, window, commands.clone()).expect("run completes");

        // Six navigation commands, GoTo counted once each: four GoTo, Previous, Next.
        let navigation_commands = commands
            .iter()
            .filter(|command| command_kind(**command) == CommandKind::Navigation)
            .count() as u64;
        for surface in Surface::ALL {
            let navigation = outcome
                .report
                .latency
                .histogram(CommandKind::Navigation, surface)
                .expect("navigation recorded");
            assert_eq!(navigation.total_recorded(), navigation_commands);
        }
        assert_eq!(navigation_commands, 6);
        assert_eq!(outcome.report.hot_path_violations, 0);
        assert_eq!(outcome.final_state, oracle(5, &commands));
    }

    #[gpui::test]
    fn closing_the_window_mid_run_delivers_window_closed(cx: &mut TestAppContext) {
        cx.update(register_keys);
        let window = cx.add_window(|window, cx| PresenterProbe::new(5, window, cx));
        let delivered: Delivered = Rc::new(RefCell::new(None));
        let sink = delivered.clone();
        let commands = vec![PresenterCommand::Next; 5];
        cx.update(|cx| {
            drive(
                window,
                commands,
                Duration::from_secs(3),
                move |result, _| *sink.borrow_mut() = Some(result),
                cx,
            )
            .expect("drive starts")
        });
        pump_frame(cx, window);
        cx.update_window(window.into(), |_, window, _| window.remove_window())
            .expect("probe window is open");
        cx.run_until_parked();

        let result = delivered.borrow_mut().take();
        assert!(
            matches!(result, Some(Err(DriveError::WindowClosed))),
            "the drive delivers WindowClosed, not silence"
        );
    }

    #[test]
    fn surface_slots_are_distinct_and_cover_every_surface() {
        assert_ne!(slot(Surface::Preview), slot(Surface::Program));
        for surface in Surface::ALL {
            assert!(slot(surface) < 2);
        }
    }
}
