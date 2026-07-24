use crate::input::{grid_message, neokey_message};
use octessera_hal::{NeoKey, NeoTrellis, SeesawInterrupt};
use playback_runtime::HostMessage;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const INPUT_SERVICE_INTERVAL: Duration = Duration::from_millis(4);
const OUTPUT_RETRY_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone)]
pub(crate) enum SeesawCommand {
    GridFrame([[u8; 3]; 64]),
    NeoKeyColors([[u8; 3]; 4]),
}

pub(crate) struct SeesawIo {
    pub(crate) input_rx: Receiver<HostMessage>,
    pub(crate) command_tx: Sender<SeesawCommand>,
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
    interrupt: SeesawInterrupt,
) -> SeesawIo {
    let (input_tx, input_rx) = mpsc::channel::<HostMessage>();
    let (command_tx, command_rx) = mpsc::channel::<SeesawCommand>();
    thread::spawn(move || {
        let mut previous_neokey = [false; 4];
        let mut outputs = DesiredLedOutputs::default();
        let mut last_input_service = Instant::now() - INPUT_SERVICE_INTERVAL;
        loop {
            let service_due = last_input_service.elapsed() >= INPUT_SERVICE_INTERVAL;
            if service_due || interrupt.pending() {
                scan_inputs(&mut trellis, &mut neokey, &mut previous_neokey, &input_tx);
                last_input_service = Instant::now();
            }

            let mut output = HardwareLedOutput {
                trellis: &mut trellis,
                neokey: &mut neokey,
            };
            drain_commands(&command_rx, &mut outputs, &mut output, Instant::now());

            thread::sleep(Duration::from_millis(2));
        }
    });

    SeesawIo {
        input_rx,
        command_tx,
    }
}

fn drain_commands<W: LedOutputWriter>(
    command_rx: &Receiver<SeesawCommand>,
    outputs: &mut DesiredLedOutputs,
    writer: &mut W,
    now: Instant,
) {
    for _ in 0..32 {
        let Ok(command) = command_rx.try_recv() else {
            break;
        };
        outputs.accept(command);
    }
    outputs.apply_due(writer, now);
}

fn scan_inputs(
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    previous_neokey: &mut [bool; 4],
    input_tx: &Sender<HostMessage>,
) {
    if let Ok(presses) = trellis.scan_keys() {
        for (x, y, pressed) in presses {
            crate::wake_trace::log_trellis_event(x, y, pressed);
            let _ = input_tx.send(grid_message(x, y, pressed));
        }
    }

    if let Ok(keys) = neokey.scan_interrupts() {
        for (key, pressed) in keys {
            let index = usize::from(key.min(3));
            if previous_neokey[index] == pressed {
                continue;
            }
            previous_neokey[index] = pressed;
            crate::wake_trace::log_neokey_transition(key, pressed);
            if let Some(message) = neokey_message(key, pressed) {
                let _ = input_tx.send(message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeLedOutput {
        grid_failures: usize,
        key_failures: usize,
        grid_attempts: usize,
        key_attempts: Vec<u8>,
        grid_frames: Vec<[[u8; 3]; 64]>,
    }

    impl LedOutputWriter for FakeLedOutput {
        fn write_grid(&mut self, frame: &[[u8; 3]; 64]) -> Result<(), String> {
            self.grid_attempts += 1;
            self.grid_frames.push(*frame);
            if self.grid_failures == 0 {
                Ok(())
            } else {
                self.grid_failures -= 1;
                Err("injected grid failure".into())
            }
        }

        fn write_key(&mut self, key: u8, _color: [u8; 3]) -> Result<(), String> {
            self.key_attempts.push(key);
            if self.key_failures == 0 {
                Ok(())
            } else {
                self.key_failures -= 1;
                Err("injected key failure".into())
            }
        }
    }

    fn fake_output(grid_failures: usize, key_failures: usize) -> FakeLedOutput {
        FakeLedOutput {
            grid_failures,
            key_failures,
            grid_attempts: 0,
            key_attempts: Vec::new(),
            grid_frames: Vec::new(),
        }
    }

    #[test]
    fn failed_outputs_remain_desired_and_retry_at_bounded_cadence() {
        let (command_tx, command_rx) = mpsc::channel();
        let first = [[1; 3]; 64];
        let latest = [[2; 3]; 64];
        let keys = [[3; 3]; 4];
        command_tx.send(SeesawCommand::GridFrame(first)).unwrap();
        command_tx.send(SeesawCommand::GridFrame(latest)).unwrap();
        command_tx.send(SeesawCommand::NeoKeyColors(keys)).unwrap();

        let start = Instant::now();
        let mut outputs = DesiredLedOutputs::default();
        let mut writer = fake_output(1, 1);
        drain_commands(&command_rx, &mut outputs, &mut writer, start);

        assert_eq!(outputs.desired_grid, Some(latest));
        assert_eq!(outputs.applied_grid, None);
        assert_eq!(writer.grid_attempts, 1);
        assert_eq!(writer.grid_frames, vec![latest]);
        assert_eq!(writer.key_attempts, vec![0, 1, 2, 3]);

        drain_commands(
            &command_rx,
            &mut outputs,
            &mut writer,
            start + OUTPUT_RETRY_INTERVAL - Duration::from_millis(1),
        );
        assert_eq!(writer.grid_attempts, 1);
        assert_eq!(writer.key_attempts, vec![0, 1, 2, 3]);

        drain_commands(
            &command_rx,
            &mut outputs,
            &mut writer,
            start + OUTPUT_RETRY_INTERVAL,
        );
        assert_eq!(outputs.applied_grid, Some(latest));
        assert_eq!(writer.grid_attempts, 2);
        assert_eq!(writer.key_attempts, vec![0, 1, 2, 3, 0]);
        assert_eq!(outputs.applied_key_valid, [true; 4]);
    }

    #[test]
    fn shutdown_black_outputs_are_retryable() {
        let (command_tx, command_rx) = mpsc::channel();
        let black_grid = [[0; 3]; 64];
        let black_keys = [[0; 3]; 4];
        command_tx
            .send(SeesawCommand::GridFrame(black_grid))
            .unwrap();
        command_tx
            .send(SeesawCommand::NeoKeyColors(black_keys))
            .unwrap();

        let start = Instant::now();
        let mut outputs = DesiredLedOutputs::default();
        let mut writer = fake_output(1, 1);
        drain_commands(&command_rx, &mut outputs, &mut writer, start);
        assert_eq!(outputs.desired_grid, Some(black_grid));
        assert_eq!(outputs.applied_grid, None);
        assert_eq!(outputs.desired_keys, Some(black_keys));
        assert_eq!(outputs.applied_key_valid, [false, true, true, true]);

        drain_commands(
            &command_rx,
            &mut outputs,
            &mut writer,
            start + OUTPUT_RETRY_INTERVAL,
        );
        assert_eq!(outputs.applied_grid, Some(black_grid));
        assert_eq!(outputs.applied_key_valid, [true; 4]);
    }
}
