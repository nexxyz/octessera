use super::WorkerCommand;
use crate::types::RuntimeMessagesPayload;
use playback_runtime::MidiControlChangeBatch;
use std::collections::VecDeque;

pub(super) const MAX_COMMANDS_PER_WAKE: usize = 16;
pub(super) const RETAINED_RUNTIME_OUTBOX_BATCHES: usize = 64;

/// MIDI realtime runs first, then the Control Changes of the batch (latest value per channel
/// and controller), then everything else.
pub(super) fn queue_by_priority(
    command: WorkerCommand,
    realtime: &mut Vec<WorkerCommand>,
    control_changes: &mut MidiControlChangeBatch,
    normal: &mut VecDeque<WorkerCommand>,
) {
    match command {
        WorkerCommand::NativeMidiRealtime(..) => realtime.push(command),
        WorkerCommand::NativeMidiControlChange(change) => control_changes.push(change),
        _ => normal.push_back(command),
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
