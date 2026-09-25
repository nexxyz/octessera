use crate::audio::AudioService;
#[path = "host_audio_command_validation.rs"]
mod host_audio_command_validation;
use host_audio_command_validation::{
    audio_queue_failure, index_u8, invalid_audio_command, momentary_fx_target,
    validate_audio_command,
};
use playback_runtime::{RuntimeAdapterError, RuntimeAudioCommand};
use realtime_engine::synth::{
    prepare_momentary_fx_start_with_epoch, prepare_momentary_fx_update, validate_momentary_fx_type,
    DrumParamId, FmParamId, PluckParamId, SampleBankParamId, SynthParamId,
    DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::EngineEvent;
use std::path::Path;

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
            generation,
            request_id,
            config,
        } => {
            audio.enqueue_full_config(
                *revision,
                *generation,
                request_id.clone(),
                config.clone(),
                samples_dir.to_path_buf(),
            )?;
            Ok(())
        }
        RuntimeAudioCommand::SetMasterVolume {
            generation,
            volume_pct,
        } => {
            audio.send(EngineEvent::SetMasterVolume {
                generation: *generation,
                volume_pct: *volume_pct,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetDspConfig { generation, config } => {
            audio.send(EngineEvent::SetDspConfig {
                generation: *generation,
                config: *config,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            generation,
            volume_pct,
            pan_pos,
        } => {
            audio.send(EngineEvent::SetInstrumentMixer {
                instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
                generation: *generation,
                volume_pct: *volume_pct,
                pan_pos: *pan_pos,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot,
            generation,
            config,
        } => {
            audio
                .enqueue_instrument_slot(
                    *instrument_slot,
                    *generation,
                    config.clone(),
                    samples_dir.to_path_buf(),
                )
                .map_err(audio_queue_failure)?;
            Ok(())
        }
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            generation,
            pan_pos,
            volume_pct,
        } => {
            audio.send(EngineEvent::SetFxBusMixer {
                bus_index: index_u8(*bus_index, "FX bus index")?,
                generation: *generation,
                pan_pos: *pan_pos,
                volume_pct: *volume_pct,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot,
            generation,
            path,
            value,
        } => {
            let param = SynthParamId::from_path(path).ok_or_else(|| {
                invalid_audio_command(format!("unsupported synth parameter path `{path}`"))
            })?;
            audio.send(EngineEvent::SetSynthParam {
                instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
                generation: *generation,
                param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetFmParam {
            instrument_slot,
            generation,
            path,
            value,
        } => {
            let param = FmParamId::from_path(path).ok_or_else(|| {
                invalid_audio_command(format!("unsupported FM parameter path `{path}`"))
            })?;
            audio.send(EngineEvent::SetFmParam {
                instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
                generation: *generation,
                param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetPluckParam {
            instrument_slot,
            generation,
            path,
            value,
        } => {
            let param = PluckParamId::from_path(path).ok_or_else(|| {
                invalid_audio_command(format!("unsupported Plucked parameter path `{path}`"))
            })?;
            audio.send(EngineEvent::SetPluckParam {
                instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
                generation: *generation,
                param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetDrumParam {
            instrument_slot,
            voice,
            generation,
            path,
            value,
        } => {
            let param = DrumParamId::from_path(path).ok_or_else(|| {
                invalid_audio_command(format!("unsupported Drum parameter path `{path}`"))
            })?;
            audio.send(EngineEvent::SetDrumParam {
                instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
                voice: *voice,
                generation: *generation,
                param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot,
            generation,
            path,
            value,
        } => {
            let param = SampleBankParamId::from_path(path).ok_or_else(|| {
                invalid_audio_command(format!("unsupported sample parameter path `{path}`"))
            })?;
            audio.send(EngineEvent::SetSampleBankParam {
                instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
                generation: *generation,
                param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetFxBusParam {
            bus_index,
            slot_index,
            generation,
            param,
            value,
        } => {
            audio.send(EngineEvent::SetFxBusParam {
                bus_index: index_u8(*bus_index, "FX bus index")?,
                slot_index: index_u8(*slot_index, "FX slot index")?,
                generation: *generation,
                param: *param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            generation,
            fx_type,
            params,
        } => {
            audio
                .enqueue_fx_bus_slot(
                    *bus_index,
                    *slot_index,
                    *generation,
                    fx_type.clone(),
                    params.clone(),
                )
                .map_err(audio_queue_failure)?;
            Ok(())
        }
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            generation,
            fx_type,
            params,
        } => {
            audio
                .enqueue_global_fx_slot(*slot_index, *generation, fx_type.clone(), params.clone())
                .map_err(audio_queue_failure)?;
            Ok(())
        }
        RuntimeAudioCommand::SetGlobalFxParam {
            slot_index,
            generation,
            param,
            value,
        } => {
            audio.send(EngineEvent::SetGlobalFxParam {
                slot_index: index_u8(*slot_index, "global FX slot index")?,
                generation: *generation,
                param: *param,
                value: *value,
            })?;
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxStart {
            id,
            epoch,
            fx_type,
            params,
            target,
        } => {
            validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
            let prepared = prepare_momentary_fx_start_with_epoch(
                id.clone(),
                *epoch,
                fx_type.clone(),
                params.clone(),
                momentary_fx_target(target),
                DEFAULT_AUDIO_SAMPLE_RATE,
            )
            .ok_or_else(|| invalid_audio_command("invalid momentary FX type".into()))?;
            audio.send(EngineEvent::PreparedMomentaryFxStart { config: prepared })?;
            audio
                .remember_momentary_fx_type(id, *epoch, fx_type)
                .map_err(audio_queue_failure)?;
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxUpdate { id, epoch, params } => {
            let (active_epoch, fx_type) = audio
                .momentary_fx_type(id)
                .map_err(audio_queue_failure)?
                .ok_or_else(|| invalid_audio_command(format!("unknown momentary FX id `{id}`")))?;
            if active_epoch != *epoch {
                return Err(invalid_audio_command(format!(
                    "stale momentary FX epoch for `{id}`"
                )));
            }
            let update = prepare_momentary_fx_update(
                *epoch,
                fx_type.clone(),
                params.clone(),
                DEFAULT_AUDIO_SAMPLE_RATE,
            )
            .ok_or_else(|| invalid_audio_command("invalid momentary FX update".into()))?;
            audio.send(EngineEvent::MomentaryFxUpdate(update))?;
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxStop { id, epoch } => {
            let (active_epoch, _) = audio
                .momentary_fx_type(id)
                .map_err(audio_queue_failure)?
                .ok_or_else(|| invalid_audio_command(format!("unknown momentary FX id `{id}`")))?;
            if active_epoch != *epoch {
                return Err(invalid_audio_command(format!(
                    "stale momentary FX epoch for `{id}`"
                )));
            }
            audio.send(EngineEvent::MomentaryFxStop { epoch: *epoch })?;
            audio
                .remove_momentary_fx_type(id, *epoch)
                .map_err(audio_queue_failure)?;
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

#[cfg(test)]
#[path = "host_audio_command_tests.rs"]
mod tests;
