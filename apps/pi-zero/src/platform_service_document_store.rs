use super::{default_patch_path, recovery_patch_path, save_json, PiPlatformService};

impl PiPlatformService {
    pub(crate) fn save_recovery_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        if self.store_write_barrier.is_blocked() {
            return Err(
                "patch save blocked while restore awaits restored-state acknowledgement".into(),
            );
        }
        super::platform_service_store::validate_patch_document(&self.store_dir, payload)?;
        save_json(&recovery_patch_path(&self.store_dir), payload)
    }

    pub(crate) fn load_system_now(&self) -> Result<Option<serde_json::Value>, String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        super::load_json(&self.store_dir.join("system.json"))
    }

    pub(crate) fn save_system_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        if self.store_write_barrier.is_blocked() {
            return Err(
                "system save blocked while restore awaits restored-state acknowledgement".into(),
            );
        }
        crate::usb_config_validation::validate_pi_audio_outputs_payload(payload)?;
        let patch = super::load_json(&default_patch_path(&self.store_dir))?
            .ok_or_else(|| "Default patch is missing".to_string())?;
        playback_runtime::compose_local_system_patch_documents(payload, &patch)?;
        save_json(&self.store_dir.join("system.json"), payload)
    }
}
