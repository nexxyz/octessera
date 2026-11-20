use super::{run_command_batch, BatchStep, DispatchReply, RuntimeWorker, WorkerCommand};
use playback_runtime::SyncSource;
use std::sync::mpsc::Receiver;
use std::time::Instant;

impl RuntimeWorker {
    pub(super) fn is_internal_playing(&self) -> bool {
        self.playback.config().sync_source == SyncSource::Internal
            && self.playback.last_status().is_some_and(|status| {
                status.transport == playback_runtime::RuntimeTransportState::Playing
            })
    }

    fn is_transport_playing(&self) -> bool {
        self.playback.last_status().is_some_and(|status| {
            status.transport == playback_runtime::RuntimeTransportState::Playing
        })
    }

    pub(super) fn handle_command_batch(
        &mut self,
        first: WorkerCommand,
        rx: &Receiver<WorkerCommand>,
    ) {
        let errors = run_command_batch(first, rx, |step| match step {
            BatchStep::Command(command) => self.handle_command(command),
            BatchStep::Advance if self.is_internal_playing() => self.maybe_advance(),
            BatchStep::Advance => Ok(()),
        });
        for error in errors {
            self.handle_error(error);
        }
    }

    pub(super) fn handle_ready_commands(&mut self, rx: &Receiver<WorkerCommand>) {
        if let Ok(first) = rx.try_recv() {
            self.handle_command_batch(first, rx);
        }
    }

    fn handle_command(&mut self, command: WorkerCommand) -> Result<(), String> {
        #[cfg(debug_assertions)]
        let started_at = Instant::now();
        let was_internal_playing = self.is_internal_playing();
        let was_playing = self.is_transport_playing();
        match command {
            WorkerCommand::Dispatch(message, reply) => {
                let output = self.playback.dispatch(
                    playback_runtime::RuntimeDispatchInput::HostMessage(message),
                    &mut self.runner,
                    &mut self.adapter,
                );
                if output.is_ok() {
                    self.observe_accepted_snapshot_revision();
                }
                let result = output.map(|output| DispatchReply {
                    seq: self.next_runtime_seq,
                    messages: output.messages,
                });
                if let Err(err) = &result {
                    let _ = reply.send(Err(err.clone()));
                    return Err(err.clone());
                }
                let _ = reply.send(result);
            }
            WorkerCommand::NativeMidiRealtime(bytes, received_at) => {
                let output = self.handle_midi_realtime(bytes, received_at)?;
                self.emit_runtime_output(output)?;
            }
            WorkerCommand::NativeMidiControlChange(change) => {
                let output = self.playback.dispatch(
                    playback_runtime::RuntimeDispatchInput::HostMessage(change.into()),
                    &mut self.runner,
                    &mut self.adapter,
                )?;
                self.emit_runtime_output(output)?;
            }
            WorkerCommand::NativeMidiNote(note) => {
                let output = self.playback.dispatch(
                    playback_runtime::RuntimeDispatchInput::HostMessage(note.into()),
                    &mut self.runner,
                    &mut self.adapter,
                )?;
                self.emit_runtime_output(output)?;
            }
            WorkerCommand::PresentationMetrics(metrics) => {
                let output = self
                    .playback
                    .update_presentation_metrics(metrics, Instant::now());
                self.emit_runtime_output(output)?;
            }
            WorkerCommand::PrepareExit(done) => {
                let result = self.prepare_for_exit();
                let _ = done.send(());
                result?;
            }
        }
        if !was_playing && self.is_transport_playing() {
            self.adapter.audio.request_output_reopen();
        }
        let is_internal_playing = self.is_internal_playing();
        if was_internal_playing != is_internal_playing || !is_internal_playing {
            self.last_advance_at = Instant::now();
        }
        self.last_ui_refresh_at = Instant::now();
        #[cfg(debug_assertions)]
        self.perf.record_command(started_at.elapsed());
        Ok(())
    }
}
