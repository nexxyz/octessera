use crate::protocol::RuntimeStoreResult;

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_midi_result(&mut self, result: RuntimeStoreResult) -> Result<(), String> {
        match result {
            RuntimeStoreResult::MidiListOutputsResult { outputs } => {
                self.midi_outputs = outputs;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiListInputsResult { inputs } => {
                self.midi_inputs = inputs;
                self.display.runtime_error_presentation = None;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiStatus {
                ok,
                message,
                selected_out_id,
                selected_in_id,
            } => {
                self.midi_status = Some(if ok {
                    "MIDI ok".into()
                } else {
                    message.unwrap_or_else(|| "MIDI error".into())
                });
                self.selected_midi_output_id = selected_out_id;
                self.selected_midi_input_id = selected_in_id;
            }
            _ => {}
        }
        Ok(())
    }
}
