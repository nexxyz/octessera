use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{HostMessage, RuntimeStoreResult};
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl DesktopPlaybackHostAdapter {
    pub(super) fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        std::fs::write(self.store_dir.join("default.json"), content).map_err(|e| e.to_string())
    }

    pub(super) fn list_preset_names(&self) -> Result<Vec<String>, String> {
        let presets_dir = self.store_dir.join("presets");
        let mut names: Vec<String> = Vec::new();
        if presets_dir.is_dir() {
            for entry in std::fs::read_dir(&presets_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub(super) fn load_preset_payload(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let path = self.store_dir.join("presets").join(format!("{name}.json"));
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
        let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        std::fs::write(presets_dir.join(format!("{name}.json")), content).map_err(|e| e.to_string())
    }

    pub(super) fn delete_preset_payload(&self, name: &str) -> bool {
        let path = self.store_dir.join("presets").join(format!("{name}.json"));
        path.is_file() && std::fs::remove_file(&path).is_ok()
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
        payload: &serde_json::Value,
        mode: Option<&str>,
    ) -> Result<Vec<HostMessage>, String> {
        if mode == Some("deferred") {
            self.pending_default_save = Some((
                payload.clone(),
                Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS),
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
