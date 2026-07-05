use super::*;

pub(crate) fn config() -> NativeMenuConfig {
    NativeMenuConfig {
        behavior_id: "life".into(),
        behavior_ids: vec!["life".into(), "glider".into(), "none".into()],
        l1_items: vec![
            NativeMenuItem {
                label: "Behavior".into(),
                key: Some("behaviorId".into()),
                value: NativeMenuValue::Enum {
                    options: vec!["life".into(), "glider".into(), "none".into()],
                    selected: 0,
                },
                children: vec![],
            },
            NativeMenuItem {
                label: "Step Rate".into(),
                key: Some("algorithmStep".into()),
                value: NativeMenuValue::Enum {
                    options: vec!["1/16", "1/8", "1/4", "1/2", "1/1"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    selected: 1,
                },
                children: vec![],
            },
            NativeMenuItem {
                label: "Spawn Count".into(),
                key: Some("behavior.randomCellsPerTick".into()),
                value: NativeMenuValue::Number {
                    value: 12,
                    min: 0,
                    max: 20,
                    step: 1,
                },
                children: vec![],
            },
            NativeMenuItem {
                label: "Spawn Interval".into(),
                key: Some("behavior.randomTickInterval".into()),
                value: NativeMenuValue::Number {
                    value: 1,
                    min: 1,
                    max: 20,
                    step: 1,
                },
                children: vec![],
            },
            NativeMenuItem {
                label: "Spawn".into(),
                key: Some("behavior.spawn".into()),
                value: NativeMenuValue::Action(NativeMenuAction::BehaviorAction(
                    "spawnRandom".into(),
                )),
                children: vec![],
            },
            NativeMenuItem {
                label: "Reset".into(),
                key: Some("behavior.reset".into()),
                value: NativeMenuValue::Action(NativeMenuAction::ResetBehavior),
                children: vec![],
            },
        ],
        behavior_target_items: behavior_target_items(),
        part_labels: (0..PART_COUNT)
            .map(|index| format!("P{}: life", index + 1))
            .collect(),
        part_names: vec!["life".into(); PART_COUNT],
        part_auto_names: vec![true; PART_COUNT],
        sense_parts: vec![default_sense_part_config(); PART_COUNT],
        active_part_index: 0,
        param_mods: vec![NativeParamModsConfig::default(); PART_COUNT],
        xy_x_binding: None,
        xy_y_binding: None,
        aux_auto_map_enabled: true,
        aux_bindings: vec![NativeAuxBindingConfig::default(); AUX_ENCODER_COUNT],
        instrument_labels: vec!["I1: synth".into()],
        instrument_names: vec!["synth".into()],
        instrument_types: vec!["synth".into()],
        instrument_auto_names: vec![true],
        instrument_note_behaviors: vec!["oneshot".into()],
        instrument_routes: vec!["direct".into()],
        instrument_volumes: vec![100],
        instrument_pan_positions: vec![16],
        instrument_sample_slots: vec![0],
        instrument_synth_configs: vec![serde_json::json!({})],
        instrument_synth_osc1_waveforms: vec!["saw".into()],
        instrument_synth_osc2_waveforms: vec!["square".into()],
        instrument_synth_filter_types: vec!["lowpass".into()],
        instrument_synth_filter_cutoffs: vec![8000],
        instrument_synth_gain_pct: vec![80],
        instrument_synth_filter_resonance: vec![20],
        instrument_sample_tune_semis: vec![0],
        instrument_sample_gain_pct: vec![100],
        instrument_sample_base_velocity: vec![100],
        instrument_sample_amp_velocity_sensitivity_pct: vec![100],
        instrument_sample_velocity_levels_enabled: vec![false],
        instrument_sample_velocity_high: vec![120],
        instrument_sample_velocity_medium: vec![85],
        instrument_sample_velocity_low: vec![45],
        instrument_sample_amp_envs: vec![serde_json::json!({})],
        instrument_sample_filters: vec![serde_json::json!({})],
        instrument_sample_filter_envs: vec![serde_json::json!({})],
        instrument_midi_enabled: vec![false],
        instrument_midi_channels: vec![1],
        instrument_midi_velocity: vec![100],
        instrument_midi_duration_ms: vec![120],
        fx_buses: vec![default_fx_bus_config(); FX_BUS_COUNT],
        global_fx_slots: vec!["none".into(); GLOBAL_FX_SLOT_COUNT],
        global_fx_params: vec![serde_json::json!({}); GLOBAL_FX_SLOT_COUNT],
        sample_browser: None,
        sample_favourite_dirs: vec![],
        sample_builtin_favourite_dirs: vec![],
        algorithm_step_pulses: 12,
        master_volume: 100,
        note_length_ms: 150,
        velocity_scale_pct: 100,
        velocity_curve: "linear".into(),
        voice_stealing_mode: "auto-balanced".into(),
        audio_output_buffer_frames: 256,
        auto_save_default: true,
        ghost_cells: false,
        input_events_while_paused: true,
        numeric_display_mode: "bar+numbers".into(),
        screen_sleep_seconds: 60,
        grid_brightness: 25,
        display_brightness: 75,
        button_brightness: 35,
        midi_enabled: false,
        midi_clock_out_enabled: false,
        midi_clock_in_enabled: false,
        midi_respond_to_start_stop: true,
        preset_names: vec![],
        preset_draft_name: "New Preset".into(),
        preset_rename_source: None,
        midi_outputs: vec![],
        midi_inputs: vec![],
        dance_mode: "mix".into(),
        dance_fx_type: "none".into(),
        dance_fx_target: "master".into(),
        dance_fx_params: serde_json::Map::new(),
        xy_release: "sample-hold".into(),
        xy_invert_x: false,
        xy_invert_y: false,
        bpm: 120,
        swing_pct: 0,
        sync_source: SyncSource::Internal,
    }
}

pub(crate) fn behavior_target_items() -> Vec<Vec<NativeMenuItem>> {
    (0..PART_COUNT)
        .map(|part_index| {
            vec![
                NativeMenuItem {
                    label: "Step Rate".into(),
                    key: Some(format!("parts.{part_index}.algorithmStep")),
                    value: NativeMenuValue::Enum {
                        options: vec!["1/16", "1/8", "1/4", "1/2", "1/1"]
                            .into_iter()
                            .map(String::from)
                            .collect(),
                        selected: 1,
                    },
                    children: vec![],
                },
                NativeMenuItem {
                    label: "Spawn Count".into(),
                    key: Some(format!(
                        "parts.{part_index}.l1.behaviorConfig.randomCellsPerTick"
                    )),
                    value: NativeMenuValue::Number {
                        value: 12,
                        min: 0,
                        max: 20,
                        step: 1,
                    },
                    children: vec![],
                },
                NativeMenuItem {
                    label: "Spawn Interval".into(),
                    key: Some(format!(
                        "parts.{part_index}.l1.behaviorConfig.randomTickInterval"
                    )),
                    value: NativeMenuValue::Number {
                        value: 1,
                        min: 1,
                        max: 20,
                        step: 1,
                    },
                    children: vec![],
                },
            ]
        })
        .collect()
}
