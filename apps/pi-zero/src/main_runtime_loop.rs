use crate::encoder_queue::PendingEncoderTurns;
use crate::hardware_runtime_scheduler::{
    prepare_dispatch_message, DisplaySnapshotDue, HardwareRuntimeScheduler,
};
use crate::host_adapter::{PiPlaybackHostAdapter, PiPowerRequest};
use crate::power_lifecycle::{
    PowerAction, PowerLifecycle, PowerLifecycleCallbacks, PowerLifecycleResult,
};
use crate::render_loop::RenderWorker;
use crate::runtime_loop::{
    dispatch_runtime_message, handle_deferred_host_work, process_runtime_output,
};
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{HostAdapter, HostMessage, NativeRunner, PlaybackRuntime};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HARDWARE_EVENT_BUDGET: usize = 16;

pub(crate) fn drain_host_messages(
    input_rx: &mpsc::Receiver<HostMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    if adapter.shutdown_pending() {
        return;
    }
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
    if adapter.shutdown_pending() {
        return;
    }
    let _ = crate::encoder_queue::drain_encoder_events(
        event_rx,
        pending_encoder_turns,
        |message| {
            if adapter.shutdown_pending() {
                return Err(());
            }
            dispatch_or_log(playback, runner, adapter, message);
            Ok::<(), ()>(())
        },
        crate::wake_trace::log_encoder_event,
    );
}

pub(crate) fn maybe_advance_runtime(
    scheduler: &mut HardwareRuntimeScheduler,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
    ui_profiler: &mut UiProfiler,
) -> bool {
    if adapter.shutdown_pending() {
        return shutdown_if_requested(playback, adapter, render_worker);
    }
    let now = Instant::now();
    let runtime_snapshot_requested =
        if let Some(advance) = scheduler.next_runtime_advance(now, playback) {
            let request_snapshot = advance.request_snapshot;
            let revision_before = playback.last_snapshot_revision();
            advance_playback_if_due(
                advance.elapsed,
                advance.lateness,
                request_snapshot,
                playback,
                runner,
                adapter,
                ui_profiler,
            );
            let revision_after = playback.last_snapshot_revision();
            let completed_at = Instant::now();
            if request_snapshot {
                scheduler.record_snapshot_attempt(
                    completed_at,
                    DisplaySnapshotDue::default(),
                    revision_before,
                    revision_after,
                );
            } else {
                scheduler.observe_snapshot_revision(completed_at, revision_before, revision_after);
            }
            scheduler.record_runtime_advance_complete(completed_at, playback);
            if adapter.shutdown_pending() {
                return shutdown_if_requested(playback, adapter, render_worker);
            }
            request_snapshot
        } else {
            scheduler.observe_snapshot(Instant::now(), playback);
            false
        };
    if !runtime_snapshot_requested {
        request_periodic_snapshot_if_due(now, scheduler, playback, runner, adapter);
    }
    service_render_if_due(now, scheduler, playback, adapter, render_worker);
    shutdown_if_requested(playback, adapter, render_worker)
}

fn advance_playback_if_due(
    elapsed: Duration,
    lateness: Duration,
    request_snapshot: bool,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    ui_profiler: &mut UiProfiler,
) {
    if adapter.shutdown_pending() {
        return;
    }
    let profile_enabled = ui_profiler.enabled();
    if request_snapshot {
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
    if let Some(started) = advance_started {
        ui_profiler.record_runtime(lateness, started.elapsed());
    }
}

fn request_periodic_snapshot_if_due(
    now: Instant,
    scheduler: &mut HardwareRuntimeScheduler,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    if adapter.shutdown_pending() {
        return;
    }
    let due = scheduler.display_snapshot_due(now, runner);
    if !due.any() {
        return;
    }
    let revision_before = playback.last_snapshot_revision();
    dispatch_or_log(
        playback,
        runner,
        adapter,
        scheduler.display_snapshot_message(playback),
    );
    let revision_after = playback.last_snapshot_revision();
    scheduler.record_snapshot_attempt(now, due, revision_before, revision_after);
}

fn dispatch_or_log(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    message: HostMessage,
) {
    if adapter.shutdown_pending() {
        return;
    }
    let dispatch = adapter.handle_transfer_input(&message);
    dispatch_transfer_statuses(playback, runner, adapter);
    if !dispatch {
        return;
    }
    let message = prepare_dispatch_message(playback, message);
    if let Err(error) = dispatch_runtime_message(playback, runner, adapter, message) {
        eprintln!("pi runtime dispatch failed: {error}");
    }
}

fn service_render_if_due(
    now: Instant,
    scheduler: &mut HardwareRuntimeScheduler,
    playback: &mut PlaybackRuntime,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) {
    if adapter.shutdown_pending() {
        return;
    }
    if !scheduler.snapshot_publication_due(now, playback) {
        return;
    }
    scheduler.record_snapshot_publication_attempt(now);
    let snapshot_revision = playback.last_snapshot_revision();
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
    let accepted = render_worker.publish_snapshot(snapshot, oled);
    if !accepted {
        eprintln!("pi render worker rejected snapshot publication");
    } else {
        scheduler.record_snapshot_publication_accepted(snapshot_revision);
    }
}

fn shutdown_if_requested(
    playback: &PlaybackRuntime,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    let Some(request) = adapter.take_power_request() else {
        return false;
    };
    match request {
        PiPowerRequest::Reboot | PiPowerRequest::Shutdown => {
            let action = match request {
                PiPowerRequest::Reboot => PowerAction::Reboot,
                PiPowerRequest::Shutdown => PowerAction::Shutdown,
                PiPowerRequest::ApplyDeviceConfigReboot => unreachable!(),
            };
            let mut callbacks = RaspberryPowerCallbacks {
                playback,
                adapter,
                render_worker,
                request,
            };
            let mut lifecycle = PowerLifecycle::default();
            report_power_lifecycle_result(lifecycle.execute(action, &mut callbacks))
        }
        PiPowerRequest::ApplyDeviceConfigReboot => {
            finalize_device_apply_power_request(playback, adapter, render_worker, request)
        }
    }
}

struct RaspberryPowerCallbacks<'a> {
    playback: &'a PlaybackRuntime,
    adapter: &'a mut PiPlaybackHostAdapter,
    render_worker: &'a RenderWorker,
    request: PiPowerRequest,
}

impl PowerLifecycleCallbacks for RaspberryPowerCallbacks<'_> {
    fn save_recovery(&mut self) -> Result<(), String> {
        self.adapter.save_recovery_for_power()
    }

    fn panic_external_midi(&mut self) -> Result<(), String> {
        HostAdapter::panic_external_midi(self.adapter).map_err(|error| error.to_string())
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        HostAdapter::silence_internal_audio(self.adapter).map_err(|error| error.to_string())
    }

    fn acknowledge_terminal(&mut self, _action: PowerAction) -> Result<(), String> {
        let snapshot = self
            .playback
            .last_snapshot()
            .cloned()
            .ok_or_else(|| "pi power request has no latest native snapshot".to_string())?;
        let oled = self
            .adapter
            .oled_publication_for_snapshot(&snapshot, false)?;
        self.render_worker
            .publish_terminal_preserving(snapshot, oled)
    }

    fn submit_power(&mut self, _action: PowerAction) -> Result<(), String> {
        power_pi_system(self.request)
    }
}

fn report_power_lifecycle_result(result: PowerLifecycleResult) -> bool {
    match result {
        PowerLifecycleResult::Submitted => true,
        PowerLifecycleResult::Failed(failure) => {
            eprintln!("pi power lifecycle failed: {failure}");
            failure.accepted
        }
        PowerLifecycleResult::Duplicate => {
            eprintln!("pi power lifecycle rejected a duplicate request");
            true
        }
    }
}

fn finalize_device_apply_power_request(
    playback: &PlaybackRuntime,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
    request: PiPowerRequest,
) -> bool {
    let terminal = (|| {
        let snapshot = playback
            .last_snapshot()
            .cloned()
            .ok_or_else(|| "pi power request has no latest native snapshot".to_string())?;
        let oled = adapter.oled_publication_for_snapshot(&snapshot, false)?;
        render_worker.publish_terminal_preserving(snapshot, oled)
    })();
    if let Err(error) = terminal {
        eprintln!("pi device-apply terminal render failed: {error}");
        return true;
    }
    if let Err(error) = power_pi_system(request) {
        eprintln!("pi device-apply power request failed: {error}");
    }
    true
}

fn dispatch_transfer_statuses(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
) {
    while let Some(status) = adapter.take_transfer_status() {
        if let Err(error) = dispatch_runtime_message(playback, runner, adapter, status) {
            eprintln!("pi transfer status dispatch failed: {error}");
            break;
        }
    }
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
    outcome: crate::orange_reboot::OrangePowerRequestOutcome,
) -> Result<(), String> {
    match outcome {
        crate::orange_reboot::OrangePowerRequestOutcome::Accepted => Ok(()),
        crate::orange_reboot::OrangePowerRequestOutcome::Rejected => {
            Err(format!("Orange {action} request was rejected"))
        }
        crate::orange_reboot::OrangePowerRequestOutcome::NotSubmitted => {
            Err(format!("Orange {action} request was not submitted"))
        }
        crate::orange_reboot::OrangePowerRequestOutcome::Indeterminate => {
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
        PiPowerRequest::ApplyDeviceConfigReboot => &[
            ("sudo", &["-n", "/usr/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/usr/sbin/reboot"]),
            ("sudo", &["-n", "/sbin/reboot"]),
            ("/usr/bin/systemctl", &["reboot"]),
            ("/bin/systemctl", &["reboot"]),
            ("/usr/sbin/reboot", &[]),
            ("/sbin/reboot", &[]),
        ],
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
