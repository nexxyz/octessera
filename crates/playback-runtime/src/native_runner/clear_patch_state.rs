use super::play_fx_config::default_play_fx_selected;
use super::*;

impl NativeRunner {
    pub(super) fn clear_patch_state(&mut self) -> Result<(), String> {
        self.stop_for_config_load();
        self.clear_all_modulation_sources();

        self.current_preset_name = None;
        self.preset_rename_source = None;
        self.preset_draft_name = fresh_preset_name();

        let behavior = platform_core::get_native_behavior("none")
            .ok_or_else(|| "unsupported native behavior `none`".to_string())?;
        self.behavior = behavior;
        self.behavior_config = Value::Null;
        self.layer_behavior_configs = vec![Value::Null; LAYER_COUNT];
        self.layer_behavior_config_history = vec![BTreeMap::new(); LAYER_COUNT];
        self.layer_behavior_ids = vec!["none".into(); LAYER_COUNT];
        self.layer_names = vec!["none".into(); LAYER_COUNT];
        self.layer_auto_names = vec![true; LAYER_COUNT];
        self.random_seed = super::random_streams::DEFAULT_RANDOM_SEED;
        self.layer_seeded = vec![false; LAYER_COUNT];

        self.global_sound = GlobalSoundConfig {
            velocity_scale_pct: 100,
            velocity_curve: VelocityCurve::Linear,
            note_length_ms: 120,
        };
        self.base_mapping_config = default_mapping_config();
        self.mapping_config = self.base_mapping_config.clone();
        self.transport.bpm = 120.0;
        self.transport.swing_a = 4;
        self.transport.swing_b = 4;
        self.voice_stealing_mode = "auto-balanced".into();
        self.link_layers = default_link_layers();
        self.grid_assign = None;
        self.trigger_probability_maps =
            vec![vec!["full".into(); GRID_WIDTH * GRID_HEIGHT]; LAYER_COUNT];
        self.instruments = (0..INSTRUMENT_COUNT)
            .map(NativeInstrumentSlot::reset)
            .collect();
        self.note_behaviors = note_behaviors_from_instruments(&self.instruments);
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();

        self.engine = Self::build_engine(
            behavior,
            Value::Null,
            self.interpretation_profile_for_layer(self.active_layer_index),
            self.mapping_config_for_layer(self.active_layer_index),
            self.global_sound.clone(),
            self.note_behaviors.clone(),
            self.active_layer_index,
        )?;
        self.layer_engines = (0..LAYER_COUNT)
            .map(|index| {
                if index == self.active_layer_index {
                    Ok(None)
                } else {
                    Self::build_engine(
                        behavior,
                        Value::Null,
                        self.interpretation_profile_for_layer(index),
                        self.mapping_config_for_layer(index),
                        self.global_sound.clone(),
                        self.note_behaviors.clone(),
                        index,
                    )
                    .map(Some)
                }
            })
            .collect::<Result<Vec<_>, String>>()?;

        self.transport.algorithm_step_pulses = DEFAULT_ALGORITHM_STEP_RED;
        self.transport.layer_algorithm_step_pulses = vec![DEFAULT_ALGORITHM_STEP_RED; LAYER_COUNT];
        self.sync_engine_runtime_config();
        for instrument in &mut self.instruments {
            instrument.tile_stamp = NativeTileStamp::default();
        }
        self.drum_selected_voices.fill(0);
        self.pending.tile_hits.clear();
        self.sample_browser = None;
        self.fx_buses = default_fx_buses();
        self.global_fx_slots = default_global_fx_slots();
        self.global_fx_params = default_global_fx_params();
        self.play_fx_selected = default_play_fx_selected();
        self.play_fx_assignments.clear();
        self.active_play_fx.clear();
        self.play_mode = "mix".into();
        self.active_play_mode = "none".into();
        self.xy_touch = NativeXyTouch {
            x: 0.5,
            y: 0.5,
            display_x: 0.5,
            display_y: 0.5,
            active: false,
        };
        self.xy_x_binding = None;
        self.xy_y_binding = None;
        self.xy_release = "sample-hold".into();
        self.xy_smoothing_ms = DEFAULT_XY_SMOOTHING_MS;
        self.xy_invert_x = false;
        self.xy_invert_y = false;
        self.xy_x_glide = None;
        self.xy_y_glide = None;
        self.param_mods = vec![NativeParamMods::default(); LAYER_COUNT];
        self.midi_cc_mappings = super::midi_cc_in::default_midi_cc_mappings();
        self.midi_cc_learn = None;
        self.faders = Default::default();
        self.aux_bindings.fill(None);
        self.shift_aux_bindings.fill(None);
        self.trigger_gate_modes = vec!["full".into(); LAYER_COUNT];
        self.trigger_gate_restore_modes = vec![None; LAYER_COUNT];
        self.play_live = Default::default();
        self.play_transpose_selected = vec![true; LAYER_COUNT];
        self.play_transpose_enabled = vec![true; LAYER_COUNT];
        self.play_transpose_offsets = vec![0; LAYER_COUNT];
        self.play_transpose_active_notes = vec![BTreeMap::new(); LAYER_COUNT];
        self.display.help_popup = None;
        self.display.confirm_dialog = None;
        self.pending.pending_menu_apply = None;
        self.pending.pending_autosave_payload_due_at = None;
        self.mark_config_dirty();
        self.commit_full_configuration_runtime_plan();
        self.show_toast("Cleared all");
        self.menu.rebuild(self.menu_config());
        Ok(())
    }
}
