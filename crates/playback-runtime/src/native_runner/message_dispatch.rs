use crate::protocol::{HostMessage, RunnerMessage, RuntimePlatformRequest};
use std::time::Instant;

use super::{DeviceInput, NativeRunner, RuntimeTransportState};

impl NativeRunner {
    pub fn send_music_first(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        if self.transport.transport != RuntimeTransportState::Playing
            || self.display.runtime_error_presentation.is_some()
            || !matches!(
                &message,
                HostMessage::TransportPulseStep { .. }
                    | HostMessage::MidiRealtimeClock { .. }
                    | HostMessage::DeviceInput { .. }
            )
            || matches!(&message, HostMessage::DeviceInput { input, .. }
                if !matches!(input.get("type").and_then(serde_json::Value::as_str),
                    Some("encoder_turn" | "encoder_press" | "grid_press" | "grid_release"
                        | "button_a" | "button_s" | "button_shift" | "button_fn" | "button_combined_modifier")))
        {
            return <Self as super::CoreRunner>::send(self, message)
                .map(order_terminal_presentation);
        }
        if let HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        } = &message
        {
            if input.get("type").and_then(serde_json::Value::as_str) != Some("encoder_turn")
                || input.get("delta").and_then(serde_json::Value::as_i64) != Some(0)
            {
                self.display.transients.mark_presentation_due();
            }
        }
        self.pending.presentation_deferred = true;
        let result = <Self as super::CoreRunner>::send(self, message);
        self.pending.presentation_deferred = false;
        result.map(order_terminal_presentation)
    }

    pub(super) fn require_synchronous_action_presentation(&mut self) {
        self.pending.presentation_deferred = false;
        self.pending.suppress_snapshot_response = false;
    }

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

    fn send_presented_runtime_error_input(
        &mut self,
        input: serde_json::Value,
    ) -> Result<Vec<RunnerMessage>, String> {
        let input = serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
        self.handle_presented_runtime_error_input(input)
    }
}

fn order_terminal_presentation(mut messages: Vec<RunnerMessage>) -> Vec<RunnerMessage> {
    let Some(terminal) = messages.iter().position(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect,
            crate::RuntimePlatformEffect::Shutdown | crate::RuntimePlatformEffect::Reboot))
    )) else { return messages; };
    let Some(snapshot) = messages.iter().position(|message| {
        matches!(message,
            RunnerMessage::Snapshot { snapshot } if snapshot["display"]["splash"] == "shutdown"
        )
    }) else {
        return messages;
    };
    if terminal < snapshot {
        let effect = messages.remove(terminal);
        messages.insert(snapshot, effect);
    }
    messages
}

impl super::CoreRunner for NativeRunner {
    fn register_platform_request(&mut self, request: &RuntimePlatformRequest) {
        if request.operation() == crate::RuntimeOperation::StoreSaveDefault {
            self.register_default_write_request(&request.request_id, request.revision);
        }
    }

    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let flush_time = Instant::now();
        self.advance_xy_smoothing_at(flush_time)?;
        let presented_error_input =
            matches!(&message, HostMessage::PresentedRuntimeErrorInput { .. });
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
            HostMessage::PresentedRuntimeErrorInput { input } => {
                self.send_presented_runtime_error_input(input)
            }
            HostMessage::MidiRealtimeStart => self.send_midi_realtime_start(),
            HostMessage::MidiRealtimeContinue => self.send_midi_realtime_continue(),
            HostMessage::MidiRealtimeStop => self.send_midi_realtime_stop(),
            HostMessage::TransportStop => self.send_transport_stop(),
            HostMessage::MidiRealtimeClock { pulses } => self.send_midi_realtime_clock(pulses),
            HostMessage::RuntimeResult { result } => self.send_runtime_result(result),
        }?;
        if presented_error_input {
            return Ok(messages);
        }
        if self.pending.presentation_deferred {
            messages.extend(self.flush_deferred_menu_apply_music_first(flush_time)?);
        } else {
            messages.extend(self.flush_deferred_menu_apply_at(flush_time)?);
        }
        self.append_runtime_config_if_changed(&mut messages);
        Ok(messages)
    }
}
