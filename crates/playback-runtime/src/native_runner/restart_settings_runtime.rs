use crate::native_menu::NativeMenuAction;
use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};
use serde_json::Value;
use std::sync::Arc;

use super::restart_settings::{DefaultSaveScope, DefaultWriteCompletion, RestartSetting};
use super::UsbDataRole;
use super::{NativeConfirmDialog, NativeManualSaveRequest, NativeRunner, NativeToast};

impl NativeRunner {
    pub(super) fn begin_system_parameter_edit(&mut self, key: &str) {
        if let Ok(document) =
            super::system_persistence::SystemPersistenceState::document_for_ordinary_save(self)
        {
            if let Some(value) =
                super::system_persistence::SystemPersistenceState::ordinary_system_value(
                    &document, key,
                )
            {
                self.pending.system_persistence.begin_edit(key, value);
            }
        }
    }

    pub(super) fn finish_system_parameter_edit(&mut self, key: &str) {
        let Ok(document) =
            super::system_persistence::SystemPersistenceState::document_for_ordinary_save(self)
        else {
            return;
        };
        let Some(value) = super::system_persistence::SystemPersistenceState::ordinary_system_value(
            &document, key,
        ) else {
            return;
        };
        if !self.pending.system_persistence.finish_edit(key, &value) {
            if let Ok(live_document) =
                super::system_persistence::SystemPersistenceState::system_document(self)
            {
                self.pending
                    .system_persistence
                    .clear_dirty_if_matches_baseline(&live_document);
            }
            return;
        }
        let Some(dirty_revision) = self.pending.system_persistence.dirty_revision else {
            return;
        };
        if self
            .pending
            .system_persistence
            .complete_auto_save(document.clone(), dirty_revision)
        {
            self.outbox
                .push_platform_effect(RuntimePlatformEffect::StoreSaveSystem { payload: document });
        }
    }

    pub(super) fn finish_restart_sensitive_edit(&mut self, key: &str) {
        let Some(setting) = RestartSetting::from_key(key) else {
            return;
        };
        let current = self.config_payload();
        if self.restart_settings.finish_edit(&current, setting) {
            self.commit_restart_sensitive_setting(setting);
        }
    }

    pub(super) fn commit_restart_sensitive_setting(&mut self, setting: RestartSetting) {
        self.mark_system_dirty();
        if self.menu.state.editing {
            return;
        }
        let current = self.config_payload();
        if self.restart_settings.is_saving() {
            self.show_toast("System Apply save pending, try again");
            return;
        }
        let setting_payload = (!self.restart_settings.has_pending_write())
            .then(|| self.setting_payload_for_restart(&current, setting))
            .flatten();
        self.restart_settings
            .open_save_choice(setting, setting_payload);
        self.display.confirm_dialog = Some(self.save_choice_dialog());
    }

    pub(super) fn start_restart_system_save(&mut self, payload: Value, scope: DefaultSaveScope) {
        if self.restart_settings.has_pending_write()
            || self.pending.system_persistence.has_pending_request()
            || self.pending.system_persistence.has_completed_auto_save()
        {
            self.show_toast("System save pending, try again");
            return;
        }
        if !self
            .restart_settings
            .start_write(payload.clone(), scope, self.config_revision, None)
        {
            self.display.confirm_dialog = Some(self.save_choice_dialog());
            return;
        }
        self.display.confirm_dialog = Some(saving_dialog());
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::StoreSaveSystem { payload });
    }

    pub fn register_native_default_write(
        &mut self,
        request_id: &str,
        revision: u64,
        is_auto: bool,
    ) -> bool {
        let scope = if is_auto {
            DefaultSaveScope::Autosave
        } else {
            DefaultSaveScope::Ordinary
        };
        if !self
            .restart_settings
            .register_native_write_with_patch_revision(
                request_id,
                revision,
                scope,
                self.dirty_revision,
            )
        {
            return false;
        }
        self.pending.pending_save_revision = Some(revision);
        true
    }

    pub fn attach_native_default_write_payload(
        &mut self,
        request_id: &str,
        revision: u64,
        payload: Arc<Value>,
    ) -> bool {
        let Ok(system) = super::system_persistence::SystemPersistenceState::system_document(self)
        else {
            return false;
        };
        if super::compose_local_system_patch_documents(&system, &payload).is_err() {
            return false;
        }
        self.restart_settings
            .attach_native_payload(request_id, revision, payload)
    }

    pub(super) fn register_default_write(
        &mut self,
        payload: Value,
        scope: DefaultSaveScope,
    ) -> bool {
        let revision = payload
            .get("revision")
            .and_then(Value::as_u64)
            .unwrap_or(self.config_revision);
        self.restart_settings
            .track_write(payload, scope, revision, self.dirty_revision)
    }

    pub(super) fn register_default_write_request(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
    ) {
        self.restart_settings.register_request(request_id, revision);
    }

    pub(super) fn pending_default_write_revision(&self) -> Option<u64> {
        self.restart_settings.pending_write_revision()
    }

    pub(super) fn abandon_pending_default_write(&mut self) {
        self.restart_settings.abandon_pending_write();
        self.restart_settings.cancel();
        self.pending.pending_save_revision = None;
        self.display.confirm_dialog = None;
    }

    pub(super) fn reboot_blocked_by_pending_saves(&self) -> Option<&'static str> {
        if self.pending.system_persistence.has_pending_request()
            || self.pending.system_persistence.has_completed_auto_save()
        {
            return Some("System save pending, try again");
        }
        if self.pending.system_persistence.dirty_revision.is_some() {
            return Some("Save or apply System settings before reboot");
        }
        if self.restart_settings.has_pending_patch_write()
            || self.pending.pending_save_revision.is_some()
            || matches!(
                &self.pending.manual_save_request,
                Some(NativeManualSaveRequest::Default)
            )
        {
            return Some("Patch save pending, try again");
        }
        if self.pending.native_preset_write.is_some()
            || matches!(
                &self.pending.manual_save_request,
                Some(NativeManualSaveRequest::Preset { .. })
            )
        {
            return Some("Preset save pending, try again");
        }
        self.config_dirty.then_some(if self.auto_save_default {
            "Patch save pending, try again"
        } else {
            "Save or discard Patch before reboot"
        })
    }

    pub(super) fn system_apply_pending(&self) -> bool {
        self.restart_settings
            .pending_write_scope()
            .is_some_and(DefaultSaveScope::is_restart)
    }

    pub(super) fn apply_restart_default_save_result(
        &mut self,
        result: &RuntimeStoreResult,
        request_id: &str,
        revision: Option<u64>,
    ) -> Option<DefaultWriteCompletion> {
        if result.operation() != crate::RuntimeOperation::StoreSaveDefault {
            return None;
        }
        self.apply_restart_save_completion(result, request_id, revision, true)
    }

    pub(super) fn apply_restart_system_save_result(
        &mut self,
        result: &RuntimeStoreResult,
        request_id: &str,
        revision: Option<u64>,
    ) -> Option<DefaultWriteCompletion> {
        if result.operation() != crate::RuntimeOperation::StoreSaveSystem {
            return None;
        }
        self.apply_restart_save_completion(result, request_id, revision, false)
    }

    fn apply_restart_save_completion(
        &mut self,
        result: &RuntimeStoreResult,
        request_id: &str,
        revision: Option<u64>,
        retry_patch_autosave: bool,
    ) -> Option<DefaultWriteCompletion> {
        let completion = self.restart_settings.acknowledge_request(
            request_id,
            revision,
            result.error_facts().is_none(),
            self.config_revision,
        )?;
        if self.pending.pending_save_revision == Some(completion.revision) {
            self.pending.pending_save_revision = None;
        }
        if retry_patch_autosave && !completion.succeeded && self.auto_save_default {
            self.pending.pending_autosave_payload_due_at =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(150));
        }
        if completion.restart_flow {
            if completion.succeeded {
                if completion.scope == DefaultSaveScope::RestartSetting {
                    self.reconcile_dirty_with_persisted_default();
                }
                self.display.confirm_dialog = Some(restart_choice_dialog(completion.host_role));
            } else {
                self.display.confirm_dialog = None;
                self.display.toast = Some(NativeToast {
                    message: "Save failed".into(),
                    offset: 0,
                });
            }
        } else {
            self.refresh_restart_save_choice();
        }
        Some(completion)
    }

    pub(super) fn cancel_restart_flow(&mut self) {
        self.restart_settings.cancel();
        self.display.confirm_dialog = None;
    }

    pub(super) fn execute_restart_action(
        &mut self,
        action: &str,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        match action {
            "restart.saveSetting" => {
                if self.restart_settings.has_pending_write() {
                    self.display.confirm_dialog = Some(self.save_choice_dialog());
                    return Ok(None);
                }
                let Some(setting) = self.restart_settings.save_choice_setting() else {
                    return Ok(None);
                };
                let Some(payload) =
                    self.setting_payload_for_restart(&self.config_payload(), setting)
                else {
                    return Ok(None);
                };
                self.start_restart_system_save(payload, DefaultSaveScope::RestartSetting);
                Ok(None)
            }
            "restart.saveEverything" => {
                if !self.restart_settings.is_save_choice() {
                    return Ok(None);
                }
                if self.restart_settings.has_pending_write() {
                    self.display.confirm_dialog = Some(self.save_choice_dialog());
                    return Ok(None);
                }
                let payload =
                    super::system_persistence::SystemPersistenceState::system_document(self)?;
                self.start_restart_system_save(payload, DefaultSaveScope::RestartEverything);
                Ok(None)
            }
            "restart.reboot" if self.restart_settings.is_restart_choice() => {
                self.restart_settings.continue_after_save();
                self.start_power_action("system.reboot")
            }
            _ => Ok(None),
        }
    }

    fn reconcile_dirty_with_persisted_default(&mut self) {
        if let Ok(current) =
            super::system_persistence::SystemPersistenceState::system_document(self)
        {
            if current == *self.restart_settings.persisted_default {
                self.pending.system_persistence.dirty_revision = None;
            } else if self.pending.system_persistence.dirty_revision.is_none() {
                self.mark_system_dirty();
            }
        }
    }

    fn setting_payload_for_restart(
        &self,
        current: &Value,
        setting: RestartSetting,
    ) -> Option<Value> {
        let payload = self.restart_settings.setting_payload(current, setting)?;
        let patch = super::split_local_system_patch_documents(current)
            .ok()?
            .patch;
        super::compose_local_system_patch_documents(&payload, &patch)
            .is_ok()
            .then_some(payload)
    }

    fn refresh_restart_save_choice(&mut self) {
        let Some(setting) = self.restart_settings.save_choice_setting() else {
            return;
        };
        if self.restart_settings.has_pending_write() {
            return;
        }
        let payload = self.setting_payload_for_restart(&self.config_payload(), setting);
        self.restart_settings
            .update_save_choice_setting_payload(payload.clone());
        self.display.confirm_dialog = Some(self.save_choice_dialog());
    }

    fn save_choice_dialog(&self) -> NativeConfirmDialog {
        save_choice_dialog(
            self.restart_settings
                .save_choice_setting_payload()
                .is_some(),
            self.usb_data_role == UsbDataRole::Host,
        )
    }
}

fn save_choice_dialog(can_save_setting: bool, host_role: bool) -> NativeConfirmDialog {
    let mut options = vec!["Cancel".into()];
    if can_save_setting {
        options.push("Save this one".into());
    }
    options.push("Save System".into());
    NativeConfirmDialog {
        title: "Apply System".into(),
        lines: if host_role {
            vec![
                "Restart required.".into(),
                "Before Host reboot:".into(),
                "Unplug computer USB.".into(),
                "Audio/MIDI/SD2 off.".into(),
            ]
        } else {
            vec!["Restart required.".into()]
        },
        options,
        cursor: 0,
        action: NativeMenuAction::PlatformEffect("restart.saveChoice".into()),
        cancel_toast: Some("Cancelled".into()),
        confirm_before_execute: false,
    }
}

fn saving_dialog() -> NativeConfirmDialog {
    NativeConfirmDialog {
        title: "Saving...".into(),
        lines: vec!["Please wait".into()],
        options: Vec::new(),
        cursor: 0,
        action: NativeMenuAction::PlatformEffect("restart.saving".into()),
        cancel_toast: None,
        confirm_before_execute: false,
    }
}

fn restart_choice_dialog(host_role: bool) -> NativeConfirmDialog {
    NativeConfirmDialog {
        title: "Restart?".into(),
        lines: if host_role {
            vec![
                "System saved.".into(),
                "Reboot to apply?".into(),
                "Unplug computer USB".into(),
                "before Host reboot.".into(),
                "Audio/MIDI/SD2 off.".into(),
            ]
        } else {
            vec!["System saved.".into(), "Reboot to apply?".into()]
        },
        options: vec!["Continue".into(), "Reboot now".into()],
        cursor: 0,
        action: NativeMenuAction::PlatformEffect("restart.reboot".into()),
        cancel_toast: None,
        confirm_before_execute: false,
    }
}
