use super::{action_item, bool_item, group, NativeMenuAction, NativeMenuConfig, NativeMenuItem};

pub(super) fn saves_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Saves",
        vec![
            group(
                "Library",
                vec![
                    group(
                        "Save As",
                        vec![
                            super::text_item(
                                "Name",
                                "system.draftName",
                                config.preset_draft_name.clone(),
                                32,
                            ),
                            action_item(
                                "Save",
                                "preset.saveAs.save",
                                NativeMenuAction::PlatformEffect("preset.saveAs".into()),
                            ),
                        ],
                    ),
                    action_item(
                        "Save Current",
                        "preset.saveCurrent",
                        NativeMenuAction::PlatformEffect("preset.saveCurrent".into()),
                    ),
                    preset_action_group("Load", "preset.load", &config.preset_names),
                    preset_rename_group(config),
                    preset_action_group("Delete", "preset.delete", &config.preset_names),
                    action_item(
                        "Refresh List",
                        "preset.refresh",
                        NativeMenuAction::PlatformEffect("preset.refresh".into()),
                    ),
                ],
            ),
            group(
                "Default",
                vec![
                    action_item(
                        "Save Default",
                        "default.save",
                        NativeMenuAction::PlatformEffect("default.save".into()),
                    ),
                    action_item(
                        "Load Default",
                        "default.load",
                        NativeMenuAction::PlatformEffect("default.load".into()),
                    ),
                    bool_item("Auto Save", "autoSaveDefault", config.auto_save_default),
                    bool_item("Backups", "rollingBackups", config.rolling_backups),
                ],
            ),
            group(
                "Factory",
                vec![action_item(
                    "Load Factory",
                    "factory.load",
                    NativeMenuAction::PlatformEffect("factory.load".into()),
                )],
            ),
            action_item(
                "Load Empty",
                "system.clearAll",
                NativeMenuAction::PlatformEffect("system.clearAll".into()),
            ),
        ],
    )
}

fn preset_action_group(label: &str, action_prefix: &str, names: &[String]) -> NativeMenuItem {
    let children = if names.is_empty() {
        vec![action_item(
            "(none)",
            format!("{action_prefix}.none"),
            NativeMenuAction::PlatformEffect("preset.refresh".into()),
        )]
    } else {
        names
            .iter()
            .map(|name| {
                action_item(
                    name.clone(),
                    format!("{action_prefix}.{name}"),
                    NativeMenuAction::PlatformEffect(format!("{action_prefix}:{name}")),
                )
            })
            .collect()
    };
    group(label, children)
}

fn preset_rename_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = if config.preset_names.is_empty() {
        vec![action_item(
            "(none)",
            "preset.rename.none",
            NativeMenuAction::PlatformEffect("preset.refresh".into()),
        )]
    } else {
        config
            .preset_names
            .iter()
            .map(|name| {
                action_item(
                    name.clone(),
                    format!("preset.renamePick.{name}"),
                    NativeMenuAction::PlatformEffect(format!("preset.renamePick:{name}")),
                )
            })
            .collect()
    };
    if config.preset_rename_source.is_some() {
        children.push(super::text_item(
            "New Name",
            "system.draftName",
            config.preset_draft_name.clone(),
            32,
        ));
        children.push(action_item(
            "Apply",
            "preset.rename.apply",
            NativeMenuAction::PlatformEffect("preset.renameApply".into()),
        ));
    }
    group("Rename", children)
}
