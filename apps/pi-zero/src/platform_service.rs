#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::device_update;
use crate::persistence::atomic_write_json;
use crate::sample_browser::sample_entries;
#[cfg(all(test, any(unix, windows)))]
use crate::setup_portal::SetupPortalEnvironment;
use crate::setup_portal::SetupPortalService;
use crate::setup_portal_worker;
use playback_runtime::{
    HostMessage, RuntimePlatformRequest, RuntimeStoreResult, RuntimeSystemInfoError,
};
use std::path::{Path, PathBuf};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
#[path = "platform_service_setup_portal.rs"]
mod platform_service_setup_portal;
#[cfg(test)]
#[path = "platform_service_test_support.rs"]
mod platform_service_test_support;
#[path = "platform_service_worker.rs"]
mod platform_service_worker;
#[path = "system_info.rs"]
mod system_info;
const JOB_QUEUE_CAPACITY: usize = 32;
const RESULT_QUEUE_CAPACITY: usize = 32;

pub struct PiPlatformService {
    store_dir: PathBuf,
    jobs: SyncSender<PlatformJob>,
    results: Receiver<HostMessage>,
    setup_portal: SetupPortalService,
    setup_portal_stop: Arc<AtomicBool>,
}

impl PiPlatformService {
    pub fn new(store_dir: PathBuf, samples_dir: PathBuf) -> Self {
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        {
            Self::new_with_executor(store_dir, samples_dir, device_update::production_executor())
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        Self::new_with_executor(store_dir, samples_dir)
    }

    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    pub(crate) fn new_with_update_executor(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        executor: Arc<dyn device_update::UpdateExecutor>,
    ) -> Self {
        Self::new_with_executor(store_dir, samples_dir, executor)
    }

    fn new_with_executor(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))] update_executor: Arc<
            dyn device_update::UpdateExecutor,
        >,
    ) -> Self {
        let setup_portal = SetupPortalService::production();
        Self::new_with_setup_portal(
            store_dir,
            samples_dir,
            setup_portal,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            update_executor,
        )
    }

    fn new_with_setup_portal(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        setup_portal: SetupPortalService,
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))] update_executor: Arc<
            dyn device_update::UpdateExecutor,
        >,
    ) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::sync_channel(JOB_QUEUE_CAPACITY);
        let (results_tx, results_rx) = mpsc::sync_channel(RESULT_QUEUE_CAPACITY);
        let worker_store_dir = store_dir.clone();
        let setup_portal_stop = Arc::new(AtomicBool::new(false));
        setup_portal_worker::spawn(
            results_tx.clone(),
            setup_portal.clone(),
            setup_portal_stop.clone(),
        );
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        platform_service_worker::spawn(
            worker_store_dir,
            samples_dir,
            jobs_rx,
            results_tx,
            update_executor,
        );
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        platform_service_worker::spawn(worker_store_dir, samples_dir, jobs_rx, results_tx);
        Self {
            store_dir,
            jobs: jobs_tx,
            results: results_rx,
            setup_portal,
            setup_portal_stop,
        }
    }

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn new_with_setup_environment(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        environment: SetupPortalEnvironment,
    ) -> Self {
        Self::new_with_setup_portal(
            store_dir,
            samples_dir,
            SetupPortalService::test(environment),
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            device_update::production_executor(),
        )
    }

    pub fn save_recovery_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        save_json(&self.store_dir.join("recovery-save.json"), payload)
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub fn save_default_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        save_json(&self.store_dir.join("default.json"), payload)
    }

    pub fn enqueue(&self, job: PlatformJob) -> Result<(), String> {
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => "pi platform service queue is full".to_string(),
            TrySendError::Disconnected(_) => "pi platform service stopped".to_string(),
        })
    }
}

impl Drop for PiPlatformService {
    fn drop(&mut self) {
        self.setup_portal_stop.store(true, Ordering::Release);
    }
}

pub struct PlatformJob {
    pub request: RuntimePlatformRequest,
    pub kind: PlatformJobKind,
}

pub enum PlatformJobKind {
    ListPresets,
    LoadPreset {
        name: String,
    },
    SavePreset {
        name: String,
        payload: serde_json::Value,
    },
    DeletePreset {
        name: String,
    },
    SaveDefault {
        payload: serde_json::Value,
        is_auto: Option<bool>,
    },
    SaveBackup {
        payload: serde_json::Value,
    },
    ListSamples {
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    },
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    UsbSdTransferStart,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    UsbSdTransferStop,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    UpdateCheck,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    UpdateApply,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    Rollback,
    SystemInfo,
    #[cfg(test)]
    TestBarrier {
        completed: SyncSender<()>,
    },
    #[cfg(test)]
    TestGate {
        entered: SyncSender<()>,
        release: Receiver<()>,
    },
}

impl PlatformJob {
    pub fn new(request: RuntimePlatformRequest, kind: PlatformJobKind) -> Self {
        Self { request, kind }
    }
}

fn handle_job(
    store_dir: &Path,
    samples_dir: &Path,
    job: PlatformJob,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        PlatformJobKind::UpdateCheck => device_update::run("check", update_executor),
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        PlatformJobKind::UpdateApply => device_update::run("apply", update_executor),
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
fn usb_storage_message(action: &str, stdout: &str) -> String {
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

fn save_backup(store_dir: &Path, payload: &serde_json::Value) -> Result<(), String> {
    let dir = store_dir.join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    save_json(&dir.join(format!("bak-{millis}.json")), payload)?;
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("bak-") && name.ends_with(".json") {
            paths.push(path);
        }
    }
    paths.sort();
    for path in paths.iter().take(paths.len().saturating_sub(20)) {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn store_error(message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::StoreError { message }
}

pub fn list_presets(store_dir: &Path) -> Result<Vec<String>, String> {
    let mut names = std::collections::BTreeSet::new();
    if !store_dir.is_dir() {
        return Ok(Vec::new());
    }
    for entry in std::fs::read_dir(store_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_file() {
            continue;
        }
        if let Some(name) = entry
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(preset_name_from_file_name)
        {
            names.insert(name);
        }
    }
    let patch_dir = store_dir.join("patches");
    if patch_dir.is_dir() {
        for entry in std::fs::read_dir(&patch_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.path().is_file() {
                continue;
            }
            if let Some(name) = entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(preset_name_from_file_name)
            {
                names.insert(name);
            }
        }
    }
    Ok(names.into_iter().collect())
}

pub fn preset_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !playback_runtime::is_valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(store_dir.join(format!("{name}.json")))
}

pub fn preset_patch_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !playback_runtime::is_valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(store_dir.join("patches").join(format!("{name}.json")))
}

pub fn preset_load_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let patch = preset_patch_path(store_dir, name)?;
    if patch.is_file() {
        return Ok(patch);
    }
    preset_path(store_dir, name)
}

fn preset_name_from_file_name(file_name: &str) -> Option<String> {
    if matches!(
        file_name,
        "default.json" | "default.patch.json" | "device.json" | "recovery-save.json"
    ) || file_name.starts_with("bak-")
    {
        return None;
    }
    let name = file_name.strip_suffix(".json")?;
    playback_runtime::is_valid_preset_name(name).then(|| name.to_string())
}

pub fn load_json(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn save_json(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
    atomic_write_json(path, payload)
}

fn delete_preset_payload(store_dir: &Path, name: &str) -> bool {
    let Ok(legacy) = preset_path(store_dir, name) else {
        return false;
    };
    let Ok(patch) = preset_patch_path(store_dir, name) else {
        return false;
    };
    let mut removed = false;
    for path in [legacy, patch] {
        if path.is_file() && std::fs::remove_file(path).is_ok() {
            removed = true;
        }
    }
    removed
}

#[cfg(test)]
#[path = "platform_service_tests.rs"]
mod tests;
