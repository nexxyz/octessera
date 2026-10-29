use super::system_bluetooth::bluetooth_group;
use super::system_io::{sd_card_2_group, usb_data_role_item};
use super::{
    action_item, bool_item, group, NativeBluetoothRows, NativeMenuAction, NativeMenuConfig,
    NativeMenuItem,
};

pub(super) fn connections_group(config: &NativeMenuConfig, host_role: bool) -> NativeMenuItem {
    let mut children = vec![
        wifi_group(config),
        bluetooth_group(NativeBluetoothRows {
            enabled: config.bluetooth_enabled,
            audio: config.bluetooth_audio,
            paired: &config.bluetooth_paired,
            found: &config.bluetooth_found,
        }),
        sd_card_2_group(host_role),
    ];
    if config.usb_data_role_available {
        children.push(usb_data_role_item(config));
    }
    group("Connections", children)
}

fn wifi_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Wi-Fi",
        vec![
            bool_item("Wi-Fi", "wifi.enabled", config.wifi_enabled),
            action_item(
                "Setup Portal",
                "system.configureWifi",
                NativeMenuAction::PlatformEffect("system.configureWifi".into()),
            ),
        ],
    )
}
