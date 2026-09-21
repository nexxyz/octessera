use crate::audio::AudioManager;
use crate::main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};
use crate::{host_adapter::PiPlaybackHostAdapter, sample_browser::builtin_favourite_dirs};
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimeIngest, RuntimePlatformEffect, SyncSource, TimingProbeOptions, TimingProbeScenario,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod cadence;
mod live_probe;
mod live_report;
mod probe_options;

use cadence::{apply_scenario, observe_advance, LiveCadence};
use live_probe::{LiveProbeHost, LiveProbeRunner, LiveSummary, LiveTimingProbeReport};
use live_report::{
    intervals_u128, primary_stream_report, print_live_summary, slow_sends,
    summarize_advance_correlations, summarize_counts, summarize_us,
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
fn run_live_audio_probe(
    options: &TimingProbeOptions,
) -> Result<Vec<LiveTimingProbeReport>, String> {
    let overrides = timing_probe_overrides()?;
    let mut reports = Vec::new();
    for scenario in &options.scenarios {
        for duration in &options.durations {
            for wake_interval_ms in &options.wake_intervals_ms {
                reports.push(run_live_one(
                    *scenario,
                    *duration,
                    *wake_interval_ms,
                    options.snapshots,
                    overrides,
                )?);
            }
        }
    }
    Ok(reports)
}

fn run_live_one(
    scenario: TimingProbeScenario,
    duration: Duration,
    wake_interval_ms: u64,
    snapshots: bool,
    overrides: TimingProbeOverrides,
) -> Result<LiveTimingProbeReport, String> {
    if wake_interval_ms == 0 {
        return Err("wake interval must be positive".into());
    }
    let (audio, geometry) = AudioManager::new_timing_probe(
        overrides.output_buffer_frames,
        overrides.internal_block_frames,
        playback_runtime::AudioOutputSet::jack(),
    )?;
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
            playback_runtime::AudioOutputSet::jack(),
        ),
        event_started_at: Instant::now(),
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
            sample_builtin_favourite_dirs: builtin_favourite_dirs(),
            ..NativeRunnerConfig::default()
        })?,
        send_us: Vec::new(),
        sends: Vec::new(),
        batches: Vec::new(),
        advance_correlations: Vec::new(),
        playing_statuses: 0,
        measure_transport: false,
    };
    initialize_live_host_state(&mut playback, &mut runner, &mut host)?;
    send_runtime_message(
        &mut playback,
        &mut runner,
        &mut host,
        HostMessage::MidiRealtimeStart,
    )?;
    std::thread::sleep(Duration::from_millis(2_000));

    let event_started_at = Instant::now();
    host.event_started_at = event_started_at;
    host.events.clear();
    host.audio_send_us.clear();
    host.audio_commands = 0;
    host.platform_effects = 0;
    host.midi_messages = 0;
    runner.send_us.clear();
    runner.sends.clear();
    runner.batches.clear();
    runner.advance_correlations.clear();
    runner.playing_statuses = 0;
    apply_scenario(
        scenario,
        0,
        0,
        snapshots,
        &mut playback,
        &mut runner,
        &mut host,
    )?;
    runner.playing_statuses = 0;
    runner.measure_transport = true;
    let duration_ms = duration.as_millis() as u64;
    let measured_duration_ms = duration_ms - duration_ms % wake_interval_ms;
    let mut cadence = LiveCadence::new();
    let mut wake_late_us = Vec::new();
    let mut advance_us = Vec::new();
    let mut loop_us = Vec::new();
    let mut previous_ms = 0;
    for endpoint_ms in cadence::wake_endpoints(measured_duration_ms, wake_interval_ms) {
        let target = cadence.target(endpoint_ms);
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target.duration_since(now));
        }
        wake_late_us.push(Instant::now().saturating_duration_since(target).as_micros() as f64);
        let loop_started = Instant::now();
        apply_scenario(
            scenario,
            endpoint_ms,
            previous_ms,
            snapshots,
            &mut playback,
            &mut runner,
            &mut host,
        )?;
        let elapsed = cadence.elapsed_until(Instant::now());
        let advance_started = Instant::now();
        let sends_before = runner.sends.len();
        let batches_before = runner.batches.len();
        let output = playback.advance_duration_with_output(elapsed, &mut runner, &mut host)?;
        runner.advance_correlations.push(observe_advance(
            &runner.sends[sends_before..],
            &runner.batches[batches_before..],
        ));
        process_live_output(&mut playback, &mut runner, &mut host, output)?;
        advance_us.push(advance_started.elapsed().as_micros() as f64);
        flush_live_deferred(&mut playback, &mut runner, &mut host)?;
        loop_us.push(loop_started.elapsed().as_micros() as f64);
        previous_ms = endpoint_ms;
    }
    let event_times = host
        .events
        .iter()
        .map(|event| event.at_us)
        .collect::<Vec<_>>();
    let intervals = intervals_u128(&event_times);
    let correlation = summarize_advance_correlations(&runner.advance_correlations);
    Ok(LiveTimingProbeReport {
        scenario,
        duration_ms,
        measured_duration_ms,
        wake_interval_ms,
        force_snapshots: snapshots,
        output_buffer_frames: geometry.output_buffer_frames,
        internal_block_frames: geometry.internal_block_frames,
        events: host.events.len(),
        event_intervals_us: summarize_us(&intervals),
        primary_stream: primary_stream_report(&host.events),
        event_producing_advances: correlation.event_producing_advances,
        multi_pulse_advances: correlation.multi_pulse_advances,
        multi_pulse_event_producing_advances: correlation.multi_pulse_event_producing_advances,
        pulses_on_event_producing_advances: correlation.pulses_on_event_producing_advances,
        wake_late_us: summarize_us(&wake_late_us),
        advance_us: summarize_us(&advance_us),
        loop_us: summarize_us(&loop_us),
        audio_send_us: summarize_us(&host.audio_send_us),
        runner_send_us: summarize_us(&runner.send_us),
        slow_sends: slow_sends(&runner.sends),
        event_batches: summarize_counts(&runner.batches),
        audio_commands: host.audio_commands,
        platform_effects: host.platform_effects,
        midi_messages: host.midi_messages,
        playing_statuses: runner.playing_statuses,
    })
}

#[derive(Clone, Copy)]
struct TimingProbeOverrides {
    output_buffer_frames: Option<u32>,
    internal_block_frames: Option<usize>,
}

fn timing_probe_overrides() -> Result<TimingProbeOverrides, String> {
    Ok(TimingProbeOverrides {
        output_buffer_frames: read_frame_override("OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES")?,
        internal_block_frames: read_frame_override("OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES")?
            .map(|value| value as usize),
    })
}

fn read_frame_override(name: &str) -> Result<Option<u32>, String> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<u32>()
            .map(Some)
            .map_err(|_| format!("{name} must be an unsigned frame count")),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(format!("cannot read {name}: {error}")),
    }
}

fn initialize_live_host_state(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
) -> Result<(), String> {
    let output = playback.dispatch_runner_messages(
        vec![playback_runtime::RunnerMessage::PlatformEffects {
            effects: vec![
                RuntimePlatformEffect::StoreLoadDefault,
                RuntimePlatformEffect::MidiListOutputsRequest,
                RuntimePlatformEffect::MidiListInputsRequest,
            ],
        }],
        runner,
        host,
    )?;
    process_live_output(playback, runner, host, output)
}

fn send_play_page_input(
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
    let output = playback.dispatch(
        playback_runtime::RuntimeDispatchInput::HostMessage(message),
        runner,
        host,
    )?;
    process_live_output(playback, runner, host, output)
}

fn process_live_output(
    playback: &mut PlaybackRuntime,
    runner: &mut LiveProbeRunner,
    host: &mut LiveProbeHost,
    output: RuntimeIngest,
) -> Result<(), String> {
    for message in &output.messages {
        host.inner.ingest_oled_frame(message);
        if let RunnerMessage::Snapshot { snapshot } = message {
            host.inner.accept_oled_frame_reference(snapshot);
        }
    }
    let fault = host
        .inner
        .oled_frame_fault()
        .map(crate::oled_frame_cache::OledFrameCacheFault::into_runtime_fault);
    let fault_output = playback.report_oled_cache_fault(fault);
    for message in &fault_output.messages {
        host.inner.ingest_oled_frame(message);
        if let RunnerMessage::Snapshot { snapshot } = message {
            host.inner.accept_oled_frame_reference(snapshot);
        }
    }
    for follow_up in fault_output.follow_ups {
        send_runtime_message(playback, runner, host, follow_up)?;
    }
    for follow_up in output.follow_ups {
        send_runtime_message(playback, runner, host, follow_up)?;
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
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        process_live_output(playback, runner, host, output)?;
    }
    for follow_up in host.inner.flush_due_default_save()? {
        send_runtime_message(playback, runner, host, follow_up)?;
    }
    Ok(())
}
