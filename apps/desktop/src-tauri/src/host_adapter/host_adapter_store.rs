use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::persistence::{
    atomic_write_json, preset_file_path, preset_load_file_path, preset_name_from_file_name,
    preset_patch_file_path,
};
use playback_runtime::{HostMessage, RuntimePlatformRequest, RuntimeStoreResult};
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl DesktopPlaybackHostAdapter {
    pub(super) fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        atomic_write_json(&self.store_dir.join("default.json"), payload)
    }

    pub(super) fn list_preset_names(&self) -> Result<Vec<String>, String> {
        let presets_dir = self.store_dir.join("presets");
        let mut names = std::collections::BTreeSet::new();
        if presets_dir.is_dir() {
            for entry in std::fs::read_dir(&presets_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
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
        let patch_dir = presets_dir.join("patches");
        if patch_dir.is_dir() {
            for entry in std::fs::read_dir(&patch_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
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

    pub(super) fn load_preset_payload(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let path = preset_load_file_path(&self.store_dir.join("presets"), name)?;
        if !path.is_file() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&content).ok())
    }

    pub(super) fn save_preset_payload(
        &self,
        name: &str,
        payload: &serde_json::Value,
    ) -> Result<(), String> {
        let presets_dir = self.store_dir.join("presets");
        std::fs::create_dir_all(&presets_dir).map_err(|e| e.to_string())?;
        let path = preset_patch_file_path(&presets_dir, name)?;
        atomic_write_json(&path, payload)
    }

    pub(super) fn delete_preset_payload(&self, name: &str) -> Result<bool, String> {
        let presets_dir = self.store_dir.join("presets");
        let legacy = preset_file_path(&presets_dir, name)?;
        let patch = preset_patch_file_path(&presets_dir, name)?;
        let mut removed = false;
        for path in [legacy, patch] {
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
                removed = true;
            }
        }
        Ok(removed)
    }

    pub(super) fn load_default_result(&mut self) -> Result<Vec<HostMessage>, String> {
        self.pending_default_save = None;
        match self.load_default_payload()? {
            Ok(payload) => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult { payload },
            }]),
            Err(error) => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::StoreError {
                    message: format!("Default load failed: {error}"),
                },
            }]),
        }
    }

    pub(super) fn save_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
        payload: &serde_json::Value,
        mode: Option<&str>,
    ) -> Result<Vec<HostMessage>, String> {
        if mode == Some("deferred") {
            self.pending_default_save = Some((
                payload.clone(),
                Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS),
                request.clone(),
            ));
            return Ok(vec![]);
        }
        self.pending_default_save = None;
        self.save_default_payload(payload)?;
        Ok(vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            },
        }])
    }

    pub(super) fn save_backup_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        let dir = self.store_dir.join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        atomic_write_json(&dir.join(format!("bak-{millis}.json")), payload)?;
        rotate_backups(&dir)
    }

    pub(super) fn save_recovery_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        atomic_write_json(&self.store_dir.join("recovery-save.json"), payload)
    }

    fn load_default_payload(&self) -> Result<Result<Option<serde_json::Value>, String>, String> {
        let path = self.store_dir.join("default.json");
        if !path.is_file() {
            return Ok(Ok(None));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&content)
            .map(Some)
            .map_err(|e| e.to_string()))
    }
}

fn rotate_backups(dir: &std::path::Path) -> Result<(), String> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
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
