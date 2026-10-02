use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};
use serde_json::Value;
use std::sync::Arc;

use super::{NativeRunner, NativeToast};

impl NativeRunner {
    pub(super) fn apply_system_store_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<Option<RuntimeStoreResult>, String> {
        let (result, request_id, revision) = match result {
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            } if !matches!(result.as_ref(), RuntimeStoreResult::Identified { .. }) => {
                (*result, request_id, revision)
            }
            RuntimeStoreResult::RuntimeFailure { error }
                if error.request_id.is_some() || error.revision.is_some() =>
            {
                let request_id = error.request_id.clone().unwrap_or_default();
                let revision = error.revision;
                (
                    RuntimeStoreResult::RuntimeFailure { error },
                    request_id,
                    revision,
                )
            }
            _ => return Ok(None),
        };
        let operation = result.operation();
        if !super::store::is_system_store_operation(&operation) {
            return Ok(None);
        }
        if !super::system_persistence::SystemPersistenceState::accepts_result(&result) {
            return Ok(None);
        }
        let Some(pending) = self.pending.system_persistence.take_matching_request(
            &operation,
            &request_id,
            revision,
        ) else {
            return Ok(None);
        };
        let operation_for_restart = operation.clone();
        let request_id_for_restart = request_id.clone();
        let is_auto_save = self
            .pending
            .system_persistence
            .is_auto_save_request(&request_id);

        let accepted = match result {
            RuntimeStoreResult::LoadSystemResult {
                payload: Some(system),
            } => {
                if let Err(message) = self.apply_system_document_preserving_patch(&system) {
                    self.system_store_failure(operation, message, Some(request_id), revision)?
                } else {
                    let baseline = self.pending.system_persistence.record_system_load(&system);
                    self.restart_settings.update_system_baseline(baseline);
                    if self.restore_rehydration_pending() {
                        self.outbox
                            .push_platform_effect(RuntimePlatformEffect::StoreLoadDefault);
                    }
                    self.display.toast = Some(NativeToast {
                        message: "System loaded".into(),
                        offset: 0,
                    });
                    system_store_success(operation, request_id, revision)
                }
            }
            RuntimeStoreResult::LoadSystemResult { payload: None } => self.system_store_failure(
                operation,
                "System document is unavailable".into(),
                Some(request_id),
                revision,
            )?,
            RuntimeStoreResult::SaveSystemResult { ok: true } => {
                let baseline = self
                    .pending
                    .system_persistence
                    .acknowledge_system_save(&pending);
                self.restart_settings
                    .update_system_baseline(Arc::clone(&baseline));
                if !is_auto_save {
                    self.display.toast = Some(NativeToast {
                        message: "System saved".into(),
                        offset: 0,
                    });
                }
                system_store_success(operation, request_id, revision)
            }
            RuntimeStoreResult::SaveSystemResult { ok: false } => self.system_store_failure(
                operation,
                "System save failed".into(),
                Some(request_id),
                revision,
            )?,
            RuntimeStoreResult::RuntimeFailure { error } => {
                let failure = RuntimeStoreResult::RuntimeFailure {
                    error: error
                        .with_context(crate::RuntimeErrorDomain::Storage, operation, None, None)
                        .with_identity(Some(request_id), revision),
                };
                self.apply_error_presentation_result(failure.clone())?;
                failure
            }
            _ => return Ok(None),
        };
        if operation_for_restart == crate::RuntimeOperation::StoreLoadSystem
            && accepted.error_facts().is_some()
            && self.restore_rehydration_pending()
        {
            self.fail_restore_rehydration();
        }
        if operation_for_restart == crate::RuntimeOperation::StoreSaveSystem
            && pending.apply_scope.is_some()
        {
            self.apply_restart_system_save_result(&accepted, &request_id_for_restart, revision);
        }
        if operation_for_restart == crate::RuntimeOperation::StoreSaveSystem {
            self.pending
                .system_persistence
                .finish_auto_save_request(&request_id_for_restart);
            if let Some((payload, _)) = self.pending.system_persistence.take_waiting_auto_save() {
                self.outbox
                    .push_platform_effect(RuntimePlatformEffect::StoreSaveSystem { payload });
            }
        }
        Ok(Some(accepted))
    }

    fn system_store_failure(
        &mut self,
        operation: crate::RuntimeOperation,
        message: String,
        request_id: Option<String>,
        revision: Option<u64>,
    ) -> Result<RuntimeStoreResult, String> {
        let failure = RuntimeStoreResult::RuntimeFailure {
            error: crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Storage,
                crate::RuntimeErrorCode::OperationFailed,
                operation,
                Some(message),
            )
            .with_identity(request_id, revision),
        };
        self.apply_error_presentation_result(failure.clone())?;
        Ok(failure)
    }

    pub(super) fn apply_store_persistence_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        self.apply_store_persistence_result_with_default_feedback(result, true)
    }

    pub(super) fn apply_store_persistence_result_with_default_feedback(
        &mut self,
        result: RuntimeStoreResult,
        allow_default_feedback: bool,
    ) -> Result<(), String> {
        if result.operation() == crate::RuntimeOperation::StoreLoadDefault
            && self.restore_rehydration_pending()
        {
            return Ok(());
        }
        match result {
            RuntimeStoreResult::ListPresetsResult { names } => {
                self.preset_names = names;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            } => {
                let _ = self.apply_default_patch_result_for_runtime(
                    RuntimeStoreResult::LoadDefaultResult {
                        payload: Some(payload),
                    },
                )?;
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
                if self.restart_settings.take_native_payload_missing() {
                    self.apply_error_presentation_result(RuntimeStoreResult::RuntimeFailure {
                        error: crate::RuntimeErrorFacts::new(
                            crate::RuntimeErrorDomain::Storage,
                            crate::RuntimeErrorCode::OperationFailed,
                            crate::RuntimeOperation::StoreSaveDefault,
                            Some("native save payload unavailable".into()),
                        ),
                    })?;
                } else if allow_default_feedback {
                    self.show_saved_patch_feedback();
                }
                self.acknowledge_config_save(None);
            }
            result @ (RuntimeStoreResult::LoadSystemResult { .. }
            | RuntimeStoreResult::SaveSystemResult { .. }) => {
                return self.apply_system_store_result(result).map(|_| ());
            }
            RuntimeStoreResult::SaveBackupResult { .. }
            | RuntimeStoreResult::SaveRecoveryResult { .. }
            | RuntimeStoreResult::DeletePresetResult { ok: false, .. }
            | RuntimeStoreResult::SaveDefaultResult { ok: false, .. } => {}
            RuntimeStoreResult::LoadDefaultResult { payload: None } => {}
            _ => {}
        }
        Ok(())
    }

    pub(super) fn apply_restored_patch_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<Option<(RuntimeStoreResult, bool)>, String> {
        let (request_id, revision) = match &result {
            RuntimeStoreResult::Identified {
                request_id,
                revision,
                ..
            } => (Some(request_id.as_str()), *revision),
            RuntimeStoreResult::RuntimeFailure { error } => {
                (error.request_id.as_deref(), error.revision)
            }
            _ => (None, None),
        };
        let Some(request_id) = request_id else {
            return Ok(None);
        };
        let request_id = request_id.to_owned();
        if !self.accept_restore_patch_request(&request_id, revision) {
            return Ok(None);
        }

        let result = match result {
            RuntimeStoreResult::Identified { result, .. } => *result,
            result => result,
        };
        match result {
            RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            } => {
                let accepted = self.apply_default_patch_result_for_runtime(
                    RuntimeStoreResult::LoadDefaultResult {
                        payload: Some(payload),
                    }
                    .with_identity(request_id.clone(), revision),
                )?;
                if accepted.error_facts().is_some() {
                    self.fail_restore_patch_rehydration();
                    return Ok(Some((accepted, false)));
                }
                let accepted_patch = match &accepted {
                    RuntimeStoreResult::Identified { result, .. } => matches!(
                        result.as_ref(),
                        RuntimeStoreResult::LoadDefaultResult { payload: Some(_) }
                    ),
                    RuntimeStoreResult::LoadDefaultResult { payload: Some(_) } => true,
                    _ => false,
                };
                if accepted_patch {
                    self.finish_restore_rehydration();
                    Ok(Some((accepted, true)))
                } else {
                    self.restored_patch_failure(
                        "unexpected response while applying restored Patch".into(),
                        request_id,
                        revision,
                    )
                    .map(|result| Some((result, false)))
                }
            }
            RuntimeStoreResult::LoadDefaultResult { payload: None } => self
                .restored_patch_failure(
                    "restored Patch document is unavailable".into(),
                    request_id,
                    revision,
                )
                .map(|result| Some((result, false))),
            RuntimeStoreResult::RuntimeFailure { error } => {
                let failure = self.apply_default_patch_result_for_runtime(
                    RuntimeStoreResult::RuntimeFailure { error }
                        .with_identity(request_id.clone(), revision),
                )?;
                self.fail_restore_patch_rehydration();
                Ok(Some((failure, false)))
            }
            _ => self
                .restored_patch_failure(
                    "unexpected response for restored Patch request".into(),
                    request_id,
                    revision,
                )
                .map(|result| Some((result, false))),
        }
    }

    pub(super) fn apply_default_patch_result_for_runtime(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<RuntimeStoreResult, String> {
        let (result, request_id, revision) = match result {
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            } => (*result, Some(request_id), revision),
            RuntimeStoreResult::RuntimeFailure { error } => {
                let request_id = error.request_id.clone();
                let revision = error.revision;
                (
                    RuntimeStoreResult::RuntimeFailure { error },
                    request_id,
                    revision,
                )
            }
            result => (result, None, None),
        };
        let accepted = match result {
            RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            } => match self.apply_default_patch_document(payload.clone()) {
                Ok(()) => RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(payload),
                },
                Err(message) => RuntimeStoreResult::RuntimeFailure {
                    error: crate::RuntimeErrorFacts::new(
                        crate::RuntimeErrorDomain::Storage,
                        crate::RuntimeErrorCode::InvalidPayload,
                        crate::RuntimeOperation::StoreLoadDefault,
                        Some(message),
                    ),
                },
            },
            RuntimeStoreResult::RuntimeFailure { error } => RuntimeStoreResult::RuntimeFailure {
                error: error.with_context(
                    crate::RuntimeErrorDomain::Storage,
                    crate::RuntimeOperation::StoreLoadDefault,
                    None,
                    None,
                ),
            },
            other => other,
        };
        let accepted = if let Some(request_id) = request_id {
            accepted.with_identity(request_id, revision)
        } else {
            accepted
        };
        if accepted.error_facts().is_some() {
            let presentation_result = match &accepted {
                RuntimeStoreResult::Identified { result, .. } => result.as_ref().clone(),
                result => result.clone(),
            };
            self.apply_error_presentation_result(presentation_result)?;
        }
        Ok(accepted)
    }

    fn apply_default_patch_document(&mut self, payload: Value) -> Result<(), String> {
        let system = super::system_persistence::SystemPersistenceState::system_document(self)?;
        super::compose_local_system_patch_documents(&system, &payload)?;
        self.apply_local_patch_payload_preserving_device(payload.clone())?;
        self.pending.saved_patch_baseline = Some(Arc::new(payload));
        self.stop_for_config_load();
        self.display.toast = Some(NativeToast {
            message: "Patch loaded".into(),
            offset: 0,
        });
        self.current_preset_name = None;
        Ok(())
    }

    fn restored_patch_failure(
        &mut self,
        message: String,
        request_id: String,
        revision: Option<u64>,
    ) -> Result<RuntimeStoreResult, String> {
        let failure = RuntimeStoreResult::RuntimeFailure {
            error: crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Storage,
                crate::RuntimeErrorCode::InvalidPayload,
                crate::RuntimeOperation::StoreLoadDefault,
                Some(message),
            )
            .with_identity(Some(request_id), revision),
        };
        self.fail_restore_patch_rehydration();
        self.apply_error_presentation_result(failure.clone())?;
        Ok(failure)
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

    pub(super) fn acknowledge_patch_save(&mut self, captured_patch_dirty_revision: Option<u64>) {
        if captured_patch_dirty_revision.is_some()
            && self.dirty_revision == captured_patch_dirty_revision
        {
            self.config_dirty = false;
            self.dirty_revision = None;
        }
    }

    pub(super) fn retry_config_save_after_restore_failure(&mut self) {
        self.pending.pending_save_revision = None;
    }
}

fn system_store_success(
    operation: crate::RuntimeOperation,
    request_id: String,
    revision: Option<u64>,
) -> RuntimeStoreResult {
    RuntimeStoreResult::OperationSucceeded {
        operation,
        request_id: Some(request_id),
        revision,
    }
}
