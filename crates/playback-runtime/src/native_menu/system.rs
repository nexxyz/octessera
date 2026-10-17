use super::system_io::{audio_group, hdmi_video_group, midi_group, sd_card_2_group};
use super::system_saves::{load_preset_group, saves_group};
use super::system_setup::setup_group;
use super::{
    action_item, bool_item, enum_item, group, number_item, selected_index, NativeMenuAction,
    NativeMenuConfig, NativeMenuItem,
};

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
                    "Audio+Video",
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
        children.push(super::system_bluetooth::bluetooth_group(config));
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
