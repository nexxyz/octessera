use super::*;
use crate::RuntimeConfig;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn recording_protocol_json_uses_public_field_names() {
    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::RecordingStartAudio { max_minutes: 5 })
            .unwrap(),
        json!({ "type": "recording_start_audio", "maxMinutes": 5 })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::UsbSdTransferStart).unwrap(),
        json!({ "type": "usb_sd_transfer_start" })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::UsbSdTransferStop).unwrap(),
        json!({ "type": "usb_sd_transfer_stop" })
    );

    assert_eq!(
        serde_json::to_value(RuntimeStoreResult::UsbSdTransferStatus {
            active: true,
            message: "USB SD2 transfer active".into(),
        })
        .unwrap(),
        json!({ "type": "usb_sd_transfer_status", "active": true, "message": "USB SD2 transfer active" })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::RecordingStop).unwrap(),
        json!({ "type": "recording_stop" })
    );
}

#[test]
fn system_info_protocol_is_typed_and_identity_safe() {
    let info = RuntimeSystemInfo {
        os: "linux".into(),
        os_version: "6.6".into(),
        octessera_version: "0.7.0".into(),
        primary_ip: Some("192.168.1.5".into()),
        primary_mac: Some("aa:bb:cc:dd:ee:ff".into()),
        hostname: "octessera".into(),
        board_profile: "raspberry-pi-zero-2w".into(),
    };
    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::SystemInfoRequest).unwrap(),
        json!({ "type": "system_info_request" })
    );
    assert_eq!(
        serde_json::to_value(RuntimeStoreResult::SystemInfoResult { info }).unwrap(),
        json!({
            "type": "system_info_result",
            "info": {
                "os": "linux",
                "osVersion": "6.6",
                "octesseraVersion": "0.7.0",
                "primaryIp": "192.168.1.5",
                "primaryMac": "aa:bb:cc:dd:ee:ff",
                "hostname": "octessera",
                "boardProfile": "raspberry-pi-zero-2w"
            }
        })
    );
    let error = RuntimeStoreResult::SystemInfoError {
        error: RuntimeSystemInfoError::unavailable("not connected"),
    }
    .with_identity("platform-4".into(), Some(2));
    assert_eq!(
        error.error_facts().unwrap().request_id.as_deref(),
        Some("platform-4")
    );
    assert_eq!(error.error_facts().unwrap().revision, Some(2));
}

#[test]
fn device_update_protocol_is_dedicated_and_backwards_compatible() {
    let effect = RuntimePlatformEffect::UpdateApply;
    assert_eq!(effect.operation(), RuntimeOperation::DeviceUpdate);
    let result = RuntimeStoreResult::DeviceUpdateStatus {
        ok: false,
        message: "helper output".into(),
    };
    assert_eq!(result.operation(), RuntimeOperation::DeviceUpdate);
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        json!({
            "type": "device_update_status",
            "ok": false,
            "message": "helper output"
        })
    );
    assert_eq!(
        serde_json::from_value::<RuntimeStoreResult>(json!({
            "type": "device_update_status"
        }))
        .unwrap(),
        RuntimeStoreResult::DeviceUpdateStatus {
            ok: false,
            message: String::new(),
        }
    );
    assert_eq!(
        serde_json::from_value::<RuntimeStoreResult>(json!({
            "type": "operation_succeeded",
            "operation": "runtime_dispatch"
        }))
        .unwrap()
        .operation(),
        RuntimeOperation::RuntimeDispatch
    );
}

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
                error: None,
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
        serde_json::to_value(RunnerMessage::RuntimeConfigChanged {
            config: RuntimeConfig {
                bpm: 93.5,
                sync_source: SyncSource::External,
                midi_clock_out_enabled: true,
                midi_out_enabled: true,
            },
        })
        .unwrap(),
        json!({
            "type": "runtime_config_changed",
            "config": {
                "bpm": 93.5,
                "syncSource": "external",
                "midiClockOutEnabled": true,
                "midiOutEnabled": true,
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
        id: "sparks".into(),
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: RuntimeMomentaryFxTarget::FxBus { index: 3 },
    };
    assert_eq!(
        serde_json::to_value(&fx_command).unwrap(),
        json!({
            "type": "momentary_fx_start",
            "id": "sparks",
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
        serde_json::to_value(RuntimePlatformEffect::UsbApplyReboot {
            payload: json!({ "runtimeConfig": { "usb": { "audioOut": "jack", "midiOutEnabled": false } } }),
        })
        .unwrap(),
        json!({
            "type": "usb_apply_reboot",
            "payload": { "runtimeConfig": { "usb": { "audioOut": "jack", "midiOutEnabled": false } } }
        })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::RecordingStartAudio { max_minutes: 5 })
            .unwrap(),
        json!({ "type": "recording_start_audio", "maxMinutes": 5 })
    );

    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::RecordingStop).unwrap(),
        json!({ "type": "recording_stop" })
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
                "id": "sparks",
                "fxType": "stutter",
                "params": {},
                "target": { "type": "fx_bus", "index": 3 },
            },
        })
    );
}

#[test]
fn every_runtime_audio_command_round_trips_through_json() {
    let params = BTreeMap::from([(String::from("mixPct"), json!(35))]);
    let commands = vec![
        RuntimeAudioCommand::SetAudioConfig {
            revision: 4,
            request_id: Some("audio-4".into()),
            config: json!({ "instruments": [] }),
        },
        RuntimeAudioCommand::SetMasterVolume { volume_pct: 82.0 },
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: 1,
            volume_pct: Some(74.0),
            pan_pos: Some(16),
        },
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 2,
            config: json!({ "type": "synth" }),
        },
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index: 2,
            pan_pos: Some(12),
            volume_pct: Some(66.0),
        },
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 3,
            path: "synth.filter.cutoffHz".into(),
            value: 440.0,
        },
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: 4,
            path: "sample.tuneSemis".into(),
            value: 2.0,
        },
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 1,
            slot_index: 0,
            fx_type: "delay".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index: 1,
            fx_type: "compressor".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::MomentaryFxStart {
            id: "spark:0".into(),
            fx_type: "freeze".into(),
            params: params.clone(),
            target: RuntimeMomentaryFxTarget::Global,
        },
        RuntimeAudioCommand::MomentaryFxUpdate {
            id: "spark:0".into(),
            params: params.clone(),
        },
        RuntimeAudioCommand::MomentaryFxStop {
            id: "spark:0".into(),
        },
        RuntimeAudioCommand::SamplePreview {
            instrument_slot: 5,
            sample_slot: 2,
            path: "kits/hat.wav".into(),
            velocity: 96,
        },
    ];

    for command in commands {
        let encoded = serde_json::to_value(&command).unwrap();
        assert_eq!(
            serde_json::from_value::<RuntimeAudioCommand>(encoded).unwrap(),
            command
        );
    }
}

#[test]
fn runtime_error_metadata_json_uses_stable_typed_fields() {
    let status = RuntimeStatus {
        state: RuntimeStatusState::Error,
        transport: RuntimeTransportState::Playing,
        current_ppqn_pulse: 24,
        pending_resync: false,
        sync_source: SyncSource::Internal,
        message: Some("disk full".into()),
        error: Some(RuntimeErrorMetadata {
            domain: RuntimeErrorDomain::Storage,
            code: RuntimeErrorCode::OperationFailed,
            operation: RuntimeOperation::StoreSaveDefault,
            recovery: RuntimeRecovery::RetainLastGood,
            request_id: Some("req-7".into()),
            revision: Some(3),
            message: Some("disk full".into()),
        }),
    };

    assert_eq!(
        serde_json::to_value(status).unwrap(),
        json!({
            "state": "error",
            "transport": "playing",
            "currentPpqnPulse": 24,
            "pendingResync": false,
            "syncSource": "internal",
            "message": "disk full",
            "error": {
                "domain": "storage",
                "code": "operation_failed",
                "operation": "store_save_default",
                "recovery": "retain_last_good",
                "requestId": "req-7",
                "revision": 3,
                "message": "disk full"
            }
        })
    );

    let legacy_status = serde_json::from_value::<RuntimeStatus>(json!({
        "state": "running",
        "transport": "playing",
        "currentPpqnPulse": 24,
        "pendingResync": false,
        "syncSource": "internal",
        "message": null
    }))
    .unwrap();
    assert_eq!(legacy_status.error, None);
}
