use super::RuntimeWorker;

impl RuntimeWorker {
    /// Applies deferred menu work, writes the pending default save and finishes a recording.
    pub(super) fn prepare_for_exit(&mut self) -> Result<(), String> {
        let runner_messages = self.runner.flush_pending_deferred_work_now()?;
        if !runner_messages.is_empty() {
            let output = self.playback.dispatch_runner_messages(
                runner_messages,
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.emit_runtime_output(output)?;
        }
        if let Err(error) = self.adapter.finish_for_exit() {
            self.handle_persistence_error(error);
        }
        Ok(())
    }
}
