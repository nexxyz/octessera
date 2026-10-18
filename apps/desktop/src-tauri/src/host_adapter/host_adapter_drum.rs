use super::host_adapter_audio_validation::{
    ensure_finite, invalid_audio_command, validate_instrument_slot,
};
use super::host_adapter_musical::{
    invalid_musical_event, validate_musical_channel, validate_musical_range,
};
use super::DesktopPlaybackHostAdapter;
use playback_runtime::{CellHit, RuntimeAdapterError, RuntimeOperation};
use realtime_engine::synth::DrumParamId;
use rodio_engine_source::EngineEvent;

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_drum_param(
        &self,
        instrument_slot: usize,
        voice: u8,
        generation: u64,
        path: &str,
        value: f32,
    ) -> Result<(), RuntimeAdapterError> {
        validate_instrument_slot(instrument_slot)?;
        let param = DrumParamId::from_path(path).ok_or_else(|| {
            invalid_audio_command(format!("unsupported Drum parameter path `{path}`"))
        })?;
        if voice >= 8 || (!param.is_voice_param() && voice != 0) {
            return Err(invalid_audio_command(format!("invalid Drum voice {voice}")));
        }
        ensure_finite(value, "Drum parameter")?;
        self.send_engine_event(
            EngineEvent::SetDrumParam {
                instrument_slot: instrument_slot as u8,
                voice,
                generation,
                param,
                value,
            },
            RuntimeOperation::AudioCommand,
        )
    }

    pub(super) fn handle_runtime_cell_hit(
        &mut self,
        hit: &CellHit,
    ) -> Result<(), RuntimeAdapterError> {
        validate_musical_channel(hit.instrument_slot)?;
        validate_musical_range(hit.voice, 7, "cell voice")?;
        let limit = realtime_engine::synth::CELL_PITCH_CENTS_LIMIT;
        if !(-limit..=limit).contains(&hit.pitch_cents) || hit.velocity > 127 {
            return Err(invalid_musical_event("invalid cell pitch or velocity"));
        }
        self.send_engine_event(
            EngineEvent::CellHit {
                instrument_slot: hit.instrument_slot,
                voice: hit.voice,
                cell: hit.cell,
                pitch_cents: hit.pitch_cents,
                velocity: hit.velocity,
            },
            RuntimeOperation::MusicalEvent,
        )
    }
}
