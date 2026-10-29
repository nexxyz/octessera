//! Live device IO apply on both boards. The platform worker saves the System
//! file the USB gadget helpers read, then asks the board to rebuild the gadget
//! or switch the USB role. A failed live step keeps the saved System, so the
//! next boot applies it; during an SD2 transfer the USB step waits for the
//! transfer to stop.

use playback_runtime::{RuntimeStoreResult, UsbDataRole};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) const APPLY_FAILED_MESSAGE: &str = "USB apply failed, reboot to apply";
const SD2_PENDING_MESSAGE: &str = "Saved, applies when SD2 stops";
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UsbChange {
    Gadget,
    Role(UsbDataRole),
}

pub(crate) type UsbApplier = Arc<dyn Fn(UsbChange) -> Result<(), String> + Send + Sync>;

/// What the platform worker needs for a device apply: the board's USB helper
/// and the SD2 transfer state file.
pub(crate) struct DeviceApply {
    pub(crate) usb_applier: UsbApplier,
    pub(crate) storage_state: PathBuf,
}

impl DeviceApply {
    pub(crate) fn apply(
        &self,
        change: Option<UsbChange>,
        save_system: impl FnOnce() -> Result<(), String>,
    ) -> RuntimeStoreResult {
        let saved = sd2_active(change, &self.storage_state).and_then(|active| {
            if active && change == Some(UsbChange::Role(UsbDataRole::Host)) {
                return Err("Stop SD2 transfer first".to_string());
            }
            save_system().map(|()| active)
        });
        match saved {
            Ok(true) => status(true, SD2_PENDING_MESSAGE),
            Ok(false) => apply_usb(change, &self.usb_applier),
            Err(message) => {
                eprintln!("device apply save failed: {message}");
                status(false, &message)
            }
        }
    }
}

/// The USB helpers refuse to touch the gadget during an SD2 transfer; stopping
/// the transfer rebuilds it from the saved System instead.
fn sd2_active(change: Option<UsbChange>, storage_state: &Path) -> Result<bool, String> {
    if change.is_none() {
        return Ok(false);
    }
    match std::fs::symlink_metadata(storage_state) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "USB SD2 storage state cannot be inspected: {error}"
        )),
    }
}

fn apply_usb(change: Option<UsbChange>, applier: &UsbApplier) -> RuntimeStoreResult {
    let Some(change) = change else {
        return status(true, "Applied");
    };
    match applier(change) {
        Ok(()) => status(true, "USB ready"),
        Err(error) => {
            eprintln!("USB {change:?} apply failed: {error}");
            status(false, APPLY_FAILED_MESSAGE)
        }
    }
}

fn status(ok: bool, message: &str) -> RuntimeStoreResult {
    RuntimeStoreResult::DeviceConfigStatus {
        ok,
        message: message.into(),
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn production_usb_applier() -> UsbApplier {
    Arc::new(|change| match change {
        UsbChange::Gadget => run_unit("restart", "octessera-usb-gadget.service"),
        UsbChange::Role(role) => run_unit(
            "start",
            &format!("octessera-usb-role@{}.service", role.as_str()),
        ),
    })
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn production_usb_applier() -> UsbApplier {
    Arc::new(|change| match change {
        UsbChange::Gadget => match crate::orange_reboot::request_usb_gadget() {
            crate::orange_reboot::OrangePowerRequestOutcome::Accepted => Ok(()),
            outcome => Err(format!("USB gadget request outcome: {outcome:?}")),
        },
        UsbChange::Role(_) => Err("Orange has no USB role switch".into()),
    })
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
/// Root work runs as its own systemd unit: sudo from inside the app's service
/// would inherit its narrow capability bounding set.
fn run_unit(verb: &str, unit: &str) -> Result<(), String> {
    let mut command = std::process::Command::new("sudo");
    command.args(["-n", "/usr/bin/systemctl", verb, unit]);
    run_bounded(command, HELPER_TIMEOUT).map_err(|error| format!("{unit}: {error}"))
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn run_bounded(
    mut command: std::process::Command,
    timeout: std::time::Duration,
) -> Result<(), String> {
    use std::io::Read;
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to launch: {error}"))?;
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timed out after {timeout:?}"));
            }
            Err(error) => return Err(format!("wait failed: {error}")),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut detail = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut detail);
    }
    Err(if detail.trim().is_empty() {
        format!("exited with {status}")
    } else {
        detail.trim().to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sd2_transfer_refuses_host_and_defers_other_usb_steps_to_its_stop() {
        let root = crate::test_temp_dir::unique_temp_path("octessera-device-apply");
        std::fs::create_dir_all(&root).unwrap();
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = calls.clone();
        let device_apply = DeviceApply {
            usb_applier: Arc::new(move |change| {
                observed.lock().unwrap().push(change);
                Ok(())
            }),
            storage_state: root.join("storage.state"),
        };
        let gadget = Some(UsbChange::Gadget);
        let host = Some(UsbChange::Role(UsbDataRole::Host));
        assert_eq!(
            device_apply.apply(gadget, || Ok(())),
            status(true, "USB ready")
        );
        std::fs::write(&device_apply.storage_state, b"active").unwrap();
        let mut saves = 0;
        assert_eq!(
            device_apply.apply(host, || {
                saves += 1;
                Ok(())
            }),
            status(false, "Stop SD2 transfer first")
        );
        for change in [gadget, Some(UsbChange::Role(UsbDataRole::Gadget))] {
            assert_eq!(
                device_apply.apply(change, || {
                    saves += 1;
                    Ok(())
                }),
                status(true, SD2_PENDING_MESSAGE)
            );
        }
        assert_eq!(device_apply.apply(None, || Ok(())), status(true, "Applied"));
        assert_eq!(saves, 2);
        assert_eq!(*calls.lock().unwrap(), vec![UsbChange::Gadget]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(all(unix, not(feature = "hardware-orange-pi-zero-2w")))]
    #[test]
    fn a_hung_helper_is_killed_at_the_timeout() {
        let mut command = std::process::Command::new("sleep");
        command.arg("5");
        let started = std::time::Instant::now();
        let error = run_bounded(command, std::time::Duration::from_millis(100)).unwrap_err();
        assert!(error.starts_with("timed out"), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn usb_steps_report_ready_or_a_reboot_hint() {
        let ok: UsbApplier = Arc::new(|_| Ok(()));
        let failing: UsbApplier = Arc::new(|_| Err("helper failed".into()));
        assert_eq!(apply_usb(None, &failing), status(true, "Applied"));
        assert_eq!(
            apply_usb(Some(UsbChange::Gadget), &ok),
            status(true, "USB ready")
        );
        assert_eq!(
            apply_usb(Some(UsbChange::Role(UsbDataRole::Host)), &failing),
            status(false, APPLY_FAILED_MESSAGE)
        );
    }
}
