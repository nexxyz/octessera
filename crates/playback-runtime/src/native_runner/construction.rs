use super::*;

impl NativeRunner {
    pub fn new(config: NativeRunnerConfig) -> Result<Self, String> {
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
        let instruments = default_instruments();
        let sense_parts = default_sense_parts();
        let fx_buses = default_fx_buses();
        let global_fx_slots = default_global_fx_slots();
        let global_fx_params = default_global_fx_params();
        let menu = NativeMenuModel::new(NativeMenuConfig {
            behavior_id: behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            l1_items: vec![],
            behavior_target_items: vec![vec![]; PART_COUNT],
            part_labels: (0..PART_COUNT)
                .map(|index| format!("P{}: life", index + 1))
                .collect(),
            part_names: vec![behavior.id().into(); PART_COUNT],
            part_auto_names: vec![true; PART_COUNT],
            sense_parts: sense_part_configs(&sense_parts),
            active_part_index: 0,
            param_mods: vec![NativeParamModsConfig::default(); PART_COUNT],
            xy_x_binding: None,
            xy_y_binding: None,
            aux_auto_map_enabled: true,
            aux_bindings: vec![NativeAuxBindingConfig::default(); platform_core::AUX_ENCODER_COUNT],
            instrument_labels: instrument_labels(&instruments),
            instrument_names: instrument_names(&instruments),
            instrument_types: instrument_types(&instruments),
            instrument_auto_names: instrument_auto_names(&instruments),
            instrument_note_behaviors: instrument_note_behaviors(&instruments),
            instrument_routes: instrument_routes(&instruments),
            instrument_volumes: instrument_volumes(&instruments),
            instrument_pan_positions: instrument_pan_positions(&instruments),
            instrument_sample_slots: instrument_sample_slots(&instruments),
            instrument_synth_configs: instrument_synth_configs(&instruments),
            instrument_synth_osc1_waveforms: instrument_synth_osc1_waveforms(&instruments),
            instrument_synth_osc2_waveforms: instrument_synth_osc2_waveforms(&instruments),
            instrument_synth_filter_types: instrument_synth_filter_types(&instruments),
            instrument_synth_filter_cutoffs: instrument_synth_filter_cutoffs(&instruments),
            instrument_synth_gain_pct: instrument_synth_gain_pct(&instruments),
            instrument_synth_filter_resonance: instrument_synth_filter_resonance(&instruments),
            instrument_sample_tune_semis: instrument_sample_tune_semis(&instruments),
            instrument_sample_gain_pct: instrument_sample_gain_pct(&instruments),
            instrument_sample_base_velocity: instrument_sample_base_velocity(&instruments),
            instrument_sample_amp_velocity_sensitivity_pct:
                instrument_sample_amp_velocity_sensitivity_pct(&instruments),
            instrument_sample_velocity_levels_enabled: instrument_sample_velocity_levels_enabled(
                &instruments,
            ),
            instrument_sample_velocity_high: instrument_sample_velocity_high(&instruments),
            instrument_sample_velocity_medium: instrument_sample_velocity_medium(&instruments),
            instrument_sample_velocity_low: instrument_sample_velocity_low(&instruments),
            instrument_sample_amp_envs: instrument_sample_amp_envs(&instruments),
            instrument_sample_filters: instrument_sample_filters(&instruments),
            instrument_sample_filter_envs: instrument_sample_filter_envs(&instruments),
            instrument_midi_enabled: instrument_midi_enabled(&instruments),
            instrument_midi_channels: instrument_midi_channels(&instruments),
            instrument_midi_velocity: instrument_midi_velocity(&instruments),
            instrument_midi_duration_ms: instrument_midi_duration_ms(&instruments),
            fx_buses: fx_bus_configs(&fx_buses),
            global_fx_slots: global_fx_slots.clone(),
            global_fx_params: global_fx_params.clone(),
            sample_browser: None,
            sample_favourite_dirs: Vec::new(),
            sample_builtin_favourite_dirs: config.sample_builtin_favourite_dirs.clone(),
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            master_volume: ui.master_volume,
            note_length_ms: config.global_sound.note_length_ms as u16,
            velocity_scale_pct: config.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(config.global_sound.velocity_curve).into(),
            voice_stealing_mode: "auto-balanced".into(),
            auto_save_default: false,
            ghost_cells: ui.ghost_cells,
            input_events_while_paused: true,
            numeric_display_mode: ui.numeric_display_mode.clone(),
            screen_sleep_seconds: ui.screen_sleep_seconds,
            grid_brightness: ui.grid_brightness,
            display_brightness: ui.display_brightness,
            button_brightness: ui.button_brightness,
            midi_enabled: false,
            midi_clock_out_enabled: false,
            midi_clock_in_enabled: false,
            midi_respond_to_start_stop: true,
            preset_names: Vec::new(),
            preset_draft_name: fresh_preset_name(),
            preset_rename_source: None,
            midi_outputs: Vec::new(),
            midi_inputs: Vec::new(),
            dance_mode: "mix".into(),
            dance_fx_type: "none".into(),
            dance_fx_target: "master".into(),
            dance_fx_params: serde_json::Map::new(),
            xy_release: "sample-hold".into(),
            xy_invert_x: false,
            xy_invert_y: false,
            bpm: config.bpm.round().clamp(20.0, 300.0) as u16,
            swing_pct: config.swing_pct.min(75),
            audio_output_buffer_frames: config.audio_output_buffer_frames,
            sync_source: config.sync_source.clone(),
        });
        let mut part_engines = Vec::new();
        part_engines.resize_with(PART_COUNT, || None);
        for (index, slot) in part_engines.iter_mut().enumerate().skip(1) {
            let part_behavior = platform_core::get_native_behavior(config.behavior_id.as_str())
                .ok_or_else(|| format!("unsupported native behavior `{}`", config.behavior_id))?;
            *slot = Some(Self::build_engine(
                part_behavior,
                config.behavior_config.clone(),
                config.interpretation_profile.clone(),
                config.mapping_config.clone(),
                config.global_sound.clone(),
                config.note_behaviors.clone(),
                index,
            )?);
        }
        let mut runner = Self {
            engine,
            part_engines,
            behavior,
            behavior_config: config.behavior_config.clone(),
            behavior_configs: BTreeMap::from([(
                behavior.id().to_string(),
                config.behavior_config.clone(),
            )]),
            part_behavior_configs: vec![config.behavior_config; PART_COUNT],
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config.clone(),
            base_mapping_config: config.mapping_config,
            global_sound: config.global_sound,
            note_behaviors: config.note_behaviors,
            current_ppqn_pulse: 0,
            swung_ppqn_pulse: 0,
            tick: 0,
            part_ticks: vec![0; PART_COUNT],
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            algorithm_pulse_accumulator: 0,
            part_algorithm_step_pulses: vec![DEFAULT_ALGORITHM_STEP_PULSES; PART_COUNT],
            part_pulse_accumulators: vec![0; PART_COUNT],
            transport: RuntimeTransportState::Stopped,
            sync_source: config.sync_source,
            pending_resync: false,
            bpm: config.bpm,
            swing_pct: config.swing_pct.min(75),
            audio_output_buffer_frames: normalize_audio_output_buffer_frames(
                config.audio_output_buffer_frames,
            ),
            ui,
            oled_mode: NativeOledMode::Splash,
            oled_splash_text: OLED_STARTUP_SPLASH_KEY.into(),
            oled_splash_until: Some(now + Duration::from_millis(OLED_STARTUP_SPLASH_MS)),
            startup_splash_presented: false,
            last_interaction_at: now,
            fn_hold_started_at: None,
            modifier_hint_started_at: None,
            midi_enabled: false,
            preset_names: Vec::new(),
            current_preset_name: None,
            preset_draft_name: fresh_preset_name(),
            preset_rename_source: None,
            outbox: NativeRunnerOutbox::default(),
            midi_outputs: Vec::new(),
            midi_inputs: Vec::new(),
            midi_status: None,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            input_events_while_paused: true,
            voice_stealing_mode: "auto-balanced".into(),
            midi_clock_out_enabled: false,
            midi_clock_in_enabled: false,
            midi_respond_to_start_stop: true,
            dance_mode: "mix".into(),
            active_dance_mode: "none".into(),
            dance_fx_selected: default_dance_fx_selected(),
            dance_fx_assign: None,
            dance_fx_assignments: vec![],
            active_dance_fx: Vec::new(),
            xy_touch: NativeXyTouch {
                x: 0.5,
                y: 0.5,
                display_x: 0.5,
                display_y: 0.5,
                active: false,
            },
            xy_release: "sample-hold".into(),
            xy_invert_x: false,
            xy_invert_y: false,
            xy_x_binding: None,
            xy_y_binding: None,
            aux_auto_map_enabled: true,
            param_mods: vec![NativeParamMods::default(); PART_COUNT],
            trigger_gate_modes: vec!["full".into(); PART_COUNT],
            trigger_gate_restore_modes: vec![None; PART_COUNT],
            trigger_probability_assign: None,
            trigger_probability_maps: vec![
                vec!["full".into(); GRID_WIDTH * GRID_HEIGHT];
                PART_COUNT
            ],
            part_behavior_ids: vec![behavior.id().into(); PART_COUNT],
            part_names: vec![behavior.id().into(); PART_COUNT],
            part_auto_names: vec![true; PART_COUNT],
            save_grid_states: vec![true; PART_COUNT],
            sense_parts,
            aux_bindings: vec![None; platform_core::AUX_ENCODER_COUNT],
            active_part_index: 0,
            instruments,
            sample_assign: None,
            fx_buses,
            global_fx_slots,
            global_fx_params,
            sample_browser: None,
            sample_builtin_favourite_dirs: config.sample_builtin_favourite_dirs,
            sample_favourite_dirs: Vec::new(),
            help_popup: None,
            confirm_dialog: None,
            menu,
            event_dot_on: false,
            event_dot_pulses_remaining: 0,
            transport_flash: "none",
            transport_flash_pulses_remaining: 0,
            auto_save_default: false,
            config_dirty: false,
            pending_autosave_payload_due_at: None,
            auto_save_flash_serial: 0,
            auto_save_flash_pulses_remaining: 0,
            audio_config_revision: 0,
            last_snapshot_audio_config_revision: None,
            suppress_snapshot_response: false,
            trigger_probability_rng: 0xC311_5A7E_2024_0001,
            toast: None,
            toast_expires_at: None,
            aux_turn_toast_cooldown_until: None,
            pending_aux_turn_toast: None,
            pending_menu_apply: None,
            pending_audio_output_buffer_reboot_prompt: false,
            menu_scroll_offset: 0,
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

    pub fn apply_runtime_config(&mut self, config: &RuntimeConfig) {
        self.sync_source = config.sync_source.clone();
        self.bpm = config.bpm;
    }
}
