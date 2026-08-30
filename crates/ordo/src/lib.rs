use std::{
    cell::{Cell, RefCell},
    error::Error,
    fmt,
    rc::Rc,
    thread,
    time::Duration,
};

use gpui::{
    App, AppContext, Context, Render, Window, WindowHandle, div, prelude::*, px, rgb, size,
};
use platform::{
    tasks::{OwnedWorker, WorkerExit, WorkerKind, WorkerStartError},
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
    shutdown_orderly: Rc<Cell<bool>>,
}

impl NativeRuntime {
    pub fn operator(&self) -> WindowHandle<RootView> {
        self.operator
    }

    pub fn congregation(&self) -> Option<WindowHandle<CongregationView>> {
        self.congregation
    }

    pub fn shutdown_was_orderly(&self) -> bool {
        self.shutdown_orderly.get()
    }
}

struct RuntimeWorkers(Vec<OwnedWorker<()>>);

impl RuntimeWorkers {
    fn start() -> Result<Self, WorkerStartError> {
        let storage = controlled_worker(WorkerKind::Storage)?;
        let background = controlled_worker(WorkerKind::Background)?;
        Ok(Self(vec![storage, background]))
    }

    fn shutdown(&mut self) -> bool {
        let mut orderly = true;
        for worker in &mut self.0 {
            orderly &= worker.cancel_and_join().is_ok();
        }
        orderly
    }
}

fn controlled_worker(kind: WorkerKind) -> Result<OwnedWorker<()>, WorkerStartError> {
    OwnedWorker::spawn(kind, |cancel| {
        while !cancel.is_cancelled() {
            thread::park_timeout(Duration::from_millis(1));
        }
        Ok(WorkerExit::Cancelled)
    })
}

/// Opens the native operator runtime and, for Presenter harnesses, its passive window.
pub fn open_native_runtime(
    cx: &mut App,
    with_congregation: bool,
) -> Result<NativeRuntime, RuntimeError> {
    let workers = Rc::new(RefCell::new(RuntimeWorkers::start()?));
    let shutdown_orderly = Rc::new(Cell::new(false));
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

    let operator_id = operator.window_id();
    let congregation_handle = congregation;
    let shutdown_status = shutdown_orderly.clone();
    cx.on_window_closed(move |cx, closed| {
        if closed != operator_id {
            return;
        }
        if let Some(congregation) = congregation_handle {
            let _ = congregation.update(cx, |_, window, _| window.remove_window());
        }
        shutdown_status.set(workers.borrow_mut().shutdown());
        cx.quit();
    })
    .detach();

    Ok(NativeRuntime {
        operator,
        congregation,
        shutdown_orderly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_uses_the_ordo_version() {
        assert_eq!(
            BUILD_IDENTITY.strip_prefix("ordo/"),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
