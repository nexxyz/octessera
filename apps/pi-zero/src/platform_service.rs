use crate::persistence::atomic_write_json;
use crate::sample_browser::sample_entries;
use playback_runtime::{HostMessage, RuntimeStoreResult};
use std::path::{Path, PathBuf};
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

pub enum PlatformJob {
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
    match job {
        PlatformJob::ListPresets => match list_presets(store_dir) {
            Ok(names) => RuntimeStoreResult::ListPresetsResult { names },
            Err(message) => store_error(format!("Preset list failed: {message}")),
        },
        PlatformJob::LoadPreset { name } => {
            match preset_path(store_dir, &name).and_then(|path| load_json(&path)) {
                Ok(payload) => RuntimeStoreResult::LoadPresetResult { payload, name },
                Err(message) => store_error(format!("Load {name} failed: {message}")),
            }
        }
        PlatformJob::SavePreset { name, payload } => match preset_path(store_dir, &name) {
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
        },
        PlatformJob::DeletePreset { name } => RuntimeStoreResult::DeletePresetResult {
            ok: delete_preset_payload(store_dir, &name),
            name,
        },
        PlatformJob::SaveDefault { payload, is_auto } => {
            match save_json(&store_dir.join("default.json"), &payload) {
                Ok(()) => RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto },
                Err(message) => store_error(format!("Save default failed: {message}")),
            }
        }
        PlatformJob::SaveBackup { payload } => match save_backup(store_dir, &payload) {
            Ok(()) => RuntimeStoreResult::SaveBackupResult { ok: true },
            Err(message) => store_error(format!("Save backup failed: {message}")),
        },
        PlatformJob::ListSamples {
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
    let mut names = Vec::new();
    if !store_dir.is_dir() {
        return Ok(names);
    }
    for entry in std::fs::read_dir(store_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .path()
            .file_name()
            .is_some_and(|name| name == "default.json")
        {
            continue;
        }
        if entry.path().extension().is_some_and(|ext| ext == "json") {
            if let Some(stem) = entry.path().file_stem().and_then(|stem| stem.to_str()) {
                if playback_runtime::is_valid_preset_name(stem) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(names)
}

pub fn preset_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !playback_runtime::is_valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(store_dir.join(format!("{name}.json")))
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
    let Ok(path) = preset_path(store_dir, name) else {
        return false;
    };
    path.is_file() && std::fs::remove_file(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_presets_filters_unsafe_legacy_files() {
        let dir = std::env::temp_dir().join(format!(
            "cellsymphony-pi-preset-list-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("safe.json"), "{}").unwrap();
        std::fs::write(dir.join("bad:name.json"), "{}").unwrap();
        std::fs::write(dir.join("CON.json"), "{}").unwrap();

        assert_eq!(list_presets(&dir).unwrap(), vec!["safe".to_string()]);

        let _ = std::fs::remove_dir_all(dir);
    }
}
