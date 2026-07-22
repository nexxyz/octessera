use crate::audio_config::{
    decode_sample_file, normalize_config, parse_instrument_slot_config, sample_bank_for_slot_config,
};
use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::samples::resolve_sample_file;
use crate::types::{MomentaryFxTargetPayload, QueuedAudioEvent};
use playback_runtime::{RuntimeAdapterError, RuntimeAudioCommand, RuntimeMomentaryFxTarget};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use realtime_engine::synth::{
    validate_fx_type, validate_momentary_fx_type, validate_sample_bank_param_path,
    validate_synth_param_path,
};

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        if let RuntimeAudioCommand::SetAudioConfig {
            revision,
            request_id,
            config,
        } = command
        {
            normalize_config(config).map_err(invalid_audio_command)?;
            return self
                .handle_full_audio_config(*revision, request_id.clone(), config.clone())
                .map_err(RuntimeAdapterError::from);
        }
        let event = match command {
            RuntimeAudioCommand::SetAudioConfig { .. } => unreachable!(),
            RuntimeAudioCommand::SetMasterVolume { volume_pct } => {
                QueuedAudioEvent::SetMasterVolume {
                    volume_pct: *volume_pct,
                }
            }
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                volume_pct,
                pan_pos,
            } => QueuedAudioEvent::SetInstrumentMixer {
                instrument_slot: *instrument_slot,
                volume_pct: *volume_pct,
                pan_pos: *pan_pos,
            },
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot,
                config,
            } => {
                let event = QueuedAudioEvent::SetInstrumentSlot {
                    instrument_slot: *instrument_slot,
                    config: parse_instrument_slot_config(config).map_err(invalid_audio_command)?,
                };
                self.audio.audio_control.enqueue_dynamic(event)?;
                if let Some(bank) = self.sample_bank_event(config)? {
                    self.audio
                        .audio_control
                        .enqueue_dynamic(QueuedAudioEvent::SetSampleBank {
                            instrument_slot: *instrument_slot,
                            bank,
                        })?;
                }
                return Ok(());
            }
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                pan_pos,
                volume_pct,
            } => QueuedAudioEvent::SetFxBusMixer {
                bus_index: *bus_index,
                pan_pos: *pan_pos,
                volume_pct: *volume_pct,
            },
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot,
                path,
                value,
            } => {
                validate_synth_param_path(path).map_err(invalid_audio_command)?;
                QueuedAudioEvent::SetSynthParam {
                    instrument_slot: *instrument_slot,
                    path: path.clone(),
                    value: *value,
                }
            }
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot,
                path,
                value,
            } => {
                validate_sample_bank_param_path(path).map_err(invalid_audio_command)?;
                QueuedAudioEvent::SetSampleBankParam {
                    instrument_slot: *instrument_slot,
                    path: path.clone(),
                    value: *value,
                }
            }
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                fx_type,
                params,
            } => {
                validate_fx_type(fx_type).map_err(invalid_audio_command)?;
                QueuedAudioEvent::SetFxBusSlot {
                    bus_index: *bus_index,
                    slot_index: *slot_index,
                    fx_type: fx_type.clone(),
                    params: params.clone(),
                }
            }
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                fx_type,
                params,
            } => {
                validate_fx_type(fx_type).map_err(invalid_audio_command)?;
                QueuedAudioEvent::SetGlobalFxSlot {
                    slot_index: *slot_index,
                    fx_type: fx_type.clone(),
                    params: params.clone(),
                }
            }
            RuntimeAudioCommand::MomentaryFxStart {
                id,
                fx_type,
                params,
                target,
            } => {
                validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
                QueuedAudioEvent::MomentaryFxStart {
                    id: id.clone(),
                    fx_type: fx_type.clone(),
                    params: params.clone(),
                    target: momentary_fx_target_payload(target),
                }
            }
            RuntimeAudioCommand::MomentaryFxUpdate { id, params } => {
                QueuedAudioEvent::MomentaryFxUpdate {
                    id: id.clone(),
                    params: params.clone(),
                }
            }
            RuntimeAudioCommand::MomentaryFxStop { id } => {
                QueuedAudioEvent::MomentaryFxStop { id: id.clone() }
            }
            RuntimeAudioCommand::SamplePreview {
                instrument_slot,
                path,
                velocity,
                ..
            } => self.sample_preview_event(*instrument_slot, path, *velocity)?,
        };
        self.audio
            .audio_control
            .enqueue_dynamic(event)
            .map_err(Into::into)
    }

    fn sample_preview_event(
        &self,
        instrument_slot: usize,
        path: &str,
        velocity: u8,
    ) -> Result<QueuedAudioEvent, RuntimeAdapterError> {
        let full_path = resolve_sample_file(path)
            .ok_or_else(|| RuntimeAdapterError::from("invalid sample path"))?;
        let buffer = self
            .load_sample(&full_path)
            .ok_or_else(|| RuntimeAdapterError::from("sample decode failed"))?;
        Ok(QueuedAudioEvent::PreviewSample {
            instrument_slot: instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1) as u8,
            buffer,
            velocity,
        })
    }

    fn sample_bank_event(
        &self,
        config: &serde_json::Value,
    ) -> Result<Option<realtime_engine::synth::SampleBankConfig>, RuntimeAdapterError> {
        sample_bank_for_slot_config(config, resolve_sample_file, |path| self.load_sample(path))
            .map_err(invalid_audio_command)
    }

    fn load_sample(&self, path: &str) -> Option<realtime_engine::synth::SampleBuffer> {
        if let Ok(cache) = self.audio.sample_cache.lock() {
            if let Some(buffer) = cache.get(path) {
                return Some(buffer.clone());
            }
        } else {
            return None;
        }
        let buffer = decode_sample_file(path)?;
        if let Ok(mut cache) = self.audio.sample_cache.lock() {
            cache.insert(path.to_string(), buffer.clone());
        }
        Some(buffer)
    }
}

fn invalid_audio_command(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(playback_runtime::RuntimeErrorFacts::new(
        playback_runtime::RuntimeErrorDomain::Audio,
        playback_runtime::RuntimeErrorCode::InvalidPayload,
        playback_runtime::RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

fn momentary_fx_target_payload(target: &RuntimeMomentaryFxTarget) -> MomentaryFxTargetPayload {
    match target {
        RuntimeMomentaryFxTarget::Global => MomentaryFxTargetPayload::Global,
        RuntimeMomentaryFxTarget::FxBus { index } => {
            MomentaryFxTargetPayload::FxBus { index: *index }
        }
        RuntimeMomentaryFxTarget::Instrument { index } => {
            MomentaryFxTargetPayload::Instrument { index: *index }
        }
    }
}
