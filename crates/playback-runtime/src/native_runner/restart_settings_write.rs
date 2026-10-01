use super::{DefaultSaveScope, RestartSettingsState};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::native_runner) struct DefaultWrite {
    revision: u64,
    request_revision: Option<u64>,
    captured_patch_dirty_revision: Option<u64>,
    payload: Option<Arc<Value>>,
    pub(in crate::native_runner) scope: DefaultSaveScope,
    request_id: Option<String>,
    native_origin: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::native_runner) struct DefaultWriteCompletion {
    pub(in crate::native_runner) revision: u64,
    pub(in crate::native_runner) scope: DefaultSaveScope,
    pub(in crate::native_runner) captured_patch_dirty_revision: Option<u64>,
    pub(in crate::native_runner) restart_flow: bool,
    pub(in crate::native_runner) succeeded: bool,
    pub(in crate::native_runner) host_role: bool,
    pub(in crate::native_runner) show_saved_feedback: bool,
    pub(in crate::native_runner) saved_patch_payload: Option<Arc<Value>>,
}

impl RestartSettingsState {
    pub(in crate::native_runner) fn start_write(
        &mut self,
        payload: Value,
        scope: DefaultSaveScope,
        revision: u64,
        captured_patch_dirty_revision: Option<u64>,
    ) -> bool {
        if self.has_pending_write() {
            return false;
        }
        self.pending_write = Some(DefaultWrite {
            revision,
            request_revision: None,
            captured_patch_dirty_revision,
            payload: Some(Arc::new(payload)),
            scope,
            request_id: None,
            native_origin: false,
        });
        self.native_payload_missing = false;
        if scope.is_restart() {
            self.flow = super::RestartFlow::Saving;
        }
        true
    }

    pub(in crate::native_runner) fn track_write(
        &mut self,
        payload: Value,
        scope: DefaultSaveScope,
        revision: u64,
        captured_patch_dirty_revision: Option<u64>,
    ) -> bool {
        if self.has_pending_write() {
            return false;
        }
        self.pending_write = Some(DefaultWrite {
            revision,
            request_revision: None,
            captured_patch_dirty_revision,
            payload: Some(Arc::new(payload)),
            scope,
            request_id: None,
            native_origin: false,
        });
        self.native_payload_missing = false;
        true
    }

    pub(in crate::native_runner) fn register_native_write_with_patch_revision(
        &mut self,
        request_id: &str,
        revision: u64,
        scope: DefaultSaveScope,
        captured_patch_dirty_revision: Option<u64>,
    ) -> bool {
        if self.has_pending_write() {
            return false;
        }
        self.pending_write = Some(DefaultWrite {
            revision,
            request_revision: Some(revision),
            captured_patch_dirty_revision,
            payload: None,
            scope,
            request_id: Some(request_id.into()),
            native_origin: true,
        });
        self.native_payload_missing = false;
        if scope.is_restart() {
            self.flow = super::RestartFlow::Saving;
        }
        true
    }

    pub(in crate::native_runner) fn attach_native_payload(
        &mut self,
        request_id: &str,
        revision: u64,
        payload: Arc<Value>,
    ) -> bool {
        let Some(write) = self.pending_write.as_mut() else {
            return false;
        };
        if write.revision != revision
            || write.request_id.as_deref() != Some(request_id)
            || payload.get("kind").and_then(Value::as_str) != Some("octessera.patch")
            || payload.get("schemaVersion").and_then(Value::as_u64) != Some(2)
            || write.payload.is_some()
        {
            return false;
        }
        write.payload = Some(payload);
        true
    }

    pub(in crate::native_runner) fn take_native_payload_missing(&mut self) -> bool {
        std::mem::take(&mut self.native_payload_missing)
    }

    pub(in crate::native_runner) fn native_write_matches(
        &self,
        request_id: &str,
        revision: u64,
    ) -> bool {
        self.pending_write.as_ref().is_some_and(|write| {
            write.native_origin
                && write.revision == revision
                && write.request_id.as_deref() == Some(request_id)
                && write.payload.as_ref().is_some_and(|payload| {
                    payload.get("kind").and_then(Value::as_str) == Some("octessera.patch")
                        && payload.get("schemaVersion").and_then(Value::as_u64) == Some(2)
                })
        })
    }

    pub(in crate::native_runner) fn has_pending_native_write(&self) -> bool {
        self.pending_write
            .as_ref()
            .is_some_and(|write| write.native_origin)
    }

    pub(in crate::native_runner) fn register_request(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
    ) {
        let Some(write) = self.pending_write.as_mut() else {
            return;
        };
        if revision.is_some_and(|revision| revision != write.revision)
            || write
                .request_id
                .as_deref()
                .is_some_and(|registered| registered != request_id)
        {
            return;
        }
        write.request_id = Some(request_id.into());
        write.request_revision = revision;
    }

    pub(in crate::native_runner) fn pending_write_revision(&self) -> Option<u64> {
        self.pending_write.as_ref().map(|write| write.revision)
    }

    pub(in crate::native_runner) fn pending_write_payload(&self) -> Option<Arc<Value>> {
        self.pending_write
            .as_ref()
            .and_then(|write| write.payload.as_ref().cloned())
    }

    pub(in crate::native_runner) fn acknowledge_write(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
        succeeded: bool,
        current_revision: u64,
    ) -> Option<DefaultWriteCompletion> {
        let revision = revision?;
        let write = self.pending_write.as_mut()?;
        if write.revision != revision || write.request_id.as_deref() != Some(request_id) {
            return None;
        }
        let write = self.pending_write.take()?;
        let payload_missing = succeeded && write.payload.is_none();
        let succeeded = succeeded && !payload_missing;
        let show_saved_feedback =
            succeeded && (!write.native_origin || write.revision == current_revision);
        let restart_flow = write.scope.is_restart() && self.is_saving();
        let host_role = write.payload.as_deref().is_some_and(super::payload_is_host);
        let saved_patch_payload = (succeeded && write.scope.is_patch())
            .then(|| write.payload.as_ref().map(Arc::clone))
            .flatten();
        if succeeded && write.scope.is_restart() {
            self.persisted_default = write.payload.expect("successful System write has payload");
        }
        if payload_missing {
            self.native_payload_missing = true;
        }
        if restart_flow {
            self.flow = if succeeded {
                super::RestartFlow::RestartChoice
            } else {
                super::RestartFlow::Idle
            };
        }
        Some(DefaultWriteCompletion {
            revision: write.revision,
            scope: write.scope,
            captured_patch_dirty_revision: write.captured_patch_dirty_revision,
            restart_flow,
            succeeded,
            host_role,
            show_saved_feedback,
            saved_patch_payload,
        })
    }

    pub(in crate::native_runner) fn acknowledge_request(
        &mut self,
        request_id: &str,
        request_revision: Option<u64>,
        succeeded: bool,
        current_revision: u64,
    ) -> Option<DefaultWriteCompletion> {
        let write = self.pending_write.as_ref()?;
        if write.request_id.as_deref() != Some(request_id)
            || write.request_revision != request_revision
        {
            return None;
        }
        self.acknowledge_write(
            request_id,
            Some(write.revision),
            succeeded,
            current_revision,
        )
    }
}
