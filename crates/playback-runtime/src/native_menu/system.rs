use super::section_labels::SYSTEM_LABEL;
use super::system_connections::connections_group;
use super::system_io::{audio_group, hdmi_video_group, midi_group};
use super::system_maintenance::{live_show_group, maintenance_group};
use super::system_saves::{load_preset_group, saves_group};
use super::{
    action_item, bool_item, enum_item, group, number_item, selected_index, NativeMenuAction,
    NativeMenuConfig, NativeMenuItem,
};

pub(super) fn system_group(config: &NativeMenuConfig, sync_index: usize) -> NativeMenuItem {
    let host_role = config.usb_data_role_available && config.usb_data_role.is_host();
    let mut children = vec![
        load_preset_group(&config.preset_names),
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
        audio_group(config, host_role),
        midi_group(config, sync_index, host_role),
    ];
    if config.board {
        children.push(connections_group(config, host_role));
    }
    children.push(display_group(config));
    if config.board {
        children.push(live_show_group());
    }
    children.extend([
        maintenance_group(config.board),
        action_item(
            "Panic",
            "midi.panic",
            NativeMenuAction::PlatformEffect("midi.panic".into()),
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
    group(SYSTEM_LABEL, children)
}

fn display_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = vec![
        bool_item("Ghost Cells", "ghostCells", config.ghost_cells),
        bool_item("Auto Map", "auxAutoMapEnabled", config.aux_auto_map_enabled),
        enum_item(
            "Num Style",
            "numericDisplayMode",
            vec!["bar", "numbers"],
            selected_index(&["bar", "numbers"], &config.numeric_display_mode),
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
    ];
    if config.board {
        children.push(hdmi_video_group(config));
    }
    group("Display", children)
}
