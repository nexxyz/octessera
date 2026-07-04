mod audio;
mod audio_config_parse;
mod diagnostics;
mod dsp_profile;
mod encoder_queue;
mod hardware_fault;
mod hardware_init;
mod hardware_test;
mod hardware_test_noise;
mod host_adapter;
mod host_audio_command;
mod host_audio_prep;
mod input;
mod main_paths;
mod main_runtime_loop;
mod oled_test;
mod render;
mod render_loop;
mod runtime_loop;
mod sample_browser;
mod seesaw_io;
mod ui_profile;

use audio::AudioManager;
use encoder_queue::PendingEncoderTurns;
use hardware_init::{init_encoders, init_hardware, HardwareDevices};
use host_adapter::PiPlaybackHostAdapter;
use input::{midi_realtime_message, MidiMessage};
use playback_runtime::{
    NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, SyncSource,
};
use render::{HardwareRenderCache, HardwareRenderTargets};
use runtime_loop::initialize_host_state;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use ui_profile::UiProfiler;

use main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};
use main_runtime_loop::{
    drain_encoder_events, drain_host_messages, drain_midi_messages, flush_pending_encoder_turns,
    maybe_advance_runtime,
};

const PLAYBACK_TICK_MS: u64 = 8;
const SNAPSHOT_INTERVAL_MS: u64 = 100;
const RENDER_INTERVAL_MS: u64 = 33;
const PI_SD_CARD_SAMPLE_DIR: &str = "sd-card";

fn main() {
    let _ = simple_logger::init();

    run_requested_utility();

    println!("Cell Symphony - Pi native runtime");

    let hardware = match init_hardware() {
        Ok(devices) => devices,
        Err(fault) => hardware_fault::run_hardware_fault_mode(fault),
    };
    let (event_rx, _encoders) = match init_encoders() {
        Ok(encoders) => encoders,
        Err(mut fault) => {
            fault.attach_outputs(
                Some(hardware.oled),
                Some(hardware.trellis),
                Some(hardware.neokey),
            );
            hardware_fault::run_hardware_fault_mode(fault);
        }
    };
    let HardwareDevices {
        _i2c_bus,
        mut oled,
        trellis,
        neokey,
        input_interrupt,
        _dac,
    } = hardware;
    let seesaw_io = seesaw_io::spawn(trellis, neokey, input_interrupt);
    let audio = init_audio();

    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });

    let (mut playback, mut runner) = init_runtime();

    let store_dir = default_store_dir();
    let samples_dir = default_samples_dir();
    ensure_runtime_dirs(&store_dir, &samples_dir);

    let mut adapter = PiPlaybackHostAdapter::new(
        audio.as_ref().map(AudioManager::service),
        store_dir,
        samples_dir,
        midi_handler,
    );
    if let Err(error) = initialize_host_state(&mut playback, &mut runner, &mut adapter) {
        eprintln!("pi host state initialization failed: {error}");
    }

    let _ = oled.write_frame(&vec![0_u8; 128 * 128 * 2]);
    let mut last_tick = Instant::now();
    let mut last_snapshot_request = Instant::now();
    let mut last_render = Instant::now() - Duration::from_millis(RENDER_INTERVAL_MS);
    let mut render_cache = HardwareRenderCache::default();
    let mut render_targets = HardwareRenderTargets {
        oled: &mut oled,
        seesaw_tx: &seesaw_io.command_tx,
    };
    let mut pending_encoder_turns = PendingEncoderTurns::default();
    let mut ui_profiler = UiProfiler::from_process();
    let profile_enabled = ui_profiler.enabled();
    let mut last_loop_start = if profile_enabled {
        Some(Instant::now())
    } else {
        None
    };
    let tick_duration = Duration::from_millis(PLAYBACK_TICK_MS);
    let snapshot_interval = Duration::from_millis(SNAPSHOT_INTERVAL_MS);
    let render_interval = Duration::from_millis(RENDER_INTERVAL_MS);

    loop {
        let loop_start = profile_enabled.then(Instant::now);
        let loop_gap = loop_start
            .zip(last_loop_start)
            .map(|(loop_start, last)| loop_start.duration_since(last));
        last_loop_start = loop_start;
        drain_midi_messages(&midi_rx, &mut playback, &mut runner, &mut adapter);
        let host_input_started = profile_enabled.then(Instant::now);
        drain_host_messages(
            &seesaw_io.input_rx,
            &mut playback,
            &mut runner,
            &mut adapter,
        );
        if let Some(started) = host_input_started {
            ui_profiler.record_host_input(started.elapsed());
        }
        drain_encoder_events(
            &event_rx,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
        );
        flush_pending_encoder_turns(
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
        );

        if maybe_advance_runtime(
            &mut last_tick,
            tick_duration,
            &mut last_snapshot_request,
            snapshot_interval,
            &mut last_render,
            render_interval,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
            &mut render_cache,
            &mut render_targets,
            &mut ui_profiler,
        ) {
            break;
        }

        if let (Some(gap), Some(started)) = (loop_gap, loop_start) {
            ui_profiler.record_loop(gap, started.elapsed());
            ui_profiler.maybe_report();
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn run_requested_utility() {
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
    if diagnostics::diagnostic_requested() {
        std::process::exit(exit_code(diagnostics::run_pre_hardware_diagnostics()));
    }
    if oled_test::requested() {
        std::process::exit(exit_code(oled_test::run()));
    }
    if hardware_test::noise_requested() {
        std::process::exit(exit_code(hardware_test::run_noise_only()));
    }
    if hardware_test::requested() {
        std::process::exit(exit_code(hardware_test::run()));
    }
}

fn exit_code(success: bool) -> i32 {
    if success {
        0
    } else {
        1
    }
}

fn init_audio() -> Option<AudioManager> {
    match AudioManager::new() {
        Ok(audio) => {
            println!("Audio ready");
            Some(audio)
        }
        Err(error) => {
            println!("Audio init failed: {error} (continuing without audio)");
            None
        }
    }
}

fn init_runtime() -> (PlaybackRuntime, NativeRunner) {
    let playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        sample_builtin_favourite_dirs: vec![String::new(), PI_SD_CARD_SAMPLE_DIR.into()],
        ..NativeRunnerConfig::default()
    })
    .expect("native runner should initialize");
    runner.apply_runtime_config(playback.config());
    (playback, runner)
}
