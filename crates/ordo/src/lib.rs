use std::{cell::RefCell, error::Error, fmt, rc::Rc, thread};

use gpui::{
    App, AppContext, Context, QuitMode, Render, Window, WindowHandle, div, prelude::*, px, rgb,
    size,
};
use platform::{
    tasks::{OwnedWorker, WorkerExit, WorkerFailure, WorkerKind, WorkerStartError},
    windows::{preparation_window_options, presenter_window_options},
};
use presentation::{CongregationState, OutputStatus, ProgramFrame};
use ui::RootView;

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

struct RuntimeWorkers(Vec<OwnedWorker<()>>);

impl RuntimeWorkers {
    fn start() -> Result<Self, WorkerStartError> {
        let storage = controlled_worker(WorkerKind::Storage)?;
        let background = controlled_worker(WorkerKind::Background)?;
        Ok(Self(vec![storage, background]))
    }

    fn shutdown(&mut self) -> Vec<WorkerFailure> {
        for worker in &self.0 {
            worker.cancel();
        }
        let mut failures = Vec::new();
        for worker in &mut self.0 {
            if let Err(failure) = worker.join() {
                failures.push(failure);
            }
        }
        failures
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
    // Ordo owns exit on Windows/Linux; explicit mode also makes startup rollback recoverable.
    cx.set_quit_mode(QuitMode::Explicit);
    let operator_bounds = gpui::Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
    let operator = cx
        .open_window(preparation_window_options(operator_bounds), |_, cx| {
            cx.new(|cx| RootView::new(BUILD_IDENTITY, cx))
        })
        .map_err(|_| RuntimeError::Window("operator"))?;

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

    let workers = match RuntimeWorkers::start() {
        Ok(workers) => Rc::new(RefCell::new(workers)),
        Err(error) => {
            if let Some(congregation) = congregation {
                let _ = congregation.update(cx, |_, window, _| window.remove_window());
            }
            let _ = operator.update(cx, |_, window, _| window.remove_window());
            return Err(error.into());
        }
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

        let report = RuntimeWorkers(vec![storage, background]).shutdown();

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

        let report = RuntimeWorkers(vec![failed]).shutdown();

        assert_eq!(
            report,
            &[WorkerFailure::Failed {
                worker: WorkerKind::Storage,
                message: "controlled failure".to_owned(),
            }]
        );
    }
}
