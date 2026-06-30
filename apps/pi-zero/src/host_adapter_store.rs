use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::RuntimeStoreResult;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl PiPlaybackHostAdapter {
    pub(super) fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        std::fs::create_dir_all(&self.store_dir).map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        std::fs::write(self.store_dir.join("default.json"), content).map_err(|e| e.to_string())
    }

    pub(super) fn list_presets(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        if !self.store_dir.is_dir() {
            return Ok(names);
        }
        for entry in std::fs::read_dir(&self.store_dir).map_err(|e| e.to_string())? {
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
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub(super) fn preset_path(&self, name: &str) -> PathBuf {
        let safe = name.replace(['/', '\\'], "_");
        self.store_dir.join(format!("{safe}.json"))
    }

    pub(super) fn load_preset_payload(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        load_json(&self.preset_path(name))
    }

    pub(super) fn save_preset_payload(
        &self,
        name: &str,
        payload: &serde_json::Value,
    ) -> Result<bool, String> {
        let path = self.preset_path(name);
        let existed = path.is_file();
        save_json(&path, payload)?;
        Ok(existed)
    }

    pub(super) fn delete_preset_payload(&self, name: &str) -> bool {
        let path = self.preset_path(name);
        path.is_file() && std::fs::remove_file(path).is_ok()
    }

    pub(super) fn load_default_result(&mut self) -> Result<RuntimeStoreResult, String> {
        self.pending_default_save = None;
        Ok(RuntimeStoreResult::LoadDefaultResult {
            payload: load_json(&self.store_dir.join("default.json"))?,
        })
    }

    pub(super) fn save_default_result(
        &mut self,
        payload: &serde_json::Value,
        mode: Option<&str>,
    ) -> Result<Option<RuntimeStoreResult>, String> {
        if mode == Some("deferred") {
            self.pending_default_save = Some((
                payload.clone(),
                Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS),
            ));
            return Ok(None);
        }
        self.pending_default_save = None;
        self.save_default_payload(payload)?;
        Ok(Some(RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: None,
        }))
    }
}

fn load_json(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&content).ok())
}

fn save_json(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}
