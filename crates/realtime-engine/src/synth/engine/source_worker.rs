#[cfg(feature = "routing-tree-executor")]
use super::routing_tree_worker::RoutingTreeOutputBlock;
use super::source_worker_health::{
    SourceWorkerHealth, SourceWorkerHealthSnapshot, SourceWorkerHealthState,
};
use super::source_worker_lease::OwnerLease;
use super::source_worker_lifecycle::{CompletedEnvelope, OwnerEnvelope};
use super::source_worker_load::{SourceWorkerLoad, SourceWorkerLoadSnapshot};
use super::source_worker_protocol::{RoutingTreeWork, SourceWorkerMode, WorkStamp, WorkerPhase};
use super::source_worker_retirement::{SourceWorkerCloseState, SourceWorkerRetirement};
use super::SynthEngine;
use crossbeam_channel::{Receiver, Sender};
#[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
use std::time::Duration;
use std::time::Instant;

pub(super) const SOURCE_WORKER_COUNT: usize = 2;

pub struct SourceWorkerRuntime {
    mode: SourceWorkerMode,
    work_txs: Option<[Sender<RoutingTreeWork>; SOURCE_WORKER_COUNT]>,
    #[cfg(test)]
    done_txs: Option<[Sender<CompletedEnvelope>; SOURCE_WORKER_COUNT]>,
    done_rxs: Option<[Receiver<CompletedEnvelope>; SOURCE_WORKER_COUNT]>,
    home_txs: Option<[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT]>,
    home_rxs: Option<[Receiver<OwnerEnvelope>; SOURCE_WORKER_COUNT]>,
    fault_txs: Option<[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT]>,
    worker_exited: Option<[Arc<AtomicBool>; SOURCE_WORKER_COUNT]>,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    jobs_started: Option<[Arc<AtomicU64>; SOURCE_WORKER_COUNT]>,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    render_attempts: AtomicU64,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    worker_pauses: Option<[Arc<AtomicBool>; SOURCE_WORKER_COUNT]>,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    worker_pause_entries: Option<[Arc<AtomicBool>; SOURCE_WORKER_COUNT]>,
    runtime_close: Option<Arc<SourceWorkerCloseState>>,
    health: Arc<SourceWorkerHealthState>,
    runtime_generation: u64,
    next_sequence: u64,
    expected_stamp: Option<WorkStamp>,
    expected_phase: Option<WorkerPhase>,
    in_flight_mask: u8,
    completed_mask: u8,
    sample_rate: u32,
    lookahead_frames: usize,
    #[cfg(feature = "routing-tree-executor")]
    routing_output_spares: Option<[RoutingTreeOutputBlock; SOURCE_WORKER_COUNT]>,
    #[cfg(feature = "routing-tree-executor")]
    routing_output_stamp: Option<WorkStamp>,
    #[cfg(feature = "routing-tree-executor")]
    routing_output_ready: bool,
    #[cfg(feature = "routing-tree-executor")]
    routing_tree_reprime_pending: bool,
    #[cfg(feature = "routing-tree-executor")]
    routing_absolute_deadline: Option<Instant>,
    #[cfg(all(feature = "routing-tree-executor", any(test, feature = "test-support")))]
    routing_tree_probe: Option<Arc<RoutingTreePipelineProbe>>,
    load: Option<SourceWorkerLoad>,
    source_load_observations:
        [Option<super::source_worker_load::SourceWorkerLoadObservation>; SOURCE_WORKER_COUNT],
    bus_load_observations:
        [Option<super::source_worker_load::SourceWorkerLoadObservation>; SOURCE_WORKER_COUNT],
    bus_dispatch_residency: [u8; super::super::types::BUS_COUNT],
    bus_dispatch_residency_valid: bool,
    force_fault_mask: u8,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    deadline_override: Option<Duration>,
}

impl SourceWorkerRuntime {
    pub fn mode(&self) -> SourceWorkerMode {
        self.mode
    }

    pub fn lookahead_frames(&self) -> usize {
        self.lookahead_frames
    }

    pub fn retire(mut self) -> SourceWorkerRetirement {
        #[cfg(feature = "routing-tree-executor")]
        self.clear_routing_absolute_deadline();
        let Some(close) = self.runtime_close.take() else {
            return SourceWorkerRetirement::inline();
        };
        close.closed.store(true, Ordering::Release);
        SourceWorkerRetirement::new(&close)
    }

    pub fn health_snapshot(&self) -> SourceWorkerHealthSnapshot {
        self.health.snapshot()
    }

    pub fn load_snapshot(&self) -> Option<SourceWorkerLoadSnapshot> {
        self.load.as_ref().map(SourceWorkerLoad::snapshot)
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

    fn latch_invalid_block(&self) {
        self.health.latch(SourceWorkerHealth::InvalidBlock, 0b11);
    }
}

mod mailboxes;

mod deadlines;

mod quantum;

mod controls;

#[cfg(all(feature = "routing-tree-executor", any(test, feature = "test-support")))]
mod pipeline_probe;
#[cfg(all(feature = "routing-tree-executor", any(test, feature = "test-support")))]
pub use pipeline_probe::RoutingTreePipelineProbe;

mod recovery;

#[cfg(test)]
pub(super) mod test_support;

#[cfg(test)]
mod sample_test_support;

#[cfg(test)]
mod recovery_test_support;

#[cfg(test)]
mod collection_test_support;

#[cfg(test)]
mod residency_test_support;

#[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
mod feature_support;

#[cfg(feature = "routing-tree-executor")]
mod routing_tree_control_gate;
#[cfg(feature = "routing-tree-executor")]
mod routing_tree_pipeline;

mod runtime_helpers;

mod construction;
