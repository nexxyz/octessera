use super::*;

#[test]
pub(crate) fn fresh_lightning_active_config_menu_uses_native_defaults() {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "lightning".into(),
        behavior_config: Value::Null,
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    assert_lightning_defaults(&runner.menu_config().worlds_items, 0);
}

#[test]
pub(crate) fn fresh_lightning_target_config_menu_uses_native_defaults() {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "lightning".into(),
        behavior_config: Value::Null,
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    assert_lightning_defaults(&runner.menu_config().behavior_target_items[0], 0);
}

fn assert_lightning_defaults(items: &[crate::native_menu::NativeMenuItem], layer_index: usize) {
    let prefix = format!("layers.{layer_index}.worlds.behaviorConfig");
    assert_eq!(
        number_for_key(items, &format!("{prefix}.branchChancePct")),
        Some(25)
    );
    assert_eq!(
        number_for_key(items, &format!("{prefix}.jitterChancePct")),
        Some(20)
    );
    assert_eq!(
        number_for_key(items, &format!("{prefix}.decayTicks")),
        Some(4)
    );
    assert_eq!(
        number_for_key(items, &format!("{prefix}.leaderLimit")),
        Some(3)
    );
}

#[test]
pub(crate) fn invalid_step_rate_pulses_use_the_canonical_menu_default() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.algorithm_step_pulses = 7;
    runner.transport.layer_algorithm_step_pulses[0] = 7;

    let config = runner.menu_config();

    assert_eq!(
        enum_selected_for_key(&config.worlds_items, "algorithmStep"),
        Some(crate::timing_units::DEFAULT_NOTE_UNIT.into())
    );
    assert_eq!(
        enum_selected_for_key(&config.behavior_target_items[0], "layers.0.algorithmStep"),
        Some(crate::timing_units::DEFAULT_NOTE_UNIT.into())
    );
}

fn enum_selected_for_key(
    items: &[crate::native_menu::NativeMenuItem],
    key: &str,
) -> Option<String> {
    items.iter().find_map(|item| {
        if item.key.as_deref() == Some(key) {
            if let crate::native_menu::NativeMenuValue::Enum { options, selected } = &item.value {
                return options.get(*selected).cloned();
            }
        }
        enum_selected_for_key(&item.children, key)
    })
}

fn number_for_key(items: &[crate::native_menu::NativeMenuItem], key: &str) -> Option<i32> {
    items.iter().find_map(|item| {
        if item.key.as_deref() == Some(key) {
            if let crate::native_menu::NativeMenuValue::Number { value, .. } = item.value {
                return Some(value);
            }
        }
        number_for_key(&item.children, key)
    })
}
