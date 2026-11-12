use super::{action_item, group, NativeMenuAction, NativeMenuItem, PlatformAction};

pub(super) fn maintenance_group(device: bool) -> NativeMenuItem {
    let mut children = vec![updates_group(device)];
    if device {
        children.push(action_item(
            "Backup / Restore",
            "system.backupRestore",
            NativeMenuAction::PlatformEffect(PlatformAction::SystemBackupRestore),
        ));
    }
    children.extend([
        action_item(
            "Hardware Test",
            "system.hardwareTest",
            NativeMenuAction::PlatformEffect(PlatformAction::SystemHardwareTest),
        ),
        reset_group(),
        action_item(
            "Sys. Info",
            "system.info",
            NativeMenuAction::PlatformEffect(PlatformAction::SystemInfo),
        ),
    ]);
    group("Maintenance", children)
}

fn updates_group(device: bool) -> NativeMenuItem {
    let mut children = vec![action_item(
        "Check",
        "system.updateCheck",
        NativeMenuAction::PlatformEffect(PlatformAction::SystemUpdateCheck),
    )];
    if device {
        children.extend([
            action_item(
                "Apply",
                "system.updateApply",
                NativeMenuAction::PlatformEffect(PlatformAction::SystemUpdateApply),
            ),
            action_item(
                "Rollback",
                "system.rollback",
                NativeMenuAction::PlatformEffect(PlatformAction::SystemRollback),
            ),
        ]);
    }
    group("Updates", children)
}

fn reset_group() -> NativeMenuItem {
    group(
        "Reset",
        vec![
            action_item(
                "Load Empty",
                "system.clearAll",
                NativeMenuAction::PlatformEffect(PlatformAction::SystemClearAll),
            ),
            action_item(
                "Load Factory",
                "factory.load",
                NativeMenuAction::PlatformEffect(PlatformAction::FactoryLoad),
            ),
        ],
    )
}

pub(super) fn live_show_group() -> NativeMenuItem {
    group(
        "Live Show Mode",
        vec![
            action_item(
                "Start 48h",
                "maintenance.hold",
                NativeMenuAction::PlatformEffect(PlatformAction::MaintenanceHold),
            ),
            action_item(
                "End",
                "maintenance.release",
                NativeMenuAction::PlatformEffect(PlatformAction::MaintenanceRelease),
            ),
        ],
    )
}
