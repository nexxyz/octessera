use super::{
    action_item, bool_item, group, keyed_group, pick_list, NativeMenuAction, NativeMenuConfig,
    NativeMenuItem, PlatformAction,
};

pub(super) fn load_preset_group(names: &[String]) -> NativeMenuItem {
    pick_list(keyed_group(
        "Load Preset",
        "preset.load",
        preset_action_children("preset.load", PlatformAction::PresetLoad, names),
    ))
}

pub(super) fn saves_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Saves",
        vec![
            group(
                "Library",
                vec![
                    save_as_group(&config.preset_draft_name),
                    pick_list(preset_action_group(
                        "Load",
                        "preset.library.load",
                        PlatformAction::PresetLoad,
                        &config.preset_names,
                    )),
                    preset_rename_group(
                        &config.preset_names,
                        &config.preset_draft_name,
                        config.preset_rename_source.as_deref(),
                    ),
                    preset_action_group(
                        "Delete",
                        "preset.library.delete",
                        PlatformAction::PresetDelete,
                        &config.preset_names,
                    ),
                    action_item(
                        "Save Current",
                        "preset.saveCurrent",
                        NativeMenuAction::PlatformEffect(PlatformAction::PresetSaveCurrent),
                    ),
                    action_item(
                        "Refresh List",
                        "preset.refresh",
                        NativeMenuAction::PlatformEffect(PlatformAction::PresetRefresh),
                    ),
                ],
            ),
            group(
                "Default",
                vec![
                    bool_item("Auto Save", "autoSaveDefault", config.auto_save_default),
                    bool_item("Backups", "rollingBackups", config.rolling_backups),
                    action_item(
                        "Save Patch",
                        "default.save",
                        NativeMenuAction::PlatformEffect(PlatformAction::DefaultSave),
                    ),
                    action_item(
                        "Load Patch",
                        "default.load",
                        NativeMenuAction::PlatformEffect(PlatformAction::DefaultLoad),
                    ),
                ],
            ),
            group(
                "System",
                vec![
                    action_item(
                        "Save System",
                        "system.save",
                        NativeMenuAction::PlatformEffect(PlatformAction::SystemSave),
                    ),
                    action_item(
                        "Load System",
                        "system.load",
                        NativeMenuAction::PlatformEffect(PlatformAction::SystemLoad),
                    ),
                ],
            ),
        ],
    )
}

pub(super) fn save_as_group(draft_name: &str) -> NativeMenuItem {
    keyed_group(
        "Save As",
        "preset.library.saveAs",
        vec![
            super::text_item("Name", "system.draftName", draft_name, 32),
            action_item(
                "Save",
                "preset.saveAs.save",
                NativeMenuAction::PlatformEffect(PlatformAction::PresetSaveAs),
            ),
        ],
    )
}

pub(super) fn preset_action_group(
    label: &str,
    group_key: &str,
    action: fn(String) -> PlatformAction,
    names: &[String],
) -> NativeMenuItem {
    keyed_group(
        label,
        group_key,
        preset_action_children(group_key, action, names),
    )
}

/// One action per preset, keyed under its own list so a focus restore finds the list it was in.
pub(super) fn preset_action_children(
    group_key: &str,
    action: fn(String) -> PlatformAction,
    names: &[String],
) -> Vec<NativeMenuItem> {
    if names.is_empty() {
        vec![action_item(
            "(none)",
            format!("{group_key}.none"),
            NativeMenuAction::PlatformEffect(PlatformAction::PresetRefresh),
        )]
    } else {
        names
            .iter()
            .map(|name| {
                action_item(
                    name.clone(),
                    format!("{group_key}.{name}"),
                    NativeMenuAction::PlatformEffect(action(name.clone())),
                )
            })
            .collect()
    }
}

pub(super) fn preset_rename_group(
    names: &[String],
    draft_name: &str,
    rename_source: Option<&str>,
) -> NativeMenuItem {
    let mut children = if names.is_empty() {
        vec![action_item(
            "(none)",
            "preset.rename.none",
            NativeMenuAction::PlatformEffect(PlatformAction::PresetRefresh),
        )]
    } else {
        names
            .iter()
            .map(|name| {
                action_item(
                    name.clone(),
                    format!("preset.renamePick.{name}"),
                    NativeMenuAction::PlatformEffect(PlatformAction::PresetRenamePick(
                        name.clone(),
                    )),
                )
            })
            .collect()
    };
    if rename_source.is_some() {
        children.push(super::text_item(
            "New Name",
            "system.draftName",
            draft_name,
            32,
        ));
        children.push(action_item(
            "Apply",
            "preset.rename.apply",
            NativeMenuAction::PlatformEffect(PlatformAction::PresetRenameApply),
        ));
    }
    keyed_group("Rename", "preset.library.rename", children)
}
