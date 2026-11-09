use super::DesktopPlaybackHostAdapter;
use playback_runtime::{
    invalid_audio_command, validate_tile_hit, RuntimeAdapterError, RuntimeOperation, TileHit,
};
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
        let param = DrumParamId::from_path(path).ok_or_else(|| {
            invalid_audio_command(format!("unsupported Drum parameter path `{path}`"))
        })?;
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

    pub(super) fn handle_runtime_tile_hit(
        &mut self,
        hit: &TileHit,
        at: Option<std::time::Instant>,
    ) -> Result<(), RuntimeAdapterError> {
        validate_tile_hit(hit)?;
        self.send_engine_event(
            EngineEvent::TileHit {
                instrument_slot: hit.instrument_slot,
                voice: hit.voice,
                tile: hit.tile,
                pitch_cents: hit.pitch_cents,
                velocity: hit.velocity,
                at,
            },
            RuntimeOperation::MusicalEvent,
        )
    }
}
