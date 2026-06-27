use crate::protocol::{
    RunnerMessage, RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState, RuntimeUiPulse,
};

use super::{NativeOledMode, NativeRunner, NativeToast};

impl NativeRunner {
    pub(super) fn trigger_ui_pulse_message(&self) -> RunnerMessage {
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TriggerPulse { duration_ms: 45 },
        }
    }

    pub(super) fn transport_ui_pulse_message(&self) -> Option<RunnerMessage> {
        if self.transport_flash_pulses_remaining != 6 {
            return None;
        }
        match self.transport_flash {
            "measure" | "beat" => Some(RunnerMessage::UiPulse {
                pulse: RuntimeUiPulse::TransportFlash {
                    flash: self.transport_flash.into(),
                    duration_ms: 90,
                },
            }),
            _ => None,
        }
    }

    pub(super) fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: RuntimeStatusState::Running,
            transport: self.transport.clone(),
            current_ppqn_pulse: self.current_ppqn_pulse,
            pending_resync: self.pending_resync,
            sync_source: self.sync_source.clone(),
            message: None,
        }
    }

    pub(super) fn messages_with_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.advance_oled_sleep_state();
        if self.oled_mode == NativeOledMode::Splash
            && self.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            self.startup_splash_presented = true;
        }
        self.advance_toast_state();
        let snapshot = self.next_snapshot()?;
        let autosave_pending = self.pending_autosave_payload_due_at.is_some();
        let save_default_effect =
            if self.auto_save_default && self.config_dirty && !autosave_pending {
                self.config_dirty = false;
                self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
                self.auto_save_flash_pulses_remaining = 8;
                self.toast = Some(NativeToast {
                    message: "Saved default".into(),
                    offset: 0,
                });
                Some(RuntimePlatformEffect::StoreSaveDefault {
                    payload: self.config_payload(),
                    mode: Some("deferred".into()),
                })
            } else {
                None
            };
        let mut messages = Vec::with_capacity(5);
        if let Some(effect) = save_default_effect {
            messages.push(RunnerMessage::PlatformEffects {
                effects: vec![effect],
            });
        }
        if self.outbox.has_platform_effects() {
            messages.push(RunnerMessage::PlatformEffects {
                effects: self.outbox.drain_platform_effects(),
            });
        }
        if self.outbox.has_audio_commands() {
            messages.push(RunnerMessage::AudioCommands {
                commands: self.outbox.drain_audio_commands(),
            });
        }
        messages.extend([
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus {
                status: self.status(),
            },
        ]);
        if self.auto_save_flash_pulses_remaining > 0 {
            self.auto_save_flash_pulses_remaining -= 1;
        }
        Ok(messages)
    }

    pub(super) fn messages_with_effects(
        &mut self,
        effects: Vec<RuntimePlatformEffect>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = vec![RunnerMessage::PlatformEffects { effects }];
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }

    pub(super) fn messages_with_input_result(
        &mut self,
        result: platform_core::NativeInputResult,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = Vec::new();
        self.apply_runtime_modulation(&result.mapped_intents, self.active_part_index);
        let events = self.apply_sampler_assignments(
            result.events,
            &result.mapped_intents,
            self.active_part_index,
            result.emitted_events.len(),
        );
        if !events.is_empty() {
            self.event_dot_on = true;
            self.event_dot_pulses_remaining = 1;
            messages.push(self.trigger_ui_pulse_message());
            messages.push(RunnerMessage::MusicalEvents { events });
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }
}
