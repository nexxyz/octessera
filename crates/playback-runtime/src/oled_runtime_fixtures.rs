use super::support::{canonical_oled_snapshot, FakeHost};
use crate::{
    PlaybackRuntime, RunnerMessage, RuntimeStatus, RuntimeStatusState, RuntimeTransportState,
    SyncSource,
};
use serde_json::Value;

pub(super) fn snapshot(title: &str) -> Value {
    canonical_oled_snapshot(title)
}

pub(super) fn status() -> RuntimeStatus {
    RuntimeStatus {
        state: RuntimeStatusState::Idle,
        transport: RuntimeTransportState::Stopped,
        current_ppqn_pulse: 0,
        pending_resync: false,
        sync_source: SyncSource::Internal,
        message: None,
        error: None,
    }
}

pub(super) fn present(runtime: &mut PlaybackRuntime, snapshot: Value) -> Vec<RunnerMessage> {
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages_with_output(
            vec![
                RunnerMessage::Snapshot { snapshot },
                RunnerMessage::RuntimeStatus { status: status() },
            ],
            &mut host,
        )
        .unwrap()
        .messages
}
