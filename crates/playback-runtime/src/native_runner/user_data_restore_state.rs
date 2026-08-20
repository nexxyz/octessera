use super::{
    NativeRunner, NativeUserDataRestoreState, RuntimeUserDataRestorePhase,
    RuntimeUserDataRestoreStatus,
};

impl NativeRunner {
    pub(super) fn apply_user_data_restore_status(
        &mut self,
        status: RuntimeUserDataRestoreStatus,
        request_id: Option<String>,
        revision: Option<u64>,
    ) {
        if let Some(current) = self.display.user_data_restore.as_ref() {
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
        let rehydration_pending = status.phase == RuntimeUserDataRestorePhase::Succeeded;
        if status.phase == RuntimeUserDataRestorePhase::Failed {
            self.retry_config_save_after_restore_failure();
        }
        self.display.user_data_restore = Some(NativeUserDataRestoreState {
            status,
            request_id,
            revision,
            rehydration_pending,
        });
        if rehydration_pending {
            self.outbox
                .push_platform_effect(super::RuntimePlatformEffect::StoreLoadDefault);
        }
    }

    pub(super) fn user_data_restore_is_active(&self) -> bool {
        self.display
            .user_data_restore
            .as_ref()
            .is_some_and(|restore| restore.status.phase == RuntimeUserDataRestorePhase::Restoring)
    }

    pub(super) fn restore_rehydration_pending(&self) -> bool {
        self.display
            .user_data_restore
            .as_ref()
            .is_some_and(|restore| restore.rehydration_pending)
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
        self.retry_config_save_after_restore_failure();
    }
}

fn restore_phase_rank(phase: &RuntimeUserDataRestorePhase) -> u8 {
    match phase {
        RuntimeUserDataRestorePhase::Restoring => 0,
        RuntimeUserDataRestorePhase::Succeeded | RuntimeUserDataRestorePhase::Failed => 1,
    }
}
