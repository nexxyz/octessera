use crate::audio::AudioManager;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};
use playback_runtime::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
    run_timing_probe, CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner,
    NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeAudioCommand, RuntimeConfig,
    RuntimePlatformEffect, SyncSource, TimingProbeOptions, TimingProbeScenario,
};
use rodio_engine_source::EngineEvent;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) fn requested() -> bool {
    std::env::var("CELLSYMPHONY_PI_TIMING_PROBE").as_deref() == Ok("1")
        || std::env::args().any(|arg| arg == "--timing-probe")
}

pub(crate) fn run() -> bool {
    let options = match options_from_env_and_args() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("timing probe options failed: {error}");
            return false;
        }
    };
    if audio_drain_requested() {
        return run_audio_drain_probe(&options);
    }
    if runtime_only_requested() {
        return run_runtime_only(&options);
    }
    match run_live_audio_probe(&options) {
        Ok(reports) => {
            print_live_summary(&reports);
            match serde_json::to_string_pretty(&reports) {
                Ok(body) => println!("{body}"),
                Err(error) => {
                    eprintln!("timing probe JSON encode failed: {error}");
                    return false;
                }
            }
            true
        }
        Err(error) => {
            eprintln!("timing probe failed: {error}");
            false
        }
    }
}

#[derive(Serialize)]
struct AudioDrainProbeReport {
    duration_ms: u64,
    interval_ms: u64,
    marks: usize,
    drain_latency_us: LiveSummary,
}

#[derive(Serialize)]
struct LiveTimingProbeReport {
    scenario: TimingProbeScenario,
    duration_ms: u64,
    force_snapshots: bool,
    events: usize,
    event_intervals_us: LiveSummary,
    primary_stream: Option<LiveStreamReport>,
    wake_late_us: LiveSummary,
    advance_us: LiveSummary,
    loop_us: LiveSummary,
    audio_send_us: LiveSummary,
    runner_send_us: LiveSummary,
    slow_sends: Vec<SlowSendReport>,
    event_batches: LiveSummary,
    audio_commands: u64,
    platform_effects: u64,
    midi_messages: u64,
    playing_statuses: u64,
}

#[derive(Serialize)]
struct LiveStreamReport {
    key: String,
    events: usize,
    intervals_us: LiveSummary,
    first_window_interval_us: LiveSummary,
    last_window_interval_us: LiveSummary,
}

#[derive(Clone, Copy, Default, Serialize)]
struct LiveSummary {
    count: usize,
    min: f64,
    max: f64,
    mean: f64,
    p95: f64,
    p99: f64,
    p999: f64,
    p9999: f64,
    over_1ms: usize,
    over_5ms: usize,
    over_10ms: usize,
    over_20ms: usize,
}

#[derive(Clone)]
struct LiveEventRecord {
    at_us: u128,
    key: String,
}

struct LiveProbeRunner {
    inner: NativeRunner,
    send_us: Vec<f64>,
    sends: Vec<LiveSendRecord>,
    batches: Vec<usize>,
}

#[derive(Clone)]
struct LiveSendRecord {
    label: String,
    duration_us: f64,
}

#[derive(Serialize)]
struct SlowSendReport {
    label: String,
    duration_us: f64,
}

struct LiveProbeHost {
    inner: PiPlaybackHostAdapter,
    started_at: Instant,
    events: Vec<LiveEventRecord>,
    audio_send_us: Vec<f64>,
    audio_commands: u64,
    platform_effects: u64,
    midi_messages: u64,
}

impl playback_runtime::CoreRunner for LiveProbeRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let label = message_label(&message);
        let started = Instant::now();
        let responses = self.inner.send(message)?;
        let duration_us = started.elapsed().as_micros() as f64;
        self.send_us.push(duration_us);
        self.sends.push(LiveSendRecord { label, duration_us });
        for response in &responses {
            if let RunnerMessage::MusicalEvents { events } = response {
                self.batches.push(events.len());
            }
        }
        Ok(responses)
    }
}

impl HostAdapter for LiveProbeHost {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), String> {
        self.events.push(LiveEventRecord {
            at_us: self.started_at.elapsed().as_micros(),
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
        effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String> {
        self.platform_effects = self.platform_effects.saturating_add(1);
        self.inner.handle_platform_effect(effect)
    }

    fn handle_audio_command(&mut self, command: &RuntimeAudioCommand) -> Result<(), String> {
        self.audio_commands = self.audio_commands.saturating_add(1);
        self.inner.handle_audio_command(command)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.midi_messages = self.midi_messages.saturating_add(1);
        self.inner.handle_midi_message(bytes)
    }
}

fn run_live_audio_probe(
    options: &TimingProbeOptions,
) -> Result<Vec<LiveTimingProbeReport>, String> {
    let mut reports = Vec::new();
    for scenario in &options.scenarios {
        for duration in &options.durations {
            reports.push(run_live_one(*scenario, *duration, options.snapshots)?);
        }
    }
    Ok(reports)
}

fn run_live_one(
    scenario: TimingProbeScenario,
    duration: Duration,
    snapshots: bool,
) -> Result<LiveTimingProbeReport, String> {
    let audio = AudioManager::new()?;
    let store_dir = default_store_dir();
    let samples_dir = default_samples_dir();
    ensure_runtime_dirs(&store_dir, &samples_dir);
    let midi_handler = Arc::new(|_bytes: Vec<u8>| {});
    let mut host = LiveProbeHost {
        inner: PiPlaybackHostAdapter::new(
            Some(audio.service()),
            store_dir,
            samples_dir,
            midi_handler,
        ),
        started_at: Instant::now(),
        events: Vec::new(),
        audio_send_us: Vec::new(),
        audio_commands: 0,
        platform_effects: 0,
        midi_messages: 0,
    };
    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let mut runner = LiveProbeRunner {
        inner: NativeRunner::new(NativeRunnerConfig {
            behavior_id: "sequencer".into(),
            sample_builtin_favourite_dirs: vec![String::new(), "sd-card".into()],
            ..NativeRunnerConfig::default()
        })?,
        send_us: Vec::new(),
        sends: Vec::new(),
        batches: Vec::new(),
    };
    runner.inner.apply_runtime_config(playback.config());
    initialize_live_host_state(&mut playback, &mut runner, &mut host)?;
    send_runtime_message(
        &mut playback,
        &mut runner,
        &mut host,
        HostMessage::MidiRealtimeStart,
    )?;
    std::thread::sleep(Duration::from_millis(2_000));

    let started_at = Instant::now();
    host.started_at = started_at;
    host.events.clear();
    host.audio_send_us.clear();
    host.audio_commands = 0;
    host.platform_effects = 0;
    host.midi_messages = 0;
    runner.send_us.clear();
    runner.sends.clear();
    runner.batches.clear();
    let mut last_tick = started_at;
    let mut wake_late_us = Vec::new();
    let mut advance_us = Vec::new();
    let mut loop_us = Vec::new();
    let mut playing_statuses = 0_u64;
    for ms in 0..duration.as_millis() as u64 {
        let target = started_at + Duration::from_millis(ms);
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target.duration_since(now));
        }
        wake_late_us.push(Instant::now().saturating_duration_since(target).as_micros() as f64);
        let loop_started = Instant::now();
        apply_live_scenario(
            scenario,
            ms,
            snapshots,
            &mut playback,
            &mut runner,
            &mut host,
        )?;
        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);
        last_tick = now;
        let advance_started = Instant::now();
        playback.advance_duration(elapsed, &mut runner, &mut host)?;
        advance_us.push(advance_started.elapsed().as_micros() as f64);
        playing_statuses = playing_statuses.saturating_add(count_playing_statuses(&runner));
        flush_live_deferred(&mut playback, &mut runner, &mut host)?;
        loop_us.push(loop_started.elapsed().as_micros() as f64);
    }
    let event_times = host
        .events
        .iter()
        .map(|event| event.at_us)
        .collect::<Vec<_>>();
    let intervals = intervals_u128(&event_times);
    Ok(LiveTimingProbeReport {
        scenario,
        duration_ms: duration.as_millis() as u64,
        force_snapshots: snapshots,
        events: host.events.len(),
        event_intervals_us: summarize(&intervals),
        primary_stream: primary_stream_report(&host.events),
        wake_late_us: summarize(&wake_late_us),
        advance_us: summarize(&advance_us),
        loop_us: summarize(&loop_us),
        audio_send_us: summarize(&host.audio_send_us),
        runner_send_us: summarize(&runner.send_us),
        slow_sends: slow_sends(&runner.sends),
        event_batches: summarize_usize(&runner.batches),
        audio_commands: host.audio_commands,
        platform_effects: host.platform_effects,
        midi_messages: host.midi_messages,
        playing_statuses,
    })
}

fn initialize_live_host_state(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
) -> Result<(), String> {
    for effect in [
        RuntimePlatformEffect::StoreLoadDefault,
        RuntimePlatformEffect::MidiListOutputsRequest,
        RuntimePlatformEffect::MidiListInputsRequest,
    ] {
        for follow_up in host.handle_platform_effect(&effect)? {
            send_runtime_message(playback, runner, host, follow_up)?;
        }
    }
    Ok(())
}

fn apply_live_scenario(
    scenario: TimingProbeScenario,
    ms: u64,
    snapshots: bool,
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
) -> Result<(), String> {
    match scenario {
        TimingProbeScenario::Idle => Ok(()),
        TimingProbeScenario::SenseStress if ms == 0 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::SenseStress if ms % 250 == 20 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "encoder_press", "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::SenseStress if ms % 250 == 120 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "button_a", "pressed": true }),
            snapshots,
        ),
        TimingProbeScenario::StopStart if ms > 0 && ms % 1000 == 0 => {
            send_runtime_message(playback, runner, host, HostMessage::MidiRealtimeStop)
        }
        TimingProbeScenario::StopStart if ms > 0 && ms % 1000 == 100 => {
            send_runtime_message(playback, runner, host, HostMessage::MidiRealtimeStart)
        }
        TimingProbeScenario::EncoderStress if ms % 40 == 0 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": if (ms / 40) % 2 == 0 { 1 } else { -1 }, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::MuteStress if ms % 500 == 0 => send_fn_play(playback, runner, host),
        TimingProbeScenario::DancePageStress if ms % 250 == 0 => {
            send_dance_page_input(playback, runner, host, ((ms / 250) % 5) as usize)
        }
        _ => Ok(()),
    }
}

fn send_dance_page_input(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
    y: usize,
) -> Result<(), String> {
    send_device_input(
        playback,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": true }),
        false,
    )?;
    send_device_input(
        playback,
        runner,
        host,
        json!({ "type": "grid_press", "x": 7, "y": y }),
        false,
    )?;
    send_device_input(
        playback,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": false }),
        false,
    )
}

fn send_fn_play(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
) -> Result<(), String> {
    send_device_input(
        playback,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": true }),
        false,
    )?;
    send_device_input(
        playback,
        runner,
        host,
        json!({ "type": "button_s", "pressed": true }),
        false,
    )?;
    send_device_input(
        playback,
        runner,
        host,
        json!({ "type": "button_fn", "pressed": false }),
        false,
    )
}

fn send_device_input(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
    input: Value,
    snapshots: bool,
) -> Result<(), String> {
    send_runtime_message(
        playback,
        runner,
        host,
        HostMessage::DeviceInput {
            input,
            request_snapshot: Some(snapshots),
        },
    )
}

fn send_runtime_message(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
    message: HostMessage,
) -> Result<(), String> {
    let mut queue = VecDeque::from([message]);
    while let Some(message) = queue.pop_front() {
        let responses = runner.send(message)?;
        for follow_up in playback.ingest_runner_messages(responses, host)? {
            queue.push_back(follow_up);
        }
    }
    Ok(())
}

fn flush_live_deferred(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
) -> Result<(), String> {
    let responses = runner.inner.flush_deferred_menu_apply()?;
    if !responses.is_empty() {
        let mut queue = VecDeque::from(playback.ingest_runner_messages(responses, host)?);
        while let Some(message) = queue.pop_front() {
            send_runtime_message(playback, runner, host, message)?;
        }
    }
    for follow_up in host.inner.flush_due_default_save()? {
        send_runtime_message(playback, runner, host, follow_up)?;
    }
    Ok(())
}

fn count_playing_statuses(_runner: &LiveProbeRunner) -> u64 {
    0
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
        HostMessage::TransportPulseStep { pulses, .. } => format!("pulse:{pulses}"),
        HostMessage::MidiRealtimeStart => "midi_start".into(),
        HostMessage::MidiRealtimeContinue => "midi_continue".into(),
        HostMessage::MidiRealtimeStop => "midi_stop".into(),
        HostMessage::MidiRealtimeClock { pulses } => format!("midi_clock:{pulses}"),
        HostMessage::RuntimeResult { .. } => "runtime_result".into(),
    }
}

fn slow_sends(sends: &[LiveSendRecord]) -> Vec<SlowSendReport> {
    let mut sorted = sends.to_vec();
    sorted.sort_by(|a, b| b.duration_us.total_cmp(&a.duration_us));
    sorted
        .into_iter()
        .take(8)
        .map(|send| SlowSendReport {
            label: send.label,
            duration_us: send.duration_us,
        })
        .collect()
}

fn options_from_env_and_args() -> Result<TimingProbeOptions, String> {
    let mut options = TimingProbeOptions {
        realtime: true,
        config: default_config_path(),
        ..TimingProbeOptions::default()
    };
    if let Ok(value) = std::env::var("CELLSYMPHONY_PI_TIMING_PROBE_DURATIONS") {
        options.durations = parse_timing_probe_durations(&value)?;
    }
    if let Ok(value) = std::env::var("CELLSYMPHONY_PI_TIMING_PROBE_SCENARIOS") {
        options.scenarios = parse_timing_probe_scenarios(&value)?;
    }
    if let Ok(value) = std::env::var("CELLSYMPHONY_PI_TIMING_PROBE_CONFIG") {
        options.config = Some(value);
    }
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--timing-probe" => {}
            "--timing-probe-duration" | "--timing-probe-durations" => {
                options.durations = parse_timing_probe_durations(&next(&mut iter, &arg)?)?
            }
            "--timing-probe-scenario" | "--timing-probe-scenarios" => {
                options.scenarios = parse_timing_probe_scenarios(&next(&mut iter, &arg)?)?
            }
            "--timing-probe-config" => options.config = Some(next(&mut iter, &arg)?),
            "--timing-probe-no-config" => options.config = None,
            "--timing-probe-snapshots" => options.snapshots = true,
            _ => {}
        }
    }
    Ok(options)
}

fn run_runtime_only(options: &TimingProbeOptions) -> bool {
    match run_timing_probe(options) {
        Ok(reports) => {
            print_timing_probe_summary(&reports);
            match serde_json::to_string_pretty(&reports) {
                Ok(body) => println!("{body}"),
                Err(error) => {
                    eprintln!("timing probe JSON encode failed: {error}");
                    return false;
                }
            }
            true
        }
        Err(error) => {
            eprintln!("timing probe failed: {error}");
            false
        }
    }
}

fn run_audio_drain_probe(options: &TimingProbeOptions) -> bool {
    let duration = options
        .durations
        .first()
        .copied()
        .unwrap_or_else(|| Duration::from_secs(60));
    match run_audio_drain_one(duration) {
        Ok(report) => {
            eprintln!(
                "AudioDrain {}ms marks={} p99={:.0}us p999={:.0}us p9999={:.0}us max={:.0}us >5ms={} >10ms={} >20ms={}",
                report.duration_ms,
                report.marks,
                report.drain_latency_us.p99,
                report.drain_latency_us.p999,
                report.drain_latency_us.p9999,
                report.drain_latency_us.max,
                report.drain_latency_us.over_5ms,
                report.drain_latency_us.over_10ms,
                report.drain_latency_us.over_20ms
            );
            match serde_json::to_string_pretty(&report) {
                Ok(body) => println!("{body}"),
                Err(error) => {
                    eprintln!("audio drain probe JSON encode failed: {error}");
                    return false;
                }
            }
            true
        }
        Err(error) => {
            eprintln!("audio drain probe failed: {error}");
            false
        }
    }
}

fn run_audio_drain_one(duration: Duration) -> Result<AudioDrainProbeReport, String> {
    let audio = AudioManager::new()?;
    let service = audio.service();
    let interval = audio_drain_interval();
    let (report_tx, report_rx) = std::sync::mpsc::channel::<u128>();
    let started_at = Instant::now();
    let mut sent = 0usize;
    while started_at.elapsed() < duration {
        let target = started_at + interval * (sent as u32);
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target.duration_since(now));
        }
        service.send_realtime(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx: report_tx.clone(),
        })?;
        sent += 1;
    }
    drop(report_tx);
    std::thread::sleep(Duration::from_millis(100));
    let latencies = report_rx
        .try_iter()
        .map(|latency| latency as f64)
        .collect::<Vec<_>>();
    Ok(AudioDrainProbeReport {
        duration_ms: duration.as_millis() as u64,
        interval_ms: interval.as_millis() as u64,
        marks: latencies.len(),
        drain_latency_us: summarize(&latencies),
    })
}

fn runtime_only_requested() -> bool {
    std::env::var("CELLSYMPHONY_PI_TIMING_PROBE_RUNTIME_ONLY").as_deref() == Ok("1")
        || std::env::args().any(|arg| arg == "--timing-probe-runtime-only")
}

fn audio_drain_requested() -> bool {
    std::env::var("CELLSYMPHONY_PI_TIMING_PROBE_AUDIO_DRAIN").as_deref() == Ok("1")
        || std::env::args().any(|arg| arg == "--timing-probe-audio-drain")
}

fn audio_drain_interval() -> Duration {
    let millis = std::env::var("CELLSYMPHONY_PI_TIMING_PROBE_AUDIO_DRAIN_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(10)
        .clamp(1, 1000);
    Duration::from_millis(millis)
}

fn default_config_path() -> Option<String> {
    let path: PathBuf = default_store_dir().join("default.json");
    path.exists().then(|| path.to_string_lossy().into_owned())
}

fn next(iter: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("missing value for {name}"))
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

fn intervals_u128(times: &[u128]) -> Vec<f64> {
    times
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]) as f64)
        .collect()
}

fn primary_stream_report(events: &[LiveEventRecord]) -> Option<LiveStreamReport> {
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

fn stream_report(events: &[LiveEventRecord], key: String) -> Option<LiveStreamReport> {
    let times = events
        .iter()
        .filter(|event| event.key == key)
        .map(|event| event.at_us)
        .collect::<Vec<_>>();
    if times.len() < 2 {
        return None;
    }
    let intervals = intervals_u128(&times);
    let window = intervals.len().min(128);
    Some(LiveStreamReport {
        key,
        events: times.len(),
        intervals_us: summarize(&intervals),
        first_window_interval_us: summarize(
            &intervals.iter().take(window).copied().collect::<Vec<_>>(),
        ),
        last_window_interval_us: summarize(
            &intervals
                .iter()
                .rev()
                .take(window)
                .copied()
                .collect::<Vec<_>>(),
        ),
    })
}

fn summarize_usize(values: &[usize]) -> LiveSummary {
    summarize(&values.iter().map(|value| *value as f64).collect::<Vec<_>>())
}

fn summarize(values: &[f64]) -> LiveSummary {
    if values.is_empty() {
        return LiveSummary::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    LiveSummary {
        count: values.len(),
        min: sorted[0],
        max: *sorted.last().unwrap(),
        mean: values.iter().sum::<f64>() / values.len() as f64,
        p95: percentile(&sorted, 9500),
        p99: percentile(&sorted, 9900),
        p999: percentile(&sorted, 9990),
        p9999: percentile(&sorted, 9999),
        over_1ms: values.iter().filter(|value| **value > 1_000.0).count(),
        over_5ms: values.iter().filter(|value| **value > 5_000.0).count(),
        over_10ms: values.iter().filter(|value| **value > 10_000.0).count(),
        over_20ms: values.iter().filter(|value| **value > 20_000.0).count(),
    }
}

fn percentile(sorted: &[f64], basis_points: usize) -> f64 {
    let index = ((sorted.len() - 1) * basis_points) / 10_000;
    sorted[index]
}

fn print_live_summary(reports: &[LiveTimingProbeReport]) {
    for report in reports {
        eprintln!(
            "{:?} {}ms live-audio events={} interval_p95={:.0}us wake_late_p95={:.0}us loop_p95={:.0}us audio_send_p95={:.0}us send_p95={:.0}us batch_max={:.0}",
            report.scenario,
            report.duration_ms,
            report.events,
            report.event_intervals_us.p95,
            report.wake_late_us.p95,
            report.loop_us.p95,
            report.audio_send_us.p95,
            report.runner_send_us.p95,
            report.event_batches.max
        );
        if let Some(stream) = &report.primary_stream {
            eprintln!(
                "  primary={} events={} interval_p95={:.0}us p99={:.0}us p999={:.0}us max={:.0}us first_p95={:.0}us last_p95={:.0}us",
                stream.key,
                stream.events,
                stream.intervals_us.p95,
                stream.intervals_us.p99,
                stream.intervals_us.p999,
                stream.intervals_us.max,
                stream.first_window_interval_us.p95,
                stream.last_window_interval_us.p95
            );
        }
        eprintln!(
            "  wake_late p99={:.0}us p999={:.0}us p9999={:.0}us max={:.0}us >5ms={} >10ms={} >20ms={}",
            report.wake_late_us.p99,
            report.wake_late_us.p999,
            report.wake_late_us.p9999,
            report.wake_late_us.max,
            report.wake_late_us.over_5ms,
            report.wake_late_us.over_10ms,
            report.wake_late_us.over_20ms
        );
        eprintln!(
            "  loop p99={:.0}us p999={:.0}us p9999={:.0}us max={:.0}us >5ms={} >10ms={} >20ms={}",
            report.loop_us.p99,
            report.loop_us.p999,
            report.loop_us.p9999,
            report.loop_us.max,
            report.loop_us.over_5ms,
            report.loop_us.over_10ms,
            report.loop_us.over_20ms
        );
        for send in &report.slow_sends {
            eprintln!("  slow_send={} {:.0}us", send.label, send.duration_us);
        }
    }
}
