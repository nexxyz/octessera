use super::*;

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
        part_labels: vec![
            "P1: life".into(),
            "P2: life".into(),
            "P3: life".into(),
            "P4: life".into(),
            "P5: life".into(),
            "P6: life".into(),
            "P7: life".into(),
            "P8: life".into(),
        ],
        part_names: vec!["life".into(); 8],
        part_auto_names: vec![true; 8],
        sense_parts: vec![default_sense_part_config(); 8],
        active_part_index: 0,
        param_mods: vec![NativeParamModsConfig::default(); 8],
        xy_x_binding: None,
        xy_y_binding: None,
        aux_bindings: vec![NativeAuxBindingConfig::default(); 4],
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
        dance_mode: "none".into(),
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
fn root_snapshot_includes_l4_separator_and_system() {
    let menu = NativeMenuModel::new(config());
    let snapshot = menu.snapshot();

    assert_eq!(snapshot.path, "MENU");
    assert_eq!(
        snapshot.lines,
        vec![
            "> L1: Life",
            "> L2: Sense",
            "> L3: Voice",
            "> L4: Dance",
            "",
            "> System",
        ]
    );
    assert_eq!(snapshot.selected_row, Some(0));
    assert_eq!(
        snapshot.colors,
        vec![0x8ED1, 0x8D5C, 0xC59B, 0xFFFF, 0xFFFF, 0xB50D]
    );
}

#[test]
fn rebuild_preserves_navigation_state() {
    let mut menu = NativeMenuModel::new(config());
    let _ = menu.press();
    let _ = menu.press();
    let _ = menu.press();
    menu.turn(1);

    let mut next = config();
    next.behavior_id = "glider".into();
    next.l1_items[0].value = NativeMenuValue::Enum {
        options: vec!["life".into(), "glider".into(), "none".into()],
        selected: 1,
    };
    menu.rebuild(next);

    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L1: Life/P1: life");
    assert_eq!(snapshot.selected_row, Some(0));
    assert!(menu.state.editing);
    assert_eq!(menu.selected_behavior().as_deref(), Some("glider"));
}

#[test]
fn navigation_skips_separator_rows_when_turning() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    menu.turn(1);
    menu.turn(1);
    assert_eq!(menu.snapshot().selected_row, Some(3));

    menu.turn(1);

    let snapshot = menu.snapshot();
    assert_eq!(snapshot.selected_row, Some(5));
    assert_eq!(snapshot.lines[5], "> System");
}

#[test]
fn system_submenu_uses_abbreviated_path_and_section_colors() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();

    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "SYS");
    assert_eq!(snapshot.lines, vec!["> Saves", "> Sound", "> MIDI", "> UI"]);
    assert_eq!(snapshot.colors, vec![0xB50D, 0xB50D, 0xB50D, 0xB50D]);
    assert_eq!(snapshot.selected_row, Some(0));
}

#[test]
fn entering_l1_selects_active_part_row() {
    let mut menu = NativeMenuModel::new(NativeMenuConfig {
        active_part_index: 2,
        ..config()
    });
    let _ = menu.press();
    menu.state.cursor = 2;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L1: Life");
    assert_eq!(snapshot.selected_row, Some(2));
    assert_eq!(snapshot.lines[2], "> P3: life");
}

#[test]
fn entering_l2_selects_active_part_row_after_aux_mappings_group() {
    let mut menu = NativeMenuModel::new(NativeMenuConfig {
        active_part_index: 2,
        ..config()
    });
    menu.turn(1);
    let _ = menu.press();
    menu.state.cursor = 3;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.selected_row, Some(3));
    assert_eq!(snapshot.lines[3], "> P3: life");
}

#[test]
fn l2_starts_with_aux_mappings_and_part_rows_are_enterable() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    let _ = menu.press();
    let snapshot = menu.snapshot();

    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.lines[0], "> Aux Mappings");
    assert_eq!(snapshot.lines[1], "> P1: life");

    menu.turn(1);
    let _ = menu.press();
    let part_snapshot = menu.snapshot();
    assert_eq!(part_snapshot.path, "L2: Sense/P1: life");
    assert!(part_snapshot.lines.iter().any(|line| line == "> Scanning"));
    assert!(part_snapshot.lines.iter().any(|line| line == "> Events"));
    assert!(part_snapshot
        .lines
        .iter()
        .any(|line| line == "> Note Mapping"));
}

#[test]
fn snapshot_scrolls_to_keep_selected_row_visible() {
    let mut menu = NativeMenuModel::new(config());
    let _ = menu.press();
    menu.state.cursor = 7;
    let snapshot = menu.snapshot();

    assert_eq!(snapshot.path, "L1: Life");
    assert_eq!(snapshot.selected_row, Some(6));
    assert_eq!(snapshot.lines[6], "> P8: life");
    assert!(!snapshot.lines.iter().any(|line| line == "> P1: life"));
}

#[test]
fn voice_instrument_rows_expose_configuration_groups() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    menu.turn(1);
    let _ = menu.press();
    let _ = menu.press();
    let _ = menu.press();
    let snapshot = menu.snapshot();

    assert_eq!(snapshot.path, "L3: Voice/Instruments/I1: synth");
    assert!(snapshot
        .lines
        .windows(2)
        .any(|pair| { pair[0] == "  Type:" && pair[1].trim() == "synth" }));
    assert!(snapshot.lines.iter().any(|line| line == "> Synth"));
    assert!(!snapshot.lines.iter().any(|line| line == "> Sampler"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Clone"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Reset"));
    assert!(snapshot.lines.iter().any(|line| line == "  Name"));
}

#[test]
fn voice_menu_exposes_fx_bus_and_global_fx_groups() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(2);
    let _ = menu.press();
    let snapshot = menu.snapshot();

    assert_eq!(snapshot.path, "L3: Voice");
    assert!(snapshot.lines.iter().any(|line| line == "> Instruments"));
    assert!(snapshot.lines.iter().any(|line| line == "> FX Buses"));
    assert!(snapshot.lines.iter().any(|line| line == "> Global FX"));

    menu.turn(1);
    let _ = menu.press();
    let buses = menu.snapshot();
    assert_eq!(buses.path, "L3: Voice/FX Buses");
    assert!(buses.lines.iter().any(|line| line == "> B1: (none)"));
    let _ = menu.press();
    let bus = menu.snapshot();
    assert!(bus.lines.iter().any(|line| line == "  Name"));
}

#[test]
fn root_matches_current_canonical_menu_without_playback_group() {
    let menu = NativeMenuModel::new(config());
    let labels = menu
        .root
        .children
        .iter()
        .filter(|item| !item.label.is_empty())
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        vec!["L1: Life", "L2: Sense", "L3: Voice", "L4: Dance", "System"]
    );
}

#[test]
fn l2_spec_rows_include_probability_mapping_and_axis_controls() {
    let menu = NativeMenuModel::new(config());
    let part = &menu.root.children[1].children[1];
    let trigger_prob = part
        .children
        .iter()
        .find(|item| item.label == "Trigger Prob.")
        .expect("trigger probability group");
    assert!(trigger_prob
        .children
        .iter()
        .any(|item| item.label == "Map Probability Grid"));

    let note_mapping = part
        .children
        .iter()
        .find(|item| item.label == "Note Mapping")
        .expect("note mapping group");
    let scale = note_mapping
        .children
        .iter()
        .find(|item| item.label == "Scale")
        .expect("scale row");
    assert!(matches!(
        &scale.value,
        NativeMenuValue::Enum { options, .. }
            if options.contains(&"harmonic_minor".to_string())
                && options.contains(&"major_pentatonic".to_string())
    ));
    assert!(note_mapping
        .children
        .iter()
        .any(|item| item.label == "Out of Range"));

    let x_axis = part
        .children
        .iter()
        .find(|item| item.label == "X Axis")
        .expect("x axis group");
    let labels = x_axis
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "Pitch Steps",
            "Velocity",
            "Filter Cutoff",
            "Filter Resonance"
        ]
    );
}

#[test]
fn conditional_rows_follow_scan_lane_and_sampler_state() {
    let menu = NativeMenuModel::new(config());
    let part = &menu.root.children[1].children[1];
    let scanning = part
        .children
        .iter()
        .find(|item| item.label == "Scanning")
        .expect("scanning group");
    assert_eq!(scanning.children.len(), 1);
    assert_eq!(scanning.children[0].label, "Scan Mode");
    let x_axis = part
        .children
        .iter()
        .find(|item| item.label == "X Axis")
        .expect("x axis group");
    let velocity = x_axis
        .children
        .iter()
        .find(|item| item.label == "Velocity")
        .expect("velocity group");
    assert_eq!(velocity.children.len(), 1);
    assert_eq!(velocity.children[0].label, "Enabled");

    let mut cfg = config();
    cfg.sense_parts[0].scan_mode = "scanning".into();
    cfg.sense_parts[0].x_velocity.enabled = true;
    cfg.instrument_types[0] = "sampler".into();
    cfg.instrument_sample_velocity_levels_enabled[0] = false;
    let menu = NativeMenuModel::new(cfg);
    let part = &menu.root.children[1].children[1];
    let scanning = part
        .children
        .iter()
        .find(|item| item.label == "Scanning")
        .expect("scanning group");
    assert!(scanning
        .children
        .iter()
        .any(|item| item.label == "Scan Axis"));
    let x_axis = part
        .children
        .iter()
        .find(|item| item.label == "X Axis")
        .expect("x axis group");
    let velocity = x_axis
        .children
        .iter()
        .find(|item| item.label == "Velocity")
        .expect("velocity group");
    assert!(velocity.children.iter().any(|item| item.label == "Curve"));
    let sampler = menu.root.children[2].children[0].children[0]
        .children
        .iter()
        .find(|item| item.label == "Sampler")
        .expect("sampler group");
    assert!(!sampler
        .children
        .iter()
        .any(|item| item.label == "Velocity Levels"
            && !matches!(item.value, NativeMenuValue::Bool { .. })));
}

#[test]
fn l4_spec_rows_show_only_selected_dance_page_controls() {
    let menu = NativeMenuModel::new(config());
    let dance = &menu.root.children[3];
    let labels = dance
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(labels, vec!["Dance Page", "BPM"]);
    let bpm = dance
        .children
        .iter()
        .find(|item| item.label == "BPM")
        .unwrap();
    assert!(matches!(
        bpm.value,
        NativeMenuValue::Number {
            min: 40,
            max: 240,
            step: 1,
            ..
        }
    ));

    let mut fx_config = config();
    fx_config.dance_mode = "fx".into();
    let fx_menu = NativeMenuModel::new(fx_config);
    let fx_labels = fx_menu.root.children[3]
        .children
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(fx_labels.contains(&"FX Type"));
    assert!(fx_labels.contains(&"Target"));
    assert!(!fx_labels.contains(&"Trigger Gate"));
}

#[test]
fn number_params_emit_bar_values_and_pan_uses_marker_format() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![2, 0, 0, 3];
    menu.state.cursor = 2;
    let pan = menu.snapshot();

    assert!(pan
        .lines
        .windows(2)
        .any(|pair| pair[0] == "  Pan Pos:" && pair[1] == "  C"));
    let pan_value_row = pan.selected_row.unwrap() + 1;
    assert!(matches!(
        pan.bar_values.get(pan_value_row),
        Some(Some(NativeMenuBarValue { style: Some(style), .. })) if style == "marker"
    ));

    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5, 1];
    menu.state.cursor = 0;
    let volume = menu.snapshot();
    assert!(volume
        .lines
        .windows(2)
        .any(|pair| pair[0] == "  Master Vol:"));
    assert!(matches!(
        volume.bar_values.get(volume.selected_row.unwrap() + 1),
        Some(Some(NativeMenuBarValue {
            frac_pct: 100,
            style: None,
            ..
        }))
    ));

    let mut numbers_config = config();
    numbers_config.numeric_display_mode = "numbers".into();
    let mut menu = NativeMenuModel::new(numbers_config);
    menu.state.stack = vec![5, 1];
    menu.state.cursor = 0;
    let numbers = menu.snapshot();
    assert!(numbers.bar_values.iter().all(Option::is_none));
}

#[test]
fn mixer_menu_uses_current_volume_and_pan_values() {
    let mut config = config();
    config.instrument_volumes = vec![70];
    config.instrument_pan_positions = vec![10];
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![2, 0, 0, 3];
    menu.state.cursor = 1;
    let _snapshot = menu.snapshot();
    let NativeMenuValue::Number { value, .. } = menu.current_siblings()[1].value else {
        panic!("volume should be number");
    };
    assert_eq!(value, 70);
    menu.state.cursor = 2;
    let pan = menu.snapshot();
    assert!(pan
        .lines
        .windows(2)
        .any(|pair| pair[0] == "  Pan Pos:" && pair[1] == "  L6"));
}

#[test]
fn scale_menu_uses_legacy_scale_ids_and_display_labels() {
    let mut config = config();
    config.sense_parts[0].scale = "major_pentatonic".into();
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![1, 1, 4];
    menu.state.cursor = 3;
    let scale = menu.current_siblings()[3].clone();
    let NativeMenuValue::Enum { options, .. } = scale.value else {
        panic!("scale should be enum");
    };
    assert!(options.contains(&"major_pentatonic".to_string()));
    assert!(options.contains(&"minor_pentatonic".to_string()));
    let snapshot = menu.snapshot();
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.contains("Maj Pentatonic")));
}

#[test]
fn pitch_note_params_use_legacy_note_name_display() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![1, 1, 4];
    menu.state.cursor = 0;
    let lowest = menu.snapshot();
    assert!(lowest.lines.iter().any(|line| line.trim() == "C1 (24)"));
    menu.state.cursor = 1;
    let highest = menu.snapshot();
    assert!(highest.lines.iter().any(|line| line.trim() == "C6 (84)"));
    menu.state.cursor = 2;
    let starting = menu.snapshot();
    assert!(starting.lines.iter().any(|line| line.trim() == "C4 (60)"));
}

#[test]
fn dance_fx_page_is_flat_and_shows_selected_type_params() {
    let mut config = config();
    config.dance_mode = "fx".into();
    config.dance_fx_type = "stutter".into();
    config
        .dance_fx_params
        .insert("rateHz".into(), serde_json::json!(12));
    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![3];
    let _snapshot = menu.snapshot();
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "FX Type"));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Rate Hz"));
    assert!(menu.current_siblings().iter().any(|item| {
        item.label == "Rate Hz" && matches!(item.value, NativeMenuValue::Number { value: 12, .. })
    }));
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Depth"));
    assert!(!menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Stutter"));
}

#[test]
fn fx_slot_groups_show_selected_effect_params() {
    let mut config = config();
    config.fx_buses[0].slot1_type = "delay".into();
    config.fx_buses[0].slot1_params =
        serde_json::json!({ "timeMs": 333, "feedback": 0.42, "mixPct": 44 });
    config.global_fx_slots[0] = "vinyl".into();
    config.global_fx_params[0] =
        serde_json::json!({ "cracklePct": 9, "warpDepthPct": 6, "mixPct": 80 });

    let mut menu = NativeMenuModel::new(config);
    menu.state.stack = vec![2, 1, 0, 0];
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Feedback"
            && matches!(item.value, NativeMenuValue::Number { value: 42, .. })));

    menu.state.stack = vec![2, 2, 0];
    assert!(menu
        .current_siblings()
        .iter()
        .any(|item| item.label == "Crackle %"
            && matches!(item.value, NativeMenuValue::Number { value: 9, .. })));
}

#[test]
fn parameter_picker_exposes_binding_actions_for_aux_param_and_xy_targets() {
    let menu = NativeMenuModel::new(config());
    assert!(contains_set_binding(
        &menu.root,
        "aux:0:turn",
        "sound.noteLengthMs"
    ));
    assert!(contains_set_binding(
        &menu.root,
        "param:0:x:0",
        "instruments.0.mixer.volume"
    ));

    let mut xy_config = config();
    xy_config.dance_mode = "xy".into();
    let xy_menu = NativeMenuModel::new(xy_config);
    assert!(contains_set_binding(
        &xy_menu.root,
        "xy:x",
        "sound.velocityScalePct"
    ));
}

#[test]
fn aux_click_picker_exposes_assignable_actions() {
    let menu = NativeMenuModel::new(config());

    assert!(contains_aux_click_action(
        &menu.root,
        0,
        "sample.assign:0:0"
    ));
    assert!(contains_aux_click_action(&menu.root, 0, "dance.fx.map"));
    assert!(contains_aux_click_reset(&menu.root, 0));
}

#[test]
fn native_menu_help_targets_resolve_to_specific_tsv_rows() {
    let mut targets = Vec::new();
    let mut missing = Vec::new();
    for config in representative_help_configs() {
        let menu = NativeMenuModel::new(config);
        targets.extend(
            menu.help_targets()
                .into_iter()
                .filter(|target| target.kind != "action" || !target.key.is_empty()),
        );
    }
    targets.sort_by(|a, b| (&a.kind, &a.key, &a.path).cmp(&(&b.kind, &b.key, &b.path)));
    targets.dedup_by(|a, b| a.kind == b.kind && a.key == b.key && a.path == b.path);
    missing.extend(
        targets
            .into_iter()
            .filter(|target| crate::native_help::resolve_native_help_entry(target).is_none())
            .map(|target| format!("{} {} {}", target.kind, target.key, target.path)),
    );
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "missing help entries: {missing:#?}");
}

fn representative_help_configs() -> Vec<NativeMenuConfig> {
    let mut configs = vec![config()];

    let mut dynamic = config();
    dynamic.preset_names = vec!["One".into()];
    dynamic.preset_rename_source = Some("One".into());
    dynamic.midi_outputs = vec![("0".into(), "Out".into())];
    dynamic.midi_inputs = vec![("0".into(), "In".into())];
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

#[test]
fn action_rows_are_wired_or_explicit_none_placeholders() {
    let menu = NativeMenuModel::new(config());
    let mut unresolved = Vec::new();
    collect_unresolved_actions(&menu.root, "MENU".into(), &mut unresolved);
    assert_eq!(unresolved, Vec::<String>::new());
}

fn collect_unresolved_actions(item: &NativeMenuItem, path: String, unresolved: &mut Vec<String>) {
    let path = if path == "MENU" {
        item.label.clone()
    } else {
        format!("{path} > {}", item.label)
    };
    if matches!(item.value, NativeMenuValue::Action(NativeMenuAction::Noop))
        && item.label != "(none)"
    {
        unresolved.push(path.clone());
    }
    for child in &item.children {
        collect_unresolved_actions(child, path.clone(), unresolved);
    }
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
