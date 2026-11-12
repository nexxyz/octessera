use super::{
    action_item, bool_item, group, keyed_group, NativeBluetoothRows, NativeMenuAction,
    NativeMenuItem, PlatformAction,
};

pub(super) fn bluetooth_group(rows: NativeBluetoothRows<'_>) -> NativeMenuItem {
    let mut children = vec![bool_item("Bluetooth", "bluetooth.enabled", rows.enabled)];
    if rows.enabled {
        children.extend([
            bool_item("Audio Out", "bluetooth.audio", rows.audio),
            device_rows_group(
                "Devices",
                "toggle",
                PlatformAction::BluetoothToggle,
                rows.paired,
            ),
            pair_new_group(rows.found, rows.searching),
            device_rows_group(
                "Forget",
                "forget",
                PlatformAction::BluetoothForget,
                rows.paired,
            ),
        ]);
    }
    group("Bluetooth", children)
}

/// The found devices, then a dim `Searching...` row while the scan runs, or `Search again` once
/// it has ended.
fn pair_new_group(found: &[(String, String)], searching: bool) -> NativeMenuItem {
    let mut pair_new = device_rows_group("Pair New", "pair", PlatformAction::BluetoothPair, found);
    pair_new.children.push(if searching {
        group("Searching...", vec![])
    } else {
        action_item(
            "Search again",
            "bluetooth.scan",
            NativeMenuAction::PlatformEffect(PlatformAction::BluetoothScan),
        )
    });
    pair_new
}

/// Keyed so the page opens while empty; Pair New must open to start searching.
fn device_rows_group(
    label: &str,
    verb: &str,
    action: fn(String) -> PlatformAction,
    devices: &[(String, String)],
) -> NativeMenuItem {
    keyed_group(
        label,
        format!("bluetooth.{verb}"),
        devices
            .iter()
            .map(|(address, name)| {
                action_item(
                    name.clone(),
                    format!("bluetooth.{verb}:{address}"),
                    NativeMenuAction::PlatformEffect(action(address.clone())),
                )
            })
            .collect(),
    )
}
