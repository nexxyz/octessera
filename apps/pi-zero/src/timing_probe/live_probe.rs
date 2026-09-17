use super::cadence::AdvanceCorrelation;
use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent, NativeRunner, RunnerMessage, RuntimeAudioCommand,
    RuntimePlatformRequest,
};
use serde::Serialize;
use serde_json::Value;
use std::time::Instant;

#[derive(Serialize)]
pub(super) struct LiveTimingProbeReport {
    pub(super) scenario: playback_runtime::TimingProbeScenario,
    pub(super) duration_ms: u64,
    pub(super) measured_duration_ms: u64,
    pub(super) wake_interval_ms: u64,
    pub(super) force_snapshots: bool,
    pub(super) output_buffer_frames: u32,
    pub(super) internal_block_frames: usize,
    pub(super) events: usize,
    pub(super) event_intervals_us: LiveSummary,
    pub(super) primary_stream: Option<LiveStreamReport>,
    pub(super) event_producing_advances: usize,
    pub(super) multi_pulse_advances: usize,
    pub(super) multi_pulse_event_producing_advances: usize,
    pub(super) pulses_on_event_producing_advances: LiveCountSummary,
    pub(super) wake_late_us: LiveSummary,
    pub(super) advance_us: LiveSummary,
    pub(super) loop_us: LiveSummary,
    pub(super) audio_send_us: LiveSummary,
    pub(super) runner_send_us: LiveSummary,
    pub(super) slow_sends: Vec<SlowSendReport>,
    pub(super) event_batches: LiveCountSummary,
    pub(super) audio_commands: u64,
    pub(super) platform_effects: u64,
    pub(super) midi_messages: u64,
    pub(super) playing_statuses: u64,
}

#[derive(Serialize)]
pub(super) struct LiveStreamReport {
    pub(super) key: String,
    pub(super) events: usize,
    pub(super) intervals_us: LiveSummary,
    pub(super) first_window_interval_us: LiveSummary,
    pub(super) last_window_interval_us: LiveSummary,
}

#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct LiveSummary {
    pub(super) count: usize,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) mean: f64,
    pub(super) p95: f64,
    pub(super) p99: f64,
    pub(super) p999: f64,
    pub(super) p9999: f64,
    pub(super) over_1ms: usize,
    pub(super) over_5ms: usize,
    pub(super) over_10ms: usize,
    pub(super) over_20ms: usize,
}

#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct LiveCountSummary {
    pub(super) count: usize,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) mean: f64,
    pub(super) p95: f64,
    pub(super) p99: f64,
    pub(super) p999: f64,
    pub(super) p9999: f64,
}

#[derive(Clone)]
pub(super) struct LiveEventRecord {
    pub(super) at_us: u128,
    pub(super) key: String,
}

pub(super) struct LiveProbeRunner {
    pub(super) inner: NativeRunner,
    pub(super) send_us: Vec<f64>,
    pub(super) sends: Vec<LiveSendRecord>,
    pub(super) batches: Vec<usize>,
    pub(super) advance_correlations: Vec<AdvanceCorrelation>,
    pub(super) playing_statuses: u64,
    pub(super) measure_transport: bool,
}

#[derive(Clone)]
pub(super) struct LiveSendRecord {
    pub(super) label: String,
    pub(super) duration_us: f64,
    pub(super) pulses: Option<u32>,
}

#[derive(Serialize)]
pub(super) struct SlowSendReport {
    pub(super) label: String,
    pub(super) duration_us: f64,
}

pub(super) struct LiveProbeHost {
    pub(super) inner: PiPlaybackHostAdapter,
    pub(super) event_started_at: Instant,
    pub(super) events: Vec<LiveEventRecord>,
    pub(super) audio_send_us: Vec<f64>,
    pub(super) audio_commands: u64,
    pub(super) platform_effects: u64,
    pub(super) midi_messages: u64,
}

impl playback_runtime::CoreRunner for LiveProbeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let pulses = match &message {
            HostMessage::TransportPulseStep { pulses, .. } => Some(*pulses),
            _ => None,
        };
        let label = message_label(&message);
        let started = Instant::now();
        let responses = self.inner.send(message)?;
        let duration_us = started.elapsed().as_micros() as f64;
        self.send_us.push(duration_us);
        self.sends.push(LiveSendRecord {
            label,
            duration_us,
            pulses,
        });
        for response in &responses {
            match response {
                RunnerMessage::MusicalEvents { events } | RunnerMessage::MidiEvents { events } => {
                    self.batches.push(events.len());
                }
                RunnerMessage::RuntimeStatus { status }
                    if self.measure_transport
                        && status.transport == playback_runtime::RuntimeTransportState::Playing
                        && status.error.is_none() =>
                {
                    self.playing_statuses = self.playing_statuses.saturating_add(1);
                }
                _ => {}
            }
        }
        Ok(responses)
    }
}

impl HostAdapter for LiveProbeHost {
    fn handle_musical_event(
        &mut self,
        event: &MusicalEvent,
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.events.push(LiveEventRecord {
            at_us: self.event_started_at.elapsed().as_micros(),
            key: event_key(event),
        });
        let started = Instant::now();
        let result = self.inner.handle_musical_event(event);
        self.audio_send_us
            .push(started.elapsed().as_micros() as f64);
        result
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, playback_runtime::RuntimeAdapterError> {
        self.platform_effects = self.platform_effects.saturating_add(1);
        self.inner.handle_platform_effect(request)
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.audio_commands = self.audio_commands.saturating_add(1);
        self.inner.handle_audio_command(command)
    }

    fn handle_midi_message(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.midi_messages = self.midi_messages.saturating_add(1);
        self.inner.handle_midi_message(bytes)
    }

    fn silence_internal_audio(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.inner.silence_internal_audio()
    }

    fn panic_external_midi(&mut self) -> Result<(), playback_runtime::RuntimeAdapterError> {
        self.inner.panic_external_midi()
    }
}

fn event_key(event: &MusicalEvent) -> String {
    match event {
        MusicalEvent::NoteOn { channel, note, .. } => format!("note_on:{channel}:{note}"),
        MusicalEvent::NoteOff { channel, note } => format!("note_off:{channel}:{note}"),
        MusicalEvent::Cc {
            channel,
            controller,
            ..
        } => format!("cc:{channel}:{controller}"),
    }
}

fn message_label(message: &HostMessage) -> String {
    match message {
        HostMessage::DeviceInput { input, .. } => {
            let kind = input
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("device");
            let id = input.get("id").and_then(Value::as_str).unwrap_or("");
            if id.is_empty() {
                kind.into()
            } else {
                format!("{kind}:{id}")
            }
        }
        HostMessage::PresentedRuntimeErrorInput { .. } => "presented_runtime_error_input".into(),
        HostMessage::TransportPulseStep { pulses, .. } => format!("pulse:{pulses}"),
        HostMessage::MidiRealtimeStart => "midi_start".into(),
        HostMessage::MidiRealtimeContinue => "midi_continue".into(),
        HostMessage::MidiRealtimeStop => "midi_stop".into(),
        HostMessage::TransportStop => "transport_stop".into(),
        HostMessage::MidiRealtimeClock { pulses } => format!("midi_clock:{pulses}"),
        HostMessage::RuntimeResult { .. } => "runtime_result".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::LiveProbeRunner;
    use playback_runtime::{CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, SyncSource};

    #[test]
    fn playing_statuses_count_only_timed_runner_statuses_after_start_reset() {
        let mut runner = LiveProbeRunner {
            inner: NativeRunner::new(NativeRunnerConfig::default()).unwrap(),
            send_us: Vec::new(),
            sends: Vec::new(),
            batches: Vec::new(),
            advance_correlations: Vec::new(),
            playing_statuses: 0,
            measure_transport: false,
        };
        runner.inner.test_enable_midi();
        runner.send(HostMessage::MidiRealtimeStart).unwrap();
        runner.playing_statuses = 0;
        runner.measure_transport = true;
        for _ in 0..288 {
            runner
                .send(HostMessage::TransportPulseStep {
                    pulses: 1,
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: None,
                })
                .unwrap();
        }

        assert_eq!(runner.playing_statuses, 288);
    }

    #[test]
    fn pulse_batches_are_recorded_for_correlations() {
        let mut runner = LiveProbeRunner {
            inner: NativeRunner::new(NativeRunnerConfig::default()).unwrap(),
            send_us: Vec::new(),
            sends: Vec::new(),
            batches: Vec::new(),
            advance_correlations: Vec::new(),
            playing_statuses: 0,
            measure_transport: false,
        };
        runner.inner.test_enable_midi();
        runner.send(HostMessage::MidiRealtimeStart).unwrap();
        runner.measure_transport = true;
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: 2,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: None,
            })
            .unwrap();
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: 3,
                source: SyncSource::External,
                at_ppqn_pulse: None,
                request_snapshot: None,
            })
            .unwrap();

        assert_eq!(
            runner
                .sends
                .iter()
                .filter_map(|send| send.pulses)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }
}
