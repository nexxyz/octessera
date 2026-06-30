use crate::encoder_queue::PendingEncoderTurns;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::{encoder_press_message, grid_message, neokey_message, MidiMessage};
use crate::render::{HardwareRenderCache, HardwareRenderTargets};
use crate::runtime_loop::{dispatch_runtime_message, handle_deferred_host_work};
use crate::ui_profile::UiProfiler;
use cellsymphony_hal::{encoder_gpio::HardwareEvent, NeoKey, NeoTrellis, OledSsd1351};
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HARDWARE_EVENT_BUDGET: usize = 16;

pub(crate) fn drain_midi_messages(
    midi_rx: &mpsc::Receiver<MidiMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
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

pub(crate) fn drain_encoder_events(
    event_rx: &mpsc::Receiver<HardwareEvent>,
    pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
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

pub(crate) fn poll_grid(
    trellis: &mut NeoTrellis,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    if let Ok(presses) = trellis.scan_keys() {
        for (x, y, pressed) in presses {
            dispatch_or_log(playback, runner, adapter, grid_message(x, y, pressed));
        }
    }
}

pub(crate) fn poll_neokey(
    neokey: &mut NeoKey,
    previous_neokey: &mut [bool; 4],
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
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
pub(crate) fn maybe_advance_runtime(
    last_tick: &mut Instant,
    tick_duration: Duration,
    last_snapshot_request: &mut Instant,
    snapshot_interval: Duration,
    last_render: &mut Instant,
    render_interval: Duration,
    pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    oled: &mut OledSsd1351,
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    render_cache: &mut HardwareRenderCache,
    ui_profiler: &mut UiProfiler,
) -> bool {
    if last_tick.elapsed() < tick_duration {
        return false;
    }
    let now = Instant::now();
    let profile_enabled = ui_profiler.enabled();
    let lateness =
        profile_enabled.then(|| now.duration_since(*last_tick).saturating_sub(tick_duration));
    let elapsed_ms = now.duration_since(*last_tick).as_millis() as u64;
    *last_tick = now;
    flush_pending_encoder_turns(pending_encoder_turns, playback, runner, adapter);
    if now.duration_since(*last_snapshot_request) >= snapshot_interval {
        playback.request_next_snapshot();
        *last_snapshot_request = now;
    }
    let advance_started = profile_enabled.then(Instant::now);
    if let Err(error) = playback.advance(elapsed_ms, runner, adapter) {
        eprintln!("pi playback advance failed: {error}");
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        eprintln!("pi deferred host work failed: {error}");
    }
    if let (Some(lateness), Some(started)) = (lateness, advance_started) {
        ui_profiler.record_runtime(lateness, started.elapsed());
    }
    render_if_due(
        now,
        last_render,
        render_interval,
        playback,
        runner,
        oled,
        trellis,
        neokey,
        render_cache,
        ui_profiler,
    );
    shutdown_if_requested(adapter)
}

pub(crate) fn flush_pending_encoder_turns(
    pending: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    for message in pending.take_messages() {
        dispatch_or_log(playback, runner, adapter, message);
    }
}

fn dispatch_or_log(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    message: HostMessage,
) {
    if let Err(error) = dispatch_runtime_message(playback, runner, adapter, message) {
        eprintln!("pi runtime dispatch failed: {error}");
    }
}

#[allow(clippy::too_many_arguments)]
fn render_if_due(
    now: Instant,
    last_render: &mut Instant,
    render_interval: Duration,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    oled: &mut OledSsd1351,
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    render_cache: &mut HardwareRenderCache,
    ui_profiler: &mut UiProfiler,
) {
    if now.duration_since(*last_render) < render_interval {
        return;
    }
    *last_render = now;
    let mut targets = HardwareRenderTargets {
        oled,
        trellis,
        neokey,
    };
    crate::render_loop::render_latest_snapshot(
        playback,
        runner,
        &mut targets,
        render_cache,
        ui_profiler,
        render_interval,
    );
}

fn shutdown_if_requested(adapter: &mut PiPlaybackHostAdapter) -> bool {
    if !adapter.take_shutdown_request() {
        return false;
    }
    if let Err(error) = shutdown_pi_system() {
        eprintln!("pi shutdown failed: {error}");
        return false;
    }
    true
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
