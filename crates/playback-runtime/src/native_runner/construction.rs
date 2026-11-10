use super::construction_seed::NativeRunnerConstructionSeed;
use super::*;
use std::sync::Arc;

impl NativeRunner {
    pub fn new(config: NativeRunnerConfig) -> Result<Self, String> {
        let mut runner = Self::new_with_unbuilt_menu(config)?;
        runner.menu.rebuild(runner.menu_config());
        Ok(runner)
    }

    /// Builds the menu after the startup System and Patch loads have been applied.
    pub fn finish_startup_load(&mut self) {
        if !self.menu.is_built() {
            self.menu.rebuild(self.menu_config());
        }
    }

    /// Startup and configuration transactions build the menu once, after loading.
    pub fn new_with_unbuilt_menu(config: NativeRunnerConfig) -> Result<Self, String> {
        if !config
            .audio_optimization
            .is_supported(config.audio_optimization_capacity_available)
        {
            return Err("audio optimization capacity is unavailable".into());
        }
        let behavior = platform_core::get_native_behavior(&config.behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{}`", config.behavior_id))?;
        let ui = NativeUiState::default();
        let now = Instant::now();
        let seed = NativeRunnerConstructionSeed::new(&config, ui, now);
        let mut layers = Vec::with_capacity(LAYER_COUNT);
        for index in 0..LAYER_COUNT {
            let engine = Self::build_engine(
                behavior,
                config.behavior_config.clone(),
                config.interpretation_profile.clone(),
                config.mapping_config.clone(),
                config.global_sound.clone(),
                config.note_behaviors.clone(),
                index,
            )?;
            layers.push(NativeLayer::new(
                index,
                behavior.id(),
                config.behavior_config.clone(),
                engine,
            ));
        }
        let mut runner = Self {
            base_interpretation_profile: config.interpretation_profile,
            base_mapping_config: config.mapping_config,
            global_sound: seed.global_sound,
            dsp_config: seed.dsp_config,
            audio_io: seed.audio_io,
            note_behaviors: config.note_behaviors,
            transport: NativeTransportState::new(
                seed.bpm,
                (seed.swing_a, seed.swing_b),
                seed.sync_source.clone(),
                seed.algorithm_step_pulses,
            ),
            delayed_link_events: vec![Vec::new(); LAYER_COUNT],
            link_arp: Default::default(),
            display: NativeDisplayState::new(seed.ui, seed.now, seed.hdmi),
            midi: Default::default(),
            patch_store: patch_store::NativePatchStore::new(seed.preset_names),
            outbox: NativeRunnerOutbox::default(),
            midi_outputs: seed.midi_outputs,
            midi_inputs: seed.midi_inputs,
            midi_status: None,
            bluetooth: Default::default(),
            wifi_enabled: true,
            input_events_while_paused: seed.input_events_while_paused,
            voice_stealing_mode: seed.voice_stealing_mode,
            play_mode: seed.play_mode,
            active_play_mode: ActivePlayMode::None,
            drum_selected_voices: vec![0; INSTRUMENT_COUNT],
            play_fx_selected: seed.play_fx_selected,
            grid_assign: None,
            play_fx_assignments: vec![],
            active_play_fx: Vec::new(),
            xy: NativeXyState {
                touch: NativeXyTouch::default(),
                release: seed.xy_release,
                smoothing_ms: seed.xy_smoothing_ms,
                invert_x: seed.xy_invert_x,
                invert_y: seed.xy_invert_y,
                x_glide: None,
                y_glide: None,
                x_binding: seed.xy_x_binding,
                y_binding: seed.xy_y_binding,
            },
            aux_auto_map_enabled: seed.aux_auto_map_enabled,
            aux_priority: seed.aux_priority,
            play_live: Default::default(),
            transpose: Default::default(),
            layers,
            random_seed: super::random_streams::DEFAULT_RANDOM_SEED,
            seeded_link_streams: super::random_streams::SeededLinkStreams::new(
                super::random_streams::DEFAULT_RANDOM_SEED,
            ),
            link_lfos: seed.link_lfos,
            midi_cc_mappings: super::midi_cc_in::default_midi_cc_mappings(),
            midi_cc_learn: None,
            faders: Default::default(),
            modulation_process: ModulationProcessState::default(),
            aux_bindings: seed.aux_bindings,
            shift_aux_bindings: seed.shift_aux_bindings,
            active_layer_index: seed.active_layer_index,
            instruments: seed.instruments,
            fx_buses: seed.fx_buses,
            global_fx_slots: seed.global_fx_slots,
            global_fx_params: seed.global_fx_params,
            sample_browser: None,
            sample_availability: seed.sample_availability,
            sample_builtin_favourite_dirs: seed.sample_builtin_favourite_dirs,
            sample_favourite_dirs: seed.sample_favourite_dirs,
            menu: NativeMenuModel::unbuilt(),
            auto_save_default: seed.auto_save_default,
            rolling_backups: seed.rolling_backups,
            config_dirty: false,
            config_revision: 0,
            dirty_revision: None,
            audio_revisions: Default::default(),
            trigger_probability_rng: TRIGGER_PROBABILITY_RNG_INITIAL_SEED,
            pending: NativePendingState::default(),
            restart_settings: RestartSettingsState::default(),
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
            active_link_refresh_calls: 0,
            #[cfg(any(test, feature = "test-support"))]
            test_snapshot_failure: Cell::new(false),
        };
        runner.seed_visible_state()?;
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
        let documents = super::split_system_patch_documents(&runner.config_payload())?;
        runner.restart_settings = RestartSettingsState::new(documents.system.clone());
        runner.pending.system_persistence.saved_baseline = Some(Arc::new(documents.system));
        Ok(runner)
    }
}
