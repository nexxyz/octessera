use super::{
    encode_runtime_responses, retain_runtime_outbox_batch, RuntimeMessagesPayload, RuntimeWorker,
    RUNTIME_MESSAGES_EVENT,
};
use playback_runtime::{
    RunnerMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeIngest,
    RuntimeOperation,
};
use std::time::Duration;
#[cfg(debug_assertions)]
use std::time::Instant;
use tauri::Emitter;

impl RuntimeWorker {
    pub(super) fn handle_error(&mut self, err: String) {
        self.handle_typed_error(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Runtime,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::RuntimeDispatch,
            Some(err),
        ));
    }

    pub(super) fn handle_typed_error(&mut self, error: RuntimeErrorFacts) {
        match self
            .playback
            .recover_from_facts(error, &mut self.runner, &mut self.adapter)
        {
            Ok(output) => {
                if let Err(error) = self.emit_runtime_output(output) {
                    eprintln!("failed to emit runtime recovery: {error}");
                }
            }
            Err(error) => eprintln!("runtime recovery failed: {error}"),
        }
    }

    pub(super) fn handle_emission_error(&mut self, error: String) {
        self.playback.latch_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Runtime,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::RuntimeEmission,
            Some(error),
        ));
    }

    pub(super) fn handle_persistence_error(&mut self, error: String) {
        self.playback.latch_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Storage,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::Persistence,
            Some(error),
        ));
    }

    pub(super) fn emit_runtime_output(&mut self, output: RuntimeIngest) -> Result<(), String> {
        if let Err(error) = self.emit_runner_messages(output.messages) {
            self.handle_emission_error(error);
            return Ok(());
        }
        for follow_up in output.follow_ups {
            let output = self.playback.dispatch(
                playback_runtime::RuntimeDispatchInput::HostMessage(follow_up),
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.emit_runtime_output(output)?;
        }
        Ok(())
    }

    pub(super) fn poll_audio_failures(&mut self) {
        while let Ok(failure) = self.audio_failure_rx.try_recv() {
            self.handle_typed_error(failure.facts);
        }
    }

    pub(super) fn emit_runner_messages(
        &mut self,
        responses: Vec<RunnerMessage>,
    ) -> Result<(), String> {
        #[cfg(debug_assertions)]
        let started_at = Instant::now();
        let values = encode_runtime_responses(responses)?;
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
        let outbox_lock_failed = {
            match self.runtime_outbox.lock() {
                Ok(mut guard) => {
                    retain_runtime_outbox_batch(&mut guard, payload.clone());
                    false
                }
                Err(_) => true,
            }
        };
        if outbox_lock_failed {
            self.handle_persistence_error("runtime outbox lock failed".into());
        }
        self.app_handle
            .emit(RUNTIME_MESSAGES_EVENT, payload)
            .map_err(|e| format!("failed to emit runtime messages: {e}"))?;
        self.playback
            .clear_error(playback_runtime::RuntimeOperation::RuntimeEmission);
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
