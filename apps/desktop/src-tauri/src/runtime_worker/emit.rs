use super::{
    append_audio_error_values, encode_runtime_responses, retain_runtime_outbox_batch,
    RuntimeMessagesPayload, RuntimeWorker, RUNTIME_MESSAGES_EVENT,
};
use playback_runtime::{NativeRunner, NativeRunnerConfig, RunnerMessage};
use std::time::{Duration, Instant};
use tauri::Emitter;

impl RuntimeWorker {
    pub(super) fn handle_error(&mut self, err: String) {
        if let Ok(runner) = NativeRunner::new(NativeRunnerConfig::default()) {
            self.runner = runner;
            self.runner.apply_runtime_config(self.playback.config());
        }
        if let Ok(mut guard) = self.audio_error.lock() {
            *guard = Some(err);
        }
        let _ = self.emit_runner_messages(Vec::new());
    }

    pub(super) fn emit_runner_messages(
        &mut self,
        responses: Vec<RunnerMessage>,
    ) -> Result<(), String> {
        #[cfg(debug_assertions)]
        let started_at = Instant::now();
        let values =
            append_audio_error_values(encode_runtime_responses(responses)?, &self.audio_error);
        if values.is_empty() {
            self.maybe_exit_after_shutdown_request();
            #[cfg(debug_assertions)]
            self.perf.record_emit(started_at.elapsed());
            return Ok(());
        }
        self.next_runtime_seq = self.next_runtime_seq.saturating_add(1);
        let payload = RuntimeMessagesPayload {
            seq: self.next_runtime_seq,
            messages: values,
        };
        if let Ok(mut guard) = self.runtime_outbox.lock() {
            retain_runtime_outbox_batch(&mut guard, payload.clone());
        }
        self.app_handle
            .emit(RUNTIME_MESSAGES_EVENT, payload)
            .map_err(|e| format!("failed to emit runtime messages: {e}"))?;
        self.maybe_exit_after_shutdown_request();
        #[cfg(debug_assertions)]
        self.perf.record_emit(started_at.elapsed());
        Ok(())
    }

    fn maybe_exit_after_shutdown_request(&mut self) {
        if !self.adapter.take_shutdown_request() {
            return;
        }
        let app_handle = self.app_handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            app_handle.exit(0);
        });
    }
}
