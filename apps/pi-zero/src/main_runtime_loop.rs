use crate::encoder_queue::PendingEncoderTurns;
use crate::host_adapter::{PiPlaybackHostAdapter, PiPowerRequest};
use crate::input::encoder_press_message;
use crate::render_loop::RenderWorker;
use crate::runtime_loop::{
    dispatch_runtime_message, handle_deferred_host_work, process_runtime_output,
};
use crate::snapshot_cadence::SnapshotCadence;
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimeTransportState, SyncSource,
};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[path = "power_lifecycle.rs"]
mod power_lifecycle;

const HARDWARE_EVENT_BUDGET: usize = 16;
const IDLE_MAINTENANCE_INTERVAL: Duration = Duration::from_millis(50);

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
                crate::wake_trace::log_encoder_event(event);
                pending_encoder_turns.enqueue(id, delta);
                continue;
            }
            HardwareEvent::EncoderPress { id } => {
                crate::wake_trace::log_encoder_event(event);
                flush_pending_encoder_turns(pending_encoder_turns, playback, runner, adapter);
                encoder_press_message(id)
            }
            HardwareEvent::EncoderRelease { .. } => {
                crate::wake_trace::log_encoder_event(event);
                continue;
            }
        };
        dispatch_or_log(playback, runner, adapter, message);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn maybe_advance_runtime(
    last_tick: &mut Instant,
    tick_duration: Duration,
    snapshot_interval: Duration,
    last_render: &mut Instant,
    render_interval: Duration,
    snapshot_cadence: &mut SnapshotCadence,
    last_published_snapshot_revision: &mut u64,
    _pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
    ui_profiler: &mut UiProfiler,
) -> bool {
    let now = Instant::now();
    snapshot_cadence.observe_accepted_snapshot(now, playback.last_snapshot_revision());
    let effective_tick_duration = if runtime_tick_needed(playback) {
        tick_duration
    } else {
        IDLE_MAINTENANCE_INTERVAL
    };
    if now.duration_since(*last_tick) >= effective_tick_duration {
        advance_playback_if_due(
            now,
            last_tick,
            effective_tick_duration,
            snapshot_cadence,
            snapshot_interval,
            playback,
            runner,
            adapter,
            ui_profiler,
        );
        snapshot_cadence
            .observe_accepted_snapshot(Instant::now(), playback.last_snapshot_revision());
    }
    request_periodic_snapshot_if_due(now, snapshot_cadence, playback, runner, adapter);
    snapshot_cadence.observe_accepted_snapshot(Instant::now(), playback.last_snapshot_revision());
    service_render_if_due(
        now,
        last_render,
        render_interval,
        last_published_snapshot_revision,
        playback,
        adapter,
        render_worker,
    );
    shutdown_if_requested(playback, adapter, render_worker)
}

#[allow(clippy::too_many_arguments)]
fn advance_playback_if_due(
    now: Instant,
    last_tick: &mut Instant,
    tick_duration: Duration,
    snapshot_cadence: &SnapshotCadence,
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
    if transport_snapshot_due(now, snapshot_cadence, snapshot_interval, playback) {
        playback.request_next_snapshot();
    }
    let advance_started = profile_enabled.then(Instant::now);
    match playback.advance_duration_with_output(elapsed, runner, adapter) {
        Ok(output) => {
            if let Err(error) = process_runtime_output(playback, runner, adapter, output) {
                eprintln!("pi playback output processing failed: {error}");
            }
        }
        Err(error) => eprintln!("pi playback advance failed: {error}"),
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        eprintln!("pi deferred host work failed: {error}");
    }
    if let (Some(lateness), Some(started)) = (lateness, advance_started) {
        ui_profiler.record_runtime(lateness, started.elapsed());
    }
}

fn transport_snapshot_due(
    now: Instant,
    snapshot_cadence: &SnapshotCadence,
    snapshot_interval: Duration,
    playback: &PlaybackRuntime,
) -> bool {
    is_internal_playing(playback) && snapshot_cadence.periodic_due(now, snapshot_interval)
}

fn request_periodic_snapshot_if_due(
    now: Instant,
    snapshot_cadence: &SnapshotCadence,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    if !snapshot_cadence.timed_display_due(now, runner) {
        return;
    }
    let message = periodic_snapshot_message(playback);
    dispatch_or_log(playback, runner, adapter, message);
}

fn periodic_snapshot_message(playback: &PlaybackRuntime) -> HostMessage {
    HostMessage::TransportPulseStep {
        pulses: 0,
        source: playback.config().sync_source.clone(),
        at_ppqn_pulse: playback
            .last_status()
            .map(|status| status.current_ppqn_pulse),
        request_snapshot: Some(true),
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

fn runtime_tick_needed(playback: &PlaybackRuntime) -> bool {
    playback.has_scheduled_midi() || is_internal_playing(playback)
}

#[allow(clippy::too_many_arguments)]
fn service_render_if_due(
    now: Instant,
    last_render: &mut Instant,
    render_interval: Duration,
    last_published_snapshot_revision: &mut u64,
    playback: &mut PlaybackRuntime,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) {
    if now.duration_since(*last_render) < render_interval {
        return;
    }
    *last_render = now;
    let snapshot_revision = playback.last_snapshot_revision();
    if snapshot_revision == 0 {
        return;
    }
    let snapshot_changed = *last_published_snapshot_revision != snapshot_revision;
    if !snapshot_changed {
        return;
    }
    let Some(snapshot) = crate::runtime_loop::latest_snapshot(playback).cloned() else {
        return;
    };
    let oled = match adapter.oled_publication_for_snapshot(&snapshot, false) {
        Ok(oled) => oled,
        Err(error) => {
            eprintln!("pi OLED publication unavailable: {error}");
            return;
        }
    };
    if !render_worker.publish_snapshot(snapshot, oled) {
        eprintln!("pi render worker rejected snapshot publication");
        return;
    }
    *last_published_snapshot_revision = snapshot_revision;
}

fn shutdown_if_requested(
    playback: &PlaybackRuntime,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    let Some(request) = adapter.take_power_request() else {
        return false;
    };
    power_lifecycle::finalize_power_request(
        || {
            let snapshot = playback
                .last_snapshot()
                .cloned()
                .ok_or_else(|| "pi power request has no latest native snapshot".to_string())?;
            let oled = adapter.oled_publication_for_snapshot(&snapshot, false)?;
            render_worker.publish_terminal_preserving(snapshot, oled)
        },
        || power_pi_system(request),
    )
}

fn power_pi_system(_request: PiPowerRequest) -> Result<(), String> {
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
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
    #[cfg(not(feature = "hardware-raspberry-pi-zero-2w"))]
    {
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        {
            match _request {
                PiPowerRequest::Reboot => {
                    orange_power_result("reboot", crate::orange_reboot::request_reboot())
                }
                PiPowerRequest::Shutdown => {
                    orange_power_result("poweroff", crate::orange_reboot::request_shutdown())
                }
            }
        }
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        {
            let _ = _request;
            Err("power request is unavailable in this profile".into())
        }
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn orange_power_result(
    action: &str,
    outcome: crate::orange_reboot::OrangeHelperOutcome,
) -> Result<(), String> {
    match outcome {
        crate::orange_reboot::OrangeHelperOutcome::Accepted => Ok(()),
        crate::orange_reboot::OrangeHelperOutcome::Rejected => {
            Err(format!("Orange {action} request was rejected"))
        }
        crate::orange_reboot::OrangeHelperOutcome::NotSubmitted => {
            Err(format!("Orange {action} request was not submitted"))
        }
        crate::orange_reboot::OrangeHelperOutcome::Indeterminate => {
            Err(format!("Orange {action} request outcome is indeterminate"))
        }
    }
}

#[cfg(feature = "hardware-raspberry-pi-zero-2w")]
fn power_command_attempts(
    request: PiPowerRequest,
) -> &'static [(&'static str, &'static [&'static str])] {
    match request {
        PiPowerRequest::Reboot => &[
            ("sudo", &["-n", "/usr/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/usr/sbin/reboot"]),
            ("sudo", &["-n", "/sbin/reboot"]),
            ("/usr/bin/systemctl", &["reboot"]),
            ("/bin/systemctl", &["reboot"]),
            ("/usr/sbin/reboot", &[]),
            ("/sbin/reboot", &[]),
        ],
        PiPowerRequest::Shutdown => &[
            ("sudo", &["-n", "/usr/bin/systemctl", "poweroff"]),
            ("sudo", &["-n", "/bin/systemctl", "poweroff"]),
            ("sudo", &["-n", "/usr/sbin/poweroff"]),
            ("sudo", &["-n", "/sbin/poweroff"]),
            ("/usr/bin/systemctl", &["poweroff"]),
            ("/bin/systemctl", &["poweroff"]),
            ("/usr/sbin/poweroff", &[]),
            ("/sbin/poweroff", &[]),
        ],
    }
}

#[cfg(test)]
mod periodic_snapshot_tests {
    use super::*;
    use playback_runtime::RuntimeConfig;

    #[test]
    fn periodic_snapshot_message_requests_snapshot_without_advancing_pulses() {
        let playback = PlaybackRuntime::new(RuntimeConfig {
            sync_source: SyncSource::External,
            ..RuntimeConfig::default()
        });

        let HostMessage::TransportPulseStep {
            pulses,
            source,
            request_snapshot,
            ..
        } = periodic_snapshot_message(&playback)
        else {
            panic!("expected transport snapshot request");
        };

        assert_eq!(pulses, 0);
        assert_eq!(source, SyncSource::External);
        assert_eq!(request_snapshot, Some(true));
    }

    #[test]
    fn stopped_idle_maintenance_does_not_claim_snapshot_deadlines() {
        let playback = PlaybackRuntime::new(RuntimeConfig::default());
        let now = Instant::now();
        let stale_snapshot = now - Duration::from_secs(5);
        let snapshot_cadence = SnapshotCadence::new(stale_snapshot, 0);

        assert!(!transport_snapshot_due(
            now,
            &snapshot_cadence,
            Duration::from_millis(16),
            &playback
        ));
    }
}

#[cfg(all(test, feature = "hardware-raspberry-pi-zero-2w"))]
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
