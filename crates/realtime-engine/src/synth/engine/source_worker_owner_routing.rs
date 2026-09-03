use super::super::source_worker_health::{SourceWorkerHealth, SourceWorkerHealthState};
use super::super::source_worker_protocol::WorkerPhase;
use super::{CompletedEnvelope, OwnerEnvelope, SOURCE_WORKER_COUNT};
use crossbeam_channel::Sender;

pub(super) fn route_owner(
    owner: OwnerEnvelope,
    home_txs: &[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    fault_txs: &[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    health: &SourceWorkerHealthState,
    home: bool,
    fallback_parity: usize,
) -> Option<OwnerEnvelope> {
    let parity = if owner.parity < SOURCE_WORKER_COUNT {
        owner.parity
    } else {
        health.latch(SourceWorkerHealth::CompletionFailed, 0b11);
        fallback_parity
    };
    if parity >= SOURCE_WORKER_COUNT {
        return Some(owner);
    }
    let target = if home {
        &home_txs[parity]
    } else {
        &fault_txs[parity]
    };
    match target.try_send(owner) {
        Ok(()) => None,
        Err(error) => {
            let owner = error.into_inner();
            health.latch(SourceWorkerHealth::CompletionFailed, worker_mask(parity));
            if home {
                match fault_txs[parity].try_send(owner) {
                    Ok(()) => None,
                    Err(error) => Some(error.into_inner()),
                }
            } else {
                Some(owner)
            }
        }
    }
}

pub(super) fn route_completion(
    completion: CompletedEnvelope,
    home_txs: &[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    fault_txs: &[Sender<OwnerEnvelope>; SOURCE_WORKER_COUNT],
    health: &SourceWorkerHealthState,
    fallback_parity: usize,
) -> Option<OwnerEnvelope> {
    let home = completion.phase == WorkerPhase::Sources
        && completion.render_ok
        && !completion.worker_exited
        && !completion.transport_failed;
    route_owner(
        completion.owner,
        home_txs,
        fault_txs,
        health,
        home,
        fallback_parity,
    )
}

pub(super) fn worker_mask(parity: usize) -> u8 {
    if parity < SOURCE_WORKER_COUNT {
        1 << parity
    } else {
        0b11
    }
}
