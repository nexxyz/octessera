#[cfg(feature = "source-worker-benchmark-timing")]
use super::super::source_worker_timing::SourceWorkerTimingProbe;
#[cfg(feature = "routing-tree-benchmark")]
use super::routing_tree_worker::RoutingTreeOutputBlock;
use super::source_worker_health::{
    SourceWorkerHealth, SourceWorkerHealthSnapshot, SourceWorkerHealthState,
};
use super::source_worker_lease::OwnerLease;
#[cfg(test)]
use super::source_worker_lifecycle::SourceWorkerLifecycle;
use super::source_worker_lifecycle::{
    CompletedEnvelope, OwnerEnvelope, SourceWorkerCloseState, SourceWorkerScratch,
};
use super::source_worker_load::{SourceWorkerLoad, SourceWorkerLoadSnapshot};
use super::source_worker_protocol::{SourceWorkerMode, WorkStamp, WorkerCommand, WorkerPhase};
use super::source_worker_retirement::SourceWorkerRetirement;
use super::SynthEngine;
use super::BLOCK_SLOT_SCRATCH_FRAMES;
use crossbeam_channel::{Receiver, Sender};
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use std::time::Duration;
use std::time::Instant;

pub(super) const SOURCE_WORKER_COUNT: usize = 2;

pub struct SourceWorkerRuntime {
    mode: SourceWorkerMode,
    work_txs: Option<[Sender<WorkerCommand>; SOURCE_WORKER_COUNT]>,
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
    #[cfg(feature = "source-worker-benchmark-timing")]
    coordinator_remainder_started_at: Option<Instant>,
    #[cfg(feature = "source-worker-benchmark-timing")]
    timing_output_sequence: Option<u64>,
    #[cfg(all(
        feature = "routing-tree-benchmark",
        feature = "source-worker-benchmark-timing"
    ))]
    routing_coordinator_remainder_started_at: Option<(u64, Instant)>,
    #[cfg(any(test, feature = "test-support"))]
    worker_pauses: Option<[Arc<AtomicBool>; SOURCE_WORKER_COUNT]>,
    #[cfg(any(test, feature = "test-support"))]
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
    #[cfg(feature = "routing-tree-benchmark")]
    routing_output_spares: Option<[RoutingTreeOutputBlock; SOURCE_WORKER_COUNT]>,
    #[cfg(feature = "routing-tree-benchmark")]
    routing_output_stamp: Option<WorkStamp>,
    #[cfg(feature = "routing-tree-benchmark")]
    routing_output_ready: bool,
    #[cfg(feature = "routing-tree-benchmark")]
    routing_tree_reprime_pending: bool,
    #[cfg(feature = "routing-tree-benchmark")]
    routing_absolute_deadline: Option<Instant>,
    #[cfg(all(
        feature = "routing-tree-benchmark",
        any(test, feature = "test-support")
    ))]
    routing_tree_probe: Option<Arc<RoutingTreePipelineProbe>>,
    load: Option<SourceWorkerLoad>,
    source_load_observations:
        [Option<super::source_worker_load::SourceWorkerLoadObservation>; SOURCE_WORKER_COUNT],
    bus_load_observations:
        [Option<super::source_worker_load::SourceWorkerLoadObservation>; SOURCE_WORKER_COUNT],
    bus_dispatch_residency: [u8; super::super::types::BUS_COUNT],
    bus_dispatch_residency_valid: bool,
    force_fault_mask: u8,
    #[cfg(any(test, feature = "test-support"))]
    deadline_override: Option<Duration>,
    #[cfg(test)]
    before_bus_dispatch: Option<fn(&mut SourceWorkerRuntime, &mut Instant)>,
    #[cfg(test)]
    after_bus_dispatch: Option<fn(&mut SourceWorkerRuntime)>,
}

impl SourceWorkerRuntime {
    pub fn mode(&self) -> SourceWorkerMode {
        self.mode
    }

    pub fn lookahead_frames(&self) -> usize {
        self.lookahead_frames
    }

    pub fn retire(mut self) -> SourceWorkerRetirement {
        #[cfg(feature = "source-worker-benchmark-timing")]
        self.freeze_timing(self.health.status().is_terminal(), None);
        #[cfg(feature = "routing-tree-benchmark")]
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

    pub(super) fn render_source_block(&mut self, engine: &mut SynthEngine, frames: usize) -> bool {
        self.render_source_block_with(engine, frames, |_| ())
            .is_some()
    }

    pub(super) fn render_source_block_with<R>(
        &mut self,
        engine: &mut SynthEngine,
        frames: usize,
        render: impl FnOnce(&mut SynthEngine) -> R,
    ) -> Option<R> {
        #[cfg(any(test, feature = "test-support"))]
        self.render_attempts.fetch_add(1, Ordering::Relaxed);
        if self.mode == SourceWorkerMode::Inline {
            return None;
        }
        if self.health.status() != SourceWorkerHealth::Healthy {
            self.reclaim_available(engine);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return None;
        }
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            self.latch_invalid_block();
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return None;
        }
        if self.in_flight_mask != 0 || !self.home_is_ready() {
            self.latch_dispatch_failure(0b11);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return None;
        }
        self.expected_stamp = Some(WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: engine.render_plan.generation,
            quantum_sequence: self.next_sequence,
            frames,
            base_sample_clock: engine.sample_clock,
        });
        if !self.dispatch(engine) {
            self.reclaim_available(engine);
            #[cfg(feature = "source-worker-benchmark-timing")]
            self.freeze_timing(true, None);
            return None;
        }
        self.collect_with(engine, true, render)
    }

    #[cfg(test)]
    fn collect(&mut self, engine: &mut SynthEngine, wait: bool) -> bool {
        self.collect_with(engine, wait, |_| ()).is_some()
    }

    fn collect_with<R>(
        &mut self,
        engine: &mut SynthEngine,
        wait: bool,
        render: impl FnOnce(&mut SynthEngine) -> R,
    ) -> Option<R> {
        let start = Instant::now();
        let expected_frames = self.expected_stamp.map_or(0, |stamp| stamp.frames);
        let deadline = start + self.rendezvous_deadline(expected_frames);
        self.collect_wave_with_deadline(
            engine,
            wait,
            super::source_worker_protocol::WorkerPhase::Sources,
            deadline,
            true,
        )?;
        self.finish_completed(engine, render)
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

#[path = "source_worker_mailboxes.rs"]
mod mailboxes;

#[path = "source_worker_deadlines.rs"]
mod deadlines;

#[path = "source_worker_quantum.rs"]
mod quantum;

#[path = "source_worker_dispatch.rs"]
mod dispatch;

#[path = "source_worker_completion.rs"]
mod completion;

#[path = "source_worker_controls.rs"]
mod controls;

#[cfg(all(
    feature = "routing-tree-benchmark",
    any(test, feature = "test-support")
))]
#[path = "source_worker_pipeline_probe.rs"]
mod pipeline_probe;
#[cfg(all(
    feature = "routing-tree-benchmark",
    any(test, feature = "test-support")
))]
pub use pipeline_probe::RoutingTreePipelineProbe;

#[path = "source_worker_recovery.rs"]
mod recovery;

#[cfg(feature = "source-worker-benchmark-timing")]
#[path = "source_worker_timing_integration.rs"]
mod timing_integration;

#[path = "source_worker_reduce.rs"]
mod reduction;

#[cfg(test)]
#[path = "source_worker_test_support.rs"]
pub(super) mod test_support;

#[cfg(test)]
#[path = "source_worker_sample_test_support.rs"]
mod sample_test_support;

#[cfg(test)]
#[path = "source_worker_recovery_test_support.rs"]
mod recovery_test_support;

#[cfg(test)]
#[path = "source_worker_collection_test_support.rs"]
mod collection_test_support;

#[cfg(test)]
#[path = "source_worker_residency_test_support.rs"]
mod residency_test_support;

#[cfg(any(test, feature = "test-support"))]
#[path = "source_worker_feature_support.rs"]
mod feature_support;

#[cfg(feature = "routing-tree-benchmark")]
#[path = "routing_tree_control_gate.rs"]
mod routing_tree_control_gate;
#[cfg(feature = "routing-tree-benchmark")]
#[path = "routing_tree_pipeline.rs"]
mod routing_tree_pipeline;

#[path = "source_worker_runtime_helpers.rs"]
mod runtime_helpers;

#[path = "source_worker_construction.rs"]
mod construction;
