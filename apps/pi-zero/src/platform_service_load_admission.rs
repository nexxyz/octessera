#[cfg(test)]
use super::recovery_patch_path;
use super::{
    default_patch_path, load_json, preset_patch_path, validate_named_preset_document,
    validate_patch_document, PiPlatformService,
};

#[derive(Default)]
pub(super) struct LegacyPatchWrites {
    default: usize,
    named: usize,
}

impl LegacyPatchWrites {
    pub(super) fn increment(&mut self, named: bool) {
        let count = if named {
            &mut self.named
        } else {
            &mut self.default
        };
        *count += 1;
    }

    pub(super) fn decrement(&mut self, named: bool) {
        let count = if named {
            &mut self.named
        } else {
            &mut self.default
        };
        *count = count.saturating_sub(1);
    }
}

impl PiPlatformService {
    pub(crate) fn load_default_now(&self) -> Result<Option<serde_json::Value>, String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        if !self.store_write_barrier.is_blocked() && self.default_write_pending()? {
            return Err("Save pending, try again".into());
        }
        self.load_validated_patch(&default_patch_path(&self.store_dir))
    }

    pub(crate) fn load_preset_now(&self, name: &str) -> Result<Option<serde_json::Value>, String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        if self.store_write_barrier.is_blocked()
            || self.default_write_pending()?
            || self.named_write_pending()?
        {
            return Err("Save pending, try again".into());
        }
        self.load_validated_named_preset(&preset_patch_path(&self.store_dir, name)?)
    }

    #[cfg(test)]
    pub(crate) fn load_recovery_patch_now(&self) -> Result<Option<serde_json::Value>, String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        self.load_validated_patch(&recovery_patch_path(&self.store_dir))
    }

    fn load_validated_patch(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<serde_json::Value>, String> {
        let Some(patch) = load_json(path)? else {
            return Ok(None);
        };
        validate_patch_document(&self.store_dir, &patch)?;
        Ok(Some(patch))
    }

    fn load_validated_named_preset(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<serde_json::Value>, String> {
        let Some(patch) = load_json(path)? else {
            return Ok(None);
        };
        validate_named_preset_document(&self.store_dir, &patch)?;
        Ok(Some(patch))
    }

    fn default_write_pending(&self) -> Result<bool, String> {
        let pending = self
            .legacy_patch_writes
            .lock()
            .map_err(|_| "pi store write state is unavailable".to_string())?;
        Ok(pending.default > 0 || self.native_default_write().is_some())
    }

    fn named_write_pending(&self) -> Result<bool, String> {
        let pending = self
            .legacy_patch_writes
            .lock()
            .map_err(|_| "pi store write state is unavailable".to_string())?;
        Ok(pending.named > 0 || self.native_preset_write().is_some())
    }
}
