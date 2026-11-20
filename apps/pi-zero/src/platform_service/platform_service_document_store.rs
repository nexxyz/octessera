use super::{default_patch_path, recovery_patch_path, save_json, PiPlatformService};
use std::path::Path;

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

    /// Puts the factory System file in place, keeping the old one beside it.
    pub(crate) fn reset_system_now(&self) -> Result<serde_json::Value, String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        if self.store_write_barrier.is_blocked() {
            return Err(
                "System reset blocked while restore awaits restored-state acknowledgement".into(),
            );
        }
        let factory = crate::store_fault::recovery::factory_documents()?;
        if let Some(patch) = super::load_json(&default_patch_path(&self.store_dir))? {
            playback_runtime::compose_local_system_patch_documents(&factory.system, &patch)?;
        }
        let stamp = playback_runtime::fresh_preset_name();
        crate::store_fault::recovery::reset_system_file(&self.store_dir, &factory, &stamp)?;
        Ok(factory.system)
    }
}

pub(super) fn save_system_document(
    store_dir: &Path,
    payload: &serde_json::Value,
) -> Result<(), String> {
    crate::usb_config_validation::validate_pi_audio_outputs_payload(payload)?;
    let patch = super::load_json(&default_patch_path(store_dir))?
        .ok_or_else(|| "Default patch is missing".to_string())?;
    playback_runtime::compose_local_system_patch_documents(payload, &patch)?;
    save_json(&store_dir.join("system.json"), payload)
}
