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
}

impl super::CoreRunner for NativeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        match message {
            HostMessage::TransportPulseStep {
                pulses,
                request_snapshot,
                ..
            } => {
                self.current_ppqn_pulse = self.current_ppqn_pulse.saturating_add(pulses as u64);
                let mut out = Vec::new();
                let events = self.advance_algorithm(pulses)?;
                if !events.is_empty() {
                    out.push(RunnerMessage::MusicalEvents { events });
                }
                if request_snapshot.unwrap_or(false) {
                    out.push(RunnerMessage::Snapshot {
                        snapshot: self.snapshot()?,
                    });
                }
                out.push(RunnerMessage::RuntimeStatus {
                    status: self.status(),
                });
                Ok(out)
            }
            HostMessage::DeviceInput { input } => {
                let input =
                    serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
                self.handle_device_input(input)
            }
            HostMessage::MidiRealtimeStart => {
                if self.sync_source == SyncSource::External
                    && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
                {
                    return self.messages_with_snapshot();
                }
                self.transport = RuntimeTransportState::Playing;
                self.current_ppqn_pulse = 0;
                self.algorithm_pulse_accumulator = 0;
                self.transport_flash = "measure";
                self.transport_flash_pulses_remaining = 6;
                self.messages_with_snapshot()
            }
            HostMessage::MidiRealtimeContinue => {
                if self.sync_source == SyncSource::External
                    && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
                {
                    return self.messages_with_snapshot();
                }
                self.transport = RuntimeTransportState::Playing;
                self.messages_with_snapshot()
            }
            HostMessage::MidiRealtimeStop => {
                if self.sync_source == SyncSource::External
                    && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
                {
                    return self.messages_with_snapshot();
                }
                self.transport = RuntimeTransportState::Stopped;
                self.transport_flash = "none";
                self.transport_flash_pulses_remaining = 0;
                self.event_dot_on = false;
                self.event_dot_pulses_remaining = 0;
                self.messages_with_snapshot()
            }
            HostMessage::MidiRealtimeClock { pulses } => {
                if self.sync_source == SyncSource::External && !self.midi_clock_in_enabled {
                    return self.messages_with_snapshot();
                }
                self.current_ppqn_pulse = self.current_ppqn_pulse.saturating_add(pulses as u64);
                if self.sync_source == SyncSource::External
                    && self.transport == RuntimeTransportState::Playing
                {
                    let events = self.advance_algorithm(pulses)?;
                    let mut out = Vec::new();
                    if !events.is_empty() {
                        out.push(RunnerMessage::MusicalEvents { events });
                    }
                    out.extend(self.messages_with_snapshot()?);
                    return Ok(out);
                }
                self.messages_with_snapshot()
            }
            HostMessage::RuntimeResult { result } => {
                self.apply_store_result(result)?;
                self.messages_with_snapshot()
            }
        }
    }
}
