use super::WorkerCommand;
use crate::types::RuntimeMessagesPayload;
use std::collections::VecDeque;

pub(super) const MAX_COMMANDS_PER_WAKE: usize = 16;
pub(super) const RETAINED_RUNTIME_OUTBOX_BATCHES: usize = 64;

pub(super) fn queue_by_priority(
    command: WorkerCommand,
    realtime: &mut Vec<WorkerCommand>,
    normal: &mut VecDeque<WorkerCommand>,
) {
    if matches!(command, WorkerCommand::NativeMidiRealtime(_)) {
        realtime.push(command);
    } else {
        normal.push_back(command);
    }
}

pub(super) fn retain_runtime_outbox_batch(
    outbox: &mut Vec<RuntimeMessagesPayload>,
    payload: RuntimeMessagesPayload,
) {
    outbox.push(payload);
    let overflow = outbox.len().saturating_sub(RETAINED_RUNTIME_OUTBOX_BATCHES);
    if overflow > 0 {
        outbox.drain(..overflow);
    }
}
