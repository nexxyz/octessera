use super::WorkerCommand;
use crate::types::RuntimeMessagesPayload;
use playback_runtime::MidiControlChangeBatch;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;

pub(super) const MAX_COMMANDS_PER_WAKE: usize = 16;
pub(super) const RETAINED_RUNTIME_OUTBOX_BATCHES: usize = 64;

/// MIDI realtime and keyboard notes run first in arrival order, then the Control Changes of
/// the batch (latest value per channel and controller), then everything else.
pub(super) fn queue_by_priority(
    command: WorkerCommand,
    realtime: &mut Vec<WorkerCommand>,
    control_changes: &mut MidiControlChangeBatch,
    normal: &mut VecDeque<WorkerCommand>,
) {
    match command {
        WorkerCommand::NativeMidiRealtime(..) | WorkerCommand::NativeMidiNote(..) => {
            realtime.push(command);
        }
        WorkerCommand::NativeMidiControlChange(change) => control_changes.push(change),
        _ => normal.push_back(command),
    }
}

pub(super) enum BatchStep {
    Command(WorkerCommand),
    /// Advance internal playback before the next normal command and after the last one.
    Advance,
}

/// Runs every command of one wake in priority order. A failing step does not drop the rest, so
/// later dispatch replies and the exit flush still run; the failures come back in order.
pub(super) fn run_command_batch(
    first: WorkerCommand,
    rx: &Receiver<WorkerCommand>,
    mut step: impl FnMut(BatchStep) -> Result<(), String>,
) -> Vec<String> {
    let mut realtime = Vec::new();
    let mut control_changes = MidiControlChangeBatch::default();
    let mut normal = VecDeque::new();
    queue_by_priority(first, &mut realtime, &mut control_changes, &mut normal);
    for command in rx.try_iter().take(MAX_COMMANDS_PER_WAKE - 1) {
        queue_by_priority(command, &mut realtime, &mut control_changes, &mut normal);
    }
    let mut errors = Vec::new();
    let mut run = |batch_step| {
        if let Err(error) = step(batch_step) {
            errors.push(error);
        }
    };
    for command in realtime {
        run(BatchStep::Command(command));
    }
    for change in control_changes {
        run(BatchStep::Command(WorkerCommand::NativeMidiControlChange(
            change,
        )));
    }
    while let Some(command) = normal.pop_front() {
        run(BatchStep::Advance);
        run(BatchStep::Command(command));
    }
    run(BatchStep::Advance);
    errors
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
