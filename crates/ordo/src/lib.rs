use std::{
    cell::RefCell,
    error::Error,
    fmt,
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Sender},
    },
    thread,
};

use async_channel::Sender as AsyncSender;
use gpui::{
    App, AppContext, Context, Entity, Focusable, QuitMode, Render, StyledImage, Window,
    WindowHandle, div, img, prelude::*, px, rgb, size,
};
use output::{CongregationOutput, DisplaySource, GpuiDisplaySource};
use platform::{
    preferences::{PreferencesFile, installation_preferences_path},
    tasks::{OwnedWorker, WorkerExit, WorkerFailure, WorkerKind, WorkerStartError},
    windows::preparation_window_options,
};
use presentation::{CongregationState, ProgramFrame};
use shell::ShellView;
use ui::environment::UiPreferences;

pub mod canonical;
pub mod canonical_views;
pub mod diagnostics;
pub mod output;
pub mod performance;
pub mod settings;
pub mod shell;

pub const BUILD_IDENTITY: &str = concat!("ordo/", env!("CARGO_PKG_VERSION"));

/// Receives `true` when the pointer enters the congregation surface and
/// `false` when it leaves.
pub type PointerListener = Rc<dyn Fn(bool, &mut App)>;

/// Receives each mouse-button press on the congregation surface.
pub type PressListener = Rc<dyn Fn(&mut App)>;

/// Passive congregation surface: complete Program plus output status, nothing else.
///
/// The Program frame paints edge-to-edge with square corners: the `4px`
/// operator viewport radius never enters congregation output. Operator-only
/// chrome, diagnostics, focus handles, actions, and key handlers never mount
/// here. The only listeners are pointer enter/leave and press, reported to the
/// owner so the operator workspace can show its cursor locator and take focus
/// back after a click. Neither changes this surface. See [`output`] for why the
/// pointer itself cannot be hidden.
pub struct CongregationView {
    state: CongregationState,
    image: Option<Arc<gpui::Image>>,
    /// Revision of the Program frame `image` was encoded from.
    image_revision: Option<u64>,
    /// Bumped by every `set_state`. A background encode lands only while its
    /// generation is still current, so it can never repaint over a later
    /// state, even one whose Program reuses the same revision.
    generation: u64,
    on_pointer: Option<PointerListener>,
    /// The last pointer state reported through `on_pointer`.
    pointer_inside: bool,
    /// The window hover status render last saw. Element hover runs on the
    /// last known pointer position, which a fast exit off this fullscreen
    /// window leaves stale inside it, so element hover misses that leave and
    /// the next entry alike. The window's own hover status (Win32 mouse
    /// tracking) refreshes this view on each change, and render reports the
    /// change. Only changes count: GPUI's test window never reports hover, so
    /// element hover still drives tests.
    window_hovered: bool,
    on_press: Option<PressListener>,
}

impl CongregationView {
    pub fn new(state: CongregationState, cx: &mut Context<Self>) -> Self {
        Self::with_listeners(state, None, None, cx)
    }

    /// `on_pointer` receives `true` when the pointer enters this surface and
    /// `false` when it leaves. `on_press` receives every button press on it.
    pub fn with_listeners(
        state: CongregationState,
        on_pointer: Option<PointerListener>,
        on_press: Option<PressListener>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            state: state.clone(),
            image: None,
            image_revision: None,
            generation: 0,
            on_pointer,
            pointer_inside: false,
            window_hovered: false,
            on_press,
        };
        view.set_state(state, cx);
        view
    }

    /// Shows a new congregation state.
    ///
    /// Black Program applies immediately with no encode: the surface paints
    /// its black background. Content is encoded off the GPUI thread — the
    /// Program bytes (an `Arc` bump, no pixel copy) cross to the background
    /// executor for the RGBA copy plus `tiny-skia` PNG encode — and replaces
    /// the painted image only once complete, so no partial frame publishes.
    /// A superseded encode is dropped by generation, never by revision.
    pub fn set_state(&mut self, state: CongregationState, cx: &mut Context<Self>) {
        let program = state.program().clone();
        self.state = state;
        self.generation = self.generation.wrapping_add(1);
        if program.is_black() {
            self.image = None;
            self.image_revision = None;
            cx.notify();
            return;
        }
        let revision = program.revision();
        let generation = self.generation;
        let background = cx.background_spawn(async move { congregation_image(&program) });
        cx.spawn(async move |view, cx| {
            let image = background.await;
            view.update(cx, |view: &mut Self, cx| {
                if view.generation == generation {
                    view.image = Some(image);
                    view.image_revision = Some(revision);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn state(&self) -> &CongregationState {
        &self.state
    }

    /// The exact finished pixels this surface paints once the background
    /// encode lands: the congregation half of the milestone E2E compares
    /// these against the one rasterization. `None` while the surface paints
    /// black, either because Program is black or because the first content
    /// image is still encoding.
    pub fn image(&self) -> Option<&Arc<gpui::Image>> {
        self.image.as_ref()
    }

    /// Revision of the Program frame currently painted, `None` while black.
    pub fn painted_revision(&self) -> Option<u64> {
        self.image_revision
    }
}

/// Encode one Program frame to the PNG bytes the congregation `img` paints:
/// the same qualified `tiny-skia` edge the raster path encodes through, so
/// the surface shows the exact sampling the canonical worker composed. An
/// unencodable frame (unreachable behind `ProgramFrame`'s invariants) paints
/// nothing rather than a partial frame.
fn congregation_image(program: &ProgramFrame) -> Arc<gpui::Image> {
    crate::performance::hot_path::note(crate::performance::hot_path::HotPathKind::ImageDecode);
    let png = tiny_skia::IntSize::from_wh(program.width(), program.height())
        .and_then(|size| tiny_skia::Pixmap::from_vec(program.rgba().to_vec(), size))
        .and_then(|pixmap| pixmap.encode_png().ok())
        .unwrap_or_default();
    Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png))
}

impl CongregationView {
    /// Reports a pointer change once, deferred so render never updates the
    /// owner while this view is mid-update.
    fn report_pointer(&mut self, inside: bool, cx: &mut Context<Self>) {
        if self.pointer_inside == inside {
            return;
        }
        self.pointer_inside = inside;
        if let Some(on_pointer) = self.on_pointer.clone() {
            cx.defer(move |cx| on_pointer(inside, cx));
        }
    }
}

impl Render for CongregationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let window_hovered = window.is_window_hovered();
        if self.window_hovered != window_hovered {
            self.window_hovered = window_hovered;
            self.report_pointer(window_hovered, cx);
        }
        // Keyed on the revision actually painted, which can trail the state
        // while an encode is pending. A pending or black surface reports black.
        let painted = self.image_revision;
        let status = self.state.output_status();
        div()
            .id("congregation-root")
            .size_full()
            .bg(rgb(0x000000))
            .when(self.on_pointer.is_some(), |root| {
                root.on_hover(
                    cx.listener(|this, hovered: &bool, _, cx| this.report_pointer(*hovered, cx)),
                )
            })
            .when_some(self.on_press.clone(), |root, on_press| {
                root.on_any_mouse_down(move |_, _, cx| on_press(cx))
            })
            .child(
                div()
                    .debug_selector(move || match painted {
                        Some(revision) => format!("program-frame-{revision}"),
                        None => "program-frame-black".to_owned(),
                    })
                    .size_full()
                    // Black until the background encode lands: the pixels box
                    // always mounts for geometry, the `img` only with the
                    // complete image, so no partial frame ever publishes.
                    .child(
                        div()
                            .debug_selector(|| "congregation-frame-pixels".into())
                            .size_full()
                            .bg(rgb(0x000000))
                            .children(self.image.clone().map(|image| {
                                img(image).size_full().object_fit(gpui::ObjectFit::Contain)
                            })),
                    ),
            )
            .child(div().debug_selector(move || format!("output-status-{status:?}")))
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    Worker(WorkerStartError),
    PreferencesPath(std::io::Error),
    Ui(String),
    Window(&'static str),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Worker(error) => write!(
                formatter,
                "failed to start {:?} worker: {}",
                error.worker, error.reason
            ),
            Self::PreferencesPath(error) => {
                write!(
                    formatter,
                    "failed to locate installation preferences: {error}"
                )
            }
            Self::Ui(error) => write!(
                formatter,
                "failed to initialize the UI environment: {error}"
            ),
            Self::Window(role) => write!(formatter, "failed to open {role} window"),
        }
    }
}

impl Error for RuntimeError {}

impl From<WorkerStartError> for RuntimeError {
    fn from(error: WorkerStartError) -> Self {
        Self::Worker(error)
    }
}

pub struct NativeRuntime {
    operator: WindowHandle<ShellView>,
    output: Entity<CongregationOutput>,
    canonical_commands: Sender<canonical::CanonicalCommand>,
    shutdown_report: Rc<RefCell<Option<Vec<WorkerFailure>>>>,
}

impl NativeRuntime {
    pub fn operator(&self) -> WindowHandle<ShellView> {
        self.operator
    }

    /// The runtime-owned congregation-output controller. Startup leaves
    /// output disabled with no congregation window.
    pub fn output(&self) -> Entity<CongregationOutput> {
        self.output.clone()
    }

    /// The open congregation window, if output is enabled.
    pub fn congregation(&self, cx: &App) -> Option<WindowHandle<CongregationView>> {
        self.output.read(cx).window()
    }

    /// The runtime-owned canonical composition channel: snapshot operator
    /// state, send [`canonical::CanonicalCommand::Compose`], and receive the
    /// completed frame plus diagnostics the worker composed off the GPUI
    /// thread. Serialize only at the pin/handoff edge; PR 9's viewport
    /// consumers take the frame itself through this same channel.
    pub fn canonical_commands(&self) -> Sender<canonical::CanonicalCommand> {
        self.canonical_commands.clone()
    }

    pub fn shutdown_report(&self) -> Option<Vec<WorkerFailure>> {
        self.shutdown_report.borrow().clone()
    }
}

enum PreferenceCommand {
    Save(String),
    Shutdown,
}

struct RuntimeWorkers {
    preference_commands: Sender<PreferenceCommand>,
    canonical_commands: Sender<canonical::CanonicalCommand>,
    workers: Vec<OwnedWorker<()>>,
}

impl RuntimeWorkers {
    fn start(
        preferences: PreferencesFile,
        restored_preferences: AsyncSender<std::io::Result<Option<String>>>,
    ) -> Result<Self, WorkerStartError> {
        let (preference_commands, commands) = mpsc::channel();
        let (canonical_commands, canonical_requests) = mpsc::channel();
        let background = controlled_worker(WorkerKind::Background)?;
        let storage = OwnedWorker::spawn(WorkerKind::Storage, move |cancel| {
            // File access on this worker is never inside a GPUI-thread scope. The note
            // counts only if a read or write is moved onto the GPUI thread.
            crate::performance::hot_path::note(crate::performance::hot_path::HotPathKind::FileAccess);
            let _ = restored_preferences.send_blocking(preferences.read());
            while let Ok(PreferenceCommand::Save(contents)) = commands.recv() {
                crate::performance::hot_path::note(crate::performance::hot_path::HotPathKind::FileAccess);
                preferences
                    .write(&contents)
                    .map_err(|error| error.to_string())?;
            }
            Ok(if cancel.is_cancelled() {
                WorkerExit::Cancelled
            } else {
                WorkerExit::Completed(())
            })
        })?;
        let canonical = match OwnedWorker::spawn(WorkerKind::Canonical, move |cancel| {
            canonical::serve_canonical(canonical_requests, cancel)
        }) {
            Ok(worker) => worker,
            Err(error) => {
                // Roll back through the tested shutdown path: the storage
                // worker is blocked in `recv`, which cancel/unpark cannot
                // wake while this sender is live, so only the Shutdown
                // message unblocks it before its join. Never rely on drop
                // order here — joining a still-blocked worker hangs instead
                // of returning the startup error.
                let mut partial = Self {
                    preference_commands,
                    canonical_commands,
                    workers: vec![storage, background],
                };
                partial.shutdown();
                return Err(error);
            }
        };
        Ok(Self {
            preference_commands,
            canonical_commands,
            workers: vec![storage, background, canonical],
        })
    }

    fn shutdown(&mut self) -> Vec<WorkerFailure> {
        for worker in &self.workers {
            worker.cancel();
        }
        let _ = self.preference_commands.send(PreferenceCommand::Shutdown);
        let _ = self
            .canonical_commands
            .send(canonical::CanonicalCommand::Stop);
        let mut failures = Vec::new();
        for worker in &mut self.workers {
            if let Err(failure) = worker.join() {
                failures.push(failure);
            }
        }
        failures
    }
}

impl Drop for RuntimeWorkers {
    fn drop(&mut self) {
        let _ = self.preference_commands.send(PreferenceCommand::Shutdown);
        let _ = self
            .canonical_commands
            .send(canonical::CanonicalCommand::Stop);
    }
}

fn controlled_worker(kind: WorkerKind) -> Result<OwnedWorker<()>, WorkerStartError> {
    OwnedWorker::spawn(kind, |cancel| {
        while !cancel.is_cancelled() {
            thread::park();
        }
        Ok(WorkerExit::Cancelled)
    })
}

/// Opens the native operator runtime. No congregation window opens until
/// output is explicitly enabled through [`NativeRuntime::output`].
pub fn open_native_runtime(cx: &mut App) -> Result<NativeRuntime, RuntimeError> {
    let preferences_path =
        installation_preferences_path().map_err(RuntimeError::PreferencesPath)?;
    open_native_runtime_at(cx, preferences_path)
}

/// Testable runtime entry point whose installation preferences never need a real user path.
pub fn open_native_runtime_at(
    cx: &mut App,
    preferences_path: impl AsRef<Path>,
) -> Result<NativeRuntime, RuntimeError> {
    open_native_runtime_with(cx, preferences_path, GpuiDisplaySource)
}

/// Runtime entry point with an injected display source, so display tests
/// drive an in-memory topology while production reads GPUI.
pub fn open_native_runtime_with(
    cx: &mut App,
    preferences_path: impl AsRef<Path>,
    displays: impl DisplaySource + 'static,
) -> Result<NativeRuntime, RuntimeError> {
    // Ordo owns exit on Windows/Linux; explicit mode also makes startup rollback recoverable.
    cx.set_quit_mode(QuitMode::Explicit);
    ui::init(cx).map_err(RuntimeError::Ui)?;
    let (restore_tx, restore_rx) = async_channel::bounded(1);
    let workers =
        RuntimeWorkers::start(PreferencesFile::new(preferences_path.as_ref()), restore_tx)?;
    let workers = Rc::new(RefCell::new(workers));
    let canonical_commands = workers.borrow().canonical_commands.clone();
    let save_preferences = {
        let commands = workers.borrow().preference_commands.clone();
        Rc::new(move |preferences: UiPreferences| {
            crate::performance::hot_path::note(
                crate::performance::hot_path::HotPathKind::PreferenceSave,
            );
            if commands
                .send(PreferenceCommand::Save(preferences.encode()))
                .is_err()
            {
                eprintln!("preferences worker stopped before a change could be saved");
            }
        })
    };
    let operator_bounds = gpui::Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
    let operator = cx
        .open_window(preparation_window_options(operator_bounds), |window, cx| {
            cx.new(|cx| {
                let root = ShellView::with_preferences(
                    BUILD_IDENTITY,
                    UiPreferences::default(),
                    Some(save_preferences),
                    window,
                    cx,
                );
                window.focus(&root.focus_handle(cx), cx);
                root
            })
        })
        .map_err(|_| RuntimeError::Window("operator"))?;

    cx.spawn({
        let restore_target = operator;
        async move |cx| {
            let restored = restore_rx.recv().await.ok();
            let restored = match restored {
                Some(Ok(Some(contents))) => match UiPreferences::decode(&contents) {
                    Ok(preferences) => Some(preferences),
                    Err(error) => {
                        eprintln!("ignored invalid installation preferences: {error}");
                        None
                    }
                },
                Some(Ok(None)) => None,
                Some(Err(error)) => {
                    eprintln!("failed to restore installation preferences: {error}");
                    None
                }
                None => {
                    eprintln!("preferences worker stopped before startup restore");
                    None
                }
            };
            let _ = restore_target.update(cx, |root, _, cx| {
                root.finish_preferences_restore(restored, cx);
            });
        }
    })
    .detach();

    let output =
        cx.new(|cx| CongregationOutput::new(Box::new(displays), Some(operator.into()), cx));
    if operator
        .update(cx, |root, _, cx| root.follow_output(&output, cx))
        .is_err()
    {
        // Startup failed after the workers started: stop them before reporting.
        workers.borrow_mut().shutdown();
        return Err(RuntimeError::Window("operator"));
    }

    let shutdown_report = Rc::new(RefCell::new(None));

    let operator_id = operator.window_id();
    let closing_output = output.clone();
    let shutdown_status = shutdown_report.clone();
    cx.on_window_closed(move |cx, closed| {
        if closed != operator_id {
            // Window-closed observers run synchronously inside whatever update
            // removed the window, which may be the controller's own; defer so
            // the controller is never re-entered.
            let output = closing_output.clone();
            cx.defer(move |cx| output.update(cx, |output, cx| output.window_closed(closed, cx)));
            return;
        }
        closing_output.update(cx, |output, cx| output.shutdown(cx));
        let report = workers.borrow_mut().shutdown();
        for failure in &report {
            eprintln!("worker shutdown failed: {failure:?}");
        }
        shutdown_status.replace(Some(report));
        cx.quit();
    })
    .detach();

    Ok(NativeRuntime {
        operator,
        output,
        canonical_commands,
        shutdown_report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use presentation::{OutputStatus, PresentationCommand, PresentationEngine};
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn raster_feeds_live_program_with_its_dimensions() {
        use std::collections::HashMap;

        use rendering::{TargetDensity, TemplateContent, TemplateKind, Theme};

        // One mixed-language fixture through the real path: compose once per
        // density, rasterize the completed frame, and feed the Program seam.
        let content = TemplateContent {
            kind: TemplateKind::Lyrics,
            header: Some("Section label".to_owned()),
            body: "Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo — awit 95 α 中".to_owned(),
            footer: None,
            image: None,
        };
        let theme = Theme::default();
        let images: HashMap<String, Vec<u8>> = HashMap::new();

        let light_density = TargetDensity::from_percent(100).unwrap();
        let dense_density = TargetDensity::from_percent(200).unwrap();
        let (at_light, _) = rendering::render_template(&content, &theme, light_density).unwrap();
        let (at_dense, _) = rendering::render_template(&content, &theme, dense_density).unwrap();
        assert_eq!(at_light.serialize(), at_dense.serialize());

        let raster_light = rendering::rasterize(&at_light, light_density, &images).unwrap();
        let raster_dense = rendering::rasterize(&at_light, dense_density, &images).unwrap();
        // Ground truth for the byte-intact checks below; the seam itself
        // moves its buffers without copying them.
        let expected_light = raster_light.rgba().to_vec();
        let expected_dense = raster_dense.rgba().to_vec();
        let program_light = ProgramFrame::from_raster(1, raster_light).unwrap();
        let program_dense = ProgramFrame::from_raster(2, raster_dense).unwrap();

        // The density-dependent extent survives the seam instead of collapsing
        // into an undimensioned byte buffer.
        assert_eq!(
            (program_light.width(), program_light.height()),
            (1920, 1080)
        );
        assert_eq!(
            (program_dense.width(), program_dense.height()),
            (3840, 2160)
        );
        assert_eq!(program_light.rgba(), expected_light.as_slice());
        assert_eq!(program_dense.rgba(), expected_dense.as_slice());

        // Both densities ride the live engine with their dimensions intact.
        let mut engine = PresentationEngine::new(program_light, OutputStatus::Connected);
        let state = engine.apply(PresentationCommand::TakeLive(program_dense));
        assert_eq!(state.program().revision(), 2);
        assert_eq!(
            (state.program().width(), state.program().height()),
            (3840, 2160)
        );
        assert_eq!(state.program().rgba(), expected_dense.as_slice());
        assert_eq!(state.output_status(), OutputStatus::Connected);
    }

    #[test]
    fn build_identity_uses_the_ordo_version() {
        assert_eq!(
            BUILD_IDENTITY.strip_prefix("ordo/"),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn shutdown_cancels_every_worker_before_joining() {
        let (background_cancelled_tx, background_cancelled_rx) = mpsc::channel();
        let storage = OwnedWorker::spawn(WorkerKind::Storage, move |cancel| {
            while !cancel.is_cancelled() {
                thread::park();
            }
            background_cancelled_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| "background was not cancelled before storage joined".to_owned())?;
            Ok(WorkerExit::Cancelled)
        })
        .unwrap();
        let background = OwnedWorker::spawn(WorkerKind::Background, move |cancel| {
            while !cancel.is_cancelled() {
                thread::park();
            }
            background_cancelled_tx.send(()).unwrap();
            Ok(WorkerExit::Cancelled)
        })
        .unwrap();

        let (commands, _) = mpsc::channel();
        let (canonical_commands, _) = mpsc::channel();
        let report = RuntimeWorkers {
            preference_commands: commands,
            canonical_commands,
            workers: vec![storage, background],
        }
        .shutdown();

        assert!(report.is_empty());
    }

    #[test]
    fn shutdown_unblocks_channel_workers_with_live_senders() {
        // Startup-rollback coverage: `start`'s failure path delegates to
        // `shutdown`, which must end every channel-blocked worker while
        // senders stay live. Cancel/unpark cannot wake `recv`; without each
        // stop message this times out instead of hanging the suite the way
        // failed startup would.
        let preferences =
            std::env::temp_dir().join(format!("ordo-shutdown-rollback-{}", std::process::id()));
        let (restore_tx, _restore_rx) = async_channel::bounded(1);
        let mut workers = RuntimeWorkers::start(PreferencesFile::new(&preferences), restore_tx)
            .expect("workers must start");
        let live_preferences = workers.preference_commands.clone();
        let live_canonical = workers.canonical_commands.clone();
        // Join off-thread with a timeout so a regression fails instead of
        // hanging the suite the way the startup path would.
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = done_tx.send(workers.shutdown());
        });
        let report = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("shutdown must unblock channel-blocked workers while senders stay live");
        assert!(report.is_empty());
        drop((live_preferences, live_canonical));
        std::fs::remove_file(preferences).ok();
    }

    #[test]
    fn shutdown_preserves_typed_worker_failures() {
        let failed =
            OwnedWorker::spawn(
                WorkerKind::Storage,
                |_| Err("controlled failure".to_owned()),
            )
            .unwrap();

        let (commands, _) = mpsc::channel();
        let (canonical_commands, _) = mpsc::channel();
        let report = RuntimeWorkers {
            preference_commands: commands,
            canonical_commands,
            workers: vec![failed],
        }
        .shutdown();

        assert_eq!(
            report,
            &[WorkerFailure::Failed {
                worker: WorkerKind::Storage,
                message: "controlled failure".to_owned(),
            }]
        );
    }

    #[test]
    fn dropping_workers_stops_storage_with_a_surviving_sender() {
        let (commands, received_commands) = mpsc::channel();
        let storage = OwnedWorker::spawn(WorkerKind::Storage, move |_| {
            while let Ok(PreferenceCommand::Save(_)) = received_commands.recv() {}
            Ok(WorkerExit::Completed(()))
        })
        .unwrap();
        let (canonical_commands, _) = mpsc::channel();
        let workers = RuntimeWorkers {
            preference_commands: commands,
            canonical_commands,
            workers: vec![storage],
        };
        let surviving_sender = workers.preference_commands.clone();
        let (dropped_tx, dropped_rx) = mpsc::channel();

        thread::spawn(move || {
            drop(workers);
            dropped_tx.send(()).unwrap();
        });

        let result = dropped_rx.recv_timeout(Duration::from_secs(1));
        drop(surviving_sender);
        assert!(result.is_ok(), "workers deadlocked while a sender survived");
    }
}
