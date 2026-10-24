use crate::protocol::RunnerMessage;

use super::{NativeRunner, RuntimeTransportState, SyncSource, TransportFlash};

const EXTERNAL_RESYNC_PPQN: u64 = 96;

impl NativeRunner {
    pub(super) fn send_transport_pulse_step(
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
        self.append_music_first_audio_commands(&mut out);
        if !events.is_empty() {
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
            if !events.cells.is_empty() {
                out.push(RunnerMessage::CellHits { hits: events.cells });
            }
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
        self.append_snapshot_and_status(&mut out, request_snapshot.unwrap_or(false))?;
        Ok(out)
    }

    pub(super) fn send_midi_realtime_start(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport.transport = RuntimeTransportState::Playing;
        self.reset_transport_position();
        self.prime_sequencer_layer_origins();
        let now = self.display.transients.now();
        self.display
            .transients
            .trigger_transport_flash(TransportFlash::Measure, now);
        self.messages_with_snapshot()
    }

    pub(super) fn send_midi_realtime_continue(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        let was_stopped = self.transport.transport == RuntimeTransportState::Stopped;
        self.transport.transport = RuntimeTransportState::Playing;
        if was_stopped {
            self.prime_sequencer_layer_origins();
        }
        self.messages_with_snapshot()
    }

    pub(super) fn send_midi_realtime_stop(&mut self) -> Result<Vec<RunnerMessage>, String> {
        if self.should_ignore_external_start_stop() {
            return self.messages_with_snapshot();
        }
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.messages_with_snapshot()
    }

    pub(super) fn send_transport_stop(&mut self) -> Result<Vec<RunnerMessage>, String> {
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.messages_with_snapshot()
    }

    pub(super) fn send_midi_realtime_clock(
        &mut self,
        pulses: u32,
    ) -> Result<Vec<RunnerMessage>, String> {
        if !self.midi_enabled
            || self.transport.sync_source != SyncSource::External
            || !self.midi_clock_in_enabled
            || self.transport.transport != RuntimeTransportState::Playing
        {
            return self.clock_status_messages();
        }
        self.send_external_clock_pulses(pulses)
    }

    fn send_external_clock_pulses(&mut self, pulses: u32) -> Result<Vec<RunnerMessage>, String> {
        if pulses == 0 {
            return self.clock_status_messages();
        }
        let mut remaining = pulses;
        let mut out = Vec::new();
        while remaining > 0 {
            if self.transport.pending_resync {
                let until_boundary = EXTERNAL_RESYNC_PPQN
                    - (self.transport.current_ppqn_pulse % EXTERNAL_RESYNC_PPQN);
                let before_boundary = remaining.min(
                    u32::try_from(until_boundary.saturating_sub(1))
                        .expect("PPQN boundary fits u32"),
                );
                if before_boundary > 0 {
                    self.transport.current_ppqn_pulse = self
                        .transport
                        .current_ppqn_pulse
                        .saturating_add(u64::from(before_boundary));
                    let events = self.advance_algorithm(before_boundary)?;
                    self.append_music_first_audio_commands(&mut out);
                    if !events.is_empty() {
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
                        if !events.cells.is_empty() {
                            out.push(RunnerMessage::CellHits { hits: events.cells });
                        }
                    }
                    out.extend(self.clock_status_messages()?);
                    remaining -= before_boundary;
                    continue;
                }

                remaining -= 1;
                self.reset_transport_position();
                self.prime_sequencer_layer_origins();
                self.append_pending_transpose_note_offs(&mut out);
                let events = self.advance_due_layer_ticks()?;
                self.append_music_first_audio_commands(&mut out);
                if !events.is_empty() {
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
                    if !events.cells.is_empty() {
                        out.push(RunnerMessage::CellHits { hits: events.cells });
                    }
                }
                out.extend(self.clock_status_messages()?);
                continue;
            }

            let chunk = remaining;
            self.transport.current_ppqn_pulse = self
                .transport
                .current_ppqn_pulse
                .saturating_add(u64::from(chunk));
            let events = self.advance_algorithm(chunk)?;
            self.append_music_first_audio_commands(&mut out);
            if !events.is_empty() {
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
                if !events.cells.is_empty() {
                    out.push(RunnerMessage::CellHits { hits: events.cells });
                }
            }
            out.extend(self.clock_status_messages()?);
            remaining -= chunk;
        }
        Ok(out)
    }

    pub(super) fn send_runtime_result(
        &mut self,
        result: crate::protocol::RuntimeStoreResult,
    ) -> Result<Vec<RunnerMessage>, String> {
        let had_runtime_error = self.display.runtime_error_presentation.is_some();
        let save_flash_serial = self.display.auto_save_flash_serial;
        self.apply_store_result(result)?;
        if self.pending.presentation_deferred {
            if !had_runtime_error && self.display.runtime_error_presentation.is_some() {
                self.require_synchronous_action_presentation();
                self.messages_with_snapshot()
            } else {
                if self.display.auto_save_flash_serial != save_flash_serial {
                    self.display.transients.mark_presentation_due();
                }
                self.messages_without_presentation()
            }
        } else {
            self.messages_with_snapshot()
        }
    }

    fn should_ignore_external_start_stop(&self) -> bool {
        !self.midi_enabled
            || (self.transport.sync_source == SyncSource::External
                && (!self.midi_clock_in_enabled || !self.midi_respond_to_start_stop))
    }
}
