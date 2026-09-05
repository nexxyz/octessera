#[cfg(feature = "source-worker-benchmark-timing")]
use super::super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::super::source_worker_protocol::{SourceWorkerStartHook, WorkerCommand};
use super::{CompletedEnvelope, WorkerExit, SOURCE_WORKER_CHANNEL_CAPACITY};
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
#[cfg(test)]
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Instant;

#[path = "source_worker_bus_worker.rs"]
mod bus_worker;
#[cfg(feature = "routing-tree-benchmark")]
#[path = "source_worker_routing_tree_worker.rs"]
mod routing_tree;

#[cfg(test)]
thread_local! {
    static FAIL_WORKER_SPAWN: RefCell<Option<(usize, Arc<AtomicUsize>)>> =
        const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct WorkerSpawnFailureGuard {
    active_workers: Arc<AtomicUsize>,
}

#[cfg(test)]
pub(crate) fn fail_worker_spawn_at_for_test(parity: usize) -> WorkerSpawnFailureGuard {
    let active_workers = Arc::new(AtomicUsize::new(0));
    FAIL_WORKER_SPAWN.with(|failure| {
        failure.replace(Some((parity, Arc::clone(&active_workers))));
    });
    WorkerSpawnFailureGuard { active_workers }
}

#[cfg(test)]
impl Drop for WorkerSpawnFailureGuard {
    fn drop(&mut self) {
        FAIL_WORKER_SPAWN.with(|failure| failure.replace(None));
    }
}

#[cfg(test)]
impl WorkerSpawnFailureGuard {
    pub(crate) fn active_workers_for_test(&self) -> usize {
        self.active_workers.load(Ordering::Acquire)
    }
}

pub(crate) struct SourceWorkerSlot {
    pub(crate) work_tx: Option<Sender<WorkerCommand>>,
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
    pub(super) exit_on_job: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) hold_before_receive: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) panic_on_job: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) panic_on_bus: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) exit_on_bus: Arc<AtomicBool>,
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
    exit_on_job: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    hold_before_receive: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    panic_on_job: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    panic_on_bus: Arc<AtomicBool>,
    #[cfg(any(test, feature = "test-support"))]
    exit_on_bus: Arc<AtomicBool>,
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
    let exit_on_job = Arc::new(AtomicBool::new(false));
    #[cfg(any(test, feature = "test-support"))]
    let hold_before_receive = Arc::new(AtomicBool::new(hold_before_receive));
    #[cfg(not(any(test, feature = "test-support")))]
    let _ = hold_before_receive;
    #[cfg(any(test, feature = "test-support"))]
    let panic_on_job = Arc::new(AtomicBool::new(false));
    #[cfg(any(test, feature = "test-support"))]
    let panic_on_bus = Arc::new(AtomicBool::new(false));
    #[cfg(any(test, feature = "test-support"))]
    let exit_on_bus = Arc::new(AtomicBool::new(false));
    #[cfg(test)]
    let active_probe = FAIL_WORKER_SPAWN.with(|failure| {
        failure
            .borrow()
            .as_ref()
            .map(|(_, active_workers)| Arc::clone(active_workers))
    });
    let worker_exited = Arc::clone(&exited);
    let worker_jobs = Arc::clone(&jobs_started);
    #[cfg(any(test, feature = "test-support"))]
    let worker_pause = Arc::clone(&pause);
    #[cfg(any(test, feature = "test-support"))]
    let worker_pause_entered = Arc::clone(&pause_entered);
    #[cfg(any(test, feature = "test-support"))]
    let worker_exit_on_job = Arc::clone(&exit_on_job);
    #[cfg(any(test, feature = "test-support"))]
    let worker_hold_before_receive = Arc::clone(&hold_before_receive);
    #[cfg(any(test, feature = "test-support"))]
    let worker_panic_on_job = Arc::clone(&panic_on_job);
    #[cfg(any(test, feature = "test-support"))]
    let worker_panic_on_bus = Arc::clone(&panic_on_bus);
    #[cfg(any(test, feature = "test-support"))]
    let worker_exit_on_bus = Arc::clone(&exit_on_bus);
    let worker_done_tx = done_tx.clone();
    let lifecycle_done_tx = done_tx.clone();
    let join = spawn_worker_thread(parity, thread_name, move || {
        #[cfg(test)]
        if let Some(active_probe) = active_probe.as_ref() {
            active_probe.fetch_add(1, Ordering::AcqRel);
        }
        if let Some(start_hook) = start_hook {
            if start_hook(parity).is_err() {
                let _ = ready_tx.send(Err(
                    super::super::source_worker_protocol::SourceWorkerSetupError::WorkerSchedulingUnavailable {
                        parity,
                    },
                ));
                worker_exited.store(true, Ordering::Release);
                #[cfg(test)]
                if let Some(active_probe) = active_probe.as_ref() {
                    active_probe.fetch_sub(1, Ordering::AcqRel);
                }
                return WorkerExit {
                    unsent_completion: None,
                };
            }
        }
        let _ = ready_tx.send(Ok(()));
        let exit = worker_loop(
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
                exit_on_job: worker_exit_on_job,
                #[cfg(any(test, feature = "test-support"))]
                hold_before_receive: worker_hold_before_receive,
                #[cfg(any(test, feature = "test-support"))]
                panic_on_job: worker_panic_on_job,
                #[cfg(any(test, feature = "test-support"))]
                panic_on_bus: worker_panic_on_bus,
                #[cfg(any(test, feature = "test-support"))]
                exit_on_bus: worker_exit_on_bus,
                #[cfg(test)]
                reverse_completion,
            },
        );
        #[cfg(test)]
        if let Some(active_probe) = active_probe.as_ref() {
            active_probe.fetch_sub(1, Ordering::AcqRel);
        }
        exit
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
        exit_on_job,
        #[cfg(any(test, feature = "test-support"))]
        hold_before_receive,
        #[cfg(any(test, feature = "test-support"))]
        panic_on_job,
        #[cfg(any(test, feature = "test-support"))]
        panic_on_bus,
        #[cfg(any(test, feature = "test-support"))]
        exit_on_bus,
        join: Some(join),
    })
}

fn spawn_worker_thread<F>(
    _parity: usize,
    thread_name: &'static str,
    run: F,
) -> std::io::Result<JoinHandle<WorkerExit>>
where
    F: FnOnce() -> WorkerExit + Send + 'static,
{
    #[cfg(test)]
    let should_fail = FAIL_WORKER_SPAWN.with(|failure| {
        let mut failure = failure.borrow_mut();
        match failure.take() {
            Some((parity, active_workers)) if parity == _parity => {
                drop(active_workers);
                true
            }
            other => {
                *failure = other;
                false
            }
        }
    });
    #[cfg(test)]
    if should_fail {
        return Err(std::io::Error::other(
            "injected source worker spawn failure",
        ));
    }
    thread::Builder::new().name(thread_name.into()).spawn(run)
}

pub const SOURCE_WORKER_THREAD_NAMES: [&str; 2] = ["oct-dsp-src-0", "oct-dsp-src-1"];
pub const ROUTING_TREE_WORKER_THREAD_NAMES: [&str; 2] = ["oct-dsp-tree-0", "oct-dsp-tree-1"];

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
    work_rx: Receiver<WorkerCommand>,
    done_tx: Sender<CompletedEnvelope>,
    state: SourceWorkerThreadState,
) -> WorkerExit {
    #[cfg(any(test, feature = "test-support"))]
    while state.hold_before_receive.load(Ordering::Acquire) {
        std::hint::spin_loop();
    }
    while let Ok(command) = work_rx.recv() {
        if matches!(command, WorkerCommand::Buses { .. }) {
            if let Some(exit) = bus_worker::process(command, parity, &done_tx, &state) {
                return exit;
            }
            continue;
        }
        #[cfg(feature = "routing-tree-benchmark")]
        if matches!(command, WorkerCommand::RoutingTree { .. }) {
            if let Some(exit) = routing_tree::process(command, parity, &done_tx, &state) {
                return exit;
            }
            continue;
        }
        let Some(mut work) = command.into_source_work() else {
            continue;
        };
        state.jobs_started.fetch_add(1, Ordering::Relaxed);
        #[cfg(any(test, feature = "test-support"))]
        if state.exit_on_job.swap(false, Ordering::AcqRel) {
            return finish_worker(
                &state,
                send_completion(
                    &done_tx,
                    CompletedEnvelope::from_work(work, true, false, 0, 0),
                ),
            );
        }
        #[cfg(any(test, feature = "test-support"))]
        state.pause_entered.store(true, Ordering::Release);
        #[cfg(any(test, feature = "test-support"))]
        while state.pause.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        #[cfg(any(test, feature = "test-support"))]
        state.pause_entered.store(false, Ordering::Release);
        #[cfg(any(test, feature = "test-support"))]
        let should_panic = state.panic_on_job.swap(false, Ordering::AcqRel);
        #[cfg(not(any(test, feature = "test-support")))]
        let should_panic = false;
        let active_cost_units = work.active_cost_units();
        #[cfg(feature = "source-worker-benchmark-timing")]
        let timing_start = work.timing_probe.as_ref().map(|probe| probe.worker_start());
        let render_started_at = {
            #[cfg(feature = "source-worker-benchmark-timing")]
            {
                timing_start.map_or_else(Instant::now, SourceWorkerTimingProbe::render_start)
            }
            #[cfg(not(feature = "source-worker-benchmark-timing"))]
            {
                Instant::now()
            }
        };
        let render_ok = catch_unwind(AssertUnwindSafe(|| {
            if should_panic {
                panic!("source worker test panic");
            }
            work.render()
        }))
        .unwrap_or(false);
        let dsp_duration_ns = render_started_at
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        #[cfg(feature = "source-worker-benchmark-timing")]
        if render_ok {
            if let (Some(probe), Some(start)) = (work.timing_probe.as_ref(), timing_start) {
                probe.record_worker(
                    parity,
                    work.stamp.quantum_sequence,
                    start,
                    dsp_duration_ns,
                    work.dispatch_started_at,
                );
            }
        }
        if !render_ok {
            return finish_worker(
                &state,
                send_completion(
                    &done_tx,
                    CompletedEnvelope::from_work(
                        work,
                        true,
                        false,
                        dsp_duration_ns,
                        active_cost_units,
                    ),
                ),
            );
        }
        #[cfg(test)]
        wait_for_reverse_completion(&state, parity);
        let completion =
            CompletedEnvelope::from_work(work, false, true, dsp_duration_ns, active_cost_units);
        let send_result = send_completion(&done_tx, completion);
        #[cfg(test)]
        if send_result.is_none() {
            state.reverse_completion.observe_send(parity);
        }
        if let Some(exit) = send_result {
            state.exited.store(true, Ordering::Release);
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
#[path = "source_worker_worker_tests.rs"]
mod tests;
