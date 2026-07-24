use crate::protocol::{
    RunnerMessage, RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState, RuntimeUiPulse,
};

use super::algorithm::LinkRoutingInput;
use super::{NativeOledMode, NativeRunner};

impl NativeRunner {
    pub(super) fn trigger_ui_pulse_message(&self) -> RunnerMessage {
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TriggerPulse { duration_ms: 45 },
        }
    }

    pub(super) fn transport_ui_pulse_message(&self) -> Option<RunnerMessage> {
        if self.display.transport_flash_pulses_remaining != 6 {
            return None;
        }
        match self.display.transport_flash {
            "measure" | "beat" => Some(RunnerMessage::UiPulse {
                pulse: RuntimeUiPulse::TransportFlash {
                    flash: self.display.transport_flash.into(),
                    duration_ms: 90,
                },
            }),
            _ => None,
        }
    }

    pub(super) fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: RuntimeStatusState::Running,
            transport: self.transport.transport.clone(),
            current_ppqn_pulse: self.transport.current_ppqn_pulse,
            pending_resync: self.transport.pending_resync,
            sync_source: self.transport.sync_source.clone(),
            message: None,
            error: None,
        }
    }

    pub fn messages_with_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.pending.suppress_snapshot_response {
            return self.messages_without_snapshot();
        }
        self.messages_with_snapshot_response()
    }

    pub(super) fn messages_with_forced_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        let suppress_snapshot_response = self.pending.suppress_snapshot_response;
        self.pending.suppress_snapshot_response = false;
        let result = self.messages_with_snapshot_response();
        self.pending.suppress_snapshot_response = suppress_snapshot_response;
        result
    }

    fn messages_with_snapshot_response(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.advance_oled_sleep_state();
        if self.display.oled_mode == NativeOledMode::Splash
            && self.display.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            self.display.startup_splash_presented = true;
        }
        self.advance_toast_state();
        let snapshot = self.next_snapshot()?;
        let autosave_pending = self.pending.pending_autosave_payload_due_at.is_some();
        let backup_due = self.config_dirty
            && self.rolling_backups
            && !autosave_pending
            && self
                .last_backup_save_at
                .map(|last| last.elapsed() >= std::time::Duration::from_secs(300))
                .unwrap_or(true);
        let save_pending = self
            .pending
            .pending_save_revision
            .zip(self.dirty_revision)
            .is_some_and(|(pending, dirty)| pending == dirty);
        let payload =
            if (self.auto_save_default && self.config_dirty && !autosave_pending && !save_pending)
                || backup_due
            {
                Some(self.config_payload())
            } else {
                None
            };
        let save_default_effect =
            if self.auto_save_default && self.config_dirty && !autosave_pending && !save_pending {
                self.pending.pending_save_revision = Some(self.config_revision);
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
        self.append_pending_transpose_note_offs(&mut messages);
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
        Ok(messages)
    }

    pub(super) fn messages_without_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.queue_audio_config_if_changed();
        let mut messages = Vec::with_capacity(4);
        self.append_pending_transpose_note_offs(&mut messages);
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

    fn append_pending_transpose_note_offs(&mut self, messages: &mut Vec<RunnerMessage>) {
        let events = std::mem::take(&mut self.pending_transpose_note_offs);
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

    pub(super) fn messages_with_effects(
        &mut self,
        effects: Vec<RuntimePlatformEffect>,
    ) -> Result<Vec<RunnerMessage>, String> {
        if effects
            .iter()
            .any(|effect| matches!(effect, RuntimePlatformEffect::AudioCommand { .. }))
        {
            let mut messages = self.messages_with_snapshot()?;
            messages.push(RunnerMessage::PlatformEffects { effects });
            return Ok(messages);
        }
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
        let instruments = self.instruments.clone();
        let sense = self.pulses_layers.get(self.active_layer_index).cloned();
        let mut events = self.route_events_with_link_timing(
            self.active_layer_index,
            LinkRoutingInput {
                events: result.events,
                event_intents: &result.event_intents,
                instruments: &instruments,
                sense,
                transpose_offset,
            },
        )?;
        events.dedupe_note_ons_by_highest_velocity();
        if !events.is_empty() {
            self.display.event_dot_on = true;
            self.display.event_dot_pulses_remaining = 1;
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

    pub(super) fn messages_with_routed_events(
        &mut self,
        events: super::RoutedMusicalEvents,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = Vec::new();
        if !events.is_empty() {
            self.display.event_dot_on = true;
            self.display.event_dot_pulses_remaining = 1;
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
        Ok(messages)
    }
}
