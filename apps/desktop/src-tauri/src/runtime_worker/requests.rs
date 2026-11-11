use super::WorkerCommand;
use playback_runtime::{
    HostMessage, RunnerMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorMetadata,
    RuntimeOperation, RuntimeRecovery, RuntimeStatus, RuntimeStatusState, RuntimeTransportState,
    SyncSource,
};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

/// Shorter than the UI's own IPC timeout, so the UI hears why instead of timing out.
pub(crate) const DISPATCH_REPLY_TIMEOUT: Duration = Duration::from_secs(3);

/// A worker that is gone answers with a runtime error status, which the UI shows like any other.
pub(crate) fn request_worker_dispatch(
    worker_tx: &Sender<WorkerCommand>,
    message: HostMessage,
    timeout: Duration,
) -> Result<Vec<RunnerMessage>, String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    if worker_tx
        .send(WorkerCommand::Dispatch(message, reply_tx))
        .is_err()
    {
        return Ok(vec![worker_stopped_status()]);
    }
    match reply_rx.recv_timeout(timeout) {
        Ok(reply) => reply,
        Err(RecvTimeoutError::Disconnected) => Ok(vec![worker_stopped_status()]),
        Err(RecvTimeoutError::Timeout) => Err(format!(
            "runtime worker did not answer within {} s",
            timeout.as_secs()
        )),
    }
}

/// Long enough to write the last save and finish a recording, short enough to never hang the exit.
pub(crate) const EXIT_PREPARATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether the worker finished its exit work in time.
pub(crate) fn request_worker_exit_preparation(
    worker_tx: &Sender<WorkerCommand>,
    timeout: Duration,
) -> bool {
    let (done_tx, done_rx) = mpsc::channel();
    worker_tx.send(WorkerCommand::PrepareExit(done_tx)).is_ok()
        && done_rx.recv_timeout(timeout).is_ok()
}

fn worker_stopped_status() -> RunnerMessage {
    let message = "The runtime stopped. Please restart Octessera.".to_string();
    RunnerMessage::RuntimeStatus {
        status: RuntimeStatus {
            state: RuntimeStatusState::Error,
            transport: RuntimeTransportState::Stopped,
            current_ppqn_pulse: 0,
            pending_resync: false,
            sync_source: SyncSource::Internal,
            message: Some(message.clone()),
            error: Some(RuntimeErrorMetadata::new(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::RuntimeDispatch,
                RuntimeRecovery::StopAndSilence,
                Some(message),
            )),
        },
    }
}

#[cfg(test)]
mod tests;
