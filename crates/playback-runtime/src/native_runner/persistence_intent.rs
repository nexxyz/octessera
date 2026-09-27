use super::NativeRunner;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePersistenceIntent {
    revision: u64,
    default_eligible: bool,
    backup_eligible: bool,
}

impl NativePersistenceIntent {
    pub fn revision(self) -> u64 {
        self.revision
    }

    pub fn default_eligible(self) -> bool {
        self.default_eligible
    }

    pub fn backup_eligible(self) -> bool {
        self.backup_eligible
    }
}

impl NativeRunner {
    pub fn persistence_intent_at(&self, now: Instant) -> Option<NativePersistenceIntent> {
        let revision = self.dirty_revision.filter(|_| self.config_dirty)?;
        let restore_blocks_writes = self.restore_blocks_config_writes();
        let debounce_pending = self
            .pending
            .pending_autosave_payload_due_at
            .is_some_and(|due_at| due_at > now);
        if restore_blocks_writes || debounce_pending {
            return None;
        }

        let save_pending = self.restart_settings.has_pending_write()
            || self
                .pending
                .pending_save_revision
                .zip(self.dirty_revision)
                .is_some_and(|(pending, dirty)| pending == dirty);
        let default_eligible =
            self.auto_save_default && !save_pending && !self.restart_settings.is_editing();
        let backup_eligible = self.rolling_backups
            && self
                .last_backup_save_at
                .map(|last| now.saturating_duration_since(last) >= Duration::from_secs(300))
                .unwrap_or(true);
        (default_eligible || backup_eligible).then_some(NativePersistenceIntent {
            revision,
            default_eligible,
            backup_eligible,
        })
    }

    pub fn mark_native_backup_issued_at(&mut self, issued_at: Instant) {
        self.last_backup_save_at = Some(issued_at);
    }
}
