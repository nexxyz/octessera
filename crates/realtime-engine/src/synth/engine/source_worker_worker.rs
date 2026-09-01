use super::{CompletedEnvelope, WorkEnvelope, WorkerExit, SOURCE_WORKER_CHANNEL_CAPACITY};
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub(crate) struct SourceWorkerSlot {
    pub(crate) work_tx: Option<Sender<WorkEnvelope>>,
    pub(crate) done_rx: Receiver<CompletedEnvelope>,
    pub(crate) done_tx: Option<Sender<CompletedEnvelope>>,
    pub(crate) ready_rx: Receiver<()>,
    pub(crate) exited: Arc<AtomicBool>,
    #[cfg(test)]
    pub(super) jobs_started: Arc<AtomicU64>,
    #[cfg(test)]
    pub(super) pause: Arc<AtomicBool>,
    #[cfg(test)]
    pub(super) exit_on_job: Arc<AtomicBool>,
    #[cfg(test)]
    pub(super) hold_before_receive: Arc<AtomicBool>,
    #[cfg(test)]
    pub(super) panic_on_job: Arc<AtomicBool>,
    pub(super) join: Option<JoinHandle<WorkerExit>>,
}

pub(super) struct ReverseCompletionState {
    pub(super) enabled: AtomicBool,
    pub(super) parity_one_done: AtomicBool,
}

struct SourceWorkerThreadState {
    exited: Arc<AtomicBool>,
    jobs_started: Arc<AtomicU64>,
    #[cfg(test)]
    pause: Arc<AtomicBool>,
    #[cfg(test)]
    exit_on_job: Arc<AtomicBool>,
    #[cfg(test)]
    hold_before_receive: Arc<AtomicBool>,
    #[cfg(test)]
    panic_on_job: Arc<AtomicBool>,
    reverse_completion: Arc<ReverseCompletionState>,
}

pub(super) fn spawn_worker(
    parity: usize,
    reverse_completion: Arc<ReverseCompletionState>,
    hold_before_receive: bool,
) -> SourceWorkerSlot {
    let (work_tx, work_rx) = bounded(SOURCE_WORKER_CHANNEL_CAPACITY);
    let (done_tx, done_rx) = bounded(SOURCE_WORKER_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = bounded(1);
    let exited = Arc::new(AtomicBool::new(false));
    let jobs_started = Arc::new(AtomicU64::new(0));
    #[cfg(test)]
    let pause = Arc::new(AtomicBool::new(false));
    #[cfg(test)]
    let exit_on_job = Arc::new(AtomicBool::new(false));
    #[cfg(test)]
    let hold_before_receive = Arc::new(AtomicBool::new(hold_before_receive));
    #[cfg(not(test))]
    let _ = hold_before_receive;
    #[cfg(test)]
    let panic_on_job = Arc::new(AtomicBool::new(false));
    let worker_exited = Arc::clone(&exited);
    let worker_jobs = Arc::clone(&jobs_started);
    #[cfg(test)]
    let worker_pause = Arc::clone(&pause);
    #[cfg(test)]
    let worker_exit_on_job = Arc::clone(&exit_on_job);
    #[cfg(test)]
    let worker_hold_before_receive = Arc::clone(&hold_before_receive);
    #[cfg(test)]
    let worker_panic_on_job = Arc::clone(&panic_on_job);
    let worker_done_tx = done_tx.clone();
    let lifecycle_done_tx = done_tx.clone();
    let join = thread::spawn(move || {
        let _ = ready_tx.send(());
        worker_loop(
            parity,
            work_rx,
            worker_done_tx,
            SourceWorkerThreadState {
                exited: worker_exited,
                jobs_started: worker_jobs,
                #[cfg(test)]
                pause: worker_pause,
                #[cfg(test)]
                exit_on_job: worker_exit_on_job,
                #[cfg(test)]
                hold_before_receive: worker_hold_before_receive,
                #[cfg(test)]
                panic_on_job: worker_panic_on_job,
                reverse_completion,
            },
        )
    });
    SourceWorkerSlot {
        work_tx: Some(work_tx),
        done_rx,
        done_tx: Some(lifecycle_done_tx),
        ready_rx,
        exited,
        #[cfg(test)]
        jobs_started,
        #[cfg(test)]
        pause,
        #[cfg(test)]
        exit_on_job,
        #[cfg(test)]
        hold_before_receive,
        #[cfg(test)]
        panic_on_job,
        join: Some(join),
    }
}

fn worker_loop(
    parity: usize,
    work_rx: Receiver<WorkEnvelope>,
    done_tx: Sender<CompletedEnvelope>,
    state: SourceWorkerThreadState,
) -> WorkerExit {
    #[cfg(test)]
    while state.hold_before_receive.load(Ordering::Acquire) {
        std::hint::spin_loop();
    }
    while let Ok(mut work) = work_rx.recv() {
        state.jobs_started.fetch_add(1, Ordering::Relaxed);
        #[cfg(test)]
        if state.exit_on_job.swap(false, Ordering::AcqRel) {
            return finish_worker(
                &state,
                send_completion(&done_tx, CompletedEnvelope::from_work(work, true, false)),
            );
        }
        #[cfg(test)]
        while state.pause.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        #[cfg(test)]
        let should_panic = state.panic_on_job.swap(false, Ordering::AcqRel);
        #[cfg(not(test))]
        let should_panic = false;
        let render_ok = catch_unwind(AssertUnwindSafe(|| {
            if should_panic {
                panic!("source worker test panic");
            }
            work.render()
        }))
        .unwrap_or(false);
        if !render_ok {
            return finish_worker(
                &state,
                send_completion(&done_tx, CompletedEnvelope::from_work(work, true, false)),
            );
        }
        if state.reverse_completion.enabled.load(Ordering::Acquire) && parity == 0 {
            while !state
                .reverse_completion
                .parity_one_done
                .load(Ordering::Acquire)
            {
                std::hint::spin_loop();
            }
        }
        if state.reverse_completion.enabled.load(Ordering::Acquire) && parity == 1 {
            state
                .reverse_completion
                .parity_one_done
                .store(true, Ordering::Release);
        }
        if let Some(exit) =
            send_completion(&done_tx, CompletedEnvelope::from_work(work, false, true))
        {
            state.exited.store(true, Ordering::Release);
            return exit;
        }
    }
    state.exited.store(true, Ordering::Release);
    WorkerExit {
        unsent_completion: None,
    }
}

fn send_completion(
    done_tx: &Sender<CompletedEnvelope>,
    completion: CompletedEnvelope,
) -> Option<WorkerExit> {
    match done_tx.try_send(completion) {
        Ok(()) => None,
        Err(TrySendError::Full(completion) | TrySendError::Disconnected(completion)) => {
            Some(WorkerExit {
                unsent_completion: Some(CompletedEnvelope {
                    transport_failed: true,
                    ..completion
                }),
            })
        }
    }
}

fn finish_worker(state: &SourceWorkerThreadState, exit: Option<WorkerExit>) -> WorkerExit {
    state.exited.store(true, Ordering::Release);
    exit.unwrap_or(WorkerExit {
        unsent_completion: None,
    })
}

impl CompletedEnvelope {
    fn from_work(work: WorkEnvelope, worker_exited: bool, render_ok: bool) -> Self {
        Self {
            owner: work.owner,
            sequence: work.sequence,
            frames: work.frames,
            base_sample_clock: work.base_sample_clock,
            render_ok,
            worker_exited,
            transport_failed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    fn completion(parity: usize) -> CompletedEnvelope {
        CompletedEnvelope {
            owner: super::super::owner_for_test(parity),
            sequence: 1,
            frames: 128,
            base_sample_clock: 0,
            render_ok: true,
            worker_exited: false,
            transport_failed: false,
        }
    }

    #[test]
    fn full_completion_is_preserved_in_worker_exit() {
        let (done_tx, done_rx) = bounded(1);
        done_tx.try_send(completion(0)).expect("queued completion");
        let exit = send_completion(&done_tx, completion(0)).expect("worker exit");
        assert!(exit.unsent_completion.is_some());
        assert!(done_rx.try_recv().is_ok());
    }

    #[test]
    fn disconnected_completion_is_preserved_in_worker_exit() {
        let (done_tx, done_rx) = bounded(1);
        drop(done_rx);
        let exit = send_completion(&done_tx, completion(1)).expect("worker exit");
        assert!(exit.unsent_completion.is_some());
    }
}
