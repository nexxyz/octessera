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

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn startup_gate_requires_both_input_scan_and_output_ack() {
    assert_eq!(
        seesaw_startup_status(false, true),
        SeesawStartupStatus::WaitingForInputScan
    );
    assert_eq!(
        seesaw_startup_status(true, false),
        SeesawStartupStatus::WaitingForOutputFrame
    );
    assert_eq!(
        seesaw_startup_status(true, true),
        SeesawStartupStatus::Ready
    );
    assert_eq!(SEESAW_STARTUP_TIMEOUT, Duration::from_millis(750));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn dead_output_path_never_acknowledges_startup_frame() {
    let (command_tx, command_rx) = mpsc::channel();
    command_tx
        .send(SeesawCommand::GridFrame([[0; 3]; 64]))
        .unwrap();
    command_tx
        .send(SeesawCommand::NeoKeyColors([[0; 3]; 4]))
        .unwrap();
    let mut outputs = DesiredLedOutputs::default();
    let mut writer = fake_output(usize::MAX, usize::MAX);

    drain_commands(&command_rx, &mut outputs, &mut writer, Instant::now());

    assert!(!outputs.startup_complete());
    assert_eq!(
        seesaw_startup_status(true, outputs.startup_complete()),
        SeesawStartupStatus::WaitingForOutputFrame
    );
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

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn shutdown_black_cleanup_carries_a_bounded_deadline() {
    let (command_tx, command_rx) = mpsc::channel();
    let (ack_tx, _ack_rx) = mpsc::channel::<Result<(), String>>();
    let deadline = Instant::now() + SEESAW_SHUTDOWN_TIMEOUT;
    command_tx
        .send(SeesawCommand::Shutdown {
            ack: ack_tx,
            deadline,
        })
        .unwrap();

    let mut outputs = DesiredLedOutputs::default();
    let mut writer = fake_output(1, 1);
    let shutdown = drain_commands(&command_rx, &mut outputs, &mut writer, Instant::now())
        .expect("shutdown acknowledgement");
    assert_eq!(shutdown.1, deadline);
    assert_eq!(outputs.desired_grid, Some([[0; 3]; 64]));
    assert_eq!(outputs.desired_keys, Some([[0; 3]; 4]));
}
