use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item, selected_index,
    NativeMenuAction, NativeMenuConfig, NativeMenuItem,
};
use crate::native_runner::UsbDataRole;

pub(super) const MIDI_HOST_LABEL: &str = "MIDI Host";
pub(super) const USB_DEVICE_LABEL: &str = "USB Device";

pub(super) fn audio_group(config: &NativeMenuConfig, host_role: bool) -> NativeMenuItem {
    let mut children = Vec::new();
    if config.board {
        if !host_role {
            children.push(usb_audio_item(config.audio_outputs.usb()));
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
    let engine_children = vec![
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
    children.push(group("Engine", engine_children));
    group("Audio", children)
}

pub(super) fn midi_group(
    config: &NativeMenuConfig,
    sync_index: usize,
    host_role: bool,
) -> NativeMenuItem {
    let mut children = vec![bool_item("MIDI Active", "midiEnabled", config.midi_enabled)];
    if !config.usb_data_role_available || host_role {
        children.push(midi_host_group(&config.midi_outputs, &config.midi_inputs));
    }
    if config.board && !host_role {
        children.push(usb_device_group(config.usb_midi_out_enabled));
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

pub(super) fn usb_audio_item(enabled: bool) -> NativeMenuItem {
    bool_item("USB Audio", "audioOutputs.usb", enabled)
}

pub(super) fn midi_host_group(
    outputs: &[(String, String)],
    inputs: &[(String, String)],
) -> NativeMenuItem {
    group(
        MIDI_HOST_LABEL,
        vec![
            midi_ports_group("MIDI Out", "midi.output", outputs),
            midi_ports_group("MIDI In", "midi.input", inputs),
        ],
    )
}

pub(super) fn usb_device_group(usb_midi_out_enabled: bool) -> NativeMenuItem {
    group(
        USB_DEVICE_LABEL,
        vec![bool_item(
            "USB MIDI",
            "usb.midiOutEnabled",
            usb_midi_out_enabled,
        )],
    )
}

pub(super) fn midi_ports_group(
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

pub(super) fn usb_data_role_item(config: &NativeMenuConfig) -> NativeMenuItem {
    enum_item_from_strings(
        "USB Role",
        "usb.dataRole",
        vec!["Gadget".into(), "Host".into()],
        usize::from(config.usb_data_role == UsbDataRole::Host),
    )
}

pub(super) fn sd_card_2_group(host_role: bool) -> NativeMenuItem {
    let mut children = Vec::new();
    if !host_role {
        children.push(sd_transfer_start_item());
    }
    children.push(action_item(
        "Stop Transfer",
        "usb.sdTransferStop",
        NativeMenuAction::PlatformEffect("usb.sdTransferStop".into()),
    ));
    group("SD Card 2", children)
}

pub(super) fn sd_transfer_start_item() -> NativeMenuItem {
    action_item(
        "Start Transfer",
        "usb.sdTransferStart",
        NativeMenuAction::PlatformEffect("usb.sdTransferStart".into()),
    )
}

pub(super) fn hdmi_video_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mode_values = [
        "terminal",
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
        children.push(hdmi_cycle_measures_item(config.hdmi_cycle_measures));
    }
    children.push(bool_item(
        "Grid Lines",
        "hdmi.showGridlines",
        config.hdmi_show_gridlines,
    ));
    group("HDMI Video", children)
}

pub(super) fn hdmi_cycle_measures_item(measures: u8) -> NativeMenuItem {
    number_item(
        "Bars per cycle",
        "hdmi.cycleMeasures",
        i32::from(measures),
        1,
        64,
        1,
    )
}
