use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerKind {
    Storage,
    Background,
}

impl WorkerKind {
    fn thread_name(self) -> &'static str {
        match self {
            Self::Storage => "ordo-storage",
            Self::Background => "ordo-background",
        }
    }
}

#[derive(Clone)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum WorkerExit<T> {
    Completed(T),
    Cancelled,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WorkerFailure {
    Failed { worker: WorkerKind, message: String },
    Panicked(WorkerKind),
    AlreadyJoined(WorkerKind),
}

#[derive(Debug, Eq, PartialEq)]
pub struct WorkerStartError {
    pub worker: WorkerKind,
    pub reason: io::ErrorKind,
}

/// Named owner for one cooperative long-lived worker.
pub struct OwnedWorker<T> {
    kind: WorkerKind,
    cancelled: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<WorkerExit<T>, String>>>,
}

impl<T: Send + 'static> OwnedWorker<T> {
    pub fn spawn(
        kind: WorkerKind,
        work: impl FnOnce(CancellationToken) -> Result<WorkerExit<T>, String> + Send + 'static,
    ) -> Result<Self, WorkerStartError> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let token = CancellationToken(cancelled.clone());
        let thread = thread::Builder::new()
            .name(kind.thread_name().to_owned())
            .spawn(move || work(token))
            .map_err(|error| WorkerStartError {
                worker: kind,
                reason: error.kind(),
            })?;
        Ok(Self {
            kind,
            cancelled,
            thread: Some(thread),
        })
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn cancel_and_join(&mut self) -> Result<WorkerExit<T>, WorkerFailure> {
        self.cancel();
        self.join()
    }

    pub fn join(&mut self) -> Result<WorkerExit<T>, WorkerFailure> {
        let Some(thread) = self.thread.take() else {
            return Err(WorkerFailure::AlreadyJoined(self.kind));
        };
        match thread.join() {
            Ok(Ok(exit)) => Ok(exit),
            Ok(Err(message)) => Err(WorkerFailure::Failed {
                worker: self.kind,
                message,
            }),
            Err(_) => Err(WorkerFailure::Panicked(self.kind)),
        }
    }
}

impl<T> Drop for OwnedWorker<T> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
