use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::super::{
    momentary_fx_target, play_fx_tile_id, NativeActivePlayFx, NativePlayFxAssignment, NativeRunner,
    PLAY_FX_MAX_CONCURRENT,
};

const PLAY_FX_MOVING_TYPES: [&str; 3] = ["pitch_shift", "filter_sweep", "hp_riser"];

impl NativeRunner {
    /// A press of a tile whose type already sounds takes over from the older tile, which stays
    /// held underneath; the limit counts types, not tiles. A sweep type on the same Target keeps
    /// its voice and glides to the new tile's settings.
    pub(in crate::native_runner) fn play_fx_press_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let Some(assignment) = self.play_fx_assignment_at(x, y).cloned() else {
            return Vec::new();
        };
        let fx_type = assignment.config.fx_type.as_str().to_string();
        let id = play_fx_tile_id(x, y);
        if fx_type == "none" || self.play_fx_held(&id) {
            return Vec::new();
        }
        let same_type = self
            .active_play_fx
            .iter()
            .position(|active| active.fx_type == fx_type);
        if same_type.is_none() && self.active_play_fx.len() >= PLAY_FX_MAX_CONCURRENT {
            self.show_toast(format!("FX limit: {PLAY_FX_MAX_CONCURRENT} at once"));
            return Vec::new();
        }
        let mut effects = Vec::new();
        let mut voice_id = id.clone();
        if let Some(index) = same_type {
            let older = self.active_play_fx.remove(index);
            if self.play_fx_moves(&older.tile_id, &assignment) {
                effects.push(self.play_fx_update_effect(&older.voice_id, &assignment));
                voice_id = older.voice_id;
            } else {
                effects.push(play_fx_stop_effect(older.voice_id));
                effects.push(self.play_fx_start_effect_for_assignment(&assignment));
            }
            self.displaced_play_fx.push((older.tile_id, older.fx_type));
        } else {
            effects.push(self.play_fx_start_effect_for_assignment(&assignment));
        }
        self.active_play_fx.push(NativeActivePlayFx {
            tile_id: id,
            fx_type,
            voice_id,
        });
        effects
    }

    /// `None` when the tile holds no Play FX. Letting go of the sounding tile hands its type back
    /// to the most recent older tile of that type still held, the same way a press takes over;
    /// letting go of one underneath is silent.
    pub(in crate::native_runner) fn play_fx_release_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Option<Vec<RuntimePlatformEffect>> {
        let id = play_fx_tile_id(x, y);
        if let Some(index) = self
            .displaced_play_fx
            .iter()
            .position(|(held_id, _)| held_id == &id)
        {
            self.displaced_play_fx.remove(index);
            return Some(Vec::new());
        }
        let index = self
            .active_play_fx
            .iter()
            .position(|active| active.tile_id == id)?;
        let active = self.active_play_fx.remove(index);
        while let Some(index) = self
            .displaced_play_fx
            .iter()
            .rposition(|(_, held_type)| held_type == &active.fx_type)
        {
            let (tile_id, fx_type) = self.displaced_play_fx.remove(index);
            let Some(assignment) = self
                .play_fx_assignments
                .iter()
                .find(|assignment| play_fx_tile_id(assignment.x, assignment.y) == tile_id)
                .filter(|assignment| assignment.config.fx_type.as_str() == fx_type)
                .cloned()
            else {
                continue;
            };
            let (effects, voice_id) = if self.play_fx_moves(&active.tile_id, &assignment) {
                let update = self.play_fx_update_effect(&active.voice_id, &assignment);
                (vec![update], active.voice_id)
            } else {
                let start = self.play_fx_start_effect_for_assignment(&assignment);
                (
                    vec![play_fx_stop_effect(active.voice_id), start],
                    tile_id.clone(),
                )
            };
            self.active_play_fx.push(NativeActivePlayFx {
                tile_id,
                fx_type,
                voice_id,
            });
            return Some(effects);
        }
        Some(vec![play_fx_stop_effect(active.voice_id)])
    }

    fn play_fx_held(&self, id: &str) -> bool {
        self.active_play_fx
            .iter()
            .map(|active| &active.tile_id)
            .chain(self.displaced_play_fx.iter().map(|(held_id, _)| held_id))
            .any(|held_id| held_id == id)
    }

    /// A sweep type hands its running voice to the next tile of that type on the same Target,
    /// which glides it to the new settings instead of restarting it.
    fn play_fx_moves(&self, sounding_tile_id: &str, next: &NativePlayFxAssignment) -> bool {
        PLAY_FX_MOVING_TYPES.contains(&next.config.fx_type.as_str())
            && self
                .play_fx_assignments
                .iter()
                .find(|assignment| play_fx_tile_id(assignment.x, assignment.y) == sounding_tile_id)
                .is_some_and(|sounding| sounding.config.target_key == next.config.target_key)
    }

    fn play_fx_update_effect(
        &mut self,
        voice_id: &str,
        assignment: &NativePlayFxAssignment,
    ) -> RuntimePlatformEffect {
        let bpm = self.play_fx_tempo_bpm();
        self.play_live.held_fx_bpm = Some(bpm);
        RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxUpdate {
                id: voice_id.to_string(),
                epoch: 0,
                params: assignment.config.audio_params(bpm),
            },
        }
    }

    fn play_fx_start_effect_for_assignment(
        &mut self,
        assignment: &NativePlayFxAssignment,
    ) -> RuntimePlatformEffect {
        let bpm = self.play_fx_tempo_bpm();
        self.play_live.held_fx_bpm = Some(bpm);
        RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStart {
                id: play_fx_tile_id(assignment.x, assignment.y),
                epoch: 0,
                fx_type: assignment.config.fx_type.as_str().to_string(),
                params: assignment.config.audio_params(bpm),
                target: momentary_fx_target(&assignment.config.target_key),
            },
        }
    }

    fn play_fx_assignment_at(&self, x: usize, y: usize) -> Option<&NativePlayFxAssignment> {
        self.play_fx_assignments
            .iter()
            .find(|assignment| assignment.x == x && assignment.y == y)
    }
}

fn play_fx_stop_effect(id: String) -> RuntimePlatformEffect {
    RuntimePlatformEffect::AudioCommand {
        command: RuntimeAudioCommand::MomentaryFxStop { id, epoch: 0 },
    }
}
