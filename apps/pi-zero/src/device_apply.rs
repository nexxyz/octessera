//! Live device IO apply on both boards. The platform worker saves the System
//! file the USB gadget helpers read, then asks the board to rebuild the gadget
//! or switch the USB role. A failed live step keeps the saved System, so the
//! next boot applies it.

use playback_runtime::{RuntimeStoreResult, UsbDataRole};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) const APPLY_FAILED_MESSAGE: &str = "USB apply failed, reboot to apply";

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
        match ensure_storage_inactive(change, &self.storage_state).and_then(|()| save_system()) {
            Ok(()) => apply_usb(change, &self.usb_applier),
            Err(message) => {
                eprintln!("device apply save failed: {message}");
                status(false, &message)
            }
        }
    }
}

fn ensure_storage_inactive(change: Option<UsbChange>, storage_state: &Path) -> Result<(), String> {
    if change != Some(UsbChange::Role(UsbDataRole::Host)) {
        return Ok(());
    }
    match std::fs::symlink_metadata(storage_state) {
        Ok(_) => Err("Stop SD2 transfer first".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
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
        UsbChange::Gadget => run_helper("/usr/local/sbin/octessera-usb-gadget", "setup"),
        UsbChange::Role(role) => run_helper("/usr/local/sbin/octessera-usb-role", role.as_str()),
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
fn run_helper(helper: &str, argument: &str) -> Result<(), String> {
    let output = std::process::Command::new("sudo")
        .args(["-n", helper, argument])
        .output()
        .map_err(|error| format!("{helper} failed to launch: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    Err(if detail.trim().is_empty() {
        format!("{helper} exited with {}", output.status)
    } else {
        detail.trim().to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_role_is_refused_while_sd2_transfer_is_active() {
        let root = crate::test_temp_dir::unique_temp_path("octessera-device-apply");
        std::fs::create_dir_all(&root).unwrap();
        let state = root.join("storage.state");
        let host = Some(UsbChange::Role(UsbDataRole::Host));
        assert_eq!(ensure_storage_inactive(host, &state), Ok(()));
        std::fs::write(&state, b"active").unwrap();
        assert_eq!(
            ensure_storage_inactive(host, &state),
            Err("Stop SD2 transfer first".into())
        );
        for change in [
            None,
            Some(UsbChange::Gadget),
            Some(UsbChange::Role(UsbDataRole::Gadget)),
        ] {
            assert_eq!(ensure_storage_inactive(change, &state), Ok(()));
        }
        let _ = std::fs::remove_dir_all(root);
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
