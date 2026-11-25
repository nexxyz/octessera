use octessera_hal::encoder_gpio::HardwareEvent;
use octessera_hal::{NeoKey, NeoTrellis, OledSsd1351};
use platform_core::palette;
use std::any::Any;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant};

use crate::boot_oled_handoff::NativeOledGuard;
use crate::render::{fault_oled_frame_into, OLED_FRAME_BYTES};

const FAULT_FLASH_MS: u64 = 500;
const AUDIO_RETRY_AFTER: Duration = Duration::from_secs(30);
pub(crate) const FAULT_SCREEN_AWAKE: Duration = Duration::from_secs(10);
pub(crate) const FAULT_SCREEN_CONTRAST: u8 = 0x0F;

pub(crate) struct HardwareFault {
    failures: Vec<HardwareFailure>,
    oled: Option<OledSsd1351>,
    oled_handoff: Option<NativeOledGuard>,
    trellis: Option<NeoTrellis>,
    neokey: Option<NeoKey>,
    wake: Option<WakeInput>,
}

struct WakeInput {
    events: crate::runtime_handoff::Receiver<HardwareEvent>,
    _encoders: Box<dyn Any>,
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
            wake: None,
        }
    }

    #[cfg(board_not_orange)]
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

    pub(crate) fn attach_wake_encoders<E: 'static>(
        &mut self,
        events: crate::runtime_handoff::Receiver<HardwareEvent>,
        encoders: E,
    ) {
        self.wake = Some(WakeInput {
            events,
            _encoders: Box::new(encoders),
        });
    }

    pub(crate) fn open_wake_encoders(&mut self) {
        #[cfg(board_not_orange)]
        let opened = crate::hardware_init::init_encoders().map_err(|fault| fault.summary());
        #[cfg(board_orange)]
        let opened = crate::orange_runtime::init_encoders();
        match opened {
            Ok((events, encoders)) => self.attach_wake_encoders(events, encoders),
            Err(error) => {
                eprintln!("hardware fault encoders unavailable; screen stays on: {error}")
            }
        }
    }

    pub(crate) fn adopt_boot_handoff_oled(&mut self) {
        #[cfg(board_not_orange)]
        let attached = crate::boot_oled_handoff::native_attach();
        #[cfg(board_orange)]
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

    pub(crate) fn oled_mut(&mut self) -> Option<&mut OledSsd1351> {
        self.oled.as_mut()
    }

    pub(crate) fn wake_events(&self) -> Option<&crate::runtime_handoff::Receiver<HardwareEvent>> {
        self.wake.as_ref().map(|wake| &wake.events)
    }

    #[cfg(board_not_orange)]
    fn failure_names(&self) -> String {
        self.failures
            .iter()
            .map(|failure| failure.name)
            .collect::<Vec<_>>()
            .join(",")
    }

    #[cfg(board_not_orange)]
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
        #[cfg(board_not_orange)]
        bus_lines::record(&fault.failure_names());
    }
    if boot_handoff && fault.oled.is_none() {
        fault.adopt_boot_handoff_oled();
    }
    if fault.wake.is_none() {
        fault.open_wake_encoders();
    }
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    let (header, lines) = fault_screen(&fault);
    let mut lit = true;
    let started = Instant::now();
    let mut awake_until = started + FAULT_SCREEN_AWAKE;
    let mut screen_shown = None;
    loop {
        let screen_on = fault.wake.is_none() || Instant::now() < awake_until;
        if let Some(oled) = fault.oled.as_mut() {
            if screen_shown != Some(screen_on) {
                let applied = if screen_on {
                    oled.set_master_contrast(FAULT_SCREEN_CONTRAST)
                        .and_then(|()| oled.display_on())
                } else {
                    oled.display_off()
                };
                if applied.is_ok() {
                    screen_shown = Some(screen_on);
                }
            }
            if screen_on {
                fault_oled_frame_into(header, &lines, &mut frame, lit);
                let _ = oled.write_frame(&frame);
            }
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
        let flash_until = Instant::now() + Duration::from_millis(FAULT_FLASH_MS);
        match fault
            .wake
            .as_ref()
            .map(|wake| wait_for_wake(&wake.events, flash_until))
        {
            Some(Wake::Input) => awake_until = Instant::now() + FAULT_SCREEN_AWAKE,
            Some(Wake::Lost) => fault.wake = None,
            Some(Wake::Quiet) => {}
            None => thread::sleep(Duration::from_millis(FAULT_FLASH_MS)),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Wake {
    Quiet,
    Input,
    Lost,
}

fn wait_for_wake(events: &crate::runtime_handoff::Receiver<HardwareEvent>, until: Instant) -> Wake {
    match events.recv_timeout(until.saturating_duration_since(Instant::now())) {
        Ok(_) => {
            while events.try_recv().is_ok() {}
            Wake::Input
        }
        Err(RecvTimeoutError::Timeout) => Wake::Quiet,
        Err(RecvTimeoutError::Disconnected) => {
            thread::sleep(until.saturating_duration_since(Instant::now()));
            Wake::Lost
        }
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

fn part_name(name: &'static str) -> Option<&'static str> {
    Some(match name {
        "TRELLIS" => "Grid",
        "NEOKEY" => "Keys",
        "SEESAW" => "Grid + Keys",
        "SEESAW_INT" => "Key signal",
        "OLED" => "Screen",
        "I2C" => "I2C bus",
        "ENCODERS" | "encoder_main" | "encoder_aux_1" | "encoder_aux_2" | "encoder_aux_3" => {
            "Encoders"
        }
        "AUDIO" | "DAC" => return None,
        _ => name,
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

#[cfg(board_not_orange)]
mod bus_lines;
#[cfg(test)]
mod tests;
