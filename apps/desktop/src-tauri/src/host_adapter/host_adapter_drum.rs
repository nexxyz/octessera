use super::DesktopPlaybackHostAdapter;
use playback_runtime::{validate_tile_hit, RuntimeAdapterError, RuntimeOperation, TileHit};
use rodio_engine_source::EngineEvent;

impl DesktopPlaybackHostAdapter {
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
