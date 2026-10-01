use crate::protocol::RuntimePlatformRequest;

use super::{NativeRunner, RuntimeUserDataRestorePhase, RuntimeUserDataRestoreStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeUserDataRestoreState {
    pub(super) status: RuntimeUserDataRestoreStatus,
    pub(super) request_id: Option<String>,
    pub(super) revision: Option<u64>,
    pub(super) rehydration_pending: bool,
    pub(super) patch_request_id: Option<String>,
    pub(super) patch_revision: Option<u64>,
    pub(super) patch_result_handled: bool,
}

impl NativeRunner {
    pub(super) fn apply_user_data_restore_status(
        &mut self,
        status: RuntimeUserDataRestoreStatus,
        request_id: Option<String>,
        revision: Option<u64>,
    ) {
        if let Some(current) = self.display.user_data_restore.as_ref() {
            if current.rehydration_pending && current.request_id.as_deref() != request_id.as_deref()
            {
                return;
            }
            if matches!((current.revision, revision), (Some(old), Some(new)) if new < old) {
                return;
            }
            if current.request_id == request_id
                && restore_phase_rank(&status.phase) < restore_phase_rank(&current.status.phase)
            {
                return;
            }
            if current.request_id == request_id
                && matches!(
                    (&current.status.phase, &status.phase),
                    (
                        RuntimeUserDataRestorePhase::Succeeded
                            | RuntimeUserDataRestorePhase::Failed,
                        RuntimeUserDataRestorePhase::Succeeded
                            | RuntimeUserDataRestorePhase::Failed
                    )
                )
            {
                return;
            }
        }
        if matches!(
            status.phase,
            RuntimeUserDataRestorePhase::Restoring
                | RuntimeUserDataRestorePhase::Succeeded
                | RuntimeUserDataRestorePhase::Failed
        ) {
            self.abandon_pending_default_write();
        }
        if matches!(
            status.phase,
            RuntimeUserDataRestorePhase::Restoring | RuntimeUserDataRestorePhase::Succeeded
        ) {
            self.pending.pending_default_load_request = None;
        }
        let rehydration_pending = status.phase == RuntimeUserDataRestorePhase::Succeeded;
        if status.phase == RuntimeUserDataRestorePhase::Failed {
            self.abandon_pending_default_write();
        }
        self.display.user_data_restore = Some(NativeUserDataRestoreState {
            status,
            request_id,
            revision,
            rehydration_pending,
            patch_request_id: None,
            patch_revision: None,
            patch_result_handled: false,
        });
        if rehydration_pending {
            self.outbox
                .push_platform_effect(super::RuntimePlatformEffect::StoreLoadSystem);
        }
    }

    pub(super) fn user_data_restore_is_active(&self) -> bool {
        self.display
            .user_data_restore
            .as_ref()
            .is_some_and(|restore| {
                restore.status.phase == RuntimeUserDataRestorePhase::Restoring
                    || restore.rehydration_pending
            })
    }

    pub(super) fn restore_rehydration_pending(&self) -> bool {
        self.display
            .user_data_restore
            .as_ref()
            .is_some_and(|restore| restore.rehydration_pending)
    }

    pub(super) fn register_restore_patch_request(&mut self, request: &RuntimePlatformRequest) {
        let Some(restore) = self.display.user_data_restore.as_mut() else {
            return;
        };
        if restore.status.phase != RuntimeUserDataRestorePhase::Succeeded
            || !restore.rehydration_pending
            || restore.patch_request_id.is_some()
            || request.operation() != crate::RuntimeOperation::StoreLoadDefault
        {
            return;
        }
        restore.patch_request_id = Some(request.request_id.clone());
        restore.patch_revision = request.revision;
    }

    pub(super) fn routes_restore_patch_result(
        &self,
        request_id: Option<&str>,
        revision: Option<u64>,
    ) -> bool {
        let Some(restore) = self.display.user_data_restore.as_ref() else {
            return false;
        };
        if restore.rehydration_pending {
            return true;
        }
        let Some(child_request_id) = restore.patch_request_id.as_deref() else {
            return false;
        };
        if request_id.is_none() || request_id == Some(child_request_id) {
            return true;
        }
        !self
            .pending
            .pending_default_load_request
            .as_ref()
            .is_some_and(|(expected_id, expected_revision)| {
                request_id == Some(expected_id.as_str()) && revision == *expected_revision
            })
    }

    pub(super) fn accept_restore_patch_request(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
    ) -> bool {
        let Some(restore) = self.display.user_data_restore.as_mut() else {
            return false;
        };
        if !restore.rehydration_pending
            || restore.patch_result_handled
            || restore.patch_request_id.as_deref() != Some(request_id)
            || restore.patch_revision != revision
        {
            return false;
        }
        restore.patch_result_handled = true;
        true
    }

    pub(super) fn restore_blocks_config_writes(&self) -> bool {
        self.user_data_restore_is_active() || self.restore_rehydration_pending()
    }

    pub(super) fn finish_restore_rehydration(&mut self) {
        {
            let Some(restore) = self.display.user_data_restore.as_mut() else {
                return;
            };
            restore.rehydration_pending = false;
        }
    }

    pub(super) fn fail_restore_rehydration(&mut self) {
        {
            let Some(restore) = self.display.user_data_restore.as_mut() else {
                return;
            };
            if !restore.rehydration_pending {
                return;
            }
            restore.rehydration_pending = false;
            restore.status.phase = RuntimeUserDataRestorePhase::Failed;
        }
        self.abandon_pending_default_write();
    }

    pub(super) fn fail_restore_patch_rehydration(&mut self) {
        {
            let Some(restore) = self.display.user_data_restore.as_mut() else {
                return;
            };
            if !restore.rehydration_pending {
                return;
            }
            restore.status.phase = RuntimeUserDataRestorePhase::Failed;
        }
        self.abandon_pending_default_write();
    }
}

fn restore_phase_rank(phase: &RuntimeUserDataRestorePhase) -> u8 {
    match phase {
        RuntimeUserDataRestorePhase::Restoring => 0,
        RuntimeUserDataRestorePhase::Succeeded | RuntimeUserDataRestorePhase::Failed => 1,
    }
}
