use super::*;
use crate::tests::support::{canonical_oled_snapshot, FakeHost};
use crate::{
    CoreRunner, HostMessage, MusicalEvent, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimeStatus, RuntimeStatusState, RuntimeTransportState, SyncSource,
};

struct PulseRunner;

impl CoreRunner for PulseRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let HostMessage::TransportPulseStep {
            pulses,
            request_snapshot,
            ..
        } = message
        else {
            panic!("expected pulse step");
        };
        let mut messages = vec![RunnerMessage::MusicalEvents {
            events: vec![MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: None,
            }],
        }];
        if request_snapshot == Some(true) {
            messages.push(RunnerMessage::Snapshot {
                snapshot: canonical_oled_snapshot("next"),
            });
        }
        messages.push(RunnerMessage::RuntimeStatus {
            status: playing_status(pulses as u64),
        });
        Ok(messages)
    }

    fn send_system_store_result(
        &mut self,
        _message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<crate::RuntimeStoreResult>), String> {
        Ok((Vec::new(), None))
    }
}

fn playing_status(pulse: u64) -> RuntimeStatus {
    RuntimeStatus {
        state: RuntimeStatusState::Running,
        transport: RuntimeTransportState::Playing,
        current_ppqn_pulse: pulse,
        pending_resync: false,
        sync_source: SyncSource::Internal,
        message: None,
        error: None,
    }
}

#[test]
fn trace_requires_exact_opt_in() {
    assert!(!enabled(None));
    assert!(!enabled(Some("true")));
    assert!(!enabled(Some("0")));
    assert!(enabled(Some("1")));
}

#[test]
fn trace_separates_status_only_from_full_snapshot_without_changing_output() {
    let mut traced = PlaybackRuntime::new(RuntimeConfig::default());
    traced.dispatch_profile = Some(RuntimeDispatchProfile::new());
    let mut untraced = PlaybackRuntime::new(RuntimeConfig::default());
    let mut traced_host = FakeHost::default();
    let mut untraced_host = FakeHost::default();
    let initial = vec![
        RunnerMessage::Snapshot {
            snapshot: canonical_oled_snapshot("first"),
        },
        RunnerMessage::RuntimeStatus {
            status: playing_status(0),
        },
    ];
    assert_eq!(
        traced
            .ingest_runner_messages_with_output(initial.clone(), &mut traced_host)
            .unwrap(),
        untraced
            .ingest_runner_messages_with_output(initial, &mut untraced_host)
            .unwrap(),
    );
    let mut runner = PulseRunner;
    for (pulses, request_snapshot) in [(2, None), (1, Some(true))] {
        let message = HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: Some(0),
            request_snapshot,
        };
        assert_eq!(
            traced
                .dispatch_host_message(message.clone(), &mut runner, &mut traced_host)
                .unwrap(),
            untraced
                .dispatch_host_message(message, &mut runner, &mut untraced_host)
                .unwrap(),
        );
        assert_eq!(traced.oled_frame_revision(), untraced.oled_frame_revision());
        assert_eq!(
            traced.last_snapshot_revision(),
            untraced.last_snapshot_revision()
        );
    }
    assert_eq!(traced_host.musical_events, untraced_host.musical_events);
    assert_eq!(traced_host.musical_events.len(), 2);
    assert_eq!(traced_host.midi_messages, untraced_host.midi_messages);
    let profile = traced.dispatch_profile.as_ref().unwrap();
    assert_eq!(profile.dispatches, 2);
    assert_eq!(profile.pulses, 3);
    assert_eq!(profile.max_pulses, 2);
    assert_eq!(profile.snapshot_requests, 1);
    assert_eq!(profile.snapshots_received, 1);
    assert_eq!(profile.spans[Stage::StatusRefresh as usize].n, 0);
    assert_eq!(profile.spans[Stage::SnapshotClone as usize].n, 0);
    assert_eq!(profile.spans[Stage::SemanticCompare as usize].n, 0);
    assert_eq!(profile.spans[Stage::FrameRender as usize].n, 0);
    assert_eq!(profile.spans[Stage::FullSnapshotRefresh as usize].n, 1);
}
