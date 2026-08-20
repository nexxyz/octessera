use super::platform_service_store::{
    delete_preset_payload, list_presets, load_json, preset_load_path, preset_patch_path,
    save_backup, save_json,
};
use crate::device_update;
use crate::sample_browser::sample_entries;
use playback_runtime::{RuntimeStoreResult, RuntimeSystemInfoError};
use std::path::Path;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::process::Command;

use super::{system_info, PlatformJob, PlatformJobKind};

pub(super) fn handle_job(
    store_dir: &Path,
    samples_dir: &Path,
    job: PlatformJob,
    update_executor: &dyn device_update::UpdateExecutor,
) -> RuntimeStoreResult {
    let request = job.request;
    let result = match job.kind {
        PlatformJobKind::ListPresets => match list_presets(store_dir) {
            Ok(names) => RuntimeStoreResult::ListPresetsResult { names },
            Err(message) => store_error(format!("Preset list failed: {message}")),
        },
        PlatformJobKind::LoadPreset { name } => {
            match preset_load_path(store_dir, &name).and_then(|path| load_json(&path)) {
                Ok(payload) => RuntimeStoreResult::LoadPresetResult { payload, name },
                Err(message) => store_error(format!("Load {name} failed: {message}")),
            }
        }
        PlatformJobKind::SavePreset { name, payload } => {
            match preset_patch_path(store_dir, &name) {
                Ok(path) => {
                    let existed = path.is_file();
                    match save_json(&path, &payload) {
                        Ok(()) => RuntimeStoreResult::SavePresetResult {
                            name,
                            outcome: if existed { "overwritten" } else { "created" }.into(),
                        },
                        Err(message) => store_error(format!("Save {name} failed: {message}")),
                    }
                }
                Err(message) => store_error(format!("Save {name} failed: {message}")),
            }
        }
        PlatformJobKind::DeletePreset { name } => RuntimeStoreResult::DeletePresetResult {
            ok: delete_preset_payload(store_dir, &name),
            name,
        },
        PlatformJobKind::SaveDefault { payload, is_auto } => {
            match save_json(&store_dir.join("default.json"), &payload) {
                Ok(()) => RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto },
                Err(message) => store_error(format!("Save default failed: {message}")),
            }
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        PlatformJobKind::PrepareOrangeDeviceApply { .. } => {
            unreachable!("Orange device apply jobs are completed by the platform worker")
        }
        PlatformJobKind::SaveBackup { payload } => match save_backup(store_dir, &payload) {
            Ok(()) => RuntimeStoreResult::SaveBackupResult { ok: true },
            Err(message) => store_error(format!("Save backup failed: {message}")),
        },
        PlatformJobKind::ListSamples {
            instrument_slot,
            sample_slot,
            dir,
        } => match sample_entries(samples_dir, &dir) {
            Ok(entries) => RuntimeStoreResult::SampleListResult {
                instrument_slot,
                sample_slot,
                dir,
                entries,
            },
            Err(message) => RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                message,
            },
        },
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        PlatformJobKind::UsbSdTransferStart => run_usb_storage_command("storage-start"),
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        PlatformJobKind::UsbSdTransferStop => run_usb_storage_command("storage-stop"),
        PlatformJobKind::UpdateCheck => device_update::run("check", update_executor),
        PlatformJobKind::UpdateApply => device_update::run("apply", update_executor),
        PlatformJobKind::Rollback => device_update::run("rollback", update_executor),
        PlatformJobKind::SystemInfo => match system_info::collect() {
            Ok(info) => RuntimeStoreResult::SystemInfoResult {
                info: info.sanitized(),
            },
            Err(message) => RuntimeStoreResult::SystemInfoError {
                error: RuntimeSystemInfoError::unavailable(message),
            },
        },
        #[cfg(test)]
        PlatformJobKind::TestBarrier { .. } | PlatformJobKind::TestGate { .. } => {
            unreachable!("test synchronization job is handled by worker")
        }
    };
    let result = match result {
        RuntimeStoreResult::StoreError { message } => RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
        result => result,
    };
    result.with_identity(request.request_id, request.revision)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn run_usb_storage_command(action: &str) -> RuntimeStoreResult {
    match Command::new("sudo")
        .args(["-n", "/usr/local/sbin/octessera-usb-gadget", action])
        .output()
    {
        Ok(output) if output.status.success() => RuntimeStoreResult::UsbSdTransferStatus {
            active: action == "storage-start",
            message: usb_storage_message(action, &String::from_utf8_lossy(&output.stdout)),
        },
        Ok(output) => store_error(format!(
            "USB SD2 transfer {action} failed: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        )),
        Err(error) => store_error(format!("USB SD2 transfer {action} failed: {error}")),
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn usb_storage_message(action: &str, stdout: &str) -> String {
    if action != "storage-start" {
        return "USB SD2 transfer stopped".into();
    }
    if stdout
        .lines()
        .any(|line| line.trim() == "HOST_STATE=configured")
    {
        "USB SD2 transfer active".into()
    } else {
        "USB SD2 transfer waiting for host".into()
    }
}

fn store_error(message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::StoreError { message }
}
