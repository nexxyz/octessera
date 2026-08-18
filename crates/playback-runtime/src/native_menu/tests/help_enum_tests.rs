use super::*;

#[derive(Debug)]
struct EnumHelpTarget {
    target: NativeMenuHelpTarget,
    options: Vec<String>,
}

#[test]
pub(crate) fn representative_enum_help_names_every_canonical_option() {
    assert_enum_help_for_configs(representative_help_configs());
}

#[test]
pub(crate) fn behavior_enum_help_rows_resolve_by_active_keys() {
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

fn collect_enum_help_targets(menu: &NativeMenuModel) -> Vec<EnumHelpTarget> {
    let help_targets = menu.help_targets();
    let mut target_index = 0;
    let mut enum_targets = Vec::new();
    collect_enum_help_targets_from_item(
        &menu.root,
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
    enum_targets: &mut Vec<EnumHelpTarget>,
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
        if let NativeMenuValue::Enum { options, .. } = &child.value {
            assert_eq!(target.kind, "enum", "enum item has the wrong help kind");
            enum_targets.push(EnumHelpTarget {
                target,
                options: options.clone(),
            });
        }
        if !child.children.is_empty() {
            collect_enum_help_targets_from_item(child, help_targets, target_index, enum_targets);
        }
    }
}

fn assert_specific_enum_help(target: &EnumHelpTarget) {
    assert!(!target.target.key.is_empty(), "enum help target has no key");
    let entry = crate::native_help::resolve_native_help_entry(&target.target)
        .unwrap_or_else(|| panic!("missing specific enum help for {}", target.target.key));
    assert!(
        is_specific_enum_help_entry(&entry),
        "enum help for {} resolved through a generic fallback {}",
        target.target.key,
        entry.key
    );
    let copy = format!("{} {}", entry.line1, entry.line2);
    let missing = target
        .options
        .iter()
        .filter(|option| !contains_help_option(&copy, option))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "help for {} is missing runtime options {missing:?}: {copy}",
        target.target.key
    );
}

fn is_specific_enum_help_entry(entry: &crate::native_help::NativeHelpEntry) -> bool {
    let key = entry.key.trim();
    let path = entry.path.trim();
    !(path == "*" && key.is_empty()) && key != "key:*" && key != "action:*"
}

fn contains_help_option(copy: &str, option: &str) -> bool {
    let copy = copy.to_lowercase();
    let option = option.to_lowercase();
    let stable_option = option
        .split_once(':')
        .filter(|(prefix, _)| is_instrument_slot_label(prefix))
        .map(|(prefix, _)| prefix)
        .unwrap_or(&option);
    contains_bounded_option(&copy, &option) || contains_bounded_option(&copy, stable_option)
}

fn contains_bounded_option(copy: &str, option: &str) -> bool {
    bounded_contains(copy, option)
        || bounded_contains(
            &copy.replace(['_', '-'], " "),
            &option.replace(['_', '-'], " "),
        )
}

fn is_instrument_slot_label(value: &str) -> bool {
    let Some(number) = value.strip_prefix('i') else {
        return false;
    };
    !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
}

fn bounded_contains(copy: &str, option: &str) -> bool {
    if option.is_empty() {
        return false;
    }
    let mut offset = 0;
    while let Some(found) = copy[offset..].find(option) {
        let start = offset + found;
        let end = start + option.len();
        let before = copy[..start].chars().next_back();
        let after = copy[end..].chars().next();
        if !before.is_some_and(is_enum_value_char) && !after.is_some_and(is_enum_value_char) {
            return true;
        }
        offset = end;
    }
    false
}

fn is_enum_value_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '/' | '+' | '#' | '.')
}

// MIDI ports and sample filenames are generated runtime rows, not stable enum contracts.
fn is_runtime_generated_enum(target: &EnumHelpTarget) -> bool {
    let key = target.target.key.as_str();
    key.starts_with("key:midi.output")
        || key.starts_with("key:midi.input")
        || key.starts_with("key:sample.")
        || target.target.path.contains(" Browse")
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
            config.worlds_items_by_layer = vec![enum_items];
            Some(config)
        })
        .collect()
}

fn step_rate_item() -> NativeMenuItem {
    NativeMenuItem {
        label: "Step Rate".into(),
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
    }
}

fn behavior_enum_item(item: platform_core::BehaviorConfigItem) -> Option<NativeMenuItem> {
    if !matches!(item.item_type, platform_core::BehaviorConfigItemType::Enum) {
        return None;
    }
    Some(NativeMenuItem {
        label: item.label,
        key: Some(format!("layers.0.worlds.behaviorConfig.{}", item.key)),
        value: NativeMenuValue::Enum {
            options: item.options.unwrap_or_default(),
            selected: 0,
        },
        children: vec![],
    })
}
