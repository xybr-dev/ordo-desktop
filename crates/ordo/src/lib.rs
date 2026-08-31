use std::{
    cell::RefCell,
    error::Error,
    fmt,
    path::Path,
    rc::Rc,
    sync::mpsc::{self, Sender},
    thread,
};

use async_channel::Sender as AsyncSender;
use gpui::{
    App, AppContext, Context, Focusable, QuitMode, Render, Window, WindowHandle, div, prelude::*,
    px, rgb, size,
};
use platform::{
    preferences::{PreferencesFile, installation_preferences_path},
    tasks::{OwnedWorker, WorkerExit, WorkerFailure, WorkerKind, WorkerStartError},
    windows::{preparation_window_options, presenter_window_options},
};
use presentation::{CongregationState, OutputStatus, ProgramFrame};
use ui::{RootView, environment::UiPreferences};

pub const BUILD_IDENTITY: &str = concat!("ordo/", env!("CARGO_PKG_VERSION"));

/// Passive congregation surface: complete Program plus output status, nothing else.
pub struct CongregationView {
    state: CongregationState,
}

impl CongregationView {
    pub fn new(state: CongregationState, _: &mut Context<Self>) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &CongregationState {
        &self.state
    }
}

impl Render for CongregationView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let revision = self.state.program().revision();
        let status = self.state.output_status();
        div()
            .id("congregation-root")
            .size_full()
            .bg(rgb(0x000000))
            .child(div().debug_selector(move || format!("program-frame-{revision}")))
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
    operator: WindowHandle<RootView>,
    congregation: Option<WindowHandle<CongregationView>>,
    shutdown_report: Rc<RefCell<Option<Vec<WorkerFailure>>>>,
}

impl NativeRuntime {
    pub fn operator(&self) -> WindowHandle<RootView> {
        self.operator
    }

    pub fn congregation(&self) -> Option<WindowHandle<CongregationView>> {
        self.congregation
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
    workers: Vec<OwnedWorker<()>>,
}

impl RuntimeWorkers {
    fn start(
        preferences: PreferencesFile,
        restored_preferences: AsyncSender<std::io::Result<Option<String>>>,
    ) -> Result<Self, WorkerStartError> {
        let (preference_commands, commands) = mpsc::channel();
        let background = controlled_worker(WorkerKind::Background)?;
        let storage = OwnedWorker::spawn(WorkerKind::Storage, move |cancel| {
            let _ = restored_preferences.send_blocking(preferences.read());
            while let Ok(PreferenceCommand::Save(contents)) = commands.recv() {
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
        Ok(Self {
            preference_commands,
            workers: vec![storage, background],
        })
    }

    fn shutdown(&mut self) -> Vec<WorkerFailure> {
        for worker in &self.workers {
            worker.cancel();
        }
        let _ = self.preference_commands.send(PreferenceCommand::Shutdown);
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

/// Opens the native operator runtime and, for Presenter harnesses, its passive window.
pub fn open_native_runtime(
    cx: &mut App,
    with_congregation: bool,
) -> Result<NativeRuntime, RuntimeError> {
    let preferences_path =
        installation_preferences_path().map_err(RuntimeError::PreferencesPath)?;
    open_native_runtime_at(cx, with_congregation, preferences_path)
}

/// Testable runtime entry point whose installation preferences never need a real user path.
pub fn open_native_runtime_at(
    cx: &mut App,
    with_congregation: bool,
    preferences_path: impl AsRef<Path>,
) -> Result<NativeRuntime, RuntimeError> {
    // Ordo owns exit on Windows/Linux; explicit mode also makes startup rollback recoverable.
    cx.set_quit_mode(QuitMode::Explicit);
    ui::init(cx).map_err(RuntimeError::Ui)?;
    let (restore_tx, restore_rx) = async_channel::bounded(1);
    let workers =
        RuntimeWorkers::start(PreferencesFile::new(preferences_path.as_ref()), restore_tx)?;
    let workers = Rc::new(RefCell::new(workers));
    let save_preferences = {
        let commands = workers.borrow().preference_commands.clone();
        Rc::new(move |preferences: UiPreferences| {
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
                let root = RootView::with_preferences(
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

    let congregation = if with_congregation {
        let bounds = gpui::Bounds::centered(None, size(px(1920.0), px(1080.0)), cx);
        let state = CongregationState::new(
            ProgramFrame::new(0, vec![0, 0, 0, 255]).expect("black frame is complete"),
            OutputStatus::Connected,
        );
        match cx.open_window(presenter_window_options(bounds), |_, cx| {
            cx.new(|cx| CongregationView::new(state, cx))
        }) {
            Ok(window) => Some(window),
            Err(_) => {
                let _ = operator.update(cx, |_, window, _| window.remove_window());
                return Err(RuntimeError::Window("congregation"));
            }
        }
    } else {
        None
    };

    let shutdown_report = Rc::new(RefCell::new(None));

    let operator_id = operator.window_id();
    let congregation_handle = congregation;
    let shutdown_status = shutdown_report.clone();
    cx.on_window_closed(move |cx, closed| {
        if closed != operator_id {
            return;
        }
        if let Some(congregation) = congregation_handle {
            let _ = congregation.update(cx, |_, window, _| window.remove_window());
        }
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
        congregation,
        shutdown_report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

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
        let report = RuntimeWorkers {
            preference_commands: commands,
            workers: vec![storage, background],
        }
        .shutdown();

        assert!(report.is_empty());
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
        let report = RuntimeWorkers {
            preference_commands: commands,
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
        let workers = RuntimeWorkers {
            preference_commands: commands,
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
