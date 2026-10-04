use crate::encoder_queue::PendingEncoderTurns;
use crate::hardware_runtime_scheduler::{is_playing, DisplaySnapshotDue, HardwareRuntimeScheduler};
use crate::host_adapter::{PiHostAdapter, PowerRequest};
use crate::power_lifecycle::{
    PowerAction, PowerLifecycle, PowerLifecycleCallbacks, PowerLifecycleResult,
};
use crate::render_loop::RenderWorker;
use crate::runtime_loop::{
    handle_deferred_host_work, process_runtime_output, report_runtime_failure,
};
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{HostAdapter, HostMessage, NativeRunner, PlaybackRuntime};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HARDWARE_EVENT_BUDGET: usize = 16;

#[cfg(all(test, feature = "hardware-raspberry-pi-zero-2w"))]
#[path = "main_runtime_power_tests.rs"]
mod tests;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
#[path = "main_runtime_error_tests.rs"]
mod error_tests;

pub(crate) fn drain_host_messages(
    input_rx: &mpsc::Receiver<HostMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
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
    adapter: &mut PiHostAdapter,
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
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
    ui_profiler: &mut UiProfiler,
    native_scenes: &mut crate::native_scene_pump::NativeScenePump,
) -> bool {
    if adapter.shutdown_pending() {
        return shutdown_if_requested(playback, adapter, render_worker);
    }
    let now = Instant::now();
    let runtime_snapshot_requested = if let Some(advance) =
        scheduler.next_runtime_advance(now, playback, runner.next_xy_glide_deadline())
    {
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
        service_xy_glide_tick(playback, runner, adapter);
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
        scheduler.record_runtime_advance_complete(
            completed_at,
            playback,
            runner.next_xy_glide_deadline(),
        );
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
    native_scenes.poll(runner);
    let native_display_due = if is_playing(playback) {
        scheduler.display_snapshot_due(Instant::now(), runner, playback)
    } else {
        DisplaySnapshotDue::default()
    };
    if let Some(captured_at) = native_scenes.submit(
        Instant::now(),
        native_display_due,
        playback,
        runner,
        adapter,
        render_worker,
    ) {
        scheduler.record_native_scene_capture(captured_at);
    }
    if let Some(duration) = native_scenes.take_capture_duration() {
        ui_profiler.record_scene_capture(duration);
    }
    service_render_if_due(now, scheduler, playback, adapter, render_worker);
    shutdown_if_requested(playback, adapter, render_worker)
}

fn service_xy_glide_tick(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) {
    if runner.next_xy_glide_deadline().is_none() {
        return;
    }
    let message = HostMessage::TransportPulseStep {
        pulses: 0,
        source: playback.config().sync_source.clone(),
        at_ppqn_pulse: playback
            .last_status()
            .map(|status| status.current_ppqn_pulse),
        request_snapshot: Some(false),
    };
    match playback.dispatch_host_message_music_first(message, runner, adapter) {
        Ok(output) => {
            if let Err(error) = process_runtime_output(playback, runner, adapter, output) {
                report_runtime_failure(adapter, "pi XY glide output processing failed", error);
            }
        }
        Err(error) => report_runtime_failure(adapter, "pi XY glide runtime tick failed", error),
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        report_runtime_failure(adapter, "pi XY glide deferred host work failed", error);
    }
}

fn advance_playback_if_due(
    elapsed: Duration,
    lateness: Duration,
    request_snapshot: bool,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
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
    match playback.advance_duration_music_first_with_output(elapsed, runner, adapter) {
        Ok(output) => {
            if let Err(error) = process_runtime_output(playback, runner, adapter, output) {
                report_runtime_failure(adapter, "pi playback output processing failed", error);
            }
        }
        Err(error) => report_runtime_failure(adapter, "pi playback advance failed", error),
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        report_runtime_failure(adapter, "pi deferred host work failed", error);
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
    adapter: &mut PiHostAdapter,
) {
    if adapter.shutdown_pending() || is_playing(playback) {
        return;
    }
    let due = scheduler.display_snapshot_due(now, runner, playback);
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
    adapter: &mut PiHostAdapter,
    message: HostMessage,
) {
    if let Err(error) = crate::runtime_loop::dispatch(playback, runner, adapter, message) {
        report_runtime_failure(adapter, "pi runtime dispatch failed", error);
    }
}

fn service_render_if_due(
    now: Instant,
    scheduler: &mut HardwareRuntimeScheduler,
    playback: &mut PlaybackRuntime,
    adapter: &mut PiHostAdapter,
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
    let Some(snapshot) = playback.last_snapshot().cloned() else {
        return;
    };
    let oled = match adapter.core.oled_publication_for_snapshot(&snapshot, false) {
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
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    let Some(request) = adapter.take_power_request() else {
        return false;
    };
    match request {
        PowerRequest::Reboot | PowerRequest::Shutdown => {
            let action = match request {
                PowerRequest::Reboot => PowerAction::Reboot,
                PowerRequest::Shutdown => PowerAction::Shutdown,
                PowerRequest::ApplyDeviceConfig(()) => unreachable!(),
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
        PowerRequest::ApplyDeviceConfig(()) => {
            finalize_device_apply_power_request(playback, adapter, render_worker, request)
        }
    }
}

struct RaspberryPowerCallbacks<'a> {
    playback: &'a PlaybackRuntime,
    adapter: &'a mut PiHostAdapter,
    render_worker: &'a RenderWorker,
    request: PowerRequest,
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
            .core
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
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
    request: PowerRequest,
) -> bool {
    let terminal = (|| {
        let snapshot = playback
            .last_snapshot()
            .cloned()
            .ok_or_else(|| "pi power request has no latest native snapshot".to_string())?;
        let oled = adapter
            .core
            .oled_publication_for_snapshot(&snapshot, false)?;
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

fn power_pi_system(_request: PowerRequest) -> Result<(), String> {
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
                PowerRequest::Reboot => {
                    orange_power_result("reboot", crate::orange_reboot::request_reboot())
                }
                PowerRequest::Shutdown => {
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
    request: PowerRequest,
) -> &'static [(&'static str, &'static [&'static str])] {
    match request {
        PowerRequest::Reboot => &[
            ("sudo", &["-n", "/usr/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/bin/systemctl", "reboot"]),
            ("sudo", &["-n", "/usr/sbin/reboot"]),
            ("sudo", &["-n", "/sbin/reboot"]),
            ("/usr/bin/systemctl", &["reboot"]),
            ("/bin/systemctl", &["reboot"]),
            ("/usr/sbin/reboot", &[]),
            ("/sbin/reboot", &[]),
        ],
        PowerRequest::Shutdown => &[
            ("sudo", &["-n", "/usr/bin/systemctl", "poweroff"]),
            ("sudo", &["-n", "/bin/systemctl", "poweroff"]),
            ("sudo", &["-n", "/usr/sbin/poweroff"]),
            ("sudo", &["-n", "/sbin/poweroff"]),
            ("/usr/bin/systemctl", &["poweroff"]),
            ("/bin/systemctl", &["poweroff"]),
            ("/usr/sbin/poweroff", &[]),
            ("/sbin/poweroff", &[]),
        ],
        PowerRequest::ApplyDeviceConfig(()) => &[
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
