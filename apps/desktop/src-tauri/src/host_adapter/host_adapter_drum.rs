use super::host_adapter_audio::{ensure_finite, invalid_audio_command, validate_instrument_slot};
use super::host_adapter_musical::{
    invalid_musical_event, validate_musical_channel, validate_musical_range,
};
use super::DesktopPlaybackHostAdapter;
use playback_runtime::{DrumHit, RuntimeAdapterError, RuntimeOperation};
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

    pub(super) fn handle_runtime_drum_hit(
        &mut self,
        hit: &DrumHit,
    ) -> Result<(), RuntimeAdapterError> {
        validate_musical_channel(hit.instrument_slot)?;
        validate_musical_range(hit.voice, 7, "Drum voice")?;
        if !(-24..=24).contains(&hit.tune_semis) || !(1..=127).contains(&hit.velocity) {
            return Err(invalid_musical_event("invalid Drum tune or velocity"));
        }
        self.send_engine_event(
            EngineEvent::DrumHit {
                instrument_slot: hit.instrument_slot,
                voice: hit.voice,
                tune_semis: hit.tune_semis,
                velocity: hit.velocity,
            },
            RuntimeOperation::MusicalEvent,
        )
    }
}
