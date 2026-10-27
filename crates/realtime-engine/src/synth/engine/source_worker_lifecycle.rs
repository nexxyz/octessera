mod owner_routing;
mod startup;
use super::source_worker_health::{SourceWorkerHealth, SourceWorkerHealthState};
pub(super) use super::source_worker_owner::{
    CompletedEnvelope, OwnerEnvelope, SourceLanePartitionBundle, WorkerExit,
};
use super::source_worker_protocol::SourceWorkerRetirementError;
use super::source_worker_retirement::{SourceWorkerCloseState, SourceWorkerRetirement};
use crossbeam_channel::{Receiver, Sender};
use owner_routing::{route_completion, route_owner, worker_mask};
#[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub(super) mod worker;
#[cfg(test)]
use worker::ReverseCompletionState;
use worker::SourceWorkerSlot;
pub(super) const SOURCE_WORKER_COUNT: usize = 2;
pub(super) const SOURCE_WORKER_CHANNEL_CAPACITY: usize = 1;
pub(super) const SOURCE_WORKER_MAILBOX_CAPACITY: usize = 1;

static NEXT_SOURCE_WORKER_GENERATION: AtomicU64 = AtomicU64::new(1);

#[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
pub use super::source_worker_observer::SourceWorkerOwnerIdentity;

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
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub(super) destroyed_owner_identities: [Option<SourceWorkerOwnerIdentity>; SOURCE_WORKER_COUNT],
    #[cfg(test)]
    reverse_completion: Arc<ReverseCompletionState>,
}

impl SourceWorkerLifecycle {
    pub(super) fn runtime_generation(&self) -> u64 {
        self.runtime_close.generation
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
                    true,
                    parity,
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
            #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
            destroyed_owner_count: self.destroyed_owner_identities.iter().flatten().count(),
            #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
            destroyed_owner_identities: self.destroyed_owner_identities,
        };
        #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
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
        let mut unsent_completions: [Option<CompletedEnvelope>; SOURCE_WORKER_COUNT] =
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
                            unsent_completions[parity] = Some(completion);
                        }
                    }
                    Err(_) => {
                        self.health
                            .latch(SourceWorkerHealth::WorkerExited, worker_mask(parity));
                    }
                }
            }
        }
        for (parity, completion) in unsent_completions
            .into_iter()
            .enumerate()
            .filter_map(|(parity, completion)| completion.map(|completion| (parity, completion)))
        {
            if let Some(owner) = route_completion(
                completion,
                &self.home_txs,
                &self.fault_txs,
                &self.health,
                parity,
            ) {
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
                if let Some(owner) = route_completion(
                    completion,
                    &self.home_txs,
                    &self.fault_txs,
                    &self.health,
                    parity,
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
        #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
        let identity = super::source_worker_observer::owner_identity(&owner);
        #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
        if let Some(slot) = self
            .destroyed_owner_identities
            .iter_mut()
            .find(|slot| slot.is_none())
        {
            *slot = Some(identity);
        }
        drop(owner);
    }

    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub fn set_exit_on_job_for_test(&self, parity: usize) {
        self.workers[parity]
            .exit_on_job
            .store(true, Ordering::Release);
    }

    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub fn set_hold_before_receive_for_test(&self, held: bool) {
        for worker in &self.workers {
            worker.hold_before_receive.store(held, Ordering::Release);
        }
    }

    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub fn hold_control_for_test(
        &self,
    ) -> super::source_worker_retirement::SourceWorkerHoldControl {
        super::source_worker_retirement::SourceWorkerHoldControl::new(
            self.workers
                .each_ref()
                .map(|worker| Arc::clone(&worker.hold_before_receive)),
        )
    }

    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub fn set_panic_on_job_for_test(&self, parity: usize) {
        self.workers[parity]
            .panic_on_job
            .store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn set_reverse_completion_for_test(&self, enabled: bool) {
        self.reverse_completion
            .parity_one_done
            .store(false, Ordering::Release);
        self.reverse_completion
            .completion_order
            .lock()
            .expect("reverse completion observation lock")
            .clear();
        self.reverse_completion
            .enabled
            .store(enabled, Ordering::Release);
    }

    #[cfg(all(test, feature = "routing-tree-executor"))]
    pub(crate) fn take_reverse_completion_order_for_test(&self) -> Vec<usize> {
        std::mem::take(
            &mut *self
                .reverse_completion
                .completion_order
                .lock()
                .expect("reverse completion observation lock"),
        )
    }

    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub(super) fn worker_pause_controls_for_test(&self) -> [Arc<AtomicBool>; SOURCE_WORKER_COUNT] {
        self.workers
            .each_ref()
            .map(|worker| Arc::clone(&worker.pause))
    }

    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub(super) fn worker_pause_entry_controls_for_test(
        &self,
    ) -> [Arc<AtomicBool>; SOURCE_WORKER_COUNT] {
        self.workers
            .each_ref()
            .map(|worker| Arc::clone(&worker.pause_entered))
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
        runtime_generation: 1,
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
        bus_carriers: std::array::from_fn(|_| None),
        #[cfg(feature = "routing-tree-executor")]
        routing_tree: None,
    }
}
