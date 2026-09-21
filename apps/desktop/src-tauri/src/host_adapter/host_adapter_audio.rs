use crate::audio_config::{normalize_config, parse_instrument_slot_config};
use crate::audio_prep_service::AudioPrepEnqueueResult;
use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeMomentaryFxTarget, RuntimeOperation,
};
use realtime_engine::synth::{
    prepare_momentary_fx_start_with_epoch, prepare_momentary_fx_update, validate_fx_type,
    validate_momentary_fx_type, MomentaryFxTarget, SampleBankParamId, SynthParamId, BUS_COUNT,
    BUS_SLOTS_PER_BUS, DEFAULT_AUDIO_SAMPLE_RATE, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use rodio_engine_source::EngineEvent;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn audio_queue_error(
    error: rodio_engine_source::QueueSendError,
    operation: RuntimeOperation,
) -> RuntimeAdapterError {
    let code = match error {
        rodio_engine_source::QueueSendError::Full { .. } => RuntimeErrorCode::OperationFailed,
        rodio_engine_source::QueueSendError::Disconnected { .. } => {
            RuntimeErrorCode::AudioThreadFailed
        }
    };
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        code,
        operation,
        Some(error.to_string()),
    ))
}

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        match command {
            RuntimeAudioCommand::SetAudioConfig {
                revision,
                request_id,
                generation,
                config,
            } => {
                normalize_config(config).map_err(invalid_audio_command)?;
                prep_result(
                    self.audio.audio_control.enqueue_full_config(
                        *revision,
                        *generation,
                        request_id.clone(),
                        config.clone(),
                    ),
                    "full audio configuration",
                )
            }
            RuntimeAudioCommand::SetDspConfig { generation, config } => self.send_engine_event(
                EngineEvent::SetDspConfig {
                    generation: *generation,
                    config: *config,
                },
                playback_runtime::RuntimeOperation::AudioCommand,
            ),
            RuntimeAudioCommand::SetMasterVolume {
                generation,
                volume_pct,
            } => {
                ensure_finite(*volume_pct, "master volume")?;
                self.send_engine_event(
                    EngineEvent::SetMasterVolume {
                        generation: *generation,
                        volume_pct: *volume_pct,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                generation,
                volume_pct,
                pan_pos,
            } => {
                validate_instrument_slot(*instrument_slot)?;
                ensure_optional_finite(*volume_pct, "instrument volume")?;
                self.send_engine_event(
                    EngineEvent::SetInstrumentMixer {
                        instrument_slot: *instrument_slot as u8,
                        generation: *generation,
                        volume_pct: *volume_pct,
                        pan_pos: *pan_pos,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot,
                generation,
                config,
            } => {
                validate_instrument_slot(*instrument_slot)?;
                parse_instrument_slot_config(config).map_err(invalid_audio_command)?;
                prep_result(
                    self.audio.audio_control.enqueue_instrument_slot(
                        *instrument_slot,
                        *generation,
                        config.clone(),
                    ),
                    "instrument configuration",
                )
            }
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                generation,
                pan_pos,
                volume_pct,
            } => {
                validate_bus_index(*bus_index)?;
                ensure_optional_finite(*volume_pct, "FX bus volume")?;
                self.send_engine_event(
                    EngineEvent::SetFxBusMixer {
                        bus_index: *bus_index as u8,
                        generation: *generation,
                        pan_pos: *pan_pos,
                        volume_pct: *volume_pct,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot,
                generation,
                path,
                value,
            } => {
                validate_instrument_slot(*instrument_slot)?;
                let param = SynthParamId::from_path(path).ok_or_else(|| {
                    invalid_audio_command(format!("unsupported synth parameter path `{path}`"))
                })?;
                ensure_finite(*value, "synth parameter")?;
                self.send_engine_event(
                    EngineEvent::SetSynthParam {
                        instrument_slot: *instrument_slot as u8,
                        generation: *generation,
                        param,
                        value: *value,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot,
                generation,
                path,
                value,
            } => {
                validate_instrument_slot(*instrument_slot)?;
                let param = SampleBankParamId::from_path(path).ok_or_else(|| {
                    invalid_audio_command(format!("unsupported sample parameter path `{path}`"))
                })?;
                ensure_finite(*value, "sample parameter")?;
                self.send_engine_event(
                    EngineEvent::SetSampleBankParam {
                        instrument_slot: *instrument_slot as u8,
                        generation: *generation,
                        param,
                        value: *value,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::SetFxBusParam {
                bus_index,
                slot_index,
                generation,
                param,
                value,
            } => {
                validate_bus_index(*bus_index)?;
                validate_bus_slot(*slot_index)?;
                ensure_finite(*value, "FX bus parameter")?;
                self.send_engine_event(
                    EngineEvent::SetFxBusParam {
                        bus_index: *bus_index as u8,
                        slot_index: *slot_index as u8,
                        generation: *generation,
                        param: *param,
                        value: *value,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                generation,
                fx_type,
                params,
            } => {
                validate_bus_index(*bus_index)?;
                validate_bus_slot(*slot_index)?;
                validate_fx_type(fx_type).map_err(invalid_audio_command)?;
                validate_param_values(params)?;
                prep_result(
                    self.audio.audio_control.enqueue_fx_bus_slot(
                        *bus_index,
                        *slot_index,
                        *generation,
                        fx_type.clone(),
                        params.clone(),
                    ),
                    "FX bus configuration",
                )
            }
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                generation,
                fx_type,
                params,
            } => {
                validate_global_slot(*slot_index)?;
                validate_fx_type(fx_type).map_err(invalid_audio_command)?;
                validate_param_values(params)?;
                prep_result(
                    self.audio.audio_control.enqueue_global_fx_slot(
                        *slot_index,
                        *generation,
                        fx_type.clone(),
                        params.clone(),
                    ),
                    "global FX configuration",
                )
            }
            RuntimeAudioCommand::SetGlobalFxParam {
                slot_index,
                generation,
                param,
                value,
            } => {
                validate_global_slot(*slot_index)?;
                ensure_finite(*value, "global FX parameter")?;
                self.send_engine_event(
                    EngineEvent::SetGlobalFxParam {
                        slot_index: *slot_index as u8,
                        generation: *generation,
                        param: *param,
                        value: *value,
                    },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::MomentaryFxStart {
                id,
                epoch,
                fx_type,
                params,
                target,
            } => {
                validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
                validate_momentary_target(target)?;
                let prepared = prepare_momentary_fx_start_with_epoch(
                    id.clone(),
                    *epoch,
                    fx_type.clone(),
                    params.clone(),
                    momentary_target(target),
                    DEFAULT_AUDIO_SAMPLE_RATE,
                )
                .ok_or_else(|| invalid_audio_command("invalid momentary FX parameters".into()))?;
                self.send_engine_event(
                    EngineEvent::PreparedMomentaryFxStart { config: prepared },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )?;
                self.momentary_fx_types
                    .insert(id.clone(), (*epoch, fx_type.clone()));
                Ok(())
            }
            RuntimeAudioCommand::MomentaryFxUpdate { id, epoch, params } => {
                let Some((active_epoch, fx_type)) = self.momentary_fx_types.get(id) else {
                    return Err(invalid_audio_command(format!(
                        "momentary FX is not active: {id}"
                    )));
                };
                if *active_epoch != *epoch {
                    return Err(invalid_audio_command(format!(
                        "momentary FX epoch is stale for {id}"
                    )));
                }
                validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
                let prepared = prepare_momentary_fx_update(
                    *epoch,
                    fx_type.clone(),
                    params.clone(),
                    DEFAULT_AUDIO_SAMPLE_RATE,
                )
                .ok_or_else(|| invalid_audio_command("invalid momentary FX parameters".into()))?;
                self.send_engine_event(
                    EngineEvent::MomentaryFxUpdate(prepared),
                    playback_runtime::RuntimeOperation::AudioCommand,
                )
            }
            RuntimeAudioCommand::MomentaryFxStop { id, epoch } => {
                let Some((active_epoch, _)) = self.momentary_fx_types.get(id) else {
                    return Err(invalid_audio_command(format!(
                        "momentary FX is not active: {id}"
                    )));
                };
                if *active_epoch != *epoch {
                    return Err(invalid_audio_command(format!(
                        "momentary FX epoch is stale for {id}"
                    )));
                }
                self.send_engine_event(
                    EngineEvent::MomentaryFxStop { epoch: *epoch },
                    playback_runtime::RuntimeOperation::AudioCommand,
                )?;
                self.momentary_fx_types.remove(id);
                Ok(())
            }
            RuntimeAudioCommand::SamplePreview {
                instrument_slot,
                sample_slot,
                path,
                velocity,
            } => prep_result(
                {
                    validate_instrument_slot(*instrument_slot)?;
                    if *sample_slot >= SAMPLE_SLOTS_PER_INSTRUMENT {
                        return Err(invalid_audio_command(format!(
                            "invalid sample slot {sample_slot}"
                        )));
                    }
                    self.audio.audio_control.enqueue_sample_preview(
                        *instrument_slot,
                        path.clone(),
                        *velocity,
                    )
                },
                "sample preview",
            ),
        }
    }
}

fn prep_result(result: AudioPrepEnqueueResult, operation: &str) -> Result<(), RuntimeAdapterError> {
    match result {
        AudioPrepEnqueueResult::Accepted => Ok(()),
        AudioPrepEnqueueResult::Full => Err(prep_queue_error(
            playback_runtime::RuntimeErrorCode::OperationFailed,
            format!("{operation} preparation queue is full"),
        )),
        AudioPrepEnqueueResult::Disconnected => Err(prep_queue_error(
            playback_runtime::RuntimeErrorCode::AudioThreadFailed,
            format!("{operation} preparation queue disconnected"),
        )),
    }
}

fn prep_queue_error(
    code: playback_runtime::RuntimeErrorCode,
    message: String,
) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(playback_runtime::RuntimeErrorFacts::new(
        playback_runtime::RuntimeErrorDomain::Audio,
        code,
        playback_runtime::RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

fn invalid_audio_command(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(playback_runtime::RuntimeErrorFacts::new(
        playback_runtime::RuntimeErrorDomain::Audio,
        playback_runtime::RuntimeErrorCode::InvalidPayload,
        playback_runtime::RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

fn ensure_finite(value: f32, name: &str) -> Result<(), RuntimeAdapterError> {
    value
        .is_finite()
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("{name} must be finite")))
}

fn ensure_optional_finite(value: Option<f32>, name: &str) -> Result<(), RuntimeAdapterError> {
    value.map_or(Ok(()), |value| ensure_finite(value, name))
}

fn validate_param_values(params: &BTreeMap<String, Value>) -> Result<(), RuntimeAdapterError> {
    params.iter().try_for_each(|(key, value)| {
        let finite = value
            .as_f64()
            .map(|value| (value as f32).is_finite())
            .unwrap_or(false);
        finite
            .then_some(())
            .ok_or_else(|| invalid_audio_command(format!("FX parameter `{key}` must be finite")))
    })
}

fn validate_instrument_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < INSTRUMENT_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid instrument slot {index}")))
}

fn validate_bus_index(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < BUS_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid FX bus {index}")))
}

fn validate_bus_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < BUS_SLOTS_PER_BUS)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid FX bus slot {index}")))
}

fn validate_global_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < GLOBAL_FX_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid global FX slot {index}")))
}

fn validate_momentary_target(target: &RuntimeMomentaryFxTarget) -> Result<(), RuntimeAdapterError> {
    match target {
        RuntimeMomentaryFxTarget::Global => Ok(()),
        RuntimeMomentaryFxTarget::FxBus { index } => validate_bus_index(*index),
        RuntimeMomentaryFxTarget::Instrument { index } => validate_instrument_slot(*index),
    }
}

fn momentary_target(target: &RuntimeMomentaryFxTarget) -> MomentaryFxTarget {
    match target {
        RuntimeMomentaryFxTarget::Global => MomentaryFxTarget::Global,
        RuntimeMomentaryFxTarget::FxBus { index } => MomentaryFxTarget::FxBus { index: *index },
        RuntimeMomentaryFxTarget::Instrument { index } => {
            MomentaryFxTarget::Instrument { index: *index }
        }
    }
}
