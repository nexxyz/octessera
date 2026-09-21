use super::super::host_adapter_audio::audio_queue_error;
use super::{next_event, platform_request, test_adapter};
use playback_runtime::{
    HostAdapter, MusicalEvent, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeOperation, RuntimePlatformEffect,
};
use rodio_engine_source::{EngineEvent, QueueKind, QueueSendError};
use std::collections::BTreeMap;

fn audio(command: RuntimeAudioCommand) -> RuntimePlatformEffect {
    RuntimePlatformEffect::AudioCommand { command }
}

#[test]
fn platform_effect_audio_command_reaches_structural_audio_lane() {
    let (mut adapter, mut rx) = test_adapter();
    adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::MomentaryFxStart {
                id: "preview".into(),
                epoch: 4,
                fx_type: "freeze".into(),
                params: BTreeMap::new(),
                target: playback_runtime::RuntimeMomentaryFxTarget::Global,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::PreparedMomentaryFxStart { .. }
    ));
    adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "preview".into(),
                epoch: 4,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::MomentaryFxStop { epoch: 4 }
    ));
}

#[test]
fn musical_event_rejects_out_of_range_controls() {
    let cases = [
        MusicalEvent::NoteOn {
            channel: 8,
            note: 60,
            velocity: 100,
            duration_ms: None,
        },
        MusicalEvent::NoteOn {
            channel: 0,
            note: 128,
            velocity: 100,
            duration_ms: None,
        },
        MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 0,
            duration_ms: None,
        },
        MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(9),
        },
        MusicalEvent::Cc {
            channel: 0,
            controller: 128,
            value: 1,
        },
    ];
    for event in cases {
        let (mut adapter, mut rx) = test_adapter();
        let error = adapter.handle_musical_event(&event).unwrap_err();
        assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn audio_queue_errors_distinguish_full_and_disconnected_lanes() {
    let full = audio_queue_error(
        QueueSendError::Full {
            queue: QueueKind::Latest,
        },
        RuntimeOperation::AudioCommand,
    );
    assert_eq!(full.facts.code, RuntimeErrorCode::OperationFailed);
    let disconnected = audio_queue_error(
        QueueSendError::Disconnected {
            queue: QueueKind::Latest,
        },
        RuntimeOperation::AudioCommand,
    );
    assert_eq!(disconnected.facts.code, RuntimeErrorCode::AudioThreadFailed);
}

#[test]
fn latest_full_is_typed_without_emergency_and_disconnect_is_typed() {
    let (adapter, mut rx) = test_adapter();
    let full = adapter
        .handle_engine_send_result(
            Err(QueueSendError::Full {
                queue: QueueKind::Latest,
            }),
            RuntimeOperation::AudioCommand,
        )
        .unwrap_err();
    assert_eq!(full.facts.code, RuntimeErrorCode::OperationFailed);
    assert!(rx.try_recv().is_err());

    let disconnected = adapter
        .handle_engine_send_result(
            Err(QueueSendError::Disconnected {
                queue: QueueKind::Latest,
            }),
            RuntimeOperation::AudioCommand,
        )
        .unwrap_err();
    assert_eq!(disconnected.facts.code, RuntimeErrorCode::AudioThreadFailed);
    assert!(rx.try_recv().is_err());
}

#[test]
fn musical_note_preserves_exact_native_values() {
    let (mut adapter, mut rx) = test_adapter();
    adapter
        .handle_musical_event(&MusicalEvent::NoteOn {
            channel: 7,
            note: 127,
            velocity: 127,
            duration_ms: Some(10),
        })
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::NoteOn {
            instrument_slot: 7,
            note: 127,
            velocity: 127,
            duration_ms: 10,
        }
    ));
}

#[test]
fn latest_scalar_flood_remains_admitted_without_error() {
    let (mut adapter, _rx) = test_adapter();
    for index in 0..10_000 {
        adapter
            .handle_platform_effect(&platform_request(audio(
                RuntimeAudioCommand::SetMasterVolume {
                    generation: index,
                    volume_pct: (index % 101) as f32,
                },
            )))
            .unwrap();
    }
}

#[test]
fn chunked_ten_thousand_musical_events_preserve_order_without_scalar_loss() {
    const CHUNK_SIZE: usize = 250;
    const EVENT_COUNT: usize = 10_000;
    let (mut adapter, mut rx) = test_adapter();
    for chunk_start in (0..EVENT_COUNT).step_by(CHUNK_SIZE) {
        for index in chunk_start..chunk_start + CHUNK_SIZE {
            adapter
                .handle_musical_event(&MusicalEvent::NoteOn {
                    channel: (index % 8) as u8,
                    note: (index % 128) as u8,
                    velocity: 1 + (index % 127) as u8,
                    duration_ms: Some(10 + index as u32),
                })
                .unwrap();
        }
        adapter
            .handle_platform_effect(&platform_request(audio(
                RuntimeAudioCommand::SetMasterVolume {
                    generation: chunk_start as u64,
                    volume_pct: (chunk_start % 101) as f32,
                },
            )))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        let mut received = 0;
        while received < CHUNK_SIZE {
            assert!(std::time::Instant::now() < deadline);
            match rx.try_recv() {
                Ok(EngineEvent::NoteOn {
                    instrument_slot,
                    note,
                    velocity,
                    duration_ms,
                }) => {
                    let index = chunk_start + received;
                    assert_eq!(instrument_slot, (index % 8) as u8);
                    assert_eq!(note, (index % 128) as u8);
                    assert_eq!(velocity, 1 + (index % 127) as u8);
                    assert_eq!(duration_ms, 10 + index as u32);
                    received += 1;
                }
                Ok(EngineEvent::SetMasterVolume { .. }) => {}
                Ok(_) => panic!("unexpected non-musical event during musical drain"),
                Err(_) => std::thread::yield_now(),
            }
        }
    }
}

#[test]
fn desktop_maps_all_runtime_audio_command_shapes() {
    let (mut adapter, mut rx) = test_adapter();
    let params = BTreeMap::new();
    let latest_commands = [
        RuntimeAudioCommand::SetDspConfig {
            generation: 0,
            config: realtime_engine::synth::DspRuntimeConfig::default(),
        },
        RuntimeAudioCommand::SetMasterVolume {
            generation: 0,
            volume_pct: 80.0,
        },
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 0,
            generation: 0,
            volume_pct: Some(80.0),
            pan_pos: Some(16),
        },
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index: 0,
            generation: 0,
            pan_pos: Some(16),
            volume_pct: Some(80.0),
        },
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            generation: 0,
            path: "synth.filter.cutoffHz".into(),
            value: 440.0,
        },
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: 0,
            generation: 0,
            path: "sample.tuneSemis".into(),
            value: 0.0,
        },
        RuntimeAudioCommand::SetFxBusParam {
            bus_index: 0,
            slot_index: 0,
            generation: 0,
            param: realtime_engine::synth::FxParamId::MixPct,
            value: 20.0,
        },
        RuntimeAudioCommand::SetGlobalFxParam {
            slot_index: 0,
            generation: 0,
            param: realtime_engine::synth::FxParamId::MixPct,
            value: 20.0,
        },
        RuntimeAudioCommand::MomentaryFxUpdate {
            id: "fx".into(),
            epoch: 1,
            params,
        },
    ];
    adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::MomentaryFxStart {
                id: "fx".into(),
                epoch: 1,
                fx_type: "freeze".into(),
                params: BTreeMap::new(),
                target: playback_runtime::RuntimeMomentaryFxTarget::Global,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::PreparedMomentaryFxStart { .. }
    ));
    for command in latest_commands {
        adapter
            .handle_platform_effect(&platform_request(audio(command)))
            .unwrap();
    }
    adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::SamplePreview {
                instrument_slot: 0,
                sample_slot: 0,
                path: "../missing.wav".into(),
                velocity: 96,
            },
        )))
        .unwrap();
    adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "fx".into(),
                epoch: 1,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::MomentaryFxStop { epoch: 1 }
    ));
    assert!(rx.try_recv().is_err());
}

#[test]
fn desktop_maps_prepared_full_and_owner_commands() {
    let (mut adapter, mut rx) = test_adapter();
    let commands = [
        RuntimeAudioCommand::SetAudioConfig {
            revision: 1,
            request_id: Some("audio-1".into()),
            generation: 1,
            config: serde_json::json!({
                "masterVolume": 82,
                "panPositions": 33,
                "instruments": [{ "type": "synth" }],
                "mixer": { "buses": [], "master": { "slots": [] } }
            }),
        },
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            generation: 2,
            config: serde_json::json!({ "type": "synth" }),
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 0,
            slot_index: 0,
            generation: 2,
            fx_type: "delay".into(),
            params: BTreeMap::new(),
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 0,
            generation: 2,
            fx_type: "eq".into(),
            params: BTreeMap::new(),
        },
    ];
    for command in commands {
        adapter
            .handle_platform_effect(&platform_request(audio(command)))
            .unwrap();
    }
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::SetPreparedAudioConfig { generation: 1, .. }
    ));
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 2,
            sample_bank: None,
            ..
        }
    ));
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::SetPreparedFxBusSlot { generation: 2, .. }
    ));
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::SetPreparedGlobalFxSlot { generation: 2, .. }
    ));
}

#[test]
fn desktop_maps_full_audio_config_to_prepared_generation() {
    let (mut adapter, mut rx) = test_adapter();
    adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::SetAudioConfig {
                revision: 1,
                request_id: Some("audio-1".into()),
                generation: 1,
                config: serde_json::json!({
                    "masterVolume": 82,
                    "panPositions": 33,
                    "instruments": [{ "type": "synth" }],
                    "mixer": { "buses": [], "master": { "slots": [] } }
                }),
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::SetPreparedAudioConfig { generation: 1, .. }
    ));
}

#[test]
fn desktop_rejects_invalid_paths_nonfinite_values_and_fx_types() {
    let (mut adapter, _) = test_adapter();
    let invalid = [
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            generation: 0,
            path: "synth.nope".into(),
            value: 1.0,
        },
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: 0,
            generation: 0,
            path: "sample.nope".into(),
            value: 1.0,
        },
        RuntimeAudioCommand::SetMasterVolume {
            generation: 0,
            volume_pct: f32::NAN,
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 0,
            generation: 0,
            fx_type: "unknown".into(),
            params: BTreeMap::new(),
        },
    ];
    for command in invalid {
        let error = adapter
            .handle_platform_effect(&platform_request(audio(command)))
            .unwrap_err();
        assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
        assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio);
    }
}

#[test]
fn musical_and_structural_overload_are_typed_and_panic_without_teardown() {
    let (mut adapter, mut rx) = test_adapter();
    for _ in 0..512_usize {
        adapter
            .audio
            .engine_tx
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: 60,
                velocity: 100,
                duration_ms: 1_000,
            })
            .unwrap();
    }
    let musical_error = adapter
        .handle_musical_event(&MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        })
        .unwrap_err();
    assert_eq!(musical_error.facts.domain, RuntimeErrorDomain::Audio);
    assert_eq!(
        musical_error.facts.operation,
        RuntimeOperation::MusicalEvent
    );
    assert!(matches!(next_event(&mut rx), EngineEvent::AllNotesOff));

    let (mut adapter, mut rx) = test_adapter();
    adapter
        .momentary_fx_types
        .insert("fx".into(), (99, "freeze".into()));
    for epoch in 0..64_u64 {
        adapter
            .audio
            .engine_tx
            .send(EngineEvent::MomentaryFxStop { epoch })
            .unwrap();
    }
    let structural_error = adapter
        .handle_platform_effect(&platform_request(audio(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "fx".into(),
                epoch: 99,
            },
        )))
        .unwrap_err();
    assert_eq!(structural_error.facts.domain, RuntimeErrorDomain::Audio);
    assert!(matches!(next_event(&mut rx), EngineEvent::AllNotesOff));
}
