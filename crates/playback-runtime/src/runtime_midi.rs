use super::{HostAdapter, PlaybackRuntime, ScheduledMidiMessage};
use crate::protocol::{
    RuntimeAdapterError, RuntimeErrorDomain, RuntimeOperation, RuntimeStatus, RuntimeStatusState,
    RuntimeTransportState, SyncSource,
};
use platform_core::MusicalEvent;

impl PlaybackRuntime {
    pub(super) fn schedule_musical_events<H: HostAdapter>(
        &mut self,
        events: Vec<MusicalEvent>,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        for event in events {
            match host.handle_musical_event(&event) {
                Ok(()) => self.clear_error(RuntimeOperation::MusicalEvent),
                Err(error) => {
                    return Err(RuntimeAdapterError::from_facts(error.facts.with_context(
                        RuntimeErrorDomain::Audio,
                        RuntimeOperation::MusicalEvent,
                        None,
                        None,
                    )))
                }
            }
            if self.config.midi_out_enabled {
                self.send_musical_event_midi(event, host)?;
            }
        }
        Ok(())
    }

    pub(super) fn schedule_midi_events<H: HostAdapter>(
        &mut self,
        events: Vec<MusicalEvent>,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        if self.config.midi_out_enabled {
            for event in events {
                self.send_musical_event_midi(event, host)?;
            }
        }
        Ok(())
    }

    fn send_musical_event_midi<H: HostAdapter>(
        &mut self,
        event: MusicalEvent,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                self.send_midi_message(
                    &[
                        0x90 | (channel & 0x0F),
                        note.min(127),
                        velocity.clamp(1, 127),
                    ],
                    RuntimeOperation::MidiEvent,
                    host,
                )?;
                if let Some(duration_ms) = duration_ms {
                    self.scheduled_note_offs.push_back(ScheduledMidiMessage {
                        due_at_ms: self.now_ms.saturating_add(duration_ms as u64),
                        bytes: vec![0x80 | (channel & 0x0F), note.min(127), 0],
                    });
                    self.scheduled_note_offs_dirty = true;
                }
            }
            MusicalEvent::NoteOff { channel, note } => {
                self.send_midi_message(
                    &[0x80 | (channel & 0x0F), note.min(127), 0],
                    RuntimeOperation::MidiEvent,
                    host,
                )?;
            }
            MusicalEvent::Cc {
                channel,
                controller,
                value,
            } => {
                self.send_midi_message(
                    &[0xB0 | (channel & 0x0F), controller.min(127), value.min(127)],
                    RuntimeOperation::MidiEvent,
                    host,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_runtime_status<H: HostAdapter>(
        &mut self,
        status: RuntimeStatus,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        if let Some(error) = status.error.clone() {
            if self.last_good_status.is_none() {
                let mut baseline = status.clone();
                baseline.error = None;
                baseline.message = None;
                baseline.state = match baseline.transport {
                    RuntimeTransportState::Stopped => RuntimeStatusState::Idle,
                    RuntimeTransportState::Playing => RuntimeStatusState::Running,
                    RuntimeTransportState::Paused => RuntimeStatusState::Paused,
                };
                self.last_good_status = Some(baseline);
            }
            self.latch_error(error);
            return Ok(());
        }
        let previous = self.last_good_status.replace(status.clone());
        self.refresh_presentations();
        if !self.config.midi_out_enabled || status.sync_source != SyncSource::Internal {
            return Ok(());
        }
        self.send_transport_midi(previous, &status, host)
    }

    fn send_transport_midi<H: HostAdapter>(
        &mut self,
        previous: Option<RuntimeStatus>,
        status: &RuntimeStatus,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        if let Some(previous) = previous {
            if previous.transport != RuntimeTransportState::Playing
                && status.transport == RuntimeTransportState::Playing
            {
                self.send_midi_message(
                    &[if previous.transport == RuntimeTransportState::Paused {
                        0xFB
                    } else {
                        0xFA
                    }],
                    RuntimeOperation::MidiMessage,
                    host,
                )?;
            } else if previous.transport == RuntimeTransportState::Playing
                && status.transport != RuntimeTransportState::Playing
            {
                self.send_midi_message(&[0xFC], RuntimeOperation::MidiMessage, host)?;
            }
            self.send_clock_midi(previous.current_ppqn_pulse, status, host)?;
        } else if status.transport == RuntimeTransportState::Playing {
            self.send_midi_message(&[0xFA], RuntimeOperation::MidiMessage, host)?;
            self.send_clock_midi(0, status, host)?;
        }
        Ok(())
    }

    pub(super) fn send_midi_message<H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        operation: RuntimeOperation,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        match host.handle_midi_message(bytes) {
            Ok(()) => {
                self.clear_error(operation);
                Ok(())
            }
            Err(error) => Err(RuntimeAdapterError::from_facts(error.facts.with_context(
                RuntimeErrorDomain::Midi,
                operation,
                None,
                None,
            ))),
        }
    }

    fn send_clock_midi<H: HostAdapter>(
        &mut self,
        from_ppqn_pulse: u64,
        status: &RuntimeStatus,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        if self.config.midi_clock_out_enabled
            && status.transport == RuntimeTransportState::Playing
            && status.current_ppqn_pulse > from_ppqn_pulse
        {
            for _ in from_ppqn_pulse..status.current_ppqn_pulse {
                self.send_midi_message(&[0xF8], RuntimeOperation::MidiMessage, host)?;
            }
        }
        Ok(())
    }

    pub(super) fn flush_scheduled_midi<H: HostAdapter>(
        &mut self,
        host: &mut H,
    ) -> Result<(), RuntimeAdapterError> {
        if self.scheduled_note_offs_dirty {
            self.scheduled_note_offs
                .make_contiguous()
                .sort_by_key(|msg| msg.due_at_ms);
            self.scheduled_note_offs_dirty = false;
        }
        while self
            .scheduled_note_offs
            .front()
            .is_some_and(|message| message.due_at_ms <= self.now_ms)
        {
            let message = self.scheduled_note_offs.pop_front().expect("front checked");
            self.send_midi_message(&message.bytes, RuntimeOperation::MidiMessage, host)?;
        }
        Ok(())
    }
}
