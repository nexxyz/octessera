use super::bindings::{axis_binding_label, parameter_picker_group};
use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item, selected_index,
    NativeMenuAction, NativeMenuConfig, NativeMenuItem, NativeMenuValue,
};

pub(super) fn system_group(config: &NativeMenuConfig, sync_index: usize) -> NativeMenuItem {
    group(
        "System",
        vec![
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
                        ],
                    ),
                    group(
                        "Factory",
                        vec![action_item(
                            "Load Fact. Default",
                            "factory.load",
                            NativeMenuAction::PlatformEffect("factory.load".into()),
                        )],
                    ),
                ],
            ),
            group(
                "Sound",
                vec![
                    number_item(
                        "Master Vol",
                        "masterVolume",
                        i32::from(config.master_volume),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Note Length",
                        "sound.noteLengthMs",
                        i32::from(config.note_length_ms),
                        30,
                        2000,
                        10,
                    ),
                    number_item(
                        "Velocity Scale",
                        "sound.velocityScalePct",
                        i32::from(config.velocity_scale_pct),
                        0,
                        200,
                        5,
                    ),
                    enum_item(
                        "Velocity Curve",
                        "sound.velocityCurve",
                        vec!["linear", "soft", "hard"],
                        selected_index(&["linear", "soft", "hard"], &config.velocity_curve),
                    ),
                    enum_item(
                        "Voice Stealing",
                        "sound.voiceStealingMode",
                        vec!["off", "lenient", "balanced", "aggressive"],
                        selected_index(
                            &["off", "lenient", "balanced", "aggressive"],
                            &config.voice_stealing_mode,
                        ),
                    ),
                ],
            ),
            group(
                "MIDI",
                vec![
                    bool_item("Enabled", "midiEnabled", config.midi_enabled),
                    action_item(
                        "Panic",
                        "midi.panic",
                        NativeMenuAction::PlatformEffect("midi.panic".into()),
                    ),
                    midi_ports_group("MIDI Out", "midi.output", &config.midi_outputs),
                    midi_ports_group("MIDI In", "midi.input", &config.midi_inputs),
                    group(
                        "Sync & Clock",
                        vec![
                            enum_item_from_strings(
                                "Sync",
                                "midiSyncMode",
                                vec!["internal".into(), "external".into()],
                                sync_index,
                            ),
                            bool_item(
                                "Clock Out",
                                "midi.clockOutEnabled",
                                config.midi_clock_out_enabled,
                            ),
                            bool_item(
                                "Clock In",
                                "midi.clockInEnabled",
                                config.midi_clock_in_enabled,
                            ),
                            bool_item(
                                "Respond Start/Stop",
                                "midi.respondToStartStop",
                                config.midi_respond_to_start_stop,
                            ),
                        ],
                    ),
                ],
            ),
            group(
                "UI",
                vec![
                    bool_item("Ghost Cells", "ghostCells", config.ghost_cells),
                    bool_item(
                        "Input Events While Paused",
                        "inputEventsWhilePaused",
                        config.input_events_while_paused,
                    ),
                    enum_item(
                        "Numeric Display",
                        "numericDisplayMode",
                        vec!["bar", "numbers", "bar+numbers"],
                        selected_index(
                            &["bar", "numbers", "bar+numbers"],
                            &config.numeric_display_mode,
                        ),
                    ),
                    number_item(
                        "Screen Sleep",
                        "screenSleepSeconds",
                        i32::from(config.screen_sleep_seconds),
                        0,
                        600,
                        10,
                    ),
                    number_item(
                        "Display Brightness",
                        "displayBrightness",
                        i32::from(config.display_brightness),
                        10,
                        100,
                        5,
                    ),
                    number_item(
                        "Grid Brightness",
                        "gridBrightness",
                        i32::from(config.grid_brightness),
                        10,
                        100,
                        5,
                    ),
                    number_item(
                        "Button Brightness",
                        "buttonBrightness",
                        i32::from(config.button_brightness),
                        10,
                        100,
                        5,
                    ),
                ],
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

fn midi_ports_group(
    label: &str,
    action_prefix: &str,
    ports: &[(String, String)],
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "Disconnect",
        format!("{action_prefix}.none"),
        NativeMenuAction::PlatformEffect(format!("{action_prefix}:")),
    )];
    children.extend(ports.iter().map(|(id, name)| {
        action_item(
            name.clone(),
            format!("{action_prefix}.{id}"),
            NativeMenuAction::PlatformEffect(format!("{action_prefix}:{id}")),
        )
    }));
    group(label, children)
}

pub(super) fn aux_mappings_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Aux Mappings",
        (0..4)
            .map(|index| {
                let binding = config.aux_bindings.get(index).cloned().unwrap_or_default();
                group(
                    format!("Aux {}", index + 1),
                    vec![
                        parameter_picker_group(
                            axis_binding_label("Turn", binding.turn.as_ref()),
                            format!("aux:{index}:turn"),
                            binding.turn.as_ref(),
                            config,
                        ),
                        aux_click_picker_group(index, binding.click.as_ref(), config),
                    ],
                )
            })
            .collect(),
    )
}

fn aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "(none)",
        format!("aux{}.click.none", index + 1),
        NativeMenuAction::SetAuxClick {
            index,
            action: None,
        },
    )];
    if let Some(action) = current {
        children.push(action_item(
            "Current",
            format!("aux{}.click.current", index + 1),
            NativeMenuAction::SetAuxClick {
                index,
                action: Some(Box::new(action.clone())),
            },
        ));
    }
    let behavior_actions = config
        .l1_items
        .iter()
        .filter_map(|item| match &item.value {
            NativeMenuValue::Action(NativeMenuAction::BehaviorAction(action)) => Some(action_item(
                item.label.clone(),
                format!("aux{}.click.behavior.{action}", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::BehaviorAction(action.clone()))),
                },
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !behavior_actions.is_empty() {
        children.push(group("Behavior", behavior_actions));
    }
    children.push(group(
        "Sample Assign",
        config
            .instrument_labels
            .iter()
            .enumerate()
            .map(|(instrument, label)| {
                action_item(
                    label.clone(),
                    format!("aux{}.click.sample.{instrument}", index + 1),
                    NativeMenuAction::SetAuxClick {
                        index,
                        action: Some(Box::new(NativeMenuAction::PlatformEffect(format!(
                            "sample.assign:{instrument}:0"
                        )))),
                    },
                )
            })
            .collect(),
    ));
    children.push(group(
        "Actions",
        vec![
            action_item(
                "Map FX",
                format!("aux{}.click.fx_map", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::PlatformEffect(
                        "dance.fx.map".into(),
                    ))),
                },
            ),
            action_item(
                "Reset Behavior",
                format!("aux{}.click.reset", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::ResetBehavior)),
                },
            ),
        ],
    ));
    let label = current
        .map(|_| "Click: mapped".to_string())
        .unwrap_or_else(|| "Click: (none)".into());
    group(label, children)
}
