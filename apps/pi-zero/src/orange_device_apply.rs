use crate::orange_host_adapter::OrangeHostAdapter;
use crate::persistence::atomic_write_bytes;
use playback_runtime::HostAdapter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

#[cfg(test)]
pub(crate) use crate::orange_reboot::OrangePowerRequestOutcome;

pub(crate) const TRANSACTION_FILE_NAME: &str = "orange-device-config-reboot.transaction";
const DEFAULT_FILE_NAME: &str = "default.json";
const MAX_TRANSACTION_BYTES: u64 = 8 * 1024 * 1024;
const MAX_DEFAULT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub(crate) enum OrangeRunError {
    Ordinary(String),
    SpecialExit78(String),
}

impl OrangeRunError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::Ordinary(_) => 1,
            Self::SpecialExit78(_) => 78,
        }
    }
}

impl std::fmt::Display for OrangeRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ordinary(message) | Self::SpecialExit78(message) => formatter.write_str(message),
        }
    }
}

impl From<String> for OrangeRunError {
    fn from(message: String) -> Self {
        Self::Ordinary(message)
    }
}

#[derive(Debug)]
pub(crate) struct OrangeDeviceApplyTransaction {
    store_dir: PathBuf,
    boot_id: String,
    prior_default_bytes: Option<Vec<u8>>,
    store_lock: Option<Arc<Mutex<()>>>,
}

#[derive(Debug)]
pub(crate) enum OrangeShutdownRequest {
    Reboot,
    Shutdown,
    ApplyDeviceConfig(OrangeDeviceApplyTransaction),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OrangeApplyRecord {
    schema: u8,
    boot_id: String,
    prior_default_bytes: Option<Vec<u8>>,
}

pub(crate) trait OrangeApplyHost {
    fn panic_external_midi(&mut self) -> Result<(), String>;
    fn silence_internal_audio(&mut self) -> Result<(), String>;
}

impl OrangeApplyHost for OrangeHostAdapter {
    fn panic_external_midi(&mut self) -> Result<(), String> {
        HostAdapter::panic_external_midi(self).map_err(|error| error.to_string())
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        HostAdapter::silence_internal_audio(self).map_err(|error| error.to_string())
    }
}

pub(crate) fn prepare(
    store_dir: &Path,
    payload: &Value,
    store_lock: Arc<Mutex<()>>,
) -> Result<OrangeDeviceApplyTransaction, String> {
    let boot_id = current_boot_id()?;
    let mut transaction = prepare_at(store_dir, payload, &boot_id)?;
    transaction.store_lock = Some(store_lock);
    Ok(transaction)
}

fn prepare_at(
    store_dir: &Path,
    payload: &Value,
    boot_id: &str,
) -> Result<OrangeDeviceApplyTransaction, String> {
    validate_boot_id(boot_id)?;
    let default_path = default_path(store_dir);
    let prior_default_bytes = read_default_bytes(&default_path)?;
    if prior_default_bytes
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_DEFAULT_BYTES)
    {
        return Err("Orange default configuration is too large to transact".into());
    }
    let new_default_bytes = serde_json::to_vec_pretty(payload)
        .map_err(|error| format!("Orange device configuration cannot be serialized: {error}"))?;
    if new_default_bytes.len() > MAX_DEFAULT_BYTES {
        return Err("Orange device configuration is too large to transact".into());
    }
    let record = OrangeApplyRecord {
        schema: 1,
        boot_id: boot_id.into(),
        prior_default_bytes: prior_default_bytes.clone(),
    };
    write_record(store_dir, &record)?;
    if let Err(error) = atomic_write_bytes(&default_path, &new_default_bytes, 0o644) {
        let cleanup = restore_record(store_dir, &record);
        return match cleanup {
            Ok(()) => Err(format!("Orange device configuration write failed: {error}")),
            Err(cleanup_error) => Err(format!(
                "Orange device configuration write failed: {error}; rollback failed: {cleanup_error}"
            )),
        };
    }
    Ok(OrangeDeviceApplyTransaction {
        store_dir: store_dir.to_path_buf(),
        boot_id: boot_id.into(),
        prior_default_bytes,
        store_lock: None,
    })
}

pub(crate) fn recover_startup(store_dir: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(store_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Orange state root cannot be inspected: {error}")),
    };
    if !metadata.is_dir() {
        return Err("Orange state root is not a directory".into());
    }
    recover_startup_at(store_dir, &current_boot_id()?)
}

fn recover_startup_at(store_dir: &Path, boot_id: &str) -> Result<(), String> {
    validate_boot_id(boot_id)?;
    let path = transaction_path(store_dir);
    let Some(record) = read_record(&path)? else {
        return Ok(());
    };
    if record.boot_id == boot_id {
        restore_record(store_dir, &record)?;
    }
    remove_transaction(&path)
}

#[path = "orange_shutdown.rs"]
mod shutdown;
#[cfg(test)]
pub(crate) use shutdown::resolve_shutdown_request_with_reboot_request;
pub(crate) use shutdown::{
    abort_shutdown_request, finish_shutdown_resolution, resolve_shutdown_request,
    OrangeShutdownResolution,
};

impl OrangeDeviceApplyTransaction {
    pub(crate) fn rollback(self) -> Result<(), String> {
        if let Some(store_lock) = self.store_lock.clone() {
            let _guard = store_lock
                .lock()
                .map_err(|_| "pi store is unavailable".to_string())?;
            return self.rollback_unlocked();
        }
        self.rollback_unlocked()
    }

    fn rollback_unlocked(self) -> Result<(), String> {
        let record = OrangeApplyRecord {
            schema: 1,
            boot_id: self.boot_id,
            prior_default_bytes: self.prior_default_bytes,
        };
        restore_record(&self.store_dir, &record)?;
        remove_transaction(&transaction_path(&self.store_dir))
    }
}

fn write_record(store_dir: &Path, record: &OrangeApplyRecord) -> Result<(), String> {
    let content = serde_json::to_vec(record).map_err(|error| {
        format!("Orange device apply transaction cannot be serialized: {error}")
    })?;
    atomic_write_bytes(&transaction_path(store_dir), &content, 0o644)
}

fn read_record(path: &Path) -> Result<Option<OrangeApplyRecord>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Orange device apply transaction cannot be inspected: {error}"
            ))
        }
    };
    if !metadata.is_file() {
        return Err("Orange device apply transaction is not a regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o644 {
            return Err("Orange device apply transaction has unsafe permissions".into());
        }
    }
    if metadata.len() > MAX_TRANSACTION_BYTES {
        return Err("Orange device apply transaction is too large".into());
    }
    let content = fs::read(path)
        .map_err(|error| format!("Orange device apply transaction cannot be read: {error}"))?;
    let record: OrangeApplyRecord = serde_json::from_slice(&content)
        .map_err(|error| format!("Orange device apply transaction is malformed: {error}"))?;
    if record.schema != 1 {
        return Err("Orange device apply transaction schema is unsupported".into());
    }
    validate_boot_id(&record.boot_id)?;
    if record
        .prior_default_bytes
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_DEFAULT_BYTES)
    {
        return Err("Orange device apply transaction contains an oversized default".into());
    }
    Ok(Some(record))
}

fn restore_record(store_dir: &Path, record: &OrangeApplyRecord) -> Result<(), String> {
    let path = default_path(store_dir);
    match &record.prior_default_bytes {
        Some(bytes) => atomic_write_bytes(&path, bytes, 0o644),
        None => remove_default(&path),
    }
}

fn read_default_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Orange default configuration cannot be inspected: {error}"
            ))
        }
    };
    if !metadata.is_file() {
        return Err("Orange default configuration is not a regular file".into());
    }
    if metadata.len() > MAX_DEFAULT_BYTES as u64 {
        return Err("Orange default configuration is too large to transact".into());
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| format!("Orange default configuration cannot be read: {error}"))
}

fn remove_default(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(path).map_err(|error| error.to_string())?;
            sync_parent(path)
        }
        Ok(_) => Err("Orange default configuration is not a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => sync_parent(path),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_transaction(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(path).map_err(|error| error.to_string())?;
            sync_parent(path)
        }
        Ok(_) => Err("Orange device apply transaction is not a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn transaction_path(store_dir: &Path) -> PathBuf {
    store_dir.join(TRANSACTION_FILE_NAME)
}

fn default_path(store_dir: &Path) -> PathBuf {
    store_dir.join(DEFAULT_FILE_NAME)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Orange state path has no parent directory".to_string())?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("Orange state directory cannot be synced: {error}"))
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn current_boot_id() -> Result<String, String> {
    #[cfg(unix)]
    {
        let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|error| format!("Orange kernel boot ID cannot be read: {error}"))?;
        let boot_id = boot_id.trim().to_owned();
        validate_boot_id(&boot_id)?;
        Ok(boot_id)
    }
    #[cfg(all(not(unix), test))]
    {
        Ok("01234567-89ab-cdef-0123-456789abcdef".into())
    }
    #[cfg(all(not(unix), not(test)))]
    {
        Err("Orange device apply requires a Unix kernel boot ID".into())
    }
}

fn validate_boot_id(boot_id: &str) -> Result<(), String> {
    let valid = boot_id.len() == 36
        && boot_id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
            }
        });
    if valid {
        Ok(())
    } else {
        Err("Orange device apply boot ID is malformed".into())
    }
}

#[cfg(test)]
#[path = "orange_device_apply_tests.rs"]
mod tests;
