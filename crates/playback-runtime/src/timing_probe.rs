use crate::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformRequest,
    RuntimeStoreResult, SyncSource,
};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::time::{Duration, Instant};

#[path = "timing_probe/cadence.rs"]
mod timing_probe_cadence;
#[path = "timing_probe_output.rs"]
mod timing_probe_output;
mod timing_probe_report;

use timing_probe_cadence::{apply_scenario, observe_advance, AdvanceCorrelation};
use timing_probe_output::process_probe_output;
use timing_probe_report::{
    event_key, intervals, primary_stream_report, summarize_advance_correlations, summarize_counts,
    summarize_ms, summarize_pulse_counts, summarize_us,
};
pub use timing_probe_report::{
    parse_timing_probe_durations, parse_timing_probe_scenarios,
    parse_timing_probe_wake_intervals_ms, print_timing_probe_summary,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingProbeScenario {
    Idle,
    PulsesStress,
    StopStart,
    EncoderStress,
    MuteStress,
    PlayPageStress,
}

#[derive(Clone, Debug)]
pub struct TimingProbeOptions {
    pub durations: Vec<Duration>,
    pub scenarios: Vec<TimingProbeScenario>,
    pub wake_intervals_ms: Vec<u64>,
    pub config: Option<String>,
    pub snapshots: bool,
    pub realtime: bool,
}

impl Default for TimingProbeOptions {
    fn default() -> Self {
        Self {
            durations: vec![Duration::from_secs(5)],
            scenarios: vec![TimingProbeScenario::Idle],
            wake_intervals_ms: vec![1],
            config: None,
            snapshots: false,
            realtime: false,
        }
    }
}

#[derive(Default)]
struct ProbeHost {
    now_ms: u64,
    event_times_ms: Vec<u64>,
    events: Vec<EventRecord>,
    audio_commands: u64,
    platform_effects: u64,
    midi_messages: u64,
}

#[derive(Clone, Debug)]
struct EventRecord {
    pub(super) time_ms: u64,
    pub(super) key: String,
}

struct ProbeRunner {
    inner: NativeRunner,
    sends: Vec<SendMetric>,
    batches: Vec<usize>,
    advance_correlations: Vec<AdvanceCorrelation>,
    playing_statuses: u64,
}

#[derive(Clone, Default, Serialize)]
struct SendMetric {
    pulses: Option<u32>,
    duration_us: u128,
}

#[derive(Serialize)]
pub struct TimingProbeReport {
    pub scenario: TimingProbeScenario,
    pub duration_ms: u64,
    pub measured_duration_ms: u64,
    pub wake_interval_ms: u64,
    pub force_snapshots: bool,
    pub realtime: bool,
    pub events: usize,
    pub event_batches: TimingProbeCountSummary,
    pub event_producing_advances: usize,
    pub multi_pulse_advances: usize,
    pub multi_pulse_event_producing_advances: usize,
    pub pulses_on_event_producing_advances: TimingProbeCountSummary,
    pub event_intervals_ms: TimingProbeSummary,
    pub primary_stream: Option<TimingProbeStreamReport>,
    pub pulses_per_advance: TimingProbeCountSummary,
    pub runner_send_us: TimingProbeSummary,
    pub advance_us: TimingProbeSummary,
    pub wake_late_us: TimingProbeSummary,
    pub loop_us: TimingProbeSummary,
    pub first_window_interval_ms: TimingProbeSummary,
    pub last_window_interval_ms: TimingProbeSummary,
    pub audio_commands: u64,
    pub platform_effects: u64,
    pub midi_messages: u64,
    pub playing_statuses: u64,
}

#[derive(Clone, Serialize)]
pub struct TimingProbeStreamReport {
    pub key: String,
    pub events: usize,
    pub intervals_ms: TimingProbeSummary,
    pub first_window_interval_ms: TimingProbeSummary,
    pub last_window_interval_ms: TimingProbeSummary,
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct TimingProbeSummary {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub p95: f64,
    pub p99: f64,
    pub p999: f64,
    pub p9999: f64,
    pub over_1ms: usize,
    pub over_5ms: usize,
    pub over_10ms: usize,
    pub over_20ms: usize,
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct TimingProbeCountSummary {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub p95: f64,
    pub p99: f64,
    pub p999: f64,
    pub p9999: f64,
}

impl CoreRunner for ProbeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let pulses = match &message {
            HostMessage::TransportPulseStep { pulses, .. } => Some(*pulses),
            _ => None,
        };
        let started = Instant::now();
        let responses = self.inner.send(message)?;
        let duration_us = started.elapsed().as_micros();
        for response in &responses {
            match response {
                RunnerMessage::MusicalEvents { events } | RunnerMessage::MidiEvents { events } => {
                    self.batches.push(events.len());
                }
                RunnerMessage::RuntimeStatus { status }
                    if status.transport == crate::RuntimeTransportState::Playing
                        && status.error.is_none() =>
                {
                    self.playing_statuses = self.playing_statuses.saturating_add(1);
                }
                _ => {}
            }
        }
        self.sends.push(SendMetric {
            pulses,
            duration_us,
        });
        Ok(responses)
    }
}

impl HostAdapter for ProbeHost {
    fn handle_musical_event(
        &mut self,
        event: &MusicalEvent,
    ) -> Result<(), crate::RuntimeAdapterError> {
        self.event_times_ms.push(self.now_ms);
        self.events.push(EventRecord {
            time_ms: self.now_ms,
            key: event_key(event),
        });
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, crate::RuntimeAdapterError> {
        self.platform_effects += 1;
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), crate::RuntimeAdapterError> {
        self.audio_commands += 1;
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), crate::RuntimeAdapterError> {
        self.midi_messages += 1;
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), crate::RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), crate::RuntimeAdapterError> {
        Ok(())
    }
}

pub fn run_timing_probe(options: &TimingProbeOptions) -> Result<Vec<TimingProbeReport>, String> {
    let mut reports = Vec::new();
    for scenario in &options.scenarios {
        for duration in &options.durations {
            for wake_interval_ms in &options.wake_intervals_ms {
                reports.push(run_one(
                    *scenario,
                    *duration,
                    *wake_interval_ms,
                    options.snapshots,
                    options.config.as_deref(),
                    options.realtime,
                )?);
            }
        }
    }
    Ok(reports)
}

fn run_one(
    scenario: TimingProbeScenario,
    duration: Duration,
    wake_interval_ms: u64,
    snapshots: bool,
    config_path: Option<&str>,
    realtime: bool,
) -> Result<TimingProbeReport, String> {
    if wake_interval_ms == 0 {
        return Err("wake interval must be positive".into());
    }
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: true,
    });
    let mut runner = ProbeRunner {
        inner: NativeRunner::new(NativeRunnerConfig::default())?,
        sends: Vec::new(),
        batches: Vec::new(),
        advance_correlations: Vec::new(),
        playing_statuses: 0,
    };
    let mut host = ProbeHost::default();
    if let Some(path) = config_path {
        load_config(path, &mut runtime, &mut runner, &mut host)?;
    }
    send_runtime_message(
        &mut runtime,
        &mut runner,
        &mut host,
        HostMessage::DeviceInput {
            input: serde_json::json!({ "type": "button_s", "pressed": true }),
            request_snapshot: Some(true),
        },
    )?;
    let mut advance_us = Vec::new();
    let mut wake_late_us = Vec::new();
    let mut loop_us = Vec::new();
    let duration_ms = duration.as_millis() as u64;
    let measured_duration_ms = duration_ms - duration_ms % wake_interval_ms;
    host.now_ms = 0;
    apply_scenario(
        scenario,
        0,
        0,
        snapshots,
        &mut runtime,
        &mut runner,
        &mut host,
    )?;
    runner.playing_statuses = 0;
    let realtime_started_at = Instant::now();
    let mut last_realtime_tick = realtime_started_at;
    let mut previous_ms = 0;
    for endpoint_ms in timing_probe_cadence::wake_endpoints(measured_duration_ms, wake_interval_ms)
    {
        if realtime {
            let target = realtime_started_at + Duration::from_millis(endpoint_ms);
            let now = Instant::now();
            if now < target {
                std::thread::sleep(target.duration_since(now));
            }
            wake_late_us.push(Instant::now().saturating_duration_since(target).as_micros() as f64);
            host.now_ms = realtime_started_at.elapsed().as_millis() as u64;
        } else {
            host.now_ms = endpoint_ms;
        }
        let loop_started_at = Instant::now();
        apply_scenario(
            scenario,
            endpoint_ms,
            previous_ms,
            snapshots,
            &mut runtime,
            &mut runner,
            &mut host,
        )?;
        let sends_before = runner.sends.len();
        let batches_before = runner.batches.len();
        let started = Instant::now();
        let advance_duration = if realtime {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(last_realtime_tick);
            last_realtime_tick = now;
            elapsed
        } else {
            Duration::from_millis(wake_interval_ms)
        };
        let output =
            runtime.advance_duration_with_output(advance_duration, &mut runner, &mut host)?;
        runner.advance_correlations.push(observe_advance(
            &runner.sends[sends_before..],
            &runner.batches[batches_before..],
        ));
        process_probe_output(&mut runtime, &mut runner, &mut host, output)?;
        advance_us.push(started.elapsed().as_micros() as f64);
        loop_us.push(loop_started_at.elapsed().as_micros() as f64);
        previous_ms = endpoint_ms;
    }
    let intervals = intervals(&host.event_times_ms);
    let window = intervals.len().min(128);
    let correlation = summarize_advance_correlations(&runner.advance_correlations);
    Ok(TimingProbeReport {
        scenario,
        duration_ms,
        measured_duration_ms,
        wake_interval_ms,
        force_snapshots: snapshots,
        realtime,
        events: host.event_times_ms.len(),
        event_batches: summarize_counts(&runner.batches),
        event_producing_advances: correlation.event_producing_advances,
        multi_pulse_advances: correlation.multi_pulse_advances,
        multi_pulse_event_producing_advances: correlation.multi_pulse_event_producing_advances,
        pulses_on_event_producing_advances: correlation.pulses_on_event_producing_advances,
        event_intervals_ms: summarize_ms(&intervals),
        primary_stream: primary_stream_report(&host.events),
        pulses_per_advance: summarize_pulse_counts(
            &runner
                .sends
                .iter()
                .filter_map(|send| send.pulses.map(f64::from))
                .collect::<Vec<_>>(),
        ),
        runner_send_us: summarize_us(
            &runner
                .sends
                .iter()
                .map(|send| send.duration_us as f64)
                .collect::<Vec<_>>(),
        ),
        advance_us: summarize_us(&advance_us),
        wake_late_us: summarize_us(&wake_late_us),
        loop_us: summarize_us(&loop_us),
        first_window_interval_ms: summarize_ms(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_ms: summarize_ms(
            &intervals
                .iter()
                .rev()
                .take(window)
                .copied()
                .collect::<Vec<_>>(),
        ),
        audio_commands: host.audio_commands,
        platform_effects: host.platform_effects,
        midi_messages: host.midi_messages,
        playing_statuses: runner.playing_statuses,
    })
}

fn send_runtime_message(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    message: HostMessage,
) -> Result<(), String> {
    let messages = runner.send(message)?;
    let output = runtime.dispatch(
        crate::RuntimeDispatchInput::RunnerMessages(messages),
        runner,
        host,
    )?;
    process_probe_output(runtime, runner, host, output)
}

fn load_config(
    path: &str,
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
) -> Result<(), String> {
    let body = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let payload = serde_json::from_str::<Value>(&body).map_err(|error| error.to_string())?;
    let messages = runner.send(HostMessage::RuntimeResult {
        result: RuntimeStoreResult::LoadDefaultResult {
            payload: Some(payload),
        },
    })?;
    let output = runtime.dispatch(
        crate::RuntimeDispatchInput::RunnerMessages(messages),
        runner,
        host,
    )?;
    process_probe_output(runtime, runner, host, output)
}
