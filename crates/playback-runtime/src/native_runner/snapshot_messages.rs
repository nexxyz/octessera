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
        if self.suppress_snapshot_response {
            return self.messages_without_snapshot();
        }
        self.advance_oled_sleep_state();
        if self.oled_mode == NativeOledMode::Splash
            && self.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            self.startup_splash_presented = true;
        }
        self.advance_toast_state();
        let snapshot = self.next_snapshot()?;
        let autosave_pending = self.pending_autosave_payload_due_at.is_some();
        let backup_due = self.config_dirty
            && self.rolling_backups
            && !autosave_pending
            && self
                .last_backup_save_at
                .map(|last| last.elapsed() >= std::time::Duration::from_secs(300))
                .unwrap_or(true);
        let payload =
            if (self.auto_save_default && self.config_dirty && !autosave_pending) || backup_due {
                Some(self.config_payload())
            } else {
                None
            };
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
                    payload: payload.clone().expect("autosave payload"),
                    mode: Some("deferred".into()),
                })
            } else {
                None
            };
        let backup_effect = if backup_due {
            self.last_backup_save_at = Some(std::time::Instant::now());
            Some(RuntimePlatformEffect::StoreSaveBackup {
                payload: payload.expect("backup payload"),
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
        if let Some(effect) = backup_effect {
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

    pub(super) fn messages_without_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.queue_audio_config_if_changed();
        let mut messages = Vec::with_capacity(4);
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
        messages.push(RunnerMessage::RuntimeStatus {
            status: self.status(),
        });
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
        self.apply_runtime_modulation(&result.mapped_intents, self.active_layer_index);
        let transpose_offset = self
            .sparks_transpose_offsets_for_routing()
            .get(self.active_layer_index)
            .copied()
            .unwrap_or(0);
        let active_transpose_notes = self
            .sparks_transpose_active_notes
            .get_mut(self.active_layer_index);
        let events = super::modulation::apply_sampler_assignments_for_instruments_routed(
            result.events,
            &result.mapped_intents,
            result.emitted_events.len(),
            &self.instruments,
            self.pulses_layers.get(self.active_layer_index),
            transpose_offset,
            active_transpose_notes,
        );
        if !events.is_empty() {
            self.event_dot_on = true;
            self.event_dot_pulses_remaining = 1;
            messages.push(self.trigger_ui_pulse_message());
            if !events.audio.is_empty() {
                messages.push(RunnerMessage::MusicalEvents {
                    events: events.audio,
                });
            }
            if !events.midi.is_empty() {
                messages.push(RunnerMessage::MidiEvents {
                    events: events.midi,
                });
            }
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }
}
