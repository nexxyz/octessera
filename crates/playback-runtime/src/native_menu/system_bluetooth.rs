use super::{
    action_item, bool_item, group, keyed_group, NativeBluetoothRows, NativeMenuAction,
    NativeMenuItem,
};

pub(super) fn bluetooth_group(rows: NativeBluetoothRows<'_>) -> NativeMenuItem {
    let mut children = vec![bool_item("Bluetooth", "bluetooth.enabled", rows.enabled)];
    if rows.enabled {
        children.extend([
            bool_item("Audio Out", "bluetooth.audio", rows.audio),
            device_rows_group("Devices", "toggle", rows.paired),
            device_rows_group("Pair New", "pair", rows.found),
            device_rows_group("Forget", "forget", rows.paired),
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
