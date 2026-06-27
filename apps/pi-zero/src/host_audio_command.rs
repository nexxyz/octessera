use crate::audio::AudioService;
use playback_runtime::{RuntimeAudioCommand, RuntimeMomentaryFxTarget};
use realtime_engine::synth::MomentaryFxTarget;
use rodio_engine_source::EngineEvent;
use std::path::Path;
use std::sync::atomic::Ordering;

pub fn send_audio_command(
    audio: Option<AudioService>,
    command: &RuntimeAudioCommand,
    samples_dir: &Path,
) -> Result<(), String> {
    let Some(audio) = audio else {
        return Ok(());
    };
    match command {
        RuntimeAudioCommand::SetAudioConfig { config, .. } => {
            let revision = audio.config_revision.fetch_add(1, Ordering::SeqCst) + 1;
            audio.enqueue_full_config(revision, config.clone(), samples_dir.to_path_buf())
        }
        RuntimeAudioCommand::SetMasterVolume { volume_pct } => {
            audio.send(EngineEvent::SetMasterVolume {
                volume_pct: *volume_pct,
            })
        }
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            volume_pct,
            pan_pos,
        } => audio.send(EngineEvent::SetInstrumentMixer {
            instrument_slot: *instrument_slot,
            volume_pct: *volume_pct,
            pan_pos: *pan_pos,
        }),
        RuntimeAudioCommand::SetFxBusMixer { bus_index, pan_pos } => {
            audio.send(EngineEvent::SetFxBusMixer {
                bus_index: *bus_index,
                pan_pos: *pan_pos,
            })
        }
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot,
            path,
            value,
        } => audio.send(EngineEvent::SetSynthParam {
            instrument_slot: *instrument_slot,
            path: path.clone(),
            value: *value,
        }),
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot,
            path,
            value,
        } => audio.send(EngineEvent::SetSampleBankParam {
            instrument_slot: *instrument_slot,
            path: path.clone(),
            value: *value,
        }),
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
        } => audio.send(EngineEvent::SetFxBusSlot {
            bus_index: *bus_index,
            slot_index: *slot_index,
            fx_type: fx_type.clone(),
            params: params.clone(),
        }),
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
        } => audio.send(EngineEvent::SetGlobalFxSlot {
            slot_index: *slot_index,
            fx_type: fx_type.clone(),
            params: params.clone(),
        }),
        RuntimeAudioCommand::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        } => audio.send(EngineEvent::MomentaryFxStart {
            id: id.clone(),
            fx_type: fx_type.clone(),
            params: params.clone(),
            target: match target {
                RuntimeMomentaryFxTarget::Global => MomentaryFxTarget::Global,
                RuntimeMomentaryFxTarget::FxBus { index } => {
                    MomentaryFxTarget::FxBus { index: *index }
                }
                RuntimeMomentaryFxTarget::Instrument { index } => {
                    MomentaryFxTarget::Instrument { index: *index }
                }
            },
        }),
        RuntimeAudioCommand::MomentaryFxUpdate { id, params } => {
            audio.send(EngineEvent::MomentaryFxUpdate {
                id: id.clone(),
                params: params.clone(),
            })
        }
        RuntimeAudioCommand::MomentaryFxStop { id } => {
            audio.send(EngineEvent::MomentaryFxStop { id: id.clone() })
        }
        RuntimeAudioCommand::SamplePreview { .. } => Ok(()),
    }
}
