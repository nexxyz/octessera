use octessera_hal::{encoder_gpio::HardwareEvent, NeoKey, NeoTrellis};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const INPUT_IDLE_NOISE_CHECK: Duration = Duration::from_secs(15);

pub(crate) struct CheckOutcome {
    pub(crate) passed: bool,
    pub(crate) warnings: Vec<String>,
    pub(crate) failures: Vec<String>,
}

/// What the no-touch check saw on each control while nobody touched it.
#[derive(Default)]
struct IdleNoise {
    grid_events: Vec<String>,
    raw_press_counts: [u32; 4],
    neokey_active_since: [Option<Duration>; 4],
    neokey_pulse_durations: [Vec<Duration>; 4],
    neokey_raw_samples: Vec<String>,
    neokey_reread_samples: Vec<String>,
    confirmed_neokey_ghosts: u32,
    encoder_events: Vec<String>,
    trellis_read_failures: u32,
    neokey_read_failures: u32,
}

impl CheckOutcome {
    fn passed_with(warnings: Vec<String>) -> Self {
        Self {
            passed: true,
            warnings,
            failures: Vec::new(),
        }
    }
}

pub(crate) fn input_idle_noise_check(
    mut trellis: Option<&mut NeoTrellis>,
    mut neokey: Option<&mut NeoKey>,
    event_rx: Option<&mpsc::Receiver<HardwareEvent>>,
) -> CheckOutcome {
    println!(
        "STEP no-touch input noise check for {} seconds",
        INPUT_IDLE_NOISE_CHECK.as_secs()
    );
    wait_for_operator("Hands off all controls now. Press Enter to start the no-touch noise check.");
    println!("Do not touch the NeoTrellis, NeoKeys, or encoders during this check.");
    let started = Instant::now();
    let deadline = started + INPUT_IDLE_NOISE_CHECK;
    let mut noise = IdleNoise::default();
    while Instant::now() < deadline {
        poll_trellis_noise(&mut trellis, &mut noise);
        poll_neokey_noise(&mut neokey, started, &mut noise);
        poll_encoder_noise(event_rx, &mut noise.encoder_events);
        thread::sleep(Duration::from_millis(10));
    }
    close_open_neokey_pulses(started.elapsed(), &mut noise);
    classify_noise(noise)
}

fn poll_trellis_noise(trellis: &mut Option<&mut NeoTrellis>, noise: &mut IdleNoise) {
    if let Some(trellis) = trellis.as_deref_mut() {
        let mut events = Vec::new();
        let scan = trellis.scan_keys(&mut events);
        noise
            .grid_events
            .extend(events.into_iter().map(|(x, y, pressed)| {
                format!("x={x} y={y} {}", if pressed { "press" } else { "release" })
            }));
        if let Err(error) = scan {
            println!("FAIL NeoTrellis idle scan: {error}");
            noise.trellis_read_failures += 1;
        }
    }
}

fn poll_neokey_noise(neokey: &mut Option<&mut NeoKey>, started: Instant, noise: &mut IdleNoise) {
    let Some(neokey) = neokey.as_deref_mut() else {
        return;
    };
    match neokey.raw_button_state() {
        Ok(raw_state) => {
            let elapsed = started.elapsed();
            let masked = raw_state & 0xF0;
            let pressed_mask = (!masked) & 0xF0;
            if pressed_mask != 0 {
                noise.neokey_raw_samples.push(format!(
                    "raw=0x{raw_state:08X} masked=0x{masked:02X} pressed_mask=0x{pressed_mask:02X}"
                ));
                let (reread_samples, confirmed) = neokey_reread_burst(neokey);
                if confirmed {
                    noise.confirmed_neokey_ghosts += 1;
                }
                noise.neokey_reread_samples.push(format!(
                    "trigger=0x{raw_state:08X} burst={}",
                    reread_samples.join(",")
                ));
            }
            update_neokey_pulses(pressed_mask, elapsed, noise);
        }
        Err(error) => {
            println!("FAIL NeoKey idle scan: {error}");
            noise.neokey_read_failures += 1;
        }
    }
}

fn poll_encoder_noise(
    event_rx: Option<&mpsc::Receiver<HardwareEvent>>,
    encoder_events: &mut Vec<String>,
) {
    if let Some(event_rx) = event_rx {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                HardwareEvent::EncoderTurn { id, delta } => {
                    encoder_events.push(format!("turn id={} delta={delta}", encoder_label(id)));
                }
                HardwareEvent::EncoderPress { id } => {
                    encoder_events.push(format!("press id={}", encoder_label(id)));
                }
                HardwareEvent::EncoderRelease { id } => {
                    encoder_events.push(format!("release id={}", encoder_label(id)));
                }
            }
        }
    }
}

fn update_neokey_pulses(pressed_mask: u32, elapsed: Duration, noise: &mut IdleNoise) {
    for key in 0..4 {
        let pressed = (pressed_mask & (1 << (key + 4))) != 0;
        if pressed {
            noise.raw_press_counts[key] += 1;
            if noise.neokey_active_since[key].is_none() {
                noise.neokey_active_since[key] = Some(elapsed);
            }
        } else if let Some(started_at) = noise.neokey_active_since[key].take() {
            noise.neokey_pulse_durations[key].push(elapsed.saturating_sub(started_at));
        }
    }
}

fn close_open_neokey_pulses(elapsed: Duration, noise: &mut IdleNoise) {
    for key in 0..4 {
        if let Some(started_at) = noise.neokey_active_since[key].take() {
            noise.neokey_pulse_durations[key].push(elapsed.saturating_sub(started_at));
        }
    }
}

fn classify_noise(noise: IdleNoise) -> CheckOutcome {
    let read_failures = noise.trellis_read_failures + noise.neokey_read_failures;
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let ghost_keys = noise
        .raw_press_counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(index, count)| format!("index={index} samples={count}"))
        .collect::<Vec<_>>();
    let tolerated_neokey_noise = !ghost_keys.is_empty() && noise.confirmed_neokey_ghosts == 0;
    if read_failures == 0
        && (ghost_keys.is_empty() || tolerated_neokey_noise)
        && noise.grid_events.is_empty()
        && noise.encoder_events.is_empty()
    {
        if tolerated_neokey_noise {
            println!("WARN no-touch input noise check passed within tolerance");
            warnings.push(format!(
                "NeoKey raw one-sample noise tolerated: {}",
                ghost_keys.join(" ")
            ));
        } else {
            println!("PASS no-touch input noise check");
        }
        return CheckOutcome::passed_with(warnings);
    }
    append_noise_findings(
        &mut warnings,
        &mut failures,
        &noise,
        &ghost_keys,
        tolerated_neokey_noise,
    );
    CheckOutcome {
        passed: read_failures == 0 && failures.is_empty(),
        warnings,
        failures,
    }
}

fn append_noise_findings(
    warnings: &mut Vec<String>,
    failures: &mut Vec<String>,
    noise: &IdleNoise,
    ghost_keys: &[String],
    tolerated_neokey_noise: bool,
) {
    let IdleNoise {
        grid_events,
        neokey_pulse_durations,
        neokey_raw_samples,
        neokey_reread_samples,
        confirmed_neokey_ghosts: confirmed_ghosts,
        encoder_events,
        trellis_read_failures,
        neokey_read_failures,
        ..
    } = noise;
    if !grid_events.is_empty() {
        println!("FAIL NeoTrellis idle events: {}", grid_events.join("; "));
        failures.push(format!(
            "NeoTrellis idle events: {}",
            grid_events.join("; ")
        ));
    }
    if !ghost_keys.is_empty() {
        print_neokey_noise(
            ghost_keys,
            neokey_raw_samples,
            neokey_reread_samples,
            neokey_pulse_durations,
            *confirmed_ghosts,
        );
        let message = format!(
            "NeoKey idle raw ghost presses: {} confirmed_reread={confirmed_ghosts}",
            ghost_keys.join(" ")
        );
        if tolerated_neokey_noise {
            warnings.push(message);
        } else {
            failures.push(message);
        }
    }
    if !encoder_events.is_empty() {
        println!("FAIL encoder idle events: {}", encoder_events.join("; "));
        failures.push(format!(
            "encoder idle events: {}",
            encoder_events.join("; ")
        ));
    }
    if trellis_read_failures + neokey_read_failures != 0 {
        failures.push(format!(
            "idle input scan failures: trellis={trellis_read_failures} neokey={neokey_read_failures}"
        ));
    }
}

fn print_neokey_noise(
    ghost_keys: &[String],
    raw_samples: &[String],
    reread_samples: &[String],
    pulse_durations: &[Vec<Duration>; 4],
    confirmed_ghosts: u32,
) {
    let level = if confirmed_ghosts == 0 {
        "WARN"
    } else {
        "FAIL"
    };
    println!(
        "{level} NeoKey idle raw ghost presses: {} confirmed_reread={confirmed_ghosts}",
        ghost_keys.join(" ")
    );
    for sample in raw_samples.iter().take(24) {
        println!("NEOKEY_RAW {sample}");
    }
    for sample in reread_samples.iter().take(12) {
        println!("NEOKEY_REREAD {sample}");
    }
    if raw_samples.len() > 24 {
        println!("NEOKEY_RAW ... {} more samples", raw_samples.len() - 24);
    }
    for (index, durations) in pulse_durations.iter().enumerate() {
        if !durations.is_empty() {
            let durations_ms = durations
                .iter()
                .map(|duration| duration.as_millis().to_string())
                .collect::<Vec<_>>()
                .join(",");
            println!("NEOKEY_PULSE index={index} durations_ms={durations_ms}");
        }
    }
}

fn neokey_reread_burst(neokey: &mut NeoKey) -> (Vec<String>, bool) {
    let mut samples = Vec::new();
    let mut confirmed = false;
    for _ in 0..8 {
        thread::sleep(Duration::from_millis(1));
        match neokey.raw_button_state() {
            Ok(raw_state) => {
                let masked = raw_state & 0xF0;
                let pressed_mask = (!masked) & 0xF0;
                if pressed_mask != 0 {
                    confirmed = true;
                }
                samples.push(format!("0x{raw_state:08X}/0x{pressed_mask:02X}"));
            }
            Err(error) => {
                confirmed = true;
                samples.push(format!("ERR:{error}"));
            }
        }
    }
    (samples, confirmed)
}

fn wait_for_operator(prompt: &str) {
    println!("WAIT {prompt}");
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
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
