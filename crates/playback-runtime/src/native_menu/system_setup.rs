use super::system_io::usb_data_role_item;
use super::{action_item, group, NativeMenuAction, NativeMenuConfig, NativeMenuItem};

pub(super) fn setup_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = Vec::new();
    if config.usb_data_role_available {
        children.push(usb_data_role_item(config));
    }
    children.push(updates_group());
    if config.jack_audio_required {
        children.extend([show_hold_group(), wifi_group()]);
    }
    children.extend([
        action_item(
            "Configure WiFi",
            "system.configureWifi",
            NativeMenuAction::PlatformEffect("system.configureWifi".into()),
        ),
        action_item(
            "Backup / Restore",
            "system.backupRestore",
            NativeMenuAction::PlatformEffect("system.backupRestore".into()),
        ),
        action_item(
            "Hardware Test",
            "system.hardwareTest",
            NativeMenuAction::PlatformEffect("system.hardwareTest".into()),
        ),
    ]);
    group("Setup", children)
}

fn updates_group() -> NativeMenuItem {
    group(
        "Updates",
        vec![
            action_item(
                "Check",
                "system.updateCheck",
                NativeMenuAction::PlatformEffect("system.updateCheck".into()),
            ),
            action_item(
                "Apply",
                "system.updateApply",
                NativeMenuAction::PlatformEffect("system.updateApply".into()),
            ),
            action_item(
                "Rollback",
                "system.rollback",
                NativeMenuAction::PlatformEffect("system.rollback".into()),
            ),
        ],
    )
}

fn show_hold_group() -> NativeMenuItem {
    group(
        "Live Show Mode",
        vec![
            action_item(
                "Start 48h",
                "maintenance.hold",
                NativeMenuAction::PlatformEffect("maintenance.hold".into()),
            ),
            action_item(
                "End",
                "maintenance.release",
                NativeMenuAction::PlatformEffect("maintenance.release".into()),
            ),
        ],
    )
}

fn wifi_group() -> NativeMenuItem {
    group(
        "Wi-Fi",
        vec![
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
        ],
    )
}
