use super::super::source_worker_protocol::{RoutingTreeWork, SourceWorkerStartHook};
use super::{CompletedEnvelope, WorkerExit, SOURCE_WORKER_CHANNEL_CAPACITY};
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

mod routing_tree;

pub(crate) struct SourceWorkerSlot {
    pub(crate) work_tx: Option<Sender<RoutingTreeWork>>,
    pub(crate) done_rx: Receiver<CompletedEnvelope>,
    pub(crate) done_tx: Option<Sender<CompletedEnvelope>>,
    pub(crate) ready_rx:
        Receiver<Result<(), super::super::source_worker_protocol::SourceWorkerSetupError>>,
    pub(crate) exited: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) jobs_started: Arc<AtomicU64>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) pause: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) pause_entered: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) hold_before_receive: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) panic_on_job: Arc<AtomicBool>,
    pub(super) join: Option<JoinHandle<WorkerExit>>,
}

#[cfg(test)]
pub(super) struct ReverseCompletionState {
    pub(super) enabled: AtomicBool,
    pub(super) parity_one_done: AtomicBool,
    pub(super) completion_order: Mutex<Vec<usize>>,
}

#[cfg(test)]
impl ReverseCompletionState {
    fn observe_send(&self, parity: usize) {
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }
        self.completion_order
            .lock()
            .expect("reverse completion observation lock")
            .push(parity);
        if parity == 1 {
            self.parity_one_done.store(true, Ordering::Release);
        } else if parity == 0 {
            self.parity_one_done.store(false, Ordering::Release);
        }
    }
}

struct SourceWorkerThreadState {
    exited: Arc<AtomicBool>,
    jobs_started: Arc<AtomicU64>,
    #[cfg(any(test, feature = "test-support"))]
    pause: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pause_entered: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    hold_before_receive: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    panic_on_job: Arc<AtomicBool>,
    #[cfg(test)]
    reverse_completion: Arc<ReverseCompletionState>,
}

pub(super) fn spawn_worker_named(
    parity: usize,
    #[cfg(test)] reverse_completion: Arc<ReverseCompletionState>,
    hold_before_receive: bool,
    start_hook: Option<SourceWorkerStartHook>,
    thread_name: &'static str,
) -> Result<SourceWorkerSlot, super::super::source_worker_protocol::SourceWorkerSetupError> {
    let (work_tx, work_rx) = bounded(SOURCE_WORKER_CHANNEL_CAPACITY);
    let (done_tx, done_rx) = bounded(SOURCE_WORKER_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = bounded(1);
    let exited = Arc::new(AtomicBool::new(false));
    let jobs_started = Arc::new(AtomicU64::new(0));
    #[cfg(any(test, feature = "test-support"))]
    let pause = Arc::new(AtomicBool::new(false));
    #[cfg(any(test, feature = "test-support"))]
    let pause_entered = Arc::new(AtomicBool::new(false));
    #[cfg(any(test, feature = "test-support"))]
    let hold_before_receive = Arc::new(AtomicBool::new(hold_before_receive));
    #[cfg(not(any(test, feature = "test-support")))]
    let _ = hold_before_receive;
    #[cfg(any(test, feature = "test-support"))]
    let panic_on_job = Arc::new(AtomicBool::new(false));
    let worker_exited = Arc::clone(&exited);
    let worker_jobs = Arc::clone(&jobs_started);
    #[cfg(any(test, feature = "test-support"))]
    let worker_pause = Arc::clone(&pause);
    #[cfg(any(test, feature = "test-support"))]
    let worker_pause_entered = Arc::clone(&pause_entered);
    #[cfg(any(test, feature = "test-support"))]
    let worker_hold_before_receive = Arc::clone(&hold_before_receive);
    #[cfg(any(test, feature = "test-support"))]
    let worker_panic_on_job = Arc::clone(&panic_on_job);
    let worker_done_tx = done_tx.clone();
    let lifecycle_done_tx = done_tx.clone();
    let join = spawn_worker_thread(thread_name, move || {
        if let Some(start_hook) = start_hook {
            if start_hook(parity).is_err() {
                let _ = ready_tx.send(Err(
                    super::super::source_worker_protocol::SourceWorkerSetupError::WorkerSchedulingUnavailable {
                        parity,
                    },
                ));
                worker_exited.store(true, Ordering::Release);
                return WorkerExit {
                    unsent_completion: None,
                };
            }
        }
        let _ = ready_tx.send(Ok(()));
        worker_loop(
            parity,
            work_rx,
            worker_done_tx,
            SourceWorkerThreadState {
                exited: worker_exited,
                jobs_started: worker_jobs,
                #[cfg(any(test, feature = "test-support"))]
                pause: worker_pause,
                #[cfg(any(test, feature = "test-support"))]
                pause_entered: worker_pause_entered,
                #[cfg(any(test, feature = "test-support"))]
                hold_before_receive: worker_hold_before_receive,
                #[cfg(any(test, feature = "test-support"))]
                panic_on_job: worker_panic_on_job,
                #[cfg(test)]
                reverse_completion,
            },
        )
    })
    .map_err(|_| {
        super::super::source_worker_protocol::SourceWorkerSetupError::WorkerThreadUnavailable
    })?;
    Ok(SourceWorkerSlot {
        work_tx: Some(work_tx),
        done_rx,
        done_tx: Some(lifecycle_done_tx),
        ready_rx,
        exited,
        #[cfg(any(test, feature = "test-support"))]
        jobs_started,
        #[cfg(any(test, feature = "test-support"))]
        pause,
        #[cfg(any(test, feature = "test-support"))]
        pause_entered,
        #[cfg(any(test, feature = "test-support"))]
        hold_before_receive,
        #[cfg(any(test, feature = "test-support"))]
        panic_on_job,
        join: Some(join),
    })
}

fn spawn_worker_thread<F>(
    thread_name: &'static str,
    run: F,
) -> std::io::Result<JoinHandle<WorkerExit>>
where
    F: FnOnce() -> WorkerExit + Send + 'static,
{
    thread::Builder::new().name(thread_name.into()).spawn(run)
}

impl SourceWorkerSlot {
    pub(super) fn shutdown_after_spawn_failure(mut self) {
        self.work_tx.take();
        self.done_tx.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn worker_loop(
    parity: usize,
    work_rx: Receiver<RoutingTreeWork>,
    done_tx: Sender<CompletedEnvelope>,
    state: SourceWorkerThreadState,
) -> WorkerExit {
    #[cfg(any(test, feature = "test-support"))]
    while state.hold_before_receive.load(Ordering::Acquire) {
        std::hint::spin_loop();
    }
    while let Ok(work) = work_rx.recv() {
        if let Some(exit) = routing_tree::process(work, parity, &done_tx, &state) {
            return exit;
        }
    }
    state.exited.store(true, Ordering::Release);
    WorkerExit {
        unsent_completion: None,
    }
}

#[cfg(test)]
fn wait_for_reverse_completion(state: &SourceWorkerThreadState, parity: usize) {
    if !state.reverse_completion.enabled.load(Ordering::Acquire) {
        return;
    }
    if parity == 0 {
        while !state
            .reverse_completion
            .parity_one_done
            .load(Ordering::Acquire)
        {
            std::hint::spin_loop();
        }
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

#[cfg(test)]
mod tests;
