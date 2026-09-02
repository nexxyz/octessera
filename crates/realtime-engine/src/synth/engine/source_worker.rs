#[cfg(feature = "source-worker-benchmark-timing")]
use super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::source_worker_health::{
    SourceWorkerHealth, SourceWorkerHealthSnapshot, SourceWorkerHealthState,
};
use super::source_worker_lease::OwnerLease;
use super::source_worker_lifecycle::{
    CompletedEnvelope, OwnerEnvelope, SourceWorkerCloseState, SourceWorkerLifecycle,
    SourceWorkerScratch, WorkEnvelope,
};
use super::source_worker_protocol::SourceWorkerMode;
use super::source_worker_retirement::SourceWorkerRetirement;
use super::source_worker_transfer;
use super::SynthEngine;
use super::BLOCK_SLOT_SCRATCH_FRAMES;
use crossbeam_channel::{Receiver, Sender, TryRecvError};
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) const SOURCE_WORKER_COUNT: usize = 2;
const SOURCE_WORKER_DEADLINE_FRACTION: f64 = 0.35;

pub struct SourceWorkerRuntime {
    mode: SourceWorkerMode,
    work_txs: Option<[Sender<WorkEnvelope>; SOURCE_WORKER_COUNT]>,
    #[cfg(test)]
    done_txs: Option<[Sender<CompletedEnvelope>; SOURCE_WORKER_COUNT]>,
    done_rxs: Option<[Receiver<CompletedEnvelope>; SOURCE_WORKER_COUNT]>,
    home_txs: Option<[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT]>,
    home_rxs: Option<[Receiver<OwnerEnvelope>; SOURCE_WORKER_COUNT]>,
    fault_txs: Option<[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT]>,
    worker_exited: Option<[Arc<AtomicBool>; SOURCE_WORKER_COUNT]>,
    #[cfg(any(test, feature = "test-support"))]
    jobs_started: Option<[Arc<AtomicU64>; SOURCE_WORKER_COUNT]>,
    #[cfg(any(test, feature = "test-support"))]
    render_attempts: AtomicU64,
    #[cfg(feature = "source-worker-benchmark-timing")]
    timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
    #[cfg(feature = "source-worker-benchmark-timing")]
    dispatch_started_at: Option<Instant>,
    #[cfg(any(test, feature = "test-support"))]
    worker_pauses: Option<[Arc<AtomicBool>; SOURCE_WORKER_COUNT]>,
    runtime_close: Option<Arc<SourceWorkerCloseState>>,
    health: Arc<SourceWorkerHealthState>,
    next_sequence: u64,
    expected_sequence: Option<u64>,
    expected_frames: usize,
    expected_base_sample_clock: u64,
    in_flight_mask: u8,
    completed_mask: u8,
    sample_rate: u32,
    #[cfg(any(test, feature = "test-support"))]
    deadline_override: Option<Duration>,
}

impl SourceWorkerRuntime {
    pub fn inline() -> Self {
        Self {
            mode: SourceWorkerMode::Inline,
            work_txs: None,
            #[cfg(test)]
            done_txs: None,
            done_rxs: None,
            home_txs: None,
            home_rxs: None,
            fault_txs: None,
            worker_exited: None,
            #[cfg(any(test, feature = "test-support"))]
            jobs_started: None,
            #[cfg(any(test, feature = "test-support"))]
            render_attempts: AtomicU64::new(0),
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe: None,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at: None,
            #[cfg(any(test, feature = "test-support"))]
            worker_pauses: None,
            runtime_close: None,
            health: Arc::new(SourceWorkerHealthState::new(SourceWorkerHealth::Disabled)),
            next_sequence: 0,
            expected_sequence: None,
            expected_frames: 0,
            expected_base_sample_clock: 0,
            in_flight_mask: 0,
            completed_mask: 0,
            sample_rate: 0,
            #[cfg(any(test, feature = "test-support"))]
            deadline_override: None,
        }
    }

    pub(super) fn new(lifecycle: &SourceWorkerLifecycle, sample_rate: u32) -> Option<Self> {
        let workers = &lifecycle.workers;
        let work_txs = [
            workers[0].work_tx.as_ref()?.clone(),
            workers[1].work_tx.as_ref()?.clone(),
        ];
        #[cfg(test)]
        let done_txs = [
            workers[0].done_tx.as_ref()?.clone(),
            workers[1].done_tx.as_ref()?.clone(),
        ];
        Some(Self {
            mode: SourceWorkerMode::Persistent,
            work_txs: Some(work_txs),
            #[cfg(test)]
            done_txs: Some(done_txs),
            done_rxs: Some([workers[0].done_rx.clone(), workers[1].done_rx.clone()]),
            home_txs: Some(lifecycle.home_txs.clone()),
            home_rxs: Some(lifecycle.home_rxs.clone()),
            fault_txs: Some(lifecycle.fault_txs.clone()),
            worker_exited: Some([
                Arc::clone(&workers[0].exited),
                Arc::clone(&workers[1].exited),
            ]),
            #[cfg(any(test, feature = "test-support"))]
            jobs_started: Some([
                Arc::clone(&workers[0].jobs_started),
                Arc::clone(&workers[1].jobs_started),
            ]),
            #[cfg(any(test, feature = "test-support"))]
            render_attempts: AtomicU64::new(0),
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe: None,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at: None,
            #[cfg(any(test, feature = "test-support"))]
            worker_pauses: Some(lifecycle.worker_pause_controls_for_test()),
            runtime_close: Some(Arc::clone(&lifecycle.runtime_close)),
            health: Arc::clone(&lifecycle.health),
            next_sequence: 0,
            expected_sequence: None,
            expected_frames: 0,
            expected_base_sample_clock: 0,
            in_flight_mask: 0,
            completed_mask: 0,
            sample_rate,
            #[cfg(any(test, feature = "test-support"))]
            deadline_override: None,
        })
    }

    pub fn mode(&self) -> SourceWorkerMode {
        self.mode
    }

    pub fn retire(mut self) -> SourceWorkerRetirement {
        #[cfg(feature = "source-worker-benchmark-timing")]
        self.freeze_timing(self.health.status().is_terminal(), None);
        let Some(close) = self.runtime_close.take() else {
            return SourceWorkerRetirement::inline();
        };
        close.closed.store(true, Ordering::Release);
        SourceWorkerRetirement::new(&close)
    }

    pub fn health_snapshot(&self) -> SourceWorkerHealthSnapshot {
        self.health.snapshot()
    }

    fn rendezvous_deadline(&self, frames: usize) -> Duration {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(deadline) = self.deadline_override {
            return deadline;
        }
        let quantum_seconds = if self.sample_rate == 0 {
            0.0
        } else {
            frames as f64 / self.sample_rate as f64
        };
        Duration::from_secs_f64(quantum_seconds * SOURCE_WORKER_DEADLINE_FRACTION)
    }

    fn home_is_ready(&self) -> bool {
        self.home_txs
            .as_ref()
            .is_some_and(|home_txs| home_txs.iter().all(Sender::is_full))
    }

    fn lease_home(&self, parity: usize) -> Option<OwnerLease> {
        let owner = self.home_rxs.as_ref()?.get(parity)?.try_recv().ok()?;
        Some(OwnerLease {
            owner: Some(owner),
            parity,
            home_tx: self.home_txs.as_ref()?[parity].clone(),
            fault_tx: self.fault_txs.as_ref()?[parity].clone(),
            health: Arc::clone(&self.health),
        })
    }

    pub fn with_controls_ready<R>(
        &mut self,
        engine: &mut SynthEngine,
        apply: impl FnOnce(&mut SynthEngine) -> R,
    ) -> Option<R> {
        if self.mode == SourceWorkerMode::Inline {
            return Some(apply(engine));
        }
        self.reclaim_available(engine);
        if self.health.status() != SourceWorkerHealth::Healthy
            || self.in_flight_mask != 0
            || self.completed_mask != 0
            || !self.home_is_ready()
        {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_completion_failure(0b11);
            return None;
        };
        match source_worker_transfer::with_both_source_partitions(
            engine,
            &mut first,
            &mut second,
            |engine, _| apply(engine),
        ) {
            Ok(result) => {
                first.return_home();
                second.return_home();
                Some(result)
            }
            Err(()) => {
                self.latch_completion_failure(0b11);
                None
            }
        }
    }

    pub fn with_recovered_owners<R>(
        &mut self,
        engine: &mut SynthEngine,
        inspect: impl FnOnce(&SynthEngine) -> R,
    ) -> Option<R> {
        if self.mode == SourceWorkerMode::Inline {
            return Some(inspect(engine));
        }
        self.reclaim_available(engine);
        if !self.home_is_ready() {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return None;
        };
        match source_worker_transfer::with_both_source_partitions_read_only(
            engine,
            &mut first,
            &mut second,
            inspect,
        ) {
            Ok(result) => {
                first.return_home();
                second.return_home();
                Some(result)
            }
            Err(()) => None,
        }
    }

    pub(super) fn render_source_block(&mut self, engine: &mut SynthEngine, frames: usize) -> bool {
        #[cfg(any(test, feature = "test-support"))]
        self.render_attempts.fetch_add(1, Ordering::Relaxed);
        if self.mode == SourceWorkerMode::Inline {
            return false;
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            self.reclaim_available(engine);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            self.latch_invalid_block();
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        if self.in_flight_mask != 0 || !self.home_is_ready() {
            self.latch_dispatch_failure(0b11);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        self.expected_frames = frames;
        self.expected_base_sample_clock = engine.sample_clock;
        if !self.dispatch(engine) {
            self.reclaim_available(engine);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return false;
        }
        self.collect(engine, true)
    }

    fn collect(&mut self, engine: &mut SynthEngine, wait: bool) -> bool {
        let start = Instant::now();
        #[cfg(feature = "source-worker-benchmark-timing")]
        self.record_dispatch_to_deadline_start(start);
        let deadline = self.rendezvous_deadline(self.expected_frames);
        while self.in_flight_mask != 0 {
            for parity in 0..SOURCE_WORKER_COUNT {
                if self.in_flight_mask & (1 << parity) == 0 || !self.home_is_empty(parity) {
                    continue;
                }
                let receive_result = self
                    .done_rxs
                    .as_ref()
                    .map(|done_rxs| done_rxs[parity].try_recv());
                match receive_result {
                    Some(Ok(completion)) => {
                        self.accept_completion(completion);
                    }
                    Some(Err(TryRecvError::Empty)) | None => {}
                    Some(Err(TryRecvError::Disconnected)) => {
                        self.latch_completion_failure(1 << parity);
                        self.in_flight_mask &= !(1 << parity);
                    }
                }
            }
            if self.health.status() != SourceWorkerHealth::Healthy {
                #[cfg(feature = "source-worker-benchmark-timing")]
                self.freeze_timing(
                    true,
                    (self.health.status() == SourceWorkerHealth::DeadlineMiss)
                        .then(|| self.dispatch_elapsed())
                        .flatten(),
                );
                self.reclaim_available(engine);
            }
            if self.in_flight_mask == 0 {
                break;
            }
            if !wait && self.health.status() == SourceWorkerHealth::Healthy {
                break;
            }
            if start.elapsed() >= deadline {
                if self.health.status() == SourceWorkerHealth::Healthy {
                    self.latch_deadline_or_exit();
                }
                #[cfg(feature = "source-worker-benchmark-timing")]
                self.freeze_timing(true, self.dispatch_elapsed());
                self.reclaim_available(engine);
                return false;
            }
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            self.reclaim_available(engine);
            return false;
        }
        if self.in_flight_mask != 0 {
            return false;
        }
        self.finish_completed(engine)
    }

    fn home_is_empty(&self, parity: usize) -> bool {
        self.home_txs
            .as_ref()
            .is_some_and(|home_txs| !home_txs[parity].is_full())
    }

    fn latch_dispatch_failure(&self, mask: u8) {
        self.health.latch(SourceWorkerHealth::DispatchFailed, mask);
    }

    fn latch_completion_failure(&self, mask: u8) {
        self.health
            .latch(SourceWorkerHealth::CompletionFailed, mask);
    }

    fn latch_deadline_or_exit(&self) {
        let Some(workers) = self.worker_exited.as_ref() else {
            self.latch_completion_failure(0b11);
            return;
        };
        for (parity, worker) in workers.iter().enumerate() {
            if self.in_flight_mask & (1 << parity) == 0 {
                continue;
            }
            if worker.load(Ordering::Acquire) {
                self.health
                    .latch(SourceWorkerHealth::WorkerExited, 1 << parity);
            } else {
                self.health
                    .latch(SourceWorkerHealth::DeadlineMiss, 1 << parity);
            }
        }
    }

    fn latch_invalid_block(&self) {
        self.health.latch(SourceWorkerHealth::InvalidBlock, 0b11);
    }
}

impl Drop for SourceWorkerRuntime {
    fn drop(&mut self) {
        if let Some(close) = self.runtime_close.as_ref() {
            close.closed.store(true, Ordering::Release);
        }
    }
}

#[path = "source_worker_mailboxes.rs"]
mod mailboxes;

#[path = "source_worker_dispatch.rs"]
mod dispatch;

#[path = "source_worker_completion.rs"]
mod completion;

#[cfg(feature = "source-worker-benchmark-timing")]
#[path = "source_worker_timing_integration.rs"]
mod timing_integration;

#[path = "source_worker_reduce.rs"]
mod reduction;

#[cfg(test)]
#[path = "source_worker_test_support.rs"]
pub(super) mod test_support;

#[cfg(any(test, feature = "test-support"))]
#[path = "source_worker_feature_support.rs"]
mod feature_support;
