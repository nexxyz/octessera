mod audio;
mod diagnostics;
mod encoder_queue;
mod host_adapter;
mod input;
mod render;
mod runtime_loop;

use audio::AudioManager;
use cellsymphony_hal::{
    encoder_gpio::HardwareEvent, EncoderGpio, I2CBus, I2sDac, NeoKey, NeoTrellis, OledSsd1351,
};
use encoder_queue::PendingEncoderTurns;
use host_adapter::PiPlaybackHostAdapter;
use input::{
    encoder_press_message, grid_message, midi_realtime_message, neokey_message, MidiMessage,
};
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, SyncSource,
};
use render::{render_snapshot_cached, HardwareRenderCache, HardwareRenderTargets};
use runtime_loop::{
    dispatch_runtime_message, handle_deferred_host_work, initialize_host_state, latest_snapshot,
    sync_playback_config_from_snapshot,
};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const PLAYBACK_TICK_MS: u64 = 8;
const SNAPSHOT_INTERVAL_MS: u64 = 100;
const RENDER_INTERVAL_MS: u64 = 33;
const HARDWARE_EVENT_BUDGET: usize = 16;

fn main() {
    let _ = simple_logger::init();
    println!("Cell Symphony - Pi native runtime");

    if diagnostics::diagnostic_requested() {
        std::process::exit(if diagnostics::run_pre_hardware_diagnostics() {
            0
        } else {
            1
        });
    }

    let (_i2c_bus, mut oled, mut trellis, mut neokey, _dac) = init_hardware();
    let audio = init_audio();

    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });

    let (mut playback, mut runner) = init_runtime();

    let mut adapter = PiPlaybackHostAdapter::new(
        audio.as_ref(),
        default_store_dir(),
        default_samples_dir(),
        midi_handler,
    );
    if let Err(error) = initialize_host_state(&mut playback, &mut runner, &mut adapter) {
        eprintln!("pi host state initialization failed: {error}");
    }

    let event_rx = init_encoders();

    let _ = oled.write_frame(&vec![0_u8; 128 * 128 * 2]);
    let mut previous_neokey = [false; 4];
    let mut last_tick = Instant::now();
    let mut last_snapshot_request = Instant::now();
    let mut last_render = Instant::now() - Duration::from_millis(RENDER_INTERVAL_MS);
    let mut render_cache = HardwareRenderCache::default();
    let mut pending_encoder_turns = PendingEncoderTurns::default();
    let tick_duration = Duration::from_millis(PLAYBACK_TICK_MS);
    let snapshot_interval = Duration::from_millis(SNAPSHOT_INTERVAL_MS);
    let render_interval = Duration::from_millis(RENDER_INTERVAL_MS);

    loop {
        drain_midi_messages(&midi_rx, &mut playback, &mut runner, &mut adapter);
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

        poll_grid(&mut trellis, &mut playback, &mut runner, &mut adapter);
        poll_neokey(
            &mut neokey,
            &mut previous_neokey,
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
            &mut oled,
            &mut trellis,
            &mut neokey,
            &mut render_cache,
        ) {
            break;
        }

        thread::sleep(Duration::from_millis(1));
    }
}

fn dispatch_or_log(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
    message: HostMessage,
) {
    if let Err(error) = dispatch_runtime_message(playback, runner, adapter, message) {
        eprintln!("pi runtime dispatch failed: {error}");
    }
}

fn init_hardware() -> (I2CBus, OledSsd1351, NeoTrellis, NeoKey, I2sDac) {
    let i2c_bus = I2CBus::new(1).expect("I2C init failed");
    let oled = OledSsd1351::new().expect("OLED init failed");
    let trellis = NeoTrellis::new("/dev/i2c-1").expect("Trellis init failed");
    let neokey = NeoKey::new("/dev/i2c-1").expect("NeoKey init failed");
    let dac = I2sDac::new().expect("DAC init failed");
    (i2c_bus, oled, trellis, neokey, dac)
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
        ..NativeRunnerConfig::default()
    })
    .expect("native runner should initialize");
    runner.apply_runtime_config(playback.config());
    (playback, runner)
}

fn init_encoders() -> mpsc::Receiver<HardwareEvent> {
    let (event_tx, event_rx) = mpsc::channel::<HardwareEvent>();
    for (index, pins) in cellsymphony_hal::pinmap::ENCODERS.iter().enumerate() {
        let id = match index {
            0 => "encoder_main",
            1 => "encoder_aux_1",
            2 => "encoder_aux_2",
            3 => "encoder_aux_3",
            _ => unreachable!("encoder pin count follows platform capabilities"),
        };
        EncoderGpio::new(id, pins, event_tx.clone()).expect("Encoder init failed");
    }
    event_rx
}

fn drain_midi_messages(
    midi_rx: &mpsc::Receiver<MidiMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) {
    while let Ok(message) = midi_rx.try_recv() {
        match message {
            MidiMessage::Realtime { bytes } => {
                if let Err(error) = playback.handle_midi_realtime_bytes(&bytes, runner, adapter) {
                    eprintln!("pi realtime MIDI handling failed: {error}");
                }
            }
        }
    }
}

fn drain_encoder_events(
    event_rx: &mpsc::Receiver<HardwareEvent>,
    pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) {
    for _ in 0..HARDWARE_EVENT_BUDGET {
        let Ok(event) = event_rx.try_recv() else {
            break;
        };
        let message = match event {
            HardwareEvent::EncoderTurn { id, delta } => {
                pending_encoder_turns.enqueue(id, delta);
                continue;
            }
            HardwareEvent::EncoderPress { id } => {
                flush_pending_encoder_turns(pending_encoder_turns, playback, runner, adapter);
                encoder_press_message(id)
            }
        };
        dispatch_or_log(playback, runner, adapter, message);
    }
}

fn poll_grid(
    trellis: &mut NeoTrellis,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) {
    if let Ok(presses) = trellis.scan_keys() {
        for (x, y, pressed) in presses {
            dispatch_or_log(playback, runner, adapter, grid_message(x, y, pressed));
        }
    }
}

fn poll_neokey(
    neokey: &mut NeoKey,
    previous_neokey: &mut [bool; 4],
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) {
    if let Ok(keys) = neokey.scan() {
        for (key, pressed) in keys {
            let index = usize::from(key.min(3));
            if previous_neokey[index] == pressed {
                continue;
            }
            previous_neokey[index] = pressed;
            if let Some(message) = neokey_message(key, pressed) {
                dispatch_or_log(playback, runner, adapter, message);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn maybe_advance_runtime(
    last_tick: &mut Instant,
    tick_duration: Duration,
    last_snapshot_request: &mut Instant,
    snapshot_interval: Duration,
    last_render: &mut Instant,
    render_interval: Duration,
    pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
    oled: &mut OledSsd1351,
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    render_cache: &mut HardwareRenderCache,
) -> bool {
    if last_tick.elapsed() < tick_duration {
        return false;
    }
    let now = Instant::now();
    let elapsed_ms = now.duration_since(*last_tick).as_millis() as u64;
    *last_tick = now;
    flush_pending_encoder_turns(pending_encoder_turns, playback, runner, adapter);
    if now.duration_since(*last_snapshot_request) >= snapshot_interval {
        playback.request_next_snapshot();
        *last_snapshot_request = now;
    }
    if let Err(error) = playback.advance(elapsed_ms, runner, adapter) {
        eprintln!("pi playback advance failed: {error}");
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        eprintln!("pi deferred host work failed: {error}");
    }
    if now.duration_since(*last_render) >= render_interval {
        *last_render = now;
        render_latest_snapshot(playback, runner, oled, trellis, neokey, render_cache);
    }
    if !adapter.take_shutdown_request() {
        return false;
    }
    if let Err(error) = shutdown_pi_system() {
        eprintln!("pi shutdown failed: {error}");
        return false;
    }
    true
}

fn render_latest_snapshot(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    oled: &mut OledSsd1351,
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    render_cache: &mut HardwareRenderCache,
) {
    if let Some(snapshot) = latest_snapshot(playback).cloned() {
        sync_playback_config_from_snapshot(playback, runner, &snapshot);
        render_snapshot_cached(
            &mut HardwareRenderTargets {
                oled,
                trellis,
                neokey,
            },
            &snapshot,
            render_cache,
        );
    }
}

fn flush_pending_encoder_turns(
    pending: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) {
    for message in pending.take_messages() {
        dispatch_or_log(playback, runner, adapter, message);
    }
}

fn default_store_dir() -> PathBuf {
    std::env::var_os("CELLSYMPHONY_PI_STORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(feature = "hardware-pi") {
                PathBuf::from("/var/lib/cellsymphony")
            } else {
                PathBuf::from("config")
            }
        })
}

fn shutdown_pi_system() -> Result<(), String> {
    #[cfg(feature = "hardware-pi")]
    {
        let status = std::process::Command::new("systemctl")
            .arg("poweroff")
            .status()
            .map_err(|e| format!("failed to launch systemctl poweroff: {e}"))?;
        if !status.success() {
            return Err(format!("systemctl poweroff exited with status {status}"));
        }
    }
    Ok(())
}

fn default_samples_dir() -> PathBuf {
    std::env::var_os("CELLSYMPHONY_PI_SAMPLES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("samples"))
}
