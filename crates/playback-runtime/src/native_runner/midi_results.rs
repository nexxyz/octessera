use crate::protocol::{MidiPort, RuntimeStoreResult};

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_midi_result(&mut self, result: RuntimeStoreResult) -> Result<(), String> {
        match result {
            RuntimeStoreResult::MidiListOutputsResult { outputs } => {
                self.midi_outputs = outputs;
                self.menu.show_midi_ports(
                    "MIDI Out",
                    "midi.output",
                    &midi_port_rows(&self.midi_outputs),
                );
            }
            RuntimeStoreResult::MidiListInputsResult { inputs } => {
                self.midi_inputs = inputs;
                self.display.runtime_error_presentation = None;
                self.menu.show_midi_ports(
                    "MIDI In",
                    "midi.input",
                    &midi_port_rows(&self.midi_inputs),
                );
            }
            RuntimeStoreResult::MidiStatus {
                ok,
                message,
                selected_out_id,
                selected_in_id,
            } => {
                let selection_changed = self.selected_midi_output_id != selected_out_id
                    || self.selected_midi_input_id != selected_in_id;
                self.midi_status = Some(if ok {
                    "MIDI ok".into()
                } else {
                    message.unwrap_or_else(|| "MIDI error".into())
                });
                self.selected_midi_output_id = selected_out_id;
                self.selected_midi_input_id = selected_in_id;
                if selection_changed {
                    self.mark_system_dirty();
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub(super) fn midi_port_rows(ports: &[MidiPort]) -> Vec<(String, String)> {
    ports
        .iter()
        .map(|port| (port.id.clone(), port.name.clone()))
        .collect()
}
