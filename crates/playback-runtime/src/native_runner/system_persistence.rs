use crate::{RuntimeOperation, RuntimePlatformRequest, RuntimeStoreResult};
use serde_json::{json, Value};
use std::sync::Arc;

use super::restart_settings::DefaultSaveScope;
use super::NativeRunner;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SystemPersistenceState {
    pub(super) dirty_revision: Option<u64>,
    pub(super) saved_baseline: Option<Arc<Value>>,
    pending_request: Option<PendingSystemStoreRequest>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingSystemStoreRequest {
    operation: RuntimeOperation,
    request_id: String,
    revision: Option<u64>,
    save_payload: Option<Arc<Value>>,
    captured_system_dirty_revision: Option<u64>,
    pub(super) apply_scope: Option<DefaultSaveScope>,
}

impl SystemPersistenceState {
    pub(super) fn mark_dirty(&mut self, revision: u64) {
        self.dirty_revision = Some(revision);
    }

    pub(super) fn register_request(
        &mut self,
        request: &RuntimePlatformRequest,
        apply_scope: Option<DefaultSaveScope>,
    ) {
        let operation = request.operation();
        let save_payload = match &request.effect {
            crate::RuntimePlatformEffect::StoreSaveSystem { payload } => {
                Some(Arc::new(payload.clone()))
            }
            _ => None,
        };
        if !matches!(
            &operation,
            RuntimeOperation::StoreLoadSystem | RuntimeOperation::StoreSaveSystem
        ) {
            return;
        }
        self.pending_request = Some(PendingSystemStoreRequest {
            operation,
            request_id: request.request_id.clone(),
            revision: request.revision,
            captured_system_dirty_revision: save_payload.as_ref().and(self.dirty_revision),
            save_payload,
            apply_scope,
        });
    }

    pub(super) fn take_matching_request(
        &mut self,
        operation: &RuntimeOperation,
        request_id: &str,
        revision: Option<u64>,
    ) -> Option<PendingSystemStoreRequest> {
        self.pending_request.as_ref().filter(|request| {
            &request.operation == operation
                && request.request_id == request_id
                && request.revision == revision
        })?;
        self.pending_request.take()
    }

    pub(super) fn record_system_load(&mut self, payload: &Value) -> Arc<Value> {
        self.saved_baseline = Some(Arc::new(payload.clone()));
        self.dirty_revision = None;
        Arc::clone(
            self.saved_baseline
                .as_ref()
                .expect("System baseline was set"),
        )
    }

    pub(super) fn acknowledge_system_save(
        &mut self,
        request: &PendingSystemStoreRequest,
    ) -> Arc<Value> {
        let payload = request
            .save_payload
            .as_ref()
            .expect("accepted System Save request captured its submitted payload");
        self.saved_baseline = Some(Arc::clone(payload));
        if request.captured_system_dirty_revision.is_some()
            && self.dirty_revision == request.captured_system_dirty_revision
        {
            self.dirty_revision = None;
        }
        Arc::clone(payload)
    }

    pub(super) fn has_pending_request(&self) -> bool {
        self.pending_request.is_some()
    }

    pub(super) fn has_saved_aux_side(&self, bank: &str, slot: usize, side: &str) -> bool {
        let slot = format!("aux{}", slot + 1);
        self.saved_baseline
            .as_deref()
            .and_then(|system| system.get("runtimeConfig"))
            .and_then(|runtime| runtime.get(bank))
            .and_then(|bindings| bindings.get(slot.as_str()))
            .and_then(|binding| binding.get(side))
            .is_some_and(|value| !value.is_null())
    }

    pub(super) fn system_document(runner: &NativeRunner) -> Result<Value, String> {
        let device = super::device_config_payload_from_payload(runner.config_payload())?;
        Ok(json!({
            "kind": "octessera.system",
            "schemaVersion": 1,
            "runtimeConfig": device["runtimeConfig"],
        }))
    }

    pub(super) fn patch_document(runner: &NativeRunner) -> Result<Value, String> {
        super::local_patch_payload_for_save(&runner.config_payload())
    }

    pub(super) fn accepts_result(result: &RuntimeStoreResult) -> bool {
        matches!(
            result,
            RuntimeStoreResult::LoadSystemResult { .. }
                | RuntimeStoreResult::SaveSystemResult { .. }
                | RuntimeStoreResult::RuntimeFailure { .. }
        )
    }
}

#[cfg(test)]
#[path = "system_menu_cutover_tests.rs"]
mod menu_tests;
#[cfg(test)]
#[path = "system_persistence_tests.rs"]
mod tests;
