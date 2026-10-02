use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::persistence::{atomic_write_json, preset_name_from_file_name, preset_patch_file_path};
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePlatformRequest, RuntimeStoreResult,
};
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl DesktopPlaybackHostAdapter {
    pub(super) fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        atomic_write_json(&self.store_dir.join("default.patch.json"), payload)
    }

    pub(super) fn load_system_result(&self, request: &RuntimePlatformRequest) -> Vec<HostMessage> {
        let result = match self.load_system_payload() {
            Ok(payload) => RuntimeStoreResult::LoadSystemResult { payload },
            Err(error) => RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Storage,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::StoreLoadSystem,
                    Some(format!("System load failed: {error}")),
                ),
            },
        };
        vec![HostMessage::RuntimeResult {
            result: result.with_identity(request.request_id.clone(), request.revision),
        }]
    }

    pub(super) fn save_system_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        atomic_write_json(&self.store_dir.join("system.json"), payload)
    }

    fn load_system_payload(&self) -> Result<Option<serde_json::Value>, String> {
        let path = self.store_dir.join("system.json");
        if !path.is_file() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        serde_json::from_str(&content)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub(super) fn list_preset_names(&self) -> Result<Vec<String>, String> {
        let presets_dir = self.store_dir.join("presets");
        let mut names = std::collections::BTreeSet::new();
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

    pub(super) fn load_preset_result(
        &self,
        request: &RuntimePlatformRequest,
        name: &str,
    ) -> Result<Vec<HostMessage>, String> {
        if self.pending_default_save.is_pending() {
            return Ok(vec![HostMessage::RuntimeResult {
                result: pending_patch_load_failure(RuntimeOperation::StoreLoadPreset)
                    .with_identity(request.request_id.clone(), request.revision),
            }]);
        }
        let payload = self.load_preset_payload(name)?;
        Ok(vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: name.to_string(),
                payload,
            }
            .with_identity(request.request_id.clone(), request.revision),
        }])
    }

    pub(super) fn load_preset_payload(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let path = preset_patch_file_path(&self.store_dir.join("presets"), name)?;
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
        let patch = preset_patch_file_path(&presets_dir, name)?;
        if patch.is_file() {
            std::fs::remove_file(&patch).map_err(|e| e.to_string())?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn load_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, String> {
        if self.pending_default_save.is_pending() {
            return Ok(vec![HostMessage::RuntimeResult {
                result: pending_patch_load_failure(RuntimeOperation::StoreLoadDefault)
                    .with_identity(request.request_id.clone(), request.revision),
            }]);
        }
        match self.load_default_payload()? {
            Ok(payload) => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult { payload }
                    .with_identity(request.request_id.clone(), request.revision),
            }]),
            Err(error) => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: RuntimeErrorFacts::new(
                        RuntimeErrorDomain::Storage,
                        RuntimeErrorCode::OperationFailed,
                        RuntimeOperation::StoreLoadDefault,
                        Some(format!("Default load failed: {error}")),
                    ),
                }
                .with_identity(request.request_id.clone(), request.revision),
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
            self.pending_default_save.schedule(
                payload.clone(),
                Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS),
                request.clone(),
            );
            return Ok(vec![]);
        }
        self.pending_default_save.cancel();
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
        atomic_write_json(&self.store_dir.join("recovery-save.patch.json"), payload)
    }

    fn load_default_payload(&self) -> Result<Result<Option<serde_json::Value>, String>, String> {
        let path = self.store_dir.join("default.patch.json");
        if !path.is_file() {
            return Ok(Ok(None));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&content)
            .map(Some)
            .map_err(|e| e.to_string()))
    }
}

fn pending_patch_load_failure(operation: RuntimeOperation) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Storage,
            RuntimeErrorCode::OperationFailed,
            operation,
            Some("Save pending, try again".to_string()),
        ),
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
