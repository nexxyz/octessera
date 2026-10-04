use crate::platform_service::PiPlatformService;
use playback_runtime::{RuntimePlatformRequest, RuntimeStoreResult, UsbDataRole};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const MAX_SYSTEM_BYTES: usize = 1024 * 1024;
pub(crate) fn apply(service: &PiPlatformService, payload: &Value) -> Result<(), String> {
    service.apply_device_config(payload)
}

pub(crate) fn apply_locked(
    store_dir: &Path,
    payload: &Value,
    storage_state: &Path,
    apply_role: &dyn Fn(UsbDataRole) -> Result<(), String>,
) -> Result<(), String> {
    crate::usb_config_validation::validate_pi_audio_outputs_payload(payload)?;
    let role = parse_role(payload)?;
    ensure_storage_inactive(role, storage_state)?;
    let patch = crate::platform_service::load_json(&crate::platform_service::default_patch_path(
        store_dir,
    ))?
    .ok_or_else(|| "Default patch is missing".to_string())?;
    playback_runtime::compose_local_system_patch_documents(payload, &patch)?;
    let prior = read_system_bytes(store_dir)?;
    let new_system = serde_json::to_vec_pretty(payload)
        .map_err(|error| format!("device configuration cannot be serialized: {error}"))?;
    if new_system.len() > MAX_SYSTEM_BYTES {
        return Err("device configuration is too large".into());
    }
    apply_transaction(store_dir, &new_system, prior, role, apply_role)
}

pub(crate) type UsbRoleApplier =
    std::sync::Arc<dyn Fn(UsbDataRole) -> Result<(), String> + Send + Sync>;

fn ensure_storage_inactive(role: UsbDataRole, storage_state: &Path) -> Result<(), String> {
    if role != UsbDataRole::Host {
        return Ok(());
    }
    match std::fs::symlink_metadata(storage_state) {
        Ok(_) => Err("USB SD2 transfer is active; host role apply is unavailable".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "USB SD2 storage state cannot be inspected: {error}"
        )),
    }
}

fn apply_transaction<F>(
    store_dir: &Path,
    new_system: &[u8],
    prior: Option<Vec<u8>>,
    role: UsbDataRole,
    apply_role: F,
) -> Result<(), String>
where
    F: FnOnce(UsbDataRole) -> Result<(), String>,
{
    save_system_bytes(store_dir, new_system)?;
    if let Err(error) = apply_role(role) {
        return match restore_system(store_dir, prior) {
            Ok(()) => Err(format!("USB data-role apply failed: {error}")),
            Err(rollback) => Err(format!(
                "USB data-role apply failed: {error}; System rollback failed: {rollback}"
            )),
        };
    }
    Ok(())
}

fn restore_system(store_dir: &Path, prior: Option<Vec<u8>>) -> Result<(), String> {
    match prior {
        Some(bytes) => save_system_bytes(store_dir, &bytes),
        None => remove_system(store_dir),
    }
}

fn parse_role(payload: &Value) -> Result<UsbDataRole, String> {
    crate::usb_config::parse_usb_runtime_config(payload)
        .map(|config| config.data_role)
        .map_err(|error| error.to_string())
}

fn read_system_bytes(store_dir: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = store_dir.join("system.json");
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("System settings cannot be inspected: {error}")),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("System settings are not a regular file".into());
    }
    std::fs::read(path)
        .map(Some)
        .map_err(|error| format!("System settings cannot be read: {error}"))
}

fn save_system_bytes(store_dir: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::persistence::atomic_write_bytes(&store_dir.join("system.json"), bytes, 0o644)
}

fn remove_system(store_dir: &Path) -> Result<(), String> {
    let path = store_dir.join("system.json");
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            std::fs::remove_file(&path).map_err(|error| error.to_string())
        }
        Ok(_) => Err("System settings are not a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn apply_usb_role(role: UsbDataRole) -> Result<(), String> {
    let output = Command::new("sudo")
        .args(["-n", "/usr/local/sbin/octessera-usb-role", role.as_str()])
        .output()
        .map_err(|error| format!("USB data-role helper failed to launch: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        Err(if detail.trim().is_empty() {
            format!("USB data-role helper exited with {}", output.status)
        } else {
            detail.trim().to_string()
        })
    }
}

pub(crate) fn unavailable(request: &RuntimePlatformRequest) -> playback_runtime::HostMessage {
    playback_runtime::HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: playback_runtime::RuntimeErrorFacts::new(
                playback_runtime::RuntimeErrorDomain::Runtime,
                playback_runtime::RuntimeErrorCode::Unavailable,
                playback_runtime::RuntimeOperation::RuntimeDispatch,
                Some("USB SD2 transfer is unavailable while USB data role is host".into()),
            )
            .with_identity(Some(request.request_id.clone()), request.revision),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn service() -> (PiPlatformService, PathBuf) {
        let root = crate::test_temp_dir::unique_temp_path("octessera-rpi-device-apply");
        let store = root.join("store");
        std::fs::create_dir_all(&store).unwrap();
        (PiPlatformService::new(store, root.join("samples")), root)
    }

    #[test]
    fn role_helper_failure_restores_prior_system_bytes() {
        let (_service, root) = service();
        let prior = br#"{"prior":true}"#.to_vec();
        save_system_bytes(&root.join("store"), &prior).unwrap();
        let error = apply_transaction(
            &root.join("store"),
            br#"{"new":true}"#,
            Some(prior.clone()),
            playback_runtime::UsbDataRole::Host,
            |_| Err("helper failed".into()),
        )
        .unwrap_err();
        assert!(error.contains("helper failed"));
        assert_eq!(
            std::fs::read(root.join("store/system.json")).unwrap(),
            prior
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn successful_role_apply_leaves_new_system_and_no_power_result() {
        let (_service, root) = service();
        apply_transaction(
            &root.join("store"),
            br#"{"new":true}"#,
            None,
            playback_runtime::UsbDataRole::Gadget,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(root.join("store/system.json")).unwrap(),
            br#"{"new":true}"#
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn device_apply_accepts_local_samples_in_assigned_and_unassigned_slots() {
        let (_service, root) = service();
        let store = root.join("store");
        let mut full: Value =
            serde_json::from_str(include_str!("../../../config/generated/pi/default.json"))
                .unwrap();
        full["runtimeConfig"]["instruments"][0]["type"] = serde_json::json!("sampler");
        full["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
            serde_json::json!("userdata/User Kit/custom.wav");
        full["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"] =
            serde_json::json!("sd-card/octessera/samples/kick.wav");
        full["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = serde_json::json!([
            { "level": null, "sampleSlot": 0, "x": 0, "y": 0 }
        ]);
        let documents = playback_runtime::split_local_system_patch_documents(&full).unwrap();
        std::fs::write(
            crate::platform_service::default_patch_path(&store),
            serde_json::to_vec(&documents.patch).unwrap(),
        )
        .unwrap();
        std::fs::write(
            store.join("system.json"),
            serde_json::to_vec(&documents.system).unwrap(),
        )
        .unwrap();

        apply_locked(
            &store,
            &documents.system,
            &root.join("storage.state"),
            &|_| Ok(()),
        )
        .unwrap();
        let patch = crate::platform_service::load_json(
            &crate::platform_service::default_patch_path(&store),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
            "userdata/User Kit/custom.wav"
        );
        assert_eq!(
            patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"],
            "sd-card/octessera/samples/kick.wav"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
