use super::{platform_request, test_adapter};
use crate::types::QueuedAudioEvent;
use playback_runtime::{
    HostAdapter, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain, RuntimePlatformEffect,
};
use std::collections::BTreeMap;
use std::time::Duration;

#[test]
fn platform_effect_audio_command_reaches_audio_queue() {
    let (mut adapter, rx) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStop {
                id: "preview".into(),
            },
        }))
        .unwrap();
    assert!(follow_ups.is_empty());
    assert!(
        matches!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), QueuedAudioEvent::MomentaryFxStop { id } if id == "preview")
    );
}

#[test]
fn desktop_accepts_each_dynamic_runtime_audio_command_and_rejects_unknown_fx() {
    let (mut adapter, rx) = test_adapter();
    let params = BTreeMap::new();
    let commands = vec![
        RuntimeAudioCommand::SetMasterVolume { volume_pct: 80.0 },
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 0,
            volume_pct: Some(80.0),
            pan_pos: Some(16),
        },
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            config: serde_json::json!({ "type": "synth" }),
        },
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index: 0,
            pan_pos: Some(16),
            volume_pct: Some(80.0),
        },
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            path: "synth.filter.cutoffHz".into(),
            value: 440.0,
        },
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: 0,
            path: "sample.tuneSemis".into(),
            value: 0.0,
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 0,
            slot_index: 0,
            fx_type: "delay".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 0,
            fx_type: "eq".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::MomentaryFxStart {
            id: "fx".into(),
            fx_type: "freeze".into(),
            params: params.clone(),
            target: playback_runtime::RuntimeMomentaryFxTarget::Global,
        },
        RuntimeAudioCommand::MomentaryFxUpdate {
            id: "fx".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::MomentaryFxStop { id: "fx".into() },
    ];

    for command in commands {
        adapter
            .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
                command,
            }))
            .unwrap();
    }
    assert!(rx.recv_timeout(Duration::from_secs(1)).is_ok());

    let error = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index: 0,
                fx_type: "unknown".into(),
                params: BTreeMap::new(),
            },
        }))
        .unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio);

    let config_error = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SetAudioConfig {
                revision: 8,
                request_id: Some("audio-8".into()),
                config: serde_json::json!({ "instruments": "invalid" }),
            },
        }))
        .unwrap_err();
    assert_eq!(config_error.facts.code, RuntimeErrorCode::InvalidPayload);

    let preview_error = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SamplePreview {
                instrument_slot: 0,
                sample_slot: 0,
                path: "../missing.wav".into(),
                velocity: 96,
            },
        }))
        .unwrap_err();
    assert_eq!(
        preview_error.facts.message.as_deref(),
        Some("invalid sample path")
    );
}

#[test]
fn full_audio_config_command_sets_instruments_and_sample_banks() {
    let (mut adapter, rx) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SetAudioConfig {
                revision: 7,
                request_id: None,
                config: serde_json::json!({
                    "masterVolume": 82,
                    "voiceStealingMode": "auto-hard",
                    "panPositions": 33,
                    "instruments": [{
                        "type": "synth",
                        "mixer": { "route": "direct", "panPos": 16, "volume": 77 }
                    }],
                    "mixer": { "buses": [], "master": { "slots": [] } }
                }),
            },
        }))
        .unwrap();

    assert!(follow_ups.is_empty());
    assert!(
        matches!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), QueuedAudioEvent::SetAudioConfig { instruments, sample_banks: Some(_), voice_stealing_mode: Some(realtime_engine::synth::VoiceStealingMode::AutoHard), .. } if instruments.master_volume == 82.0)
    );
}

#[test]
fn full_audio_config_command_reuses_sample_bank_signature() {
    let (mut adapter, rx) = test_adapter();
    let command = RuntimePlatformEffect::AudioCommand {
        command: RuntimeAudioCommand::SetAudioConfig {
            revision: 7,
            request_id: None,
            config: serde_json::json!({
                "masterVolume": 82,
                "panPositions": 33,
                "instruments": [{ "type": "synth" }],
                "mixer": { "buses": [], "master": { "slots": [] } }
            }),
        },
    };

    adapter
        .handle_platform_effect(&platform_request(command.clone()))
        .unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetAudioConfig {
            sample_banks: Some(_),
            ..
        }
    ));

    adapter
        .handle_platform_effect(&platform_request(command.clone()))
        .unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetAudioConfig {
            sample_banks: None,
            ..
        }
    ));
    assert!(rx.try_recv().is_err());
}

#[test]
fn sampler_slot_command_enqueues_slot_and_single_sample_bank_update() {
    let (mut adapter, rx) = test_adapter();

    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: 2,
                config: serde_json::json!({
                    "type": "sampler",
                    "sample": {
                        "slots": [{ "path": "missing.wav" }],
                        "tuneSemis": -2.0,
                        "amp": { "gainPct": 55.0, "velocitySensitivityPct": 35.0 }
                    },
                    "mixer": { "route": "direct", "panPos": 16, "volume": 77 }
                }),
            },
        }))
        .unwrap();

    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetInstrumentSlot {
            instrument_slot: 2,
            ..
        }
    ));
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        QueuedAudioEvent::SetSampleBank { instrument_slot: 2, bank }
            if bank.tune_semis == -2.0 && bank.gain_pct == 55.0
    ));
}
