use crate::native_menu::NativeMenuAction;
use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};
use serde_json::Value;

use super::restart_settings::{DefaultSaveScope, DefaultWriteCompletion, RestartSetting};
use super::UsbDataRole;
use super::{validate_config_payload, NativeConfirmDialog, NativeRunner, NativeToast};

impl NativeRunner {
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
        self.mark_config_dirty();
        if self.menu.state.editing {
            return;
        }
        let current = self.config_payload();
        let has_pending_write = self.restart_settings.has_pending_write();
        if has_pending_write && self.auto_save_default {
            self.restart_settings.defer_restart_after_pending_write();
            return;
        }
        let setting_payload = (!has_pending_write)
            .then(|| self.setting_payload_for_restart(&current, setting))
            .flatten();
        if self.auto_save_default && !has_pending_write {
            self.start_restart_default_save(current, DefaultSaveScope::RestartEverything);
        } else {
            self.restart_settings
                .open_save_choice(setting, setting_payload.clone());
            self.display.confirm_dialog = Some(self.save_choice_dialog());
        }
    }

    pub(super) fn start_restart_default_save(&mut self, payload: Value, scope: DefaultSaveScope) {
        if !self
            .restart_settings
            .start_write(payload.clone(), scope, self.config_revision)
        {
            self.display.confirm_dialog = Some(self.save_choice_dialog());
            return;
        }
        self.pending.pending_autosave_payload_due_at = None;
        self.pending.pending_save_revision = Some(self.config_revision);
        self.display.confirm_dialog = Some(saving_dialog());
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::StoreSaveDefault {
                payload,
                mode: scope.mode(),
            });
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
        self.restart_settings.track_write(payload, scope, revision)
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
        self.pending.pending_save_revision = None;
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
        let completion = self.restart_settings.acknowledge_write(
            request_id,
            revision,
            result.error_facts().is_none(),
        )?;
        if self.pending.pending_save_revision == revision {
            self.pending.pending_save_revision = None;
        }
        let restart_after_pending_write =
            completion.succeeded && self.restart_settings.take_restart_after_pending_write();
        if !completion.succeeded && self.auto_save_default {
            self.pending.pending_autosave_payload_due_at =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(150));
        }
        if restart_after_pending_write && completion.succeeded {
            self.start_restart_default_save(
                self.config_payload(),
                DefaultSaveScope::RestartEverything,
            );
        } else if completion.restart_flow {
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
                let Some(payload) = self.restart_settings.save_choice_setting_payload() else {
                    return Ok(None);
                };
                self.start_restart_default_save(payload, DefaultSaveScope::RestartSetting);
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
                self.start_restart_default_save(
                    self.config_payload(),
                    DefaultSaveScope::RestartEverything,
                );
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
        let mut current = self.config_payload();
        let mut baseline = self.restart_settings.persisted_default.clone();
        if let (Some(current), Some(baseline)) = (current.as_object_mut(), baseline.as_object_mut())
        {
            current.remove("revision");
            baseline.remove("revision");
        }
        if current == baseline {
            self.config_dirty = false;
            self.dirty_revision = None;
        } else {
            self.config_dirty = true;
        }
    }

    fn setting_payload_for_restart(
        &self,
        current: &Value,
        setting: RestartSetting,
    ) -> Option<Value> {
        self.restart_settings
            .setting_payload(current, setting, self.config_revision)
            .filter(|payload| {
                !setting.is_audio_output() || validate_config_payload(payload).is_ok()
            })
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
        options.push("Save this setting".into());
    }
    options.push("Save everything".into());
    NativeConfirmDialog {
        title: "Save Setting".into(),
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
                "Saved. Reboot to".into(),
                "apply?".into(),
                "Unplug computer USB".into(),
                "before Host reboot.".into(),
                "Audio/MIDI/SD2 off.".into(),
            ]
        } else {
            vec!["Saved. Reboot to".into(), "apply?".into()]
        },
        options: vec!["Continue".into(), "Reboot now".into()],
        cursor: 0,
        action: NativeMenuAction::PlatformEffect("restart.reboot".into()),
        cancel_toast: None,
        confirm_before_execute: false,
    }
}
