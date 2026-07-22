use std::time::Instant;

use super::NativeRunner;

impl NativeRunner {
    pub fn flush_pending_deferred_work_now(
        &mut self,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        if let Some(pending) = &mut self.pending.pending_menu_apply {
            pending.due_at = Instant::now();
        }
        if self.pending.pending_autosave_payload_due_at.is_some() {
            self.pending.pending_autosave_payload_due_at = Some(Instant::now());
        }
        self.flush_deferred_menu_apply()
    }
}
