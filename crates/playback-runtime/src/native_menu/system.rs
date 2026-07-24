use super::system_saves::saves_group;
use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item, selected_index,
    NativeMenuAction, NativeMenuConfig, NativeMenuItem,
};

pub(super) fn system_group(config: &NativeMenuConfig, sync_index: usize) -> NativeMenuItem {
    group(
        "System",
        vec![
            saves_group(config),
            group(
                "Recording",
                vec![
                    number_item(
                        "Max Time",
                        "recording.maxMinutes",
                        i32::from(config.recording_max_minutes),
                        1,
                        120,
                        1,
                    ),
                    action_item(
                        "Start Audio",
                        "recording.startAudio",
                        NativeMenuAction::PlatformEffect("recording.startAudio".into()),
                    ),
                    action_item(
                        "Stop",
                        "recording.stop",
                        NativeMenuAction::PlatformEffect("recording.stop".into()),
                    ),
                ],
            ),
            group(
                "USB",
                vec![
                    enum_item(
                        "Audio Out",
                        "usb.audioOut",
                        vec!["jack", "usb", "both"],
                        selected_index(&["jack", "usb", "both"], &config.usb_audio_out),
                    ),
                    bool_item(
                        "MIDI Out",
                        "usb.midiOutEnabled",
                        config.usb_midi_out_enabled,
                    ),
                    action_item(
                        "Save & Reboot",
                        "usb.applyReboot",
                        NativeMenuAction::PlatformEffect("usb.applyReboot".into()),
                    ),
                    action_item(
                        "Start SD2 Xfer",
                        "usb.sdTransferStart",
                        NativeMenuAction::PlatformEffect("usb.sdTransferStart".into()),
                    ),
                    action_item(
                        "Stop SD2 Xfer",
                        "usb.sdTransferStop",
                        NativeMenuAction::PlatformEffect("usb.sdTransferStop".into()),
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
                        "Voice Limit",
                        "sound.voiceStealingMode",
                        vec![
                            "fixed12",
                            "fixed16",
                            "auto-soft",
                            "auto-balanced",
                            "auto-hard",
                            "none",
                        ],
                        selected_index(
                            &[
                                "fixed12",
                                "fixed16",
                                "auto-soft",
                                "auto-balanced",
                                "auto-hard",
                                "none",
                            ],
                            &config.voice_stealing_mode,
                        ),
                    ),
                    enum_item(
                        "Output Buffer",
                        "sound.audioOutputBufferFrames",
                        vec!["64", "128", "256", "512", "1024", "2048"],
                        selected_index(
                            &["64", "128", "256", "512", "1024", "2048"],
                            &config.audio_output_buffer_frames.to_string(),
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
                                "Follow S/S",
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
                    bool_item("Auto Map", "auxAutoMapEnabled", config.aux_auto_map_enabled),
                    enum_item(
                        "Number Style",
                        "numericDisplayMode",
                        vec!["bar", "numbers", "bar+numbers"],
                        selected_index(
                            &["bar", "numbers", "bar+numbers"],
                            &config.numeric_display_mode,
                        ),
                    ),
                    number_item(
                        "Dim Timer",
                        "dimTimerSeconds",
                        i32::from(config.dim_timer_seconds),
                        0,
                        600,
                        10,
                    ),
                    number_item(
                        "OLED Sleep",
                        "screenSleepSeconds",
                        i32::from(config.screen_sleep_seconds),
                        0,
                        600,
                        10,
                    ),
                    number_item(
                        "OLED Bright",
                        "displayBrightness",
                        i32::from(config.display_brightness),
                        10,
                        100,
                        5,
                    ),
                    number_item(
                        "Grid Bright",
                        "gridBrightness",
                        i32::from(config.grid_brightness),
                        10,
                        100,
                        5,
                    ),
                    number_item(
                        "Button Bright",
                        "buttonBrightness",
                        i32::from(config.button_brightness),
                        10,
                        100,
                        5,
                    ),
                ],
            ),
            updates_group(),
            hdmi_group(config),
            group(
                "Diagnostics",
                vec![action_item(
                    "Hardware Test",
                    "system.hardwareTest",
                    NativeMenuAction::PlatformEffect("system.hardwareTest".into()),
                )],
            ),
            action_item(
                "Info",
                "system.info",
                NativeMenuAction::PlatformEffect("system.info".into()),
            ),
            action_item(
                "Basic Help",
                "system.controlsHelp",
                NativeMenuAction::PlatformEffect("system.controlsHelp".into()),
            ),
            action_item(
                "Reboot",
                "system.reboot",
                NativeMenuAction::PlatformEffect("system.reboot".into()),
            ),
            action_item(
                "Shutdown",
                "system.shutdown",
                NativeMenuAction::PlatformEffect("system.shutdown".into()),
            ),
        ],
    )
}

fn hdmi_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "HDMI",
        vec![
            enum_item(
                "Mode",
                "hdmi.mode",
                vec![
                    "none",
                    "live-grid",
                    "plain-grid",
                    "active-behavior",
                    "cycle-behaviors",
                ],
                selected_index(
                    &[
                        "none",
                        "live-grid",
                        "plain-grid",
                        "active-behavior",
                        "cycle-behaviors",
                    ],
                    &config.hdmi_mode,
                ),
            ),
            number_item(
                "Cycle Bars",
                "hdmi.cycleMeasures",
                i32::from(config.hdmi_cycle_measures),
                1,
                64,
                1,
            ),
            bool_item(
                "Grid Lines",
                "hdmi.showGridlines",
                config.hdmi_show_gridlines,
            ),
        ],
    )
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

pub(super) use super::system_aux::aux_mappings_group;
