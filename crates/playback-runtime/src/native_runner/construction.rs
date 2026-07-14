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
        let pulses_layers = default_pulses_layers();
        let fx_buses = default_fx_buses();
        let global_fx_slots = default_global_fx_slots();
        let global_fx_params = default_global_fx_params();
        let menu = NativeMenuModel::new(NativeMenuConfig {
            behavior_id: behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            worlds_items: vec![],
            worlds_items_by_layer: vec![vec![]; LAYER_COUNT],
            behavior_target_items: vec![vec![]; LAYER_COUNT],
            layer_labels: (0..LAYER_COUNT)
                .map(|index| format!("L{}: life", index + 1))
                .collect(),
            layer_names: vec![behavior.id().into(); LAYER_COUNT],
            layer_auto_names: vec![true; LAYER_COUNT],
            pulses_layers: pulses_layer_configs(&pulses_layers),
            active_layer_index: 0,
            param_mods: vec![NativeParamModsConfig::default(); LAYER_COUNT],
            xy_x_binding: None,
            xy_y_binding: None,
            aux_auto_map_enabled: true,
            aux_bindings: vec![NativeAuxBindingConfig::default(); platform_core::AUX_ENCODER_COUNT],
            shift_aux_bindings: vec![
                NativeAuxBindingConfig::default();
                platform_core::AUX_ENCODER_COUNT
            ],
            instrument_labels: instrument_labels(&instruments),
            instrument_names: instrument_names(&instruments),
            instrument_types: instrument_types(&instruments),
            instrument_auto_names: instrument_auto_names(&instruments),
            instrument_note_behaviors: instrument_note_behaviors(&instruments),
            instrument_routes: instrument_routes(&instruments),
            instrument_volumes: instrument_volumes(&instruments),
            instrument_pan_positions: instrument_pan_positions(&instruments),
            instrument_sample_slots: instrument_sample_slots(&instruments),
            instrument_sample_paths: instrument_sample_paths(&instruments),
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
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_RED,
            master_volume: ui.master_volume,
            note_length_ms: config.global_sound.note_length_ms as u16,
            velocity_scale_pct: config.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(config.global_sound.velocity_curve).into(),
            voice_stealing_mode: "auto-balanced".into(),
            auto_save_default: false,
            rolling_backups: true,
            ghost_cells: ui.ghost_cells,
            input_events_while_paused: true,
            numeric_display_mode: ui.numeric_display_mode.clone(),
            screen_sleep_seconds: ui.screen_sleep_seconds,
            dim_timer_seconds: ui.dim_timer_seconds,
            grid_brightness: ui.grid_brightness,
            display_brightness: ui.display_brightness,
            button_brightness: ui.button_brightness,
            midi_enabled: false,
            midi_clock_out_enabled: false,
            midi_clock_in_enabled: false,
            midi_respond_to_start_stop: true,
            usb_audio_out: "jack".into(),
            usb_midi_out_enabled: false,
            recording_max_minutes: 10,
            preset_names: Vec::new(),
            preset_draft_name: fresh_preset_name(),
            preset_rename_source: None,
            midi_outputs: Vec::new(),
            midi_inputs: Vec::new(),
            sparks_mode: "mix".into(),
            sparks_fx_type: "none".into(),
            sparks_fx_target: "master".into(),
            sparks_fx_params: serde_json::Map::new(),
            xy_release: "sample-hold".into(),
            xy_invert_x: false,
            xy_invert_y: false,
            bpm: config.bpm.round().clamp(20.0, 300.0) as u16,
            swing_pct: config.swing_pct.min(75),
            audio_output_buffer_frames: config.audio_output_buffer_frames,
            sync_source: config.sync_source.clone(),
        });
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
        let mut runner = Self {
            engine,
            layer_engines,
            behavior,
            behavior_config: config.behavior_config.clone(),
            behavior_configs: BTreeMap::from([(
                behavior.id().to_string(),
                config.behavior_config.clone(),
            )]),
            layer_behavior_configs: vec![config.behavior_config; LAYER_COUNT],
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config.clone(),
            base_mapping_config: config.mapping_config,
            global_sound: config.global_sound,
            note_behaviors: config.note_behaviors,
            current_ppqn_pulse: 0,
            swung_ppqn_pulse: 0,
            tick: 0,
            layer_ticks: vec![0; LAYER_COUNT],
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_RED,
            algorithm_pulse_accumulator: 0,
            layer_algorithm_step_pulses: vec![DEFAULT_ALGORITHM_STEP_RED; LAYER_COUNT],
            layer_pulse_accumulators: vec![0; LAYER_COUNT],
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
            usb_audio_out: "jack".into(),
            usb_midi_out_enabled: false,
            recording_max_minutes: 10,
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
            sparks_mode: "mix".into(),
            active_sparks_mode: "none".into(),
            sparks_fx_selected: default_sparks_fx_selected(),
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
            xy_release: "sample-hold".into(),
            xy_invert_x: false,
            xy_invert_y: false,
            xy_x_binding: None,
            xy_y_binding: None,
            aux_auto_map_enabled: true,
            param_mods: vec![NativeParamMods::default(); LAYER_COUNT],
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
            layer_behavior_ids: vec![behavior.id().into(); LAYER_COUNT],
            layer_names: vec![behavior.id().into(); LAYER_COUNT],
            layer_auto_names: vec![true; LAYER_COUNT],
            save_grid_states: vec![true; LAYER_COUNT],
            pulses_layers,
            aux_bindings: vec![None; platform_core::AUX_ENCODER_COUNT],
            shift_aux_bindings: vec![None; platform_core::AUX_ENCODER_COUNT],
            active_layer_index: 0,
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
            usb_sd_transfer_modal: None,
            menu,
            event_dot_on: false,
            event_dot_pulses_remaining: 0,
            transport_flash: "none",
            transport_flash_pulses_remaining: 0,
            auto_save_default: false,
            rolling_backups: true,
            config_dirty: false,
            pending_autosave_payload_due_at: None,
            last_backup_save_at: None,
            auto_save_flash_serial: 0,
            auto_save_flash_until: None,
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
