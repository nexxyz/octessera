use cellsymphony_hal::{encoder_gpio::HardwareEvent, I2CBus, I2sDac, NeoKey, NeoTrellis};
use std::io;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::hardware_init::init_encoders;
use crate::hardware_test_noise::input_idle_noise_check;

const TEST_DURATION: Duration = Duration::from_secs(90);
const NEOKEY_DEBOUNCE_SAMPLES: u8 = 2;

pub(crate) fn requested() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--hardware-test")
        || std::env::var("CELLSYMPHONY_PI_HARDWARE_TEST")
            .ok()
            .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

pub(crate) fn noise_requested() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--hardware-noise-test")
        || std::env::var("CELLSYMPHONY_PI_HARDWARE_NOISE_TEST")
            .ok()
            .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

pub(crate) fn run_noise_only() -> bool {
    println!("Cell Symphony no-touch hardware noise test mode");
    let options = NoiseTestOptions::from_args();
    let _i2c = match I2CBus::new(1) {
        Ok(bus) => bus,
        Err(error) => return fail(format!("I2C init failed: {error}")),
    };
    let mut trellis = if options.skip_trellis {
        println!("SKIP NeoTrellis noise source");
        None
    } else {
        match NeoTrellis::new("/dev/i2c-1") {
            Ok(trellis) => Some(trellis),
            Err(error) => return fail(format!("NeoTrellis init failed: {error}")),
        }
    };
    let mut neokey = if options.skip_neokey {
        println!("SKIP NeoKey noise source");
        None
    } else {
        match NeoKey::new("/dev/i2c-1") {
            Ok(neokey) => Some(neokey),
            Err(error) => return fail(format!("NeoKey init failed: {error}")),
        }
    };
    let encoder_handles = if options.skip_encoders {
        println!("SKIP encoder noise source");
        None
    } else {
        match init_encoders() {
            Ok(encoders) => Some(encoders),
            Err(fault) => return fail(format!("encoder init failed: {}", fault.summary())),
        }
    };
    let outcome = input_idle_noise_check(
        trellis.as_mut(),
        neokey.as_mut(),
        encoder_handles.as_ref().map(|(event_rx, _)| event_rx),
    );
    print_summary(&outcome.warnings, &outcome.failures);
    if outcome.passed {
        println!("PASS no-touch hardware noise test complete");
    } else {
        println!("FAIL no-touch hardware noise test complete");
    }
    outcome.passed
}

struct NoiseTestOptions {
    skip_trellis: bool,
    skip_neokey: bool,
    skip_encoders: bool,
}

impl NoiseTestOptions {
    fn from_args() -> Self {
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        Self {
            skip_trellis: args.iter().any(|arg| arg == "--skip-trellis"),
            skip_neokey: args.iter().any(|arg| arg == "--skip-neokey"),
            skip_encoders: args.iter().any(|arg| arg == "--skip-encoders"),
        }
    }
}

pub(crate) fn run() -> bool {
    println!("Cell Symphony hardware test mode");
    let _i2c = match I2CBus::new(1) {
        Ok(bus) => bus,
        Err(error) => return fail(format!("I2C init failed: {error}")),
    };
    let mut trellis = match NeoTrellis::new("/dev/i2c-1") {
        Ok(trellis) => trellis,
        Err(error) => return fail(format!("NeoTrellis init failed: {error}")),
    };
    let mut neokey = match NeoKey::new("/dev/i2c-1") {
        Ok(neokey) => neokey,
        Err(error) => return fail(format!("NeoKey init failed: {error}")),
    };
    let _dac = match I2sDac::new() {
        Ok(dac) => dac,
        Err(error) => return fail(format!("DAC init failed: {error}")),
    };
    let (event_rx, _encoders) = match init_encoders() {
        Ok(encoders) => encoders,
        Err(fault) => return fail(format!("encoder init failed: {}", fault.summary())),
    };

    println!("PASS hardware initialized: NeoTrellis, NeoKey, DAC, encoders");
    let mut passed = true;
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    if !trellis_led_check(&mut trellis) {
        failures.push("NeoTrellis LED/orientation check reported failures".to_string());
        passed = false;
    }
    if !neokey_led_check(&mut neokey) {
        failures.push("NeoKey LED check reported failures".to_string());
        passed = false;
    }
    let noise_outcome =
        input_idle_noise_check(Some(&mut trellis), Some(&mut neokey), Some(&event_rx));
    warnings.extend(noise_outcome.warnings);
    failures.extend(noise_outcome.failures);
    passed &= noise_outcome.passed;
    if !input_event_check(&mut trellis, &mut neokey, &event_rx) {
        failures.push("Interactive input logging reported failures".to_string());
        passed = false;
    }
    if !audio_check() {
        failures.push("Audio check reported failures".to_string());
        passed = false;
    }
    print_summary(&warnings, &failures);
    if passed {
        println!("PASS hardware test mode complete");
    } else {
        println!("FAIL hardware test mode complete with failed checks");
    }
    passed
}

fn trellis_led_check(trellis: &mut NeoTrellis) -> bool {
    let mut passed = true;
    println!("STEP NeoTrellis LED board colors: 0x2E red, 0x2F green, 0x30 blue, 0x31 white");
    let mut frame = [[0_u8; 3]; 64];
    fill_board(&mut frame, 2, [80, 0, 0]);
    fill_board(&mut frame, 3, [0, 80, 0]);
    fill_board(&mut frame, 0, [0, 0, 80]);
    fill_board(&mut frame, 1, [80, 80, 80]);
    passed &= report(
        "NeoTrellis board color write",
        trellis.write_led_frame(&frame),
    );
    wait_for_operator("Confirm four board color regions, then press Enter.");

    println!("STEP NeoTrellis sweep: red pixel left-to-right, bottom-to-top");
    for y in 0..8 {
        for x in 0..8 {
            frame = [[0_u8; 3]; 64];
            frame[y * 8 + x] = [120, 0, 0];
            if let Err(error) = trellis.write_led_frame(&frame) {
                println!("FAIL NeoTrellis sweep write x={x} y={y}: {error}");
                passed = false;
                break;
            }
            thread::sleep(Duration::from_millis(70));
        }
    }

    println!("STEP NeoTrellis corners: (0,0)=red (7,0)=green (0,7)=blue (7,7)=white");
    frame = [[0_u8; 3]; 64];
    frame[0] = [120, 0, 0];
    frame[7] = [0, 120, 0];
    frame[56] = [0, 0, 120];
    frame[63] = [120, 120, 120];
    passed &= report("NeoTrellis corner write", trellis.write_led_frame(&frame));
    wait_for_operator("Confirm corner orientation, then press Enter.");
    passed
}

fn neokey_led_check(neokey: &mut NeoKey) -> bool {
    let mut passed = true;
    println!("STEP NeoKey LEDs: key0 red, key1 green, key2 blue, key3 white");
    for (index, color) in [[120, 0, 0], [0, 120, 0], [0, 0, 120], [80, 80, 80]]
        .into_iter()
        .enumerate()
    {
        passed &= report(
            &format!("NeoKey LED {index}"),
            neokey.set_led(index as u8, color[0], color[1], color[2]),
        );
    }
    wait_for_operator("Confirm NeoKey LED order/colors, then press Enter.");
    for index in 0..4 {
        if let Err(error) = neokey.set_led(index, 32, 16, 0) {
            println!("FAIL NeoKey amber reset {index}: {error}");
            passed = false;
        }
    }
    passed
}

fn print_summary(warnings: &[String], failures: &[String]) {
    println!(
        "SUMMARY warnings={} failures={}",
        warnings.len(),
        failures.len()
    );
    for warning in warnings {
        println!("SUMMARY WARN {warning}");
    }
    for failure in failures {
        println!("SUMMARY FAIL {failure}");
    }
}

fn input_event_check(
    trellis: &mut NeoTrellis,
    neokey: &mut NeoKey,
    event_rx: &mpsc::Receiver<HardwareEvent>,
) -> bool {
    let mut passed = true;
    println!(
        "STEP input event logging for up to {} seconds",
        TEST_DURATION.as_secs()
    );
    println!("Press grid cells, NeoKeys, and turn/click all encoders now.");
    println!("Press Enter when finished, or wait for the timer to expire.");
    let deadline = Instant::now() + TEST_DURATION;
    let (done_tx, done_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        let _ = done_tx.send(());
    });
    let mut previous_neokey = [false; 4];
    let mut candidate_neokey = [false; 4];
    let mut candidate_counts = [0_u8; 4];
    let mut raw_neokey = [false; 4];
    let mut raw_transition_counts = [0_u32; 4];
    let mut stable_transition_counts = [0_u32; 4];
    let mut grid_seen = [[false; 8]; 8];
    let mut frame = [[0_u8; 3]; 64];
    while Instant::now() < deadline {
        if done_rx.try_recv().is_ok() {
            println!("STEP input event logging skipped by operator");
            break;
        }
        match trellis.scan_keys() {
            Ok(events) => {
                for (x, y, pressed) in events {
                    println!(
                        "GRID {} x={x} y={y}",
                        if pressed { "press" } else { "release" }
                    );
                    if x < 8 && y < 8 {
                        grid_seen[y][x] = true;
                        frame[y * 8 + x] = if pressed { [90, 90, 0] } else { [0, 0, 32] };
                        if let Err(error) = trellis.write_led_frame(&frame) {
                            println!("FAIL NeoTrellis input feedback write: {error}");
                            passed = false;
                        }
                    }
                }
            }
            Err(error) => {
                println!("FAIL NeoTrellis scan: {error}");
                passed = false;
            }
        }
        match neokey.scan() {
            Ok(keys) => {
                for (key, pressed) in keys {
                    let index = usize::from(key);
                    if raw_neokey[index] != pressed {
                        raw_neokey[index] = pressed;
                        raw_transition_counts[index] += 1;
                    }
                    if candidate_neokey[index] == pressed {
                        candidate_counts[index] = candidate_counts[index].saturating_add(1);
                    } else {
                        candidate_neokey[index] = pressed;
                        candidate_counts[index] = 1;
                    }
                    if previous_neokey[index] != pressed
                        && candidate_counts[index] >= NEOKEY_DEBOUNCE_SAMPLES
                    {
                        previous_neokey[index] = pressed;
                        stable_transition_counts[index] += 1;
                        println!(
                            "NEOKEY {} index={key}",
                            if pressed { "press" } else { "release" }
                        );
                        let color = if pressed { [90, 90, 90] } else { [32, 16, 0] };
                        let _ = neokey.set_led(key, color[0], color[1], color[2]);
                    }
                }
            }
            Err(error) => {
                println!("FAIL NeoKey scan: {error}");
                passed = false;
            }
        }
        while let Ok(event) = event_rx.try_recv() {
            match event {
                HardwareEvent::EncoderTurn { id, delta } => {
                    println!("ENCODER turn id={} delta={delta}", encoder_label(id));
                }
                HardwareEvent::EncoderPress { id } => {
                    println!("ENCODER press id={}", encoder_label(id))
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    let missing = missing_grid_cells(&grid_seen);
    if missing.is_empty() {
        println!("PASS NeoTrellis grid coverage: all cells seen");
    } else {
        println!("WARN NeoTrellis grid cells not seen: {}", missing.join(" "));
    }
    for index in 0..4 {
        let suppressed =
            raw_transition_counts[index].saturating_sub(stable_transition_counts[index]);
        if suppressed > 0 {
            println!(
                "WARN NeoKey raw noise index={index} raw_transitions={} stable_transitions={} suppressed={suppressed}",
                raw_transition_counts[index], stable_transition_counts[index]
            );
        }
    }
    println!("STEP input event logging complete");
    passed
}

fn audio_check() -> bool {
    println!("STEP ALSA 440 Hz speaker-test");
    wait_for_operator("Connect speakers/headphones and set a safe volume, then press Enter.");
    let passed = match Command::new("timeout")
        .args([
            "15",
            "speaker-test",
            "-D",
            "hw:0,0",
            "-c",
            "2",
            "-t",
            "sine",
            "-f",
            "440",
            "-l",
            "1",
        ])
        .status()
    {
        Ok(status) if status.success() => {
            println!("PASS speaker-test completed");
            true
        }
        Ok(status) => {
            println!("FAIL speaker-test exited with {status}");
            false
        }
        Err(error) => {
            println!("FAIL speaker-test unavailable: {error}");
            false
        }
    };
    wait_for_operator("Confirm whether audio was audible, then press Enter.");
    wait_for_operator("Hardware test is complete. Press Enter to exit.");
    passed
}

fn fill_board(frame: &mut [[u8; 3]; 64], board: usize, color: [u8; 3]) {
    let base_x = (board % 2) * 4;
    let base_y = (board / 2) * 4;
    for y in base_y..base_y + 4 {
        for x in base_x..base_x + 4 {
            frame[y * 8 + x] = color;
        }
    }
}

fn wait_for_operator(prompt: &str) {
    println!("WAIT {prompt}");
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

fn report(label: &str, result: Result<(), String>) -> bool {
    match result {
        Ok(()) => {
            println!("PASS {label}");
            true
        }
        Err(error) => {
            println!("FAIL {label}: {error}");
            false
        }
    }
}

fn encoder_label(id: &str) -> &str {
    match id {
        "encoder_main" => "main",
        "encoder_aux_1" => "aux1",
        "encoder_aux_2" => "aux2",
        "encoder_aux_3" => "aux3",
        _ => id,
    }
}

fn missing_grid_cells(grid_seen: &[[bool; 8]; 8]) -> Vec<String> {
    let mut missing = Vec::new();
    for (y, row) in grid_seen.iter().enumerate() {
        for (x, seen) in row.iter().enumerate() {
            if !seen {
                missing.push(format!("({x},{y})"));
            }
        }
    }
    missing
}

fn fail(message: String) -> bool {
    println!("FAIL {message}");
    false
}
