use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn runtime_protocol_json_uses_public_field_names_and_defaults() {
    assert_eq!(
        serde_json::to_value(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: Some(true),
        })
        .unwrap(),
        json!({
            "type": "device_input",
            "input": { "type": "button_s", "pressed": true },
            "requestSnapshot": true,
        })
    );

    assert_eq!(
        serde_json::from_value::<HostMessage>(json!({
            "type": "device_input",
            "input": { "type": "button_s", "pressed": true },
        }))
        .unwrap(),
        HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        }
    );

    assert_eq!(
        serde_json::to_value(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: Some(96),
            request_snapshot: Some(false),
        })
        .unwrap(),
        json!({
            "type": "transport_pulse_step",
            "pulses": 24,
            "source": "internal",
            "atPpqnPulse": 96,
            "requestSnapshot": false,
        })
    );

    assert_eq!(
        serde_json::from_value::<HostMessage>(json!({
            "type": "transport_pulse_step",
            "pulses": 1,
            "source": "external",
        }))
        .unwrap(),
        HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::External,
            at_ppqn_pulse: None,
            request_snapshot: None,
        }
    );

    assert_eq!(
        serde_json::to_value(RunnerMessage::RuntimeStatus {
            status: RuntimeStatus {
                state: RuntimeStatusState::Running,
                transport: RuntimeTransportState::Playing,
                current_ppqn_pulse: 96,
                pending_resync: true,
                sync_source: SyncSource::External,
                message: None,
            },
        })
        .unwrap(),
        json!({
            "type": "runtime_status",
            "status": {
                "state": "running",
                "transport": "playing",
                "currentPpqnPulse": 96,
                "pendingResync": true,
                "syncSource": "external",
                "message": null,
            },
        })
    );

    assert_eq!(
        serde_json::to_value(RuntimeUiPulse::TransportFlash {
            flash: "start".into(),
            duration_ms: 120,
        })
        .unwrap(),
        json!({ "type": "transport_flash", "flash": "start", "durationMs": 120 })
    );

    assert_eq!(
        serde_json::to_value(RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TriggerPulse { duration_ms: 80 },
        })
        .unwrap(),
        json!({
            "type": "ui_pulse",
            "pulse": { "type": "trigger_pulse", "durationMs": 80 },
        })
    );
}

#[test]
fn runtime_effect_json_uses_public_audio_store_and_sample_field_names() {
    assert_eq!(
        serde_json::to_value(RuntimeAudioCommand::SamplePreview {
            instrument_slot: 1,
            sample_slot: 2,
            path: "kick.wav".into(),
            velocity: 100,
        })
        .unwrap(),
        json!({
            "type": "sample_preview",
            "instrumentSlot": 1,
            "sampleSlot": 2,
            "path": "kick.wav",
            "velocity": 100,
        })
    );

    let fx_command = RuntimeAudioCommand::MomentaryFxStart {
        id: "dance".into(),
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: RuntimeMomentaryFxTarget::FxBus { index: 3 },
    };
    assert_eq!(
        serde_json::to_value(&fx_command).unwrap(),
        json!({
            "type": "momentary_fx_start",
            "id": "dance",
            "fxType": "stutter",
            "params": {},
            "target": { "type": "fx_bus", "index": 3 },
        })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::SampleListRequest {
            instrument_slot: 1,
            sample_slot: 2,
            dir: "drums".into(),
        })
        .unwrap(),
        json!({
            "type": "sample_list_request",
            "instrumentSlot": 1,
            "sampleSlot": 2,
            "dir": "drums",
        })
    );

    assert_eq!(
        serde_json::to_value(RuntimeStoreResult::SampleListResult {
            instrument_slot: 1,
            sample_slot: 2,
            dir: "drums".into(),
            entries: vec![SampleEntry {
                name: "Kicks".into(),
                path: "drums/Kicks".into(),
                is_dir: true,
            }],
        })
        .unwrap(),
        json!({
            "type": "sample_list_result",
            "instrumentSlot": 1,
            "sampleSlot": 2,
            "dir": "drums",
            "entries": [{ "name": "Kicks", "path": "drums/Kicks", "isDir": true }],
        })
    );

    assert_eq!(
        serde_json::to_value(RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(false),
        })
        .unwrap(),
        json!({ "type": "save_default_result", "ok": true, "isAuto": false })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::AudioCommand {
            command: fx_command,
        })
        .unwrap(),
        json!({
            "type": "audio_command",
            "command": {
                "type": "momentary_fx_start",
                "id": "dance",
                "fxType": "stutter",
                "params": {},
                "target": { "type": "fx_bus", "index": 3 },
            },
        })
    );
}
