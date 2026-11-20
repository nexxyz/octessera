use crate::audio::AudioService;
use playback_runtime::{
    apply_runtime_audio_command, validate_audio_command, MomentaryFxTypes, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimeAudioCommandHost,
};
use rodio_engine_source::EngineEvent;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// Without an audio service a command is only validated.
pub fn send_audio_command(
    audio: Option<&AudioService>,
    momentary_fx_types: &mut MomentaryFxTypes,
    command: &RuntimeAudioCommand,
    samples_dir: &Path,
) -> Result<(), RuntimeAdapterError> {
    let Some(audio) = audio else {
        return validate_audio_command(command);
    };
    apply_runtime_audio_command(
        &mut PiAudioCommandHost {
            audio,
            samples_dir,
            momentary_fx_types,
        },
        command,
    )
}

struct PiAudioCommandHost<'a> {
    audio: &'a AudioService,
    samples_dir: &'a Path,
    momentary_fx_types: &'a mut MomentaryFxTypes,
}

impl RuntimeAudioCommandHost for PiAudioCommandHost<'_> {
    fn send_audio_event(&mut self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        self.audio.send(event)
    }

    fn enqueue_full_config(
        &mut self,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio.enqueue_full_config(
            revision,
            generation,
            request_id,
            config,
            self.samples_dir.to_path_buf(),
        )
    }

    fn enqueue_instrument_slot(
        &mut self,
        instrument_slot: usize,
        generation: u64,
        config: Value,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio.enqueue_instrument_slot(
            instrument_slot,
            generation,
            config,
            self.samples_dir.to_path_buf(),
        )
    }

    fn enqueue_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio
            .enqueue_fx_bus_slot(bus_index, slot_index, generation, fx_type, params)
    }

    fn enqueue_global_fx_slot(
        &mut self,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio
            .enqueue_global_fx_slot(slot_index, generation, fx_type, params)
    }

    fn enqueue_sample_preview(
        &mut self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
    ) -> Result<(), RuntimeAdapterError> {
        self.audio.enqueue_sample_preview(
            instrument_slot,
            path,
            velocity,
            self.samples_dir.to_path_buf(),
        )
    }

    fn momentary_fx_types(&mut self) -> &mut MomentaryFxTypes {
        self.momentary_fx_types
    }
}

#[cfg(test)]
mod tests;
