use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeAudioCommand, RuntimePlatformEffect, RuntimeStatus,
    RuntimeTransportState, SyncSource,
};
use platform_core::MusicalEvent;
use serde_json::Value;

const PPQN: f64 = 24.0;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeConfig {
    pub bpm: f64,
    pub sync_source: SyncSource,
    pub midi_clock_out_enabled: bool,
    pub midi_out_enabled: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            sync_source: SyncSource::Internal,
            midi_clock_out_enabled: false,
            midi_out_enabled: true,
        }
    }
}

pub trait CoreRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String>;
}

pub trait HostAdapter {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), String>;

    fn handle_platform_effect(
        &mut self,
        effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String>;

    fn handle_audio_command(&mut self, command: &RuntimeAudioCommand) -> Result<(), String>;

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntime {
    config: RuntimeConfig,
    pulse_remainder: f64,
    now_ms: u64,
    last_status: Option<RuntimeStatus>,
    last_snapshot: Option<Value>,
    scheduled_note_offs: Vec<ScheduledMidiMessage>,
    request_next_snapshot: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct ScheduledMidiMessage {
    due_at_ms: u64,
    bytes: Vec<u8>,
}

impl PlaybackRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            pulse_remainder: 0.0,
            now_ms: 0,
            last_status: None,
            last_snapshot: None,
            scheduled_note_offs: Vec::new(),
            request_next_snapshot: false,
        }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: RuntimeConfig) {
        self.config = config;
    }

    pub fn request_next_snapshot(&mut self) {
        self.request_next_snapshot = true;
    }

    pub fn last_snapshot(&self) -> Option<&Value> {
        self.last_snapshot.as_ref()
    }

    pub fn last_status(&self) -> Option<&RuntimeStatus> {
        self.last_status.as_ref()
    }

    pub fn advance<R: CoreRunner, H: HostAdapter>(
        &mut self,
        elapsed_ms: u64,
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
        self.flush_scheduled_midi(host)?;
        if self.config.sync_source == SyncSource::External {
            return Ok(());
        }

        let pulses_per_ms = (self.config.bpm * PPQN) / 60_000.0;
        self.pulse_remainder += pulses_per_ms * (elapsed_ms as f64);
        let pulses = self.pulse_remainder.floor() as u32;
        self.pulse_remainder -= pulses as f64;
        if pulses == 0 {
            return Ok(());
        }

        let request_snapshot = if self.request_next_snapshot {
            self.request_next_snapshot = false;
            Some(true)
        } else {
            None
        };
        self.dispatch_host_message(
            HostMessage::TransportPulseStep {
                pulses,
                source: SyncSource::Internal,
                at_ppqn_pulse: self
                    .last_status
                    .as_ref()
                    .map(|status| status.current_ppqn_pulse),
                request_snapshot,
            },
            runner,
            host,
        )
    }

    pub fn handle_midi_realtime_bytes<R: CoreRunner, H: HostAdapter>(
        &mut self,
        bytes: &[u8],
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        let mut clock_pulses = 0_u32;
        for byte in bytes {
            match *byte {
                0xF8 => clock_pulses = clock_pulses.saturating_add(1),
                0xFA => self.dispatch_host_message(HostMessage::MidiRealtimeStart, runner, host)?,
                0xFB => {
                    self.dispatch_host_message(HostMessage::MidiRealtimeContinue, runner, host)?
                }
                0xFC => self.dispatch_host_message(HostMessage::MidiRealtimeStop, runner, host)?,
                _ => {}
            }
        }

        if clock_pulses > 0 {
            self.dispatch_host_message(
                HostMessage::MidiRealtimeClock {
                    pulses: clock_pulses,
                },
                runner,
                host,
            )?;
        }
        Ok(())
    }

    pub fn panic<H: HostAdapter>(&mut self, host: &mut H) -> Result<(), String> {
        self.scheduled_note_offs.clear();
        host.handle_midi_message(&[0xFC])?;
        for channel in 0..16_u8 {
            host.handle_midi_message(&[0xB0 | channel, 120, 0])?;
            host.handle_midi_message(&[0xB0 | channel, 123, 0])?;
        }
        Ok(())
    }

    pub fn ingest_runner_messages<H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        host: &mut H,
    ) -> Result<Vec<HostMessage>, String> {
        let follow_ups = self.process_runner_messages(messages, host)?;
        self.flush_scheduled_midi(host)?;
        Ok(follow_ups)
    }

    fn dispatch_host_message<R: CoreRunner, H: HostAdapter>(
        &mut self,
        message: HostMessage,
        runner: &mut R,
        host: &mut H,
    ) -> Result<(), String> {
        let mut queue = std::collections::VecDeque::from([message]);
        while let Some(next) = queue.pop_front() {
            let responses = runner.send(next)?;
            for follow_up in self.process_runner_messages(responses, host)? {
                queue.push_back(follow_up);
            }
        }
        self.flush_scheduled_midi(host)
    }

    fn process_runner_messages<H: HostAdapter>(
        &mut self,
        messages: Vec<RunnerMessage>,
        host: &mut H,
    ) -> Result<Vec<HostMessage>, String> {
        let mut follow_ups = Vec::new();
        for message in messages {
            match message {
                RunnerMessage::Snapshot { snapshot } => self.last_snapshot = Some(snapshot),
                RunnerMessage::PlatformEffects { effects } => {
                    for effect in effects {
                        for follow_up in host.handle_platform_effect(&effect)? {
                            follow_ups.push(follow_up);
                        }
                    }
                }
                RunnerMessage::MusicalEvents { events } => {
                    self.schedule_musical_events(events, host)?;
                }
                RunnerMessage::AudioCommands { commands } => {
                    for command in commands {
                        host.handle_audio_command(&command)?;
                    }
                }
                RunnerMessage::RuntimeStatus { status } => {
                    self.apply_runtime_status(status, host)?;
                }
            }
        }
        Ok(follow_ups)
    }

    fn schedule_musical_events<H: HostAdapter>(
        &mut self,
        events: Vec<MusicalEvent>,
        host: &mut H,
    ) -> Result<(), String> {
        for event in events {
            host.handle_musical_event(&event)?;

            if !self.config.midi_out_enabled {
                continue;
            }

            match event {
                MusicalEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                    duration_ms,
                } => {
                    host.handle_midi_message(&[
                        0x90 | (channel & 0x0F),
                        note.min(127),
                        velocity.clamp(1, 127),
                    ])?;
                    if let Some(duration_ms) = duration_ms {
                        self.scheduled_note_offs.push(ScheduledMidiMessage {
                            due_at_ms: self.now_ms.saturating_add(duration_ms as u64),
                            bytes: vec![0x80 | (channel & 0x0F), note.min(127), 0],
                        });
                    }
                }
                MusicalEvent::NoteOff { channel, note } => {
                    host.handle_midi_message(&[0x80 | (channel & 0x0F), note.min(127), 0])?;
                }
                MusicalEvent::Cc {
                    channel,
                    controller,
                    value,
                } => {
                    host.handle_midi_message(&[
                        0xB0 | (channel & 0x0F),
                        controller.min(127),
                        value.min(127),
                    ])?;
                }
            }
        }

        Ok(())
    }

    fn apply_runtime_status<H: HostAdapter>(
        &mut self,
        status: RuntimeStatus,
        host: &mut H,
    ) -> Result<(), String> {
        let previous = self.last_status.replace(status.clone());
        if !self.config.midi_out_enabled || status.sync_source != SyncSource::Internal {
            return Ok(());
        }

        if let Some(previous) = previous {
            if previous.transport != RuntimeTransportState::Playing
                && status.transport == RuntimeTransportState::Playing
            {
                let transport_byte = if previous.transport == RuntimeTransportState::Paused {
                    0xFB
                } else {
                    0xFA
                };
                host.handle_midi_message(&[transport_byte])?;
            } else if previous.transport == RuntimeTransportState::Playing
                && status.transport != RuntimeTransportState::Playing
            {
                host.handle_midi_message(&[0xFC])?;
            }

            if self.config.midi_clock_out_enabled
                && status.transport == RuntimeTransportState::Playing
                && status.current_ppqn_pulse > previous.current_ppqn_pulse
            {
                for _ in previous.current_ppqn_pulse..status.current_ppqn_pulse {
                    host.handle_midi_message(&[0xF8])?;
                }
            }
        } else if status.transport == RuntimeTransportState::Playing {
            host.handle_midi_message(&[0xFA])?;
            if self.config.midi_clock_out_enabled {
                for _ in 0..status.current_ppqn_pulse {
                    host.handle_midi_message(&[0xF8])?;
                }
            }
        }
        Ok(())
    }

    fn flush_scheduled_midi<H: HostAdapter>(&mut self, host: &mut H) -> Result<(), String> {
        self.scheduled_note_offs.sort_by_key(|msg| msg.due_at_ms);
        while self
            .scheduled_note_offs
            .first()
            .is_some_and(|message| message.due_at_ms <= self.now_ms)
        {
            let message = self.scheduled_note_offs.remove(0);
            host.handle_midi_message(&message.bytes)?;
        }
        Ok(())
    }
}
