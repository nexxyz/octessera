use crate::platform_service::{PiPlatformService, USB_STORAGE_STATE_PATH};
use playback_runtime::{RuntimePlatformRequest, RuntimeStoreResult, UsbDataRole};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const MAX_DEFAULT_BYTES: usize = 1024 * 1024;
pub(crate) fn apply(service: &PiPlatformService, payload: &Value) -> Result<(), String> {
    service.apply_device_config(payload)
}

pub(crate) fn apply_locked(store_dir: &Path, payload: &Value) -> Result<(), String> {
    crate::usb_config_validation::validate_pi_audio_outputs_payload(payload)?;
    let role = parse_role(payload)?;
    ensure_storage_inactive(role, Path::new(USB_STORAGE_STATE_PATH))?;
    let prior = read_default_bytes(store_dir)?;
    let new_default = serde_json::to_vec_pretty(payload)
        .map_err(|error| format!("device configuration cannot be serialized: {error}"))?;
    if new_default.len() > MAX_DEFAULT_BYTES {
        return Err("device configuration is too large".into());
    }
    apply_transaction(store_dir, &new_default, prior, role, apply_usb_role)
}

pub(crate) type UsbRoleApplier =
    std::sync::Arc<dyn Fn(UsbDataRole) -> Result<(), String> + Send + Sync>;

pub(crate) fn save_default_if_role_changed(
    store_dir: &Path,
    payload: &Value,
    is_auto: Option<bool>,
    storage_state: &Path,
    apply_role: &dyn Fn(UsbDataRole) -> Result<(), String>,
) -> Option<RuntimeStoreResult> {
    if let Err(message) = crate::usb_config_validation::validate_pi_audio_outputs_payload(payload) {
        return Some(store_error(message));
    }
    let new_role = match parse_role(payload) {
        Ok(role) => role,
        Err(message) => return Some(store_error(message)),
    };
    let prior = match read_default_bytes(store_dir) {
        Ok(prior) => prior,
        Err(message) => return Some(store_error(message)),
    };
    let prior_role = match prior.as_deref() {
        Some(bytes) => match serde_json::from_slice::<Value>(bytes)
            .map_err(|error| format!("default configuration cannot be parsed: {error}"))
            .and_then(|payload| parse_role(&payload))
        {
            Ok(role) => role,
            Err(message) => return Some(store_error(message)),
        },
        None => UsbDataRole::Gadget,
    };
    if let Err(message) = ensure_storage_inactive(new_role, storage_state) {
        return Some(store_error(message));
    }
    if prior_role == new_role {
        return None;
    }
    let new_default = match serde_json::to_vec_pretty(payload) {
        Ok(bytes) if bytes.len() <= MAX_DEFAULT_BYTES => bytes,
        Ok(_) => return Some(store_error("device configuration is too large".into())),
        Err(error) => {
            return Some(store_error(format!(
                "device configuration cannot be serialized: {error}"
            )))
        }
    };
    Some(
        match apply_transaction(store_dir, &new_default, prior, new_role, apply_role) {
            Ok(()) => RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto },
            Err(message) => store_error(format!("Save default failed: {message}")),
        },
    )
}

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
    new_default: &[u8],
    prior: Option<Vec<u8>>,
    role: UsbDataRole,
    apply_role: F,
) -> Result<(), String>
where
    F: FnOnce(UsbDataRole) -> Result<(), String>,
{
    save_default_bytes(store_dir, new_default)?;
    if let Err(error) = apply_role(role) {
        return match restore_default(store_dir, prior) {
            Ok(()) => Err(format!("USB data-role apply failed: {error}")),
            Err(rollback) => Err(format!(
                "USB data-role apply failed: {error}; default rollback failed: {rollback}"
            )),
        };
    }
    Ok(())
}

fn restore_default(store_dir: &Path, prior: Option<Vec<u8>>) -> Result<(), String> {
    match prior {
        Some(bytes) => save_default_bytes(store_dir, &bytes),
        None => remove_default(store_dir),
    }
}

fn parse_role(payload: &Value) -> Result<UsbDataRole, String> {
    crate::usb_config::parse_usb_runtime_config(payload)
        .map(|config| config.data_role)
        .map_err(|error| error.to_string())
}

fn read_default_bytes(store_dir: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = store_dir.join("default.json");
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "default configuration cannot be inspected: {error}"
            ))
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("default configuration is not a regular file".into());
    }
    std::fs::read(path)
        .map(Some)
        .map_err(|error| format!("default configuration cannot be read: {error}"))
}

fn save_default_bytes(store_dir: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::persistence::atomic_write_bytes(&store_dir.join("default.json"), bytes, 0o644)
}

fn remove_default(store_dir: &Path) -> Result<(), String> {
    let path = store_dir.join("default.json");
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            std::fs::remove_file(&path).map_err(|error| error.to_string())
        }
        Ok(_) => Err("default configuration is not a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn store_error(message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::StoreError { message }
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
        let root = std::env::temp_dir().join(format!(
            "octessera-rpi-device-apply-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = root.join("store");
        std::fs::create_dir_all(&store).unwrap();
        (PiPlatformService::new(store, root.join("samples")), root)
    }

    #[test]
    fn role_helper_failure_restores_prior_default_bytes() {
        let (_service, root) = service();
        let prior = br#"{"prior":true}"#.to_vec();
        save_default_bytes(&root.join("store"), &prior).unwrap();
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
            std::fs::read(root.join("store/default.json")).unwrap(),
            prior
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn successful_role_apply_leaves_new_default_and_no_power_result() {
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
            std::fs::read(root.join("store/default.json")).unwrap(),
            br#"{"new":true}"#
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
