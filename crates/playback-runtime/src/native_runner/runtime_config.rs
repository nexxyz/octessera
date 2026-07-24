use crate::protocol::RunnerMessage;
use crate::runtime::RuntimeConfig;

use super::NativeRunner;

impl NativeRunner {
    fn runtime_config(&self) -> RuntimeConfig {
        RuntimeConfig {
            bpm: self.transport.bpm,
            sync_source: self.transport.sync_source.clone(),
            midi_clock_out_enabled: self.midi_clock_out_enabled,
            midi_out_enabled: self.midi_enabled && self.selected_midi_output_id.is_some(),
        }
    }

    pub(super) fn append_runtime_config_if_changed(&mut self, messages: &mut Vec<RunnerMessage>) {
        let config = self.runtime_config();
        if self.last_published_runtime_config.as_ref() == Some(&config) {
            return;
        }
        self.last_published_runtime_config = Some(config.clone());
        let position = messages
            .iter()
            .rposition(|message| matches!(message, RunnerMessage::RuntimeStatus { .. }))
            .unwrap_or(messages.len());
        messages.insert(position, RunnerMessage::RuntimeConfigChanged { config });
    }
}
