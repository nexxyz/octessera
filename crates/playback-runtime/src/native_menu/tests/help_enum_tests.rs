use super::*;

#[test]
pub(crate) fn representative_enum_help_resolves_to_specific_entries() {
    assert_enum_help_for_configs(representative_help_configs());
}

#[test]
pub(crate) fn behavior_enum_help_resolves_to_specific_entries_by_active_keys() {
    assert_enum_help_for_configs(representative_behavior_help_configs());
}

fn assert_enum_help_for_configs(configs: Vec<NativeMenuConfig>) {
    let mut checked = 0;
    for config in configs {
        let menu = NativeMenuModel::new(config);
        for target in collect_enum_help_targets(&menu) {
            if is_runtime_generated_enum(&target) {
                continue;
            }
            checked += 1;
            assert_specific_enum_help(&target);
        }
    }
    assert!(
        checked > 0,
        "representative configs contained no enum targets"
    );
}

fn collect_enum_help_targets(menu: &NativeMenuModel) -> Vec<NativeMenuHelpTarget> {
    let help_targets = menu.help_targets();
    let mut target_index = 0;
    let mut enum_targets = Vec::new();
    collect_enum_help_targets_from_item(
        &menu.browsable_root(),
        &help_targets,
        &mut target_index,
        &mut enum_targets,
    );
    assert_eq!(
        target_index,
        help_targets.len(),
        "recursive enum walker lost a menu help target"
    );
    enum_targets
}

fn collect_enum_help_targets_from_item(
    item: &NativeMenuItem,
    help_targets: &[NativeMenuHelpTarget],
    target_index: &mut usize,
    enum_targets: &mut Vec<NativeMenuHelpTarget>,
) {
    for child in &item.children {
        if child.label.is_empty() {
            continue;
        }
        let target = help_targets
            .get(*target_index)
            .unwrap_or_else(|| panic!("missing help target for menu item {}", child.label))
            .clone();
        *target_index += 1;
        if matches!(child.value, NativeMenuValue::Enum { .. }) {
            assert_eq!(target.kind, "enum", "enum item has the wrong help kind");
            enum_targets.push(target);
        }
        if !child.children.is_empty() {
            collect_enum_help_targets_from_item(child, help_targets, target_index, enum_targets);
        }
    }
}

fn assert_specific_enum_help(target: &NativeMenuHelpTarget) {
    assert!(!target.key.is_empty(), "enum help target has no key");
    let entry = crate::native_help::resolve_native_help_entry(target)
        .unwrap_or_else(|| panic!("missing specific enum help for {}", target.key));
    assert!(
        is_specific_enum_help_entry(&entry),
        "enum help for {} resolved through a generic fallback {}",
        target.key,
        entry.key
    );
    assert!(
        !entry.line1.trim().is_empty(),
        "enum help for {} does not say what the setting does",
        target.key
    );
    if target.key == "key:layers.*.link.pitch.scale" {
        let copy = format!("{} {}", entry.line1, entry.line2);
        assert!(copy.contains("pitch classes"));
        assert!(copy.contains("simultaneous chords or inversions"));
        assert!(copy.contains("12-TET approximations"));
    }
}

fn is_specific_enum_help_entry(entry: &crate::native_help::NativeHelpEntry) -> bool {
    let key = entry.key.trim();
    let path = entry.path.trim();
    !(path == "*" && key.is_empty()) && key != "key:*" && key != "action:*"
}

// MIDI ports and sample filenames are generated runtime rows, not stable enum contracts.
fn is_runtime_generated_enum(target: &NativeMenuHelpTarget) -> bool {
    let key = target.key.as_str();
    key.starts_with("key:midi.output")
        || key.starts_with("key:midi.input")
        || key.starts_with("key:sample.")
        || key == "key:instruments.*.drum.voice"
        || target.path.contains(" Browse")
}

fn representative_behavior_help_configs() -> Vec<NativeMenuConfig> {
    platform_core::list_native_behavior_ids()
        .iter()
        .filter_map(|behavior_id| {
            let behavior = platform_core::get_native_behavior(behavior_id)?;
            let state = behavior
                .init(serde_json::Value::Null)
                .unwrap_or_else(|error| {
                    panic!("default behavior state for {behavior_id}: {error}")
                });
            let config_items = behavior
                .config_menu(&state)
                .unwrap_or_else(|error| panic!("behavior menu for {behavior_id}: {error}"))?;
            let mut enum_items = Vec::new();
            if *behavior_id != "none" {
                enum_items.push(step_rate_item());
            }
            enum_items.extend(config_items.into_iter().filter_map(behavior_enum_item));

            let mut config = config();
            config.behavior_id = (*behavior_id).into();
            config.layer_labels[0] = format!("L1: {behavior_id}");
            config.build_items_by_layer = vec![enum_items];
            Some(config)
        })
        .collect()
}

fn step_rate_item() -> NativeMenuItem {
    NativeMenuItem {
        label: "Step Rate".into(),
        breadcrumb: None,
        key: Some("layers.0.algorithmStep".into()),
        value: NativeMenuValue::Enum {
            options: crate::timing_units::NOTE_UNIT_OPTIONS
                .iter()
                .copied()
                .map(String::from)
                .collect(),
            selected: crate::timing_units::note_unit_selection_index(
                crate::timing_units::DEFAULT_NOTE_UNIT,
            ),
        },
        children: vec![],
        pick: NativeMenuPick::None,
    }
}

fn behavior_enum_item(item: platform_core::BehaviorConfigItem) -> Option<NativeMenuItem> {
    if !matches!(item.item_type, platform_core::BehaviorConfigItemType::Enum) {
        return None;
    }
    Some(NativeMenuItem {
        label: item.label,
        breadcrumb: None,
        key: Some(format!("layers.0.build.behaviorConfig.{}", item.key)),
        value: NativeMenuValue::Enum {
            options: item.options.unwrap_or_default(),
            selected: 0,
        },
        children: vec![],
        pick: NativeMenuPick::None,
    })
}
