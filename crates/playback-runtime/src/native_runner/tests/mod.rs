use super::*;

mod audio_menu;
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

fn snapshot_from(messages: &[RunnerMessage]) -> Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        })
        .expect("snapshot message")
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
