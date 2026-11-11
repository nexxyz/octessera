use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

use super::play_fx_config::{play_fx_audio_params, play_fx_target_key, play_fx_type};
use super::{
    momentary_fx_target, play_fx_tile_id, NativePlayFxAssignment, NativeRunner,
    PLAY_FX_MAX_CONCURRENT,
};

impl NativeRunner {
    /// A press of a tile whose type already sounds takes over from the older tile, which stays
    /// held underneath; the limit counts types, not tiles.
    pub(super) fn play_fx_press_effects(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<RuntimePlatformEffect> {
        let Some(assignment) = self.play_fx_assignment_at(x, y).cloned() else {
            return Vec::new();
        };
        let fx_type = play_fx_type(&assignment.config).to_string();
        let id = play_fx_tile_id(x, y);
        if fx_type == "none" || self.play_fx_held(&id) {
            return Vec::new();
        }
        let same_type = self
            .active_play_fx
            .iter()
            .position(|(_, active_type)| active_type == &fx_type);
        if same_type.is_none() && self.active_play_fx.len() >= PLAY_FX_MAX_CONCURRENT {
            self.show_toast(format!("FX limit: {PLAY_FX_MAX_CONCURRENT} at once"));
            return Vec::new();
        }
        let mut effects = Vec::new();
        if let Some(index) = same_type {
            let older = self.active_play_fx.remove(index);
            effects.push(play_fx_stop_effect(older.0.clone()));
            self.displaced_play_fx.push(older);
        }
        effects.push(self.play_fx_start_effect_for_assignment(&assignment));
        self.active_play_fx.push((id, fx_type));
        effects
    }

    /// `None` when the tile holds no Play FX. Letting go of the sounding tile hands its type back
    /// to the most recent older tile of that type still held; letting go of one underneath is
    /// silent.
    pub(super) fn play_fx_release_effects(
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
            .position(|(active_id, _)| active_id == &id)?;
        let (id, fx_type) = self.active_play_fx.remove(index);
        let mut effects = vec![play_fx_stop_effect(id)];
        while let Some(index) = self
            .displaced_play_fx
            .iter()
            .rposition(|(_, held_type)| held_type == &fx_type)
        {
            let older = self.displaced_play_fx.remove(index);
            let Some(assignment) = self
                .play_fx_assignments
                .iter()
                .find(|assignment| play_fx_tile_id(assignment.x, assignment.y) == older.0)
                .filter(|assignment| play_fx_type(&assignment.config) == fx_type)
                .cloned()
            else {
                continue;
            };
            effects.push(self.play_fx_start_effect_for_assignment(&assignment));
            self.active_play_fx.push(older);
            break;
        }
        Some(effects)
    }

    fn play_fx_held(&self, id: &str) -> bool {
        self.active_play_fx
            .iter()
            .chain(&self.displaced_play_fx)
            .any(|(held_id, _)| held_id == id)
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
                fx_type: play_fx_type(&assignment.config).to_string(),
                params: play_fx_audio_params(&assignment.config, bpm),
                target: momentary_fx_target(play_fx_target_key(&assignment.config)),
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
