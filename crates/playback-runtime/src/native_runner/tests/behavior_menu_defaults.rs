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
