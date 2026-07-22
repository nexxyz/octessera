use super::support::{set_runtime_playing, FakeHost, FakeRunner};
use crate::{
    CoreRunner, HostMessage, MusicalEvent, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimePlatformEffect, SyncSource,
};
use serde_json::json;

#[test]
fn internal_clock_advances_runner_and_schedules_note_offs() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: true,
        midi_out_enabled: true,
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    runtime.advance(500, &mut runner, &mut host).unwrap();

    assert!(matches!(
        runner.seen[0],
        HostMessage::TransportPulseStep { pulses: 24, .. }
    ));
    assert_eq!(host.midi_messages[0], vec![0x90, 60, 100]);
    assert_eq!(host.midi_messages[1], vec![0xF8]);
    assert_eq!(host.midi_messages.len(), 25);

    runtime.advance(30, &mut runner, &mut host).unwrap();
    assert!(host.midi_messages.contains(&vec![0x80, 60, 0]));
}

#[test]
fn internal_clock_preserves_sub_millisecond_transport_time() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    for _ in 0..100 {
        runtime
            .advance_duration(
                std::time::Duration::from_micros(8_900),
                &mut runner,
                &mut host,
            )
            .unwrap();
    }

    let pulses = runner
        .seen
        .iter()
        .filter_map(|message| match message {
            HostMessage::TransportPulseStep { pulses, .. } => Some(*pulses),
            _ => None,
        })
        .sum::<u32>();
    assert_eq!(pulses, 42);
}

#[test]
fn midi_only_events_send_midi_without_host_audio_and_schedule_note_off() {
    struct MidiOnlyRunner;

    impl CoreRunner for MidiOnlyRunner {
        fn send(&mut self, _message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
            Ok(vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 1,
                    note: 64,
                    velocity: 90,
                    duration_ms: Some(30),
                }],
            }])
        }
    }

    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: true,
    });
    let mut runner = MidiOnlyRunner;
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    runtime.advance(500, &mut runner, &mut host).unwrap();
    runtime.advance(30, &mut runner, &mut host).unwrap();

    assert!(host.musical_events.is_empty());
    assert_eq!(host.midi_messages[0], vec![0x91, 64, 90]);
    assert!(host.midi_messages.contains(&vec![0x81, 64, 0]));
}

#[test]
fn external_midi_realtime_bytes_preserve_clock_control_order() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::External,
        midi_clock_out_enabled: true,
        midi_out_enabled: true,
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();

    runtime
        .handle_midi_realtime_bytes(
            &[0xF8, 0xF8, 0xFA, 0xF8, 0xFB, 0xF8, 0xFC],
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert_eq!(runner.seen.len(), 6);
    assert!(matches!(
        runner.seen[0],
        HostMessage::MidiRealtimeClock { pulses: 2 }
    ));
    assert!(matches!(runner.seen[1], HostMessage::MidiRealtimeStart));
    assert!(matches!(
        runner.seen[2],
        HostMessage::MidiRealtimeClock { pulses: 1 }
    ));
    assert!(matches!(runner.seen[3], HostMessage::MidiRealtimeContinue));
    assert!(matches!(
        runner.seen[4],
        HostMessage::MidiRealtimeClock { pulses: 1 }
    ));
    assert!(matches!(runner.seen[5], HostMessage::MidiRealtimeStop));
    assert!(host.midi_messages.is_empty());
}

#[test]
fn host_effect_results_round_trip_back_into_runner() {
    struct EffectRunner;

    impl CoreRunner for EffectRunner {
        fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
            match message {
                HostMessage::TransportPulseStep { .. } => {
                    Ok(vec![RunnerMessage::PlatformEffects {
                        effects: vec![RuntimePlatformEffect::StoreListPresets],
                    }])
                }
                HostMessage::RuntimeResult { result } => Ok(vec![RunnerMessage::Snapshot {
                    snapshot: json!({ "roundTrip": result }),
                }]),
                _ => Ok(vec![]),
            }
        }
    }

    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = EffectRunner;
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);

    runtime.advance(500, &mut runner, &mut host).unwrap();

    assert_eq!(host.effects, vec![RuntimePlatformEffect::StoreListPresets]);
    assert_eq!(
        runtime.last_snapshot(),
        Some(&json!({
            "roundTrip": {
                "type": "identified",
                "requestId": "platform-1",
                "revision": null,
                "result": {
                    "type": "list_presets_result",
                    "names": ["Factory", "Live Set"]
                }
            }
        }))
    );
}
