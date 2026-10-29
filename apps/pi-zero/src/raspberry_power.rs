//! Raspberry power requests: the ordinary reboot/shutdown/app-restart lifecycle;
//! reboot and shutdown end in a systemctl/reboot command.

use crate::host_adapter::{PiHostAdapter, PowerRequest};
use crate::host_power_lifecycle::run_ordinary_power_lifecycle;
use crate::power_lifecycle::{PowerAction, PowerLifecycleResult};
use crate::render_loop::RenderWorker;
use playback_runtime::{NativeRunner, PlaybackRuntime};

#[cfg(all(test, feature = "hardware-raspberry-pi-zero-2w"))]
mod tests;

pub(crate) fn shutdown_if_requested(
    playback: &PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    let Some(request) = adapter.take_power_request() else {
        return false;
    };
    let action = match request {
        PowerRequest::Reboot => PowerAction::Reboot,
        PowerRequest::Shutdown => PowerAction::Shutdown,
        PowerRequest::RestartApp => PowerAction::RestartApp,
    };
    report_power_lifecycle_result(run_ordinary_power_lifecycle(
        playback,
        runner,
        adapter,
        render_worker,
        action,
        |_| power_pi_system(request),
    ))
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

fn power_pi_system(request: PowerRequest) -> Result<(), String> {
    if request == PowerRequest::RestartApp {
        crate::power_lifecycle::submit_app_restart();
        return Ok(());
    }
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    {
        let attempts = power_command_attempts(request);
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
        Err("power request is unavailable in this profile".into())
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
        PowerRequest::RestartApp => &[],
    }
}
