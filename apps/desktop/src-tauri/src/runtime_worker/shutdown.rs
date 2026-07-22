use super::RuntimeWorker;

impl RuntimeWorker {
    pub(super) fn flush_pending_host_work_now(&mut self) -> Result<(), String> {
        let runner_messages = self.runner.flush_pending_deferred_work_now()?;
        if !runner_messages.is_empty() {
            let output = self.playback.dispatch_runner_messages(
                runner_messages,
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.emit_runtime_output(output)?;
        }
        if let Err(error) = self.adapter.flush_pending_default_save_now() {
            self.handle_persistence_error(error);
        }
        Ok(())
    }
}
