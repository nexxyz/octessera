use crate::protocol::{HostMessage, RunnerMessage};
use std::time::Instant;

use super::{DeviceInput, NativeRunner};

impl NativeRunner {
    fn send_device_input(
        &mut self,
        input: serde_json::Value,
        request_snapshot: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let input = serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
        if request_snapshot.unwrap_or(true) {
            return self.handle_device_input(input);
        }
        self.pending.suppress_snapshot_response = true;
        let messages = self.handle_device_input(input);
        self.pending.suppress_snapshot_response = false;
        messages
    }
}

impl super::CoreRunner for NativeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let flush_time = Instant::now();
        let mut messages = match message {
            HostMessage::TransportPulseStep {
                pulses,
                request_snapshot,
                ..
            } => self.send_transport_pulse_step(pulses, request_snapshot),
            HostMessage::DeviceInput {
                input,
                request_snapshot,
            } => self.send_device_input(input, request_snapshot),
            HostMessage::MidiRealtimeStart => self.send_midi_realtime_start(),
            HostMessage::MidiRealtimeContinue => self.send_midi_realtime_continue(),
            HostMessage::MidiRealtimeStop => self.send_midi_realtime_stop(),
            HostMessage::TransportStop => self.send_transport_stop(),
            HostMessage::MidiRealtimeClock { pulses } => self.send_midi_realtime_clock(pulses),
            HostMessage::RuntimeResult { result } => self.send_runtime_result(result),
        }?;
        messages.extend(self.flush_deferred_menu_apply_at(flush_time)?);
        self.append_runtime_config_if_changed(&mut messages);
        Ok(messages)
    }
}
