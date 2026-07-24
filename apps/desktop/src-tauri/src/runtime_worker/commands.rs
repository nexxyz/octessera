use super::{queue_by_priority, RuntimeWorker, WorkerCommand, MAX_COMMANDS_PER_WAKE};
use playback_runtime::SyncSource;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

impl RuntimeWorker {
    pub(super) fn is_internal_playing(&self) -> bool {
        self.playback.config().sync_source == SyncSource::Internal
            && self.playback.last_status().is_some_and(|status| {
                status.transport == playback_runtime::RuntimeTransportState::Playing
            })
    }

    pub(super) fn handle_command_batch(
        &mut self,
        first: WorkerCommand,
        rx: &Receiver<WorkerCommand>,
    ) -> Result<(), String> {
        let mut realtime = Vec::new();
        let mut normal = std::collections::VecDeque::new();
        queue_by_priority(first, &mut realtime, &mut normal);
        for _ in 1..MAX_COMMANDS_PER_WAKE {
            match rx.try_recv() {
                Ok(command) => queue_by_priority(command, &mut realtime, &mut normal),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        for command in realtime {
            self.handle_command(command)?;
        }
        while let Some(command) = normal.pop_front() {
            if self.is_internal_playing() {
                self.maybe_advance()?;
            }
            self.handle_command(command)?;
        }
        if self.is_internal_playing() {
            self.maybe_advance()?;
        }
        Ok(())
    }

    pub(super) fn handle_ready_commands(
        &mut self,
        rx: &Receiver<WorkerCommand>,
    ) -> Result<(), String> {
        let Ok(first) = rx.try_recv() else {
            return Ok(());
        };
        self.handle_command_batch(first, rx)
    }

    fn handle_command(&mut self, command: WorkerCommand) -> Result<(), String> {
        #[cfg(debug_assertions)]
        let started_at = Instant::now();
        let was_internal_playing = self.is_internal_playing();
        match command {
            WorkerCommand::Dispatch(message, reply) => {
                let result = self
                    .playback
                    .dispatch(
                        playback_runtime::RuntimeDispatchInput::HostMessage(
                            self.prepare_dispatch_message(message),
                        ),
                        &mut self.runner,
                        &mut self.adapter,
                    )
                    .map(|output| output.messages);
                if let Err(err) = &result {
                    let _ = reply.send(Err(err.clone()));
                    return Err(err.clone());
                }
                let _ = reply.send(result);
            }
            WorkerCommand::NativeMidiRealtime(bytes) => {
                let output = self.handle_midi_realtime(bytes)?;
                self.emit_runtime_output(output)?;
            }
            WorkerCommand::DirectAudio(command, reply) => {
                let output = self.playback.dispatch_runner_messages(
                    vec![playback_runtime::RunnerMessage::AudioCommands {
                        commands: vec![command],
                    }],
                    &mut self.runner,
                    &mut self.adapter,
                )?;
                self.emit_runtime_output(output)?;
                let _ = reply.send(Ok(()));
            }
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
