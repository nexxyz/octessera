use super::system_saves::{load_preset_group, saves_group};
use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item, selected_index,
    NativeMenuAction, NativeMenuConfig, NativeMenuItem,
};
use crate::native_runner::UsbDataRole;

pub(super) fn system_group(config: &NativeMenuConfig, sync_index: usize) -> NativeMenuItem {
    let host_role = config.usb_data_role_available && config.usb_data_role.is_host();
    let mut children = vec![
        load_preset_group(&config.preset_names),
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
                    "St. Audio+OLED",
                    "recording.startAudioOled",
                    NativeMenuAction::PlatformEffect("recording.startAudioOled".into()),
                ),
                action_item(
                    "Stop",
                    "recording.stop",
                    NativeMenuAction::PlatformEffect("recording.stop".into()),
                ),
            ],
        ),
        group(
            "Notes",
            vec![
                number_item(
                    "Note Length",
                    "sound.noteLengthMs",
                    i32::from(config.note_length_ms),
                    30,
                    2000,
                    10,
                ),
                number_item(
                    "Vel Scale",
                    "sound.velocityScalePct",
                    i32::from(config.velocity_scale_pct),
                    0,
                    200,
                    5,
                ),
                enum_item(
                    "Vel Curve",
                    "sound.velocityCurve",
                    vec!["linear", "soft", "hard"],
                    selected_index(&["linear", "soft", "hard"], &config.velocity_curve),
                ),
            ],
        ),
        midi_group(config, sync_index, host_role),
        audio_group(config, host_role),
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
    ];
    if config.jack_audio_required {
        children.push(sd_card_2_group(host_role));
        children.push(hdmi_video_group(config));
    }
    children.extend([
        saves_group(config),
        setup_group(config),
        reset_group(),
        action_item(
            "Panic",
            "midi.panic",
            NativeMenuAction::PlatformEffect("midi.panic".into()),
        ),
        action_item(
            "Sys. Info",
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
    ]);
    group("System", children)
}

fn audio_group(config: &NativeMenuConfig, host_role: bool) -> NativeMenuItem {
    let mut children = Vec::new();
    if config.jack_audio_required {
        if !host_role {
            children.push(bool_item(
                "USB Audio",
                "audioOutputs.usb",
                config.audio_outputs.usb(),
            ));
        }
        children.push(bool_item(
            "HDMI Audio",
            "audioOutputs.hdmi",
            config.audio_outputs.hdmi(),
        ));
    }
    children.push(number_item(
        "Master Vol",
        "masterVolume",
        i32::from(config.master_volume),
        0,
        100,
        1,
    ));
    if config.audio_optimization_capacity_available {
        children.push(enum_item(
            "Perf. Mode",
            "sound.optimizeFor",
            vec!["latency", "capacity"],
            selected_index(
                &["latency", "capacity"],
                match config.audio_optimization {
                    crate::native_runner::AudioOptimization::Latency => "latency",
                    crate::native_runner::AudioOptimization::Capacity => "capacity",
                },
            ),
        ));
    }
    children.push(enum_item(
        "Polyphony",
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
    ));
    let mut engine_children = vec![
        enum_item(
            "CPU Warn %",
            "dsp.workerWarningThreshold",
            vec!["70", "75", "80", "85", "90", "95"],
            selected_index(
                &["70", "75", "80", "85", "90", "95"],
                config.dsp_config.worker_warning_threshold.id(),
            ),
        ),
        enum_item(
            "Bus Idle",
            "dsp.busIdleThreshold",
            vec!["exact", "-140", "-120", "-100", "-80"],
            selected_index(
                &["exact", "-140", "-120", "-100", "-80"],
                config.dsp_config.bus_idle_threshold.id(),
            ),
        ),
    ];
    if !config.audio_optimization_capacity_available {
        engine_children.push(enum_item(
            "Buf Frames",
            "sound.audioOutputBufferFrames",
            vec!["64", "128", "256", "512", "1024", "2048"],
            selected_index(
                &["64", "128", "256", "512", "1024", "2048"],
                &config.audio_output_buffer_frames.to_string(),
            ),
        ));
    }
    children.push(group("Engine", engine_children));
    group("Audio", children)
}

fn midi_group(config: &NativeMenuConfig, sync_index: usize, host_role: bool) -> NativeMenuItem {
    let mut children = vec![bool_item("MIDI Active", "midiEnabled", config.midi_enabled)];
    if !config.usb_data_role_available || host_role {
        children.push(group(
            "MIDI Host",
            vec![
                midi_ports_group("MIDI Out", "midi.output", &config.midi_outputs),
                midi_ports_group("MIDI In", "midi.input", &config.midi_inputs),
            ],
        ));
    }
    if config.jack_audio_required && !host_role {
        children.push(group(
            "USB Device",
            vec![bool_item(
                "USB MIDI",
                "usb.midiOutEnabled",
                config.usb_midi_out_enabled,
            )],
        ));
    }
    children.push(group(
        "Sync / Clock",
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
    ));
    group("MIDI", children)
}

fn usb_data_role_item(config: &NativeMenuConfig) -> NativeMenuItem {
    enum_item_from_strings(
        "USB Role",
        "usb.dataRole",
        vec!["Gadget".into(), "Host".into()],
        usize::from(config.usb_data_role == UsbDataRole::Host),
    )
}

fn sd_card_2_group(host_role: bool) -> NativeMenuItem {
    let mut children = Vec::new();
    if !host_role {
        children.push(action_item(
            "Start Transfer",
            "usb.sdTransferStart",
            NativeMenuAction::PlatformEffect("usb.sdTransferStart".into()),
        ));
    }
    children.push(action_item(
        "Stop Transfer",
        "usb.sdTransferStop",
        NativeMenuAction::PlatformEffect("usb.sdTransferStop".into()),
    ));
    group("SD Card 2", children)
}

fn hdmi_video_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mode_values = [
        "none",
        "live-grid",
        "plain-grid",
        "active-behavior",
        "cycle-behaviors",
    ];
    let mut children = vec![enum_item(
        "Mode",
        "hdmi.mode",
        vec![
            "Terminal",
            "live-grid",
            "plain-grid",
            "active-behavior",
            "cycle-behaviors",
        ],
        selected_index(&mode_values, &config.hdmi_mode),
    )];
    if config.hdmi_mode == "cycle-behaviors" {
        children.push(number_item(
            "Bars per cycle",
            "hdmi.cycleMeasures",
            i32::from(config.hdmi_cycle_measures),
            1,
            64,
            1,
        ));
    }
    children.push(bool_item(
        "Grid Lines",
        "hdmi.showGridlines",
        config.hdmi_show_gridlines,
    ));
    group("HDMI Video", children)
}

fn setup_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = Vec::new();
    if config.usb_data_role_available {
        children.push(usb_data_role_item(config));
    }
    children.extend([
        updates_group(),
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
