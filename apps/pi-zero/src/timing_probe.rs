use crate::audio::AudioManager;
use crate::main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};
use crate::{host_adapter::PiPlaybackHostAdapter, sample_browser::SD_CARD_SAMPLE_BROWSER_DIR};
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformEffect,
    SyncSource, TimingProbeOptions, TimingProbeScenario,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod live_report;
mod probe_options;

use live_report::{
    event_key, intervals_u128, message_label, primary_stream_report, print_live_summary,
    slow_sends, summarize, summarize_usize,
};
pub(crate) use probe_options::requested;
use probe_options::{
    audio_drain_requested, options_from_env_and_args, run_audio_drain_probe, run_runtime_only,
    runtime_only_requested,
};

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
    let audio = AudioManager::new(None, crate::usb_config::UsbAudioOut::Jack)?;
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
            false,
            crate::usb_config::UsbAudioOut::Jack,
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
            sample_builtin_favourite_dirs: vec![String::new(), SD_CARD_SAMPLE_BROWSER_DIR.into()],
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
        playing_statuses = playing_statuses.saturating_add(0);
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
        TimingProbeScenario::PulsesStress if ms == 0 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::PulsesStress if ms % 250 == 20 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "encoder_press", "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::PulsesStress if ms % 250 == 120 => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "button_a", "pressed": true }),
            snapshots,
        ),
        TimingProbeScenario::StopStart if ms > 0 && ms.is_multiple_of(1000) => {
            send_runtime_message(playback, runner, host, HostMessage::MidiRealtimeStop)
        }
        TimingProbeScenario::StopStart if ms > 0 && ms % 1000 == 100 => {
            send_runtime_message(playback, runner, host, HostMessage::MidiRealtimeStart)
        }
        TimingProbeScenario::EncoderStress if ms.is_multiple_of(40) => send_device_input(
            playback,
            runner,
            host,
            json!({ "type": "encoder_turn", "delta": if (ms / 40).is_multiple_of(2) { 1 } else { -1 }, "id": "main" }),
            snapshots,
        ),
        TimingProbeScenario::MuteStress if ms.is_multiple_of(500) => {
            send_fn_play(playback, runner, host)
        }
        TimingProbeScenario::SparksPageStress if ms.is_multiple_of(250) => {
            send_sparks_page_input(playback, runner, host, ((ms / 250) % 5) as usize)
        }
        _ => Ok(()),
    }
}

fn send_sparks_page_input(
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
