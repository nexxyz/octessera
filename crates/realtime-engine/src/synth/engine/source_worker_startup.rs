use super::super::source_worker_health::{SourceWorkerHealth, SourceWorkerHealthState};
use super::super::source_worker_protocol::{SourceWorkerSetupError, SourceWorkerStartHook};
use super::worker::spawn_worker_named;
#[cfg(test)]
use super::worker::ReverseCompletionState;
use super::{SourceWorkerCloseState, SourceWorkerLifecycle, SOURCE_WORKER_MAILBOX_CAPACITY};
use crossbeam_channel::bounded;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

impl SourceWorkerLifecycle {
    pub(crate) fn start_with_hold_and_hook(
        hold_before_receive: bool,
        start_hook: Option<SourceWorkerStartHook>,
    ) -> Result<Self, SourceWorkerSetupError> {
        Self::start_with_worker_names(
            hold_before_receive,
            start_hook,
            super::worker::SOURCE_WORKER_THREAD_NAMES,
        )
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(crate) fn start_routing_tree_with_hold_and_hook(
        hold_before_receive: bool,
        start_hook: Option<SourceWorkerStartHook>,
    ) -> Result<Self, SourceWorkerSetupError> {
        Self::start_with_worker_names(
            hold_before_receive,
            start_hook,
            super::worker::ROUTING_TREE_WORKER_THREAD_NAMES,
        )
    }

    fn start_with_worker_names(
        hold_before_receive: bool,
        start_hook: Option<SourceWorkerStartHook>,
        thread_names: [&'static str; 2],
    ) -> Result<Self, SourceWorkerSetupError> {
        #[cfg(test)]
        let reverse_completion = Arc::new(ReverseCompletionState {
            enabled: AtomicBool::new(false),
            parity_one_done: AtomicBool::new(false),
            completion_order: std::sync::Mutex::new(Vec::new()),
        });
        let generation = super::NEXT_SOURCE_WORKER_GENERATION.fetch_add(1, Ordering::Relaxed);
        let runtime_close = Arc::new(SourceWorkerCloseState {
            closed: AtomicBool::new(false),
            generation,
        });
        let health = Arc::new(SourceWorkerHealthState::new(SourceWorkerHealth::Healthy));
        let (home_tx_0, home_rx_0) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let (home_tx_1, home_rx_1) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let (fault_tx_0, fault_rx_0) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let (fault_tx_1, fault_rx_1) = bounded(SOURCE_WORKER_MAILBOX_CAPACITY);
        let first_worker = match spawn_worker_named(
            0,
            #[cfg(test)]
            Arc::clone(&reverse_completion),
            hold_before_receive,
            start_hook,
            thread_names[0],
        ) {
            Ok(worker) => worker,
            Err(error) => {
                runtime_close.closed.store(true, Ordering::Release);
                return Err(error);
            }
        };
        let second_worker = match spawn_worker_named(
            1,
            #[cfg(test)]
            Arc::clone(&reverse_completion),
            hold_before_receive,
            start_hook,
            thread_names[1],
        ) {
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

    pub(crate) fn prewarm(&mut self) -> Result<(), SourceWorkerSetupError> {
        if self.prewarmed {
            return Ok(());
        }
        for worker in &self.workers {
            match worker.ready_rx.recv() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(SourceWorkerSetupError::PrewarmFailed),
            }
        }
        self.prewarmed = true;
        Ok(())
    }
}
