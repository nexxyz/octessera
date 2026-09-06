use super::construction_seed::NativeRunnerConstructionSeed;
use super::*;

impl NativeRunner {
    pub fn new(config: NativeRunnerConfig) -> Result<Self, String> {
        if !config
            .audio_optimization
            .is_supported(config.audio_optimization_capacity_available)
        {
            return Err("audio optimization capacity is unavailable".into());
        }
        let behavior = platform_core::get_native_behavior(&config.behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{}`", config.behavior_id))?;
        let engine = Self::build_engine(
            behavior,
            config.behavior_config.clone(),
            config.interpretation_profile.clone(),
            config.mapping_config.clone(),
            config.global_sound.clone(),
            config.note_behaviors.clone(),
            0,
        )?;
        let ui = NativeUiState::default();
        let now = Instant::now();
        let seed = NativeRunnerConstructionSeed::new(&config, behavior, ui, now);
        let mut layer_engines = Vec::new();
        layer_engines.resize_with(LAYER_COUNT, || None);
        for (index, slot) in layer_engines.iter_mut().enumerate().skip(1) {
            let layer_behavior = platform_core::get_native_behavior(config.behavior_id.as_str())
                .ok_or_else(|| format!("unsupported native behavior `{}`", config.behavior_id))?;
            *slot = Some(Self::build_engine(
                layer_behavior,
                config.behavior_config.clone(),
                config.interpretation_profile.clone(),
                config.mapping_config.clone(),
                config.global_sound.clone(),
                config.note_behaviors.clone(),
                index,
            )?);
        }
        let preset_draft_name = fresh_preset_name();
        let menu = NativeMenuModel::new(seed.initial_menu_config(&preset_draft_name));
        let mut runner = Self {
            engine,
            layer_engines,
            behavior,
            behavior_config: config.behavior_config.clone(),
            layer_behavior_configs: vec![config.behavior_config.clone(); LAYER_COUNT],
            layer_behavior_config_history: vec![BTreeMap::new(); LAYER_COUNT],
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config.clone(),
            base_mapping_config: config.mapping_config,
            global_sound: seed.global_sound,
            dsp_config: seed.dsp_config,
            audio_optimization: seed.audio_optimization,
            audio_optimization_capacity_available: seed.audio_optimization_capacity_available,
            jack_audio_required: config.jack_audio_required,
            note_behaviors: config.note_behaviors,
            transport: NativeTransportState::new(
                seed.bpm,
                seed.swing_pct,
                seed.sync_source.clone(),
                seed.algorithm_step_pulses,
            ),
            delayed_link_events: vec![Vec::new(); LAYER_COUNT],
            link_arp_held_notes: vec![Vec::new(); LAYER_COUNT],
            link_arp_rotating_phase: vec![0; LAYER_COUNT],
            link_arp_random_state: LINK_ARP_RANDOM_SEED,
            audio_output_buffer_frames: normalize_audio_output_buffer_frames(
                seed.audio_output_buffer_frames,
            ),
            display: NativeDisplayState::new(seed.ui, seed.now, seed.hdmi),
            midi_enabled: seed.midi_enabled,
            audio_outputs: seed.audio_outputs,
            usb_midi_out_enabled: seed.usb_midi_out_enabled,
            recording_max_minutes: seed.recording_max_minutes,
            preset_names: seed.preset_names,
            current_preset_name: None,
            preset_draft_name,
            preset_rename_source: None,
            outbox: NativeRunnerOutbox::default(),
            midi_outputs: seed.midi_outputs,
            midi_inputs: seed.midi_inputs,
            midi_status: None,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            input_events_while_paused: seed.input_events_while_paused,
            voice_stealing_mode: seed.voice_stealing_mode,
            midi_clock_out_enabled: seed.midi_clock_out_enabled,
            midi_clock_in_enabled: seed.midi_clock_in_enabled,
            midi_respond_to_start_stop: seed.midi_respond_to_start_stop,
            sparks_mode: seed.sparks_mode,
            active_sparks_mode: "none".into(),
            sparks_fx_selected: seed.sparks_fx_selected,
            sparks_fx_assign: None,
            sparks_fx_assignments: vec![],
            active_sparks_fx: Vec::new(),
            xy_touch: NativeXyTouch {
                x: 0.5,
                y: 0.5,
                display_x: 0.5,
                display_y: 0.5,
                active: false,
            },
            xy_release: seed.xy_release,
            xy_invert_x: seed.xy_invert_x,
            xy_invert_y: seed.xy_invert_y,
            xy_x_binding: seed.xy_x_binding,
            xy_y_binding: seed.xy_y_binding,
            aux_auto_map_enabled: seed.aux_auto_map_enabled,
            param_mods: seed.param_mods,
            trigger_gate_modes: vec!["full".into(); LAYER_COUNT],
            trigger_gate_restore_modes: vec![None; LAYER_COUNT],
            sparks_transpose_selected: vec![true; LAYER_COUNT],
            sparks_transpose_enabled: vec![true; LAYER_COUNT],
            sparks_transpose_offsets: vec![0; LAYER_COUNT],
            sparks_transpose_active_notes: vec![BTreeMap::new(); LAYER_COUNT],
            pending_transpose_note_offs: RoutedMusicalEvents::default(),
            trigger_probability_assign: None,
            trigger_probability_maps: vec![
                vec!["full".into(); GRID_WIDTH * GRID_HEIGHT];
                LAYER_COUNT
            ],
            layer_behavior_ids: seed.layer_behavior_ids,
            layer_names: seed.layer_names,
            layer_auto_names: seed.layer_auto_names,
            save_grid_states: seed.save_grid_states,
            link_lfos: seed.link_lfos,
            modulation_process: ModulationProcessState::default(),
            pulses_layers: seed.pulses_layers,
            aux_bindings: seed.aux_bindings,
            shift_aux_bindings: seed.shift_aux_bindings,
            active_layer_index: seed.active_layer_index,
            instruments: seed.instruments,
            sample_assign: None,
            fx_buses: seed.fx_buses,
            global_fx_slots: seed.global_fx_slots,
            global_fx_params: seed.global_fx_params,
            sample_browser: None,
            sample_availability: seed.sample_availability,
            sample_builtin_favourite_dirs: seed.sample_builtin_favourite_dirs,
            sample_favourite_dirs: seed.sample_favourite_dirs,
            menu,
            auto_save_default: seed.auto_save_default,
            rolling_backups: seed.rolling_backups,
            config_dirty: false,
            config_revision: 0,
            dirty_revision: None,
            last_backup_save_at: None,
            audio_config_revision: 0,
            last_snapshot_audio_config_revision: None,
            last_published_runtime_config: None,
            trigger_probability_rng: 0xC311_5A7E_2024_0001,
            pending: NativePendingState::default(),
            #[cfg(test)]
            behavior_state_serialization_calls: Cell::new(0),
            #[cfg(test)]
            layer_behavior_rebuilds: 0,
            #[cfg(test)]
            fast_autosave_marks: 0,
            #[cfg(test)]
            modulation_process_calls: 0,
            #[cfg(test)]
            engine_runtime_sync_calls: 0,
            #[cfg(test)]
            active_pulses_refresh_calls: 0,
            #[cfg(any(test, feature = "test-support"))]
            test_snapshot_failure: Cell::new(false),
        };
        runner.seed_visible_state()?;
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
        runner
            .engine
            .set_interpretation_profile(runner.interpretation_profile.clone());
        runner.menu.rebuild(runner.menu_config());
        Ok(runner)
    }
}
