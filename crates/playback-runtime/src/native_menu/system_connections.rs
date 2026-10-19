use super::system_bluetooth::bluetooth_group;
use super::system_io::{sd_card_2_group, usb_data_role_item};
use super::{action_item, group, NativeMenuAction, NativeMenuConfig, NativeMenuItem};

pub(super) fn connections_group(config: &NativeMenuConfig, host_role: bool) -> NativeMenuItem {
    let mut children = vec![wifi_group(config)];
    if config.jack_audio_required {
        children.extend([bluetooth_group(config), sd_card_2_group(host_role)]);
    }
    if config.usb_data_role_available {
        children.push(usb_data_role_item(config));
    }
    group("Connections", children)
}

fn wifi_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = Vec::new();
    if config.jack_audio_required {
        children.extend([
            action_item(
                "Turn Off",
                "wifi.off",
                NativeMenuAction::PlatformEffect("wifi.off".into()),
            ),
            action_item(
                "Turn On",
                "wifi.on",
                NativeMenuAction::PlatformEffect("wifi.on".into()),
            ),
        ]);
    }
    children.push(action_item(
        "Setup Portal",
        "system.configureWifi",
        NativeMenuAction::PlatformEffect("system.configureWifi".into()),
    ));
    group("Wi-Fi", children)
}
