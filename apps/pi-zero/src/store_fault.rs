//! The screen shown when the saved settings won't load: the user picks the last patch backup that
//! still loads (when the patch is the broken file), Restore factory settings (only the broken file
//! is replaced, and kept beside it) or Shut down.

pub(crate) mod recovery;
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
    LastBackup,
    Factory,
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
    #[cfg(board_orange)]
    crate::shutdown_signal::restore_default_handlers();
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
    let choices = available_choices(store_dir);
    let mut choice = choices[0];
    let mut lines = choice_lines(&choices, choice, None);
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
                choice = step(&choices, choice, delta);
                lines = choice_lines(&choices, choice, None);
            }
            HardwareEvent::EncoderPress { .. } if choice != Choice::ShutDown => {
                match restore(store_dir, choice) {
                    Ok(()) => {
                        let first = if choice == Choice::LastBackup {
                            "The last backup"
                        } else {
                            "Factory settings"
                        };
                        lines = message_lines(&[
                            first,
                            "is back; the old",
                            "file was kept.",
                            "",
                            "Starting again...",
                        ]);
                        show(&mut devices, &lines, true, &mut shown, &mut drawn);
                        thread::sleep(RESTART_PAUSE);
                        std::process::exit(1);
                    }
                    Err(error) => {
                        eprintln!("restore failed: {error}");
                        lines = choice_lines(&choices, choice, Some("Restore failed."));
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

fn available_choices(store_dir: &Path) -> Vec<Choice> {
    let backup = recovery::factory_documents()
        .ok()
        .and_then(|factory| recovery::loadable_patch_backup(store_dir, &factory));
    let mut choices = Vec::new();
    if backup.is_some() {
        choices.push(Choice::LastBackup);
    }
    choices.extend([Choice::Factory, Choice::ShutDown]);
    choices
}

fn step(choices: &[Choice], choice: Choice, delta: i8) -> Choice {
    let count = choices.len() as isize;
    let index = choices.iter().position(|item| *item == choice).unwrap_or(0) as isize;
    choices[(index + isize::from(delta.signum())).rem_euclid(count) as usize]
}

fn choice_lines(choices: &[Choice], choice: Choice, note: Option<&str>) -> Vec<String> {
    let (first, second) = match note {
        Some(note) => (note, "Check the SD card."),
        None => ("My saved settings", "won't load."),
    };
    let mut lines = vec![
        first.to_string(),
        second.to_string(),
        "Presets are safe.".to_string(),
    ];
    if choices.len() < 3 {
        lines.push(String::new());
    }
    lines.extend(choices.iter().map(|item| {
        let mark = if *item == choice { "> " } else { "  " };
        let label = match item {
            Choice::LastBackup => "Last backup",
            Choice::Factory => "Restore factory",
            Choice::ShutDown => "Shut down",
        };
        format!("{mark}{label}")
    }));
    lines.push("Turn, then press.".to_string());
    lines
}

fn message_lines(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| line.to_string()).collect()
}

fn restore(store_dir: &Path, choice: Choice) -> Result<(), String> {
    let factory = recovery::factory_documents()?;
    let stamp = playback_runtime::fresh_preset_name();
    if choice == Choice::LastBackup {
        let backup = recovery::loadable_patch_backup(store_dir, &factory)
            .ok_or("no patch backup loads any more")?;
        recovery::restore_patch_backup(store_dir, &backup, &stamp)?;
        eprintln!("restored the last patch backup that loads");
    } else {
        let replaced = recovery::restore_factory_files(store_dir, &factory, &stamp)?;
        eprintln!("restored factory {}", replaced.join(" and "));
    }
    crate::pi_boot_config::read_composed_runtime_config(store_dir)
        .map(|_| ())
        .map_err(|error| format!("settings still won't load: {error}"))
}

fn shut_down() -> Result<(), String> {
    #[cfg(board_orange)]
    {
        match crate::orange_reboot::request_shutdown() {
            crate::orange_reboot::OrangePowerRequestOutcome::Accepted => Ok(()),
            outcome => Err(format!("power request {outcome:?}")),
        }
    }
    #[cfg(any(board_raspberry, board_host))]
    {
        crate::raspberry_power::power_off()
    }
}
