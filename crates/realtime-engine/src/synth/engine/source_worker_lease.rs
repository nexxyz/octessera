use super::source_worker_health::{SourceWorkerHealth, SourceWorkerHealthState};
use super::source_worker_lifecycle::OwnerEnvelope;
use crossbeam_channel::Sender;
use std::sync::Arc;

pub(super) struct OwnerLease {
    pub(super) owner: Option<OwnerEnvelope>,
    pub(super) parity: usize,
    pub(super) home_tx: Sender<OwnerEnvelope>,
    pub(super) fault_tx: Sender<OwnerEnvelope>,
    pub(super) health: Arc<SourceWorkerHealthState>,
}

impl OwnerLease {
    pub(super) fn take_owner(&mut self) -> Option<OwnerEnvelope> {
        self.owner.take()
    }

    pub(super) fn restore_owner(&mut self, owner: OwnerEnvelope) {
        self.owner = Some(owner);
    }

    pub(super) fn return_home(&mut self) {
        let Some(owner) = self.owner.take() else {
            return;
        };
        match self.home_tx.try_send(owner) {
            Ok(()) => {}
            Err(error) => {
                self.health
                    .latch(SourceWorkerHealth::CompletionFailed, 1 << self.parity);
                self.return_fault_owner(error.into_inner());
            }
        }
    }

    pub(super) fn return_fault(&mut self) {
        let Some(owner) = self.owner.take() else {
            return;
        };
        self.health
            .latch(SourceWorkerHealth::CompletionFailed, 1 << self.parity);
        self.return_fault_owner(owner);
    }

    fn return_fault_owner(&mut self, owner: OwnerEnvelope) {
        match self.fault_tx.try_send(owner) {
            Ok(()) => {}
            Err(error) => {
                self.health
                    .latch(SourceWorkerHealth::CompletionFailed, 1 << self.parity);
                match self.home_tx.try_send(error.into_inner()) {
                    Ok(()) => {}
                    Err(error) => {
                        self.health
                            .latch(SourceWorkerHealth::CompletionFailed, 1 << self.parity);
                        self.owner = Some(error.into_inner());
                    }
                }
            }
        }
    }
}

impl Drop for OwnerLease {
    fn drop(&mut self) {
        self.return_fault();
    }
}
