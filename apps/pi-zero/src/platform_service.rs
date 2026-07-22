use crate::persistence::atomic_write_json;
use crate::sample_browser::sample_entries;
use playback_runtime::{HostMessage, RuntimeOperation, RuntimePlatformRequest, RuntimeStoreResult};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;

const JOB_QUEUE_CAPACITY: usize = 32;
const RESULT_QUEUE_CAPACITY: usize = 32;

pub struct PiPlatformService {
    store_dir: PathBuf,
    jobs: SyncSender<PlatformJob>,
    results: Receiver<HostMessage>,
}

impl PiPlatformService {
    pub fn new(store_dir: PathBuf, samples_dir: PathBuf) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::sync_channel(JOB_QUEUE_CAPACITY);
        let (results_tx, results_rx) = mpsc::sync_channel(RESULT_QUEUE_CAPACITY);
        let worker_store_dir = store_dir.clone();
        thread::spawn(move || run_worker(worker_store_dir, samples_dir, jobs_rx, results_tx));
        Self {
            store_dir,
            jobs: jobs_tx,
            results: results_rx,
        }
    }

    pub fn save_recovery_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        save_json(&self.store_dir.join("recovery-save.json"), payload)
    }

    pub fn save_default_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        save_json(&self.store_dir.join("default.json"), payload)
    }

    pub fn enqueue(&self, job: PlatformJob) -> Result<(), String> {
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => "pi platform service queue is full".to_string(),
            TrySendError::Disconnected(_) => "pi platform service stopped".to_string(),
        })
    }

    pub fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = Vec::new();
        for _ in 0..max_results {
            match self.results.try_recv() {
                Ok(result) => results.push(result),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        results
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
    UsbSdTransferStart,
    UsbSdTransferStop,
    UpdateCheck,
    UpdateApply,
    Rollback,
}

impl PlatformJob {
    pub fn new(request: RuntimePlatformRequest, kind: PlatformJobKind) -> Self {
        Self { request, kind }
    }
}

fn run_worker(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: SyncSender<HostMessage>,
) {
    while let Ok(job) = jobs.recv() {
        let result = handle_job(&store_dir, &samples_dir, job);
        if results.send(HostMessage::RuntimeResult { result }).is_err() {
            break;
        }
    }
}

fn handle_job(store_dir: &Path, samples_dir: &Path, job: PlatformJob) -> RuntimeStoreResult {
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
        PlatformJobKind::UsbSdTransferStart => run_usb_storage_command("storage-start"),
        PlatformJobKind::UsbSdTransferStop => run_usb_storage_command("storage-stop"),
        PlatformJobKind::UpdateCheck => run_update_command("check"),
        PlatformJobKind::UpdateApply => run_update_command("apply"),
        PlatformJobKind::Rollback => run_update_command("rollback"),
    };
    let result = match result {
        RuntimeStoreResult::StoreError { message } => RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
        result => result,
    };
    result.with_identity(request.request_id, request.revision)
}

fn run_update_command(action: &str) -> RuntimeStoreResult {
    match Command::new("sudo")
        .args(["-n", "/usr/local/sbin/octessera-update", action])
        .output()
    {
        Ok(output) if output.status.success() => RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::RuntimeDispatch,
            request_id: None,
            revision: None,
        },
        Ok(output) => store_error(format!(
            "Update {action} failed{}{}",
            command_output_suffix(&output.stderr),
            command_output_suffix(&output.stdout)
        )),
        Err(error) => store_error(format!("Update {action} failed: {error}")),
    }
}

fn command_output_suffix(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!(": {trimmed}")
    }
}

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
mod tests {
    use super::*;

    #[test]
    fn list_presets_filters_unsafe_legacy_files() {
        let dir = std::env::temp_dir().join(format!(
            "octessera-pi-preset-list-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("safe.json"), "{}").unwrap();
        std::fs::write(dir.join("default.json"), "{}").unwrap();
        std::fs::write(dir.join("recovery-save.json"), "{}").unwrap();
        std::fs::write(dir.join("bak-123.json"), "{}").unwrap();
        std::fs::write(dir.join("bad:name.json"), "{}").unwrap();
        std::fs::write(dir.join("CON.json"), "{}").unwrap();

        assert_eq!(list_presets(&dir).unwrap(), vec!["safe".to_string()]);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn preset_patch_files_are_preferred_and_delete_removes_legacy_copy() {
        let dir = std::env::temp_dir().join(format!(
            "octessera-pi-preset-patch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Jam.json"), r#"{"legacy":true}"#).unwrap();
        std::fs::create_dir_all(dir.join("patches")).unwrap();
        std::fs::write(dir.join("patches").join("Jam.json"), r#"{"patch":true}"#).unwrap();
        std::fs::write(dir.join("Jam.patch.json"), r#"{"legacy_patch_name":true}"#).unwrap();

        assert_eq!(
            list_presets(&dir).unwrap(),
            vec!["Jam".to_string(), "Jam.patch".to_string()]
        );
        assert_eq!(
            load_json(&preset_load_path(&dir, "Jam").unwrap()).unwrap(),
            Some(serde_json::json!({ "patch": true }))
        );
        save_json(
            &preset_patch_path(&dir, "New").unwrap(),
            &serde_json::json!({ "kind": "octessera.patch" }),
        )
        .unwrap();
        assert!(dir.join("patches").join("New.json").is_file());
        assert!(!dir.join("New.json").is_file());
        assert!(delete_preset_payload(&dir, "Jam"));
        assert!(!dir.join("Jam.json").exists());
        assert!(!dir.join("patches").join("Jam.json").exists());
        assert!(dir.join("Jam.patch.json").exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn usb_storage_start_reports_waiting_until_host_configures_gadget() {
        assert_eq!(
            usb_storage_message("storage-start", "HOST_STATE=not attached\n"),
            "USB SD2 transfer waiting for host"
        );
        assert_eq!(
            usb_storage_message("storage-start", "HOST_STATE=configured\n"),
            "USB SD2 transfer active"
        );
    }
}
