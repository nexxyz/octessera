use super::queue::{
    queue_by_priority, retain_runtime_outbox_batch, RETAINED_RUNTIME_OUTBOX_BATCHES,
};
use super::WorkerCommand;
use crate::types::RuntimeMessagesPayload;
use std::collections::VecDeque;
use std::sync::mpsc;

#[test]
fn runtime_outbox_retains_capped_monotonic_tail() {
    let mut outbox = Vec::new();
    for seq in 1..=RETAINED_RUNTIME_OUTBOX_BATCHES as u64 + 2 {
        retain_runtime_outbox_batch(
            &mut outbox,
            RuntimeMessagesPayload {
                seq,
                messages: vec![serde_json::json!(seq)],
            },
        );
    }

    assert_eq!(outbox.len(), RETAINED_RUNTIME_OUTBOX_BATCHES);
    assert_eq!(outbox.first().map(|payload| payload.seq), Some(3));
    assert_eq!(
        outbox.last().map(|payload| payload.seq),
        Some(RETAINED_RUNTIME_OUTBOX_BATCHES as u64 + 2)
    );
}

#[test]
fn worker_command_priority_separates_midi_realtime_from_normal_work() {
    let (dispatch_tx, _) = mpsc::channel();
    let mut realtime = Vec::new();
    let mut normal = VecDeque::new();

    queue_by_priority(
        WorkerCommand::Dispatch(playback_runtime::HostMessage::MidiRealtimeStop, dispatch_tx),
        &mut realtime,
        &mut normal,
    );
    queue_by_priority(
        WorkerCommand::NativeMidiRealtime(vec![0xF8]),
        &mut realtime,
        &mut normal,
    );

    assert_eq!(realtime.len(), 1);
    assert_eq!(normal.len(), 1);
    assert!(matches!(realtime[0], WorkerCommand::NativeMidiRealtime(_)));
    assert!(matches!(normal[0], WorkerCommand::Dispatch(_, _)));
}
