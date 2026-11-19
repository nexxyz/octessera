use super::{BoardWorker, DesiredLedOutputs, SeesawCommand, SeesawIo, ShutdownRequest};
use octessera_hal::{NeoKey, NeoTrellis, SeesawInterrupt};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

pub(super) const SEESAW_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(750);
pub(super) const SEESAW_STARTUP_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SeesawStartupStatus {
    WaitingForInputScan,
    WaitingForOutputFrame,
    Ready,
}

pub(super) fn seesaw_startup_status(
    scan_succeeded: bool,
    output_succeeded: bool,
) -> SeesawStartupStatus {
    if !scan_succeeded {
        SeesawStartupStatus::WaitingForInputScan
    } else if !output_succeeded {
        SeesawStartupStatus::WaitingForOutputFrame
    } else {
        SeesawStartupStatus::Ready
    }
}

impl DesiredLedOutputs {
    pub(super) fn startup_complete(&self) -> bool {
        self.applied_grid == Some([[0; 3]; 64])
            && self.applied_key_valid == [true; 4]
            && self.applied_keys == [[0; 3]; 4]
    }

    fn shutdown_complete(&self) -> bool {
        self.startup_complete()
    }
}

struct OrangeWorker {
    startup_scan_succeeded: bool,
    startup_tx: Option<Sender<Result<(), String>>>,
    shutdown_ack: Option<ShutdownRequest>,
}

impl BoardWorker for OrangeWorker {
    fn start(&mut self, outputs: &mut DesiredLedOutputs) {
        outputs.accept(SeesawCommand::GridFrame([[0; 3]; 64]));
        outputs.accept(SeesawCommand::NeoKeyColors([[0; 3]; 4]));
    }

    fn record_scan(&mut self, succeeded: bool) {
        self.startup_scan_succeeded = succeeded;
    }

    fn finish_pass(
        &mut self,
        shutdown: Option<ShutdownRequest>,
        outputs: &DesiredLedOutputs,
    ) -> bool {
        self.shutdown_ack = shutdown.or(self.shutdown_ack.take());
        if let Some((ack, deadline)) = self.shutdown_ack.take() {
            if outputs.shutdown_complete() {
                let _ = ack.send(Ok(()));
                return true;
            }
            if Instant::now() >= deadline {
                let _ = ack.send(Err("Seesaw black-frame cleanup timed out".into()));
                return true;
            }
            self.shutdown_ack = Some((ack, deadline));
        }

        if self.startup_tx.is_some()
            && seesaw_startup_status(self.startup_scan_succeeded, outputs.startup_complete())
                == SeesawStartupStatus::Ready
        {
            let ack = self
                .startup_tx
                .take()
                .expect("Seesaw startup sender is present");
            if ack.send(Ok(())).is_err() {
                return true;
            }
        }
        false
    }
}

pub(crate) fn spawn_interrupt(
    trellis: NeoTrellis,
    neokey: NeoKey,
    interrupt: SeesawInterrupt,
) -> Result<SeesawIo, String> {
    let (startup_tx, startup_rx) = mpsc::channel();
    let seesaw = super::spawn(
        trellis,
        neokey,
        interrupt,
        OrangeWorker {
            startup_scan_succeeded: false,
            startup_tx: Some(startup_tx),
            shutdown_ack: None,
        },
    );
    match startup_rx.recv_timeout(SEESAW_STARTUP_TIMEOUT) {
        Ok(result) => result.map(|_| seesaw),
        Err(error) => {
            let _ = seesaw.shutdown();
            Err(format!("Seesaw startup handshake timed out: {error}"))
        }
    }
}

impl SeesawIo {
    pub(crate) fn shutdown(self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        let deadline = Instant::now() + SEESAW_SHUTDOWN_TIMEOUT;
        self.command_tx
            .send(SeesawCommand::Shutdown {
                ack: ack_tx,
                deadline,
            })
            .map_err(|error| format!("Seesaw shutdown command failed: {error}"))?;
        let ack_result = ack_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| format!("Seesaw shutdown acknowledgement failed: {error}"))?;
        let worker = self
            .worker
            .lock()
            .map_err(|_| "Seesaw worker mutex poisoned".to_string())?
            .take();
        if let Some(worker) = worker {
            worker
                .join()
                .map_err(|_| "Seesaw worker panicked during shutdown".to_string())?;
        }
        ack_result
    }
}
