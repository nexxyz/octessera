//! The screen shown when the saved settings won't load: the user picks Restore factory settings
//! (only the broken file is replaced, and kept beside it) or Shut down.

mod recovery;
#[cfg(test)]
mod tests;

use crate::hardware_fault::{HardwareFault, FAULT_SCREEN_AWAKE, FAULT_SCREEN_CONTRAST};
use crate::render::{fault_oled_frame_into, OLED_FRAME_BYTES};
use crate::usb_config::UsbRuntimeConfig;
use octessera_hal::encoder_gpio::HardwareEvent;
use octessera_hal::OledSsd1351;
use playback_runtime::AudioOptimization;
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant};

const HEADER: &str = "CAN'T START";
const RESTART_PAUSE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Choice {
    Restore,
    ShutDown,
}

pub(crate) fn read_boot_runtime_config_or_restore(
    store_dir: &Path,
    boot_handoff: bool,
) -> (UsbRuntimeConfig, AudioOptimization) {
    match crate::usb_config::read_boot_runtime_config(store_dir) {
        Ok(config) => config,
        Err(error) => run_store_fault_mode(&error.to_string(), store_dir, boot_handoff),
    }
}

fn run_store_fault_mode(error: &str, store_dir: &Path, boot_handoff: bool) -> ! {
    eprintln!("saved settings won't load ({error}); showing the restore screen");
    let mut devices = HardwareFault::new();
    if boot_handoff {
        devices.adopt_boot_handoff_oled();
    } else {
        match OledSsd1351::new() {
            Ok(oled) => devices.attach_outputs(Some(oled), None, None),
            Err(error) => eprintln!("restore screen OLED unavailable: {error}"),
        }
    }
    devices.open_wake_encoders();
    let mut choice = Choice::Restore;
    let mut lines = choice_lines(choice, None);
    let mut awake_until = Instant::now() + FAULT_SCREEN_AWAKE;
    let mut input_lost = devices.wake_events().is_none();
    let mut shown = None;
    let mut drawn = false;
    loop {
        let screen_on = input_lost || Instant::now() < awake_until;
        show(&mut devices, &lines, screen_on, &mut shown, &mut drawn);
        let event = devices
            .wake_events()
            .map(|events| events.recv_timeout(Duration::from_millis(500)));
        let event = match event {
            Some(Ok(event)) => event,
            Some(Err(RecvTimeoutError::Timeout)) => continue,
            Some(Err(RecvTimeoutError::Disconnected)) | None => {
                input_lost = true;
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        awake_until = Instant::now() + FAULT_SCREEN_AWAKE;
        if !screen_on {
            continue;
        }
        drawn = false;
        match event {
            HardwareEvent::EncoderTurn { delta, .. } if delta != 0 => {
                choice = other(choice);
                lines = choice_lines(choice, None);
            }
            HardwareEvent::EncoderPress { .. } if choice == Choice::Restore => {
                match restore(store_dir) {
                    Ok(()) => {
                        lines = message_lines(&[
                            "Factory settings",
                            "are back; the old",
                            "file was kept.",
                            "",
                            "Starting again...",
                        ]);
                        show(&mut devices, &lines, true, &mut shown, &mut drawn);
                        thread::sleep(RESTART_PAUSE);
                        std::process::exit(1);
                    }
                    Err(error) => {
                        eprintln!("factory restore failed: {error}");
                        lines = choice_lines(choice, Some("Restore failed."));
                    }
                }
            }
            HardwareEvent::EncoderPress { .. } => {
                lines = message_lines(&[
                    "Shutting down...",
                    "",
                    "Unplug power once",
                    "the screen is dark.",
                ]);
                show(&mut devices, &lines, true, &mut shown, &mut drawn);
                if let Err(error) = shut_down() {
                    eprintln!("shutdown from the restore screen failed: {error}");
                    lines = message_lines(&[
                        "Shut down failed.",
                        "",
                        "Unplug power to",
                        "turn it off.",
                    ]);
                    drawn = false;
                }
            }
            _ => {}
        }
    }
}

fn show(
    devices: &mut HardwareFault,
    lines: &[String],
    screen_on: bool,
    shown: &mut Option<bool>,
    drawn: &mut bool,
) {
    let Some(oled) = devices.oled_mut() else {
        return;
    };
    if *shown != Some(screen_on) {
        let switched = if screen_on {
            oled.set_master_contrast(FAULT_SCREEN_CONTRAST)
                .and_then(|()| oled.display_on())
        } else {
            oled.display_off()
        };
        if switched.is_ok() {
            *shown = Some(screen_on);
        }
        *drawn = false;
    }
    if screen_on && !*drawn {
        let mut frame = vec![0_u8; OLED_FRAME_BYTES];
        fault_oled_frame_into(HEADER, lines, &mut frame, true);
        let _ = oled.write_frame(&frame);
        *drawn = true;
    }
}

fn other(choice: Choice) -> Choice {
    match choice {
        Choice::Restore => Choice::ShutDown,
        Choice::ShutDown => Choice::Restore,
    }
}

fn choice_lines(choice: Choice, note: Option<&str>) -> Vec<String> {
    let mark = |item| if choice == item { "> " } else { "  " };
    let (first, second) = match note {
        Some(note) => (note, "Check the SD card."),
        None => ("My saved settings", "won't load."),
    };
    vec![
        first.to_string(),
        second.to_string(),
        "Presets are safe.".to_string(),
        String::new(),
        format!("{}Restore factory", mark(Choice::Restore)),
        format!("{}Shut down", mark(Choice::ShutDown)),
        "Turn, then press.".to_string(),
    ]
}

fn message_lines(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| line.to_string()).collect()
}

fn restore(store_dir: &Path) -> Result<(), String> {
    let factory = recovery::factory_documents()?;
    let replaced = recovery::restore_factory_files(
        store_dir,
        &factory,
        &playback_runtime::fresh_preset_name(),
    )?;
    eprintln!("restored factory {}", replaced.join(" and "));
    crate::pi_boot_config::read_composed_runtime_config(store_dir)
        .map(|_| ())
        .map_err(|error| format!("settings still won't load: {error}"))
}

fn shut_down() -> Result<(), String> {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        match crate::orange_reboot::request_shutdown() {
            crate::orange_reboot::OrangePowerRequestOutcome::Accepted => Ok(()),
            outcome => Err(format!("power request {outcome:?}")),
        }
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        crate::raspberry_power::power_off()
    }
}
