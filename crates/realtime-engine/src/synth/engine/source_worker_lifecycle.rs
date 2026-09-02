#[cfg(test)]
use super::source_lane_renderer::SampleSourceContext;
use super::source_worker_health::{SourceWorkerHealth, SourceWorkerHealthState};
pub(super) use super::source_worker_owner::{
    CompletedEnvelope, OwnerEnvelope, SourceLanePartitionBundle, SourceWorkerScratch, WorkEnvelope,
    WorkerExit,
};
use super::source_worker_protocol::SourceWorkerRetirementError;
use super::source_worker_retirement::SourceWorkerRetirement;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[path = "source_worker_worker.rs"]
pub(super) mod worker;
pub use worker::SOURCE_WORKER_THREAD_NAMES;
use worker::{spawn_worker, ReverseCompletionState, SourceWorkerSlot};

pub(super) const SOURCE_WORKER_COUNT: usize = 2;
pub(super) const SOURCE_WORKER_CHANNEL_CAPACITY: usize = 1;
pub(super) const SOURCE_WORKER_MAILBOX_CAPACITY: usize = 1;

static NEXT_SOURCE_WORKER_GENERATION: AtomicU64 = AtomicU64::new(1);

#[cfg(any(test, feature = "test-support"))]
pub use super::source_worker_observer::SourceWorkerOwnerIdentity;

pub(super) struct SourceWorkerCloseState {
    pub(super) closed: AtomicBool,
    pub(super) generation: u64,
}

pub struct SourceWorkerLifecycle {
    pub(super) workers: [SourceWorkerSlot; SOURCE_WORKER_COUNT],
    pub(super) prewarmed: bool,
    pub(super) home_txs: [Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    pub(super) home_rxs: [Receiver<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    pub(super) fault_txs: [Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    pub(super) fault_rxs: [Receiver<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    pub(super) completion_rxs: [Receiver<CompletedEnvelope>; SOURCE_WORKER_COUNT],
    pub(super) runtime_close: Arc<SourceWorkerCloseState>,
    pub(super) health: Arc<SourceWorkerHealthState>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) destroyed_owner_identities: [Option<SourceWorkerOwnerIdentity>; SOURCE_WORKER_COUNT],
    #[cfg(test)]
    reverse_completion: Arc<ReverseCompletionState>,
}

impl SourceWorkerLifecycle {
    pub(super) fn start_with_hold(
        hold_before_receive: bool,
    ) -> Result<Self, super::source_worker_protocol::SourceWorkerSetupError> {
        let reverse_completion = Arc::new(ReverseCompletionState {
            enabled: AtomicBool::new(false),
            parity_one_done: AtomicBool::new(false),
        });
        let generation = NEXT_SOURCE_WORKER_GENERATION.fetch_add(1, Ordering::Relaxed);
        let runtime_close = Arc::new(SourceWorkerCloseState {
            closed: AtomicBool::new(false),
            generation,
        });
        let health = Arc::new(SourceWorkerHealthState::new(SourceWorkerHealth::Healthy));
        let (home_tx_0, home_rx_0) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let (home_tx_1, home_rx_1) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let (fault_tx_0, fault_rx_0) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let (fault_tx_1, fault_rx_1) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let first_worker =
            match spawn_worker(0, Arc::clone(&reverse_completion), hold_before_receive) {
                Ok(worker) => worker,
                Err(error) => {
                    runtime_close.closed.store(true, Ordering::Release);
                    return Err(error);
                }
            };
        let second_worker =
            match spawn_worker(1, Arc::clone(&reverse_completion), hold_before_receive) {
                Ok(worker) => worker,
                Err(error) => {
                    runtime_close.closed.store(true, Ordering::Release);
                    first_worker.shutdown_after_spawn_failure();
                    return Err(error);
                }
            };
        let workers = [first_worker, second_worker];
        let completion_rxs = [workers[0].done_rx.clone(), workers[1].done_rx.clone()];
        Ok(Self {
            workers,
            prewarmed: false,
            home_txs: [home_tx_0, home_tx_1],
            home_rxs: [home_rx_0, home_rx_1],
            fault_txs: [fault_tx_0, fault_tx_1],
            fault_rxs: [fault_rx_0, fault_rx_1],
            completion_rxs,
            runtime_close,
            health,
            #[cfg(any(test, feature = "test-support"))]
            destroyed_owner_identities: std::array::from_fn(|_| None),
            #[cfg(test)]
            reverse_completion,
        })
    }

    pub(super) fn prewarm(&mut self) -> bool {
        if self.prewarmed {
            return true;
        }
        let ready = self
            .workers
            .iter()
            .all(|worker| worker.ready_rx.recv().is_ok());
        self.prewarmed = ready;
        ready
    }

    pub(super) fn seed_home(&mut self, owners: [OwnerEnvelope; SOURCE_WORKER_COUNT]) -> bool {
        let mut seeded = true;
        for owner in owners {
            let parity = owner.parity;
            if let Err(error) = self.home_txs[parity].try_send(owner) {
                seeded = false;
                self.health
                    .latch(SourceWorkerHealth::CompletionFailed, 1 << parity);
                if let Some(owner) = route_owner(
                    error.into_inner(),
                    &self.home_txs,
                    &self.fault_txs,
                    &self.health,
                ) {
                    self.finish_owner(owner);
                }
            }
        }
        seeded
    }

    pub fn shutdown(
        self,
        retirement: SourceWorkerRetirement,
    ) -> super::source_worker_protocol::SourceWorkerShutdown {
        let retirement_error = self.validate_retirement(&retirement).err();
        if retirement_error.is_some() {
            self.runtime_close.closed.store(true, Ordering::Release);
        }
        self.shutdown_report(retirement_error)
    }

    pub fn validate_retirement(
        &self,
        retirement: &SourceWorkerRetirement,
    ) -> Result<(), SourceWorkerRetirementError> {
        let Some(close) = retirement.close.upgrade() else {
            return Err(SourceWorkerRetirementError::CloseStateUnavailable);
        };
        if !Arc::ptr_eq(&self.runtime_close, &close) {
            return Err(SourceWorkerRetirementError::CloseStateMismatch);
        }
        if retirement.generation != self.runtime_close.generation {
            return Err(SourceWorkerRetirementError::GenerationMismatch {
                expected: self.runtime_close.generation,
                actual: retirement.generation,
            });
        }
        if !close.closed.load(Ordering::Acquire) {
            return Err(SourceWorkerRetirementError::RuntimeStillOpen);
        }
        Ok(())
    }

    pub fn shutdown_after_runtime_drop(
        self,
    ) -> super::source_worker_protocol::SourceWorkerShutdown {
        let retirement_error = if self.runtime_close.closed.load(Ordering::Acquire) {
            None
        } else {
            Some(SourceWorkerRetirementError::RuntimeStillOpen)
        };
        self.runtime_close.closed.store(true, Ordering::Release);
        self.shutdown_report(retirement_error)
    }

    pub fn shutdown_after_retirement_error(
        self,
        error: SourceWorkerRetirementError,
    ) -> super::source_worker_protocol::SourceWorkerShutdown {
        self.runtime_close.closed.store(true, Ordering::Release);
        self.shutdown_report(Some(error))
    }

    fn shutdown_report(
        mut self,
        retirement_error: Option<SourceWorkerRetirementError>,
    ) -> super::source_worker_protocol::SourceWorkerShutdown {
        let joined_workers = self.shutdown_inner();
        let shutdown = super::source_worker_protocol::SourceWorkerShutdown {
            joined_workers,
            retirement_error,
            #[cfg(any(test, feature = "test-support"))]
            destroyed_owner_count: self.destroyed_owner_identities.iter().flatten().count(),
            #[cfg(any(test, feature = "test-support"))]
            destroyed_owner_identities: self.destroyed_owner_identities,
        };
        #[cfg(any(test, feature = "test-support"))]
        super::source_worker_observer::notify_shutdown_probe(&shutdown);
        shutdown
    }

    pub fn mark_runtime_closed(&self) {
        self.runtime_close.closed.store(true, Ordering::Release);
    }

    fn shutdown_inner(&mut self) -> usize {
        for worker in &mut self.workers {
            worker.work_tx.take();
            worker.done_tx.take();
        }
        let mut joined = 0;
        let mut unsent_owners: [Option<OwnerEnvelope>; SOURCE_WORKER_COUNT] =
            std::array::from_fn(|_| None);
        for (parity, worker) in self.workers.iter_mut().enumerate() {
            if let Some(join) = worker.join.take() {
                match join.join() {
                    Ok(exit) => {
                        joined += 1;
                        if let Some(completion) = exit.unsent_completion {
                            self.health.latch(
                                if completion.worker_exited {
                                    SourceWorkerHealth::WorkerExited
                                } else {
                                    SourceWorkerHealth::CompletionFailed
                                },
                                worker_mask(completion.owner.parity),
                            );
                            unsent_owners[parity] = Some(completion.owner);
                        }
                    }
                    Err(_) => {
                        self.health
                            .latch(SourceWorkerHealth::WorkerExited, worker_mask(parity));
                    }
                }
            }
        }
        for owner in unsent_owners.into_iter().flatten() {
            if let Some(owner) = route_owner(owner, &self.home_txs, &self.fault_txs, &self.health) {
                self.finish_owner(owner);
            }
        }
        for parity in 0..SOURCE_WORKER_COUNT {
            while let Ok(completion) = self.completion_rxs[parity].try_recv() {
                if completion.worker_exited || completion.transport_failed {
                    self.health.latch(
                        if completion.worker_exited {
                            SourceWorkerHealth::WorkerExited
                        } else {
                            SourceWorkerHealth::CompletionFailed
                        },
                        worker_mask(completion.owner.parity),
                    );
                }
                if let Some(owner) = route_owner(
                    completion.owner,
                    &self.home_txs,
                    &self.fault_txs,
                    &self.health,
                ) {
                    self.finish_owner(owner);
                }
            }
        }
        for parity in 0..SOURCE_WORKER_COUNT {
            while let Ok(owner) = self.home_rxs[parity].try_recv() {
                self.finish_owner(owner);
            }
            while let Ok(owner) = self.fault_rxs[parity].try_recv() {
                self.finish_owner(owner);
            }
        }
        joined
    }

    fn finish_owner(&mut self, owner: OwnerEnvelope) {
        #[cfg(any(test, feature = "test-support"))]
        let identity = super::source_worker_observer::owner_identity(&owner);
        #[cfg(any(test, feature = "test-support"))]
        if let Some(slot) = self
            .destroyed_owner_identities
            .iter_mut()
            .find(|slot| slot.is_none())
        {
            *slot = Some(identity);
        }
        drop(owner);
    }

    #[cfg(test)]
    pub(crate) fn set_pause_for_test(&self, paused: bool) {
        for worker in &self.workers {
            worker.pause.store(paused, Ordering::Release);
        }
    }

    #[cfg(test)]
    pub(crate) fn fault_owner_identities_for_test(&self) -> [Option<SourceWorkerOwnerIdentity>; 2] {
        std::array::from_fn(|parity| {
            let owner = self.fault_rxs[parity].try_recv().ok()?;
            let identity = super::source_worker_observer::owner_identity(&owner);
            self.fault_txs[parity]
                .try_send(owner)
                .expect("fault escrow");
            Some(identity)
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_exit_on_job_for_test(&self, parity: usize) {
        self.workers[parity]
            .exit_on_job
            .store(true, Ordering::Release);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_hold_before_receive_for_test(&self, held: bool) {
        for worker in &self.workers {
            worker.hold_before_receive.store(held, Ordering::Release);
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn hold_control_for_test(
        &self,
    ) -> super::source_worker_retirement::SourceWorkerHoldControl {
        super::source_worker_retirement::SourceWorkerHoldControl::new(
            self.workers
                .each_ref()
                .map(|worker| Arc::clone(&worker.hold_before_receive)),
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_panic_on_job_for_test(&self, parity: usize) {
        self.workers[parity]
            .panic_on_job
            .store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn disconnect_completion_for_test(&mut self, parity: usize) {
        let (done_tx, done_rx) = bounded(0);
        drop(done_tx);
        self.workers[parity].done_rx = done_rx;
    }

    #[cfg(test)]
    pub(crate) fn disconnect_work_for_test(&mut self, parity: usize) {
        self.workers[parity].work_tx.take();
    }

    #[cfg(test)]
    pub(crate) fn fill_work_channel_for_test(&self, parity: usize) -> bool {
        let mut engine = super::SynthEngine::new(48_000);
        let owner = OwnerEnvelope {
            parity,
            partitions: SourceLanePartitionBundle {
                synth: engine
                    .synth_voice_pool
                    .take_partition(parity)
                    .expect("test synth partition"),
                sample: engine
                    .sample_voice_pool
                    .take_partition(parity)
                    .expect("test sample partition"),
            },
            scratch: SourceWorkerScratch::new(),
        };
        let work = WorkEnvelope {
            owner,
            sequence: u64::MAX,
            frames: 0,
            base_sample_clock: engine.sample_clock,
            synth_context: engine.synth_source_context(),
            sample_context: SampleSourceContext {
                sample_rate: engine.sample_rate,
            },
        };
        self.workers[parity]
            .work_tx
            .as_ref()
            .expect("source worker lifecycle is active")
            .try_send(work)
            .is_ok()
    }

    #[cfg(test)]
    pub(crate) fn work_channel_is_full_for_test(&self, parity: usize) -> bool {
        self.workers[parity]
            .work_tx
            .as_ref()
            .expect("source worker lifecycle is active")
            .is_full()
    }

    #[cfg(test)]
    pub(crate) fn set_reverse_completion_for_test(&self, enabled: bool) {
        self.reverse_completion
            .parity_one_done
            .store(false, Ordering::Release);
        self.reverse_completion
            .enabled
            .store(enabled, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn jobs_started_for_test(&self) -> [u64; SOURCE_WORKER_COUNT] {
        self.workers
            .each_ref()
            .map(|worker| worker.jobs_started.load(Ordering::Relaxed))
    }

    #[cfg(test)]
    pub(crate) fn join_handles_present_for_test(&self) -> [bool; SOURCE_WORKER_COUNT] {
        self.workers.each_ref().map(|worker| worker.join.is_some())
    }

    #[cfg(test)]
    pub(crate) fn set_pause_for_parity_for_test(&self, parity: usize, paused: bool) {
        self.workers[parity].pause.store(paused, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn retirement_after_runtime_drop_for_test(&self) -> SourceWorkerRetirement {
        super::source_worker_retirement::SourceWorkerRetirement::new(&self.runtime_close)
    }
}

impl Drop for SourceWorkerLifecycle {
    fn drop(&mut self) {
        while !self.runtime_close.closed.load(Ordering::Acquire) {
            thread::park_timeout(Duration::from_millis(1));
        }
        self.shutdown_inner();
    }
}

#[cfg(test)]
pub(super) fn owner_for_test(parity: usize) -> OwnerEnvelope {
    let mut engine = super::SynthEngine::new(48_000);
    OwnerEnvelope {
        parity,
        partitions: SourceLanePartitionBundle {
            synth: engine
                .synth_voice_pool
                .take_partition(parity)
                .expect("synth partition"),
            sample: engine
                .sample_voice_pool
                .take_partition(parity)
                .expect("sample partition"),
        },
        scratch: SourceWorkerScratch::new(),
    }
}

fn route_owner(
    owner: OwnerEnvelope,
    home_txs: &[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    fault_txs: &[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    health: &SourceWorkerHealthState,
) -> Option<OwnerEnvelope> {
    let parity = owner.parity;
    if parity >= SOURCE_WORKER_COUNT {
        health.latch(SourceWorkerHealth::CompletionFailed, 0b11);
        return Some(owner);
    }
    match home_txs[parity].try_send(owner) {
        Ok(()) => None,
        Err(error) => {
            let owner = error.into_inner();
            health.latch(SourceWorkerHealth::CompletionFailed, worker_mask(parity));
            match fault_txs[parity].try_send(owner) {
                Ok(()) => None,
                Err(error) => {
                    health.latch(SourceWorkerHealth::CompletionFailed, worker_mask(parity));
                    Some(error.into_inner())
                }
            }
        }
    }
}

fn worker_mask(parity: usize) -> u8 {
    if parity < SOURCE_WORKER_COUNT {
        1 << parity
    } else {
        0b11
    }
}
