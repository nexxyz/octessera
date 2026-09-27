use crate::protocol::{RunnerMessage, RuntimePlatformEffect, RuntimeStatus, RuntimeStatusState};

use super::algorithm::LinkRoutingInput;
use super::restart_settings::DefaultSaveScope;
use super::{NativeOledMode, NativeRunner};

impl NativeRunner {
    pub(super) fn append_music_first_audio_commands(&mut self, messages: &mut Vec<RunnerMessage>) {
        if !self.pending.presentation_deferred {
            return;
        }
        self.queue_audio_config_if_changed();
        if self.outbox.has_audio_commands() {
            messages.push(RunnerMessage::AudioCommands {
                commands: self.outbox.drain_audio_commands(),
            });
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
        let now = self.display.transients.now();
        self.display.transients.advance(now);
        if self.pending.presentation_deferred
            && self.transport.transport == super::RuntimeTransportState::Playing
        {
            if !self.pending.suppress_snapshot_response {
                self.display.transients.mark_presentation_due();
            }
            return self.messages_without_presentation();
        }
        if self.pending.suppress_snapshot_response {
            return self.messages_without_snapshot();
        }
        self.messages_with_snapshot_response()
    }

    pub(super) fn clock_status_messages(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.pending.presentation_deferred
            && self.transport.transport == super::RuntimeTransportState::Playing
        {
            self.display
                .transients
                .advance(self.display.transients.now());
            return self.messages_without_presentation();
        }
        self.messages_with_snapshot()
    }

    pub(super) fn messages_with_forced_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        let suppress_snapshot_response = self.pending.suppress_snapshot_response;
        self.pending.suppress_snapshot_response = false;
        let result = self.messages_with_snapshot_response();
        self.pending.suppress_snapshot_response = suppress_snapshot_response;
        result
    }

    fn messages_with_snapshot_response(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.pending.presentation_deferred
            && self.transport.transport == super::RuntimeTransportState::Playing
        {
            self.display.transients.mark_presentation_due();
            return self.messages_without_presentation();
        }
        let now = self.display.transients.now();
        self.display.transients.advance(now);
        self.advance_oled_sleep_state();
        if self.display.oled_mode == NativeOledMode::Splash
            && self.display.oled_splash_text == super::OLED_STARTUP_SPLASH_KEY
        {
            self.display.startup_splash_presented = true;
        }
        self.advance_toast_state();
        let snapshot = self.next_snapshot()?;
        self.display.transients.acknowledge_snapshot_pending();
        let persistence_effects = self.pending_persistence_effects();
        let mut messages = Vec::with_capacity(5);
        self.append_pending_transpose_note_offs(&mut messages);
        self.append_pending_drum_hits(&mut messages);
        for effect in persistence_effects {
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

    pub(super) fn pending_persistence_effects(&mut self) -> Vec<RuntimePlatformEffect> {
        if self.pending.external_autosave_deferred {
            return Vec::new();
        }
        let restore_blocks_config_writes = self.restore_blocks_config_writes();
        let autosave_pending = self.pending.pending_autosave_payload_due_at.is_some();
        let backup_due = !restore_blocks_config_writes
            && self.config_dirty
            && self.rolling_backups
            && !autosave_pending
            && self
                .last_backup_save_at
                .map(|last| last.elapsed() >= std::time::Duration::from_secs(300))
                .unwrap_or(true);
        let save_pending = self.restart_settings.has_pending_write()
            || self
                .pending
                .pending_save_revision
                .zip(self.dirty_revision)
                .is_some_and(|(pending, dirty)| pending == dirty);
        let payload = if (self.auto_save_default
            && !restore_blocks_config_writes
            && self.config_dirty
            && !autosave_pending
            && !save_pending)
            || backup_due
        {
            Some(self.config_payload())
        } else {
            None
        };
        let mut effects = Vec::with_capacity(2);
        if self.auto_save_default
            && !restore_blocks_config_writes
            && self.config_dirty
            && !self.restart_settings.is_editing()
            && !autosave_pending
            && !save_pending
        {
            let autosave_payload = payload.clone().expect("autosave payload");
            if self.register_default_write(autosave_payload, DefaultSaveScope::Autosave) {
                self.pending.pending_save_revision = Some(self.config_revision);
                effects.push(RuntimePlatformEffect::StoreSaveDefault {
                    payload: payload.clone().expect("autosave payload"),
                    mode: Some("deferred".into()),
                });
            }
        }
        if backup_due {
            self.last_backup_save_at = Some(std::time::Instant::now());
            effects.push(RuntimePlatformEffect::StoreSaveBackup {
                payload: payload.expect("backup payload"),
            });
        }
        effects
    }

    pub(super) fn messages_without_snapshot(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.pending.presentation_deferred
            && self.transport.transport == super::RuntimeTransportState::Playing
        {
            return self.messages_without_presentation();
        }
        self.queue_audio_config_if_changed();
        let mut messages = self.drain_pending_output();
        self.append_snapshot_and_status(&mut messages, false)?;
        Ok(messages)
    }

    pub(super) fn append_snapshot_and_status(
        &mut self,
        messages: &mut Vec<RunnerMessage>,
        requested: bool,
    ) -> Result<(), String> {
        let now = self.display.transients.now();
        self.display.transients.advance(now);
        if self.pending.presentation_deferred
            && self.transport.transport == super::RuntimeTransportState::Playing
        {
            if requested {
                self.display.transients.mark_presentation_due();
            }
            messages.push(RunnerMessage::RuntimeStatus {
                status: self.status(),
            });
            return Ok(());
        }
        let pending = self.display.transients.snapshot_pending();
        if requested || pending {
            self.advance_oled_sleep_state();
            self.advance_toast_state();
            let snapshot = self.snapshot()?;
            self.display.transients.acknowledge_snapshot_pending();
            messages.push(RunnerMessage::Snapshot { snapshot });
        }
        messages.push(RunnerMessage::RuntimeStatus {
            status: self.status(),
        });
        Ok(())
    }

    pub(super) fn messages_without_presentation(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.queue_audio_config_if_changed();
        let mut messages = self.drain_pending_output();
        messages.push(RunnerMessage::RuntimeStatus {
            status: self.status(),
        });
        Ok(messages)
    }

    fn drain_pending_output(&mut self) -> Vec<RunnerMessage> {
        let mut messages = Vec::with_capacity(4);
        self.append_pending_transpose_note_offs(&mut messages);
        self.append_pending_drum_hits(&mut messages);
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
        messages
    }

    pub(super) fn append_pending_transpose_note_offs(&mut self, messages: &mut Vec<RunnerMessage>) {
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
        if !events.drum.is_empty() {
            messages.push(RunnerMessage::DrumHits { hits: events.drum });
        }
    }

    fn append_pending_drum_hits(&mut self, messages: &mut Vec<RunnerMessage>) {
        if !self.pending.drum_hits.is_empty() {
            messages.push(RunnerMessage::DrumHits {
                hits: std::mem::take(&mut self.pending.drum_hits),
            });
        }
    }

    pub(super) fn messages_with_effects(
        &mut self,
        effects: Vec<RuntimePlatformEffect>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let effects = effects
            .into_iter()
            .map(|effect| self.outbox.stamp_platform_effect(effect))
            .collect::<Vec<_>>();
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
            .play_transpose_offsets_for_routing()
            .get(self.active_layer_index)
            .copied()
            .unwrap_or(0);
        let instruments = self.instruments.clone();
        let sense = self.link_layers.get(self.active_layer_index).cloned();
        let events = self.route_events_with_link_timing(
            self.active_layer_index,
            LinkRoutingInput {
                events: result.events,
                event_intents: &result.event_intents,
                instruments: &instruments,
                sense,
                transpose_offset,
            },
        )?;
        self.track_emitted_route_notes(self.active_layer_index, &events);
        self.append_music_first_audio_commands(&mut messages);
        if !events.is_empty() {
            let now = self.display.transients.now();
            self.display.transients.trigger_event_dot(now);
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
            if !events.drum.is_empty() {
                messages.push(RunnerMessage::DrumHits { hits: events.drum });
            }
        }
        messages.extend(self.messages_with_snapshot()?);
        Ok(messages)
    }

    pub(super) fn messages_with_routed_events(
        &mut self,
        events: super::RoutedMusicalEvents,
    ) -> Result<Vec<RunnerMessage>, String> {
        self.track_emitted_route_notes(self.active_layer_index, &events);
        let mut messages = Vec::new();
        self.append_music_first_audio_commands(&mut messages);
        if !events.is_empty() {
            let now = self.display.transients.now();
            self.display.transients.trigger_event_dot(now);
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
            if !events.drum.is_empty() {
                messages.push(RunnerMessage::DrumHits { hits: events.drum });
            }
        }
        Ok(messages)
    }
}
