use super::super::*;

impl NativeRunner {
    pub(in crate::native_runner) fn build_engine(
        behavior: NativeBehavior,
        behavior_config: Value,
        interpretation_profile: InterpretationProfile,
        mapping_config: platform_core::MappingConfig,
        global_sound: GlobalSoundConfig,
        note_behaviors: Vec<NoteBehavior>,
        layer_index: usize,
    ) -> Result<NativeLayerEngine, String> {
        NativeLayerEngine::new(NativeLayerEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile,
            mapping_config,
            global_sound,
            note_behaviors,
            layer_index,
            random_seed: None,
        })
    }

    pub(in crate::native_runner) fn rebuild_engine(
        &mut self,
        behavior: NativeBehavior,
    ) -> Result<(), String> {
        let behavior_id = behavior.id().to_string();
        let config = if behavior_id == self.active_behavior().id() {
            self.active_layer().behavior_config.clone()
        } else {
            self.remembered_layer_behavior_config(self.active_layer_index, &behavior_id)
        };
        if let Some(layer) = self.layers.get_mut(self.active_layer_index) {
            layer.behavior_id = behavior_id.clone();
        }
        self.replace_layer_engine_with_config(
            self.active_layer_index,
            behavior,
            config.clone(),
            None,
        )?;
        self.set_layer_behavior_config(self.active_layer_index, &behavior_id, config);
        Ok(())
    }

    pub(in crate::native_runner) fn reset_transport_position(&mut self) {
        self.clear_lfo_audio();
        self.drain_all_layer_notes();
        self.transport.pending_resync = false;
        self.transport.awaiting_external_downbeat = false;
        self.play_live.rate_resyncs = [false; GRID_HEIGHT];
        self.transport.tick = 0;
        self.transport.current_ppqn_pulse = 0;
        self.transport.swung_ppqn_pulse = 0;
        self.transport.applied_swing = self.swing();
        self.transport.layer_ticks.fill(0);
        let now = self.display.transients.now();
        self.display.transients.reset(now);
        self.reset_global_lfo_phases();
        self.restart_random_streams();
        for layer in &mut self.layers {
            layer.engine.reset_transport_phase();
        }
        self.transport.layer_pulse_accumulators.fill(0);
        for queue in &mut self.delayed_link_events {
            queue.clear();
        }
        self.clear_all_link_arp_state();
        self.trigger_probability_rng = TRIGGER_PROBABILITY_RNG_INITIAL_SEED;
    }

    pub(in crate::native_runner) fn sync_engine_runtime_config(&mut self) {
        #[cfg(test)]
        {
            self.engine_runtime_sync_calls = self.engine_runtime_sync_calls.saturating_add(1);
        }
        self.note_behaviors = note_behaviors_from_instruments(&self.instruments);
        for layer in &mut self.layers {
            layer.engine.set_global_sound(self.global_sound.clone());
            layer.engine.set_note_behaviors(self.note_behaviors.clone());
        }
    }
}
