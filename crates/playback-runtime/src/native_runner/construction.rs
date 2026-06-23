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
            voice_stealing_mode: "balanced".into(),
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
            tick: 0,
            algorithm_step_pulses: DEFAULT_ALGORITHM_STEP_PULSES,
            algorithm_pulse_accumulator: 0,
            part_algorithm_step_pulses: vec![DEFAULT_ALGORITHM_STEP_PULSES; PART_COUNT],
            part_pulse_accumulators: vec![0; PART_COUNT],
            transport: RuntimeTransportState::Stopped,
            sync_source: config.sync_source,
            pending_resync: false,
            bpm: config.bpm,
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
            queued_platform_effects: Vec::new(),
            midi_outputs: Vec::new(),
            midi_inputs: Vec::new(),
            midi_status: None,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            input_events_while_paused: true,
            voice_stealing_mode: "balanced".into(),
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
            auto_save_flash_serial: 0,
            auto_save_flash_pulses_remaining: 0,
            audio_config_revision: 0,
            last_snapshot_audio_config_revision: None,
            trigger_probability_rng: 0xC311_5A7E_2024_0001,
            toast: None,
            toast_expires_at: None,
            aux_turn_toast_cooldown_until: None,
            pending_aux_turn_toast: None,
            pending_menu_apply: None,
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
        self.menu.rebuild(self.menu_config());
    }

    pub fn flush_deferred_menu_apply(
        &mut self,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        self.flush_deferred_menu_apply_at(Instant::now())
    }

    pub(super) fn schedule_deferred_menu_apply(&mut self, key: &str) {
        self.pending_menu_apply = Some(PendingMenuApply {
            due_at: Instant::now() + Duration::from_millis(DEFERRED_MENU_APPLY_MS),
            key: key.into(),
        });
    }

    pub(super) fn clear_deferred_menu_apply(&mut self) {
        self.pending_menu_apply = None;
    }

    pub(super) fn flush_deferred_menu_apply_at(
        &mut self,
        now: Instant,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        let Some(pending) = &self.pending_menu_apply else {
            return Ok(Vec::new());
        };
        if pending.due_at > now {
            return Ok(Vec::new());
        }
        let _ = pending.key.as_str();
        self.pending_menu_apply = None;
        self.apply_menu_state()?;
        self.messages_with_snapshot()
    }

    #[cfg(test)]
    pub(super) fn make_deferred_menu_apply_due_for_test(&mut self) {
        if let Some(pending) = &mut self.pending_menu_apply {
            pending.due_at = Instant::now();
        }
    }

    #[cfg(test)]
    pub(super) fn set_toast_for_test(&mut self, message: &str) {
        self.show_toast(message);
    }

    #[cfg(test)]
    pub(super) fn advance_toast_for_test(&mut self) {
        if let Some(toast) = &mut self.toast {
            toast.offset = toast.offset.saturating_add(1);
        }
    }

    #[cfg(test)]
    pub(super) fn age_toast_state_for_test(&mut self, millis: u64) {
        let delta = Duration::from_millis(millis);
        if let Some(expires_at) = &mut self.toast_expires_at {
            *expires_at -= delta;
        }
        if let Some(cooldown_until) = &mut self.aux_turn_toast_cooldown_until {
            *cooldown_until -= delta;
        }
    }

    pub(super) fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(NativeToast {
            message: message.into(),
            offset: 0,
        });
        self.toast_expires_at = Some(Instant::now() + Duration::from_millis(1800));
    }

    pub(super) fn show_or_queue_aux_turn_toast(&mut self, message: impl Into<String>) {
        let message = message.into();
        let now = Instant::now();
        if self
            .aux_turn_toast_cooldown_until
            .is_some_and(|cooldown_until| now < cooldown_until)
        {
            self.pending_aux_turn_toast = Some(PendingNativeToast { message });
            return;
        }
        self.toast = Some(NativeToast { message, offset: 0 });
        self.toast_expires_at = Some(now + Duration::from_millis(1200));
        self.aux_turn_toast_cooldown_until = Some(now + Duration::from_millis(500));
        self.pending_aux_turn_toast = None;
    }

    pub(super) fn build_engine(
        behavior: NativeBehavior,
        behavior_config: Value,
        interpretation_profile: InterpretationProfile,
        mapping_config: platform_core::MappingConfig,
        global_sound: GlobalSoundConfig,
        note_behaviors: Vec<NoteBehavior>,
        part_index: usize,
    ) -> Result<NativePartEngine, String> {
        NativePartEngine::new(NativePartEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile,
            mapping_config,
            global_sound,
            note_behaviors,
            part_index,
        })
    }

    pub(super) fn rebuild_engine(&mut self, behavior: NativeBehavior) -> Result<(), String> {
        self.engine = Self::build_engine(
            behavior,
            self.behavior_config.clone(),
            self.interpretation_profile.clone(),
            self.mapping_config.clone(),
            self.global_sound.clone(),
            self.note_behaviors.clone(),
            self.active_part_index,
        )?;
        self.behavior = behavior;
        self.reset_transport_position();
        self.menu.rebuild(self.menu_config());
        Ok(())
    }

    pub(super) fn reset_transport_position(&mut self) {
        self.tick = 0;
        self.current_ppqn_pulse = 0;
        self.algorithm_pulse_accumulator = 0;
        self.transport_flash = "none";
        self.transport_flash_pulses_remaining = 0;
        self.event_dot_on = false;
        self.event_dot_pulses_remaining = 0;
        self.engine.reset_transport_phase();
        for engine in self.part_engines.iter_mut().flatten() {
            engine.reset_transport_phase();
        }
        for accumulator in &mut self.part_pulse_accumulators {
            *accumulator = 0;
        }
    }

    pub(super) fn sync_engine_runtime_config(&mut self) {
        self.note_behaviors = note_behaviors_from_instruments(&self.instruments);
        self.engine.set_global_sound(self.global_sound.clone());
        self.engine.set_note_behaviors(self.note_behaviors.clone());
        for engine in self.part_engines.iter_mut().flatten() {
            engine.set_global_sound(self.global_sound.clone());
            engine.set_note_behaviors(self.note_behaviors.clone());
        }
    }

    pub(super) fn record_display_interaction(&mut self) -> bool {
        let now = Instant::now();
        self.last_interaction_at = now;
        if self.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
            return false;
        }
        if self.oled_mode == NativeOledMode::Off {
            self.oled_mode = NativeOledMode::Normal;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            return true;
        }
        if self.oled_mode == NativeOledMode::Splash {
            self.oled_mode = NativeOledMode::Normal;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            return true;
        }
        false
    }

    pub(super) fn advance_oled_sleep_state(&mut self) {
        let now = Instant::now();
        if self.oled_mode == NativeOledMode::Splash
            && self
                .oled_splash_until
                .is_some_and(|deadline| now >= deadline)
        {
            if self.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
                self.oled_mode = NativeOledMode::Normal;
                self.oled_splash_text.clear();
                self.oled_splash_until = None;
                self.show_toast("Help: Sh+Fn+Enter");
                return;
            }
            if self.ui.screen_sleep_seconds == 0 {
                self.oled_mode = NativeOledMode::Normal;
                self.oled_splash_text.clear();
                self.oled_splash_until = None;
                return;
            }
            self.oled_mode = NativeOledMode::Off;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            return;
        }
        if self.ui.screen_sleep_seconds == 0 {
            if self.oled_mode == NativeOledMode::Off {
                self.oled_mode = NativeOledMode::Normal;
            }
            return;
        }
        if self.oled_mode == NativeOledMode::Normal
            && now.duration_since(self.last_interaction_at)
                >= Duration::from_secs(u64::from(self.ui.screen_sleep_seconds))
        {
            self.oled_mode = NativeOledMode::Splash;
            self.oled_splash_text = OLED_SLEEP_SPLASH_KEY.into();
            self.oled_splash_until = Some(now + Duration::from_millis(OLED_SLEEP_SPLASH_MS));
        }
    }
}
