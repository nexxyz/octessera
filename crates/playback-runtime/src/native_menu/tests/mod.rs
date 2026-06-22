use super::*;
use platform_core::AUX_ENCODER_COUNT;
use platform_core::PART_COUNT;

mod dance;
mod help;
mod root;
mod sense;
mod voice;

fn config() -> NativeMenuConfig {
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
        algorithm_step_pulses: 12,
        master_volume: 100,
        note_length_ms: 150,
        velocity_scale_pct: 100,
        velocity_curve: "linear".into(),
        voice_stealing_mode: "balanced".into(),
        auto_save_default: true,
        ghost_cells: false,
        input_events_while_paused: true,
        numeric_display_mode: "bar+numbers".into(),
        screen_sleep_seconds: 60,
        grid_brightness: 75,
        display_brightness: 75,
        button_brightness: 75,
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
        sync_source: SyncSource::Internal,
    }
}

#[test]
fn fx_bus_and_global_fx_option_sets_do_not_overlap_except_none() {
    for fx_type in FX_BUS_SLOT_OPTIONS {
        if *fx_type == "none" {
            continue;
        }
        assert!(is_valid_fx_bus_slot_type(fx_type));
        assert!(
            !is_valid_global_fx_slot_type(fx_type),
            "{fx_type} should not be global"
        );
    }
    for fx_type in GLOBAL_FX_SLOT_OPTIONS {
        if *fx_type == "none" {
            continue;
        }
        assert!(is_valid_global_fx_slot_type(fx_type));
        assert!(
            !is_valid_fx_bus_slot_type(fx_type),
            "{fx_type} should not be a bus FX"
        );
    }
}

#[test]
fn native_menu_snapshot_rows_fit_oled_width() {
    for config in representative_help_configs() {
        let mut menu = NativeMenuModel::new(config);
        for cursor in 0..menu.root.children.len() {
            menu.state.stack.clear();
            menu.state.cursor = cursor;
            for line in menu.snapshot().lines {
                assert!(line.chars().count() <= 20, "row too wide: {line:?}");
            }
        }
    }
}

fn representative_help_configs() -> Vec<NativeMenuConfig> {
    let mut configs = vec![config()];

    let mut dynamic = config();
    dynamic.preset_names = vec!["One".into()];
    dynamic.preset_rename_source = Some("One".into());
    dynamic.midi_outputs = vec![("0".into(), "Out".into())];
    dynamic.midi_inputs = vec![("0".into(), "In".into())];
    dynamic.instrument_labels = vec!["I1: sampler".into()];
    dynamic.instrument_names = vec!["sampler".into()];
    dynamic.instrument_types = vec!["sampler".into()];
    dynamic.sample_browser = Some(NativeSampleBrowserConfig {
        instrument_slot: 0,
        sample_slot: 0,
        dir: String::new(),
        entries: vec![
            NativeSampleEntryConfig {
                name: "Drums".into(),
                path: "Drums".into(),
                is_dir: true,
            },
            NativeSampleEntryConfig {
                name: "kick.wav".into(),
                path: "kick.wav".into(),
                is_dir: false,
            },
        ],
    });
    configs.push(dynamic);

    let mut scanning = config();
    scanning.sense_parts[0].scan_mode = "scanning".into();
    scanning.sense_parts[0].x_velocity.enabled = true;
    scanning.sense_parts[0].x_filter_cutoff.enabled = true;
    scanning.sense_parts[0].x_filter_resonance.enabled = true;
    scanning.sense_parts[0].y_pitch_enabled = true;
    scanning.sense_parts[0].y_velocity.enabled = true;
    scanning.sense_parts[0].y_filter_cutoff.enabled = true;
    scanning.sense_parts[0].y_filter_resonance.enabled = true;
    configs.push(scanning);

    for instrument_type in ["none", "synth", "sampler", "midi"] {
        let mut cfg = config();
        cfg.instrument_types[0] = instrument_type.into();
        cfg.instrument_sample_velocity_levels_enabled[0] = true;
        configs.push(cfg);
    }

    for fx_type in FX_BUS_SLOT_OPTIONS {
        let mut cfg = config();
        cfg.fx_buses[0].slot1_type = (*fx_type).into();
        cfg.fx_buses[0].slot1_params = serde_json::json!({});
        configs.push(cfg);
    }

    for fx_type in GLOBAL_FX_SLOT_OPTIONS {
        let mut cfg = config();
        cfg.global_fx_slots[0] = (*fx_type).into();
        cfg.global_fx_params[0] = serde_json::json!({});
        configs.push(cfg);
    }

    for dance_mode in ["mix", "pan", "fx", "trigger-gate", "xy"] {
        let mut cfg = config();
        cfg.dance_mode = dance_mode.into();
        cfg.dance_fx_type = "stutter".into();
        cfg.dance_fx_params
            .insert("rateHz".into(), serde_json::json!(8));
        configs.push(cfg);
    }

    for fx_type in ["stutter", "freeze", "filter_sweep", "pitch_shift"] {
        let mut cfg = config();
        cfg.dance_mode = "fx".into();
        cfg.dance_fx_type = fx_type.into();
        configs.push(cfg);
    }

    configs
}

fn contains_set_binding(item: &NativeMenuItem, target: &str, key: &str) -> bool {
    if let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { target: t, binding }) =
        &item.value
    {
        if t == target && binding.key == key {
            return true;
        }
    }
    item.children
        .iter()
        .any(|child| contains_set_binding(child, target, key))
}

fn contains_aux_click_action(item: &NativeMenuItem, index: usize, action_key: &str) -> bool {
    if let NativeMenuValue::Action(NativeMenuAction::SetAuxClick {
        index: action_index,
        action: Some(action),
    }) = &item.value
    {
        if *action_index == index {
            if let NativeMenuAction::PlatformEffect(effect) = action.as_ref() {
                if effect == action_key {
                    return true;
                }
            }
        }
    }
    item.children
        .iter()
        .any(|child| contains_aux_click_action(child, index, action_key))
}

fn find_item_by_key<'a>(item: &'a NativeMenuItem, key: &str) -> Option<&'a NativeMenuItem> {
    if item.key.as_deref() == Some(key) {
        return Some(item);
    }
    item.children
        .iter()
        .find_map(|child| find_item_by_key(child, key))
}

#[test]
fn sample_browser_label_includes_selected_slot_context_without_body_rows() {
    let mut cfg = config();
    cfg.instrument_types[0] = "sampler".into();
    cfg.instrument_sample_slots[0] = 2;
    cfg.sample_browser = Some(NativeSampleBrowserConfig {
        instrument_slot: 0,
        sample_slot: 2,
        dir: String::new(),
        entries: vec![NativeSampleEntryConfig {
            name: "Long Folder Name".into(),
            path: "Long Folder Name".into(),
            is_dir: true,
        }],
    });

    let root = build_root(cfg);
    let browser = find_item_by_key(&root, "sample.choose:0:2").unwrap();

    assert_eq!(browser.label, "S3 Browse");
    assert_eq!(browser.children[0].label, "..");
    assert_eq!(browser.children[1].label, "[Long Folder Name]");
}

fn contains_aux_click_reset(item: &NativeMenuItem, index: usize) -> bool {
    if let NativeMenuValue::Action(NativeMenuAction::SetAuxClick {
        index: action_index,
        action: Some(action),
    }) = &item.value
    {
        if *action_index == index && matches!(action.as_ref(), NativeMenuAction::ResetBehavior) {
            return true;
        }
    }
    item.children
        .iter()
        .any(|child| contains_aux_click_reset(child, index))
}
