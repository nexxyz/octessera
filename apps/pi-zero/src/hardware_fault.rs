use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351};
use platform_core::palette;
use std::thread;
use std::time::Duration;

use crate::boot_oled_handoff::NativeOledGuard;
use crate::render::{fault_oled_frame_into, OLED_FRAME_BYTES};

const FAULT_FLASH_MS: u64 = 500;

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
        let handoff = match crate::boot_oled_handoff::native_attach() {
            Ok(handoff) => handoff,
            Err(error) => {
                eprintln!("hardware fault OLED handoff attach failed: {error}");
                return;
            }
        };
        match OledSsd1351::adopt_existing() {
            Ok(oled) => self.oled = Some(oled),
            Err(error) => {
                handoff.mark_failed();
                eprintln!("hardware fault OLED adoption failed: {error}");
            }
        }
        self.oled_handoff = Some(handoff);
    }

    pub(crate) fn summary(&self) -> String {
        self.failures
            .iter()
            .map(|failure| format!("{}: {}", failure.name, failure.message))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(crate) fn run_hardware_fault_mode(mut fault: Box<HardwareFault>, boot_handoff: bool) -> ! {
    eprintln!("entering hardware fault mode; service will stay alive until reboot");
    if boot_handoff && fault.oled.is_none() {
        fault.adopt_boot_handoff_oled();
    }
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    let lines = fault_lines(&fault);
    let mut lit = true;
    loop {
        if let Some(oled) = fault.oled.as_mut() {
            fault_oled_frame_into(&lines, &mut frame, lit);
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
        lit = !lit;
        thread::sleep(Duration::from_millis(FAULT_FLASH_MS));
    }
}

const FAULT_LINE_CHARS: usize = 18;

fn fault_lines(fault: &HardwareFault) -> Vec<String> {
    let mut parts: Vec<&str> = Vec::new();
    for failure in &fault.failures {
        let part = part_name(failure.name);
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    let mut parts = parts.join(" + ");
    if parts.chars().count() > FAULT_LINE_CHARS {
        parts = "Several parts".to_string();
    }
    let timed_out = fault
        .failures
        .iter()
        .any(|failure| is_i2c_timeout(&failure.message));
    let lines: &[&str] = if timed_out {
        &[
            "not responding",
            "",
            "Unplug power, then",
            "plug it back in.",
            "A reboot alone",
            "will not do it.",
        ]
    } else {
        &[
            "not found",
            "",
            "Unplug power,",
            "check the cables,",
            "plug it back in.",
        ]
    };
    std::iter::once(parts)
        .chain(lines.iter().map(|line| line.to_string()))
        .collect()
}

fn part_name(name: &str) -> &'static str {
    match name {
        "TRELLIS" => "Grid",
        "NEOKEY" => "Keys",
        "SEESAW_INT" => "Key signal",
        "OLED" => "Screen",
        "DAC" => "Audio",
        "I2C" => "I2C bus",
        _ => "Encoders",
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

    #[test]
    fn timed_out_grid_and_keys_ask_for_a_power_cycle() {
        let fault = fault(&[
            (
                "TRELLIS",
                "NeoTrellis reset failed: Connection timed out (os error 110)",
            ),
            (
                "NEOKEY",
                "NeoKey HW ID read failed: Connection timed out (os error 110)",
            ),
        ]);
        assert_eq!(
            fault_lines(&fault),
            [
                "Grid + Keys",
                "not responding",
                "",
                "Unplug power, then",
                "plug it back in.",
                "A reboot alone",
                "will not do it.",
            ]
        );
    }

    #[test]
    fn missing_keys_ask_to_check_the_cables() {
        let fault = fault(&[(
            "NEOKEY",
            "NeoKey reset failed: Input/output error (os error 5)",
        )]);
        assert_eq!(
            fault_lines(&fault),
            [
                "Keys",
                "not found",
                "",
                "Unplug power,",
                "check the cables,",
                "plug it back in.",
            ]
        );
    }

    #[test]
    fn encoders_share_one_name_and_long_part_lists_collapse() {
        let encoders = fault(&[("encoder_main", "busy"), ("encoder_aux_1", "busy")]);
        assert_eq!(fault_lines(&encoders)[0], "Encoders");
        let many = fault(&[
            ("TRELLIS", "gone"),
            ("NEOKEY", "gone"),
            ("SEESAW_INT", "gone"),
        ]);
        assert_eq!(fault_lines(&many)[0], "Several parts");
    }

    #[test]
    fn every_fault_line_fits_the_oled() {
        for message in ["Connection timed out (os error 110)", "No such file"] {
            let fault = fault(&[("I2C", message), ("DAC", message)]);
            let lines = fault_lines(&fault);
            assert!(lines.len() <= 7);
            for line in lines {
                assert!(line.chars().count() <= FAULT_LINE_CHARS, "{line}");
            }
        }
    }
}
