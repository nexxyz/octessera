use super::{
    action_item, bool_item, group, keyed_group, NativeMenuAction, NativeMenuConfig, NativeMenuItem,
};

pub(super) fn bluetooth_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = vec![bool_item(
        "Bluetooth",
        "bluetooth.enabled",
        config.bluetooth_enabled,
    )];
    if config.bluetooth_enabled {
        children.extend([
            device_rows_group("Devices", "toggle", &config.bluetooth_paired),
            device_rows_group("Pair New", "pair", &config.bluetooth_found),
            device_rows_group("Forget", "forget", &config.bluetooth_paired),
        ]);
    }
    group("Bluetooth", children)
}

/// Keyed so the page opens while empty; Pair New must open to start searching.
fn device_rows_group(label: &str, verb: &str, devices: &[(String, String)]) -> NativeMenuItem {
    keyed_group(
        label,
        format!("bluetooth.{verb}"),
        devices
            .iter()
            .map(|(address, name)| {
                let action = format!("bluetooth.{verb}:{address}");
                action_item(
                    name.clone(),
                    action.clone(),
                    NativeMenuAction::PlatformEffect(action),
                )
            })
            .collect(),
    )
}
