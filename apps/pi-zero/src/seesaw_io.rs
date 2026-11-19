use crate::input::{grid_message, neokey_message};
use octessera_hal::{NeoKey, NeoTrellis, SeesawInterrupt};
use playback_runtime::HostMessage;
use scan_schedule::ScanSchedule;
use setup_refresh::{SeesawBoard, SetupRefresh};
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use orange_worker::spawn_interrupt;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use raspberry_worker::spawn_interrupt;

/// After a failed Seesaw transaction, leave the bus alone instead of retrying into a busy device.
const I2C_ERROR_BACKOFF: Duration = Duration::from_millis(50);

type ShutdownRequest = (Sender<Result<(), String>>, Instant);

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
    pub(crate) input_tx: Sender<HostMessage>,
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
    /// Takes a command into the desired outputs; a shutdown asks for black
    /// outputs and is handed back to the caller to acknowledge.
    fn accept(&mut self, command: SeesawCommand) -> Option<ShutdownRequest> {
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
            SeesawCommand::Shutdown { ack, deadline } => {
                self.accept(SeesawCommand::GridFrame([[0; 3]; 64]));
                self.accept(SeesawCommand::NeoKeyColors([[0; 3]; 4]));
                return Some((ack, deadline));
            }
        }
        None
    }

    fn resend_after_setup(&mut self, grid: bool, keys: bool) {
        if grid {
            self.applied_grid = None;
        }
        if keys {
            self.applied_key_valid = [false; 4];
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
            Err(error) => {
                eprintln!("{error}");
                self.next_grid_attempt_at = Some(now + I2C_ERROR_BACKOFF);
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
                Err(error) => {
                    eprintln!("{error}");
                    failed = true;
                }
            }
        }
        self.next_key_attempt_at = failed.then_some(now + I2C_ERROR_BACKOFF);
    }
}

fn attempt_due(next_attempt_at: Option<Instant>, now: Instant) -> bool {
    next_attempt_at.is_none_or(|deadline| now >= deadline)
}

/// The per-board parts of the Seesaw worker: what runs before the first pass,
/// what a finished input scan records, and when the worker stops.
trait BoardWorker: Send + 'static {
    fn start(&mut self, outputs: &mut DesiredLedOutputs);
    fn record_scan(&mut self, succeeded: bool);
    /// Returns true when the worker should stop.
    fn finish_pass(
        &mut self,
        shutdown: Option<ShutdownRequest>,
        outputs: &DesiredLedOutputs,
    ) -> bool;
}

fn spawn(
    mut trellis: NeoTrellis,
    mut neokey: NeoKey,
    interrupt: SeesawInterrupt,
    mut board: impl BoardWorker,
) -> SeesawIo {
    let (input_tx, input_rx) = mpsc::channel::<HostMessage>();
    let (command_tx, command_rx) = mpsc::channel::<SeesawCommand>();
    let worker_input_tx = input_tx.clone();
    let worker = thread::spawn(move || {
        let mut previous_neokey = [false; 4];
        let mut outputs = DesiredLedOutputs::default();
        let mut schedule = ScanSchedule::new(Instant::now());
        let mut setup_refresh = SetupRefresh::new(Instant::now());
        board.start(&mut outputs);
        loop {
            let now = Instant::now();
            if schedule.scan_due(now, interrupt.pending(), interrupt.asserted()) {
                let scan = scan_inputs(
                    &mut trellis,
                    &mut neokey,
                    &mut previous_neokey,
                    &worker_input_tx,
                )
                .inspect_err(|error| eprintln!("{error}"));
                board.record_scan(scan.is_ok());
                let found_input = scan.as_ref().is_ok_and(|found| *found);
                schedule.record_scan(scan.is_ok(), found_input, neokey.settling(), Instant::now());
                outputs.resend_after_setup(trellis.take_led_refresh(), neokey.take_led_refresh());
            } else if !schedule.backing_off() {
                if let Some(due) = setup_refresh.take_due(now) {
                    let more = reapply_setup_part(&mut trellis, &mut neokey, due);
                    setup_refresh.record_part(more, Instant::now());
                }
            }

            let mut output = HardwareLedOutput {
                trellis: &mut trellis,
                neokey: &mut neokey,
            };
            let shutdown = drain_commands(&command_rx, &mut outputs, &mut output, Instant::now());
            if board.finish_pass(shutdown, &outputs) {
                break;
            }

            thread::sleep(Duration::from_millis(2));
        }
    });
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let _ = worker;

    SeesawIo {
        input_rx,
        input_tx,
        command_tx,
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        worker: Arc::new(Mutex::new(Some(worker))),
    }
}

fn drain_commands<W: LedOutputWriter>(
    command_rx: &Receiver<SeesawCommand>,
    outputs: &mut DesiredLedOutputs,
    writer: &mut W,
    now: Instant,
) -> Option<ShutdownRequest> {
    let mut shutdown = None;
    for _ in 0..32 {
        let Ok(command) = command_rx.try_recv() else {
            break;
        };
        shutdown = outputs.accept(command).or(shutdown);
    }
    outputs.apply_due(writer, now);
    shutdown
}

fn reapply_setup_part(
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    (board, part): (SeesawBoard, usize),
) -> bool {
    let result = match board {
        SeesawBoard::Trellis(index) => trellis
            .reapply_setup_part(index, part)
            .map_err(|error| format!("NeoTrellis setup refresh failed: {error}")),
        SeesawBoard::NeoKey => neokey
            .reapply_setup_part(part)
            .map_err(|error| format!("NeoKey setup refresh failed: {error}")),
    };
    result.unwrap_or_else(|error| {
        eprintln!("{error}");
        false
    })
}

fn scan_inputs(
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    previous_neokey: &mut [bool; 4],
    input_tx: &Sender<HostMessage>,
) -> Result<bool, String> {
    let mut presses = Vec::new();
    let trellis_result = trellis.scan_keys(&mut presses);
    let mut found_input = !presses.is_empty();
    for (x, y, pressed) in presses {
        let _ = input_tx.send(grid_message(x, y, pressed));
    }
    trellis_result.map_err(|error| format!("NeoTrellis input scan failed: {error}"))?;

    let keys = neokey
        .scan_interrupts()
        .map_err(|error| format!("NeoKey input scan failed: {error}"))?;
    {
        for (key, pressed) in keys {
            let index = usize::from(key.min(3));
            if previous_neokey[index] == pressed {
                continue;
            }
            previous_neokey[index] = pressed;
            found_input = true;
            if let Some(message) = neokey_message(key, pressed) {
                let _ = input_tx.send(message);
            }
        }
    }
    Ok(found_input)
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_worker;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod raspberry_worker;
mod scan_schedule;
mod setup_refresh;
#[cfg(test)]
mod tests;
