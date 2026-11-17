use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351};
use platform_core::palette;
use std::thread;
use std::time::{Duration, Instant};

use crate::boot_oled_handoff::NativeOledGuard;
use crate::render::{fault_oled_frame_into, OLED_FRAME_BYTES};

const FAULT_FLASH_MS: u64 = 500;
const AUDIO_RETRY_AFTER: Duration = Duration::from_secs(30);

pub(crate) struct HardwareFault {
    failures: Vec<HardwareFailure>,
    oled: Option<OledSsd1351>,
    oled_handoff: Option<NativeOledGuard>,
    trellis: Option<NeoTrellis>,
    neokey: Option<NeoKey>,
}

struct HardwareFailure {
    name: &'static str,
    message: String,
}

impl HardwareFault {
    pub(crate) fn new() -> Self {
        Self {
            failures: Vec::new(),
            oled: None,
            oled_handoff: None,
            trellis: None,
            neokey: None,
        }
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }

    pub(crate) fn push(&mut self, name: &'static str, message: String) {
        eprintln!("critical hardware init failed: {name}: {message}");
        self.failures.push(HardwareFailure { name, message });
    }

    pub(crate) fn attach_outputs(
        &mut self,
        oled: Option<OledSsd1351>,
        trellis: Option<NeoTrellis>,
        neokey: Option<NeoKey>,
    ) {
        self.oled = oled;
        self.trellis = trellis;
        self.neokey = neokey;
    }

    fn adopt_boot_handoff_oled(&mut self) {
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let attached = crate::boot_oled_handoff::native_attach();
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let attached = crate::boot_oled_handoff::native_attach_after_startup_clear();
        let handoff = match attached {
            Ok(handoff) => handoff,
            Err(error) => {
                eprintln!("hardware fault OLED handoff attach failed: {error}");
                return;
            }
        };
        match OledSsd1351::adopt_existing() {
            Ok(oled) => self.oled = Some(oled),
            Err(error) => {
                eprintln!("hardware fault OLED adoption failed: {error}");
                if let Err(error) = handoff.mark_failed_result() {
                    eprintln!(
                        "hardware fault OLED handoff failure-state publication failed: {error}"
                    );
                }
            }
        }
        self.oled_handoff = Some(handoff);
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn summary(&self) -> String {
        self.failures
            .iter()
            .map(|failure| format!("{}: {}", failure.name, failure.message))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(crate) fn run_hardware_fault_mode(mut fault: Box<HardwareFault>, boot_handoff: bool) -> ! {
    let retry_audio = retries_audio(&fault);
    if retry_audio {
        eprintln!("entering audio fault mode; restarting to retry in {AUDIO_RETRY_AFTER:?}");
    } else {
        eprintln!("entering hardware fault mode; service will stay alive until reboot");
    }
    if boot_handoff && fault.oled.is_none() {
        fault.adopt_boot_handoff_oled();
    }
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    let (header, lines) = fault_screen(&fault);
    let mut lit = true;
    let started = Instant::now();
    loop {
        if let Some(oled) = fault.oled.as_mut() {
            fault_oled_frame_into(header, &lines, &mut frame, lit);
            let _ = oled.write_frame(&frame);
        }
        if let Some(trellis) = fault.trellis.as_mut() {
            let _ = trellis.write_led_frame(&trellis_fault_frame(lit));
        }
        if let Some(neokey) = fault.neokey.as_mut() {
            for index in 0..4 {
                let color = if lit {
                    platform_core::palette::MAGENTA
                } else {
                    platform_core::palette::BLACK
                };
                let _ = neokey.set_led(index, color[0], color[1], color[2]);
            }
        }
        if retry_audio && lit && started.elapsed() >= AUDIO_RETRY_AFTER {
            std::process::exit(1);
        }
        lit = !lit;
        thread::sleep(Duration::from_millis(FAULT_FLASH_MS));
    }
}

const FAULT_LINE_CHARS: usize = 19;
const MAX_FAULT_LINES: usize = 7;

fn fault_screen(fault: &HardwareFault) -> (&'static str, Vec<String>) {
    let mut parts: Vec<&str> = Vec::new();
    for failure in &fault.failures {
        if let Some(part) = part_name(failure.name) {
            if !parts.contains(&part) {
                parts.push(part);
            }
        }
    }
    if parts.is_empty() {
        return (
            "AUDIO NOT READY",
            [
                "Retrying...",
                "",
                "If this stays for a",
                "few minutes, power-",
                "cycle the device.",
            ]
            .map(str::to_string)
            .to_vec(),
        );
    }
    let mut lines = vec![parts.join(" + ")];
    if lines[0].chars().count() > FAULT_LINE_CHARS {
        lines[0] = "Several parts".to_string();
    }
    let timed_out = fault
        .failures
        .iter()
        .any(|failure| is_i2c_timeout(&failure.message));
    let mut checks: Vec<&str> = Vec::new();
    if !timed_out {
        for part in &parts {
            for check in connection_checks(part) {
                if !checks.contains(check) {
                    checks.push(check);
                }
            }
        }
    }
    lines.push(
        if timed_out {
            "not responding"
        } else {
            "not found"
        }
        .to_string(),
    );
    lines.push(String::new());
    if checks.is_empty() {
        lines.push("Unplug power and".to_string());
    } else {
        lines.push("Unplug power,".to_string());
        lines.extend(checks.iter().map(|check| check.to_string()));
    }
    lines.push("plug it back in.".to_string());
    if lines.len() > MAX_FAULT_LINES {
        lines.remove(2);
    }
    ("HARDWARE FAULT", lines)
}

fn retries_audio(fault: &HardwareFault) -> bool {
    fault
        .failures
        .iter()
        .all(|failure| part_name(failure.name).is_none())
}

fn part_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "TRELLIS" => "Grid",
        "NEOKEY" => "Keys",
        "SEESAW" => "Grid + Keys",
        "SEESAW_INT" => "Key signal",
        "OLED" => "Screen",
        "I2C" => "I2C bus",
        "AUDIO" | "DAC" => return None,
        _ => "Encoders",
    })
}

fn connection_checks(part: &str) -> &'static [&'static str] {
    match part {
        "Grid" | "Key signal" => &["check grid cable,"],
        "Keys" => &["reseat the NeoKey,"],
        "Grid + Keys" => &["check grid cable,", "reseat the NeoKey,"],
        "Screen" => &["reseat the screen,"],
        _ => &[],
    }
}

fn is_i2c_timeout(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("timed out") || lower.contains("os error 110")
}

fn trellis_fault_frame(lit: bool) -> [[u8; 3]; 64] {
    let mut frame = [[0_u8; 3]; 64];
    if !lit {
        return frame;
    }
    for (x, y) in [
        (1, 1),
        (2, 1),
        (3, 1),
        (4, 1),
        (5, 1),
        (1, 2),
        (1, 3),
        (2, 3),
        (3, 3),
        (4, 3),
        (1, 4),
        (1, 5),
        (1, 6),
    ] {
        frame[y * 8 + x] = palette::MAGENTA;
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(failures: &[(&'static str, &str)]) -> HardwareFault {
        let mut fault = HardwareFault::new();
        for (name, message) in failures {
            fault.push(name, (*message).to_string());
        }
        fault
    }

    fn screen(failures: &[(&'static str, &str)]) -> (&'static str, Vec<String>) {
        fault_screen(&fault(failures))
    }

    const TIMED_OUT: &str = "HW ID read failed: Connection timed out (os error 110)";
    const MISSING: &str = "reset failed: Input/output error (os error 5)";

    #[test]
    fn timed_out_grid_and_keys_ask_for_a_power_cycle() {
        assert_eq!(
            screen(&[("TRELLIS", TIMED_OUT), ("NEOKEY", TIMED_OUT)]),
            (
                "HARDWARE FAULT",
                [
                    "Grid + Keys",
                    "not responding",
                    "",
                    "Unplug power and",
                    "plug it back in.",
                ]
                .map(str::to_string)
                .to_vec()
            )
        );
    }

    #[test]
    fn a_seesaw_startup_timeout_names_grid_and_keys() {
        let (_, lines) = screen(&[(
            "SEESAW",
            "Seesaw startup handshake timed out: timed out waiting on channel",
        )]);
        assert_eq!(lines[..2], ["Grid + Keys", "not responding"]);
    }

    #[test]
    fn missing_parts_name_only_their_real_connections() {
        assert_eq!(
            screen(&[("NEOKEY", MISSING)]).1,
            [
                "Keys",
                "not found",
                "",
                "Unplug power,",
                "reseat the NeoKey,",
                "plug it back in."
            ]
        );
        assert_eq!(
            screen(&[("TRELLIS", MISSING)]).1,
            [
                "Grid",
                "not found",
                "",
                "Unplug power,",
                "check grid cable,",
                "plug it back in."
            ]
        );
        assert_eq!(
            screen(&[("encoder_main", "busy"), ("encoder_aux_1", "busy")]).1,
            [
                "Encoders",
                "not found",
                "",
                "Unplug power and",
                "plug it back in."
            ]
        );
    }

    #[test]
    fn long_part_lists_collapse_and_still_fit() {
        let (_, lines) = screen(&[
            ("TRELLIS", MISSING),
            ("NEOKEY", MISSING),
            ("SEESAW_INT", MISSING),
            ("OLED", MISSING),
        ]);
        assert_eq!(
            lines,
            [
                "Several parts",
                "not found",
                "Unplug power,",
                "check grid cable,",
                "reseat the NeoKey,",
                "reseat the screen,",
                "plug it back in.",
            ]
        );
    }

    #[test]
    fn audio_failures_show_the_audio_screen_without_cable_advice() {
        let expected = (
            "AUDIO NOT READY",
            [
                "Retrying...",
                "",
                "If this stays for a",
                "few minutes, power-",
                "cycle the device.",
            ]
            .map(str::to_string)
            .to_vec(),
        );
        assert_eq!(screen(&[("AUDIO", "no ALSA device")]), expected);
        assert_eq!(screen(&[("DAC", "no ALSA device")]), expected);
        let (header, lines) = screen(&[("DAC", MISSING), ("NEOKEY", MISSING)]);
        assert_eq!(header, "HARDWARE FAULT");
        assert_eq!(lines[0], "Keys");
    }

    #[test]
    fn only_audio_faults_restart_to_retry() {
        assert!(retries_audio(&fault(&[("AUDIO", "no ALSA device")])));
        assert!(retries_audio(&fault(&[("DAC", "no ALSA device")])));
        assert!(!retries_audio(&fault(&[
            ("DAC", MISSING),
            ("NEOKEY", MISSING)
        ])));
        assert!(!retries_audio(&fault(&[("I2C", MISSING)])));
        assert!(!retries_audio(&fault(&[("TRELLIS", TIMED_OUT)])));
    }

    #[test]
    fn every_fault_screen_fits_the_oled() {
        for message in [TIMED_OUT, MISSING] {
            for names in [
                &["TRELLIS", "NEOKEY", "SEESAW_INT", "OLED", "I2C"][..],
                &["SEESAW"],
                &["AUDIO"],
                &["encoder_main"],
            ] {
                let failures: Vec<_> = names.iter().map(|name| (*name, message)).collect();
                let (header, lines) = screen(&failures);
                assert!(header.chars().count() <= 15);
                assert!(lines.len() <= MAX_FAULT_LINES);
                for line in lines {
                    assert!(line.chars().count() <= FAULT_LINE_CHARS, "{line}");
                }
            }
        }
    }
}
