use super::RuntimeWorker;

impl RuntimeWorker {
    pub(super) fn flush_pending_host_work_now(&mut self) -> Result<(), String> {
        let runner_messages = self.runner.flush_pending_deferred_work_now()?;
        if !runner_messages.is_empty() {
            let follow_ups = self
                .playback
                .ingest_runner_messages(runner_messages, &mut self.adapter)?;
            self.dispatch_follow_ups(follow_ups)?;
        }
        self.adapter.flush_pending_default_save_now()
    }
}
