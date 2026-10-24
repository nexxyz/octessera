use super::{action_item, group, NativeMenuAction, NativeMenuItem};

pub(super) fn maintenance_group(device: bool) -> NativeMenuItem {
    let mut children = vec![updates_group(device)];
    if device {
        children.push(action_item(
            "Backup / Restore",
            "system.backupRestore",
            NativeMenuAction::PlatformEffect("system.backupRestore".into()),
        ));
    }
    children.extend([
        action_item(
            "Hardware Test",
            "system.hardwareTest",
            NativeMenuAction::PlatformEffect("system.hardwareTest".into()),
        ),
        reset_group(),
        action_item(
            "Sys. Info",
            "system.info",
            NativeMenuAction::PlatformEffect("system.info".into()),
        ),
    ]);
    group("Maintenance", children)
}

fn updates_group(device: bool) -> NativeMenuItem {
    let mut children = vec![action_item(
        "Check",
        "system.updateCheck",
        NativeMenuAction::PlatformEffect("system.updateCheck".into()),
    )];
    if device {
        children.extend([
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
                NativeMenuAction::PlatformEffect("system.clearAll".into()),
            ),
            action_item(
                "Load Factory",
                "factory.load",
                NativeMenuAction::PlatformEffect("factory.load".into()),
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
