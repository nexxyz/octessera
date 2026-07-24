use crate::audio::AudioService;
use crate::audio_config_parse::parse_instrument_slot_config;
use playback_runtime::{
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeMomentaryFxTarget, RuntimeOperation,
};
use realtime_engine::synth::{
    normalize_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, prepare_momentary_fx_start, validate_fx_type,
    validate_momentary_fx_type, validate_sample_bank_param_path, validate_synth_param_path,
    MomentaryFxTarget, DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::EngineEvent;
use std::path::Path;
use std::sync::atomic::Ordering;

pub fn send_audio_command(
    audio: Option<AudioService>,
    command: &RuntimeAudioCommand,
    samples_dir: &Path,
) -> Result<(), RuntimeAdapterError> {
    validate_audio_command(command, samples_dir)?;
    let Some(audio) = audio else {
        return Ok(());
    };
    match command {
        RuntimeAudioCommand::SetAudioConfig {
            revision,
            request_id,
            config,
        } => {
            audio.config_revision.fetch_max(*revision, Ordering::SeqCst);
            audio.enqueue_full_config(
                *revision,
                request_id.clone(),
                config.clone(),
                samples_dir.to_path_buf(),
            )?;
            Ok(())
        }
        RuntimeAudioCommand::SetMasterVolume { volume_pct } => {
            audio.send(EngineEvent::SetMasterVolume {
                volume_pct: *volume_pct,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            volume_pct,
            pan_pos,
        } => {
            audio.send(EngineEvent::SetInstrumentMixer {
                instrument_slot: *instrument_slot,
                volume_pct: *volume_pct,
                pan_pos: *pan_pos,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot,
            config,
        } => {
            let config = parse_instrument_slot_config(config).map_err(invalid_audio_command)?;
            audio.send(EngineEvent::SetPreparedInstrumentSlot {
                instrument_slot: *instrument_slot,
                config: prepare_instrument_slot_config(config),
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            pan_pos,
            volume_pct,
        } => {
            audio.send(EngineEvent::SetFxBusMixer {
                bus_index: *bus_index,
                pan_pos: *pan_pos,
                volume_pct: *volume_pct,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot,
            path,
            value,
        } => {
            validate_synth_param_path(path).map_err(invalid_audio_command)?;
            audio.send(EngineEvent::SetSynthParam {
                instrument_slot: *instrument_slot,
                path: path.clone(),
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot,
            path,
            value,
        } => {
            validate_sample_bank_param_path(path).map_err(invalid_audio_command)?;
            audio.send(EngineEvent::SetSampleBankParam {
                instrument_slot: *instrument_slot,
                path: path.clone(),
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
        } => {
            validate_fx_type(fx_type).map_err(invalid_audio_command)?;
            audio.send(EngineEvent::SetPreparedFxBusSlot {
                bus_index: *bus_index,
                slot_index: *slot_index,
                config: prepare_fx_bus_slot(
                    fx_type.clone(),
                    params.clone(),
                    DEFAULT_AUDIO_SAMPLE_RATE,
                ),
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
        } => {
            validate_fx_type(fx_type).map_err(invalid_audio_command)?;
            audio.send(EngineEvent::SetPreparedGlobalFxSlot {
                slot_index: *slot_index,
                config: prepare_global_fx_slot(fx_type.clone(), params.clone()),
            })?;
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        } => {
            validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
            let prepared = prepare_momentary_fx_start(
                id.clone(),
                fx_type.clone(),
                params.clone(),
                momentary_fx_target(target),
                DEFAULT_AUDIO_SAMPLE_RATE,
            )
            .ok_or_else(|| invalid_audio_command("invalid momentary FX type".into()))?;
            audio.send(EngineEvent::PreparedMomentaryFxStart(prepared))?;
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxUpdate { id, params } => {
            audio.send(EngineEvent::MomentaryFxUpdate {
                id: id.clone(),
                params: params.clone(),
            })?;
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxStop { id } => {
            audio.send(EngineEvent::MomentaryFxStop { id: id.clone() })?;
            Ok(())
        }
        RuntimeAudioCommand::SamplePreview {
            instrument_slot,
            path,
            velocity,
            ..
        } => {
            audio.enqueue_sample_preview(
                *instrument_slot,
                path.clone(),
                *velocity,
                samples_dir.to_path_buf(),
            )?;
            Ok(())
        }
    }
}

fn validate_audio_command(
    command: &RuntimeAudioCommand,
    _samples_dir: &Path,
) -> Result<(), RuntimeAdapterError> {
    match command {
        RuntimeAudioCommand::SetAudioConfig { config, .. } => {
            normalize_audio_config(config).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::SetInstrumentSlot { config, .. } => {
            parse_instrument_slot_config(config).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::SetSynthParam { path, .. } => {
            validate_synth_param_path(path).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::SetSampleBankParam { path, .. } => {
            validate_sample_bank_param_path(path).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::SetFxBusSlot { fx_type, .. }
        | RuntimeAudioCommand::SetGlobalFxSlot { fx_type, .. } => {
            validate_fx_type(fx_type).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::MomentaryFxStart { fx_type, .. } => {
            validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
        }
        _ => {}
    }
    Ok(())
}

fn momentary_fx_target(target: &RuntimeMomentaryFxTarget) -> MomentaryFxTarget {
    match target {
        RuntimeMomentaryFxTarget::Global => MomentaryFxTarget::Global,
        RuntimeMomentaryFxTarget::FxBus { index } => MomentaryFxTarget::FxBus { index: *index },
        RuntimeMomentaryFxTarget::Instrument { index } => {
            MomentaryFxTarget::Instrument { index: *index }
        }
    }
}

fn invalid_audio_command(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::InvalidPayload,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_config_parse::{decode_sample_file, resolve_sample_path, SampleLoadError};
    use std::collections::BTreeMap;

    #[test]
    fn pi_sample_preview_resolves_decodes_and_queues_preview_event() {
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-preview-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("kick.wav"), wav_bytes()).unwrap();

        let path = resolve_sample_path(&root, "kick.wav").unwrap();
        assert!(!decode_sample_file(&path).unwrap().samples.is_empty());
        assert!(resolve_sample_path(&root, "../kick.wav").is_none());
        let error = SampleLoadError::Unresolved("../kick.wav".into());
        assert_eq!(error.code(), RuntimeErrorCode::NotFound);
        assert_eq!(error.message(), "sample not found: ../kick.wav");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pi_accepts_every_valid_runtime_audio_command_at_the_adapter_boundary() {
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-command-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("kick.wav"), b"placeholder").unwrap();
        let params = BTreeMap::new();
        let commands = vec![
            RuntimeAudioCommand::SetAudioConfig {
                revision: 1,
                request_id: None,
                config: serde_json::json!({ "instruments": [] }),
            },
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
                target: RuntimeMomentaryFxTarget::Global,
            },
            RuntimeAudioCommand::MomentaryFxUpdate {
                id: "fx".into(),
                params: params.clone(),
            },
            RuntimeAudioCommand::MomentaryFxStop { id: "fx".into() },
            RuntimeAudioCommand::SamplePreview {
                instrument_slot: 0,
                sample_slot: 0,
                path: "kick.wav".into(),
                velocity: 96,
            },
        ];

        for command in commands {
            send_audio_command(None, &command, &root).unwrap();
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pi_invalid_fx_command_has_typed_invalid_payload_failure() {
        let error = send_audio_command(
            None,
            &RuntimeAudioCommand::SetFxBusSlot {
                bus_index: 0,
                slot_index: 0,
                fx_type: "unknown".into(),
                params: BTreeMap::new(),
            },
            Path::new("samples"),
        )
        .unwrap_err();
        assert_eq!(error.facts.domain, RuntimeErrorDomain::Audio);
        assert_eq!(error.facts.code, RuntimeErrorCode::InvalidPayload);
        assert_eq!(error.facts.operation, RuntimeOperation::AudioCommand);
    }

    fn wav_bytes() -> Vec<u8> {
        let samples = [0_i16, 1_000_i16];
        let data_len = samples.len() * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36_u32 + data_len as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&44_100_u32.to_le_bytes());
        bytes.extend_from_slice(&88_200_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }
}
