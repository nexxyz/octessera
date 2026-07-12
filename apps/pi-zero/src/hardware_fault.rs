use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351};
use platform_core::palette;
use std::thread;
use std::time::Duration;

use crate::render::{fault_oled_frame_into, OLED_FRAME_BYTES};

const FAULT_FLASH_MS: u64 = 500;

pub(crate) struct HardwareFault {
    failures: Vec<HardwareFailure>,
    oled: Option<OledSsd1351>,
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

    pub(crate) fn summary(&self) -> String {
        self.failures
            .iter()
            .map(|failure| format!("{}: {}", failure.name, failure.message))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(crate) fn failure_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.failures.iter().map(|failure| failure.name)
    }
}

pub(crate) fn run_hardware_fault_mode(mut fault: HardwareFault) -> ! {
    eprintln!("entering hardware fault mode; service will stay alive until reboot");
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
                    platform_core::palette::RED
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

fn fault_lines(fault: &HardwareFault) -> Vec<String> {
    let hint = if fault
        .failures
        .iter()
        .any(|failure| is_i2c_timeout(&failure.message))
    {
        "POWER CYCLE"
    } else {
        "CHECK WIRING"
    };
    let mut lines = vec![hint.to_string()];
    for failure in fault.failures.iter().take(3) {
        lines.push(short_name(failure.name).to_string());
        lines.push(concise_error(&failure.message));
    }
    lines
}

fn short_name(name: &str) -> &str {
    match name {
        "SEESAW_INT" => "SEESAW INT",
        other => other,
    }
}

fn concise_error(message: &str) -> String {
    if is_i2c_timeout(message) {
        return "I2C TIMEOUT".to_string();
    }
    let lower = message.to_ascii_lowercase();
    if lower.contains("input/output")
        || lower.contains("remote i/o")
        || lower.contains("os error 5")
    {
        return "I2C ERROR".to_string();
    }
    if lower.contains("no such file") || lower.contains("not found") {
        return "NOT FOUND".to_string();
    }
    if lower.contains("invalid") {
        return "BAD HW ID".to_string();
    }
    if lower.contains("permission") {
        return "PERMISSION".to_string();
    }
    message
        .split(':')
        .next_back()
        .unwrap_or(message)
        .trim()
        .chars()
        .take(14)
        .collect()
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
        frame[y * 8 + x] = palette::RED;
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::concise_error;

    #[test]
    fn concise_error_names_i2c_timeouts() {
        assert_eq!(
            concise_error("NeoKey HW ID read failed: Connection timed out (os error 110)"),
            "I2C TIMEOUT"
        );
    }

    #[test]
    fn concise_error_names_i2c_io_errors() {
        assert_eq!(
            concise_error("NeoKey reset failed: Input/output error (os error 5)"),
            "I2C ERROR"
        );
    }
}
