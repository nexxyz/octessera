use super::*;

mod audio_menu;
mod audio_menu_direct;
mod aux_auto_map;
mod basics;
mod browser_and_help;
mod dance_fx;
mod dance_menu;
mod dance_overlay;
mod input_events;
mod instruments;
mod menu_navigation;
mod menu_navigation_state;
mod modulation;
mod modulation_bindings;
mod runtime_control;
mod runtime_transport;
mod sense_and_voice_menu;
mod shutdown;
mod snapshot_runtime;
mod step_rates;
mod store;
mod trigger_gates;

fn snapshot_from(messages: &[RunnerMessage]) -> Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        })
        .expect("snapshot message")
}

fn led_cells(snapshot: &Value) -> Vec<Value> {
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

fn confirm_current_dialog(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap()
}

fn musical_note_ons(messages: &[RunnerMessage]) -> Vec<(u8, u8)> {
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
