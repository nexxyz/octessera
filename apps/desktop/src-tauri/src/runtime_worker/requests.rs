use super::WorkerCommand;
use playback_runtime::{HostMessage, RunnerMessage};
use std::sync::mpsc;

pub(crate) fn request_worker_dispatch(
    state: &crate::AppState,
    message: HostMessage,
) -> Result<Vec<RunnerMessage>, String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    state
        .worker_tx
        .send(WorkerCommand::Dispatch(message, reply_tx))
        .map_err(|e| format!("runtime worker unavailable: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| format!("runtime worker reply unavailable: {e}"))?
}

pub(crate) fn request_worker_audio_command(
    state: &crate::AppState,
    command: playback_runtime::RuntimeAudioCommand,
) -> Result<(), String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    state
        .worker_tx
        .send(WorkerCommand::DirectAudio(command, reply_tx))
        .map_err(|e| format!("runtime worker unavailable: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| format!("runtime worker reply unavailable: {e}"))?
}
