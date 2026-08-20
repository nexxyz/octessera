use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};

use super::{NativeRunner, NativeToast};

impl NativeRunner {
    pub(super) fn apply_store_persistence_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        match result {
            RuntimeStoreResult::ListPresetsResult { names } => {
                self.preset_names = names;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            } => {
                self.apply_config_payload(payload)?;
                self.stop_for_config_load();
                self.display.toast = Some(NativeToast {
                    message: "Default loaded".into(),
                    offset: 0,
                });
                if self.restore_rehydration_pending() {
                    self.finish_restore_rehydration();
                }
            }
            RuntimeStoreResult::LoadPresetResult { name, payload } => {
                if let Some(payload) = payload {
                    self.apply_patch_payload_preserving_device(payload)?;
                    self.stop_for_config_load();
                }
                self.display.toast = Some(NativeToast {
                    message: format!("Loaded {name}"),
                    offset: 0,
                });
                self.current_preset_name = Some(name);
            }
            RuntimeStoreResult::SavePresetResult { name, .. } => {
                if let Some(source) = self.preset_rename_source.take() {
                    if source != name {
                        self.outbox.push_platform_effect(
                            RuntimePlatformEffect::StoreDeletePreset { name: source },
                        );
                    }
                }
                self.display.toast = Some(NativeToast {
                    message: format!("Saved {name}"),
                    offset: 0,
                });
                self.current_preset_name = Some(name);
                self.acknowledge_config_save(None);
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::DeletePresetResult { name, ok } if ok => {
                if self.current_preset_name.as_deref() == Some(name.as_str()) {
                    self.current_preset_name = None;
                }
                self.display.toast = Some(NativeToast {
                    message: format!("Deleted {name}"),
                    offset: 0,
                });
            }
            RuntimeStoreResult::SaveDefaultResult { ok, is_auto: _ } if ok => {
                self.show_saved_default_feedback();
                self.acknowledge_config_save(None);
            }
            RuntimeStoreResult::SaveBackupResult { .. }
            | RuntimeStoreResult::SaveRecoveryResult { .. }
            | RuntimeStoreResult::DeletePresetResult { ok: false, .. }
            | RuntimeStoreResult::SaveDefaultResult { ok: false, .. } => {}
            RuntimeStoreResult::LoadDefaultResult { payload: None }
                if self.restore_rehydration_pending() =>
            {
                return Err("restored default payload is unavailable".into());
            }
            RuntimeStoreResult::LoadDefaultResult { payload: None } => {}
            _ => {}
        }
        Ok(())
    }

    pub(super) fn acknowledge_config_save(&mut self, revision: Option<u64>) {
        let Some(revision) = revision else {
            return;
        };
        if self.pending.pending_save_revision == Some(revision) {
            self.pending.pending_save_revision = None;
        }
        if self.dirty_revision == Some(revision) && self.config_revision == revision {
            self.config_dirty = false;
            self.dirty_revision = None;
        }
    }

    pub(super) fn retry_config_save_after_restore_failure(&mut self) {
        self.pending.pending_save_revision = None;
    }
}
