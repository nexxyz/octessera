use super::super::routing_tree_worker::RoutingTreeOutputBlock;
use super::super::source_worker_health::{SourceWorkerHealth, SourceWorkerHealthState};
use super::super::source_worker_lifecycle::SourceWorkerLifecycle;
use super::super::source_worker_load::SourceWorkerLoad;
use super::super::source_worker_protocol::SourceWorkerMode;
use super::SourceWorkerRuntime;
use std::sync::Arc;

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
            worker_pauses: None,
            #[cfg(any(test, feature = "test-support"))]
            worker_pause_entries: None,
            runtime_close: None,
            health: Arc::new(SourceWorkerHealthState::new(SourceWorkerHealth::Disabled)),
            runtime_generation: 0,
            next_sequence: 0,
            expected_stamp: None,
            in_flight_mask: 0,
            completed_mask: 0,
            sample_rate: 0,
            lookahead_frames: 0,
            routing_output_spares: None,
            routing_output_stamp: None,
            routing_output_ready: false,
            routing_tree_reprime_pending: false,
            routing_absolute_deadline: None,
            #[cfg(any(test, feature = "test-support"))]
            routing_tree_probe: None,
            load: None,
            source_load_observations: std::array::from_fn(|_| None),
            #[cfg(any(test, feature = "test-support"))]
            deadline_override: None,
        }
    }

    pub(in super::super) fn new_routing_tree(
        lifecycle: &SourceWorkerLifecycle,
        sample_rate: u32,
        active_frames: usize,
    ) -> Option<Self> {
        Self::new_with_mode(
            lifecycle,
            sample_rate,
            active_frames,
            SourceWorkerMode::RoutingTree,
            active_frames,
        )
    }

    fn new_with_mode(
        lifecycle: &SourceWorkerLifecycle,
        sample_rate: u32,
        active_frames: usize,
        mode: SourceWorkerMode,
        lookahead_frames: usize,
    ) -> Option<Self> {
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
            mode,
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
            worker_pauses: Some(lifecycle.worker_pause_controls_for_test()),
            #[cfg(any(test, feature = "test-support"))]
            worker_pause_entries: Some(lifecycle.worker_pause_entry_controls_for_test()),
            runtime_close: Some(Arc::clone(&lifecycle.runtime_close)),
            health: Arc::clone(&lifecycle.health),
            runtime_generation: lifecycle.runtime_generation(),
            next_sequence: 0,
            expected_stamp: None,
            in_flight_mask: 0,
            completed_mask: 0,
            sample_rate,
            lookahead_frames,
            routing_output_spares: (mode == SourceWorkerMode::RoutingTree)
                .then(|| std::array::from_fn(|_| RoutingTreeOutputBlock::new())),
            routing_output_stamp: None,
            routing_output_ready: false,
            routing_tree_reprime_pending: false,
            routing_absolute_deadline: None,
            #[cfg(any(test, feature = "test-support"))]
            routing_tree_probe: None,
            load: Some(SourceWorkerLoad::new(active_frames, sample_rate)),
            source_load_observations: std::array::from_fn(|_| None),
            #[cfg(any(test, feature = "test-support"))]
            deadline_override: None,
        })
    }
}
