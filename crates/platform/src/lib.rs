//! Native application, window, display, and task ownership adapters.
//!
//! The permanent platform Interface begins in T0.2. GPUI platform types stay
//! behind this crate's eventual seam rather than entering domain crates.

pub mod tasks;
pub mod windows;

#[cfg(test)]
mod tests {
    use super::tasks::{OwnedWorker, WorkerExit, WorkerFailure, WorkerKind};

    #[test]
    fn owned_worker_cancels_and_joins() {
        let mut worker = OwnedWorker::<()>::spawn(WorkerKind::Background, |cancel| {
            while !cancel.is_cancelled() {
                std::thread::yield_now();
            }
            Ok(WorkerExit::Cancelled)
        })
        .unwrap();

        assert_eq!(worker.cancel_and_join(), Ok(WorkerExit::Cancelled));
    }

    #[test]
    fn worker_failures_are_typed() {
        let mut failed = OwnedWorker::<()>::spawn(WorkerKind::Background, |_| {
            Err("controlled failure".to_owned())
        })
        .unwrap();
        let mut worker = OwnedWorker::<()>::spawn(WorkerKind::Storage, |_| panic!("boom")).unwrap();

        assert_eq!(
            failed.join(),
            Err(WorkerFailure::Failed {
                worker: WorkerKind::Background,
                message: "controlled failure".to_owned(),
            })
        );
        assert_eq!(
            worker.join(),
            Err(WorkerFailure::Panicked(WorkerKind::Storage))
        );
    }
}
