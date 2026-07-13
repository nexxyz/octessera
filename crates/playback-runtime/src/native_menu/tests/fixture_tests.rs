use super::*;

#[test]
pub(crate) fn fx_bus_and_global_fx_option_sets_only_share_supported_processors() {
    let shared_fx = [
        "none",
        "vinyl",
        "eq",
        "compressor",
        "saturator",
        "distortion",
    ];
    for fx_type in FX_BUS_SLOT_OPTIONS {
        assert!(is_valid_fx_bus_slot_type(fx_type));
        if is_valid_global_fx_slot_type(fx_type) {
            assert!(
                shared_fx.contains(fx_type),
                "{fx_type} should not be global"
            );
        }
    }
    for fx_type in GLOBAL_FX_SLOT_OPTIONS {
        assert!(is_valid_global_fx_slot_type(fx_type));
        if is_valid_fx_bus_slot_type(fx_type) {
            assert!(
                shared_fx.contains(fx_type),
                "{fx_type} should not be a bus FX"
            );
        }
    }
}

#[test]
pub(crate) fn native_menu_snapshot_rows_fit_oled_width() {
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

#[test]
pub(crate) fn aux_mappings_include_plain_and_shifted_rows() {
    let root = build_root(config());

    let turn = find_item_by_key(&root, "aux:0:turn").unwrap();
    assert_eq!(turn.label, "Trn: (none)");

    let shift_turn = find_item_by_key(&root, "shiftAux:0:turn").unwrap();
    assert_eq!(shift_turn.label, "S+Trn: (none)");

    let click = find_item_by_key(&root, "aux1.click.none").unwrap();
    assert!(matches!(
        click.value,
        NativeMenuValue::Action(NativeMenuAction::SetAuxClick { index: 0, .. })
    ));

    let shift_click = find_item_by_key(&root, "shift_aux1.click.none").unwrap();
    assert!(matches!(
        shift_click.value,
        NativeMenuValue::Action(NativeMenuAction::SetShiftAuxClick { index: 0, .. })
    ));
}

pub(crate) fn representative_help_configs() -> Vec<NativeMenuConfig> {
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
    dynamic.sample_favourite_dirs = vec![String::new()];
    configs.push(dynamic);

    let mut scanning = config();
    scanning.pulses_layers[0].scan_mode = "scanning".into();
    scanning.pulses_layers[0].x_velocity.enabled = true;
    scanning.pulses_layers[0].x_filter_cutoff.enabled = true;
    scanning.pulses_layers[0].x_filter_resonance.enabled = true;
    scanning.pulses_layers[0].y_pitch_enabled = true;
    scanning.pulses_layers[0].y_velocity.enabled = true;
    scanning.pulses_layers[0].y_filter_cutoff.enabled = true;
    scanning.pulses_layers[0].y_filter_resonance.enabled = true;
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

    for sparks_mode in ["mix", "pan", "fx", "trigger-gate", "xy"] {
        let mut cfg = config();
        cfg.sparks_mode = sparks_mode.into();
        cfg.sparks_fx_type = "stutter".into();
        cfg.sparks_fx_params
            .insert("rateHz".into(), serde_json::json!(8));
        configs.push(cfg);
    }

    for fx_type in ["stutter", "freeze", "filter_sweep", "pitch_shift"] {
        let mut cfg = config();
        cfg.sparks_mode = "fx".into();
        cfg.sparks_fx_type = fx_type.into();
        configs.push(cfg);
    }

    configs
}

pub(crate) fn contains_set_binding(item: &NativeMenuItem, target: &str, key: &str) -> bool {
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

pub(crate) fn contains_aux_click_action(
    item: &NativeMenuItem,
    index: usize,
    action_key: &str,
) -> bool {
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

pub(crate) fn find_item_by_key<'a>(
    item: &'a NativeMenuItem,
    key: &str,
) -> Option<&'a NativeMenuItem> {
    if item.key.as_deref() == Some(key) {
        return Some(item);
    }
    item.children
        .iter()
        .find_map(|child| find_item_by_key(child, key))
}

fn find_path_by_key(item: &NativeMenuItem, key: &str) -> Option<Vec<usize>> {
    if item.key.as_deref() == Some(key) {
        return Some(vec![]);
    }
    item.children.iter().enumerate().find_map(|(index, child)| {
        find_path_by_key(child, key).map(|mut path| {
            path.insert(0, index);
            path
        })
    })
}

#[test]
pub(crate) fn sample_browser_label_includes_selected_slot_context_without_body_rows() {
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

#[test]
pub(crate) fn loaded_sample_row_shows_filename_idle_and_full_path_when_selected() {
    let mut cfg = config();
    cfg.instrument_types[0] = "sampler".into();
    cfg.instrument_sample_slots[0] = 2;
    cfg.instrument_sample_paths[0][2] = Some("Drum/kick/long-sample-file.wav".into());

    let root = build_root(cfg.clone());
    let loaded =
        find_item_by_key(&root, "sample.loaded:0:2:Drum/kick/long-sample-file.wav").unwrap();
    assert_eq!(loaded.label, "long-sample-file.wav");

    let mut menu = NativeMenuModel::new(cfg);
    let path = find_path_by_key(
        &menu.root,
        "sample.loaded:0:2:Drum/kick/long-sample-file.wav",
    )
    .unwrap();
    menu.state.cursor = *path.last().unwrap();
    menu.state.stack = path[..path.len() - 1].to_vec();
    let snapshot = menu.snapshot();
    let selected_row = snapshot.selected_row.unwrap();
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.contains("sample-file")));
    assert_eq!(
        snapshot.full_lines[selected_row].as_deref(),
        Some(">!Drum/kick/long-sample-file.wav")
    );
}

#[test]
pub(crate) fn sample_browser_empty_and_long_name_rows_preserve_display_contract() {
    let mut cfg = config();
    cfg.instrument_types[0] = "sampler".into();
    cfg.sample_favourite_dirs.clear();
    cfg.sample_builtin_favourite_dirs.clear();
    cfg.sample_browser = Some(NativeSampleBrowserConfig {
        instrument_slot: 0,
        sample_slot: 0,
        dir: String::new(),
        entries: vec![],
    });

    let root = build_root(cfg.clone());
    let browser = find_item_by_key(&root, "sample.choose:0:0").unwrap();
    let labels = browser
        .children
        .iter()
        .map(|child| child.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&".."));
    assert!(labels.contains(&"(empty)"));

    cfg.sample_browser = Some(NativeSampleBrowserConfig {
        instrument_slot: 0,
        sample_slot: 0,
        dir: String::new(),
        entries: vec![
            NativeSampleEntryConfig {
                name: "Extremely Long Directory Name".into(),
                path: "Extremely Long Directory Name".into(),
                is_dir: true,
            },
            NativeSampleEntryConfig {
                name: "extremely-long-sample-file.wav".into(),
                path: "extremely-long-sample-file.wav".into(),
                is_dir: false,
            },
        ],
    });

    let root = build_root(cfg.clone());
    let browser = find_item_by_key(&root, "sample.choose:0:0").unwrap();
    assert!(browser
        .children
        .iter()
        .any(|child| { child.label.starts_with('[') && child.label.contains("Directory") }));
    assert!(browser
        .children
        .iter()
        .any(|child| child.label.contains("sample-file.wav")));

    let mut menu = NativeMenuModel::new(cfg);
    menu.state.stack = find_path_by_key(&menu.root, "sample.choose:0:0").unwrap();
    for line in menu.snapshot().lines {
        assert!(line.chars().count() <= 20, "row too wide: {line:?}");
    }
}

pub(crate) fn contains_aux_click_reset(item: &NativeMenuItem, index: usize) -> bool {
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
