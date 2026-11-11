use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::persistence::{preset_name_from_file_name, preset_patch_file_path};
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePlatformRequest, RuntimeStoreResult,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use store_files::{read_json_file, write_json_atomically};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl DesktopPlaybackHostAdapter {
    pub(super) fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        write_json_atomically(&self.store_dir.join("default.patch.json"), payload)
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

    fn load_system_payload(&self) -> Result<Option<serde_json::Value>, String> {
        read_json_file(&self.store_dir.join("system.json"))
    }

    pub(super) fn list_preset_names(&self) -> Result<Vec<String>, String> {
        let presets_dir = self.store_dir.join("presets");
        let mut names = std::collections::BTreeSet::new();
        let patch_dir = presets_dir.join("patches");
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
        read_json_file(&preset_patch_file_path(
            &self.store_dir.join("presets"),
            name,
        )?)
    }

    /// Saves the preset; `true` when it replaced a preset whose name differs only in case,
    /// which names the same file on the desktop's case-insensitive store.
    pub(super) fn save_preset_payload(
        &self,
        name: &str,
        payload: &serde_json::Value,
    ) -> Result<bool, String> {
        let presets_dir = self.store_dir.join("presets");
        std::fs::create_dir_all(&presets_dir).map_err(|e| e.to_string())?;
        let path = preset_patch_file_path(&presets_dir, name)?;
        let case_variant = stored_entry(&path, false)?;
        if let Some(case_variant) = &case_variant {
            std::fs::rename(case_variant, &path).map_err(|e| e.to_string())?;
        }
        write_json_atomically(&path, payload)?;
        Ok(case_variant.is_some())
    }

    /// Deletes the preset. A name whose file a case-only rename took over is already gone, so
    /// its delete succeeds without touching the renamed preset.
    pub(super) fn delete_preset_payload(&self, name: &str) -> Result<bool, String> {
        let presets_dir = self.store_dir.join("presets");
        let patch = preset_patch_file_path(&presets_dir, name)?;
        if stored_entry(&patch, true)?.is_some() {
            std::fs::remove_file(&patch).map_err(|e| e.to_string())?;
            return Ok(true);
        }
        Ok(stored_entry(&patch, false)?.is_some())
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
        store_files::write_rotated_backup(&self.store_dir.join("backups"), payload)
    }

    pub(super) fn save_recovery_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        write_json_atomically(&self.store_dir.join("recovery-save.patch.json"), payload)
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

/// The stored file named exactly like `path`, or, with `exact` false, one named like it in
/// another case.
fn stored_entry(path: &Path, exact: bool) -> Result<Option<PathBuf>, String> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return Ok(None);
    };
    if !dir.is_dir() {
        return Ok(None);
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name();
        let Some(entry_name) = file_name.to_str() else {
            continue;
        };
        let matches = if exact {
            entry_name == name
        } else {
            entry_name != name && entry_name.to_lowercase() == name.to_lowercase()
        };
        if matches && entry.path().is_file() {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
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
