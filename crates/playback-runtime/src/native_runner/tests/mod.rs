use super::*;

mod audio_menu;
mod audio_menu_direct;
mod audio_menu_naming;
mod aux_auto_map;
mod basics;
mod behavior_menu_defaults;
mod behavior_palette;
mod browser_and_help;
mod controls;
mod dim_sleep;
mod happy_path;
mod hdmi;
mod input_events;
mod instruments;
mod life_mapping;
mod looper;
mod menu_navigation;
mod menu_navigation_state;
mod modulation;
mod modulation_behavior_targets;
mod modulation_bindings;
mod pulses_and_tones_menu;
mod runtime_control;
mod runtime_transport;
mod sample_browser_store;
mod shutdown;
mod snapshot_autosave;
mod snapshot_runtime;
mod sparks_fx;
mod sparks_menu;
mod sparks_overlay;
mod step_rates;
mod store;
mod structural_draft;
mod trigger_gates;
mod ui_scenario;

pub(crate) fn snapshot_from(messages: &[RunnerMessage]) -> Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        })
        .expect("snapshot message")
}

pub(crate) fn led_cells(snapshot: &Value) -> Vec<Value> {
    let rgb = snapshot["leds"]["rgb"].as_array().expect("led rgb array");
    (0..64)
        .map(|index| {
            let offset = index * 3;
            json!({
                "r": rgb[offset].as_u64().unwrap(),
                "g": rgb[offset + 1].as_u64().unwrap(),
                "b": rgb[offset + 2].as_u64().unwrap(),
            })
        })
        .collect()
}

pub(crate) fn led_rgb(rgb: [u8; 3]) -> Value {
    json!({ "r": rgb[0], "g": rgb[1], "b": rgb[2] })
}

pub(crate) fn dim_rgb(rgb: [u8; 3], divisor: u8) -> [u8; 3] {
    let divisor = divisor.max(1);
    [rgb[0] / divisor, rgb[1] / divisor, rgb[2] / divisor]
}

pub(crate) fn confirm_current_dialog(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

pub(crate) fn select_behavior(runner: &mut NativeRunner, behavior_id: &str) {
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::SelectBehavior(
            behavior_id.into(),
        ))
        .unwrap();
}

pub(crate) fn musical_note_ons(messages: &[RunnerMessage]) -> Vec<(u8, u8)> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::MusicalEvents { events } => events.as_slice(),
            _ => &[],
        })
        .filter_map(|event| match event {
            platform_core::MusicalEvent::NoteOn { channel, note, .. } => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}
