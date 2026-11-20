use crate::{
    invalid_audio_command, validate_audio_command, RuntimeAdapterError, RuntimeAudioCommand,
};
use realtime_engine::synth::{
    prepare_momentary_fx_start_with_epoch, prepare_momentary_fx_update, DrumParamId, EngineEvent,
    FmParamId, PluckParamId, SampleBankParamId, SynthParamId, DEFAULT_AUDIO_SAMPLE_RATE,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// The momentary FX a host has started: id to its epoch and FX type.
pub type MomentaryFxTypes = BTreeMap<String, (u64, String)>;

/// How a host carries runtime audio commands to its engine. Latest-value controls go straight
/// to the engine; structural changes and previews go to the host's preparation lane.
pub trait RuntimeAudioCommandHost {
    fn send_audio_event(&mut self, event: EngineEvent) -> Result<(), RuntimeAdapterError>;
    fn enqueue_full_config(
        &mut self,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
    ) -> Result<(), RuntimeAdapterError>;
    fn enqueue_instrument_slot(
        &mut self,
        instrument_slot: usize,
        generation: u64,
        config: Value,
    ) -> Result<(), RuntimeAdapterError>;
    fn enqueue_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError>;
    fn enqueue_global_fx_slot(
        &mut self,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError>;
    fn enqueue_sample_preview(
        &mut self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
    ) -> Result<(), RuntimeAdapterError>;
    fn momentary_fx_types(&mut self) -> &mut MomentaryFxTypes;
}

/// Validates a runtime audio command, maps its string paths to native parameter IDs and hands
/// it to the host. A momentary FX update or stop must name the active epoch of its id.
pub fn apply_runtime_audio_command(
    host: &mut impl RuntimeAudioCommandHost,
    command: &RuntimeAudioCommand,
) -> Result<(), RuntimeAdapterError> {
    validate_audio_command(command)?;
    match command {
        RuntimeAudioCommand::SetAudioConfig {
            revision,
            generation,
            request_id,
            config,
        } => host.enqueue_full_config(*revision, *generation, request_id.clone(), config.clone()),
        RuntimeAudioCommand::SetMasterVolume {
            generation,
            volume_pct,
        } => host.send_audio_event(EngineEvent::SetMasterVolume {
            generation: *generation,
            volume_pct: *volume_pct,
        }),
        RuntimeAudioCommand::SetDspConfig { generation, config } => {
            host.send_audio_event(EngineEvent::SetDspConfig {
                generation: *generation,
                config: *config,
            })
        }
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            generation,
            volume_pct,
            pan_pos,
        } => host.send_audio_event(EngineEvent::SetInstrumentMixer {
            instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
            generation: *generation,
            volume_pct: *volume_pct,
            pan_pos: *pan_pos,
        }),
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot,
            generation,
            config,
        } => host.enqueue_instrument_slot(*instrument_slot, *generation, config.clone()),
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            generation,
            pan_pos,
            volume_pct,
        } => host.send_audio_event(EngineEvent::SetFxBusMixer {
            bus_index: index_u8(*bus_index, "FX bus index")?,
            generation: *generation,
            pan_pos: *pan_pos,
            volume_pct: *volume_pct,
        }),
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot,
            generation,
            path,
            value,
        } => host.send_audio_event(EngineEvent::SetSynthParam {
            instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
            generation: *generation,
            param: param_id(SynthParamId::from_path(path), "synth", path)?,
            value: *value,
        }),
        RuntimeAudioCommand::SetFmParam {
            instrument_slot,
            generation,
            path,
            value,
        } => host.send_audio_event(EngineEvent::SetFmParam {
            instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
            generation: *generation,
            param: param_id(FmParamId::from_path(path), "FM", path)?,
            value: *value,
        }),
        RuntimeAudioCommand::SetPluckParam {
            instrument_slot,
            generation,
            path,
            value,
        } => host.send_audio_event(EngineEvent::SetPluckParam {
            instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
            generation: *generation,
            param: param_id(PluckParamId::from_path(path), "Plucked", path)?,
            value: *value,
        }),
        RuntimeAudioCommand::SetDrumParam {
            instrument_slot,
            voice,
            generation,
            path,
            value,
        } => host.send_audio_event(EngineEvent::SetDrumParam {
            instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
            voice: *voice,
            generation: *generation,
            param: param_id(DrumParamId::from_path(path), "Drum", path)?,
            value: *value,
        }),
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot,
            generation,
            path,
            value,
        } => host.send_audio_event(EngineEvent::SetSampleBankParam {
            instrument_slot: index_u8(*instrument_slot, "instrument slot")?,
            generation: *generation,
            param: param_id(SampleBankParamId::from_path(path), "sample", path)?,
            value: *value,
        }),
        RuntimeAudioCommand::SetFxBusParam {
            bus_index,
            slot_index,
            generation,
            param,
            value,
        } => host.send_audio_event(EngineEvent::SetFxBusParam {
            bus_index: index_u8(*bus_index, "FX bus index")?,
            slot_index: index_u8(*slot_index, "FX slot index")?,
            generation: *generation,
            param: *param,
            value: *value,
        }),
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            generation,
            fx_type,
            params,
        } => host.enqueue_fx_bus_slot(
            *bus_index,
            *slot_index,
            *generation,
            fx_type.clone(),
            params.clone(),
        ),
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            generation,
            fx_type,
            params,
        } => host.enqueue_global_fx_slot(*slot_index, *generation, fx_type.clone(), params.clone()),
        RuntimeAudioCommand::SetGlobalFxParam {
            slot_index,
            generation,
            param,
            value,
        } => host.send_audio_event(EngineEvent::SetGlobalFxParam {
            slot_index: index_u8(*slot_index, "global FX slot index")?,
            generation: *generation,
            param: *param,
            value: *value,
        }),
        RuntimeAudioCommand::MomentaryFxStart {
            id,
            epoch,
            fx_type,
            params,
            target,
        } => {
            let prepared = prepare_momentary_fx_start_with_epoch(
                id.clone(),
                *epoch,
                fx_type.clone(),
                params.clone(),
                target.into(),
                DEFAULT_AUDIO_SAMPLE_RATE,
            )
            .ok_or_else(|| invalid_audio_command("invalid momentary FX type".into()))?;
            host.send_audio_event(EngineEvent::PreparedMomentaryFxStart { config: prepared })?;
            host.momentary_fx_types()
                .insert(id.clone(), (*epoch, fx_type.clone()));
            Ok(())
        }
        RuntimeAudioCommand::MomentaryFxUpdate { id, epoch, params } => {
            let fx_type = active_momentary_fx_type(host.momentary_fx_types(), id, *epoch)?;
            let update = prepare_momentary_fx_update(
                *epoch,
                fx_type,
                params.clone(),
                DEFAULT_AUDIO_SAMPLE_RATE,
            )
            .ok_or_else(|| invalid_audio_command("invalid momentary FX update".into()))?;
            host.send_audio_event(EngineEvent::MomentaryFxUpdate(update))
        }
        RuntimeAudioCommand::MomentaryFxStop { id, epoch } => {
            active_momentary_fx_type(host.momentary_fx_types(), id, *epoch)?;
            host.send_audio_event(EngineEvent::MomentaryFxStop { epoch: *epoch })?;
            host.momentary_fx_types().remove(id);
            Ok(())
        }
        RuntimeAudioCommand::SamplePreview {
            instrument_slot,
            path,
            velocity,
            ..
        } => host.enqueue_sample_preview(*instrument_slot, path.clone(), *velocity),
    }
}

fn active_momentary_fx_type(
    types: &MomentaryFxTypes,
    id: &str,
    epoch: u64,
) -> Result<String, RuntimeAdapterError> {
    let (active_epoch, fx_type) = types
        .get(id)
        .ok_or_else(|| invalid_audio_command(format!("unknown momentary FX id `{id}`")))?;
    if *active_epoch != epoch {
        return Err(invalid_audio_command(format!(
            "stale momentary FX epoch for `{id}`"
        )));
    }
    Ok(fx_type.clone())
}

fn param_id<T>(param: Option<T>, kind: &str, path: &str) -> Result<T, RuntimeAdapterError> {
    param
        .ok_or_else(|| invalid_audio_command(format!("unsupported {kind} parameter path `{path}`")))
}

fn index_u8(index: usize, label: &str) -> Result<u8, RuntimeAdapterError> {
    u8::try_from(index)
        .map_err(|_| invalid_audio_command(format!("{label} is out of range: {index}")))
}

#[cfg(test)]
mod tests;
