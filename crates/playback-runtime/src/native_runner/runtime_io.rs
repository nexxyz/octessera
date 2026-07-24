use crate::protocol::{HostMessage, RunnerMessage};
use std::time::Instant;

use super::{
    wrap_help_text, DeviceInput, NativeHelpPopup, NativeRunner, NativeSystemInfoModal, NativeToast,
    NativeUsbSdTransferModal, RuntimeTransportState, SyncSource,
};

const OLED_HELP_LINE_WIDTH: usize = 18;
const EXTERNAL_RESYNC_PPQN: u64 = 96;

impl NativeRunner {
    pub(super) fn open_controls_help(&mut self) {
        self.display.help_popup = Some(NativeHelpPopup {
            title: "Help: Basic Help".into(),
            lines: wrap_help_text(
                "Help Sh+Fn+Main. Back Back. Play/Pause Space. Stop/Sync Sh+Space. Reset Stop Fn+Space. Fn nav: left layer, right Play. Sh+Fn layer mutes. Fn+Aux alt bind.",
                OLED_HELP_LINE_WIDTH,
            ),
            scroll: 0,
        });
    }

    pub(super) fn open_system_info(&mut self) {
        self.display.system_info_modal = Some(NativeSystemInfoModal::loading());
    }

    pub(super) fn open_contextual_help(&mut self) {
        let Some(target) = self.menu.current_help_target() else {
            self.display.toast = Some(NativeToast {
                message: "Missing help target".into(),
                offset: 0,
            });
            return;
        };
        let Some(help) = crate::native_help::resolve_native_help(&target) else {
            self.display.toast = Some(NativeToast {
                message: format!("Missing help: {}", target.label),
                offset: 0,
            });
            return;
        };
        let title = format!("Help: {}", help.title);
        self.display.help_popup = Some(NativeHelpPopup {
            title,
            lines: wrap_help_text(&help.detail, OLED_HELP_LINE_WIDTH),
            scroll: 0,
        });
    }

    pub(super) fn turn_help_popup(&mut self, delta: i8) {
        let Some(help) = &mut self.display.help_popup else {
            return;
        };
        let max_scroll = help.lines.len().saturating_sub(super::OLED_BODY_ROWS - 1);
        let next = (help.scroll as isize + delta as isize).clamp(0, max_scroll as isize) as usize;
        help.scroll = next;
    }

    pub(super) fn turn_confirm_dialog(&mut self, delta: i8) {
        let Some(confirm) = &mut self.display.confirm_dialog else {
            return;
        };
        let max = confirm.options.len().saturating_sub(1);
        confirm.cursor = (confirm.cursor as isize + delta as isize).clamp(0, max as isize) as usize;
    }

    pub(super) fn open_usb_sd_transfer_modal(&mut self) {
        self.display.usb_sd_transfer_modal = Some(NativeUsbSdTransferModal {
            title: "SD2 Transfer".into(),
            lines: wrap_help_text(
                "SD2 active/waiting. Eject on host, then Back/Main to stop.",
                OLED_HELP_LINE_WIDTH,
            ),
        });
    }

    pub(super) fn confirm_dialog_selection(
        &mut self,
    ) -> Result<Option<super::RuntimePlatformEffect>, String> {
        let Some(confirm) = self.display.confirm_dialog.take() else {
            return Ok(None);
        };
        if confirm.cursor == 0 {
            if let Some(message) = confirm.cancel_toast {
                self.display.toast = Some(NativeToast { message, offset: 0 });
            }
            return Ok(None);
        }
        if confirm.confirm_before_execute {
            self.display.confirm_dialog = self.confirmation_for_action(&confirm.action);
            return Ok(None);
        }
        self.execute_confirmed_action(confirm.action)
    }

    fn send_transport_pulse_step(
        &mut self,
        pulses: u32,
        request_snapshot: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.transport.current_ppqn_pulse = self
            .transport
            .current_ppqn_pulse
            .saturating_add(pulses as u64);
        let mut out = Vec::new();
        let events = self.advance_algorithm(pulses)?;
        if !events.is_empty() {
            out.push(self.trigger_ui_pulse_message());
            if !events.audio.is_empty() {
                out.push(RunnerMessage::MusicalEvents {
                    events: events.audio,
                });
            }
            if !events.midi.is_empty() {
                out.push(RunnerMessage::MidiEvents {
                    events: events.midi,
                });
            }
        }
        if let Some(pulse) = self.transport_ui_pulse_message() {
            out.push(pulse);
        }
        if self.outbox.has_platform_effects() {
            out.push(RunnerMessage::PlatformEffects {
                effects: self.outbox.drain_platform_effects(),
            });
        }
        if self.outbox.has_audio_commands() {
            out.push(RunnerMessage::AudioCommands {
                commands: self.outbox.drain_audio_commands(),
            });
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

    fn send_midi_realtime_start(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport.transport = RuntimeTransportState::Playing;
        self.reset_transport_position();
        self.display.transport_flash = "measure";
        self.display.transport_flash_pulses_remaining = 6;
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
        self.transport.transport = RuntimeTransportState::Playing;
        self.messages_with_snapshot()
    }

    fn send_midi_realtime_stop(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.messages_with_snapshot()
    }

    fn send_transport_stop(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.messages_with_snapshot()
    }

    fn send_midi_realtime_clock(&mut self, pulses: u32) -> Result<Vec<RunnerMessage>, String> {
        if self.transport.sync_source == SyncSource::External && !self.midi_clock_in_enabled {
            return self.messages_with_snapshot();
        }
        if self.transport.sync_source == SyncSource::External
            && self.transport.transport == RuntimeTransportState::Playing
        {
            return self.send_external_clock_pulses(pulses);
        }
        self.transport.current_ppqn_pulse = self
            .transport
            .current_ppqn_pulse
            .saturating_add(pulses as u64);
        self.messages_with_snapshot()
    }

    fn send_external_clock_pulses(&mut self, pulses: u32) -> Result<Vec<RunnerMessage>, String> {
        if pulses == 0 {
            return self.messages_with_snapshot();
        }
        let mut remaining = pulses;
        let mut out = Vec::new();
        while remaining > 0 {
            let chunk = if self.transport.pending_resync {
                let until_boundary = EXTERNAL_RESYNC_PPQN
                    - (self.transport.current_ppqn_pulse % EXTERNAL_RESYNC_PPQN);
                remaining.min(until_boundary as u32)
            } else {
                remaining
            };
            self.transport.current_ppqn_pulse = self
                .transport
                .current_ppqn_pulse
                .saturating_add(u64::from(chunk));
            let events = self.advance_algorithm(chunk)?;
            if !events.is_empty() {
                out.push(self.trigger_ui_pulse_message());
                if !events.audio.is_empty() {
                    out.push(RunnerMessage::MusicalEvents {
                        events: events.audio,
                    });
                }
                if !events.midi.is_empty() {
                    out.push(RunnerMessage::MidiEvents {
                        events: events.midi,
                    });
                }
            }
            if let Some(pulse) = self.transport_ui_pulse_message() {
                out.push(pulse);
            }
            out.extend(self.messages_with_snapshot()?);
            remaining -= chunk;
            if self.transport.pending_resync
                && self
                    .transport
                    .current_ppqn_pulse
                    .is_multiple_of(EXTERNAL_RESYNC_PPQN)
            {
                self.reset_transport_position();
                out.extend(self.messages_with_snapshot()?);
            }
        }
        Ok(out)
    }

    fn send_runtime_result(
        &mut self,
        result: crate::protocol::RuntimeStoreResult,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.apply_store_result(result)?;
        self.messages_with_snapshot()
    }

    fn should_ignore_external_start_stop(&self) -> bool {
        self.transport.sync_source == SyncSource::External
            && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop)
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
