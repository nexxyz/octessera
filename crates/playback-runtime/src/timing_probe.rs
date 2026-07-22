use crate::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformRequest,
    RuntimeStoreResult, SyncSource,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::time::{Duration, Instant};

mod timing_probe_report;

use timing_probe_report::{
    event_key, intervals, primary_stream_report, summarize, summarize_usize,
};
pub use timing_probe_report::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingProbeScenario {
    Idle,
    PulsesStress,
    StopStart,
    EncoderStress,
    MuteStress,
    SparksPageStress,
}

#[derive(Clone, Debug)]
pub struct TimingProbeOptions {
    pub durations: Vec<Duration>,
    pub scenarios: Vec<TimingProbeScenario>,
    pub config: Option<String>,
    pub snapshots: bool,
    pub realtime: bool,
}

impl Default for TimingProbeOptions {
    fn default() -> Self {
        Self {
            durations: vec![Duration::from_secs(5)],
            scenarios: vec![TimingProbeScenario::Idle],
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
    playing_statuses: u64,
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
    pub force_snapshots: bool,
    pub realtime: bool,
    pub events: usize,
    pub event_batches: TimingProbeSummary,
    pub event_intervals_ms: TimingProbeSummary,
    pub primary_stream: Option<TimingProbeStreamReport>,
    pub pulses_per_advance: TimingProbeSummary,
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
            if let RunnerMessage::MusicalEvents { events } = response {
                self.batches.push(events.len());
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
            reports.push(run_one(
                *scenario,
                *duration,
                options.snapshots,
                options.config.as_deref(),
                options.realtime,
            )?);
        }
    }
    Ok(reports)
}

fn run_one(
    scenario: TimingProbeScenario,
    duration: Duration,
    snapshots: bool,
    config_path: Option<&str>,
    realtime: bool,
) -> Result<TimingProbeReport, String> {
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
    };
    let mut host = ProbeHost::default();
    if let Some(path) = config_path {
        load_config(path, &mut runtime, &mut runner, &mut host)?;
    }
    send_runtime_message(
        &mut runtime,
        &mut runner,
        &mut host,
        HostMessage::MidiRealtimeStart,
    )?;
    let mut advance_us = Vec::new();
    let mut wake_late_us = Vec::new();
    let mut loop_us = Vec::new();
    let realtime_started_at = Instant::now();
    for ms in 0..duration.as_millis() as u64 {
        if realtime {
            let target = realtime_started_at + Duration::from_millis(ms);
            let now = Instant::now();
            if now < target {
                std::thread::sleep(target.duration_since(now));
            }
            wake_late_us.push(Instant::now().saturating_duration_since(target).as_micros() as f64);
        }
        let loop_started_at = Instant::now();
        host.now_ms = ms;
        apply_scenario(
            scenario,
            ms,
            snapshots,
            &mut runtime,
            &mut runner,
            &mut host,
        )?;
        let started = Instant::now();
        runtime.advance_duration(Duration::from_millis(1), &mut runner, &mut host)?;
        advance_us.push(started.elapsed().as_micros() as f64);
        loop_us.push(loop_started_at.elapsed().as_micros() as f64);
    }
    let intervals = intervals(&host.event_times_ms);
    let window = intervals.len().min(128);
    Ok(TimingProbeReport {
        scenario,
        duration_ms: duration.as_millis() as u64,
        force_snapshots: snapshots,
        realtime,
        events: host.event_times_ms.len(),
        event_batches: summarize_usize(&runner.batches),
        event_intervals_ms: summarize(&intervals),
        primary_stream: primary_stream_report(&host.events),
        pulses_per_advance: summarize(
            &runner
                .sends
                .iter()
                .filter_map(|send| send.pulses.map(f64::from))
                .collect::<Vec<_>>(),
        ),
        runner_send_us: summarize(
            &runner
                .sends
                .iter()
                .map(|send| send.duration_us as f64)
                .collect::<Vec<_>>(),
        ),
        advance_us: summarize(&advance_us),
        wake_late_us: summarize(&wake_late_us),
        loop_us: summarize(&loop_us),
        first_window_interval_ms: summarize(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_ms: summarize(
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
        playing_statuses: host.playing_statuses,
    })
}

fn apply_scenario(
    scenario: TimingProbeScenario,
    ms: u64,
    snapshots: bool,
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
) -> Result<(), String> {
    match scenario {
        TimingProbeScenario::Idle => Ok(()),
        TimingProbeScenario::PulsesStress if ms == 0 => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::PulsesStress if ms % 250 == 20 => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_press", "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::PulsesStress if ms % 250 == 120 => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "button_a", "pressed": true }),
            snapshots,
        ),
        TimingProbeScenario::StopStart if ms > 0 && ms.is_multiple_of(1000) => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "button_s", "pressed": true }),
            true,
        ),
        TimingProbeScenario::EncoderStress if ms.is_multiple_of(40) => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": if (ms / 40).is_multiple_of(2) { 1 } else { -1 }, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::MuteStress if ms.is_multiple_of(500) => {
            send_fn_play(runtime, runner, host)
        }
        TimingProbeScenario::SparksPageStress if ms.is_multiple_of(250) => {
            send_sparks_page_input(runtime, runner, host, ((ms / 250) % 5) as usize)
        }
        _ => Ok(()),
    }
}

fn send_sparks_page_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    y: usize,
) -> Result<(), String> {
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": true }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "grid_press", "x": 7, "y": y }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": false }),
        false,
    )
}

fn send_fn_play(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
) -> Result<(), String> {
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": true }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "button_s", "pressed": true }),
        false,
    )?;
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": false }),
        false,
    )
}

fn send_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    input: Value,
    snapshots: bool,
) -> Result<(), String> {
    send_runtime_message(
        runtime,
        runner,
        host,
        HostMessage::DeviceInput {
            input,
            request_snapshot: Some(snapshots),
        },
    )
}

fn send_runtime_message(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    message: HostMessage,
) -> Result<(), String> {
    let messages = runner.send(message)?;
    for message in &messages {
        if matches!(
            message,
            RunnerMessage::RuntimeStatus {
                status: crate::RuntimeStatus {
                    transport: crate::RuntimeTransportState::Playing,
                    error: None,
                    ..
                }
            }
        ) {
            host.playing_statuses += 1;
        }
    }
    runtime.ingest_runner_messages(messages, host).map(|_| ())
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
    runtime.ingest_runner_messages(messages, host).map(|_| ())
}
