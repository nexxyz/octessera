use crate::protocol::{HostMessage, RunnerMessage};

use super::{
    wrap_help_text, DeviceInput, NativeHelpPopup, NativeRunner, NativeToast, RuntimeTransportState,
    SyncSource,
};

impl NativeRunner {
    pub(super) fn open_contextual_help(&mut self) {
        let Some(target) = self.menu.current_help_target() else {
            self.toast = Some(NativeToast {
                message: "Missing help target".into(),
                offset: 0,
            });
            return;
        };
        let Some(help) = crate::native_help::resolve_native_help(&target) else {
            self.toast = Some(NativeToast {
                message: format!("Missing help: {}", target.label),
                offset: 0,
            });
            return;
        };
        let title = format!("Help: {}", help.title);
        self.help_popup = Some(NativeHelpPopup {
            title,
            lines: wrap_help_text(&help.detail, 28),
            scroll: 0,
        });
    }

    pub(super) fn turn_help_popup(&mut self, delta: i8) {
        let Some(help) = &mut self.help_popup else {
            return;
        };
        let max_scroll = help.lines.len().saturating_sub(super::OLED_BODY_ROWS - 1);
        let next = (help.scroll as isize + delta as isize).clamp(0, max_scroll as isize) as usize;
        help.scroll = next;
    }

    pub(super) fn turn_confirm_dialog(&mut self, delta: i8) {
        let Some(confirm) = &mut self.confirm_dialog else {
            return;
        };
        let max = confirm.options.len().saturating_sub(1);
        confirm.cursor = (confirm.cursor as isize + delta as isize).clamp(0, max as isize) as usize;
    }

    pub(super) fn confirm_dialog_selection(
        &mut self,
    ) -> Result<Option<super::RuntimePlatformEffect>, String> {
        let Some(confirm) = self.confirm_dialog.take() else {
            return Ok(None);
        };
        if confirm.cursor == 0 {
            self.toast = Some(NativeToast {
                message: "Cancelled".into(),
                offset: 0,
            });
            return Ok(None);
        }
        self.execute_confirmed_action(confirm.action)
    }

    fn send_transport_pulse_step(
        &mut self,
        pulses: u32,
        request_snapshot: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.current_ppqn_pulse = self.current_ppqn_pulse.saturating_add(pulses as u64);
        let mut out = Vec::new();
        let events = self.advance_algorithm(pulses)?;
        if !events.is_empty() {
            out.push(self.trigger_ui_pulse_message());
            out.push(RunnerMessage::MusicalEvents { events });
        }
        if let Some(pulse) = self.transport_ui_pulse_message() {
            out.push(pulse);
        }
        if request_snapshot.unwrap_or(false) {
            self.advance_oled_sleep_state();
            self.advance_toast_state();
            out.push(RunnerMessage::Snapshot {
                snapshot: self.snapshot()?,
            });
        }
        out.push(RunnerMessage::RuntimeStatus {
            status: self.status(),
        });
        Ok(out)
    }

    fn send_device_input(
        &mut self,
        input: serde_json::Value,
    ) -> Result<Vec<RunnerMessage>, String> {
        let input = serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
        self.handle_device_input(input)
    }

    fn send_midi_realtime_start(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport = RuntimeTransportState::Playing;
        self.reset_transport_position();
        self.transport_flash = "measure";
        self.transport_flash_pulses_remaining = 6;
        let mut messages = Vec::new();
        if let Some(pulse) = self.transport_ui_pulse_message() {
            messages.push(pulse);
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }

    fn send_midi_realtime_continue(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport = RuntimeTransportState::Playing;
        self.messages_with_snapshot()
    }

    fn send_midi_realtime_stop(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.messages_with_snapshot()
    }

    fn send_midi_realtime_clock(&mut self, pulses: u32) -> Result<Vec<RunnerMessage>, String> {
        if self.sync_source == SyncSource::External && !self.midi_clock_in_enabled {
            return self.messages_with_snapshot();
        }
        self.current_ppqn_pulse = self.current_ppqn_pulse.saturating_add(pulses as u64);
        if self.sync_source == SyncSource::External
            && self.transport == RuntimeTransportState::Playing
        {
            let mut out = Vec::new();
            let events = self.advance_algorithm(pulses)?;
            if !events.is_empty() {
                out.push(self.trigger_ui_pulse_message());
                out.push(RunnerMessage::MusicalEvents { events });
            }
            if let Some(pulse) = self.transport_ui_pulse_message() {
                out.push(pulse);
            }
            out.extend(self.messages_with_snapshot()?);
            return Ok(out);
        }
        self.messages_with_snapshot()
    }

    fn send_runtime_result(
        &mut self,
        result: crate::protocol::RuntimeStoreResult,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.apply_store_result(result)?;
        self.messages_with_snapshot()
    }

    fn should_ignore_external_start_stop(&self) -> bool {
        self.sync_source == SyncSource::External
            && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
    }
}

impl super::CoreRunner for NativeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = match message {
            HostMessage::TransportPulseStep {
                pulses,
                request_snapshot,
                ..
            } => self.send_transport_pulse_step(pulses, request_snapshot),
            HostMessage::DeviceInput { input } => self.send_device_input(input),
            HostMessage::MidiRealtimeStart => self.send_midi_realtime_start(),
            HostMessage::MidiRealtimeContinue => self.send_midi_realtime_continue(),
            HostMessage::MidiRealtimeStop => self.send_midi_realtime_stop(),
            HostMessage::MidiRealtimeClock { pulses } => self.send_midi_realtime_clock(pulses),
            HostMessage::RuntimeResult { result } => self.send_runtime_result(result),
        }?;
        messages.extend(self.flush_deferred_menu_apply()?);
        Ok(messages)
    }
}
