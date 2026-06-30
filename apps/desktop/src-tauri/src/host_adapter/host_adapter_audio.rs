use crate::audio_config::decode_sample_file;
use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::samples::resolve_sample_file;
use crate::types::{MomentaryFxTargetPayload, QueuedAudioEvent};
use playback_runtime::{RuntimeAudioCommand, RuntimeMomentaryFxTarget};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), String> {
        if let RuntimeAudioCommand::SetAudioConfig { revision, config } = command {
            return self.handle_full_audio_config(*revision, config.clone());
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
            RuntimeAudioCommand::SetFxBusMixer { bus_index, pan_pos } => {
                QueuedAudioEvent::SetFxBusMixer {
                    bus_index: *bus_index,
                    pan_pos: *pan_pos,
                }
            }
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot,
                path,
                value,
            } => QueuedAudioEvent::SetSynthParam {
                instrument_slot: *instrument_slot,
                path: path.clone(),
                value: *value,
            },
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot,
                path,
                value,
            } => QueuedAudioEvent::SetSampleBankParam {
                instrument_slot: *instrument_slot,
                path: path.clone(),
                value: *value,
            },
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                fx_type,
                params,
            } => QueuedAudioEvent::SetFxBusSlot {
                bus_index: *bus_index,
                slot_index: *slot_index,
                fx_type: fx_type.clone(),
                params: params.clone(),
            },
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                fx_type,
                params,
            } => QueuedAudioEvent::SetGlobalFxSlot {
                slot_index: *slot_index,
                fx_type: fx_type.clone(),
                params: params.clone(),
            },
            RuntimeAudioCommand::MomentaryFxStart {
                id,
                fx_type,
                params,
                target,
            } => QueuedAudioEvent::MomentaryFxStart {
                id: id.clone(),
                fx_type: fx_type.clone(),
                params: params.clone(),
                target: momentary_fx_target_payload(target),
            },
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
        self.audio.audio_control.enqueue_dynamic(event)
    }

    fn sample_preview_event(
        &self,
        instrument_slot: usize,
        path: &str,
        velocity: u8,
    ) -> Result<QueuedAudioEvent, String> {
        let full_path =
            resolve_sample_file(path).ok_or_else(|| "invalid sample path".to_string())?;
        let buffer = {
            let mut cache = self
                .audio
                .sample_cache
                .lock()
                .map_err(|_| "sample cache poisoned".to_string())?;
            if let Some(buffer) = cache.get(&full_path).cloned() {
                buffer
            } else {
                let buffer = decode_sample_file(&full_path)
                    .ok_or_else(|| "sample decode failed".to_string())?;
                cache.insert(full_path, buffer.clone());
                buffer
            }
        };
        Ok(QueuedAudioEvent::PreviewSample {
            instrument_slot: instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1) as u8,
            buffer,
            velocity,
        })
    }
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
