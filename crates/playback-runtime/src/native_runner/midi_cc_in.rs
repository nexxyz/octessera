use super::NativeRunner;
use crate::protocol::RunnerMessage;
use crate::runtime::MidiControlChange;

impl NativeRunner {
    pub(super) fn send_midi_control_change(
        &mut self,
        _change: MidiControlChange,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.messages_without_snapshot()
    }
}
