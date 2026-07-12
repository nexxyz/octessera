use crate::encoder_queue::PendingEncoderTurns;
use crate::host_adapter::{PiPlaybackHostAdapter, PiPowerRequest};
use crate::input::{encoder_press_message, MidiMessage};
use crate::render_loop::RenderWorker;
use crate::runtime_loop::{dispatch_runtime_message, handle_deferred_host_work};
use crate::temporary_neokey_hack::TemporaryNeoKeyHack;
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimeTransportState, SyncSource,
};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HARDWARE_EVENT_BUDGET: usize = 16;
const MIDI_REALTIME_BUDGET: usize = 32;

pub(crate) fn drain_midi_messages(
    midi_rx: &mpsc::Receiver<MidiMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    for _ in 0..MIDI_REALTIME_BUDGET {
        let Ok(message) = midi_rx.try_recv() else {
            break;
        };
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
    temporary_neokey_hack: &mut TemporaryNeoKeyHack,
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
                crate::wake_trace::log_encoder_event(event);
                if let Some(messages) = temporary_neokey_hack.encoder_turn_messages(id, delta) {
                    for message in messages {
                        dispatch_or_log(playback, runner, adapter, message);
                    }
                    continue;
                }
                pending_encoder_turns.enqueue(id, delta);
                continue;
            }
            HardwareEvent::EncoderPress { id } => {
                crate::wake_trace::log_encoder_event(event);
                flush_pending_encoder_turns(pending_encoder_turns, playback, runner, adapter);
                if let Some(messages) = temporary_neokey_hack.encoder_press_messages(id) {
                    for message in messages {
                        dispatch_or_log(playback, runner, adapter, message);
                    }
                    continue;
                }
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
    render_worker: &RenderWorker,
    ui_profiler: &mut UiProfiler,
) -> bool {
    let now = Instant::now();
    if now.duration_since(*last_tick) >= tick_duration {
        advance_playback_if_due(
            now,
            last_tick,
            tick_duration,
            last_snapshot_request,
            snapshot_interval,
            playback,
            runner,
            adapter,
            ui_profiler,
        );
    }
    service_render_if_due(
        now,
        last_render,
        render_interval,
        playback,
        runner,
        render_worker,
    );
    shutdown_if_requested(adapter, render_worker)
}

#[allow(clippy::too_many_arguments)]
fn advance_playback_if_due(
    now: Instant,
    last_tick: &mut Instant,
    tick_duration: Duration,
    last_snapshot_request: &mut Instant,
    snapshot_interval: Duration,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    ui_profiler: &mut UiProfiler,
) {
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
    let message = prepare_dispatch_message(playback, message);
    if let Err(error) = dispatch_runtime_message(playback, runner, adapter, message) {
        eprintln!("pi runtime dispatch failed: {error}");
    }
}

fn prepare_dispatch_message(playback: &PlaybackRuntime, message: HostMessage) -> HostMessage {
    match message {
        HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        } if is_internal_playing(playback) => HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        },
        other => other,
    }
}

fn is_internal_playing(playback: &PlaybackRuntime) -> bool {
    playback.config().sync_source == SyncSource::Internal
        && playback
            .last_status()
            .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
}

fn service_render_if_due(
    now: Instant,
    last_render: &mut Instant,
    render_interval: Duration,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    render_worker: &RenderWorker,
) {
    if now.duration_since(*last_render) < render_interval {
        return;
    }
    *last_render = now;
    let Some(snapshot) = crate::runtime_loop::latest_snapshot(playback).cloned() else {
        return;
    };
    let pulses = playback.drain_ui_pulses();
    if !crate::runtime_loop::playback_config_matches_snapshot(playback, &snapshot) {
        crate::runtime_loop::sync_playback_config_from_snapshot(playback, runner, &snapshot);
    }
    render_worker.publish_snapshot(snapshot, pulses);
}

fn shutdown_if_requested(
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    let Some(request) = adapter.take_power_request() else {
        return false;
    };
    if !render_worker.publish_shutdown() {
        eprintln!("pi shutdown render acknowledgement timed out");
    }
    if let Err(error) = power_pi_system(request) {
        eprintln!("pi power request failed: {error}");
        return false;
    }
    true
}

fn power_pi_system(_request: PiPowerRequest) -> Result<(), String> {
    #[cfg(feature = "hardware-rpi-zero-2w")]
    {
        let attempts = power_command_attempts(_request);
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
    #[cfg(not(feature = "hardware-rpi-zero-2w"))]
    {
        Ok(())
    }
}

#[cfg(feature = "hardware-rpi-zero-2w")]
fn power_command_attempts(
    request: PiPowerRequest,
) -> &'static [(&'static str, &'static [&'static str])] {
    match request {
        PiPowerRequest::Reboot => &[
            ("/usr/bin/systemctl", &["reboot"]),
            ("/bin/systemctl", &["reboot"]),
            ("/usr/sbin/reboot", &[]),
            ("/sbin/reboot", &[]),
            ("sudo", &["-n", "/usr/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/usr/sbin/reboot"]),
            ("sudo", &["-n", "/sbin/reboot"]),
        ],
        PiPowerRequest::Shutdown => &[
            ("/usr/bin/systemctl", &["poweroff"]),
            ("/bin/systemctl", &["poweroff"]),
            ("/usr/sbin/poweroff", &[]),
            ("/sbin/poweroff", &[]),
            ("sudo", &["-n", "/usr/bin/systemctl", "poweroff"]),
            ("sudo", &["-n", "/bin/systemctl", "poweroff"]),
            ("sudo", &["-n", "/usr/sbin/poweroff"]),
            ("sudo", &["-n", "/sbin/poweroff"]),
        ],
    }
}

#[cfg(all(test, feature = "hardware-rpi-zero-2w"))]
mod tests {
    use super::*;

    #[test]
    fn power_command_attempts_match_shutdown_sudoers_shape() {
        let shutdown = power_command_attempts(PiPowerRequest::Shutdown);
        assert!(shutdown
            .iter()
            .any(|attempt| *attempt == ("/usr/bin/systemctl", &["poweroff"])));
        assert!(shutdown
            .iter()
            .any(|attempt| *attempt == ("sudo", &["-n", "/usr/bin/systemctl", "poweroff"])));
        assert!(!shutdown
            .iter()
            .any(|(_, args)| args.contains(&"--no-block")));

        let reboot = power_command_attempts(PiPowerRequest::Reboot);
        assert!(reboot
            .iter()
            .any(|attempt| *attempt == ("/usr/bin/systemctl", &["reboot"])));
        assert!(reboot
            .iter()
            .any(|attempt| *attempt == ("sudo", &["-n", "/usr/bin/systemctl", "reboot"])));
        assert!(!reboot.iter().any(|(_, args)| args.contains(&"--no-block")));
    }
}
