//! Native application, window, display, and task ownership adapters.
//!
//! The permanent platform Interface begins in T0.2. GPUI platform types stay
//! behind this crate's eventual seam rather than entering domain crates.

pub mod preferences;
pub mod tasks;
pub mod windows;

#[cfg(test)]
mod tests {
    use super::tasks::{OwnedWorker, WorkerExit, WorkerFailure, WorkerKind};
    use std::{sync::mpsc, time::Duration};

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

    #[test]
    fn cancellation_wakes_a_parked_worker() {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (cancelled_tx, cancelled_rx) = mpsc::channel();
        let mut worker = OwnedWorker::<()>::spawn(WorkerKind::Background, move |cancel| {
            ready_tx.send(()).unwrap();
            std::thread::park_timeout(Duration::from_secs(1));
            cancelled_tx.send(cancel.is_cancelled()).unwrap();
            Ok(WorkerExit::Cancelled)
        })
        .unwrap();
        ready_rx.recv().unwrap();

        worker.cancel();

        assert_eq!(cancelled_rx.recv_timeout(Duration::from_secs(5)), Ok(true));
        assert_eq!(worker.join(), Ok(WorkerExit::Cancelled));
    }
}
