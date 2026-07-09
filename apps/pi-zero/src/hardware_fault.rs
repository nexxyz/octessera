use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351};
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
                let color = if lit { 180 } else { 0 };
                let _ = neokey.set_led(index, color, 0, 0);
            }
        }
        lit = !lit;
        thread::sleep(Duration::from_millis(FAULT_FLASH_MS));
    }
}

fn fault_lines(fault: &HardwareFault) -> Vec<String> {
    let mut lines = vec!["HARDWARE FAULT".to_string(), "CHECK WIRING".to_string()];
    lines.extend(
        fault
            .failures
            .iter()
            .take(5)
            .map(|failure| format!("{}: {}", failure.name, concise_error(&failure.message))),
    );
    lines
}

fn concise_error(message: &str) -> String {
    message
        .split(':')
        .next_back()
        .unwrap_or(message)
        .trim()
        .chars()
        .take(18)
        .collect()
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
        frame[y * 8 + x] = [180, 0, 0];
    }
    frame
}
