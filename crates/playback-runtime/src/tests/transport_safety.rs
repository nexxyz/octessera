use super::support::{set_runtime_playing, FakeHost};
use crate::{
    HostMessage, MusicalEvent, PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimeErrorDomain,
    RuntimeOperation, RuntimePlatformEffect, RuntimeRecovery, RuntimeStatus, RuntimeStatusState,
    RuntimeTransportState, SyncSource,
};
use serde_json::json;

fn status(transport: RuntimeTransportState, sync_source: SyncSource) -> RunnerMessage {
    RunnerMessage::RuntimeStatus {
        status: RuntimeStatus {
            state: match transport {
                RuntimeTransportState::Playing => RuntimeStatusState::Running,
                RuntimeTransportState::Paused => RuntimeStatusState::Paused,
                RuntimeTransportState::Stopped => RuntimeStatusState::Idle,
            },
            transport,
            current_ppqn_pulse: 0,
            pending_resync: false,
            sync_source,
            message: None,
            error: None,
        },
    }
}

fn stopped_status() -> RunnerMessage {
    status(RuntimeTransportState::Stopped, SyncSource::Internal)
}

fn assert_device_stop_silences_internal_audio_without_midi_panic(modifier: Option<&str>) {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = crate::NativeRunner::new(crate::NativeRunnerConfig::default()).unwrap();
    let mut host = FakeHost::default();
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "button_s", "pressed": true }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    host.silence_calls = 0;
    let input = modifier
        .map(|modifier| json!({ "type": format!("button_{modifier}"), "pressed": true }))
        .unwrap_or_else(|| json!({ "type": "button_s", "pressed": true }));
    if modifier.is_some() {
        runtime
            .dispatch_host_message(
                HostMessage::DeviceInput {
                    input,
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
        runtime
            .dispatch_host_message(
                HostMessage::DeviceInput {
                    input: json!({ "type": "button_s", "pressed": true }),
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
    } else {
        runtime
            .dispatch_host_message(
                HostMessage::DeviceInput {
                    input,
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
    }
    assert_eq!(host.silence_calls, 1);
    assert!(host
        .effects
        .iter()
        .all(|effect| !matches!(effect, RuntimePlatformEffect::MidiPanic)));
}

#[test]
fn pause_and_modifier_stops_silence_internal_audio_without_midi_panic() {
    assert_device_stop_silences_internal_audio_without_midi_panic(None);
    assert_device_stop_silences_internal_audio_without_midi_panic(Some("shift"));
    assert_device_stop_silences_internal_audio_without_midi_panic(Some("fn"));
}

#[test]
fn midi_disabled_transport_stop_has_no_outbound_bytes() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: false,
        ..RuntimeConfig::default()
    });
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    runtime
        .ingest_runner_messages(vec![stopped_status()], &mut host)
        .unwrap();

    assert!(host.midi_messages.is_empty());
    assert_eq!(host.silence_calls, 1);
}

#[test]
fn enabled_transport_stop_sends_bounded_note_cleanup_without_midi_panic() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 2,
                    note: 64,
                    velocity: 100,
                    duration_ms: Some(10_000),
                }],
            }],
            &mut host,
        )
        .unwrap();
    host.midi_messages.clear();

    runtime
        .ingest_runner_messages(vec![stopped_status()], &mut host)
        .unwrap();

    assert_eq!(host.midi_messages, vec![vec![0xFC], vec![0x82, 64, 0]]);
    assert!(host
        .midi_messages
        .iter()
        .all(|message| { !matches!(message.as_slice(), [0xB0..=0xBF, 120 | 123, 0]) }));
    assert_eq!(host.silence_calls, 1);
    assert!(!runtime.has_scheduled_midi());
}

#[test]
fn disabling_midi_flushes_scheduled_note_offs_before_the_gate_changes() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 1,
                    note: 67,
                    velocity: 100,
                    duration_ms: Some(10_000),
                }],
            }],
            &mut host,
        )
        .unwrap();
    host.midi_messages.clear();

    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::RuntimeConfigChanged {
                config: RuntimeConfig {
                    midi_out_enabled: false,
                    ..runtime.config().clone()
                },
            }],
            &mut host,
        )
        .unwrap();

    assert_eq!(host.midi_messages, vec![vec![0x81, 67, 0]]);
    assert!(!runtime.has_scheduled_midi());
    assert!(!runtime.config().midi_out_enabled);
}

#[test]
fn external_sync_flushes_owned_note_offs_without_transport_bytes() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages(
            vec![status(RuntimeTransportState::Playing, SyncSource::External)],
            &mut host,
        )
        .unwrap();
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 2,
                    note: 64,
                    velocity: 100,
                    duration_ms: Some(10_000),
                }],
            }],
            &mut host,
        )
        .unwrap();
    host.midi_messages.clear();

    runtime
        .ingest_runner_messages(
            vec![status(RuntimeTransportState::Stopped, SyncSource::External)],
            &mut host,
        )
        .unwrap();

    assert_eq!(host.midi_messages, vec![vec![0x82, 64, 0]]);
    assert_eq!(host.silence_calls, 1);
    assert!(!runtime.has_scheduled_midi());
}

#[test]
fn midi_disable_send_failure_aligns_runtime_gate_and_clears_owned_notes() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 1,
                    note: 67,
                    velocity: 100,
                    duration_ms: Some(10_000),
                }],
            }],
            &mut host,
        )
        .unwrap();
    host.midi_messages.clear();
    host.fail_midi_message = true;

    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::RuntimeConfigChanged {
                config: RuntimeConfig {
                    midi_out_enabled: false,
                    ..runtime.config().clone()
                },
            }],
            &mut host,
        )
        .unwrap();

    assert!(!runtime.config().midi_out_enabled);
    assert!(!runtime.has_scheduled_midi());
    let error = runtime.latched_errors().last().unwrap();
    assert_eq!(error.domain, RuntimeErrorDomain::Midi);
    assert_eq!(error.operation, RuntimeOperation::MidiMessage);
    assert_eq!(error.recovery, RuntimeRecovery::RetainLastGood);
    let presented_error = runtime
        .last_status()
        .and_then(|status| status.error.as_ref())
        .unwrap();
    assert_eq!(presented_error.domain, RuntimeErrorDomain::Midi);
    assert_eq!(presented_error.operation, RuntimeOperation::MidiMessage);
    assert_eq!(presented_error.recovery, RuntimeRecovery::RetainLastGood);
}
