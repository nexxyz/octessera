use crate::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformEffect,
    RuntimeStoreResult, SyncSource,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingProbeScenario {
    Idle,
    SenseStress,
    StopStart,
    EncoderStress,
    MuteStress,
    DancePageStress,
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
    time_ms: u64,
    key: String,
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
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), String> {
        self.event_times_ms.push(self.now_ms);
        self.events.push(EventRecord {
            time_ms: self.now_ms,
            key: event_key(event),
        });
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String> {
        self.platform_effects += 1;
        Ok(Vec::new())
    }

    fn handle_audio_command(&mut self, _command: &RuntimeAudioCommand) -> Result<(), String> {
        self.audio_commands += 1;
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), String> {
        self.midi_messages += 1;
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
        TimingProbeScenario::SenseStress if ms == 0 => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::SenseStress if ms % 250 == 20 => send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_press", "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::SenseStress if ms % 250 == 120 => send_input(
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
        TimingProbeScenario::DancePageStress if ms.is_multiple_of(250) => {
            send_dance_page_input(runtime, runner, host, ((ms / 250) % 5) as usize)
        }
        _ => Ok(()),
    }
}

fn send_dance_page_input(
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

fn intervals(times: &[u64]) -> Vec<f64> {
    times
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]) as f64)
        .collect()
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

fn primary_stream_report(events: &[EventRecord]) -> Option<TimingProbeStreamReport> {
    let mut keys = Vec::<String>::new();
    for event in events {
        if !keys.iter().any(|key| key == &event.key) {
            keys.push(event.key.clone());
        }
    }
    keys.into_iter()
        .filter_map(|key| stream_report(events, key))
        .max_by_key(|report| report.events)
}

fn stream_report(events: &[EventRecord], key: String) -> Option<TimingProbeStreamReport> {
    let times = events
        .iter()
        .filter(|event| event.key == key)
        .map(|event| event.time_ms)
        .collect::<Vec<_>>();
    if times.len() < 2 {
        return None;
    }
    let intervals = intervals(&times);
    let window = intervals.len().min(128);
    Some(TimingProbeStreamReport {
        key,
        events: times.len(),
        intervals_ms: summarize(&intervals),
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
    })
}

fn summarize_usize(values: &[usize]) -> TimingProbeSummary {
    summarize(&values.iter().map(|value| *value as f64).collect::<Vec<_>>())
}

fn summarize(values: &[f64]) -> TimingProbeSummary {
    if values.is_empty() {
        return TimingProbeSummary::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    TimingProbeSummary {
        count: values.len(),
        min: sorted[0],
        max: *sorted.last().unwrap(),
        mean: values.iter().sum::<f64>() / values.len() as f64,
        p95: percentile(&sorted, 9500),
        p99: percentile(&sorted, 9900),
        p999: percentile(&sorted, 9990),
        p9999: percentile(&sorted, 9999),
        over_1ms: values.iter().filter(|value| **value > 1.0).count(),
        over_5ms: values.iter().filter(|value| **value > 5.0).count(),
        over_10ms: values.iter().filter(|value| **value > 10.0).count(),
        over_20ms: values.iter().filter(|value| **value > 20.0).count(),
    }
}

fn percentile(sorted: &[f64], basis_points: usize) -> f64 {
    let index = ((sorted.len() - 1) * basis_points) / 10_000;
    sorted[index]
}

pub fn print_timing_probe_summary(reports: &[TimingProbeReport]) {
    for report in reports {
        eprintln!("{:?} {}ms realtime={} events={} playing_statuses={} interval_mean={:.2}ms p95={:.2}ms send_p95={:.0}us advance_p95={:.0}us wake_late_p95={:.0}us loop_p95={:.0}us batch_max={:.0}", report.scenario, report.duration_ms, report.realtime, report.events, report.playing_statuses, report.event_intervals_ms.mean, report.event_intervals_ms.p95, report.runner_send_us.p95, report.advance_us.p95, report.wake_late_us.p95, report.loop_us.p95, report.event_batches.max);
    }
}

pub fn parse_timing_probe_scenarios(value: &str) -> Result<Vec<TimingProbeScenario>, String> {
    value
        .split(',')
        .map(|item| match item.trim() {
            "idle" => Ok(TimingProbeScenario::Idle),
            "sense" | "sense-stress" => Ok(TimingProbeScenario::SenseStress),
            "stop-start" => Ok(TimingProbeScenario::StopStart),
            "encoder" | "encoder-stress" => Ok(TimingProbeScenario::EncoderStress),
            "mute" | "mute-stress" | "fn-play" => Ok(TimingProbeScenario::MuteStress),
            "dance-page" | "dance-pages" | "dance-page-stress" => {
                Ok(TimingProbeScenario::DancePageStress)
            }
            other => Err(format!("unknown scenario {other}")),
        })
        .collect()
}

pub fn parse_timing_probe_durations(value: &str) -> Result<Vec<Duration>, String> {
    value.split(',').map(parse_duration).collect()
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let trimmed = value.trim();
    let (number, multiplier) = trimmed
        .strip_suffix('m')
        .map(|n| (n, 60))
        .or_else(|| trimmed.strip_suffix('s').map(|n| (n, 1)))
        .ok_or_else(|| format!("duration must end in s or m: {trimmed}"))?;
    Ok(Duration::from_secs(
        number
            .parse::<u64>()
            .map_err(|_| format!("invalid duration {trimmed}"))?
            * multiplier,
    ))
}
