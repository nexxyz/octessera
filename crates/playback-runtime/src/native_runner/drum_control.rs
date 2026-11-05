use super::grid_assign::GridAssign;
use super::NativeRunner;
use crate::protocol::TileHit;

impl NativeRunner {
    pub(super) fn enter_drum_assign(&mut self, slot: usize, voice: usize) {
        if self
            .instruments
            .get(slot)
            .is_none_or(|instrument| instrument.kind != "drum")
            || voice >= 8
        {
            return;
        }
        self.grid_assign = Some(GridAssign::Drum {
            slot,
            voice: voice as u8,
        });
        self.show_toast(format!("V{}: press tiles to assign", voice + 1));
    }

    pub(super) fn preview_drum_voice(&mut self, slot: usize, voice: usize) {
        if voice >= 8
            || self
                .instruments
                .get(slot)
                .is_none_or(|instrument| instrument.kind != "drum")
        {
            return;
        }
        self.pending.tile_hits.push(TileHit {
            instrument_slot: slot as u8,
            voice: voice as u8,
            tile: 0,
            pitch_cents: 0,
            velocity: 100,
            midi_note: super::drum_config::drum_voice_midi_note(
                &self.instruments[slot].drum_config,
                voice as u8,
            ),
        });
    }
}
