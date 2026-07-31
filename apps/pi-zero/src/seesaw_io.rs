use crate::input::{grid_message, neokey_message};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use octessera_hal::SeesawInterrupt;
use octessera_hal::{NeoKey, NeoTrellis};
use playback_runtime::HostMessage;
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const INPUT_SERVICE_INTERVAL: Duration = Duration::from_millis(4);
#[cfg(feature = "hardware-orange-pi-zero-2w")]
const INPUT_SERVICE_INTERVAL: Duration = Duration::from_millis(10);
const OUTPUT_RETRY_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(feature = "hardware-orange-pi-zero-2w")]
const SEESAW_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(750);
#[cfg(feature = "hardware-orange-pi-zero-2w")]
const SEESAW_STARTUP_TIMEOUT: Duration = Duration::from_millis(750);

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeesawStartupStatus {
    WaitingForInputScan,
    WaitingForOutputFrame,
    Ready,
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn seesaw_startup_status(scan_succeeded: bool, output_succeeded: bool) -> SeesawStartupStatus {
    if !scan_succeeded {
        SeesawStartupStatus::WaitingForInputScan
    } else if !output_succeeded {
        SeesawStartupStatus::WaitingForOutputFrame
    } else {
        SeesawStartupStatus::Ready
    }
}

#[derive(Clone)]
pub(crate) enum SeesawCommand {
    GridFrame([[u8; 3]; 64]),
    NeoKeyColors([[u8; 3]; 4]),
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    Shutdown {
        ack: Sender<Result<(), String>>,
        deadline: Instant,
    },
}

pub(crate) struct SeesawIo {
    pub(crate) input_rx: Receiver<HostMessage>,
    pub(crate) command_tx: Sender<SeesawCommand>,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    worker: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

trait LedOutputWriter {
    fn write_grid(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String>;
    fn write_key(&mut self, key: u8, color: [u8; 3]) -> Result<(), String>;
}

struct HardwareLedOutput<'a> {
    trellis: &'a mut NeoTrellis,
    neokey: &'a mut NeoKey,
}

impl LedOutputWriter for HardwareLedOutput<'_> {
    fn write_grid(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String> {
        self.trellis.write_led_frame(frame)
    }

    fn write_key(&mut self, key: u8, color: [u8; 3]) -> Result<(), String> {
        self.neokey.set_led(key, color[0], color[1], color[2])
    }
}

#[derive(Default)]
struct DesiredLedOutputs {
    desired_grid: Option<[[u8; 3]; 64]>,
    desired_keys: Option<[[u8; 3]; 4]>,
    applied_grid: Option<[[u8; 3]; 64]>,
    applied_keys: [[u8; 3]; 4],
    applied_key_valid: [bool; 4],
    next_grid_attempt_at: Option<Instant>,
    next_key_attempt_at: Option<Instant>,
}

impl DesiredLedOutputs {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    fn startup_complete(&self) -> bool {
        self.applied_grid == Some([[0; 3]; 64])
            && self.applied_key_valid == [true; 4]
            && self.applied_keys == [[0; 3]; 4]
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    fn shutdown_complete(&self) -> bool {
        self.startup_complete()
    }

    fn accept(&mut self, command: SeesawCommand) {
        match command {
            SeesawCommand::GridFrame(frame) => {
                if self.desired_grid.as_ref() != Some(&frame) {
                    self.desired_grid = Some(frame);
                    self.next_grid_attempt_at = None;
                }
            }
            SeesawCommand::NeoKeyColors(colors) => {
                if self.desired_keys.as_ref() != Some(&colors) {
                    self.desired_keys = Some(colors);
                    self.next_key_attempt_at = None;
                }
            }
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            SeesawCommand::Shutdown { .. } => {}
        }
    }
    fn apply_due<W: LedOutputWriter>(&mut self, writer: &mut W, now: Instant) {
        self.apply_grid_if_due(writer, now);
        self.apply_keys_if_due(writer, now);
    }

    fn apply_grid_if_due<W: LedOutputWriter>(&mut self, writer: &mut W, now: Instant) {
        let Some(frame) = self.desired_grid else {
            return;
        };
        if self.applied_grid == Some(frame) || !attempt_due(self.next_grid_attempt_at, now) {
            return;
        }
        match writer.write_grid(&frame) {
            Ok(()) => {
                self.applied_grid = Some(frame);
                self.next_grid_attempt_at = None;
            }
            Err(_) => {
                self.next_grid_attempt_at = Some(now + OUTPUT_RETRY_INTERVAL);
            }
        }
    }

    fn apply_keys_if_due<W: LedOutputWriter>(&mut self, writer: &mut W, now: Instant) {
        let Some(colors) = self.desired_keys else {
            return;
        };
        if !attempt_due(self.next_key_attempt_at, now) {
            return;
        }
        let mut failed = false;
        for (index, color) in colors.into_iter().enumerate() {
            if self.applied_key_valid[index] && self.applied_keys[index] == color {
                continue;
            }
            match writer.write_key(index as u8, color) {
                Ok(()) => {
                    self.applied_keys[index] = color;
                    self.applied_key_valid[index] = true;
                }
                Err(_) => failed = true,
            }
        }
        self.next_key_attempt_at = failed.then_some(now + OUTPUT_RETRY_INTERVAL);
    }
}

fn attempt_due(next_attempt_at: Option<Instant>, now: Instant) -> bool {
    next_attempt_at.is_none_or(|deadline| now >= deadline)
}

pub(crate) fn spawn(
    mut trellis: NeoTrellis,
    mut neokey: NeoKey,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))] interrupt: SeesawInterrupt,
    #[cfg(feature = "hardware-orange-pi-zero-2w")] startup_tx: Sender<Result<(), String>>,
) -> SeesawIo {
    let (input_tx, input_rx) = mpsc::channel::<HostMessage>();
    let (command_tx, command_rx) = mpsc::channel::<SeesawCommand>();
    let worker = thread::spawn(move || {
        let mut previous_neokey = [false; 4];
        let mut outputs = DesiredLedOutputs::default();
        let mut last_input_service = Instant::now() - INPUT_SERVICE_INTERVAL;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut startup_scan_succeeded = false;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut startup_tx = Some(startup_tx);
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        {
            outputs.accept(SeesawCommand::GridFrame([[0; 3]; 64]));
            outputs.accept(SeesawCommand::NeoKeyColors([[0; 3]; 4]));
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut shutdown_ack = None;
        loop {
            let service_due = last_input_service.elapsed() >= INPUT_SERVICE_INTERVAL;
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            let interrupt_pending = interrupt.pending();
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            let interrupt_pending = false;
            if service_due || interrupt_pending {
                #[cfg(feature = "hardware-orange-pi-zero-2w")]
                {
                    startup_scan_succeeded =
                        scan_inputs(&mut trellis, &mut neokey, &mut previous_neokey, &input_tx)
                            .is_ok();
                }
                #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                let _ = scan_inputs(&mut trellis, &mut neokey, &mut previous_neokey, &input_tx);
                last_input_service = Instant::now();
            }

            let mut output = HardwareLedOutput {
                trellis: &mut trellis,
                neokey: &mut neokey,
            };
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            {
                shutdown_ack =
                    drain_commands(&command_rx, &mut outputs, &mut output, Instant::now())
                        .or(shutdown_ack);
                if let Some((ack, deadline)) = shutdown_ack.take() {
                    if outputs.shutdown_complete() {
                        let _ = ack.send(Ok(()));
                        break;
                    }
                    if Instant::now() >= deadline {
                        let _ = ack.send(Err("Seesaw black-frame cleanup timed out".into()));
                        break;
                    }
                    shutdown_ack = Some((ack, deadline));
                }
            }
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            drain_commands(&command_rx, &mut outputs, &mut output, Instant::now());

            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            if startup_tx.is_some()
                && seesaw_startup_status(startup_scan_succeeded, outputs.startup_complete())
                    == SeesawStartupStatus::Ready
            {
                let ack = startup_tx.take().expect("Seesaw startup sender is present");
                if ack.send(Ok(())).is_err() {
                    break;
                }
            }

            thread::sleep(Duration::from_millis(2));
        }
    });
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let _ = worker;

    SeesawIo {
        input_rx,
        command_tx,
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        worker: Arc::new(Mutex::new(Some(worker))),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn spawn_polling(trellis: NeoTrellis, neokey: NeoKey) -> Result<SeesawIo, String> {
    let (startup_tx, startup_rx) = mpsc::channel();
    let seesaw = spawn(trellis, neokey, startup_tx);
    match startup_rx.recv_timeout(SEESAW_STARTUP_TIMEOUT) {
        Ok(result) => result.map(|_| seesaw),
        Err(error) => {
            let _ = seesaw.shutdown();
            Err(format!("Seesaw startup handshake timed out: {error}"))
        }
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn spawn_interrupt(
    trellis: NeoTrellis,
    neokey: NeoKey,
    interrupt: SeesawInterrupt,
) -> SeesawIo {
    spawn(trellis, neokey, interrupt)
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
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

fn drain_commands<W: LedOutputWriter>(
    command_rx: &Receiver<SeesawCommand>,
    outputs: &mut DesiredLedOutputs,
    writer: &mut W,
    now: Instant,
) -> Option<(Sender<Result<(), String>>, Instant)> {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let mut shutdown = None;
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let shutdown = None;
    for _ in 0..32 {
        let Ok(command) = command_rx.try_recv() else {
            break;
        };
        match command {
            SeesawCommand::GridFrame(frame) => outputs.accept(SeesawCommand::GridFrame(frame)),
            SeesawCommand::NeoKeyColors(colors) => {
                outputs.accept(SeesawCommand::NeoKeyColors(colors))
            }
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            SeesawCommand::Shutdown { ack, deadline } => {
                outputs.accept(SeesawCommand::GridFrame([[0; 3]; 64]));
                outputs.accept(SeesawCommand::NeoKeyColors([[0; 3]; 4]));
                shutdown = Some((ack, deadline));
            }
        }
    }
    outputs.apply_due(writer, now);
    shutdown
}

fn scan_inputs(
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    previous_neokey: &mut [bool; 4],
    input_tx: &Sender<HostMessage>,
) -> Result<(), String> {
    let presses = trellis
        .scan_keys()
        .map_err(|error| format!("NeoTrellis input scan failed: {error}"))?;
    {
        for (x, y, pressed) in presses {
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            crate::wake_trace::log_trellis_event(x, y, pressed);
            let _ = input_tx.send(grid_message(x, y, pressed));
        }
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let keys = neokey.scan_interrupts();
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let keys = neokey.scan();
    let keys = keys.map_err(|error| format!("NeoKey input scan failed: {error}"))?;
    {
        for (key, pressed) in keys {
            let index = usize::from(key.min(3));
            if previous_neokey[index] == pressed {
                continue;
            }
            previous_neokey[index] = pressed;
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            crate::wake_trace::log_neokey_transition(key, pressed);
            if let Some(message) = neokey_message(key, pressed) {
                let _ = input_tx.send(message);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "seesaw_io_tests.rs"]
mod tests;
