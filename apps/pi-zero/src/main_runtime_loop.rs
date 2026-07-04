use crate::encoder_queue::PendingEncoderTurns;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::{encoder_press_message, MidiMessage};
use crate::render::{HardwareRenderCache, HardwareRenderTargets};
use crate::runtime_loop::{dispatch_runtime_message, handle_deferred_host_work};
use crate::ui_profile::UiProfiler;
use cellsymphony_hal::encoder_gpio::HardwareEvent;
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

pub(crate) fn drain_host_messages(
    input_rx: &mpsc::Receiver<HostMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    for _ in 0..HARDWARE_EVENT_BUDGET {
        let Ok(message) = input_rx.try_recv() else {
            break;
        };
        dispatch_or_log(playback, runner, adapter, message);
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn maybe_advance_runtime(
    last_tick: &mut Instant,
    tick_duration: Duration,
    last_snapshot_request: &mut Instant,
    snapshot_interval: Duration,
    last_render: &mut Instant,
    render_interval: Duration,
    _pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_cache: &mut HardwareRenderCache,
    targets: &mut HardwareRenderTargets<'_>,
    ui_profiler: &mut UiProfiler,
) -> bool {
    if last_tick.elapsed() < tick_duration {
        return false;
    }
    let now = Instant::now();
    let profile_enabled = ui_profiler.enabled();
    let lateness =
        profile_enabled.then(|| now.duration_since(*last_tick).saturating_sub(tick_duration));
    let elapsed = now.duration_since(*last_tick);
    *last_tick = now;
    if now.duration_since(*last_snapshot_request) >= snapshot_interval {
        playback.request_next_snapshot();
        *last_snapshot_request = now;
    }
    let advance_started = profile_enabled.then(Instant::now);
    if let Err(error) = playback.advance_duration(elapsed, runner, adapter) {
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
        render_cache,
        targets,
        ui_profiler,
    );
    shutdown_if_requested(adapter, targets)
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
    render_cache: &mut HardwareRenderCache,
    targets: &mut HardwareRenderTargets<'_>,
    ui_profiler: &mut UiProfiler,
) {
    if now.duration_since(*last_render) < render_interval {
        return;
    }
    *last_render = now;
    crate::render_loop::render_latest_snapshot(
        playback,
        runner,
        targets,
        render_cache,
        ui_profiler,
        render_interval,
    );
}

fn shutdown_if_requested(
    adapter: &mut PiPlaybackHostAdapter,
    targets: &mut HardwareRenderTargets<'_>,
) -> bool {
    if !adapter.take_shutdown_request() {
        return false;
    }
    crate::render::render_shutdown_splash(targets.oled);
    darken_hardware_for_shutdown(targets);
    if let Err(error) = shutdown_pi_system() {
        eprintln!("pi shutdown failed: {error}");
        return false;
    }
    true
}

fn darken_hardware_for_shutdown(targets: &mut HardwareRenderTargets<'_>) {
    let _ = targets
        .seesaw_tx
        .send(crate::seesaw_io::SeesawCommand::GridFrame([[0; 3]; 64]));
    let _ = targets
        .seesaw_tx
        .send(crate::seesaw_io::SeesawCommand::NeoKeyColors([[0; 3]; 4]));
}

fn shutdown_pi_system() -> Result<(), String> {
    #[cfg(feature = "hardware-pi")]
    {
        let attempts: &[(&str, &[&str])] = &[
            ("systemctl", &["--no-block", "poweroff"]),
            ("sudo", &["-n", "systemctl", "--no-block", "poweroff"]),
            ("sudo", &["-n", "poweroff"]),
        ];
        let mut errors = Vec::new();
        for (command, args) in attempts {
            match std::process::Command::new(command).args(*args).status() {
                Ok(status) if status.success() => return Ok(()),
                Ok(status) => errors.push(format!("{command} {args:?} exited with {status}")),
                Err(error) => errors.push(format!("{command} {args:?} failed to launch: {error}")),
            }
        }
        Err(errors.join("; "))
    }
    #[cfg(not(feature = "hardware-pi"))]
    {
        Ok(())
    }
}
